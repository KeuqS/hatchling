//! Test scratch directories shared by the keyring modules.

use std::path::{Path, PathBuf};

/// A test's scratch directory, created `0700` and removed on drop, so a
/// failing assertion cannot leak it. Unique per test name and process.
pub struct TestDir(PathBuf);

impl std::ops::Deref for TestDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh, existing, owner-only directory for `name`.
///
/// # Panics
///
/// If the directory cannot be made: the test cannot run without it.
#[must_use]
pub fn test_dir(name: &str) -> TestDir {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = std::env::temp_dir().join(format!(
        "hatchling-keyring-test-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    TestDir(dir)
}
