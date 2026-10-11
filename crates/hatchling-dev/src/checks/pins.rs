//! Toolchain and lockfile pin audits.
//!
//! Reproducible builds start with pinned inputs: the stable toolchain
//! (`rust-toolchain.toml`) and a `Cargo.lock` that matches the
//! manifests. The MSRV is declared separately (`rust-version` in the
//! workspace manifest) and is exercised by CI, not pinned here.

use std::path::Path;
use std::time::Duration;

use super::{Finding, read_toml};
use crate::config::PINNED_TOOLCHAIN;

/// Bound for `cargo metadata --locked`: metadata resolution
/// is fast, 2 minutes is generous.
const METADATA_TIMEOUT: Duration = Duration::from_secs(120);

/// Verify the toolchain pin and the lockfile's consistency with the
/// manifests.
///
/// # Errors
///
/// Returns [`Error`](crate::Error) if `rust-toolchain.toml` cannot be
/// read or parsed, or if the `cargo metadata` subprocess cannot run.
pub fn run(root: &Path) -> Result<Vec<Finding>, crate::Error> {
    let mut findings = Vec::new();

    let toolchain = read_toml(&root.join("rust-toolchain.toml"))?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if channel != PINNED_TOOLCHAIN {
        findings.push(Finding::new(
            "pins",
            format!("rust-toolchain.toml pins channel `{channel}`, expected `{PINNED_TOOLCHAIN}`"),
        ));
    }

    let lock = root.join("Cargo.lock");
    if lock.is_file() {
        // `cargo metadata --locked` fails when the lockfile is stale
        // relative to the manifests: the real freshness check.
        // Bounded ; only the exit status is used.
        let manifest = root.join("Cargo.toml");
        let manifest_str = manifest.to_string_lossy();
        let driven = super::driver::drive(&super::driver::Spec {
            program: "cargo",
            args: &[
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--manifest-path",
                manifest_str.as_ref(),
            ],
            current_dir: Some(root),
            env: &[],
            timeout: METADATA_TIMEOUT,
        })?;
        if !driven.status.success() {
            findings.push(Finding::new(
                "pins",
                String::from(
                    "Cargo.lock is stale relative to the manifests (`cargo metadata --locked` failed); run `cargo generate-lockfile` or build once",
                ),
            ));
        }
    } else {
        findings.push(Finding::new(
            "pins",
            String::from("Cargo.lock is missing; the build is not reproducible"),
        ));
    }

    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_wrong_toolchain_channel() {
        let root =
            std::env::temp_dir().join(format!("hatchling-dev-pins-{}-channel", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"9.9.9\"\n",
        )
        .unwrap();

        let findings = run(&root).unwrap();
        assert!(
            findings.iter().any(|f| f.message.contains("9.9.9")),
            "expected a channel finding, got {findings:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
