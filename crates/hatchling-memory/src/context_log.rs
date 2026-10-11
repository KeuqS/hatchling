//! The append-only broker context log.
//!
//! The agent can append entries through mediated operations and read
//! back its own session's entries. It cannot rewrite prior records:
//! the only mutating operation is append, and entries are
//! newline-delimited JSON in a file the agent never touches directly.
//!
//! Entries are namespaced by the broker-assigned session id, so one
//! session cannot read another's log by naming it.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::item::SessionId;
use crate::{Error, MemoryStore};

/// Maximum bytes of one context payload: the same bound as memory
/// item content. The log is append-only, so an unbounded payload is a
/// disk-fill vector; nothing useful exceeds this.
pub const MAX_CONTEXT_PAYLOAD_BYTES: usize = 32 * 1024;

/// What kind of event a context entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextKind {
    /// A tool call was issued.
    ToolCall,
    /// A tool result arrived (untrusted data).
    ToolResult,
    /// A confirmation was requested or decided.
    Confirmation,
    /// A memory write or recall happened.
    MemoryOp,
    /// A decision the session recorded. Protected under compaction:
    /// decisions are never silently dropped.
    Decision,
    /// The agent refused an action (a safety constraint). Protected
    /// under compaction: refusals are never silently dropped.
    Refusal,
    /// The user corrected the agent. Protected under compaction: user
    /// corrections are never silently dropped.
    Correction,
    /// A free-form note (compaction summaries land here).
    Note,
}

/// One immutable context entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextEntry {
    /// Monotonic sequence number within the session's log.
    pub seq: u64,
    /// The session this entry belongs to.
    pub session_id: SessionId,
    /// Unix epoch seconds, broker-stamped at append.
    pub at_secs: u64,
    /// What kind of event this is.
    pub kind: ContextKind,
    /// JSON-encoded payload.
    pub payload_json: String,
}

/// Append-only per-session context log.
///
/// Backed by `<memory-root>/context/<session-id>.log`: one JSON
/// object per line. Appends are atomic (temp-write of the single
/// line + append via a single `write` syscall... see below); there is
/// no update, no remove, no truncate.
#[derive(Debug)]
pub struct ContextLog<'s> {
    store: &'s MemoryStore,
}

impl<'s> ContextLog<'s> {
    /// Borrow the memory store's root for context logs.
    #[must_use]
    pub const fn new(store: &'s MemoryStore) -> Self {
        Self { store }
    }

    /// Appends one entry to the session's log, returning it with its
    /// broker-assigned sequence number.
    ///
    /// Sequence numbers are assigned by counting existing entries:
    /// the log is append-only, so the count only grows and `seq`
    /// values are never reused.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem failure, or
    /// [`Error::ContentTooLarge`] when the payload exceeds
    /// [`MAX_CONTEXT_PAYLOAD_BYTES`].
    pub fn append(
        &self,
        session_id: &SessionId,
        kind: ContextKind,
        payload_json: &str,
        now_secs: u64,
    ) -> Result<ContextEntry, Error> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;

        // Fail fast: an unbounded append is a disk-fill vector, and a
        // context entry can never usefully exceed the item bound.
        if payload_json.len() > MAX_CONTEXT_PAYLOAD_BYTES {
            return Err(Error::ContentTooLarge {
                len: payload_json.len(),
                max: MAX_CONTEXT_PAYLOAD_BYTES,
            });
        }
        let dir = self.context_dir()?;
        let path = Self::log_path(&dir, session_id);
        let seq = Self::entry_count(&path)?;
        let entry = ContextEntry {
            seq,
            session_id: session_id.clone(),
            at_secs: now_secs,
            kind,
            payload_json: payload_json.to_owned(),
        };
        let mut line = serde_json::to_vec(&entry).map_err(|_| Error::Corrupt)?;
        // The same frame invariant as memory items: an entry travels in
        // `ContextEntries` as its JSON re-encoded into a JSON string.
        let encoded = crate::item::reply_encoded_len(&line)?;
        if encoded > crate::item::MAX_ENCODED_ITEM_BYTES {
            return Err(Error::EncodedTooLarge {
                len: encoded,
                max: crate::item::MAX_ENCODED_ITEM_BYTES,
            });
        }
        line.push(b'\n');
        // Append-only: open with append + create, write the whole
        // line in one syscall, fsync. There is deliberately no
        // read-modify-write here — nothing already in the file can
        // be altered by an append.
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&path)
            .map_err(|source| Error::Io { source })?;
        file.write_all(&line)
            .map_err(|source| Error::Io { source })?;
        file.sync_all().map_err(|source| Error::Io { source })?;
        tracing::debug!(session = %session_id, seq, "appended context entry");
        Ok(entry)
    }

    /// Reads all entries of one session's log, in sequence order.
    ///
    /// A corrupt line fails the read: silent skipping would let a
    /// tampered log present a false history.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Corrupt`] for an unparseable line, or
    /// [`Error::Io`].
    pub fn read(&self, session_id: &SessionId) -> Result<Vec<ContextEntry>, Error> {
        let dir = self.context_dir()?;
        let path = Self::log_path(&dir, session_id);
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(Error::Io { source }),
        };
        let mut entries = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|source| Error::Io { source })?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: ContextEntry = serde_json::from_str(&line).map_err(|_| Error::Corrupt)?;
            // Belt and suspenders: an entry in session A's file must
            // belong to session A. A mismatch means the file was
            // tampered with outside the append path.
            if entry.session_id != *session_id {
                return Err(Error::Corrupt);
            }
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Directory holding per-session log files, `0700`.
    fn context_dir(&self) -> Result<PathBuf, Error> {
        // Reuse the sessions-dir discipline: broker-internal,
        // owner-only. (A sibling `context/` directory under the same
        // root.)
        let dir = self.store.memory_root().join("context");
        self.store.ensure_dir(&dir)
    }

    /// The log file. The session id alphabet has no separator, so
    /// joining is safe by construction.
    fn log_path(dir: &std::path::Path, session_id: &SessionId) -> PathBuf {
        dir.join(format!("{}.log", session_id.as_str()))
    }

    /// Counts existing entries (lines) in the log file.
    fn entry_count(path: &std::path::Path) -> Result<u64, Error> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(source) => return Err(Error::Io { source }),
        };
        let mut count = 0u64;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|source| Error::Io { source })?;
            if !line.trim().is_empty() {
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(name: &str) -> (MemoryStore, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "hatchling-memory-context-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = MemoryStore::open(&root).unwrap();
        (store, root)
    }

    fn session_id(id: &str) -> SessionId {
        id.parse().unwrap()
    }

    #[test]
    fn append_assigns_monotonic_seq_and_read_roundtrips() {
        let (store, root) = test_store("seq");
        let log = ContextLog::new(&store);
        let id = session_id("sess-log");
        let first = log
            .append(&id, ContextKind::ToolCall, r#"{"tool":"x"}"#, 1_000)
            .unwrap();
        let second = log
            .append(&id, ContextKind::ToolResult, r#"{"ok":true}"#, 1_001)
            .unwrap();
        assert_eq!(first.seq, 0);
        assert_eq!(second.seq, 1);
        // Appending must not disturb earlier entries: re-read shows
        // both, in order, byte-identical.
        let entries = log.read(&id).unwrap();
        assert_eq!(entries, vec![first, second]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sessions_cannot_read_each_others_logs() {
        // Log files are namespaced by the broker-assigned session id.
        // Reading session B's id returns B's entries (empty here) —
        // never A's.
        let (store, root) = test_store("isolation");
        let log = ContextLog::new(&store);
        let a = session_id("sess-a");
        let b = session_id("sess-b");
        log.append(&a, ContextKind::Note, r#""secret-a""#, 1_000)
            .unwrap();
        assert!(log.read(&b).unwrap().is_empty());
        assert_eq!(log.read(&a).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_session_reads_empty_not_error() {
        let (store, root) = test_store("empty");
        let log = ContextLog::new(&store);
        assert!(log.read(&session_id("ghost")).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_line_fails_the_read() {
        // Silent skipping would let a tampered log present a false
        // history; fail instead.
        let (store, root) = test_store("corrupt");
        let log = ContextLog::new(&store);
        let id = session_id("sess-tamper");
        log.append(&id, ContextKind::Note, r#""ok""#, 1_000)
            .unwrap();
        let path = root.join("context").join("sess-tamper.log");
        std::fs::write(&path, "NOT JSON\n").unwrap();
        let err = log.read(&id).unwrap_err();
        assert!(matches!(err, Error::Corrupt));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cross_session_entry_in_file_is_corrupt() {
        // An entry whose session_id disagrees with its file means the
        // file was written outside the append path.
        let (store, root) = test_store("xsession");
        let log = ContextLog::new(&store);
        let id = session_id("sess-victim");
        let other = session_id("sess-other");
        let foreign = ContextEntry {
            seq: 0,
            session_id: other,
            at_secs: 1_000,
            kind: ContextKind::Note,
            payload_json: String::from(r#""foreign""#),
        };
        let dir = root.join("context");
        // Let the log create the dir with the right mode first...
        log.append(&id, ContextKind::Note, r#""placeholder""#, 999)
            .unwrap();
        // ...then simulate a write that bypassed the append path.
        let mut line = serde_json::to_vec(&foreign).unwrap();
        line.push(b'\n');
        std::fs::write(dir.join("sess-victim.log"), line).unwrap();
        let err = log.read(&id).unwrap_err();
        assert!(matches!(err, Error::Corrupt));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn oversize_payload_is_rejected_before_touching_the_log() {
        let (store, root) = test_store("payload-bound");
        let log = ContextLog::new(&store);
        let id = session_id("sess-a");
        let big = "x".repeat(MAX_CONTEXT_PAYLOAD_BYTES + 1);
        let err = log.append(&id, ContextKind::Note, &big, 1_000).unwrap_err();
        assert!(matches!(err, Error::ContentTooLarge { .. }));
        // Nothing was written: the log file was never created.
        assert!(!root.join("context").join("sess-a.log").exists());
        let ok = "x".repeat(MAX_CONTEXT_PAYLOAD_BYTES);
        log.append(&id, ContextKind::Note, &ok, 1_000).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn payload_bound_is_32_kib() {
        assert_eq!(MAX_CONTEXT_PAYLOAD_BYTES, 32 * 1024);
    }

    #[test]
    fn an_unreadable_log_is_an_error_not_an_empty_log() {
        use std::os::unix::fs::PermissionsExt as _;
        // Root ignores file modes; the check needs an ordinary user.
        if rustix::process::geteuid().is_root() {
            return;
        }
        let (store, root) = test_store("unreadable");
        let log = ContextLog::new(&store);
        let sid = session_id("sess-unreadable");
        log.append(&sid, ContextKind::Note, "{\"note\":\"x\"}", 1)
            .unwrap();
        let path = ContextLog::log_path(&log.context_dir().unwrap(), &sid);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let result = log.read(&sid);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(matches!(result, Err(Error::Io { .. })), "{result:?}");
    }
}
