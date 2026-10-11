//! The one delivery function.
//!
//! Every way artifact bytes leave goes through [`deliver`]: channels,
//! workspace writes, and any future sharing affordance. A target is a
//! [`Sink`], and a sink is only ever handed bytes by [`deliver`], after
//! all three checks, in this order:
//!
//! 1. **The build completed.** A pending or failed build has no
//!    deliverable bytes; there is no preview path. A preview is a build
//!    kind of its own, with its own provenance.
//! 2. **The bytes are the recorded bytes.** They are re-hashed against
//!    the output hash recorded at completion, so nothing that changed
//!    since (in the store, or anywhere between) can leave.
//! 3. **The gate passes them.** Any verdict other than
//!    [`Verdict::Allow`] refuses, an error exactly as a block.
//!
//! The bytes checked are the bytes handed on: there is no second read
//! between the gate's verdict and the sink.

use hatchling_core::DeliveryRefusal;
use serde::{Deserialize, Serialize};

use crate::gate::{Reviewer, Verdict};
use crate::record::{BuildRecord, BuildStatus};
use crate::request::sha256_hex;

/// A delivery target.
pub trait Sink {
    /// The target, for the delivery record (`channel:local`,
    /// `workspace:report.md`).
    fn target(&self) -> String;

    /// Hands the target the bytes.
    ///
    /// # Errors
    ///
    /// Returns why the target could not take them.
    fn put(&mut self, bytes: &[u8]) -> Result<(), String>;
}

/// One delivery attempt, as the build record keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryRecord {
    /// Where it was going.
    pub target: String,
    /// The gate's verdict, if the bytes got as far as the gate.
    pub verdict: Option<Verdict>,
    /// Why it was refused; `None` means the bytes were delivered.
    pub refusal: Option<DeliveryRefusal>,
    /// When, in Unix seconds.
    pub at: i64,
}

impl DeliveryRecord {
    /// Whether the bytes were delivered.
    #[must_use]
    pub const fn delivered(&self) -> bool {
        self.refusal.is_none()
    }
}

/// Delivers `record`'s artifact to `sink`, if it is completed, its bytes
/// match the recorded hash, and `reviewer` passes them. Returns what
/// happened, for the record.
pub fn deliver(
    record: &BuildRecord,
    reviewer: &dyn Reviewer,
    sink: &mut dyn Sink,
    at: i64,
) -> DeliveryRecord {
    let target = sink.target();
    let outcome = |verdict: Option<Verdict>, refusal: Option<DeliveryRefusal>| DeliveryRecord {
        target: target.clone(),
        verdict,
        refusal,
        at,
    };
    let (BuildStatus::Completed, Some(provenance), Some(content)) =
        (&record.status, &record.provenance, &record.content)
    else {
        return outcome(None, Some(DeliveryRefusal::Incomplete));
    };
    let bytes = content.as_bytes();
    if sha256_hex(bytes) != provenance.output_hash {
        return outcome(None, Some(DeliveryRefusal::HashMismatch));
    }
    let verdict = reviewer.review(bytes);
    let refusal = match &verdict {
        Verdict::Allow => sink
            .put(bytes)
            .err()
            .map(|_| DeliveryRefusal::DeliveryFailed),
        Verdict::Block { .. } => Some(DeliveryRefusal::Blocked),
        Verdict::Error { .. } => Some(DeliveryRefusal::GateError),
    };
    outcome(Some(verdict), refusal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{BuilderClaim, BuilderIdentity};
    use crate::request::PinnedRequest;
    use hatchling_core::ArtifactInput;
    use std::cell::Cell;

    /// Records what it was handed.
    #[derive(Default)]
    struct Collect {
        got: Vec<Vec<u8>>,
        fail: bool,
    }

    impl Sink for Collect {
        fn target(&self) -> String {
            String::from("channel:local")
        }

        fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
            if self.fail {
                return Err(String::from("no client"));
            }
            self.got.push(bytes.to_vec());
            Ok(())
        }
    }

    /// A reviewer with a fixed verdict, counting its calls.
    struct Fixed(Verdict, Cell<usize>);

    impl Reviewer for Fixed {
        fn review(&self, _bytes: &[u8]) -> Verdict {
            self.1.set(self.1.get() + 1);
            self.0.clone()
        }
    }

    const fn allow() -> Fixed {
        Fixed(Verdict::Allow, Cell::new(0))
    }

    fn completed(content: &str) -> BuildRecord {
        let request = PinnedRequest::pin(
            "document",
            &[ArtifactInput {
                name: String::from("body"),
                content: String::from("in"),
            }],
        )
        .unwrap();
        let mut record = BuildRecord::pending(String::from("art-1"), request);
        record.complete(
            BuilderIdentity {
                skill: String::from("doc@1.0.0"),
                signer: String::from("ab"),
            },
            BuilderClaim {
                content: content.to_owned(),
                summary: Some(String::from("clean")),
                deterministic: true,
            },
            hatchling_core::ArtifactInspection {
                verdict: hatchling_core::InspectionVerdict::Pass,
                content_type: String::from("text/markdown"),
                findings: Vec::new(),
            },
            5,
        );
        record
    }

    #[test]
    fn a_completed_passed_build_is_delivered_byte_for_byte() {
        let mut sink = Collect::default();
        let reviewer = allow();
        let outcome = deliver(&completed("the artifact"), &reviewer, &mut sink, 9);
        assert_eq!(
            outcome,
            DeliveryRecord {
                target: String::from("channel:local"),
                verdict: Some(Verdict::Allow),
                refusal: None,
                at: 9,
            }
        );
        assert!(outcome.delivered());
        assert_eq!(sink.got, [b"the artifact".to_vec()]);
        assert_eq!(reviewer.1.get(), 1);
    }

    #[test]
    fn an_incomplete_build_is_refused_before_the_gate() {
        let pending = BuildRecord::pending(
            String::from("art-1"),
            PinnedRequest::pin("document", &[]).unwrap(),
        );
        let mut failed = completed("x");
        failed.fail(String::from("builder exited 3"));
        // Status alone is not enough: completed without content is not
        // deliverable either.
        let mut hollow = completed("x");
        hollow.content = None;
        let mut no_provenance = completed("x");
        no_provenance.provenance = None;
        for record in [pending, failed, hollow, no_provenance] {
            let mut sink = Collect::default();
            let reviewer = allow();
            let outcome = deliver(&record, &reviewer, &mut sink, 1);
            assert_eq!(
                outcome.refusal,
                Some(DeliveryRefusal::Incomplete),
                "{record:?}"
            );
            assert_eq!(outcome.verdict, None);
            assert!(!outcome.delivered());
            assert!(sink.got.is_empty());
            assert_eq!(reviewer.1.get(), 0, "never reaches the gate");
        }
    }

    #[test]
    fn bytes_that_changed_since_completion_are_refused() {
        // TOCTOU: the stored bytes no longer match the recorded hash.
        let mut tampered = completed("the artifact");
        tampered.content = Some(String::from("the artifact, plus a secret"));
        let mut sink = Collect::default();
        let reviewer = allow();
        let outcome = deliver(&tampered, &reviewer, &mut sink, 1);
        assert_eq!(outcome.refusal, Some(DeliveryRefusal::HashMismatch));
        assert!(sink.got.is_empty());
        assert_eq!(reviewer.1.get(), 0);
    }

    #[test]
    fn a_block_or_a_gate_error_refuses() {
        for (verdict, refusal) in [
            (
                Verdict::Block {
                    reason: String::from("secret"),
                },
                DeliveryRefusal::Blocked,
            ),
            (
                Verdict::Error {
                    reason: String::from("no policy"),
                },
                DeliveryRefusal::GateError,
            ),
        ] {
            let mut sink = Collect::default();
            let reviewer = Fixed(verdict.clone(), Cell::new(0));
            let outcome = deliver(&completed("x"), &reviewer, &mut sink, 1);
            assert_eq!(outcome.refusal, Some(refusal));
            assert_eq!(outcome.verdict, Some(verdict), "the verdict is recorded");
            assert!(sink.got.is_empty());
        }
    }

    #[test]
    fn a_failing_target_is_recorded_after_the_pass() {
        let mut sink = Collect {
            got: Vec::new(),
            fail: true,
        };
        let outcome = deliver(&completed("x"), &allow(), &mut sink, 1);
        assert_eq!(outcome.verdict, Some(Verdict::Allow));
        assert_eq!(outcome.refusal, Some(DeliveryRefusal::DeliveryFailed));
    }
}
