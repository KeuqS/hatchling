//! The broker-owned directory discipline, shared by every store that
//! lives beside the memory store.
//!
//! Factored out of [`MemoryStore`](crate::MemoryStore) so the goals and
//! tracking store applies the exact same rules rather than a
//! copy that could drift:
//!
//! - store directories are owner-only (`0700`), owned by the broker
//!   uid, and validated on every open;
//! - a directory chain is walked one component at a time, each
//!   component `symlink_metadata`-checked before anything beneath it is
//!   touched, so a planted symlink fails closed instead of redirecting
//!   the store;
//! - record files are `0600`, written by temp-write + fsync + rename +
//!   directory fsync, so a crash never leaves a torn record.
//!
//! [`existing_chain`] is the read-side walk: it validates without
//! creating or repairing anything, so a reader never changes the tree.

use std::fs::OpenOptions;
use std::io::Read as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Error;

/// Mode of every store directory: owner-only.
pub const DIR_MODE: u32 = 0o700;
/// Mode of every record file and temp file: owner-only.
pub const FILE_MODE: u32 = 0o600;

/// Makes temp file names unique within this process. Combined with the
/// pid, no two writes share a temp name, so a crashed write's orphan
/// can never collide with a later store.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Creates `path` as an owner-only directory if absent, then validates it.
///
/// Valid means a real directory (never a symlink), mode `0700`, owned
/// by `owner`. The validation always runs — a genuine creation failure
/// surfaces here as [`Error::Io`].
///
/// Concurrent opens of the same root are safe: the mode is pinned
/// unconditionally (not just on create), because a racing opener may
/// read the mode inside another thread's create→chmod window and
/// would otherwise fail closed on a transient mode. `chmod` is
/// idempotent, so all racers converge on `0700`.
///
/// # Errors
///
/// Returns [`Error::NotADirectory`], [`Error::InsecurePermissions`],
/// or [`Error::Io`].
pub fn ensure_owner_only_dir(path: &Path, owner: u32) -> Result<(), Error> {
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => return Err(Error::Io { source }),
    }
    // Reject symlinks BEFORE any chmod: `set_permissions` follows the
    // final component, so chmod-then-validate would chmod through a
    // planted link.
    let metadata = std::fs::symlink_metadata(path).map_err(|source| Error::Io { source })?;
    if !metadata.file_type().is_dir() {
        return Err(Error::NotADirectory {
            path: path.to_path_buf(),
        });
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(DIR_MODE))
        .map_err(|source| Error::Io { source })?;
    let metadata = std::fs::symlink_metadata(path).map_err(|source| Error::Io { source })?;
    if metadata.mode() & 0o777 != DIR_MODE || metadata.uid() != owner {
        return Err(Error::InsecurePermissions {
            path: path.to_path_buf(),
            expected_uid: owner,
        });
    }
    Ok(())
}

/// Walks `root/segments…`, creating each missing component as `0700`,
/// and returns the final directory.
///
/// Every level is created and validated individually: a real
/// directory (never a symlink), owned by `owner`, mode `0700`
/// (repaired when a umask or a pre-existing directory left it
/// different, then re-verified). The walk deliberately avoids
/// `create_dir_all`: that would follow a planted symlink and a chmod
/// sweep would then chmod *through* it. Each component is
/// `symlink_metadata`-checked before anything beneath it is touched.
///
/// Callers pass segments drawn from a separator-free alphabet (or
/// fixed literals), so joining is safe by construction.
///
/// # Errors
///
/// Returns [`Error::NotADirectory`], [`Error::InsecurePermissions`],
/// or [`Error::Io`].
pub fn ensure_chain(root: &Path, segments: &[&str], owner: u32) -> Result<PathBuf, Error> {
    let mut cursor = root.to_path_buf();
    for segment in segments {
        cursor.push(segment);
        match std::fs::create_dir(&cursor) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(Error::Io { source }),
        }
        // `symlink_metadata` never follows the final component: a
        // symlink here is rejected, never traversed.
        let metadata = std::fs::symlink_metadata(&cursor).map_err(|source| Error::Io { source })?;
        if !metadata.file_type().is_dir() {
            return Err(Error::NotADirectory {
                path: cursor.clone(),
            });
        }
        if metadata.uid() != owner {
            return Err(Error::InsecurePermissions {
                path: cursor.clone(),
                expected_uid: owner,
            });
        }
        if metadata.mode() & 0o777 != DIR_MODE {
            // Repair umask-influenced or pre-existing modes, then
            // re-verify: the mode check below asserts the repair.
            std::fs::set_permissions(&cursor, std::fs::Permissions::from_mode(DIR_MODE))
                .map_err(|source| Error::Io { source })?;
            let repaired =
                std::fs::symlink_metadata(&cursor).map_err(|source| Error::Io { source })?;
            if repaired.mode() & 0o777 != DIR_MODE {
                return Err(Error::InsecurePermissions {
                    path: cursor.clone(),
                    expected_uid: owner,
                });
            }
        }
    }
    Ok(cursor)
}

/// The read-side walk: validates `root/segments…` without creating or
/// repairing anything.
///
/// Returns the final directory if the whole chain exists.
///
/// `Ok(None)` means some component is absent: nothing was ever written
/// there, so a reader sees an empty store. A component that exists but
/// is not a real directory, is owned by another uid, or has any mode
/// other than `0700` fails closed — the writer always leaves `0700`
/// behind, so anything else was changed outside the store, and a
/// reader never repairs.
///
/// # Errors
///
/// Returns [`Error::NotADirectory`], [`Error::InsecurePermissions`],
/// or [`Error::Io`].
pub fn existing_chain(
    root: &Path,
    segments: &[&str],
    owner: u32,
) -> Result<Option<PathBuf>, Error> {
    let mut cursor = root.to_path_buf();
    if !existing_owner_only_dir(&cursor, owner)? {
        return Ok(None);
    }
    for segment in segments {
        cursor.push(segment);
        if !existing_owner_only_dir(&cursor, owner)? {
            return Ok(None);
        }
    }
    Ok(Some(cursor))
}

/// One component of [`existing_chain`]: `Ok(false)` when absent,
/// `Ok(true)` when it is a real `0700` directory owned by `owner`.
fn existing_owner_only_dir(path: &Path, owner: u32) -> Result<bool, Error> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(Error::Io { source }),
    };
    if !metadata.file_type().is_dir() {
        return Err(Error::NotADirectory {
            path: path.to_path_buf(),
        });
    }
    if metadata.uid() != owner || metadata.mode() & 0o777 != DIR_MODE {
        return Err(Error::InsecurePermissions {
            path: path.to_path_buf(),
            expected_uid: owner,
        });
    }
    Ok(true)
}

/// Raw bytes of one record file. `Ok(None)` when absent.
///
/// # Errors
///
/// Returns [`Error::Io`] for any failure other than absence.
pub fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(Error::Io { source }),
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| Error::Io { source })?;
    Ok(Some(bytes))
}

/// The regular files directly under `dir` whose names end in `suffix`,
/// for scoped scans. Directories, symlinks, and other names are
/// ignored.
///
/// # Errors
///
/// Returns [`Error::Io`] if the directory cannot be read.
pub fn record_files(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|source| Error::Io { source })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io { source })?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|source| Error::Io { source })?
            .is_file()
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(suffix))
        {
            files.push(path);
        }
    }
    Ok(files)
}

/// Atomic write: temp file (`0600`, unique name), fsync, rename,
/// directory fsync. A crash leaves an orphaned temp file at worst
/// — never a torn record.
///
/// # Errors
///
/// Returns [`Error::Io`]; the temp file is removed on failure.
pub fn write_atomic(dir: &Path, target: &Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write as _;
    let temp = dir.join(format!(
        ".{}.{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let outcome = (|| -> Result<(), Error> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&temp)
            .map_err(|source| Error::Io { source })?;
        file.write_all(bytes)
            .map_err(|source| Error::Io { source })?;
        file.sync_all().map_err(|source| Error::Io { source })?;
        drop(file);
        std::fs::rename(&temp, target).map_err(|source| Error::Io { source })?;
        let dir_fd = std::fs::File::open(dir).map_err(|source| Error::Io { source })?;
        dir_fd.sync_all().map_err(|source| Error::Io { source })?;
        Ok(())
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "hatchling-memory-owner-dir-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn me() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn mode(path: &Path) -> u32 {
        std::fs::symlink_metadata(path).unwrap().mode() & 0o777
    }

    #[test]
    fn existing_chain_reports_absence_without_creating() {
        let root = test_root("absent");
        // No root at all: absent, and still absent afterwards.
        assert_eq!(existing_chain(&root, &["goals", "7"], me()).unwrap(), None);
        assert!(
            !root.exists(),
            "the read-side walk must never create the root"
        );
        // Root present, chain absent: absent, nothing created.
        ensure_owner_only_dir(&root, me()).unwrap();
        assert_eq!(existing_chain(&root, &["goals", "7"], me()).unwrap(), None);
        assert!(!root.join("goals").exists());
        // Half the chain present: still absent, nothing created.
        ensure_chain(&root, &["goals"], me()).unwrap();
        assert_eq!(existing_chain(&root, &["goals", "7"], me()).unwrap(), None);
        assert!(!root.join("goals").join("7").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn existing_chain_returns_the_directory_the_writer_made() {
        let root = test_root("present");
        ensure_owner_only_dir(&root, me()).unwrap();
        let made = ensure_chain(&root, &["tracking", "7"], me()).unwrap();
        assert_eq!(made, root.join("tracking").join("7"));
        assert_eq!(
            existing_chain(&root, &["tracking", "7"], me()).unwrap(),
            Some(made)
        );
        // The empty chain is the root itself.
        assert_eq!(
            existing_chain(&root, &[], me()).unwrap(),
            Some(root.clone())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn existing_chain_never_repairs_a_changed_mode() {
        // The writer always leaves 0700. A reader that finds anything
        // else fails closed and leaves the mode exactly as it found it.
        let root = test_root("mode");
        ensure_owner_only_dir(&root, me()).unwrap();
        let dir = ensure_chain(&root, &["goals", "7"], me()).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o750)).unwrap();
        assert!(matches!(
            existing_chain(&root, &["goals", "7"], me()),
            Err(Error::InsecurePermissions { .. })
        ));
        assert_eq!(mode(&dir), 0o750, "a reader must not chmod");
        // An owner-only mode that is not exactly 0700 also fails.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        assert!(matches!(
            existing_chain(&root, &["goals", "7"], me()),
            Err(Error::InsecurePermissions { .. })
        ));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        // The root is checked too.
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            existing_chain(&root, &["goals", "7"], me()),
            Err(Error::InsecurePermissions { .. })
        ));
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn existing_chain_rejects_the_wrong_owner() {
        let root = test_root("owner");
        ensure_owner_only_dir(&root, me()).unwrap();
        ensure_chain(&root, &["goals"], me()).unwrap();
        // The real tree is owned by this uid; demanding another owner
        // must fail at the first component, the root.
        let other = me().wrapping_add(1);
        let result = existing_chain(&root, &["goals"], other);
        assert!(
            matches!(
                &result,
                Err(Error::InsecurePermissions { path, expected_uid })
                    if *path == root && *expected_uid == other
            ),
            "expected InsecurePermissions at the root, got {result:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn existing_chain_fails_closed_on_a_planted_symlink() {
        let root = test_root("symlink");
        let elsewhere = test_root("symlink-target");
        ensure_owner_only_dir(&root, me()).unwrap();
        ensure_owner_only_dir(&elsewhere, me()).unwrap();
        ensure_chain(&elsewhere, &["7"], me()).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join("goals")).unwrap();
        let result = existing_chain(&root, &["goals", "7"], me());
        assert!(
            matches!(&result, Err(Error::NotADirectory { path }) if *path == root.join("goals")),
            "expected NotADirectory at the link, got {result:?}"
        );
        // A regular file in the chain fails the same way.
        std::fs::remove_file(root.join("goals")).unwrap();
        std::fs::write(root.join("goals"), b"x").unwrap();
        assert!(matches!(
            existing_chain(&root, &["goals", "7"], me()),
            Err(Error::NotADirectory { .. })
        ));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }

    #[test]
    fn existing_chain_reports_errors_other_than_absence() {
        // A component beneath a regular file is ENOTDIR, not absence.
        let root = test_root("enotdir");
        std::fs::write(&root, b"x").unwrap();
        let below = root.join("goals");
        assert!(matches!(
            existing_owner_only_dir(&below, me()),
            Err(Error::Io { .. })
        ));
        let _ = std::fs::remove_file(&root);
    }

    #[test]
    fn record_files_lists_only_regular_files_with_the_suffix() {
        let root = test_root("records");
        ensure_owner_only_dir(&root, me()).unwrap();
        std::fs::write(root.join("a.json"), b"{}").unwrap();
        std::fs::write(root.join("b.txt"), b"{}").unwrap();
        std::fs::create_dir(root.join("c.json")).unwrap();
        std::os::unix::fs::symlink(root.join("a.json"), root.join("d.json")).unwrap();
        let files = record_files(&root, ".json").unwrap();
        assert_eq!(files, vec![root.join("a.json")]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_atomic_leaves_an_owner_only_record_and_no_temp() {
        let root = test_root("atomic");
        ensure_owner_only_dir(&root, me()).unwrap();
        let target = root.join("r.json");
        write_atomic(&root, &target, b"one").unwrap();
        write_atomic(&root, &target, b"two").unwrap();
        assert_eq!(read_bytes(&target).unwrap(), Some(b"two".to_vec()));
        assert_eq!(mode(&target), FILE_MODE);
        let names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("r.json")]);
        assert_eq!(read_bytes(&root.join("absent.json")).unwrap(), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
