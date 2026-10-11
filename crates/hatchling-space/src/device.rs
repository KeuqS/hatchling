//! Devices: pairing codes, device identities, and the device
//! endpoint's mutual authentication.
//!
//! **Pairing is explicit.** The agent asks for a pairing code for one
//! space; the user carries it to the device; the device redeems it on the
//! broker's device endpoint. A code is single-use and short-lived
//! ([`PAIRING_TTL_SECS`]); a used, expired, or unknown code is refused,
//! and every refusal is logged ([`Pairings::refusals`]). There is no
//! discovery: a device the broker was never told about gets nothing.
//!
//! **Both ends authenticate before any state moves.** The endpoint
//! opens with a fresh nonce and the broker's device key. The device signs
//! a transcript over that nonce with its own key ([`pair_transcript`],
//! [`auth_transcript`]) and sends a nonce of its own, which the broker
//! signs back ([`broker_transcript`]). The device checks that against the
//! broker key it pinned at pairing. Each transcript is domain-separated,
//! so a signature made for one step is no good for another.

use std::collections::BTreeMap;

use base64::Engine as _;
use ed25519_dalek::{Signature, VerifyingKey};
use hatchling_core::SpaceRefusal;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// How long a pairing code lasts, in seconds.
pub const PAIRING_TTL_SECS: i64 = 5 * 60;

/// Most pairing codes outstanding at once.
pub const MAX_PAIRINGS: usize = 16;

/// Most pairing refusals the log keeps; the oldest go first.
pub const MAX_REFUSALS: usize = 32;

/// Every device id starts with this.
pub const DEVICE_ID_PREFIX: &str = "dev-";

/// A device's id: [`DEVICE_ID_PREFIX`] and the SHA-256 of its key, hex.
#[must_use]
pub fn device_id(key: &VerifyingKey) -> String {
    use std::fmt::Write as _;
    Sha256::digest(key.as_bytes())
        .iter()
        .fold(String::from(DEVICE_ID_PREFIX), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// Whether `nonce` is 32 bytes of lowercase hex.
#[must_use]
pub fn is_nonce(nonce: &str) -> bool {
    nonce.len() == 64
        && nonce
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
}

/// What a pairing device signs: the broker's nonce, the code, and the key
/// it is pairing.
#[must_use]
pub fn pair_transcript(broker_nonce: &str, code: &str, device_key: &str) -> Vec<u8> {
    format!("hatchling-device-pair-v1\n{broker_nonce}\n{code}\n{device_key}").into_bytes()
}

/// What a paired device signs to authenticate: the broker's nonce and its
/// id.
#[must_use]
pub fn auth_transcript(broker_nonce: &str, device: &str) -> Vec<u8> {
    format!("hatchling-device-auth-v1\n{broker_nonce}\n{device}").into_bytes()
}

/// What the broker signs to authenticate itself to a device: the
/// device's nonce and its id.
#[must_use]
pub fn broker_transcript(device_nonce: &str, device: &str) -> Vec<u8> {
    format!("hatchling-device-broker-v1\n{device_nonce}\n{device}").into_bytes()
}

/// The key in `encoded` (base64), if it is a valid Ed25519 public key.
#[must_use]
pub fn parse_key(encoded: &str) -> Option<VerifyingKey> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let bytes: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

/// Whether `signature` (base64) is `key`'s signature over `message`.
#[must_use]
pub fn verifies(key: &VerifyingKey, message: &[u8], signature: &str) -> bool {
    base64::engine::general_purpose::STANDARD
        .decode(signature)
        .ok()
        .and_then(|bytes| Signature::from_slice(&bytes).ok())
        .is_some_and(|signature| key.verify_strict(message, &signature).is_ok())
}

/// One paired device, as the broker keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRecord {
    /// Its id.
    pub id: String,
    /// Its public key, base64.
    pub key: String,
    /// The spaces it was paired into.
    pub spaces: Vec<String>,
    /// When it was first paired, in Unix seconds.
    pub paired_at: i64,
}

/// Why a pairing code was refused. One reply goes back for all three;
/// the log keeps which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRefusal {
    /// The broker never issued it, or has forgotten it.
    Unknown,
    /// It was already redeemed.
    Used,
    /// It expired before it was redeemed.
    Expired,
}

/// One logged refusal of a pairing code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusedPairing {
    /// When, in Unix seconds.
    pub at: i64,
    /// Why.
    pub reason: PairingRefusal,
}

/// One issued pairing code.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pairing {
    space: String,
    expires_at: i64,
    used: bool,
}

/// The broker's pairing codes and its log of refusals.
#[derive(Debug)]
pub struct Pairings {
    codes: BTreeMap<String, Pairing>,
    refusals: Vec<RefusedPairing>,
}

impl Pairings {
    /// No codes, no refusals.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            codes: BTreeMap::new(),
            refusals: Vec::new(),
        }
    }

    /// Issues `code` for `space` at `now`. Returns when it expires.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::Full`] at [`MAX_PAIRINGS`] codes held.
    pub fn issue(&mut self, code: String, space: &str, now: i64) -> Result<i64, SpaceRefusal> {
        self.sweep(now);
        if self.codes.len() >= MAX_PAIRINGS {
            return Err(SpaceRefusal::Full);
        }
        let expires_at = now.saturating_add(PAIRING_TTL_SECS);
        self.codes.insert(
            code,
            Pairing {
                space: space.to_owned(),
                expires_at,
                used: false,
            },
        );
        Ok(expires_at)
    }

    /// Redeems `code` at `now`, once: returns the space it pairs into.
    ///
    /// # Errors
    ///
    /// The [`PairingRefusal`], which is also logged.
    pub fn redeem(&mut self, code: &str, now: i64) -> Result<String, PairingRefusal> {
        let outcome = match self.codes.get_mut(code) {
            None => Err(PairingRefusal::Unknown),
            Some(pairing) if pairing.used => Err(PairingRefusal::Used),
            Some(pairing) if now >= pairing.expires_at => Err(PairingRefusal::Expired),
            Some(pairing) => {
                pairing.used = true;
                Ok(pairing.space.clone())
            }
        };
        if let Err(reason) = outcome {
            self.refusals.push(RefusedPairing { at: now, reason });
            let excess = self.refusals.len().saturating_sub(MAX_REFUSALS);
            self.refusals.drain(..excess);
        }
        outcome
    }

    /// Forgets codes a full lifetime past expiry: until then a used or
    /// expired code is still refused as what it is.
    pub fn sweep(&mut self, now: i64) {
        self.codes
            .retain(|_, pairing| now < pairing.expires_at.saturating_add(PAIRING_TTL_SECS));
    }

    /// The logged refusals, oldest first.
    #[must_use]
    pub fn refusals(&self) -> &[RefusedPairing] {
        &self.refusals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer as _, SigningKey};

    const NOW: i64 = 1_790_000_000;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn a_code_pairs_once_and_only_before_it_expires() {
        let mut pairings = Pairings::empty();
        assert_eq!(
            pairings.issue(String::from("code-1"), "spc-1", NOW),
            Ok(NOW + PAIRING_TTL_SECS)
        );
        assert_eq!(
            pairings.redeem("code-1", NOW + 10),
            Ok(String::from("spc-1"))
        );
        // Replayed: refused, and logged as such.
        assert_eq!(
            pairings.redeem("code-1", NOW + 11),
            Err(PairingRefusal::Used)
        );
        pairings
            .issue(String::from("code-2"), "spc-1", NOW)
            .unwrap();
        assert_eq!(
            pairings.redeem("code-2", NOW + PAIRING_TTL_SECS),
            Err(PairingRefusal::Expired)
        );
        pairings
            .issue(String::from("code-3"), "spc-1", NOW)
            .unwrap();
        assert_eq!(
            pairings.redeem("code-3", NOW + PAIRING_TTL_SECS - 1),
            Ok(String::from("spc-1")),
            "a second before expiry is still good"
        );
        assert_eq!(
            pairings.redeem("never-issued", NOW),
            Err(PairingRefusal::Unknown)
        );
        assert_eq!(
            pairings.refusals(),
            [
                RefusedPairing {
                    at: NOW + 11,
                    reason: PairingRefusal::Used
                },
                RefusedPairing {
                    at: NOW + PAIRING_TTL_SECS,
                    reason: PairingRefusal::Expired
                },
                RefusedPairing {
                    at: NOW,
                    reason: PairingRefusal::Unknown
                },
            ]
        );
    }

    #[test]
    fn codes_and_refusals_are_bounded() {
        let mut pairings = Pairings::empty();
        for n in 0..MAX_PAIRINGS {
            let code = format!("c{n}");
            assert!(pairings.issue(code, "spc-1", NOW).is_ok());
        }
        assert_eq!(
            pairings.issue(String::from("over"), "spc-1", NOW),
            Err(SpaceRefusal::Full)
        );
        // A used or expired code is remembered for a lifetime past expiry,
        // then forgotten, which frees its place.
        let later = NOW + 2 * PAIRING_TTL_SECS;
        assert_eq!(
            pairings.redeem("c0", later - 1),
            Err(PairingRefusal::Expired)
        );
        assert!(
            pairings
                .issue(String::from("fresh"), "spc-1", later)
                .is_ok()
        );
        assert_eq!(pairings.redeem("c0", later), Err(PairingRefusal::Unknown));
        for _ in 0..MAX_REFUSALS + 3 {
            let _ = pairings.redeem("nope", NOW);
        }
        assert_eq!(pairings.refusals().len(), MAX_REFUSALS);
        assert_eq!(
            (PAIRING_TTL_SECS, MAX_PAIRINGS, MAX_REFUSALS),
            (300, 16, 32)
        );
    }

    #[test]
    fn a_device_is_named_by_its_key() {
        let id = device_id(&key(7).verifying_key());
        assert!(id.starts_with("dev-") && id.len() == 68, "{id}");
        assert_eq!(id, device_id(&key(7).verifying_key()));
        assert_ne!(id, device_id(&key(8).verifying_key()));
    }

    #[test]
    fn nonces_and_keys_are_checked_for_shape() {
        assert!(is_nonce(&"ab".repeat(32)));
        for bad in [
            "",
            &"ab".repeat(31),
            &"AB".repeat(32),
            &"zz".repeat(32),
            &"ab".repeat(33),
        ] {
            assert!(!is_nonce(bad), "{bad}");
        }
        let encoded = b64(key(1).verifying_key().as_bytes());
        assert_eq!(parse_key(&encoded), Some(key(1).verifying_key()));
        for bad in [String::from("not base64!"), b64(&[1; 31]), b64(&[1; 33])] {
            assert_eq!(parse_key(&bad), None, "{bad}");
        }
    }

    #[test]
    fn a_signature_verifies_only_for_its_key_and_its_transcript() {
        let device = key(3);
        let message = auth_transcript(&"ab".repeat(32), "dev-1");
        let signature = b64(&device.sign(&message).to_bytes());
        assert!(verifies(&device.verifying_key(), &message, &signature));
        // Another key, another transcript, another step's transcript.
        assert!(!verifies(&key(4).verifying_key(), &message, &signature));
        assert!(!verifies(
            &device.verifying_key(),
            &auth_transcript(&"cd".repeat(32), "dev-1"),
            &signature
        ));
        assert!(!verifies(
            &device.verifying_key(),
            &broker_transcript(&"ab".repeat(32), "dev-1"),
            &signature
        ));
        assert!(!verifies(&device.verifying_key(), &message, "garbage"));
        assert!(!verifies(&device.verifying_key(), &message, &b64(&[0; 64])));
    }

    #[test]
    fn the_transcripts_are_domain_separated() {
        assert_eq!(
            pair_transcript("n", "c", "k"),
            b"hatchling-device-pair-v1\nn\nc\nk".to_vec()
        );
        assert_eq!(
            auth_transcript("n", "d"),
            b"hatchling-device-auth-v1\nn\nd".to_vec()
        );
        assert_eq!(
            broker_transcript("n", "d"),
            b"hatchling-device-broker-v1\nn\nd".to_vec()
        );
    }
}
