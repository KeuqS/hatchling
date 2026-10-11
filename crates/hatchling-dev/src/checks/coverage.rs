//! Coverage floor: scheduled / on-demand, never per-push.
//!
//! Delegates measurement to `cargo llvm-cov` and lets its
//! `--fail-under-lines` enforce
//! [`COVERAGE_FLOOR`](crate::config::COVERAGE_FLOOR). Requires the
//! `llvm-tools-preview` rustup component where the check runs.

use std::path::Path;
use std::time::Duration;

use super::Finding;
use crate::Error;
use crate::config::COVERAGE_FLOOR;

/// Bound for `cargo llvm-cov --workspace`: coverage runs the
/// whole test suite under instrumentation, 20 minutes is the
/// fail-closed outer bound.
const COVERAGE_TIMEOUT: Duration = Duration::from_secs(1200);

/// Run the coverage suite and report whether the floor held.
///
/// # Errors
///
/// Returns [`Error`](crate::Error) if `cargo llvm-cov` cannot run
/// (it needs the `llvm-tools-preview` rustup component).
pub fn run(root: &Path) -> Result<Vec<Finding>, Error> {
    // `cargo llvm-cov` expects a percentage like "82.5".
    let floor_pct = format!("{:.1}", COVERAGE_FLOOR * 100.0);
    // Bounded: unlike the mutation runners, the coverage
    // floor is enforced by the tool's own `--fail-under-lines`, so a
    // non-zero exit IS a floor failure and surfaces as `ToolFailed`.
    let driven = super::driver::drive(&super::driver::Spec {
        program: "cargo",
        args: &[
            "llvm-cov",
            "--workspace",
            "--fail-under-lines",
            floor_pct.as_str(),
        ],
        current_dir: Some(root),
        env: &[],
        timeout: COVERAGE_TIMEOUT,
    })?;
    if !driven.status.success() {
        return Err(Error::ToolFailed {
            tool: String::from("cargo llvm-cov"),
            status: driven.status.to_string(),
        });
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_root(name: &str) -> std::path::PathBuf {
        // Never `std::env::temp_dir()`: it follows TMPDIR, and under
        // `cargo mutants` TMPDIR points inside the workspace scratch dir,
        // so a temp_dir()-based fake root can sit inside the real workspace
        // tree and `cargo` walks up to the real Cargo.toml. `/tmp` directly
        // can never be inside a workspace; the dir stays empty so the small
        // tmpfs is no concern.
        let root =
            Path::new("/tmp").join(format!("hatchling-dev-cov-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn run_fails_when_the_tool_cannot_run() {
        // `cargo llvm-cov` cannot succeed in a directory that is not a
        // cargo workspace; the failure must surface as an error, never
        // as an empty finding list. Kills `run -> Ok(vec![])` and the
        // dropped `!` on the status check.
        let root = fake_root("no-workspace");
        let err = run(&root).unwrap_err();
        assert!(
            err.to_string().contains("cargo llvm-cov"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
