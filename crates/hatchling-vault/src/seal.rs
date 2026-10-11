//! Sealed vault: encryption at rest over the [`Vault`] store.
//!
//! [`SealedVault`] wraps [`Vault`]: every secret is stored as a
//! versioned XChaCha20-Poly1305 envelope, so the bytes on disk are
//! useless without the 256-bit [`MasterKey`]. The key is loaded
//! through a [`KeySource`] ([`FileKey`] ships now; the trait is the
//! extension point for a platform-keyring backend) and is zeroized on
//! drop via the `ZeroizeOnDrop` derive.
//!
//! Honest threat model: the real boundary stays peer authorization
//! plus filesystem permissions. Against a live file-read attacker a
//! file-held key adds nothing — [`FileKey`]'s documentation says so.
//! Encryption at rest defends the *at-rest* class: backups, disk
//! images, volume mounts that escape the broker, accidental
//! disclosure of secret files. The key path is independently
//! configurable so it can live apart from the vault.
//!
//! Fail-closed rules: a sealed vault never falls back to plaintext
//! ([`Error::NotSealed`]); authentication failure is one generic
//! error ([`Error::DecryptFailed`]) whether the key is wrong or the
//! bytes were tampered with; a missing or mispermissioned key file
//! refuses to start.
//!
//! Scrubbing discipline: key material and plaintext are zeroized after
//! use. Ciphertext is not — it is stored on disk by design, so
//! scrubbing a heap copy of it buys nothing.
//!
//! There is deliberately no `list`. [`migrate_plaintext`] and
//! [`rekey`] enumerate the store directory, but they are one-shot
//! operator actions that transform files in place and never return
//! secret material to any caller; the broker never exposes them over
//! the protocol.

use std::io::Read as _;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, AeadInPlace, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::vault::{Vault, ensure_owner_only_dir};
use crate::{Error, Secret, SecretName};
use hatchling_core::MAX_SECRET_BYTES;

/// Envelope magic: identifies a sealed envelope and its version.
/// `HVL1` = hatchling vault, envelope v1. A future algorithm or layout
/// bumps the version; v1 readers reject anything else fail-closed
/// ([`Error::NotSealed`]).
const ENVELOPE_MAGIC: &[u8; 4] = b"HVL1";
/// `XChaCha20` nonce length in bytes. Fresh OS randomness per `store`;
/// the 192-bit width makes random nonces safe.
const NONCE_LEN: usize = 24;
/// Poly1305 authentication tag length in bytes.
const TAG_LEN: usize = 16;
/// Fixed envelope overhead: magic + nonce + tag. Sealed reads bound
/// the allocation at `MAX_SECRET_BYTES + ENVELOPE_OVERHEAD`.
const ENVELOPE_OVERHEAD: usize = ENVELOPE_MAGIC.len() + NONCE_LEN + TAG_LEN;
/// Master-key length in bytes (256 bits).
const MASTER_KEY_LEN: usize = 32;
/// Mode of the key file: owner-only.
const KEY_FILE_MODE: u32 = 0o600;

/// The 256-bit master key for the sealed vault.
///
/// Zeroized on drop via the `ZeroizeOnDrop` derive (declarative: no
/// `Drop` body for a mutant to delete), redacted in `Debug`, and
/// deliberately not `Clone` — key material is never duplicated. The
/// single owner (the broker) hands out borrows; [`SealedVault`]
/// holds `&MasterKey`, never an owned copy.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; 32]);

impl MasterKey {
    /// Generates a fresh key from the OS RNG.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Random`] if the OS provides no randomness —
    /// without it there is no key, so the sealed vault cannot start.
    pub fn generate() -> Result<Self, Error> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|source| Error::Random { source })?;
        Ok(Self(bytes))
    }

    /// Wraps 32 bytes of existing key material (e.g. read from a key
    /// file). The caller vouches for the source; this type's job is
    /// hygiene from here on.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The key bytes, for wrapping under a KEK (`keyring.rs`). Never
    /// leaves the crate.
    pub(crate) const fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    /// The AEAD cipher keyed by this master key.
    fn cipher(&self) -> XChaCha20Poly1305 {
        let key: chacha20poly1305::Key = self.0.into();
        XChaCha20Poly1305::new(&key)
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey([redacted])")
    }
}

/// How a sealed vault obtains its master key.
///
/// The trait is the extension point: [`FileKey`] ships now; a
/// platform-keyring backend can implement this trait later without
/// changing the envelope format or the vault API. Implementations
/// fail closed — no fallback to a weaker source.
pub trait KeySource {
    /// Loads the existing master key, or creates, persists, and
    /// returns a new one. Creation uses OS randomness.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on any load or creation failure.
    fn load_or_create(&self) -> Result<MasterKey, Error>;
}

/// Master key in a `0600` file, created with OS-random bytes if absent.
///
/// The parent directory is created `0700` if absent and validated
/// (`0700`, owned by the broker uid) if present — the same check as
/// the vault root, via the shared helper. An existing key file that
/// is not a regular file, has the wrong mode or owner, or holds
/// anything other than exactly 32 bytes fails closed.
///
/// Honest threat model: this is defense-in-depth for the *at-rest*
/// class — backups, disk images, volume mounts that escape the
/// broker, accidental disclosure of secret files. Against a live
/// file-read attacker it adds nothing over `0600` (the attacker
/// could read the key file as easily as the vault), so keep the key
/// path off the backup set — or on a separate volume — for the full
/// benefit. The real boundary stays peer authorization plus
/// filesystem permissions.
///
/// Operational hazard: the key file is load-bearing. Deleting it
/// does not delete the vault — the next start creates a *different*
/// key, and every existing secret then fails closed
/// ([`Error::DecryptFailed`]), loudly. Back the key file up with the
/// vault, or the sealed data is unrecoverable. First-run creation is
/// the explicit provisioning step; after that, the file must be
/// treated as irreplaceable state.
pub struct FileKey {
    /// The key file path. The parent directory carries the `0700`
    /// discipline; the file itself is `0600`.
    path: PathBuf,
}

impl FileKey {
    /// The key file path. Parent-directory and file discipline are
    /// enforced on load, not here.
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Loads or creates the key, requiring file and parent ownership
    /// by `owner`. Split from [`load_or_create`](KeySource::load_or_create)
    /// so tests exercise the checks hermetically; the production path
    /// always passes the real uid.
    fn load_or_create_as(&self, owner: u32) -> Result<MasterKey, Error> {
        let parent = self.path.parent().ok_or_else(|| Error::KeyLoad {
            path: self.path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "key path has no parent directory",
            ),
        })?;
        ensure_owner_only_dir(parent, owner)?;
        match std::fs::File::open(&self.path) {
            Ok(file) => Self::read_key(&self.path, &file, owner),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => self.create_key(owner),
            Err(source) => Err(Error::KeyLoad {
                path: self.path.clone(),
                source,
            }),
        }
    }

    /// Reads and validates an existing key file: a regular file (never
    /// a symlink), mode `0600`, owned by `owner`, holding exactly 32
    /// bytes. Anything else fails closed.
    fn read_key(path: &Path, file: &std::fs::File, owner: u32) -> Result<MasterKey, Error> {
        // `symlink_metadata`: a symlink key file is never accepted,
        // even one pointing at a well-formed key.
        let metadata = std::fs::symlink_metadata(path).map_err(|source| Error::KeyLoad {
            path: path.to_path_buf(),
            source,
        })?;
        if !metadata.file_type().is_file()
            || metadata.mode() & 0o777 != KEY_FILE_MODE
            || metadata.uid() != owner
        {
            return Err(Error::InsecureKeyFile {
                path: path.to_path_buf(),
            });
        }
        // Bounded read: one byte past the key length, so a longer file
        // is *detected* and rejected, never silently truncated. A key
        // file with trailing junk is corrupt, not "a key plus junk".
        let read_limit = u64::try_from(MASTER_KEY_LEN)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut bytes = Vec::new();
        file.take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|source| Error::KeyLoad {
                path: path.to_path_buf(),
                source,
            })?;
        if bytes.len() != MASTER_KEY_LEN {
            let len = bytes.len();
            // Rejected key material must not linger. (`Vec::zeroize`
            // resets the length, so the count is captured first: the
            // `InvalidKey` error reports how many bytes were found.)
            bytes.zeroize();
            return Err(Error::InvalidKey {
                path: path.to_path_buf(),
                len,
            });
        }
        let mut key = [0u8; MASTER_KEY_LEN];
        key.copy_from_slice(&bytes);
        bytes.zeroize();
        Ok(MasterKey::from_bytes(key))
    }

    /// Creates the key file with fresh OS-random bytes, mode `0600`.
    /// If a concurrent creator wins the race (`AlreadyExists`), reads
    /// the key it wrote instead of failing.
    ///
    /// Creation-leg I/O failures are [`Error::Io`], not
    /// [`Error::KeyLoad`]: `KeyLoad` is the *read* leg
    /// (`load_or_create_as`, `read_key`), and the two legs must stay
    /// distinguishable — silently creating a fresh key over an
    /// unreadable one would orphan every secret sealed under the old
    /// key, so "create on any error" must be a testably different
    /// outcome from "fail the load".
    fn create_key(&self, owner: u32) -> Result<MasterKey, Error> {
        let key = MasterKey::generate()?;
        let opened = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(KEY_FILE_MODE)
            .open(&self.path);
        match opened {
            Ok(mut file) => {
                use std::io::Write as _;
                file.write_all(&key.0)
                    .map_err(|source| Error::Io { source })?;
                file.sync_all().map_err(|source| Error::Io { source })?;
                Ok(key)
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lost the creation race: the winner's key is the key.
                // `key` drops here, zeroized by the derive.
                let file = std::fs::File::open(&self.path).map_err(|source| Error::KeyLoad {
                    path: self.path.clone(),
                    source,
                })?;
                Self::read_key(&self.path, &file, owner)
            }
            Err(source) => Err(Error::Io { source }),
        }
    }
}

impl FileKey {
    /// Loads an existing key file, never creating one: a KEK
    /// that is missing must fail the unlock, not be replaced by a fresh
    /// one that opens nothing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::KeyLoad`] if the file is absent or unreadable,
    /// or the [`load_or_create`](KeySource::load_or_create) checks'
    /// errors for a file that is not a sound key file.
    pub fn load(&self) -> Result<MasterKey, Error> {
        self.load_as(rustix::process::getuid().as_raw())
    }

    fn load_as(&self, owner: u32) -> Result<MasterKey, Error> {
        let file = std::fs::File::open(&self.path).map_err(|source| Error::KeyLoad {
            path: self.path.clone(),
            source,
        })?;
        Self::read_key(&self.path, &file, owner)
    }
}

impl KeySource for FileKey {
    fn load_or_create(&self) -> Result<MasterKey, Error> {
        self.load_or_create_as(rustix::process::getuid().as_raw())
    }
}

/// The sealed vault: [`Vault`] storage with every secret encrypted.
///
/// Borrows the [`MasterKey`]: the key has exactly one owner (the
/// broker), and this type never clones it. Method shapes mirror
/// [`Vault`]'s; `store` seals the envelope and `fetch` opens it.
/// `remove` and `exists` delegate unchanged — they never touch key
/// material.
pub struct SealedVault<'a> {
    vault: Vault,
    key: &'a MasterKey,
}

impl<'a> SealedVault<'a> {
    /// Opens the sealed vault: opens the underlying store, borrowing
    /// `key` for all cryptographic operations.
    ///
    /// No migration happens here. A store that still holds plaintext
    /// must be migrated explicitly via [`migrate_plaintext`];
    /// `fetch` on a non-envelope file fails closed with
    /// [`Error::NotSealed`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotADirectory`],
    /// [`Error::InsecurePermissions`], or [`Error::Io`] exactly as
    /// [`Vault::open`] does.
    pub fn open(root: &Path, key: &'a MasterKey) -> Result<Self, Error> {
        let vault = Vault::open(root)?;
        Ok(Self { vault, key })
    }

    /// Stores `secret` under `name` as a sealed envelope, atomically
    /// replacing any existing value. The secret value is never logged;
    /// overwrites are.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SealFailed`] (defensive: sealing with a valid
    /// key and a fresh nonce cannot fail) or [`Error::Io`] on
    /// filesystem failure.
    pub fn store(&self, name: &SecretName, secret: &Secret) -> Result<(), Error> {
        let envelope = seal_envelope(self.key, secret.as_bytes())?;
        self.vault.store_raw(name, &envelope)?;
        tracing::info!(secret_name = name.as_str(), "stored sealed secret");
        Ok(())
    }

    /// Fetches and unseals the secret stored under `name`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] if no secret is stored under `name`,
    /// [`Error::NotSealed`] if the file is not an envelope (never read
    /// as plaintext), [`Error::Malformed`] for a truncated envelope,
    /// [`Error::DecryptFailed`] for a wrong key or tampered bytes
    /// (indistinguishable by design), [`Error::TooLarge`] for a
    /// foreign-grown file, or [`Error::Io`].
    pub fn fetch(&self, name: &SecretName) -> Result<Secret, Error> {
        let envelope = self
            .vault
            .fetch_raw(name, MAX_SECRET_BYTES + ENVELOPE_OVERHEAD)?;
        let secret = open_envelope(self.key, name, &envelope)?;
        tracing::debug!(secret_name = name.as_str(), "fetched sealed secret");
        Ok(secret)
    }

    /// Removes the secret stored under `name`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] if no secret is stored under `name`,
    /// or [`Error::Io`].
    pub fn remove(&self, name: &SecretName) -> Result<(), Error> {
        self.vault.remove(name)
    }

    /// Whether a secret is stored under `name`. Informational only —
    /// subject to TOCTOU; `fetch` is the authority.
    #[must_use]
    pub fn exists(&self, name: &SecretName) -> bool {
        self.vault.exists(name)
    }
}

/// Seals `plaintext` into a versioned envelope:
/// magic || nonce || ciphertext+tag.
///
/// The nonce is fresh OS randomness per call; encryption happens in
/// place in the envelope's tail, so no intermediate ciphertext buffer
/// exists to scrub. Plaintext is borrowed, never copied except into
/// the envelope being built.
fn seal_envelope(key: &MasterKey, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    let cipher = key.cipher();
    // Fresh OS randomness per seal: XChaCha's 192-bit nonce makes
    // random nonces safe (reuse is catastrophic only for repeated
    // nonces under one key, which randomness at this width prevents).
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).map_err(|source| Error::Random { source })?;
    let nonce = XNonce::from(nonce_bytes);
    let mut envelope = Vec::with_capacity(ENVELOPE_OVERHEAD + plaintext.len());
    envelope.extend_from_slice(ENVELOPE_MAGIC);
    envelope.extend_from_slice(nonce.as_slice());
    let body_start = envelope.len();
    envelope.extend_from_slice(plaintext);
    // `body_start` was just read from the length after pushing exactly
    // magic + nonce, so the split below is infallible by construction.
    debug_assert_eq!(body_start, ENVELOPE_MAGIC.len() + NONCE_LEN);
    let (_, body) = envelope.split_at_mut(body_start);
    let tag = cipher
        .encrypt_in_place_detached(&nonce, b"", body)
        .map_err(|e| Error::SealFailed { source: e })?;
    envelope.extend_from_slice(tag.as_slice());
    Ok(envelope)
}

/// Whether `bytes` carry the sealed-envelope magic.
fn has_magic(bytes: &[u8]) -> bool {
    bytes
        .get(..ENVELOPE_MAGIC.len())
        .is_some_and(|magic| magic == ENVELOPE_MAGIC)
}

/// Opens a sealed envelope: checks the magic, splits the nonce, and
/// authenticates + decrypts the body.
///
/// Anything without the magic is [`Error::NotSealed`] — the sealed
/// vault never guesses at plaintext. A magic-bearing but truncated
/// envelope is [`Error::Malformed`]. Authentication failure is the
/// single generic [`Error::DecryptFailed`]: wrong key, tampered
/// ciphertext, and tampered tag are indistinguishable by design.
fn open_envelope(key: &MasterKey, name: &SecretName, envelope: &[u8]) -> Result<Secret, Error> {
    let not_sealed = || Error::NotSealed { name: name.clone() };
    let malformed = || Error::Malformed { name: name.clone() };
    // Magic first, with the length gate before it: a file shorter
    // than the magic cannot be an envelope at all, so it is
    // `NotSealed` (a short plaintext file), never `Malformed`.
    let Some(magic_slice) = envelope.get(..ENVELOPE_MAGIC.len()) else {
        return Err(not_sealed());
    };
    // `get(..4)` yields exactly four bytes on `Some`, so this
    // conversion is infallible; the error arm is fail-closed dead
    // code, not a panic.
    let magic = <&[u8; 4]>::try_from(magic_slice).map_err(|_| malformed())?;
    if magic != ENVELOPE_MAGIC {
        return Err(not_sealed());
    }
    let nonce: [u8; NONCE_LEN] = envelope
        .get(ENVELOPE_MAGIC.len()..ENVELOPE_MAGIC.len() + NONCE_LEN)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(malformed)?;
    let ciphertext = envelope
        .get(ENVELOPE_MAGIC.len() + NONCE_LEN..)
        .ok_or_else(malformed)?;
    // The smallest valid body is the bare authentication tag: an empty
    // plaintext seals to magic + nonce + tag.
    if ciphertext.len() < TAG_LEN {
        return Err(malformed());
    }
    let nonce = XNonce::from(nonce);
    let plaintext = key
        .cipher()
        .decrypt(&nonce, ciphertext)
        .map_err(|_| Error::DecryptFailed { name: name.clone() })?;
    Secret::new(plaintext)
}

/// One directory entry that could hold a secret: dotfiles (crashed
/// temp files) and names outside the [`SecretName`] alphabet are not
/// secrets and come back as `None`. A non-UTF-8 file name cannot be a
/// `SecretName` (the alphabet is ASCII), so it is skipped, not failed.
fn secret_entry(entry: std::io::Result<std::fs::DirEntry>) -> Result<Option<SecretName>, Error> {
    let entry = entry.map_err(|source| Error::Io { source })?;
    let name = entry.file_name();
    let name_str = name.to_string_lossy();
    if name_str.starts_with('.') {
        return Ok(None);
    }
    Ok(name_str.parse().ok())
}

/// Outcome of [`migrate_plaintext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrateReport {
    /// Files encrypted into sealed envelopes.
    pub encrypted: usize,
    /// Files left untouched (already sealed, temp files, foreign names).
    pub skipped: usize,
}

/// One-shot migration: encrypts every plaintext secret file under
/// `root` in place, after which it opens as a [`SealedVault`].
///
/// Operator action, run once when adopting the sealed vault — not a
/// runtime API. Files already carrying the envelope magic are
/// verified against `key` and left untouched (a magic-bearing file
/// that does not authenticate fails the migration closed — it is
/// tampered or corrupt, not "already migrated"); dotfiles and
/// foreign names are skipped and counted. Each rewrite goes through
/// the vault's atomic temp-write + rename pipeline, so an
/// interrupted migration is resumed by re-running it: already-sealed
/// files authenticate and are skipped.
///
/// Enumeration happens here and only here. This function never
/// returns secret material to its caller — it transforms files in
/// place — and the broker never exposes it over the protocol.
///
/// # Errors
///
/// Returns [`Error::NotSealed`] for a plaintext file encountered
/// mid-migration, [`Error::Malformed`] or [`Error::DecryptFailed`]
/// for a tampered envelope, and I/O or permission errors from the
/// vault root.
pub fn migrate_plaintext(root: &Path, key: &MasterKey) -> Result<MigrateReport, Error> {
    let vault = Vault::open(root)?;
    let mut report = MigrateReport {
        encrypted: 0,
        skipped: 0,
    };
    for entry in std::fs::read_dir(root).map_err(|source| Error::Io { source })? {
        let Some(name) = secret_entry(entry)? else {
            report.skipped += 1;
            continue;
        };
        // Plaintext files were written by `Vault::store`, so they fit
        // the plaintext bound; the sealed bound also covers envelopes
        // from an interrupted earlier pass. Anything larger is foreign
        // and fails the migration fail-closed.
        let mut bytes = vault.fetch_raw(&name, MAX_SECRET_BYTES + ENVELOPE_OVERHEAD)?;
        if has_magic(&bytes) {
            // Already sealed — but "already" is verified, not assumed:
            // a magic-bearing file that does not authenticate under
            // this key is tampered or corrupt, and the migration fails
            // closed instead of leaving it behind silently. The opened
            // `Secret` drops zeroized at the end of the statement;
            // nothing is returned to the caller.
            open_envelope(key, &name, &bytes)?;
            bytes.zeroize();
            report.skipped += 1;
            continue;
        }
        let envelope = seal_envelope(key, &bytes)?;
        // The plaintext copy has served its purpose; scrub it.
        bytes.zeroize();
        vault.store_raw(&name, &envelope)?;
        report.encrypted += 1;
    }
    Ok(report)
}

/// Outcome of [`rekey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RekeyReport {
    /// Files this run re-encrypted under the new key.
    pub rekeyed: usize,
    /// Files left untouched: temp files, foreign names, and files
    /// already under the new key (e.g. from an interrupted earlier
    /// run — see the resumability note on [`rekey`]).
    pub skipped: usize,
}

/// One-shot master-key rotation: re-encrypts every sealed file under
/// `root` from `old_key` to `new_key`, in place.
///
/// Two phases — open every envelope first, then write every new one —
/// so a phase-1 failure aborts before any file is touched. Rotation
/// is resumable: if phase 2 is interrupted (some files under the new
/// key, some under the old), re-running with the same keys completes
/// it — a file that authenticates under `new_key` is recognized as
/// already done and skipped, so the rotation never aborts on its own
/// output. A file that authenticates under neither key aborts with
/// [`Error::DecryptFailed`]; a plaintext file aborts with
/// [`Error::NotSealed`] (migrate first).
///
/// Same one-shot operator discipline as [`migrate_plaintext`]:
/// enumeration here never returns secret material to a caller, and
/// the broker never exposes it over the protocol. Note the transient
/// memory cost: all secrets are held while re-encrypting.
///
/// # Errors
///
/// Returns [`Error::NotSealed`] for a plaintext file (migrate
/// first), [`Error::DecryptFailed`] for a file authenticating under
/// neither key, and I/O or permission errors from the vault root.
pub fn rekey(root: &Path, old_key: &MasterKey, new_key: &MasterKey) -> Result<RekeyReport, Error> {
    let vault = Vault::open(root)?;
    // Phase 1: open every envelope. Any failure aborts before the
    // first write in phase 2.
    let mut opened: Vec<(SecretName, Secret)> = Vec::new();
    let mut skipped = 0;
    for entry in std::fs::read_dir(root).map_err(|source| Error::Io { source })? {
        let Some(name) = secret_entry(entry)? else {
            skipped += 1;
            continue;
        };
        let envelope = vault.fetch_raw(&name, MAX_SECRET_BYTES + ENVELOPE_OVERHEAD)?;
        // A `DecryptFailed` under the old key may mean "already
        // rekeyed": probe the new key before giving up, so an
        // interrupted rotation resumes instead of aborting on its own
        // output. The probe's `Secret` drops zeroized immediately.
        let secret = match open_envelope(old_key, &name, &envelope) {
            Ok(secret) => Some(secret),
            Err(Error::DecryptFailed { .. }) => {
                if open_envelope(new_key, &name, &envelope).is_ok() {
                    skipped += 1;
                    None
                } else {
                    return Err(Error::DecryptFailed { name: name.clone() });
                }
            }
            Err(other) => return Err(other),
        };
        if let Some(secret) = secret {
            opened.push((name, secret));
        }
    }
    // Phase 2: every envelope opened cleanly; re-encrypt under the new
    // key through the atomic pipeline. `Secret`s drop zeroized.
    let mut rekeyed = 0;
    for (name, secret) in &opened {
        let envelope = seal_envelope(new_key, secret.as_bytes())?;
        vault.store_raw(name, &envelope)?;
        rekeyed += 1;
    }
    Ok(RekeyReport { rekeyed, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    /// A fresh directory unique to this test and process, so tests run
    /// in parallel without colliding. The directory itself is NOT
    /// created: `Vault::open` and `FileKey` create what they need with
    /// the `0700`/`0600` discipline, and a pre-created directory would
    /// carry the umask mode and fail validation.
    ///
    /// The guard removes the directory when the test ends, pass or
    /// fail.
    fn test_dir(name: &str) -> TestDir {
        let dir =
            std::env::temp_dir().join(format!("hatchling-seal-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        TestDir(dir)
    }

    /// A test's scratch directory, removed on drop so a failing
    /// assertion cannot leak it.
    struct TestDir(PathBuf);

    impl std::ops::Deref for TestDir {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_test_dir_is_removed_when_its_guard_drops() {
        let dir = test_dir("guard");
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested").join("file"), b"x").unwrap();
        let path = dir.to_path_buf();
        drop(dir);
        assert!(!path.exists(), "{} was left behind", path.display());
    }

    fn test_key() -> MasterKey {
        MasterKey::generate().unwrap()
    }

    fn secret_name(name: &str) -> SecretName {
        name.parse().unwrap()
    }

    fn secret(bytes: &[u8]) -> Secret {
        Secret::new(bytes.to_vec()).unwrap()
    }

    /// The raw bytes of the secret file on disk: the sealed vault
    /// stores one file per secret named by the secret name.
    fn raw_bytes(root: &Path, name: &SecretName) -> Vec<u8> {
        std::fs::read(root.join(name.as_str())).unwrap()
    }

    /// `MasterKey` has no equality: behavioral check — a secret sealed
    /// under `a` must open under `b`.
    fn assert_same_key(a: &MasterKey, b: &MasterKey, root: &Path) {
        let va = SealedVault::open(root, a).unwrap();
        let vb = SealedVault::open(root, b).unwrap();
        let name = secret_name("equality-probe");
        va.store(&name, &secret(b"probe")).unwrap();
        let back = vb.fetch(&name).unwrap();
        assert_eq!(back.as_bytes(), b"probe");
    }

    fn real_uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    #[test]
    fn debug_is_redacted() {
        // Deleting the custom `Debug` impl (falling back to a derived
        // one that prints the bytes) must fail this test.
        let key = MasterKey::from_bytes([0xAB; 32]);
        assert_eq!(format!("{key:?}"), "MasterKey([redacted])");
    }

    #[test]
    #[allow(
        unsafe_code,
        reason = "reads the dropped key's former stack slot through a raw pointer; the frame is alive and nothing has reused the slot, mirroring secret.rs's drop-zeroize test"
    )]
    fn drop_zeroizes_key_material() {
        // The whole point of the `ZeroizeOnDrop` derive: the
        // `drop -> ()` mutant (deleting the derive) must fail this
        // test. The key is dropped *in place* at the end of the
        // block: passing it by value to `drop()` would move it
        // first, and the scrub would land on the moved-to slot
        // instead of the observed one. (Moves always leave a stale
        // copy behind — which is why the type is non-cloneable and
        // is borrowed as `&MasterKey` across the broker rather than
        // passed by value.)
        let addr: *const u8;
        {
            let key = MasterKey::from_bytes([0xAB; 32]);
            addr = key.0.as_ptr();
        }
        // SAFETY: out-of-model read (use-after-free) — the only way
        // to observe the scrub. It pins the property the type
        // exists for, and must never be imitated in production code.
        let after = unsafe { std::slice::from_raw_parts(addr, 32) };
        assert!(
            after.iter().all(|&b| b == 0),
            "key material survived drop: {after:?}"
        );
    }

    #[test]
    fn roundtrip() {
        let dir = test_dir("roundtrip");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("api-token");
        vault.store(&name, &secret(b"super-secret-value")).unwrap();
        assert!(vault.exists(&name));
        let back = vault.fetch(&name).unwrap();
        assert_eq!(back.as_bytes(), b"super-secret-value");
        vault.remove(&name).unwrap();
        assert!(!vault.exists(&name));
        let err = vault.fetch(&name).unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[test]
    fn empty_secret_roundtrips() {
        // The smallest valid envelope: magic + nonce + bare tag.
        let dir = test_dir("empty");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("empty");
        vault.store(&name, &secret(b"")).unwrap();
        let raw = raw_bytes(&dir, &name);
        assert_eq!(raw.len(), ENVELOPE_OVERHEAD);
        let back = vault.fetch(&name).unwrap();
        assert_eq!(back.as_bytes(), b"");
    }

    #[test]
    fn on_disk_bytes_are_a_versioned_envelope_not_plaintext() {
        let dir = test_dir("envelope-shape");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("token");
        // Distinctive plaintext: a coincidental reappearance in random
        // ciphertext is not a test failure we want to chase.
        let plaintext = b"plaintext-marker-0123456789abcdef";
        vault.store(&name, &secret(plaintext)).unwrap();
        let raw = raw_bytes(&dir, &name);
        assert_eq!(
            raw.get(..ENVELOPE_MAGIC.len()),
            Some(ENVELOPE_MAGIC.as_slice())
        );
        assert_eq!(raw.len(), ENVELOPE_OVERHEAD + plaintext.len());
        assert!(
            !raw.windows(plaintext.len()).any(|w| w == plaintext),
            "plaintext appears in the stored bytes"
        );
    }

    #[test]
    fn identical_plaintexts_seal_differently() {
        // Fresh random nonce per store: two envelopes of the same
        // plaintext must differ. Pins the nonce generation — a mutant
        // replacing it with a fixed nonce fails here.
        let dir = test_dir("fresh-nonce");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let s = secret(b"same-value");
        vault.store(&secret_name("first"), &s).unwrap();
        vault.store(&secret_name("second"), &s).unwrap();
        let a = raw_bytes(&dir, &secret_name("first"));
        let b = raw_bytes(&dir, &secret_name("second"));
        assert_ne!(a, b, "two seals of the same plaintext are identical");
    }

    /// Writes arbitrary bytes as the secret file, bypassing the seal
    /// pipeline, then reads through the sealed vault.
    fn fetch_malformed(dir: &Path, key: &MasterKey, name: &SecretName, bytes: &[u8]) -> Error {
        let vault = Vault::open(dir).unwrap();
        vault.store_raw(name, bytes).unwrap();
        let sealed = SealedVault::open(dir, key).unwrap();
        sealed.fetch(name).unwrap_err()
    }

    #[test]
    fn short_file_is_not_sealed_never_malformed() {
        // A file shorter than the magic cannot be an envelope at all:
        // `NotSealed` (a short plaintext file), never `Malformed`.
        // Pins the length gate before the magic check.
        let dir = test_dir("short-file");
        let key = test_key();
        let err = fetch_malformed(&dir, &key, &secret_name("s"), b"AB");
        assert!(matches!(err, Error::NotSealed { .. }), "got {err:?}");
    }

    #[test]
    fn empty_file_is_not_sealed() {
        let dir = test_dir("empty-file");
        let key = test_key();
        let err = fetch_malformed(&dir, &key, &secret_name("s"), b"");
        assert!(matches!(err, Error::NotSealed { .. }), "got {err:?}");
    }

    #[test]
    fn wrong_magic_is_not_sealed() {
        // Long enough to be an envelope, wrong magic: still
        // `NotSealed`, not a decryption attempt.
        let dir = test_dir("wrong-magic");
        let key = test_key();
        let mut bytes = b"XXXX".to_vec();
        bytes.extend_from_slice(&[0u8; 100]);
        let err = fetch_malformed(&dir, &key, &secret_name("s"), &bytes);
        assert!(matches!(err, Error::NotSealed { .. }), "got {err:?}");
    }

    #[test]
    fn magic_only_is_malformed() {
        let dir = test_dir("magic-only");
        let key = test_key();
        let err = fetch_malformed(&dir, &key, &secret_name("s"), ENVELOPE_MAGIC);
        assert!(matches!(err, Error::Malformed { .. }), "got {err:?}");
    }

    #[test]
    fn truncated_nonce_is_malformed() {
        let dir = test_dir("truncated-nonce");
        let key = test_key();
        let mut bytes = ENVELOPE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; 10]);
        let err = fetch_malformed(&dir, &key, &secret_name("s"), &bytes);
        assert!(matches!(err, Error::Malformed { .. }), "got {err:?}");
    }

    #[test]
    fn nonce_without_tag_is_malformed() {
        let dir = test_dir("nonce-no-tag");
        let key = test_key();
        let mut bytes = ENVELOPE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; NONCE_LEN]);
        let err = fetch_malformed(&dir, &key, &secret_name("s"), &bytes);
        assert!(matches!(err, Error::Malformed { .. }), "got {err:?}");
    }

    #[test]
    fn short_body_is_malformed() {
        // Magic + full nonce + fewer than TAG_LEN body bytes.
        let dir = test_dir("short-body");
        let key = test_key();
        let mut bytes = ENVELOPE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0u8; NONCE_LEN + 5]);
        let err = fetch_malformed(&dir, &key, &secret_name("s"), &bytes);
        assert!(matches!(err, Error::Malformed { .. }), "got {err:?}");
    }

    #[test]
    fn tampered_ciphertext_is_a_generic_authentication_failure() {
        let dir = test_dir("tampered-ct");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("s");
        vault.store(&name, &secret(b"0123456789abcdef")).unwrap();
        let mut raw = raw_bytes(&dir, &name);
        let byte = raw.get_mut(ENVELOPE_MAGIC.len() + NONCE_LEN).unwrap();
        *byte ^= 0xFF;
        Vault::open(&dir).unwrap().store_raw(&name, &raw).unwrap();
        let err = vault.fetch(&name).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn tampered_tag_is_a_generic_authentication_failure() {
        let dir = test_dir("tampered-tag");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("s");
        vault.store(&name, &secret(b"0123456789abcdef")).unwrap();
        let mut raw = raw_bytes(&dir, &name);
        let last = raw.len() - 1;
        let byte = raw.get_mut(last).unwrap();
        *byte ^= 0xFF;
        Vault::open(&dir).unwrap().store_raw(&name, &raw).unwrap();
        let err = vault.fetch(&name).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn wrong_key_is_indistinguishable_from_tampering() {
        // The whole point of the single generic error: no oracle leaks
        // whether the key was wrong or the bytes were tampered with.
        let dir = test_dir("wrong-key");
        let key_a = test_key();
        let key_b = test_key();
        let va = SealedVault::open(&dir, &key_a).unwrap();
        let name = secret_name("s");
        va.store(&name, &secret(b"value")).unwrap();
        let vb = SealedVault::open(&dir, &key_b).unwrap();
        let err = vb.fetch(&name).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn oversized_envelope_is_rejected_before_allocation() {
        // A foreign-grown file must not become an unbounded allocation:
        // the sealed read bound is MAX_SECRET_BYTES + ENVELOPE_OVERHEAD.
        let dir = test_dir("oversized");
        let key = test_key();
        Vault::open(&dir).unwrap();
        let name = secret_name("big");
        let foreign = vec![0xAAu8; MAX_SECRET_BYTES + ENVELOPE_OVERHEAD + 1];
        std::fs::write(dir.join(name.as_str()), &foreign).unwrap();
        let sealed = SealedVault::open(&dir, &key).unwrap();
        let err = sealed.fetch(&name).unwrap_err();
        assert!(
            matches!(err, Error::TooLarge { len, max }
                if len == MAX_SECRET_BYTES + ENVELOPE_OVERHEAD + 1
                   && max == MAX_SECRET_BYTES + ENVELOPE_OVERHEAD),
            "got {err:?}"
        );
    }

    #[test]
    fn max_plaintext_roundtrips() {
        // Boundary: exactly MAX_SECRET_BYTES seals and opens.
        let dir = test_dir("max-plaintext");
        let key = test_key();
        let vault = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("max");
        let big = vec![0x55u8; MAX_SECRET_BYTES];
        vault.store(&name, &secret(&big)).unwrap();
        let back = vault.fetch(&name).unwrap();
        assert_eq!(back.as_bytes(), big.as_slice());
    }

    // ---- FileKey ----

    fn key_file_in(dir: &Path) -> (FileKey, PathBuf) {
        let path = dir.join("vault.key");
        (FileKey::new(path.clone()), path)
    }

    #[test]
    fn file_key_creates_with_owner_only_discipline() {
        // `FileKey` parents are single-level: the broker's key path
        // is a sibling of the vault root, whose own parent already
        // exists. (Deeper nesting is not created; it fails closed.)
        let dir = test_dir("file-key-create");
        let key_path = dir.join("vault.key");
        let source = FileKey::new(key_path.clone());
        let key = source.load_or_create_as(real_uid()).unwrap();
        // Parent created 0700, file created 0600, exactly 32 bytes.
        let parent = key_path.parent().unwrap();
        let parent_mode = std::fs::metadata(parent).unwrap().permissions().mode() & 0o777;
        assert_eq!(parent_mode, 0o700, "parent mode {parent_mode:o}");
        let file_mode = std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "key file mode {file_mode:o}");
        assert_eq!(std::fs::metadata(&key_path).unwrap().len(), 32);
        // Reload returns the same key (behavioral equality).
        let dir2 = test_dir("file-key-reload-root");
        let again = source.load_or_create_as(real_uid()).unwrap();
        assert_same_key(&key, &again, &dir2);
    }

    #[test]
    fn file_key_trait_load_or_create_uses_the_real_uid() {
        // The public `KeySource` path: create then reload.
        let dir = test_dir("file-key-trait");
        let (source, path) = key_file_in(&dir);
        let key = source.load_or_create().unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let again = source.load_or_create().unwrap();
        let dir2 = test_dir("file-key-trait-root");
        assert_same_key(&key, &again, &dir2);
    }

    #[test]
    fn file_key_wrong_mode_fails_closed() {
        let dir = test_dir("file-key-mode");
        let (source, path) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let err = source.load_or_create_as(real_uid()).unwrap_err();
        assert!(matches!(err, Error::InsecureKeyFile { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_wrong_owner_fails_closed() {
        // The parent directory check fires first: a key file whose
        // parent is not owned by the loading uid is never trusted.
        let dir = test_dir("file-key-owner");
        let (source, _) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        let err = source
            .load_or_create_as(real_uid().wrapping_add(1))
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::InsecurePermissions { .. } | Error::InsecureKeyFile { .. }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn file_key_symlink_is_rejected() {
        let dir = test_dir("file-key-symlink");
        let (source, path) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        let link = dir.join("vault-link.key");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        // `read_key` directly: the parent check is bypassed on purpose
        // so this pins the symlink rejection itself, not the parent.
        let file = std::fs::File::open(&link).unwrap();
        let err = FileKey::read_key(&link, &file, real_uid()).unwrap_err();
        assert!(matches!(err, Error::InsecureKeyFile { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_symlink_loop_fails_closed() {
        // `File::open` on a symlink loop fails with `FilesystemLoop`,
        // not `NotFound`: only a genuinely missing key file may be
        // created. A `load_or_create_as` that created on *any* I/O
        // error would mint a fresh key here (failing as `Io` from the
        // create leg) instead of failing the load closed — silently
        // orphaning every secret sealed under the real key.
        let dir = test_dir("file-key-loop");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = dir.join("vault.key");
        std::os::unix::fs::symlink(&link, &link).unwrap();
        let source = FileKey::new(link);
        let err = source.load_or_create_as(real_uid()).unwrap_err();
        assert!(matches!(err, Error::KeyLoad { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_trailing_slash_fails_closed() {
        // `File::open` on "vault.key/" (trailing slash over a regular
        // file) fails — here with `IsADirectory` — not `NotFound`:
        // only a genuinely missing key file may be created.
        // `create_new` on the same path likewise fails without
        // `AlreadyExists`, so a `load_or_create_as` that created on
        // *any* I/O error surfaces the create-leg `Io` here instead of
        // the load-leg `KeyLoad`, and the test tells them apart.
        let dir = test_dir("file-key-slash");
        let (source, path) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        let slashed = FileKey::new(PathBuf::from(format!("{}/", path.display())));
        let err = slashed.load_or_create_as(real_uid()).unwrap_err();
        assert!(matches!(err, Error::KeyLoad { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_create_missing_parent_fails_closed() {
        // `create_new` with a missing parent fails with `NotFound`,
        // not `AlreadyExists`: the lost-race branch must not run. A
        // `create_key` that treated any creation error as "already
        // exists" would try opening the missing file and report the
        // read-leg `KeyLoad` instead of the creation-leg I/O failure.
        let dir = test_dir("file-key-create-noparent");
        std::fs::create_dir_all(&dir).unwrap();
        let source = FileKey::new(dir.join("no-such-subdir").join("vault.key"));
        let err = source.create_key(real_uid()).unwrap_err();
        assert!(matches!(err, Error::Io { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_create_lost_race_reads_winner() {
        // Deterministic lost race: the key file already exists when
        // `create_key` runs, so `create_new` reports `AlreadyExists`
        // and the winner's key is read instead of failing. Pins the
        // `AlreadyExists` guard in the failing direction too: without
        // it the race is reported as a creation I/O failure.
        let dir = test_dir("file-key-race");
        let (source, _) = key_file_in(&dir);
        let winner = source.load_or_create_as(real_uid()).unwrap();
        let ours = source.create_key(real_uid()).unwrap();
        let dir2 = test_dir("file-key-race-root");
        assert_same_key(&winner, &ours, &dir2);
    }

    #[test]
    fn file_key_directory_is_rejected() {
        let dir = test_dir("file-key-dir");
        std::fs::create_dir_all(&dir).unwrap();
        let sub = dir.join("not-a-key");
        std::fs::create_dir(&sub).unwrap();
        let file = std::fs::File::open(&sub).unwrap();
        let err = FileKey::read_key(&sub, &file, real_uid()).unwrap_err();
        assert!(matches!(err, Error::InsecureKeyFile { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_short_key_is_rejected() {
        // A partial write (e.g. crashed creation) fails closed on the
        // next load: 16 bytes is not a key.
        let dir = test_dir("file-key-short");
        let (source, path) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        std::fs::write(&path, [0x11u8; 16]).unwrap();
        let err = source.load_or_create_as(real_uid()).unwrap_err();
        assert!(
            matches!(err, Error::InvalidKey { len: 16, .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn file_key_long_key_is_rejected_not_truncated() {
        // 33 bytes: the bounded read detects the extra byte and
        // rejects, never silently truncates to 32. Pins the
        // `saturating_add(1)` in the read limit.
        let dir = test_dir("file-key-long");
        let (source, path) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        std::fs::write(&path, [0x22u8; 33]).unwrap();
        let err = source.load_or_create_as(real_uid()).unwrap_err();
        assert!(
            matches!(err, Error::InvalidKey { len: 33, .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn file_key_empty_key_is_rejected() {
        let dir = test_dir("file-key-empty");
        let (source, path) = key_file_in(&dir);
        source.load_or_create_as(real_uid()).unwrap();
        std::fs::write(&path, []).unwrap();
        let err = source.load_or_create_as(real_uid()).unwrap_err();
        assert!(
            matches!(err, Error::InvalidKey { len: 0, .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn file_key_without_parent_fails() {
        // The root path has no parent directory at all.
        let source = FileKey::new(PathBuf::from("/"));
        let err = source.load_or_create_as(real_uid()).unwrap_err();
        assert!(matches!(err, Error::KeyLoad { .. }), "got {err:?}");
    }

    #[test]
    fn file_key_creation_race_reads_the_winners_key() {
        // Deterministic simulation of the `create_new` race: the file
        // already exists when `create_key` runs, so the `AlreadyExists`
        // arm must read the winner's key instead of failing. Deleting
        // that arm fails this test.
        //
        // NOTE: the directory names must be unique to this test. An
        // earlier revision shared `file-key-race` / `file-key-race-root`
        // with `file_key_create_lost_race_reads_winner`; `test_dir`
        // removes the directory on entry, so parallel runs deleted the
        // other test's directory mid-flight and failed CI with
        // `NotFound` (exit 101 on the push).
        let dir = test_dir("file-key-winner-race");
        let parent = dir.join("sub");
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = parent.join("vault.key");
        let winner_bytes = [0x77u8; 32];
        std::fs::write(&path, winner_bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let source = FileKey::new(path);
        let key = source.create_key(real_uid()).unwrap();
        let dir2 = test_dir("file-key-winner-race-root");
        assert_same_key(&key, &MasterKey::from_bytes(winner_bytes), &dir2);
    }

    // ---- migrate_plaintext ----

    #[test]
    fn migrate_encrypts_plaintext_in_place() {
        let dir = test_dir("migrate-basic");
        let key = test_key();
        // Legacy plaintext store: two secrets via the plain `Vault`.
        let plain = Vault::open(&dir).unwrap();
        plain
            .store(&secret_name("one"), &secret(b"first-value"))
            .unwrap();
        plain
            .store(&secret_name("two"), &secret(b"second-value"))
            .unwrap();

        let report = migrate_plaintext(&dir, &key).unwrap();
        assert_eq!(
            report,
            MigrateReport {
                encrypted: 2,
                skipped: 0
            }
        );
        // Bytes on disk are envelopes now, and the sealed vault reads
        // them back.
        for name in ["one", "two"] {
            let raw = raw_bytes(&dir, &secret_name(name));
            assert_eq!(
                raw.get(..ENVELOPE_MAGIC.len()),
                Some(ENVELOPE_MAGIC.as_slice())
            );
        }
        let sealed = SealedVault::open(&dir, &key).unwrap();
        assert_eq!(
            sealed.fetch(&secret_name("one")).unwrap().as_bytes(),
            b"first-value"
        );
        assert_eq!(
            sealed.fetch(&secret_name("two")).unwrap().as_bytes(),
            b"second-value"
        );
    }

    #[test]
    fn migrate_rejects_file_past_the_sealed_bound() {
        // The migration read bound is exactly
        // `MAX_SECRET_BYTES + ENVELOPE_OVERHEAD`: a larger file is
        // foreign and fails closed with that bound reported. Pins the
        // bound against `-` (a max-size secret would be rejected) and
        // `*` (the foreign file would be sealed instead of rejected).
        let dir = test_dir("migrate-oversize");
        let key = test_key();
        let plain = Vault::open(&dir).unwrap();
        plain
            .store(&secret_name("small"), &secret(b"value"))
            .unwrap();
        // Foreign bytes just past the sealed bound, without envelope
        // magic so they cannot be mistaken for an envelope.
        let big = vec![0x41u8; MAX_SECRET_BYTES + ENVELOPE_OVERHEAD + 1];
        std::fs::write(dir.join("big"), &big).unwrap();
        let err = migrate_plaintext(&dir, &key).unwrap_err();
        assert!(
            matches!(err, Error::TooLarge { max, .. } if max == MAX_SECRET_BYTES + ENVELOPE_OVERHEAD),
            "got {err:?}"
        );
    }

    #[test]
    fn migrate_is_idempotent() {
        let dir = test_dir("migrate-idempotent");
        let key = test_key();
        let plain = Vault::open(&dir).unwrap();
        plain.store(&secret_name("one"), &secret(b"value")).unwrap();
        migrate_plaintext(&dir, &key).unwrap();
        // Second run: the sealed file authenticates under the key and
        // is verified-skipped, not re-encrypted.
        let report = migrate_plaintext(&dir, &key).unwrap();
        assert_eq!(
            report,
            MigrateReport {
                encrypted: 0,
                skipped: 1
            }
        );
    }

    #[test]
    fn migrate_verifies_already_sealed_files() {
        // Magic is verified, not assumed: a magic-bearing file that
        // does not authenticate under the migration key fails closed.
        let dir = test_dir("migrate-verify");
        let key_a = test_key();
        let key_b = test_key();
        let sealed = SealedVault::open(&dir, &key_a).unwrap();
        sealed.store(&secret_name("s"), &secret(b"value")).unwrap();
        let err = migrate_plaintext(&dir, &key_b).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn migrate_rejects_tampered_envelopes() {
        let dir = test_dir("migrate-tampered");
        let key = test_key();
        let sealed = SealedVault::open(&dir, &key).unwrap();
        let name = secret_name("s");
        sealed.store(&name, &secret(b"value")).unwrap();
        let mut raw = raw_bytes(&dir, &name);
        let byte = raw.get_mut(ENVELOPE_MAGIC.len()).unwrap();
        *byte ^= 0xFF;
        Vault::open(&dir).unwrap().store_raw(&name, &raw).unwrap();
        let err = migrate_plaintext(&dir, &key).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn migrate_skips_dotfiles_and_foreign_names() {
        let dir = test_dir("migrate-skip");
        let key = test_key();
        let plain = Vault::open(&dir).unwrap();
        plain
            .store(&secret_name("real"), &secret(b"value"))
            .unwrap();
        // Crashed temp file and a name outside the SecretName alphabet:
        // not secrets, counted as skipped, left untouched.
        std::fs::write(dir.join(".tmp-12"), b"partial").unwrap();
        std::fs::write(dir.join("has space"), b"foreign").unwrap();
        let report = migrate_plaintext(&dir, &key).unwrap();
        assert_eq!(
            report,
            MigrateReport {
                encrypted: 1,
                skipped: 2
            }
        );
        assert_eq!(std::fs::read(dir.join(".tmp-12")).unwrap(), b"partial");
        assert_eq!(std::fs::read(dir.join("has space")).unwrap(), b"foreign");
    }

    #[test]
    fn migrate_resumes_a_partial_run() {
        // One file already sealed (previous interrupted run), one still
        // plaintext: re-running completes the migration.
        let dir = test_dir("migrate-resume");
        let key = test_key();
        SealedVault::open(&dir, &key)
            .unwrap()
            .store(&secret_name("done"), &secret(b"done-value"))
            .unwrap();
        Vault::open(&dir)
            .unwrap()
            .store(&secret_name("todo"), &secret(b"todo-value"))
            .unwrap();
        let report = migrate_plaintext(&dir, &key).unwrap();
        assert_eq!(
            report,
            MigrateReport {
                encrypted: 1,
                skipped: 1
            }
        );
        let sealed = SealedVault::open(&dir, &key).unwrap();
        assert_eq!(
            sealed.fetch(&secret_name("todo")).unwrap().as_bytes(),
            b"todo-value"
        );
    }

    // ---- rekey ----

    #[test]
    fn rekey_rotates_to_the_new_key() {
        let dir = test_dir("rekey-basic");
        let old = test_key();
        let new = test_key();
        let sealed = SealedVault::open(&dir, &old).unwrap();
        sealed.store(&secret_name("a"), &secret(b"alpha")).unwrap();
        sealed.store(&secret_name("b"), &secret(b"beta")).unwrap();

        let report = rekey(&dir, &old, &new).unwrap();
        assert_eq!(
            report,
            RekeyReport {
                rekeyed: 2,
                skipped: 0
            }
        );
        // New key opens everything; the old key opens nothing.
        let under_new = SealedVault::open(&dir, &new).unwrap();
        assert_eq!(
            under_new.fetch(&secret_name("a")).unwrap().as_bytes(),
            b"alpha"
        );
        let under_old = SealedVault::open(&dir, &old).unwrap();
        let err = under_old.fetch(&secret_name("a")).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn rekey_is_idempotent() {
        let dir = test_dir("rekey-idempotent");
        let old = test_key();
        let new = test_key();
        SealedVault::open(&dir, &old)
            .unwrap()
            .store(&secret_name("a"), &secret(b"alpha"))
            .unwrap();
        rekey(&dir, &old, &new).unwrap();
        let report = rekey(&dir, &old, &new).unwrap();
        assert_eq!(
            report,
            RekeyReport {
                rekeyed: 0,
                skipped: 1
            }
        );
    }

    #[test]
    fn rekey_resumes_after_an_interrupted_phase_two() {
        // File `a` is already under the new key (phase 2 wrote it, then
        // the rotation was interrupted); file `b` is still under the
        // old. Re-running must complete the rotation, not abort on the
        // already-rekeyed file. Deleting the new-key probe fails this
        // test.
        let dir = test_dir("rekey-resume");
        let old = test_key();
        let new = test_key();
        SealedVault::open(&dir, &new)
            .unwrap()
            .store(&secret_name("a"), &secret(b"alpha"))
            .unwrap();
        SealedVault::open(&dir, &old)
            .unwrap()
            .store(&secret_name("b"), &secret(b"beta"))
            .unwrap();
        let report = rekey(&dir, &old, &new).unwrap();
        assert_eq!(
            report,
            RekeyReport {
                rekeyed: 1,
                skipped: 1
            }
        );
        let under_new = SealedVault::open(&dir, &new).unwrap();
        assert_eq!(
            under_new.fetch(&secret_name("a")).unwrap().as_bytes(),
            b"alpha"
        );
        assert_eq!(
            under_new.fetch(&secret_name("b")).unwrap().as_bytes(),
            b"beta"
        );
    }

    #[test]
    fn rekey_rejects_files_under_neither_key() {
        let dir = test_dir("rekey-foreign");
        let old = test_key();
        let new = test_key();
        let third = test_key();
        SealedVault::open(&dir, &third)
            .unwrap()
            .store(&secret_name("x"), &secret(b"???"))
            .unwrap();
        let err = rekey(&dir, &old, &new).unwrap_err();
        assert!(matches!(err, Error::DecryptFailed { .. }), "got {err:?}");
    }

    #[test]
    fn rekey_rejects_plaintext() {
        // A plaintext file is a migrate-first signal, not silently
        // skipped: the sealed vault never reads it as plaintext.
        let dir = test_dir("rekey-plaintext");
        let old = test_key();
        let new = test_key();
        Vault::open(&dir)
            .unwrap()
            .store(&secret_name("p"), &secret(b"plaintext"))
            .unwrap();
        let err = rekey(&dir, &old, &new).unwrap_err();
        assert!(matches!(err, Error::NotSealed { .. }), "got {err:?}");
    }

    #[test]
    fn rekey_rejects_file_past_the_sealed_bound() {
        // The rekey read bound is exactly
        // `MAX_SECRET_BYTES + ENVELOPE_OVERHEAD`: a larger file is
        // foreign and fails closed with that bound reported. Pins the
        // bound against `-` (a max-size envelope would be rejected)
        // and `*` (the foreign file would be probed as an envelope
        // instead of rejected).
        let dir = test_dir("rekey-oversize");
        let old = test_key();
        let new = test_key();
        let sealed = SealedVault::open(&dir, &old).unwrap();
        sealed
            .store(&secret_name("small"), &secret(b"value"))
            .unwrap();
        let big = vec![0x41u8; MAX_SECRET_BYTES + ENVELOPE_OVERHEAD + 1];
        std::fs::write(dir.join("big"), &big).unwrap();
        let err = rekey(&dir, &old, &new).unwrap_err();
        assert!(
            matches!(err, Error::TooLarge { max, .. } if max == MAX_SECRET_BYTES + ENVELOPE_OVERHEAD),
            "got {err:?}"
        );
    }

    #[test]
    fn rekey_skips_dotfiles() {
        let dir = test_dir("rekey-dotfile");
        let old = test_key();
        let new = test_key();
        SealedVault::open(&dir, &old)
            .unwrap()
            .store(&secret_name("a"), &secret(b"alpha"))
            .unwrap();
        std::fs::write(dir.join(".tmp-7"), b"partial").unwrap();
        let report = rekey(&dir, &old, &new).unwrap();
        assert_eq!(
            report,
            RekeyReport {
                rekeyed: 1,
                skipped: 1
            }
        );
    }

    #[test]
    fn loading_a_key_never_creates_one() {
        let dir = test_dir("load-only");
        let path = dir.join("kek");
        assert!(matches!(
            FileKey::new(path.clone()).load(),
            Err(Error::KeyLoad { .. })
        ));
        assert!(!path.exists(), "a missing key is not created by a load");
        let created = FileKey::new(path.clone()).load_or_create().unwrap();
        let loaded = FileKey::new(path.clone()).load().unwrap();
        assert_same_key(&created, &loaded, &dir.join("vault"));
        let owner = rustix::process::getuid().as_raw();
        assert!(matches!(
            FileKey::new(path).load_as(owner + 1),
            Err(Error::InsecureKeyFile { .. })
        ));
    }
}
