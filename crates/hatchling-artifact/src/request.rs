//! Build requests, pinned.
//!
//! The broker pins a request the moment it receives it: each input's
//! content is hashed, and from then on the request *is* those bytes. The
//! builder receives exactly the pinned request ([`PinnedRequest::builder_input`]),
//! and the record keeps it, so a build is reproducible from its record
//! alone.

use hatchling_core::{ArtifactInput, BuildFailure, MAX_SKILL_INPUT_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// The capability a skill must declare to run as a builder. The skill
/// gate refuses any skill that does not declare it.
pub const ARTIFACT_BUILD_CAPABILITY: &str = "artifact.build";

/// Most inputs one build may pin.
pub const MAX_INPUTS: usize = 16;

/// Longest build kind or input name, in bytes.
pub const MAX_NAME_BYTES: usize = 64;

/// SHA-256 of `bytes`, lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// `bytes` as lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// Whether `name` is a token, as kinds and input names must be.
///
/// A token is a lowercase ASCII letter or digit, then lowercase letters,
/// digits, `.`, `_`, or `-`, at most [`MAX_NAME_BYTES`] in all. Tokens
/// hold no newline, so the inputs hash's encoding needs no escaping.
#[must_use]
pub fn is_token(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= MAX_NAME_BYTES
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// One pinned input: its name, the hash of its content, and the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedInput {
    /// The input's name.
    pub name: String,
    /// SHA-256 of the content, lowercase hex.
    pub sha256: String,
    /// The content, exactly as pinned.
    pub content: String,
}

/// A build request after pinning: the kind and the pinned inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedRequest {
    /// The build kind.
    pub kind: String,
    /// The pinned inputs, in request order.
    pub inputs: Vec<PinnedInput>,
}

impl PinnedRequest {
    /// Pins a request: validates it and hashes every input.
    ///
    /// # Errors
    ///
    /// Returns [`BuildFailure::BadRequest`] for a kind or input name that
    /// is not a token, a duplicate input name, more than [`MAX_INPUTS`]
    /// inputs, or a request too large for a builder to receive.
    pub fn pin(kind: &str, inputs: &[ArtifactInput]) -> Result<Self, BuildFailure> {
        if !is_token(kind) || inputs.len() > MAX_INPUTS {
            return Err(BuildFailure::BadRequest);
        }
        let mut pinned: Vec<PinnedInput> = Vec::with_capacity(inputs.len());
        for input in inputs {
            if !is_token(&input.name) || pinned.iter().any(|seen| seen.name == input.name) {
                return Err(BuildFailure::BadRequest);
            }
            pinned.push(PinnedInput {
                name: input.name.clone(),
                sha256: sha256_hex(input.content.as_bytes()),
                content: input.content.clone(),
            });
        }
        let request = Self {
            kind: kind.to_owned(),
            inputs: pinned,
        };
        if request.builder_input()?.len() > MAX_SKILL_INPUT_BYTES {
            return Err(BuildFailure::BadRequest);
        }
        Ok(request)
    }

    /// What the builder receives: this request as JSON. Nothing else
    /// reaches the builder.
    ///
    /// # Errors
    ///
    /// Returns [`BuildFailure::BadRequest`] if it does not encode.
    pub fn builder_input(&self) -> Result<String, BuildFailure> {
        serde_json::to_string(self).map_err(|_| BuildFailure::BadRequest)
    }

    /// The hash of the inputs: SHA-256 over a domain tag and, for each
    /// input in order, its name and content hash. Two builds have the
    /// same inputs hash exactly when they pinned the same inputs.
    #[must_use]
    pub fn inputs_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"hatchling-artifact-inputs-v1\n");
        for input in &self.inputs {
            hasher.update(input.name.as_bytes());
            hasher.update(b"\n");
            hasher.update(input.sha256.as_bytes());
            hasher.update(b"\n");
        }
        hex(&hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, content: &str) -> ArtifactInput {
        ArtifactInput {
            name: name.to_owned(),
            content: content.to_owned(),
        }
    }

    #[test]
    fn tokens_are_short_lowercase_and_plain() {
        for good in [
            "document",
            "a",
            "0",
            "page.v2",
            "my_doc-1",
            &"x".repeat(MAX_NAME_BYTES),
        ] {
            assert!(is_token(good), "{good}");
        }
        for bad in [
            "",
            "Document",
            "-doc",
            ".doc",
            "_doc",
            "doc/x",
            "doc x",
            "doc\n",
            "dóc",
            &"x".repeat(MAX_NAME_BYTES + 1),
        ] {
            assert!(!is_token(bad), "{bad:?}");
        }
        assert_eq!((MAX_NAME_BYTES, MAX_INPUTS), (64, 16));
        assert_eq!(ARTIFACT_BUILD_CAPABILITY, "artifact.build");
    }

    #[test]
    fn a_request_is_pinned_by_content_hash() {
        let request =
            PinnedRequest::pin("document", &[input("body", "hello"), input("title", "hi")])
                .unwrap();
        assert_eq!(request.kind, "document");
        assert_eq!(request.inputs.len(), 2);
        let body = request.inputs.first().unwrap();
        assert_eq!(body.name, "body");
        assert_eq!(body.content, "hello");
        assert_eq!(
            body.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(sha256_hex(b"hello"), body.sha256);
        // The builder gets the pinned request, hashes included.
        let sent: PinnedRequest = serde_json::from_str(&request.builder_input().unwrap()).unwrap();
        assert_eq!(sent, request);
    }

    #[test]
    fn the_inputs_hash_follows_the_inputs_and_nothing_else() {
        let pin = |kind: &str, inputs: &[ArtifactInput]| PinnedRequest::pin(kind, inputs).unwrap();
        let base = pin("document", &[input("a", "1"), input("b", "2")]);
        // Same inputs: same hash, whatever the kind.
        assert_eq!(
            base.inputs_hash(),
            pin("page", &[input("a", "1"), input("b", "2")]).inputs_hash()
        );
        assert_eq!(base.inputs_hash().len(), 64);
        for other in [
            pin("document", &[input("a", "1"), input("b", "3")]),
            pin("document", &[input("a", "1"), input("c", "2")]),
            pin("document", &[input("b", "2"), input("a", "1")]),
            pin("document", &[input("a", "1")]),
            pin("document", &[]),
        ] {
            assert_ne!(other.inputs_hash(), base.inputs_hash(), "{other:?}");
        }
        // The empty request has a fixed hash: the tag alone.
        assert_eq!(
            pin("document", &[]).inputs_hash(),
            sha256_hex(b"hatchling-artifact-inputs-v1\n")
        );
    }

    #[test]
    fn a_malformed_request_is_refused() {
        let refused = |kind: &str, inputs: &[ArtifactInput]| PinnedRequest::pin(kind, inputs).err();
        let bad = Some(BuildFailure::BadRequest);
        assert_eq!(refused("Doc", &[]), bad);
        assert_eq!(refused("document", &[input("Body", "x")]), bad);
        assert_eq!(
            refused("document", &[input("a", "1"), input("a", "2")]),
            bad
        );
        let many: Vec<ArtifactInput> = (0..=MAX_INPUTS)
            .map(|n| input(&format!("i{n}"), "x"))
            .collect();
        assert_eq!(refused("document", &many), bad);
        assert!(PinnedRequest::pin("document", many.get(..MAX_INPUTS).unwrap()).is_ok());
        // Too large for the builder to receive.
        let huge = "x".repeat(MAX_SKILL_INPUT_BYTES);
        assert_eq!(refused("document", &[input("a", &huge)]), bad);
    }

    #[test]
    fn the_size_bound_is_the_builder_input_s() {
        // The largest content that still fits, and one byte more.
        let overhead = PinnedRequest::pin("d", &[input("a", "")])
            .unwrap()
            .builder_input()
            .unwrap()
            .len();
        let fits = "x".repeat(MAX_SKILL_INPUT_BYTES - overhead);
        let request = PinnedRequest::pin("d", &[input("a", &fits)]).unwrap();
        assert_eq!(
            request.builder_input().unwrap().len(),
            MAX_SKILL_INPUT_BYTES
        );
        let over = format!("{fits}x");
        assert_eq!(
            PinnedRequest::pin("d", &[input("a", &over)]).err(),
            Some(BuildFailure::BadRequest)
        );
    }
}
