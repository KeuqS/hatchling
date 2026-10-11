//! Ideas: capture, list, develop, activate.
//!
//! An idea is a piece of text that moves through revisions. Capturing it
//! makes revision 1, and records **where it came from** ([`IdeaOrigin`]):
//! a chat, an explicit command, or a feed item, whose provenance label
//! the broker re-reads and keeps, transitively. Developing it refines the
//! text (the next revision, keeping the earlier revisions' hashes) or adds
//! a **note**.
//!
//! **Activation is a confirmation, not a side effect.** An idea becomes a
//! build only in three steps:
//!
//! 1. *Propose* ([`IdeaRecord::propose`]): the exact build request is
//!    pinned: the kind, the builder, the description, and every input by
//!    hash, under one request hash. The user is shown exactly that.
//! 2. *Confirm*: the broker mints a single-use confirmation for that
//!    activation when, and only when, the user confirms it. No session
//!    op can.
//! 3. *Activate*: the confirmation is spent, the request is pinned again
//!    from the idea, and the build runs only if its hash is the confirmed
//!    one ([`IdeaRecord::pinned`]).
//!
//! Refining an idea, or noting it, **withdraws** any activation awaiting
//! confirmation: what was proposed is no longer what the idea says, so
//! the confirmation has nothing left to activate. There is no path from
//! an idea to an artifact that skips the confirmation.

use hatchling_core::{
    ActivationInput, ActivationProposal, ArtifactInput, BuildFailure, IdeaRef, IdeaRefusal,
    IdeaState, IdeaSummary,
};
use serde::{Deserialize, Serialize};

use crate::request::{PinnedRequest, sha256_hex};

/// Longest idea text, in bytes.
pub const MAX_IDEA_BYTES: usize = 4096;

/// Most earlier-revision hashes an idea keeps; the oldest go first.
pub const MAX_IDEA_HISTORY: usize = 16;

/// The name an idea's text is pinned under when it is built.
pub const IDEA_INPUT: &str = "idea";

/// The name an idea's notes are pinned under, when it has any.
pub const NOTES_INPUT: &str = "notes";

/// Most development notes one idea keeps.
pub const MAX_NOTES: usize = 4;

/// Longest note, in bytes.
pub const MAX_NOTE_BYTES: usize = 400;

/// Longest activation description, in bytes.
pub const MAX_DESCRIPTION_BYTES: usize = 200;

/// Most activations one idea remembers; the oldest go first.
pub const MAX_ACTIVATIONS: usize = 4;

/// Longest title a list shows, in characters.
pub const MAX_TITLE_CHARS: usize = 80;

/// Where an idea was captured from, as the broker recorded it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from")]
pub enum IdeaOrigin {
    /// Not stated (an idea captured before, or without `from`).
    #[default]
    #[serde(rename = "unstated")]
    Unstated,
    /// A chat with the user.
    #[serde(rename = "chat")]
    Chat,
    /// An explicit command from the user.
    #[serde(rename = "command")]
    Command,
    /// A feed item, with the provenance label the broker read for it.
    #[serde(rename = "feed")]
    Feed {
        /// The item's label, transitive (`from tracking (originally from
        /// whatsapp)`).
        label: String,
    },
}

impl IdeaOrigin {
    /// The label a list shows.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Unstated => String::from("captured (source not stated)"),
            Self::Chat => String::from("captured from chat"),
            Self::Command => String::from("captured by explicit command"),
            Self::Feed { label } => format!("captured from the feed: {label}"),
        }
    }
}

/// One confirmed activation an idea was built through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activation {
    /// The activation id.
    pub activation: String,
    /// The build it made.
    pub build: String,
    /// The idea revision it built.
    pub revision: u32,
}

/// One idea, as the broker keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdeaRecord {
    /// The idea id.
    pub idea: String,
    /// The current revision, from 1.
    pub revision: u32,
    /// The current text.
    pub text: String,
    /// SHA-256 of the current text.
    pub sha256: String,
    /// The earlier revisions' hashes, oldest first, at most
    /// [`MAX_IDEA_HISTORY`].
    pub history: Vec<String>,
    /// When it last changed, in Unix seconds.
    pub updated_at: i64,
    /// Where it was captured from.
    #[serde(default)]
    pub origin: IdeaOrigin,
    /// Its development notes, oldest first, at most [`MAX_NOTES`].
    #[serde(default)]
    pub notes: Vec<String>,
    /// The activation awaiting the user's confirmation, if any.
    #[serde(default)]
    pub proposal: Option<ActivationProposal>,
    /// The activations it was built through, oldest first, at most
    /// [`MAX_ACTIVATIONS`].
    #[serde(default)]
    pub activations: Vec<Activation>,
}

/// Refuses blank text and text over [`MAX_IDEA_BYTES`].
fn check_text(text: &str) -> Result<(), IdeaRefusal> {
    if text.trim().is_empty() || text.len() > MAX_IDEA_BYTES {
        return Err(IdeaRefusal::BadText);
    }
    Ok(())
}

/// Whether `text` is non-blank, at most `max` bytes, and free of control
/// characters other than `allowed`.
fn is_short_text(text: &str, max: usize, allowed: &[char]) -> bool {
    !text.trim().is_empty()
        && text.len() <= max
        && text
            .chars()
            .all(|ch| !ch.is_control() || allowed.contains(&ch))
}

/// The hash an activation pins: a domain tag, then the kind, the builder,
/// the description, and the inputs hash, each on its own line.
#[must_use]
pub fn request_hash(request: &PinnedRequest, builder: &str, description: &str) -> String {
    sha256_hex(
        format!(
            "hatchling-activation-v1\n{}\n{builder}\n{description}\n{}\n",
            request.kind,
            request.inputs_hash()
        )
        .as_bytes(),
    )
}

impl IdeaRecord {
    /// A newly captured idea `idea`: `text` at revision 1, from `origin`.
    ///
    /// # Errors
    ///
    /// [`IdeaRefusal::BadText`] if `text` is blank or too long.
    pub fn capture(
        idea: String,
        text: &str,
        origin: IdeaOrigin,
        at: i64,
    ) -> Result<Self, IdeaRefusal> {
        check_text(text)?;
        Ok(Self {
            idea,
            revision: 1,
            text: text.to_owned(),
            sha256: sha256_hex(text.as_bytes()),
            history: Vec::new(),
            updated_at: at,
            origin,
            notes: Vec::new(),
            proposal: None,
            activations: Vec::new(),
        })
    }

    /// Refines the idea to `text`: the next revision. Withdraws any
    /// activation awaiting confirmation.
    ///
    /// # Errors
    ///
    /// [`IdeaRefusal::BadText`] if `text` is blank or too long; the idea
    /// is unchanged.
    pub fn refine(&mut self, text: &str, at: i64) -> Result<(), IdeaRefusal> {
        check_text(text)?;
        let earlier = std::mem::replace(&mut self.sha256, sha256_hex(text.as_bytes()));
        self.history.push(earlier);
        let excess = self.history.len().saturating_sub(MAX_IDEA_HISTORY);
        self.history.drain(..excess);
        text.clone_into(&mut self.text);
        self.revision = self.revision.saturating_add(1);
        self.updated_at = at;
        self.proposal = None;
        Ok(())
    }

    /// Adds a development note. Withdraws any activation awaiting
    /// confirmation.
    ///
    /// # Errors
    ///
    /// [`IdeaRefusal::BadNote`] for a blank or long note, or one with a
    /// control character other than a newline;
    /// [`IdeaRefusal::TooManyNotes`] at [`MAX_NOTES`]. Either way the
    /// idea is unchanged.
    pub fn note(&mut self, note: &str, at: i64) -> Result<(), IdeaRefusal> {
        if !is_short_text(note, MAX_NOTE_BYTES, &['\n']) {
            return Err(IdeaRefusal::BadNote);
        }
        if self.notes.len() >= MAX_NOTES {
            return Err(IdeaRefusal::TooManyNotes);
        }
        self.notes.push(note.trim().to_owned());
        self.updated_at = at;
        self.proposal = None;
        Ok(())
    }

    /// The current revision, as a build's provenance names it.
    #[must_use]
    pub fn reference(&self) -> IdeaRef {
        IdeaRef {
            idea: self.idea.clone(),
            revision: self.revision,
            sha256: self.sha256.clone(),
        }
    }

    /// The current revision as a build input, [`IDEA_INPUT`].
    #[must_use]
    pub fn input(&self) -> ArtifactInput {
        ArtifactInput {
            name: String::from(IDEA_INPUT),
            content: self.text.clone(),
        }
    }

    /// What an activation builds from: the text, and the notes under
    /// [`NOTES_INPUT`] when there are any.
    #[must_use]
    pub fn activation_inputs(&self) -> Vec<ArtifactInput> {
        let mut inputs = vec![self.input()];
        if !self.notes.is_empty() {
            inputs.push(ArtifactInput {
                name: String::from(NOTES_INPUT),
                content: self.notes.join("\n\n"),
            });
        }
        inputs
    }

    /// The request an activation of `kind` pins, from the idea as it is
    /// now.
    ///
    /// # Errors
    ///
    /// [`BuildFailure::BadRequest`] if `kind` is not a token.
    pub fn pin(&self, kind: &str) -> Result<PinnedRequest, BuildFailure> {
        PinnedRequest::pin(kind, &self.activation_inputs())
    }

    /// Proposes activation `activation`: building `request` (pinned from
    /// this idea by [`pin`](Self::pin)) with `builder`, for
    /// `description`. Replaces any earlier proposal. Returns what the
    /// user is asked to confirm.
    ///
    /// # Errors
    ///
    /// [`IdeaRefusal::BadDescription`] for a blank or long description,
    /// or one with a control character; the idea is unchanged.
    pub fn propose(
        &mut self,
        activation: String,
        request: &PinnedRequest,
        builder: &str,
        description: &str,
    ) -> Result<ActivationProposal, IdeaRefusal> {
        if !is_short_text(description, MAX_DESCRIPTION_BYTES, &[]) {
            return Err(IdeaRefusal::BadDescription);
        }
        let description = description.trim();
        let proposal = ActivationProposal {
            activation,
            idea: self.reference(),
            build_kind: request.kind.clone(),
            builder: builder.to_owned(),
            description: description.to_owned(),
            inputs: request
                .inputs
                .iter()
                .map(|input| ActivationInput {
                    name: input.name.clone(),
                    sha256: input.sha256.clone(),
                })
                .collect(),
            request_hash: request_hash(request, builder, description),
        };
        self.proposal = Some(proposal.clone());
        Ok(proposal)
    }

    /// The request to build for `activation`, pinned again from the idea,
    /// if `activation` awaits confirmation and the idea still pins exactly
    /// what was proposed.
    #[must_use]
    pub fn pinned(&self, activation: &str) -> Option<(PinnedRequest, &ActivationProposal)> {
        let proposal = self
            .proposal
            .as_ref()
            .filter(|proposal| proposal.activation == activation)?;
        let request = self.pin(&proposal.build_kind).ok()?;
        (request_hash(&request, &proposal.builder, &proposal.description) == proposal.request_hash)
            .then_some((request, proposal))
    }

    /// Records that the activation awaiting confirmation built `build`,
    /// and clears it.
    pub fn activated(&mut self, build: String) {
        if let Some(proposal) = self.proposal.take() {
            self.activations.push(Activation {
                activation: proposal.activation,
                build,
                revision: proposal.idea.revision,
            });
            let excess = self.activations.len().saturating_sub(MAX_ACTIVATIONS);
            self.activations.drain(..excess);
        }
    }

    /// Where the idea stands.
    #[must_use]
    pub fn state(&self) -> IdeaState {
        if self.proposal.is_some() {
            IdeaState::AwaitingConfirmation
        } else if !self.activations.is_empty() {
            IdeaState::Activated
        } else if self.revision > 1 || !self.notes.is_empty() {
            IdeaState::Developing
        } else {
            IdeaState::Captured
        }
    }

    /// The idea as a list shows it.
    #[must_use]
    pub fn summary(&self) -> IdeaSummary {
        let first = self.text.trim().lines().next().unwrap_or_default();
        IdeaSummary {
            idea: self.idea.clone(),
            title: first
                .chars()
                .filter(|ch| !ch.is_control())
                .take(MAX_TITLE_CHARS)
                .collect(),
            revision: self.revision,
            notes: u32::try_from(self.notes.len()).unwrap_or(u32::MAX),
            state: self.state(),
            origin: self.origin.label(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(text: &str) -> IdeaRecord {
        IdeaRecord::capture(String::from("idea-1"), text, IdeaOrigin::Chat, 5).unwrap()
    }

    #[test]
    fn an_idea_is_captured_at_revision_one() {
        let idea = captured("a zine");
        assert_eq!(
            idea,
            IdeaRecord {
                idea: String::from("idea-1"),
                revision: 1,
                text: String::from("a zine"),
                sha256: sha256_hex(b"a zine"),
                history: Vec::new(),
                updated_at: 5,
                origin: IdeaOrigin::Chat,
                notes: Vec::new(),
                proposal: None,
                activations: Vec::new(),
            }
        );
        assert_eq!(
            idea.reference(),
            IdeaRef {
                idea: String::from("idea-1"),
                revision: 1,
                sha256: sha256_hex(b"a zine"),
            }
        );
        assert_eq!(
            idea.input(),
            ArtifactInput {
                name: String::from("idea"),
                content: String::from("a zine"),
            }
        );
        let at_bound = "x".repeat(MAX_IDEA_BYTES);
        assert!(IdeaRecord::capture(String::from("i"), &at_bound, IdeaOrigin::Unstated, 0).is_ok());
        for bad in [String::new(), String::from(" \n\t"), format!("{at_bound}x")] {
            assert_eq!(
                IdeaRecord::capture(String::from("i"), &bad, IdeaOrigin::Unstated, 0),
                Err(IdeaRefusal::BadText)
            );
        }
        assert_eq!(
            (MAX_IDEA_BYTES, MAX_IDEA_HISTORY, IDEA_INPUT, NOTES_INPUT),
            (4096, 16, "idea", "notes")
        );
        assert_eq!(
            (
                MAX_NOTES,
                MAX_NOTE_BYTES,
                MAX_DESCRIPTION_BYTES,
                MAX_ACTIVATIONS,
                MAX_TITLE_CHARS
            ),
            (4, 400, 200, 4, 80)
        );
    }

    #[test]
    fn a_refinement_moves_the_revision_on_and_keeps_the_earlier_hash() {
        let mut idea = captured("a zine");
        idea.refine("a zine, eight pages", 9).unwrap();
        assert_eq!(idea.revision, 2);
        assert_eq!(idea.text, "a zine, eight pages");
        assert_eq!(idea.sha256, sha256_hex(b"a zine, eight pages"));
        assert_eq!(idea.history, [sha256_hex(b"a zine")]);
        assert_eq!(idea.updated_at, 9);
        // A refused refinement changes nothing.
        let before = idea.clone();
        assert_eq!(idea.refine("  ", 11), Err(IdeaRefusal::BadText));
        assert_eq!(idea, before);
    }

    #[test]
    fn the_history_is_bounded_oldest_first() {
        let mut idea = captured("r1");
        for revision in 2..=20 {
            idea.refine(&format!("r{revision}"), revision).unwrap();
        }
        assert_eq!(idea.revision, 20);
        assert_eq!(idea.history.len(), MAX_IDEA_HISTORY);
        // r1..r3 dropped: the oldest kept is r4, the newest r19.
        assert_eq!(idea.history.first(), Some(&sha256_hex(b"r4")));
        assert_eq!(idea.history.last(), Some(&sha256_hex(b"r19")));
        idea.revision = u32::MAX;
        idea.refine("again", 21).unwrap();
        assert_eq!(idea.revision, u32::MAX, "saturates");
    }

    #[test]
    fn each_origin_says_where_the_idea_came_from() {
        let labels: Vec<String> = [
            IdeaOrigin::Unstated,
            IdeaOrigin::Chat,
            IdeaOrigin::Command,
            IdeaOrigin::Feed {
                label: String::from("from tracking (originally from whatsapp)"),
            },
        ]
        .iter()
        .map(IdeaOrigin::label)
        .collect();
        assert_eq!(
            labels,
            [
                "captured (source not stated)",
                "captured from chat",
                "captured by explicit command",
                "captured from the feed: from tracking (originally from whatsapp)",
            ]
        );
        // A record from before reads as unstated, with nothing
        // awaiting and no notes.
        let old =
            r#"{"idea":"idea-1","revision":1,"text":"t","sha256":"s","history":[],"updated_at":0}"#;
        let idea: IdeaRecord = serde_json::from_str(old).unwrap();
        assert_eq!(idea.origin, IdeaOrigin::Unstated);
        assert_eq!(idea.state(), IdeaState::Captured);
    }

    #[test]
    fn notes_are_bounded_and_clean() {
        let mut idea = captured("a zine");
        let at_bound = "n".repeat(MAX_NOTE_BYTES);
        idea.note("  A5, \nfolded  ", 6).unwrap();
        idea.note(&at_bound, 7).unwrap();
        assert_eq!(idea.notes, [String::from("A5, \nfolded"), at_bound.clone()]);
        assert_eq!(idea.updated_at, 7);
        let before = idea.clone();
        for bad in [
            String::from(" "),
            format!("{at_bound}n"),
            String::from("a\tb"),
            String::from("a\u{7}b"),
        ] {
            assert_eq!(idea.note(&bad, 8), Err(IdeaRefusal::BadNote), "{bad:?}");
        }
        assert_eq!(idea, before);
        idea.note("three", 8).unwrap();
        idea.note("four", 8).unwrap();
        assert_eq!(idea.note("five", 9), Err(IdeaRefusal::TooManyNotes));
        assert_eq!(idea.notes.len(), MAX_NOTES);
    }

    #[test]
    fn a_proposal_pins_the_kind_builder_description_and_every_input() {
        let mut idea = captured("a zine");
        idea.note("A5", 6).unwrap();
        let request = idea.pin("document").unwrap();
        assert_eq!(
            idea.activation_inputs(),
            [
                ArtifactInput {
                    name: String::from("idea"),
                    content: String::from("a zine"),
                },
                ArtifactInput {
                    name: String::from("notes"),
                    content: String::from("A5"),
                },
            ]
        );
        let proposal = idea
            .propose(
                String::from("act-1"),
                &request,
                "doc@1.0.0",
                " a zine to print ",
            )
            .unwrap();
        assert_eq!(proposal.activation, "act-1");
        assert_eq!(proposal.idea, idea.reference());
        assert_eq!(
            (proposal.build_kind.as_str(), proposal.builder.as_str()),
            ("document", "doc@1.0.0")
        );
        assert_eq!(proposal.description, "a zine to print");
        assert_eq!(
            proposal.inputs,
            [
                ActivationInput {
                    name: String::from("idea"),
                    sha256: sha256_hex(b"a zine"),
                },
                ActivationInput {
                    name: String::from("notes"),
                    sha256: sha256_hex(b"A5"),
                },
            ]
        );
        assert_eq!(
            proposal.request_hash,
            sha256_hex(
                format!(
                    "hatchling-activation-v1\ndocument\ndoc@1.0.0\na zine to print\n{}\n",
                    request.inputs_hash()
                )
                .as_bytes()
            )
        );
        assert_eq!(idea.proposal.as_ref(), Some(&proposal));
        assert_eq!(idea.state(), IdeaState::AwaitingConfirmation);
        // The request pinned again for this activation is the same one.
        let (again, shown) = idea.pinned("act-1").unwrap();
        assert_eq!(again, request);
        assert_eq!(shown, &proposal);
        assert_eq!(idea.pinned("act-2"), None);
        assert_eq!(idea.pin("Not A Kind"), Err(BuildFailure::BadRequest));
        // Without notes, the text is the one input.
        assert_eq!(captured("x").activation_inputs().len(), 1);
    }

    #[test]
    fn every_change_to_the_idea_withdraws_what_awaits_confirmation() {
        for change in [0, 1] {
            let mut idea = captured("a zine");
            let request = idea.pin("document").unwrap();
            idea.propose(String::from("act-1"), &request, "doc@1.0.0", "d")
                .unwrap();
            if change == 0 {
                idea.refine("a different zine", 7).unwrap();
            } else {
                idea.note("bigger", 7).unwrap();
            }
            assert_eq!(idea.proposal, None);
            assert_eq!(idea.pinned("act-1"), None, "{change}");
            assert_eq!(idea.state(), IdeaState::Developing);
        }
        // An idea changed behind the proposal's back pins nothing.
        let mut idea = captured("a zine");
        let request = idea.pin("document").unwrap();
        idea.propose(String::from("act-1"), &request, "doc@1.0.0", "d")
            .unwrap();
        idea.text = String::from("swapped");
        assert_eq!(idea.pinned("act-1"), None);
    }

    #[test]
    fn a_description_is_short_and_clean() {
        let mut idea = captured("a zine");
        let request = idea.pin("document").unwrap();
        let at_bound = "d".repeat(MAX_DESCRIPTION_BYTES);
        assert!(
            idea.propose(String::from("act-1"), &request, "doc@1.0.0", &at_bound)
                .is_ok()
        );
        for bad in [
            String::from(" "),
            format!("{at_bound}d"),
            String::from("a\nb"),
        ] {
            assert_eq!(
                idea.propose(String::from("act-2"), &request, "doc@1.0.0", &bad),
                Err(IdeaRefusal::BadDescription)
            );
        }
        assert_eq!(
            idea.proposal.as_ref().map(|p| p.activation.as_str()),
            Some("act-1")
        );
    }

    #[test]
    fn an_activation_is_recorded_once_and_the_list_is_bounded() {
        let mut idea = captured("a zine");
        // Nothing awaits: nothing is recorded.
        idea.activated(String::from("art-0"));
        assert!(idea.activations.is_empty());
        for n in 1..=5 {
            idea.refine(&format!("zine {n}"), n).unwrap();
            let request = idea.pin("document").unwrap();
            idea.propose(format!("act-{n}"), &request, "doc@1.0.0", "d")
                .unwrap();
            idea.activated(format!("art-{n}"));
            assert_eq!(idea.proposal, None);
            assert_eq!(idea.state(), IdeaState::Activated);
        }
        assert_eq!(idea.activations.len(), MAX_ACTIVATIONS);
        assert_eq!(
            idea.activations.first(),
            Some(&Activation {
                activation: String::from("act-2"),
                build: String::from("art-2"),
                revision: 3,
            })
        );
        assert_eq!(
            idea.activations
                .last()
                .map(|a| (a.build.as_str(), a.revision)),
            Some(("art-5", 6))
        );
    }

    #[test]
    fn a_summary_shows_the_first_line_the_state_and_the_origin() {
        let mut idea = IdeaRecord::capture(
            String::from("idea-1"),
            &format!("\n  {}\u{7}\nmore", "t".repeat(MAX_TITLE_CHARS + 5)),
            IdeaOrigin::Command,
            0,
        )
        .unwrap();
        idea.note("n", 1).unwrap();
        assert_eq!(
            idea.summary(),
            IdeaSummary {
                idea: String::from("idea-1"),
                title: "t".repeat(MAX_TITLE_CHARS),
                revision: 1,
                notes: 1,
                state: IdeaState::Developing,
                origin: String::from("captured by explicit command"),
            }
        );
        assert_eq!(captured("x\ny").summary().title, "x");
    }

    #[test]
    fn the_largest_idea_still_fits_one_sealed_entry() {
        // Every byte a control character escapes to six: the worst case.
        let worst_text = "\u{1}".repeat(MAX_IDEA_BYTES);
        let mut idea = IdeaRecord::capture(
            String::from("idea-0123456789abcdef0123456789abcdef"),
            &worst_text,
            IdeaOrigin::Feed {
                label: "l".repeat(512),
            },
            0,
        )
        .unwrap();
        for _ in 1..MAX_IDEA_HISTORY + 2 {
            idea.refine(&worst_text, 1).unwrap();
        }
        for _ in 0..MAX_NOTES {
            idea.note(&format!("x{}x", "\n".repeat(MAX_NOTE_BYTES - 2)), 1)
                .unwrap();
        }
        // So large an idea is too large to build (its request would not
        // reach a builder), but a proposal may still be on record.
        assert_eq!(idea.pin("document"), Err(BuildFailure::BadRequest));
        let mut small = captured("x");
        let request = small.pin("document").unwrap();
        let proposal = small
            .propose(
                String::from("act-0123456789abcdef0123456789abcdef"),
                &request,
                "builder-with-a-long-name@10.10.10",
                &"d".repeat(MAX_DESCRIPTION_BYTES),
            )
            .unwrap();
        idea.proposal = Some(proposal);
        for n in 0..MAX_ACTIVATIONS {
            idea.activations.push(Activation {
                activation: format!("act-{n:032}"),
                build: format!("art-{n:032}"),
                revision: u32::MAX,
            });
        }
        let bytes = serde_json::to_vec(&idea).unwrap();
        assert!(
            bytes.len() <= hatchling_core::MAX_SECRET_BYTES,
            "{}",
            bytes.len()
        );
    }
}
