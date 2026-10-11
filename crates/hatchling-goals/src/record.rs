//! The records the store holds: goals and tracking entries.
//!
//! Every text field is bounded and screened for control characters.
//! The bounds keep any one record comfortably inside a session frame
//! even after double JSON encoding (see [`crate::feed::page`]); the
//! control-character screen keeps terminal escapes and other
//! invisible bytes out of text the feed will later show to the user.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::name::{GoalId, SourceLabel};

/// Maximum goal title bytes. A title is one line.
pub const MAX_TITLE_BYTES: usize = 256;
/// Maximum goal detail bytes.
pub const MAX_DETAIL_BYTES: usize = 4096;
/// Maximum tracking entry text bytes.
pub const MAX_TRACKING_TEXT_BYTES: usize = 4096;
/// Maximum goals one user may hold, open and closed together. Bounds
/// the goal listing and the per-user directory.
pub const MAX_GOALS_PER_USER: usize = 256;
/// Maximum entries in one user's tracking log. The log is append-only,
/// so an unbounded log is a disk-fill vector; at the text bound this
/// caps one user's log near 40 MiB.
pub const MAX_TRACKING_ENTRIES: u64 = 10_000;

/// How a goal came to exist.
///
/// v0.1 has exactly one way: the user said so. Inferred goals are a
/// later WO with its own safety review, so they are unrepresentable
/// here rather than merely unused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalOrigin {
    /// The user explicitly asked for this goal to be recorded.
    #[serde(rename = "user-explicit")]
    UserExplicit,
}

/// How a closed goal ended. Named by the user at close: the store never
/// closes a goal on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalOutcome {
    /// The user reached the goal.
    #[serde(rename = "achieved")]
    Achieved,
    /// The user stopped pursuing the goal.
    #[serde(rename = "abandoned")]
    Abandoned,
}

impl std::str::FromStr for GoalOutcome {
    type Err = Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "achieved" => Ok(Self::Achieved),
            "abandoned" => Ok(Self::Abandoned),
            other => Err(Error::InvalidName(format!(
                "goal outcome must be 'achieved' or 'abandoned', got {other:?}"
            ))),
        }
    }
}

/// Where a goal is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalStatus {
    /// The user is working toward it.
    #[serde(rename = "open")]
    Open,
    /// The user closed it. Closed goals are kept, never deleted, and
    /// can no longer be edited.
    #[serde(rename = "closed")]
    Closed {
        /// How it ended.
        outcome: GoalOutcome,
        /// Unix epoch seconds, broker-stamped at close.
        closed_at_secs: u64,
    },
}

/// One goal the user is working toward.
///
/// Timestamps are unix epoch seconds stamped by the broker (host
/// clock), never by the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    /// Store-minted identity.
    pub id: GoalId,
    /// One-line title.
    pub title: String,
    /// Free-form detail; may be empty.
    pub detail: String,
    /// Lifecycle state.
    pub status: GoalStatus,
    /// How the goal came to exist.
    pub origin: GoalOrigin,
    /// Unix epoch seconds, broker-stamped at creation.
    pub created_at_secs: u64,
    /// Unix epoch seconds, broker-stamped at the last change.
    pub updated_at_secs: u64,
}

impl Goal {
    /// True while the goal is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self.status, GoalStatus::Open)
    }
}

/// One immutable tracking entry: what happened, when, and where the
/// report came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackingEntry {
    /// Position in the user's log, from 0. Never reused: the log only
    /// grows.
    pub seq: u64,
    /// Unix epoch seconds, broker-stamped at append.
    pub at_secs: u64,
    /// Where the report came from, as stated at append.
    pub source: SourceLabel,
    /// What happened.
    pub text: String,
}

/// A validated time window over broker-stamped seconds: `since`
/// inclusive, `until` exclusive, either end open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRange {
    since_secs: Option<u64>,
    until_secs: Option<u64>,
}

impl TimeRange {
    /// The unbounded range: every timestamp.
    pub const ALL: Self = Self {
        since_secs: None,
        until_secs: None,
    };

    /// Builds a range, refusing one whose start is after its end.
    /// `since == until` is valid and empty.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRange`] when `since > until`.
    pub const fn new(since_secs: Option<u64>, until_secs: Option<u64>) -> Result<Self, Error> {
        if let (Some(since), Some(until)) = (since_secs, until_secs) {
            if since > until {
                return Err(Error::InvalidRange { since, until });
            }
        }
        Ok(Self {
            since_secs,
            until_secs,
        })
    }

    /// True when `at_secs` falls inside the window.
    #[must_use]
    pub const fn contains(&self, at_secs: u64) -> bool {
        if let Some(since) = self.since_secs {
            if at_secs < since {
                return false;
            }
        }
        if let Some(until) = self.until_secs {
            if at_secs >= until {
                return false;
            }
        }
        true
    }
}

/// Which characters a text field admits beyond printable text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lines {
    /// One line: no control characters at all.
    Single,
    /// Several lines: `\n` and `\t` are allowed, other controls are not.
    Multi,
}

/// Screens one text field: the byte bound, emptiness, and control
/// characters.
fn check_text(
    field: &'static str,
    raw: &str,
    max: usize,
    lines: Lines,
    allow_empty: bool,
) -> Result<(), Error> {
    if raw.len() > max {
        return Err(Error::InvalidText {
            field,
            reason: "is longer than its byte bound",
        });
    }
    if !allow_empty && raw.trim().is_empty() {
        return Err(Error::InvalidText {
            field,
            reason: "must not be empty",
        });
    }
    let admitted = |c: char| lines == Lines::Multi && (c == '\n' || c == '\t');
    if raw.chars().any(|c| c.is_control() && !admitted(c)) {
        return Err(Error::InvalidText {
            field,
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

/// Validates a goal title: one non-empty line of at most
/// [`MAX_TITLE_BYTES`].
///
/// # Errors
///
/// Returns [`Error::InvalidText`].
pub fn check_title(raw: &str) -> Result<(), Error> {
    check_text("title", raw, MAX_TITLE_BYTES, Lines::Single, false)
}

/// Validates a goal detail: at most [`MAX_DETAIL_BYTES`], possibly
/// empty, possibly multi-line.
///
/// # Errors
///
/// Returns [`Error::InvalidText`].
pub fn check_detail(raw: &str) -> Result<(), Error> {
    check_text("detail", raw, MAX_DETAIL_BYTES, Lines::Multi, true)
}

/// Validates tracking text: non-empty, at most
/// [`MAX_TRACKING_TEXT_BYTES`], possibly multi-line.
///
/// # Errors
///
/// Returns [`Error::InvalidText`].
pub fn check_tracking_text(raw: &str) -> Result<(), Error> {
    check_text(
        "tracking text",
        raw,
        MAX_TRACKING_TEXT_BYTES,
        Lines::Multi,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn reason(result: &Result<(), Error>) -> Option<&'static str> {
        match result {
            Err(Error::InvalidText { reason, .. }) => Some(reason),
            Ok(()) | Err(_) => None,
        }
    }

    #[test]
    fn title_bounds_are_exact() {
        assert!(check_title(&"t".repeat(MAX_TITLE_BYTES)).is_ok());
        assert_eq!(
            reason(&check_title(&"t".repeat(MAX_TITLE_BYTES + 1))),
            Some("is longer than its byte bound")
        );
        // The bound is in bytes, not chars.
        assert!(check_title(&"é".repeat(MAX_TITLE_BYTES / 2)).is_ok());
        assert!(check_title(&"é".repeat(MAX_TITLE_BYTES / 2 + 1)).is_err());
    }

    #[test]
    fn title_is_one_nonempty_line() {
        assert_eq!(reason(&check_title("")), Some("must not be empty"));
        // Emptiness is judged after trimming, before the control screen.
        assert_eq!(reason(&check_title("  \t ")), Some("must not be empty"));
        assert_eq!(reason(&check_title("   ")), Some("must not be empty"));
        assert_eq!(reason(&check_title(" x ")), None);
        assert_eq!(
            reason(&check_title("run\na 5k")),
            Some("must not contain control characters")
        );
        assert_eq!(
            reason(&check_title("run\ta 5k")),
            Some("must not contain control characters")
        );
        assert!(check_title("run a 5k").is_ok());
    }

    #[test]
    fn detail_may_be_empty_and_multiline_but_not_escaped() {
        assert!(check_detail("").is_ok());
        assert!(check_detail("week 1: 2k\n\tweek 2: 3k").is_ok());
        assert_eq!(
            reason(&check_detail("red \u{1b}[31m text")),
            Some("must not contain control characters")
        );
        assert_eq!(
            reason(&check_detail("carriage\rreturn")),
            Some("must not contain control characters")
        );
        assert!(check_detail(&"d".repeat(MAX_DETAIL_BYTES)).is_ok());
        assert!(check_detail(&"d".repeat(MAX_DETAIL_BYTES + 1)).is_err());
    }

    #[test]
    fn tracking_text_is_nonempty_and_bounded() {
        assert_eq!(reason(&check_tracking_text("")), Some("must not be empty"));
        assert_eq!(
            reason(&check_tracking_text("\n\n")),
            Some("must not be empty")
        );
        assert!(check_tracking_text("ran 3k\nfelt fine").is_ok());
        assert!(check_tracking_text(&"x".repeat(MAX_TRACKING_TEXT_BYTES)).is_ok());
        assert!(check_tracking_text(&"x".repeat(MAX_TRACKING_TEXT_BYTES + 1)).is_err());
        assert!(check_tracking_text("nul\0byte").is_err());
    }

    #[test]
    fn time_range_is_since_inclusive_until_exclusive() {
        let range = TimeRange::new(Some(10), Some(20)).unwrap();
        assert!(!range.contains(9));
        assert!(range.contains(10));
        assert!(range.contains(19));
        assert!(!range.contains(20));
        let open_start = TimeRange::new(None, Some(20)).unwrap();
        assert!(open_start.contains(0));
        assert!(!open_start.contains(20));
        let open_end = TimeRange::new(Some(10), None).unwrap();
        assert!(!open_end.contains(9));
        assert!(open_end.contains(u64::MAX));
        assert!(TimeRange::ALL.contains(0));
        assert!(TimeRange::ALL.contains(u64::MAX));
    }

    #[test]
    fn time_range_refuses_an_inverted_window() {
        assert!(matches!(
            TimeRange::new(Some(21), Some(20)),
            Err(Error::InvalidRange {
                since: 21,
                until: 20
            })
        ));
        let empty = TimeRange::new(Some(20), Some(20)).unwrap();
        assert!(!empty.contains(20));
        assert!(!empty.contains(19));
    }

    #[test]
    fn outcomes_parse_exactly() {
        assert_eq!(
            "achieved".parse::<GoalOutcome>().unwrap(),
            GoalOutcome::Achieved
        );
        assert_eq!(
            "abandoned".parse::<GoalOutcome>().unwrap(),
            GoalOutcome::Abandoned
        );
        assert!("done".parse::<GoalOutcome>().is_err());
        assert!("Achieved".parse::<GoalOutcome>().is_err());
    }

    #[test]
    fn is_open_tracks_the_status() {
        let mut goal = Goal {
            id: "g1".parse().unwrap(),
            title: String::from("t"),
            detail: String::new(),
            status: GoalStatus::Open,
            origin: GoalOrigin::UserExplicit,
            created_at_secs: 1,
            updated_at_secs: 1,
        };
        assert!(goal.is_open());
        goal.status = GoalStatus::Closed {
            outcome: GoalOutcome::Achieved,
            closed_at_secs: 2,
        };
        assert!(!goal.is_open());
    }

    #[test]
    fn goal_wire_form_is_stable() {
        let goal = Goal {
            id: "g1".parse().unwrap(),
            title: String::from("run a 5k"),
            detail: String::new(),
            status: GoalStatus::Closed {
                outcome: GoalOutcome::Abandoned,
                closed_at_secs: 9,
            },
            origin: GoalOrigin::UserExplicit,
            created_at_secs: 1,
            updated_at_secs: 9,
        };
        let json = serde_json::to_string(&goal).unwrap();
        assert_eq!(
            json,
            r#"{"id":"g1","title":"run a 5k","detail":"","status":{"closed":{"outcome":"abandoned","closed_at_secs":9}},"origin":"user-explicit","created_at_secs":1,"updated_at_secs":9}"#
        );
        assert_eq!(serde_json::from_str::<Goal>(&json).unwrap(), goal);
    }
}
