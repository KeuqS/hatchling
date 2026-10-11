//! A test's Unix socket path under `/tmp`.
//!
//! Test sockets live in `/tmp`, not `TMPDIR`: `sun_path` holds at most
//! 107 bytes, and the mutation runner points `TMPDIR` deep. Nothing else
//! clears `/tmp`, so the guard removes the socket when the test ends,
//! pass or fail: a failing assertion must not leak it.
#![allow(
    clippy::redundant_pub_crate,
    reason = "crate-visible on purpose: test modules across the crate share the guard"
)]

use std::path::Path;

/// A socket path, removed when the guard drops.
pub(crate) struct TestSocket(String);

impl TestSocket {
    /// Guards `path`. A stale socket there is the caller's to clear.
    pub(crate) fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }
}

impl std::ops::Deref for TestSocket {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for TestSocket {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl Drop for TestSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn a_test_socket_is_removed_when_its_guard_drops() {
    let path = format!("/tmp/hatchling-test-guard-{}.sock", std::process::id());
    let _ = std::fs::remove_file(&path);
    let guard = TestSocket::new(path.clone());
    let listener = std::os::unix::net::UnixListener::bind(&guard);
    assert!(listener.is_ok(), "{listener:?}");
    assert!(Path::new(&path).exists());
    assert_eq!(&*guard, path.as_str());
    drop(guard);
    assert!(!Path::new(&path).exists(), "{path} was left behind");
}
