//! Dependency-direction guard: `hatchling-dev` is dev-only.
//!
//! No runtime crate may depend on this crate in any dependency section.
//! If it did, audit tooling (or its dependencies) would ship inside the
//! sandboxed agent — widening the supply chain and the binary for no
//! runtime benefit.

use std::path::Path;

use super::{Finding, members, read_toml};
use crate::config::DEV_CRATE_NAME;

const DEP_SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// Fail if any workspace member other than `hatchling-dev` itself
/// names `hatchling-dev` in any dependency section.
///
/// # Errors
///
/// Returns [`Error`](crate::Error) if a member manifest cannot be
/// read or parsed.
pub fn run(root: &Path) -> Result<Vec<Finding>, crate::Error> {
    let mut findings = Vec::new();
    for member in members(root)? {
        if member.name == DEV_CRATE_NAME {
            continue;
        }
        let manifest = member.dir.join("Cargo.toml");
        let table = read_toml(&manifest)?;
        for section in DEP_SECTIONS {
            if table
                .get(section)
                .and_then(|s| s.as_table())
                .is_some_and(|deps| deps.contains_key(DEV_CRATE_NAME))
            {
                findings.push(Finding::new(
                    "dep_direction",
                    format!(
                        "crate `{}` lists `{DEV_CRATE_NAME}` under [{section}]; the dev crate must never be a runtime dependency",
                        member.name,
                    ),
                ));
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn synthetic_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "hatchling-dev-depdir-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("crates/runtime/src")).unwrap();
        std::fs::create_dir_all(root.join("crates/hatchling-dev/src")).unwrap();
        root
    }

    fn write_manifest(dir: &Path, name: &str, deps: &str) {
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{deps}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn catches_runtime_depending_on_dev() {
        let root = synthetic_root("catches");
        write_manifest(
            &root.join("crates/runtime"),
            "runtime",
            "[dependencies]\nhatchling-dev = \"0.1.0\"\n",
        );
        write_manifest(&root.join("crates/hatchling-dev"), "hatchling-dev", "");

        let findings = run(&root).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings.first().map(|f| f.check), Some("dep_direction"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clean_tree_passes() {
        let root = synthetic_root("clean");
        write_manifest(&root.join("crates/runtime"), "runtime", "");
        write_manifest(&root.join("crates/hatchling-dev"), "hatchling-dev", "");

        assert!(run(&root).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
