//! A digest: what changed in the user's own records, each item an
//! observation with where it came from.
//!
//! - **Every entry has its label.** An [`Entry`] is made only from a
//!   [`FeedItem`] ([`Entry::of`]); its label is the item's provenance,
//!   computed from the record at the store layer, transitive where the
//!   record allows (`from tracking (originally from whatsapp)`). There is
//!   no other way to make one, so there is no unlabeled entry.
//! - **Observations, never prescriptions.** An entry says what a record
//!   says and when: the record's own text is quoted, never restated in
//!   the feed's voice. The digest has no field for advice; a suggestion
//!   is an idea, with its own lifecycle and confirmation.
//! - **The feed reads; it never acts.** [`generate`] reaches the stores
//!   only through a [`Reader`], whose one method runs a read-only
//!   [`FeedQuery`], and only for sections whose read grant the run holds.

use std::fmt::Write as _;

use hatchling_core::ScheduleGrant;
use hatchling_goals::{FeedBody, FeedItem, FeedQuery, GoalOutcome, GoalStatus};

use crate::Window;
use crate::spec::Spec;

/// The longest quote of a record's text, in characters; a longer one is
/// cut and marked.
pub const MAX_QUOTE_CHARS: usize = 160;

/// The read-only path a digest is generated through.
pub trait Reader {
    /// Every item `query` finds, or `None` if a store cannot be read.
    fn read(&self, query: &FeedQuery) -> Option<Vec<FeedItem>>;
}

/// One digest entry: an observation, and the provenance label of the
/// record it observes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    observation: String,
    label: String,
    reference: String,
}

impl Entry {
    /// The entry for `item`.
    #[must_use]
    pub fn of(item: &FeedItem) -> Self {
        let (observation, reference) = match item.body() {
            FeedBody::Goal(goal) => {
                let state = match goal.status {
                    GoalStatus::Open => String::from("is open"),
                    GoalStatus::Closed {
                        outcome: GoalOutcome::Achieved,
                        ..
                    } => String::from("was closed: achieved"),
                    GoalStatus::Closed {
                        outcome: GoalOutcome::Abandoned,
                        ..
                    } => String::from("was closed: abandoned"),
                };
                (
                    format!(
                        "goal {} {state}, last changed {}",
                        quote(&goal.title),
                        timestamp(goal.updated_at_secs)
                    ),
                    format!("goal {}", goal.id),
                )
            }
            FeedBody::Tracking(entry) => (
                format!(
                    "reported {}: {}",
                    timestamp(entry.at_secs),
                    quote(&entry.text)
                ),
                format!("tracking {}", entry.seq),
            ),
            FeedBody::Memory(memory) => (
                format!(
                    "noted {}: {}",
                    timestamp(memory.updated_at_secs),
                    quote(&memory.content)
                ),
                format!("memory {}", memory.id.as_str()),
            ),
        };
        Self {
            observation,
            label: item.provenance().label(),
            reference,
        }
    }

    /// What the record says, and when.
    #[must_use]
    pub fn observation(&self) -> &str {
        &self.observation
    }

    /// Where it came from: the record's transitive provenance label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The record's store and id, for capturing an idea from it.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

/// What one section of a digest found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// What changed, in the store's order.
    Entries {
        /// The entries shown.
        shown: Vec<Entry>,
        /// How many more changed than the digest had room for.
        left_out: usize,
    },
    /// The run does not hold this section's read grant now: it was
    /// revoked after the feed was created. Nothing was read.
    NotPermitted,
    /// The store could not be read.
    Unreadable,
}

/// A digest: the window, and what each section found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    window: Window,
    sections: Vec<(String, Found)>,
    max_items: usize,
}

/// Whether `item` changed inside `window`.
const fn in_window(item: &FeedItem, window: Window) -> bool {
    let at = match item.body() {
        FeedBody::Goal(goal) => goal.updated_at_secs,
        FeedBody::Tracking(entry) => entry.at_secs,
        FeedBody::Memory(memory) => memory.updated_at_secs,
    };
    window.since <= at && at < window.until
}

/// Generates the digest of `spec` for `window`, through `reader`, reading
/// only the sections whose grant is in `granted`.
#[must_use]
pub fn generate(
    spec: &Spec,
    granted: &[ScheduleGrant],
    window: Window,
    reader: &dyn Reader,
) -> Digest {
    let mut left = spec.max_items();
    let mut sections = Vec::with_capacity(spec.sections().len());
    for section in spec.sections() {
        let found = if !granted.contains(&section.grant()) {
            Found::NotPermitted
        } else if let Some(items) = reader.read(&section.query(window)) {
            let changed: Vec<&FeedItem> = items
                .iter()
                .filter(|item| in_window(item, window))
                .collect();
            let shown: Vec<Entry> = changed
                .iter()
                .take(left)
                .map(|item| Entry::of(item))
                .collect();
            left = left.saturating_sub(shown.len());
            Found::Entries {
                left_out: changed.len().saturating_sub(shown.len()),
                shown,
            }
        } else {
            Found::Unreadable
        };
        sections.push((section.heading(), found));
    }
    Digest {
        window,
        sections,
        max_items: spec.max_items(),
    }
}

impl Digest {
    /// The sections, in order: each heading and what it found.
    #[must_use]
    pub fn sections(&self) -> &[(String, Found)] {
        &self.sections
    }

    /// How many changed items the item bound left out, in all.
    #[must_use]
    pub fn omitted(&self) -> usize {
        self.sections
            .iter()
            .map(|(_, found)| match found {
                Found::Entries { left_out, .. } => *left_out,
                Found::NotPermitted | Found::Unreadable => 0,
            })
            .sum()
    }

    /// The digest as the text a channel delivers.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!(
            "Feed for {} to {} (UTC)\nObservations from your own records, each with where it came from. Nothing here is a recommendation.",
            timestamp(self.window.since),
            timestamp(self.window.until)
        );
        for (heading, found) in &self.sections {
            out.push_str("\n\n");
            out.push_str(heading);
            match found {
                Found::Entries { shown, left_out } => {
                    if shown.is_empty() && *left_out == 0 {
                        out.push_str("\n- nothing changed");
                    }
                    for entry in shown {
                        out.push_str("\n- ");
                        out.push_str(&entry.observation);
                        out.push_str(" [");
                        out.push_str(&entry.label);
                        out.push_str("] (");
                        out.push_str(&entry.reference);
                        out.push(')');
                    }
                    if *left_out > 0 {
                        let _ = write!(
                            out,
                            "\n- {left_out} more not shown: a digest holds at most {} items",
                            self.max_items
                        );
                    }
                }
                Found::NotPermitted => {
                    out.push_str("\n- not read: this feed no longer holds the grant to read it");
                }
                Found::Unreadable => out.push_str("\n- not read: the store could not be read"),
            }
        }
        out
    }
}

/// `text` quoted on one line, double quotes inside it made single, cut to
/// [`MAX_QUOTE_CHARS`] and marked when longer.
fn quote(text: &str) -> String {
    let line = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "'");
    let mut chars = line.chars();
    let cut: String = chars.by_ref().take(MAX_QUOTE_CHARS).collect();
    if chars.next().is_some() {
        format!("\"{cut}…\"")
    } else {
        format!("\"{cut}\"")
    }
}

/// `secs` as a UTC timestamp.
fn timestamp(secs: u64) -> String {
    i64::try_from(secs)
        .ok()
        .and_then(|secs| jiff::Timestamp::from_second(secs).ok())
        .map_or_else(|| format!("@{secs}"), |at| at.to_string())
}
