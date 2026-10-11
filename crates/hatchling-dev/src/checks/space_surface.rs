//! Space-surface ratchet: the space store has no bulk-export
//! op.
//!
//! The vault's rule generalizes: a store the agent reaches through the
//! broker must not offer a primitive that enumerates or dumps it, because
//! sync is exactly the feature that would abuse one. This check
//! fails the gate if:
//!
//! - `SpaceOp` gains a variant outside the allowlist (the protocol
//!   surface). Sync is on it deliberately: a pull or a conflicts
//!   read returns *one named space's* items to a member of that space, and
//!   nothing reaches across spaces;
//! - the broker's `space_ops.rs` or `device.rs` gains a public or
//!   crate-public fn outside its allowlist (the store, handler, and device
//!   endpoint surface): a `list_spaces` or `export` there would be one step
//!   from the wire.
//!
//! Like the vault ratchet, the scan is syntactic, so tests that pin the
//! *rejection* of a list op and docs that describe its *absence* do not
//! trip it.

use std::path::Path;

use super::Finding;
use super::vault_surface::enum_variants;

/// The only space protocol variants: create, join, single-item access by
/// key, tasks, and pairing, one space's sync and conflict log,
/// and single-media access. None of them reaches across spaces.
const SPACE_OP_VARIANTS: [&str; 13] = [
    "Create",
    "Join",
    "Put",
    "Get",
    "Remove",
    "AddTask",
    "RunTask",
    "Pair",
    "SyncPull",
    "SyncPush",
    "Conflicts",
    "PutMedia",
    "GetMedia",
];

/// The only public functions in the broker's space ops: the store's
/// single-entry methods, the session constructor, the pairing table, and
/// the handler.
const SPACE_FNS: [&str; 20] = [
    "new",
    "insert",
    "put",
    "get",
    "put_task",
    "get_task",
    "put_item",
    "get_item",
    "remove_item",
    "put_conflict",
    "get_conflict",
    "remove_conflict",
    "put_media",
    "get_media",
    "put_device",
    "get_device",
    "broker_key",
    "for_session",
    "pairings",
    "handle_space_op",
];

/// The only public functions on the device endpoint.
const DEVICE_FNS: [&str; 6] = [
    "bind",
    "peer_uid",
    "serve",
    "open_device_endpoint",
    "serve_device",
    "pairing_refusals",
];

/// Run the space-surface ratchet.
///
/// # Errors
///
/// Returns [`Error`](crate::Error) if a source cannot be read.
pub fn run(root: &Path) -> Result<Vec<Finding>, crate::Error> {
    let mut findings = check_protocol_variants(root)?;
    findings.extend(check_fns(root, "space_ops.rs", &SPACE_FNS)?);
    findings.extend(check_fns(root, "device.rs", &DEVICE_FNS)?);
    Ok(findings)
}

fn check_protocol_variants(root: &Path) -> Result<Vec<Finding>, crate::Error> {
    let path = root.join("crates/hatchling-core/src/messages.rs");
    let source = std::fs::read_to_string(&path).map_err(crate::Error::Io)?;
    Ok(enum_variants(&source, "SpaceOp")
        .into_iter()
        .filter(|variant| !SPACE_OP_VARIANTS.contains(&variant.as_str()))
        .map(|variant| {
            Finding::new(
                "space_surface",
                format!(
                    "`SpaceOp` has variant `{variant}` outside the allowlist {SPACE_OP_VARIANTS:?}; \
                     the space protocol must not grow a bulk-export op",
                ),
            )
        })
        .collect())
}

/// The public and crate-public fns of the broker's `file`, outside
/// `allowed`.
fn check_fns(root: &Path, file: &str, allowed: &[&str]) -> Result<Vec<Finding>, crate::Error> {
    let path = root.join("crates/hatchling-broker/src").join(file);
    let source = std::fs::read_to_string(&path).map_err(crate::Error::Io)?;
    Ok(source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            [
                "pub fn ",
                "pub const fn ",
                "pub(crate) fn ",
                "pub(crate) const fn ",
            ]
            .iter()
            .find_map(|prefix| trimmed.strip_prefix(prefix))
        })
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|name| !allowed.contains(&name.as_str()))
        .map(|name| {
            Finding::new(
                "space_surface",
                format!(
                    "the broker's {file} has public fn `{name}` outside the allowlist {allowed:?}; \
                     the space store must not grow a bulk-export method",
                ),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN_MESSAGES: &str = r"
pub enum SpaceOp {
    Create { name: String },
    Join { space: String },
    Put { space: String },
    Get { space: String },
    Remove { space: String },
    AddTask { space: String },
    RunTask { space: String },
}
";

    const CLEAN_OPS: &str = r"
impl Store {
    pub const fn new() {}
    pub fn insert() {}
    pub fn get_item() {}
    fn private_helper() {}
}
pub fn handle_space_op() {}
";

    fn tree(name: &str, messages: &str, ops: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "hatchling-dev-spacesurf-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let core = root.join("crates/hatchling-core/src");
        let broker = root.join("crates/hatchling-broker/src");
        std::fs::create_dir_all(&core).unwrap();
        std::fs::create_dir_all(&broker).unwrap();
        std::fs::write(core.join("messages.rs"), messages).unwrap();
        std::fs::write(broker.join("space_ops.rs"), ops).unwrap();
        std::fs::write(broker.join("device.rs"), CLEAN_DEVICE).unwrap();
        root
    }

    const CLEAN_DEVICE: &str = r"
impl DeviceEndpoint {
    pub fn bind() {}
    pub fn serve() {}
}
pub fn serve_device() {}
fn pair() {}
";

    #[test]
    fn a_clean_tree_passes() {
        let root = tree("clean", CLEAN_MESSAGES, CLEAN_OPS);
        assert_eq!(run(&root).unwrap(), Vec::new());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_list_op_on_the_wire_is_a_finding() {
        let messages =
            CLEAN_MESSAGES.replace("    Join {", "    List { space: String },\n    Join {");
        let root = tree("wire", &messages, CLEAN_OPS);
        let findings = run(&root).unwrap();
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings.iter().all(|f| f.check == "space_surface"));
        assert!(findings.iter().any(|f| f.message.contains("`List`")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_export_method_in_the_store_is_a_finding() {
        for (name, ops) in [
            ("pubfn", format!("{CLEAN_OPS}pub fn export_all() {{}}\n")),
            (
                "constfn",
                format!("{CLEAN_OPS}    pub const fn list_spaces() {{}}\n"),
            ),
        ] {
            let root = tree(name, CLEAN_MESSAGES, &ops);
            let findings = run(&root).unwrap();
            assert_eq!(findings.len(), 1, "{name}: {findings:?}");
            assert!(
                findings
                    .iter()
                    .any(|f| f.message.contains("`export_all`")
                        || f.message.contains("`list_spaces`"))
            );
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn a_missing_source_is_an_error_not_a_pass() {
        let root = std::env::temp_dir().join(format!(
            "hatchling-dev-spacesurf-{}-missing",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        assert!(run(&root).is_err());
        for file in ["space_ops.rs", "device.rs"] {
            let partial = tree(&format!("partial-{file}"), CLEAN_MESSAGES, CLEAN_OPS);
            std::fs::remove_file(partial.join("crates/hatchling-broker/src").join(file)).unwrap();
            assert!(run(&partial).is_err(), "{file}");
            let _ = std::fs::remove_dir_all(&partial);
        }
    }

    #[test]
    fn an_export_on_the_device_endpoint_or_a_crate_public_one_is_a_finding() {
        for (name, ops, device) in [
            (
                "device",
                String::from(CLEAN_OPS),
                format!("{CLEAN_DEVICE}pub fn dump_devices() {{}}\n"),
            ),
            (
                "crate",
                format!("{CLEAN_OPS}pub(crate) fn export_store() {{}}\n"),
                String::from(CLEAN_DEVICE),
            ),
        ] {
            let root = tree(name, CLEAN_MESSAGES, &ops);
            std::fs::write(root.join("crates/hatchling-broker/src/device.rs"), device).unwrap();
            let findings = run(&root).unwrap();
            assert_eq!(findings.len(), 1, "{name}: {findings:?}");
            let message = findings
                .first()
                .map(|f| f.message.clone())
                .unwrap_or_default();
            assert!(
                message.contains("`dump_devices`") || message.contains("`export_store`"),
                "{message}"
            );
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn the_sync_ops_are_on_the_allowlist_by_name() {
        for op in [
            "Pair",
            "SyncPull",
            "SyncPush",
            "Conflicts",
            "PutMedia",
            "GetMedia",
        ] {
            assert!(SPACE_OP_VARIANTS.contains(&op), "{op}");
        }
        assert!(!SPACE_OP_VARIANTS.contains(&"List"));
        assert!(!SPACE_FNS.contains(&"drop") && DEVICE_FNS.len() == 6);
    }
}
