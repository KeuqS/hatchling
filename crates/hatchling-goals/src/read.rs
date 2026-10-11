//! The store layout and its read path, shared by the writer
//! ([`GoalsStore`](crate::GoalsStore)) and the feed's reader
//! ([`FeedReader`](crate::FeedReader)).
//!
//! ```text
//! <root>/goals/<uid>/<goal-id>.json      one 0600 file per goal
//! <root>/tracking/<uid>/log.ndjson       one 0600 append-only log per user
//! ```
//!
//! The uid is the kernel-authenticated peer, never a name the agent
//! chose: one user's goals and log are a different directory from
//! another's, so no op can name its way into them.
//!
//! Nothing in this module writes. The feed reader is read-only by
//! construction partly because this is the only store code it calls
//! (pinned by `the_feed_reader_exposes_no_write_surface`).

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};

use hatchling_memory::owner_dir;

use crate::Error;
use crate::name::GoalId;
use crate::record::{Goal, TrackingEntry};

/// Directory of per-user goal directories.
pub(crate) const GOALS_DIR: &str = "goals";
/// Directory of per-user tracking directories.
pub(crate) const TRACKING_DIR: &str = "tracking";
/// Goal file suffix. Only files with this suffix are goals; temp files
/// and anything else in the directory are ignored.
pub(crate) const GOAL_SUFFIX: &str = ".json";
/// The one tracking log file in a user's tracking directory.
pub(crate) const TRACKING_LOG: &str = "log.ndjson";

/// The file of one goal. The id alphabet has no separator, so joining
/// is safe by construction.
pub(crate) fn goal_path(dir: &Path, id: &GoalId) -> PathBuf {
    dir.join(format!("{}{GOAL_SUFFIX}", id.as_str()))
}

/// Reads one goal file. `Ok(None)` when absent.
///
/// The goal must name itself: a file whose contents carry another id
/// was copied or renamed outside the store, and is corrupt rather than
/// silently served under the wrong name.
pub(crate) fn read_goal(path: &Path, id: &GoalId) -> Result<Option<Goal>, Error> {
    let Some(bytes) = owner_dir::read_bytes(path)? else {
        return Ok(None);
    };
    let goal: Goal = serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt)?;
    if goal.id != *id {
        return Err(Error::Corrupt);
    }
    Ok(Some(goal))
}

/// Every goal in one user's directory, ordered by id.
///
/// A file named like a goal whose name is not a valid id is corrupt,
/// as is any goal file that fails to parse: a listing never silently
/// skips a record.
pub(crate) fn read_goals(dir: &Path) -> Result<Vec<Goal>, Error> {
    let mut goals = Vec::new();
    for path in owner_dir::record_files(dir, GOAL_SUFFIX)? {
        let id = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(GOAL_SUFFIX))
            .and_then(|stem| stem.parse::<GoalId>().ok())
            .ok_or(Error::Corrupt)?;
        if let Some(goal) = read_goal(&path, &id)? {
            goals.push(goal);
        }
    }
    goals.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(goals)
}

/// Counts the goal files in one user's directory.
pub(crate) fn goal_count(dir: &Path) -> Result<usize, Error> {
    Ok(owner_dir::record_files(dir, GOAL_SUFFIX)?.len())
}

/// Streams one tracking log in sequence order, keeping the entries
/// `keep` admits. An absent log is empty.
///
/// Every line must be an entry whose `seq` is its line number: a
/// corrupt line, a gap, or a reordering means the log was changed
/// outside the append path, and the read fails rather than present a
/// false history.
pub(crate) fn scan_tracking(
    path: &Path,
    mut keep: impl FnMut(&TrackingEntry) -> bool,
) -> Result<Vec<TrackingEntry>, Error> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { source }),
    };
    let mut entries = Vec::new();
    let mut expected: u64 = 0;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|source| Error::Io { source })?;
        let entry: TrackingEntry = serde_json::from_str(&line).map_err(|_| Error::Corrupt)?;
        if entry.seq != expected {
            return Err(Error::Corrupt);
        }
        expected = expected.saturating_add(1);
        if keep(&entry) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "hatchling-goals-read-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry_line(seq: u64, at: u64) -> String {
        format!(r#"{{"seq":{seq},"at_secs":{at},"source":"user","text":"t{seq}"}}"#)
    }

    fn write_log(path: &Path, lines: &[String]) {
        let mut file = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    #[test]
    fn scan_keeps_what_the_filter_admits_in_order() {
        let dir = test_dir("scan");
        let log = dir.join(TRACKING_LOG);
        write_log(
            &log,
            &[entry_line(0, 5), entry_line(1, 6), entry_line(2, 7)],
        );
        let all = scan_tracking(&log, |_| true).unwrap();
        assert_eq!(all.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![0, 1, 2]);
        let late = scan_tracking(&log, |e| e.at_secs >= 6).unwrap();
        assert_eq!(late.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![1, 2]);
        assert!(
            scan_tracking(&dir.join("absent"), |_| true)
                .unwrap()
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_refuses_a_gap_a_reorder_or_a_corrupt_line() {
        let dir = test_dir("tamper");
        let log = dir.join(TRACKING_LOG);
        // A gap: entry 1 was cut out.
        write_log(&log, &[entry_line(0, 5), entry_line(2, 7)]);
        assert!(matches!(scan_tracking(&log, |_| true), Err(Error::Corrupt)));
        // A reorder.
        write_log(&log, &[entry_line(1, 5), entry_line(0, 7)]);
        assert!(matches!(scan_tracking(&log, |_| true), Err(Error::Corrupt)));
        // A corrupt line, even one the filter would drop.
        write_log(&log, &[entry_line(0, 5), String::from("{not json")]);
        assert!(matches!(
            scan_tracking(&log, |_| false),
            Err(Error::Corrupt)
        ));
        // A blank line is not an entry.
        write_log(&log, &[entry_line(0, 5), String::new(), entry_line(1, 6)]);
        assert!(matches!(scan_tracking(&log, |_| true), Err(Error::Corrupt)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_reports_errors_other_than_absence() {
        let dir = test_dir("unreadable");
        // The open itself fails, and not with NotFound: a log path
        // beneath a regular file is ENOTDIR. Absence reads as empty;
        // this must not.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        assert!(matches!(
            scan_tracking(&blocker.join(TRACKING_LOG), |_| true),
            Err(Error::Io { .. })
        ));
        // A directory where the log should be opens, then fails to
        // read: not absence, not corrupt.
        let log = dir.join(TRACKING_LOG);
        std::fs::create_dir(&log).unwrap();
        assert!(matches!(
            scan_tracking(&log, |_| true),
            Err(Error::Io { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_goal_file_must_name_itself() {
        let dir = test_dir("selfname");
        let body = r#"{"id":"g1","title":"t","detail":"","status":"open","origin":"user-explicit","created_at_secs":1,"updated_at_secs":1}"#;
        let g1: GoalId = "g1".parse().unwrap();
        let g2: GoalId = "g2".parse().unwrap();
        std::fs::write(goal_path(&dir, &g1), body).unwrap();
        assert_eq!(
            read_goal(&goal_path(&dir, &g1), &g1).unwrap().unwrap().id,
            g1
        );
        // The same bytes under another goal's name are corrupt.
        std::fs::write(goal_path(&dir, &g2), body).unwrap();
        assert!(matches!(
            read_goal(&goal_path(&dir, &g2), &g2),
            Err(Error::Corrupt)
        ));
        assert!(matches!(read_goals(&dir), Err(Error::Corrupt)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_goals_orders_by_id_and_refuses_stray_names() {
        let dir = test_dir("list");
        for id in ["g3", "g1", "g2"] {
            let body = format!(
                r#"{{"id":"{id}","title":"t","detail":"","status":"open","origin":"user-explicit","created_at_secs":1,"updated_at_secs":1}}"#
            );
            std::fs::write(dir.join(format!("{id}.json")), body).unwrap();
        }
        // Temp files and other names are not goals.
        std::fs::write(dir.join(".123.0.tmp"), b"partial").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let ids: Vec<String> = read_goals(&dir)
            .unwrap()
            .into_iter()
            .map(|g| g.id.to_string())
            .collect();
        assert_eq!(ids, vec!["g1", "g2", "g3"]);
        assert_eq!(goal_count(&dir).unwrap(), 3);
        // A goal-suffixed file whose stem is not a valid id is corrupt.
        std::fs::write(dir.join("bad name.json"), b"{}").unwrap();
        assert!(matches!(read_goals(&dir), Err(Error::Corrupt)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unparseable_goal_is_corrupt_and_an_absent_one_is_none() {
        let dir = test_dir("corrupt");
        let g1: GoalId = "g1".parse().unwrap();
        assert!(read_goal(&goal_path(&dir, &g1), &g1).unwrap().is_none());
        std::fs::write(goal_path(&dir, &g1), b"{not json").unwrap();
        assert!(matches!(
            read_goal(&goal_path(&dir, &g1), &g1),
            Err(Error::Corrupt)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
