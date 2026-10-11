//! One-shot vault operator actions: master-key loading, plaintext
//! migration, and key rotation.
//!
//! These are not session ops — they run before the broker serves (or
//! via the `migrate-vault` / `rekey-vault` subcommands) and never touch
//! the protocol. They are the only code paths that enumerate the store
//! directory, and they transform files in place without returning
//! secret material to any caller.

use std::io::Write as _;
use std::path::Path;

use hatchling_vault::{FileKey, KeySource, MasterKey, MigrateReport, RekeyReport, SealedVault};

use crate::{Error, VaultConfig};

/// Loads the broker's master key from the configured key file,
/// provisioning it with fresh `OsRng` bytes on first run.
///
/// Fail-closed on any key-file problem: the broker never serves with
/// a missing-but-uncreatable, unreadable, mispermissioned, or corrupt
/// key. See [`FileKey`] for the threat model and the operational
/// hazard of deleting the key file.
///
/// # Errors
///
/// Returns [`Error::Vault`] if the key cannot be loaded or created.
pub fn load_master_key(vault_config: &VaultConfig) -> Result<MasterKey, Error> {
    // With a keyring the key file no longer holds the key:
    // the keyring module unlocks it through the vault's tier.
    crate::vault_keyring::unlock(
        vault_config,
        &crate::vault_keyring::KeyringConfig::from_env()?,
    )
}

/// Validates that the sealed vault opens under `key`.
///
/// Checks the root's `0700`/owner discipline plus a readable store.
/// Called once at startup so a misconfigured vault stops the broker
/// before it binds, not on the first agent connection.
///
/// # Errors
///
/// Returns [`Error::Vault`] if the vault root cannot be opened.
pub fn validate_sealed_vault(vault_config: &VaultConfig, key: &MasterKey) -> Result<(), Error> {
    SealedVault::open(&vault_config.root, key)
        .map(|_| ())
        .map_err(|source| Error::Vault { source })
}

/// One-shot migration of a plaintext store to sealed envelopes.
///
/// Operator action, run once when adopting the sealed vault. See
/// [`hatchling_vault::migrate_plaintext`] for the exact semantics
/// (magic-bearing files are verified, not assumed; interruption is
/// resumed by re-running).
///
/// # Errors
///
/// Returns [`Error::Vault`] if the store cannot be migrated.
pub fn migrate_vault(vault_config: &VaultConfig, key: &MasterKey) -> Result<MigrateReport, Error> {
    hatchling_vault::migrate_plaintext(&vault_config.root, key)
        .map_err(|source| Error::Vault { source })
}

/// One-shot master-key rotation.
///
/// `new_key_file` holds (or will hold) the replacement key: it is
/// created with fresh randomness if absent, via the same `0600`
/// discipline as the broker's own key file. The path is the
/// operator's explicit choice and may be relative to the operator's
/// cwd; the daemon's configured key path stays absolute-only. See
/// [`hatchling_vault::rekey`] for the exact semantics (two-phase,
/// resumable).
///
/// # Errors
///
/// Returns [`Error::Vault`] if the new key cannot be loaded/created
/// or the rotation fails.
pub fn rekey_vault(
    vault_config: &VaultConfig,
    old_key: &MasterKey,
    new_key_file: &Path,
) -> Result<RekeyReport, Error> {
    let new_key = FileKey::new(new_key_file.to_path_buf())
        .load_or_create()
        .map_err(|source| Error::Vault { source })?;
    hatchling_vault::rekey(&vault_config.root, old_key, &new_key)
        .map_err(|source| Error::Vault { source })
}

/// Runs the `migrate-vault` operator subcommand.
///
/// Loads the master key and encrypts a plaintext store into sealed
/// envelopes in place. Prints the migration report to stdout (via
/// `io::Write`, not `println!` — the workspace denies `print_stdout`).
///
/// # Errors
///
/// Returns [`Error`] from key loading or migration.
pub fn run_migrate_vault(config: &crate::Config) -> Result<(), Error> {
    let key = load_master_key(&config.vault)?;
    let report = migrate_vault(&config.vault, &key)?;
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(
        stdout,
        "migrate-vault: encrypted {} secret(s), skipped {} file(s)",
        report.encrypted, report.skipped
    );
    Ok(())
}

/// Runs the `rekey-vault <new-key-file>` operator subcommand.
///
/// Rotates the master key, creating the new key file with fresh
/// randomness if absent. Prints the rekey report to stdout.
///
/// # Errors
///
/// Returns [`Error`] from key loading/creation or rotation.
pub fn run_rekey_vault(config: &crate::Config, new_key_file: &Path) -> Result<(), Error> {
    let old_key = load_master_key(&config.vault)?;
    let report = rekey_vault(&config.vault, &old_key, new_key_file)?;
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(
        stdout,
        "rekey-vault: re-encrypted {} secret(s) under the new key, skipped {} file(s)",
        report.rekeyed, report.skipped
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use hatchling_vault::{Secret, SecretName};

    use super::*;

    #[test]
    fn the_master_key_loads_the_same_each_start() {
        let dir = std::env::temp_dir().join(format!("hatchling-vault-ops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let root = dir.join("vault");
        let vault = VaultConfig {
            key_file: VaultConfig::default_key_file(&root),
            root,
            grants: Vec::new(),
        };
        let first = load_master_key(&vault).unwrap();
        validate_sealed_vault(&vault, &first).unwrap();
        let name: SecretName = "probe".parse().unwrap();
        SealedVault::open(&vault.root, &first)
            .unwrap()
            .store(&name, &Secret::new(b"p".to_vec()).unwrap())
            .unwrap();
        let again = load_master_key(&vault).unwrap();
        assert!(
            SealedVault::open(&vault.root, &again)
                .unwrap()
                .fetch(&name)
                .is_ok(),
            "the same key, again"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
