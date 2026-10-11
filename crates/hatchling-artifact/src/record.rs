//! Builder claims, broker-asserted provenance, and the build record.
//!
//! A builder's output is a claim: the content it made, and optionally a
//! summary and a determinism claim. The broker parses it strictly. A
//! builder cannot write provenance, an output hash, or anything else the
//! broker owns: an unknown field makes the whole output malformed.
//!
//! Provenance is the broker's own: the inputs it pinned, the builder's
//! signing identity from admission, and the hash of the bytes
//! it received at completion. The builder's claims are kept beside it,
//! labelled as claims, and never trusted. So is the inspector's report
//! on the output, and the idea revision a build was made from.

use hatchling_core::{ArtifactInspection, BuildFailure, IdeaRef};
use serde::{Deserialize, Serialize};

use crate::deliver::DeliveryRecord;
use crate::request::{PinnedRequest, sha256_hex};

/// Most delivery records one build keeps; the oldest go first.
pub const MAX_DELIVERY_RECORDS: usize = 16;

/// A builder's output, as the wire carries it. Strict: an unknown field
/// (such as `provenance` or `output_hash`) is refused.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOutput {
    content: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    deterministic: bool,
}

/// What a builder claims it made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderClaim {
    /// The artifact content. The only part that is delivered, and only
    /// after the gate judges it.
    pub content: String,
    /// The builder's own description of its output. Recorded, never
    /// trusted: the gate does not read it.
    pub summary: Option<String>,
    /// Whether the builder claims identical inputs give identical bytes.
    pub deterministic: bool,
}

impl BuilderClaim {
    /// Parses a builder's raw output.
    ///
    /// # Errors
    ///
    /// Returns [`BuildFailure::BadOutput`] if it is not exactly a builder
    /// result: not JSON, no content, or any field the broker owns.
    pub fn parse(raw: &str) -> Result<Self, BuildFailure> {
        let wire: WireOutput = serde_json::from_str(raw).map_err(|_| BuildFailure::BadOutput)?;
        Ok(Self {
            content: wire.content,
            summary: wire.summary,
            deterministic: wire.deterministic,
        })
    }
}

/// Who built an artifact: the skill, and the key that signed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderIdentity {
    /// The builder's skill id, `name@version`.
    pub skill: String,
    /// The fingerprint of the key that signed the builder's manifest.
    pub signer: String,
}

/// Longest skill id an attribution shows, in bytes.
pub const MAX_SHOWN_ID_BYTES: usize = 128;

/// Whether `signer` is a whole key fingerprint: 64 lowercase hex digits.
fn is_fingerprint(signer: &str) -> bool {
    signer.len() == 64
        && signer
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
}

/// Whether `skill` can be shown as it is: a plain id, with no spaces or
/// punctuation that could pass for words of the attribution itself.
fn is_plain_id(skill: &str) -> bool {
    !skill.is_empty()
        && skill.len() <= MAX_SHOWN_ID_BYTES
        && skill
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
}

/// How a UI names who built an artifact.
///
/// `identity` is the builder identity provenance recorded, if any.
/// `current_signer` is the fingerprint of the key that signed the skill
/// registered under that id *now*, if one is. The attribution says
/// "verified builder" only when both are there and agree, and the
/// identity is well formed: a missing identity, a builder no longer
/// registered, one now signed by another key, or a forged record never
/// renders as verified, and never echoes text that could read as if it
/// did.
#[must_use]
pub fn attribution(identity: Option<&BuilderIdentity>, current_signer: Option<&str>) -> String {
    let Some(identity) = identity else {
        return String::from("builder unknown: not verified");
    };
    if !is_plain_id(&identity.skill) {
        return String::from("built by an unnamed builder: not verified");
    }
    if is_fingerprint(&identity.signer) && current_signer == Some(identity.signer.as_str()) {
        let short: String = identity.signer.chars().take(16).collect();
        format!(
            "built by verified builder {} (signer {short})",
            identity.skill
        )
    } else {
        format!("built by {}: not verified", identity.skill)
    }
}

/// One input as provenance names it: no content, only its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRef {
    /// The input's name.
    pub name: String,
    /// SHA-256 of its content.
    pub sha256: String,
}

/// What built an artifact, as the broker observed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The build id.
    pub build: String,
    /// The build kind.
    pub kind: String,
    /// The pinned inputs, by name and hash.
    pub inputs: Vec<InputRef>,
    /// The hash over all of them ([`PinnedRequest::inputs_hash`]).
    pub inputs_hash: String,
    /// The builder, by signing identity.
    pub builder: BuilderIdentity,
    /// SHA-256 of the output bytes, computed by the broker.
    pub output_hash: String,
    /// The output's length in bytes.
    pub output_bytes: usize,
    /// The builder's determinism claim.
    pub deterministic_claim: bool,
    /// The builder's summary claim.
    pub summary_claim: Option<String>,
    /// What the broker's inspector found in the output.
    pub inspection: ArtifactInspection,
    /// The idea revision the build was made from, if any.
    pub idea: Option<IdeaRef>,
    /// When the build completed, in Unix seconds.
    pub completed_at: i64,
}

impl Provenance {
    /// The provenance of `claim`, built by `builder` for `record`, as the
    /// broker observed and inspected it at `completed_at`. The output
    /// hash is computed here, from the bytes, never taken from the
    /// builder.
    #[must_use]
    pub fn observe(
        record: &BuildRecord,
        builder: BuilderIdentity,
        claim: &BuilderClaim,
        inspection: ArtifactInspection,
        completed_at: i64,
    ) -> Self {
        let request = &record.request;
        Self {
            build: record.build.clone(),
            kind: request.kind.clone(),
            inputs: request
                .inputs
                .iter()
                .map(|input| InputRef {
                    name: input.name.clone(),
                    sha256: input.sha256.clone(),
                })
                .collect(),
            inputs_hash: request.inputs_hash(),
            builder,
            output_hash: sha256_hex(claim.content.as_bytes()),
            output_bytes: claim.content.len(),
            deterministic_claim: claim.deterministic,
            summary_claim: claim.summary.clone(),
            inspection,
            idea: record.idea.clone(),
            completed_at,
        }
    }
}

/// Where a build stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum BuildStatus {
    /// Requested; the builder has not finished. Never deliverable.
    #[serde(rename = "pending")]
    Pending,
    /// The builder finished and its output was accepted.
    #[serde(rename = "completed")]
    Completed,
    /// The build failed. Never deliverable.
    #[serde(rename = "failed")]
    Failed {
        /// Why.
        reason: String,
    },
}

/// One build, as the broker keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildRecord {
    /// The build id.
    pub build: String,
    /// Where it stands.
    pub status: BuildStatus,
    /// The pinned request: what the builder received.
    pub request: PinnedRequest,
    /// The idea revision the request was made from, if any.
    pub idea: Option<IdeaRef>,
    /// The provenance, once completed.
    pub provenance: Option<Provenance>,
    /// The output bytes, once completed.
    pub content: Option<String>,
    /// Every delivery attempt, with its gate verdict.
    pub deliveries: Vec<DeliveryRecord>,
}

impl BuildRecord {
    /// A build that has been requested and not yet run.
    #[must_use]
    pub const fn pending(build: String, request: PinnedRequest) -> Self {
        Self {
            build,
            status: BuildStatus::Pending,
            request,
            idea: None,
            provenance: None,
            content: None,
            deliveries: Vec::new(),
        }
    }

    /// This pending build, made from the idea revision `idea`.
    #[must_use]
    pub fn from_idea(mut self, idea: IdeaRef) -> Self {
        self.idea = Some(idea);
        self
    }

    /// Records the builder's accepted output, with the provenance the
    /// broker observed and its inspector's report.
    pub fn complete(
        &mut self,
        builder: BuilderIdentity,
        claim: BuilderClaim,
        inspection: ArtifactInspection,
        completed_at: i64,
    ) {
        self.provenance = Some(Provenance::observe(
            self,
            builder,
            &claim,
            inspection,
            completed_at,
        ));
        self.content = Some(claim.content);
        self.status = BuildStatus::Completed;
    }

    /// Records a failure. Whatever the builder produced is dropped.
    pub fn fail(&mut self, reason: String) {
        self.status = BuildStatus::Failed { reason };
        self.provenance = None;
        self.content = None;
    }

    /// Keeps a delivery attempt, dropping the oldest past
    /// [`MAX_DELIVERY_RECORDS`].
    pub fn record_delivery(&mut self, delivery: DeliveryRecord) {
        self.deliveries.push(delivery);
        let excess = self.deliveries.len().saturating_sub(MAX_DELIVERY_RECORDS);
        self.deliveries.drain(..excess);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deliver::DeliveryRecord;
    use hatchling_core::ArtifactInput;

    fn request() -> PinnedRequest {
        PinnedRequest::pin(
            "document",
            &[ArtifactInput {
                name: String::from("body"),
                content: String::from("hello"),
            }],
        )
        .unwrap()
    }

    fn identity() -> BuilderIdentity {
        BuilderIdentity {
            skill: String::from("doc@1.0.0"),
            signer: "ab".repeat(32),
        }
    }

    fn passed() -> ArtifactInspection {
        ArtifactInspection {
            verdict: hatchling_core::InspectionVerdict::Pass,
            content_type: String::from("text/markdown"),
            findings: Vec::new(),
        }
    }

    fn idea() -> IdeaRef {
        IdeaRef {
            idea: String::from("idea-1"),
            revision: 3,
            sha256: sha256_hex(b"hello"),
        }
    }

    #[test]
    fn a_builder_claim_is_parsed_strictly() {
        let claim =
            BuilderClaim::parse(r#"{"content":"out","summary":"clean","deterministic":true}"#)
                .unwrap();
        assert_eq!(
            claim,
            BuilderClaim {
                content: String::from("out"),
                summary: Some(String::from("clean")),
                deterministic: true,
            }
        );
        let bare = BuilderClaim::parse(r#"{"content":""}"#).unwrap();
        assert_eq!((bare.summary, bare.deterministic), (None, false));
        for forged in [
            // Fields only the broker writes.
            r#"{"content":"x","provenance":{"builder":"trusted"}}"#,
            r#"{"content":"x","output_hash":"00"}"#,
            r#"{"content":"x","signer":"trusted"}"#,
            // Not a builder result at all.
            r#"{"summary":"clean"}"#,
            r#"{"content":7}"#,
            "content",
            "",
        ] {
            assert_eq!(
                BuilderClaim::parse(forged),
                Err(BuildFailure::BadOutput),
                "{forged}"
            );
        }
    }

    #[test]
    fn provenance_is_what_the_broker_observed() {
        let claim = BuilderClaim {
            content: String::from("the bytes"),
            summary: Some(String::from("clean")),
            deterministic: true,
        };
        let record = BuildRecord::pending(String::from("art-1"), request()).from_idea(idea());
        let provenance = Provenance::observe(&record, identity(), &claim, passed(), 1_790_000_000);
        assert_eq!(provenance.build, "art-1");
        assert_eq!(provenance.inspection, passed());
        assert_eq!(provenance.idea, Some(idea()));
        assert_eq!(provenance.kind, "document");
        assert_eq!(
            provenance.inputs,
            [InputRef {
                name: String::from("body"),
                sha256: sha256_hex(b"hello"),
            }]
        );
        assert_eq!(provenance.inputs_hash, request().inputs_hash());
        assert_eq!(provenance.builder, identity());
        assert_eq!(provenance.output_hash, sha256_hex(b"the bytes"));
        assert_eq!(provenance.output_bytes, 9);
        assert!(provenance.deterministic_claim);
        assert_eq!(provenance.summary_claim.as_deref(), Some("clean"));
        assert_eq!(provenance.completed_at, 1_790_000_000);
    }

    #[test]
    fn only_a_currently_vouched_signing_identity_renders_as_verified() {
        let signer = "ab".repeat(32);
        assert_eq!(
            attribution(Some(&identity()), Some(&signer)),
            "built by verified builder doc@1.0.0 (signer abababababababab)"
        );
        let other = "cd".repeat(32);
        let upper = "AB".repeat(32);
        let short = "ab".repeat(31);
        let unverified = [
            // Missing.
            (None, Some(signer.as_str()), "builder unknown: not verified"),
            // No longer registered, or now signed by another key.
            (Some(identity()), None, "built by doc@1.0.0: not verified"),
            (
                Some(identity()),
                Some(other.as_str()),
                "built by doc@1.0.0: not verified",
            ),
            // Malformed signers, even when they "match".
            (
                Some(BuilderIdentity {
                    skill: String::from("doc@1.0.0"),
                    signer: String::from("trusted"),
                }),
                Some("trusted"),
                "built by doc@1.0.0: not verified",
            ),
            (
                Some(BuilderIdentity {
                    skill: String::from("doc@1.0.0"),
                    signer: upper.clone(),
                }),
                Some(upper.as_str()),
                "built by doc@1.0.0: not verified",
            ),
            (
                Some(BuilderIdentity {
                    skill: String::from("doc@1.0.0"),
                    signer: short.clone(),
                }),
                Some(short.as_str()),
                "built by doc@1.0.0: not verified",
            ),
            // A forged name that reads like a verdict is never echoed.
            (
                Some(BuilderIdentity {
                    skill: String::from("x (verified builder trusted@1.0.0)"),
                    signer: signer.clone(),
                }),
                Some(signer.as_str()),
                "built by an unnamed builder: not verified",
            ),
            (
                Some(BuilderIdentity {
                    skill: String::new(),
                    signer: signer.clone(),
                }),
                Some(signer.as_str()),
                "built by an unnamed builder: not verified",
            ),
            (
                Some(BuilderIdentity {
                    skill: "d".repeat(MAX_SHOWN_ID_BYTES + 1),
                    signer: signer.clone(),
                }),
                Some(signer.as_str()),
                "built by an unnamed builder: not verified",
            ),
        ];
        for (identity, current, expected) in unverified {
            let shown = attribution(identity.as_ref(), current);
            assert_eq!(shown, expected, "{identity:?}");
            assert!(!shown.contains("verified builder"), "{shown}");
        }
        let long = BuilderIdentity {
            skill: "d".repeat(MAX_SHOWN_ID_BYTES),
            signer: signer.clone(),
        };
        assert!(attribution(Some(&long), Some(&signer)).starts_with("built by verified builder"));
        assert_eq!(MAX_SHOWN_ID_BYTES, 128);
    }

    #[test]
    fn a_record_moves_from_pending_to_completed_or_failed() {
        let mut record = BuildRecord::pending(String::from("art-1"), request());
        assert_eq!(record.status, BuildStatus::Pending);
        assert_eq!(
            (record.provenance.as_ref(), record.content.as_ref()),
            (None, None)
        );
        let claim = BuilderClaim {
            content: String::from("out"),
            summary: None,
            deterministic: false,
        };
        record.complete(identity(), claim, passed(), 7);
        assert_eq!(record.status, BuildStatus::Completed);
        assert_eq!(record.idea, None);
        assert_eq!(
            record.provenance.as_ref().map(|p| p.idea.clone()),
            Some(None)
        );
        assert_eq!(record.content.as_deref(), Some("out"));
        assert_eq!(
            record.provenance.as_ref().map(|p| p.output_hash.clone()),
            Some(sha256_hex(b"out"))
        );
        record.fail(String::from("later failure"));
        assert_eq!(
            record.status,
            BuildStatus::Failed {
                reason: String::from("later failure")
            }
        );
        assert_eq!((record.provenance, record.content), (None, None));
        // Round-trips as stored.
        let pending = BuildRecord::pending(String::from("art-2"), request());
        let back: BuildRecord =
            serde_json::from_str(&serde_json::to_string(&pending).unwrap()).unwrap();
        assert_eq!(back, pending);
    }

    #[test]
    fn delivery_records_are_bounded_oldest_first() {
        let mut record = BuildRecord::pending(String::from("art-1"), request());
        for at in 0..=i64::try_from(MAX_DELIVERY_RECORDS).unwrap() {
            record.record_delivery(DeliveryRecord {
                target: String::from("channel:local"),
                verdict: None,
                refusal: None,
                at,
            });
        }
        assert_eq!(record.deliveries.len(), MAX_DELIVERY_RECORDS);
        assert_eq!(record.deliveries.first().map(|d| d.at), Some(1));
        assert_eq!(
            record.deliveries.last().map(|d| d.at),
            Some(i64::try_from(MAX_DELIVERY_RECORDS).unwrap())
        );
        assert_eq!(MAX_DELIVERY_RECORDS, 16);
    }
}
