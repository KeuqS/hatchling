//! A feed's spec: the user's own tuning, as rules.
//!
//! Each section is one read-only query over one store. The read grant a
//! section needs is derived from it ([`Section::grant`]), so a feed's
//! grants come from what it reads and never from anyone's say-so: there
//! is no way to spell a write in a spec.

use hatchling_core::{FeedSpec, ScheduleGrant};
use hatchling_goals::{FeedQuery, SourceLabel, StatusFilter, TimeRange};
use hatchling_memory::Tag;

use crate::Window;

/// The longest look back, in hours: a week.
pub const MAX_LOOKBACK_HOURS: u32 = 168;
/// The most items one digest holds.
pub const MAX_ITEMS: u32 = 50;
/// The most memory tags one spec reads.
pub const MAX_TAGS: usize = 8;

/// One section of a digest: one read-only query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    /// Goals changed in the window, with this status.
    Goals(StatusFilter),
    /// Tracking entries recorded in the window, from this source if one
    /// is named.
    Tracking(Option<SourceLabel>),
    /// Memory items carrying this tag, changed in the window.
    Memory(Tag),
}

impl Section {
    /// The read grant this section needs.
    #[must_use]
    pub fn grant(&self) -> ScheduleGrant {
        match self {
            Self::Goals(_) => ScheduleGrant::Goals {
                action: String::from("query-goals"),
            },
            Self::Tracking(_) => ScheduleGrant::Goals {
                action: String::from("query-tracking"),
            },
            Self::Memory(_) => ScheduleGrant::Memory {
                scope: String::from("user"),
                action: String::from("recall"),
            },
        }
    }

    /// The query this section runs for `window`.
    #[must_use]
    pub fn query(&self, window: Window) -> FeedQuery {
        match self {
            Self::Goals(status) => FeedQuery::Goals {
                status: *status,
                updated_since_secs: Some(window.since),
                after: None,
            },
            Self::Tracking(source) => FeedQuery::Tracking {
                range: TimeRange::new(Some(window.since), Some(window.until))
                    .unwrap_or(TimeRange::ALL),
                source: source.clone(),
                after_seq: None,
            },
            Self::Memory(tag) => FeedQuery::Memory {
                tag: tag.clone(),
                after: None,
            },
        }
    }

    /// The heading the digest shows.
    #[must_use]
    pub fn heading(&self) -> String {
        match self {
            Self::Goals(StatusFilter::Open) => String::from("Open goals"),
            Self::Goals(StatusFilter::Closed) => String::from("Closed goals"),
            Self::Goals(StatusFilter::Any) => String::from("Goals"),
            Self::Tracking(None) => String::from("Tracking"),
            Self::Tracking(Some(source)) => format!("Tracking from {source}"),
            Self::Memory(tag) => format!("Memory tagged {}", tag.as_str()),
        }
    }
}

/// A validated feed spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    sections: Vec<Section>,
    lookback_secs: u64,
    max_items: usize,
}

impl Spec {
    /// Validates `wire`, or `None` if it is unusable: no section on, an
    /// unknown goal status, a source without tracking, a bad source label
    /// or tag, a repeated tag, more than [`MAX_TAGS`] tags, or a bound out
    /// of range.
    #[must_use]
    pub fn parse(wire: &FeedSpec) -> Option<Self> {
        if !(1..=MAX_LOOKBACK_HOURS).contains(&wire.lookback_hours)
            || !(1..=MAX_ITEMS).contains(&wire.max_items)
            || wire.memory_tags.len() > MAX_TAGS
            || (wire.tracking_source.is_some() && !wire.tracking)
        {
            return None;
        }
        let mut sections = Vec::new();
        if let Some(status) = &wire.goals {
            sections.push(Section::Goals(status.parse().ok()?));
        }
        if wire.tracking {
            let source = match &wire.tracking_source {
                Some(source) => Some(source.parse().ok()?),
                None => None,
            };
            sections.push(Section::Tracking(source));
        }
        for tag in &wire.memory_tags {
            let section = Section::Memory(tag.parse().ok()?);
            if sections.contains(&section) {
                return None;
            }
            sections.push(section);
        }
        if sections.is_empty() {
            return None;
        }
        Some(Self {
            sections,
            lookback_secs: u64::from(wire.lookback_hours).saturating_mul(3600),
            max_items: usize::try_from(wire.max_items).ok()?,
        })
    }

    /// The sections, in digest order: goals, tracking, then each tag.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// How far back each digest looks, in seconds.
    #[must_use]
    pub const fn lookback_secs(&self) -> u64 {
        self.lookback_secs
    }

    /// The most items one digest holds.
    #[must_use]
    pub const fn max_items(&self) -> usize {
        self.max_items
    }

    /// The read grants the feed needs, one per kind of section, sorted.
    #[must_use]
    pub fn grants(&self) -> Vec<ScheduleGrant> {
        let mut grants: Vec<ScheduleGrant> = self.sections.iter().map(Section::grant).collect();
        grants.sort();
        grants.dedup();
        grants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire() -> FeedSpec {
        FeedSpec {
            goals: Some(String::from("open")),
            tracking: true,
            tracking_source: Some(String::from("whatsapp")),
            memory_tags: vec![String::from("travel"), String::from("work")],
            lookback_hours: 24,
            max_items: 20,
        }
    }

    fn goals_grant(action: &str) -> ScheduleGrant {
        ScheduleGrant::Goals {
            action: action.to_owned(),
        }
    }

    fn recall() -> ScheduleGrant {
        ScheduleGrant::Memory {
            scope: String::from("user"),
            action: String::from("recall"),
        }
    }

    #[test]
    fn a_spec_reads_its_sections_in_order_with_one_read_grant_per_kind() {
        let spec = Spec::parse(&wire()).unwrap();
        assert_eq!(
            spec.sections(),
            [
                Section::Goals(StatusFilter::Open),
                Section::Tracking(Some("whatsapp".parse().unwrap())),
                Section::Memory("travel".parse().unwrap()),
                Section::Memory("work".parse().unwrap()),
            ]
        );
        assert_eq!(spec.lookback_secs(), 24 * 3600);
        assert_eq!(spec.max_items(), 20);
        assert_eq!(
            spec.grants(),
            [
                recall(),
                goals_grant("query-goals"),
                goals_grant("query-tracking")
            ]
        );
        let tracking_only = Spec::parse(&FeedSpec {
            goals: None,
            tracking: true,
            tracking_source: None,
            memory_tags: Vec::new(),
            lookback_hours: 1,
            max_items: 1,
        })
        .unwrap();
        assert_eq!(tracking_only.sections(), [Section::Tracking(None)]);
        assert_eq!(tracking_only.grants(), [goals_grant("query-tracking")]);
        assert_eq!((MAX_LOOKBACK_HOURS, MAX_ITEMS, MAX_TAGS), (168, 50, 8));
    }

    #[test]
    fn an_unusable_spec_is_refused_at_each_bound() {
        let with = |change: &dyn Fn(&mut FeedSpec)| {
            let mut spec = wire();
            change(&mut spec);
            Spec::parse(&spec).is_some()
        };
        assert!(with(&|_| ()));
        assert!(with(&|s| s.lookback_hours = 1));
        assert!(with(&|s| s.lookback_hours = MAX_LOOKBACK_HOURS));
        assert!(!with(&|s| s.lookback_hours = MAX_LOOKBACK_HOURS + 1));
        assert!(!with(&|s| s.lookback_hours = 0));
        assert!(with(&|s| s.max_items = 1));
        assert!(with(&|s| s.max_items = MAX_ITEMS));
        assert!(!with(&|s| s.max_items = 0));
        assert!(!with(&|s| s.max_items = MAX_ITEMS + 1));
        let tags = |n: usize| (0..n).map(|i| format!("t{i}")).collect::<Vec<_>>();
        assert!(with(&|s| s.memory_tags = tags(MAX_TAGS)));
        assert!(!with(&|s| s.memory_tags = tags(MAX_TAGS + 1)));
        assert!(!with(
            &|s| s.memory_tags = vec![String::from("a"), String::from("a")]
        ));
        assert!(!with(&|s| s.memory_tags = vec![String::from("Not A Tag!")]));
        assert!(!with(&|s| s.goals = Some(String::from("someday"))));
        assert!(with(&|s| s.goals = Some(String::from("any"))));
        assert!(!with(
            &|s| s.tracking_source = Some(String::from("bad label!"))
        ));
        // A source without tracking is a mistake, not a silent no-op.
        assert!(!with(&|s| s.tracking = false));
        assert!(with(&|s| {
            s.tracking = false;
            s.tracking_source = None;
        }));
        // No section on at all.
        assert!(!with(&|s| {
            s.goals = None;
            s.tracking = false;
            s.tracking_source = None;
            s.memory_tags.clear();
        }));
    }

    #[test]
    fn each_section_runs_its_one_query_over_the_window_and_says_what_it_is() {
        let window = Window {
            since: 100,
            until: 200,
        };
        assert_eq!(
            Section::Goals(StatusFilter::Any).query(window),
            FeedQuery::Goals {
                status: StatusFilter::Any,
                updated_since_secs: Some(100),
                after: None,
            }
        );
        let source: SourceLabel = "user".parse().unwrap();
        assert_eq!(
            Section::Tracking(Some(source.clone())).query(window),
            FeedQuery::Tracking {
                range: TimeRange::new(Some(100), Some(200)).unwrap(),
                source: Some(source.clone()),
                after_seq: None,
            }
        );
        let tag: Tag = "travel".parse().unwrap();
        assert_eq!(
            Section::Memory(tag.clone()).query(window),
            FeedQuery::Memory {
                tag: tag.clone(),
                after: None,
            }
        );
        let headings: Vec<String> = [
            Section::Goals(StatusFilter::Open),
            Section::Goals(StatusFilter::Closed),
            Section::Goals(StatusFilter::Any),
            Section::Tracking(None),
            Section::Tracking(Some(source)),
            Section::Memory(tag),
        ]
        .iter()
        .map(Section::heading)
        .collect();
        assert_eq!(
            headings,
            [
                "Open goals",
                "Closed goals",
                "Goals",
                "Tracking",
                "Tracking from user",
                "Memory tagged travel"
            ]
        );
    }

    #[test]
    fn a_window_ends_at_the_fire_and_never_before_the_epoch() {
        assert_eq!(
            Window::ending(1000, 300),
            Window {
                since: 700,
                until: 1000
            }
        );
        assert_eq!(
            Window::ending(100, 300),
            Window {
                since: 0,
                until: 100
            }
        );
        assert_eq!(Window::ending(-5, 300), Window { since: 0, until: 0 });
    }
}
