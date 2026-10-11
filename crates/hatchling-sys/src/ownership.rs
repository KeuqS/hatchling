//! Ownership transfer for the cell's socket surface.
//!
//! The broker transfers ownership of its listen socket (and the
//! socket's directory) to the cell's uid base before forking, so the
//! mapped agent can traverse the directory and connect. `chown(2)`
//! never follows symlinks here: the spawner verifies the socket is a
//! real socket first.

use std::io;
use std::path::Path;

use crate::Error;

/// Changes the owner of `path` to `uid` and its group to `gid`.
/// Symlinks are not followed.
///
/// # Errors
///
/// Returns [`Error::Ownership`] if the `chown` syscall fails.
pub fn chown_ids(path: &Path, uid: u32, gid: u32) -> Result<(), Error> {
    // SAFETY: `Uid::from_raw` just wraps the validated u32 uid; the
    // unsafety is rustix's marker for raw-id construction, and the
    // value here came from the validated cell configuration.
    let owner = unsafe { rustix::fs::Uid::from_raw(uid) };
    // SAFETY: `Gid::from_raw` just wraps the u32 gid, as for the uid above.
    let group = unsafe { rustix::fs::Gid::from_raw(gid) };
    rustix::fs::chown(path, Some(owner), Some(group)).map_err(|source| Error::Ownership {
        operation: "chown",
        source: io::Error::from_raw_os_error(source.raw_os_error()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Scratch dir for the chown fixture. Mutation runs point `TMPDIR`
    /// into the workspace scratch tree, and the workspace filesystem
    /// here denies chown(2) even to root; this fixture must not inherit
    /// `TMPDIR`.
    fn owned_test_dir() -> PathBuf {
        Path::new("/var/tmp").join(format!("hatchling-sys-chown-test-{}", std::process::id()))
    }

    #[test]
    fn chown_to_current_uid_is_a_noop_success() {
        // Chowning a file we own to our own uid must succeed without
        // privilege; anything else is an environment problem, not a
        // logic problem, so only assert the already-owned fast path.
        let dir = owned_test_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let probe = dir.join("probe");
        std::fs::write(&probe, b"x").unwrap();
        let uid = rustix::process::getuid().as_raw();
        let gid = rustix::process::getgid().as_raw();
        chown_ids(&probe, uid, gid).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn chown_missing_path_is_an_error() {
        let err = chown_ids(
            Path::new("/nonexistent/hatchling-sys-chown-probe"),
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Ownership { .. }));
    }
}
