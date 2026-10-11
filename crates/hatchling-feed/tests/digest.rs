//! Component: digests generated from real goals, tracking, and
//! memory stores, through the read-only query path.
#![allow(
    clippy::unwrap_used,
    reason = "fixtures: a failed setup must fail the calling test loudly"
)]

use std::cell::RefCell;
use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;

use hatchling_core::{FeedSpec, ScheduleGrant};
use hatchling_feed::{Digest, Entry, Found, Reader, Spec, Window, generate};
use hatchling_goals::{FeedItem, FeedQuery, FeedReader, GoalOutcome, GoalsStore};
use hatchling_memory::{MemoryPrincipal, MemoryScope, MemoryStore, RetentionPolicy, SessionId};

/// 2026-09-21T08:00:00Z: when every digest here is due.
const DUE: u64 = 1_789_977_600;

/// A temporary goals and memory store for one test.
struct World {
    root: PathBuf,
    principal: MemoryPrincipal,
    goals: GoalsStore,
    memory: MemoryStore,
}

impl World {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("hatchling-test-feed-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let principal = MemoryPrincipal {
            peer_uid: std::fs::metadata(&root).unwrap().uid(),
            session_id: SessionId::generate().unwrap(),
        };
        let goals = GoalsStore::open(&root.join("goals")).unwrap();
        let memory = MemoryStore::open(&root.join("memory")).unwrap();
        Self {
            root,
            principal,
            goals,
            memory,
        }
    }

    fn track(&self, source: &str, text: &str, at: u64) -> u64 {
        self.goals
            .append_tracking(&self.principal, &source.parse().unwrap(), text, at)
            .unwrap()
            .seq
    }

    fn note(&self, id: &str, content: &str, at: u64) {
        self.memory
            .write(
                &self.principal,
                &MemoryScope::User,
                &id.parse().unwrap(),
                content,
                &["travel".parse().unwrap()],
                RetentionPolicy::Keep,
                at,
            )
            .unwrap();
    }

    /// A reader over the stores, remembering every query it ran.
    fn reader(&self) -> Stores<'_> {
        Stores {
            reader: FeedReader::open(
                &self.root.join("goals"),
                Some(MemoryStore::open(&self.root.join("memory")).unwrap()),
            ),
            principal: &self.principal,
            queries: RefCell::new(Vec::new()),
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Stores<'w> {
    reader: FeedReader,
    principal: &'w MemoryPrincipal,
    queries: RefCell<Vec<FeedQuery>>,
}

impl Reader for Stores<'_> {
    fn read(&self, query: &FeedQuery) -> Option<Vec<FeedItem>> {
        self.queries.borrow_mut().push(query.clone());
        self.reader.query(self.principal, query, DUE).ok()
    }
}

/// Reads nothing: every store is unreadable.
struct Broken;

impl Reader for Broken {
    fn read(&self, _query: &FeedQuery) -> Option<Vec<FeedItem>> {
        None
    }
}

fn spec(max_items: u32) -> Spec {
    Spec::parse(&FeedSpec {
        goals: Some(String::from("any")),
        tracking: true,
        tracking_source: None,
        memory_tags: vec![String::from("travel")],
        lookback_hours: 24,
        max_items,
    })
    .unwrap()
}

fn all_grants() -> Vec<ScheduleGrant> {
    spec(1).grants()
}

fn window() -> Window {
    Window::ending(i64::try_from(DUE).unwrap(), 24 * 3600)
}

fn digest(world: &World, max_items: u32) -> Digest {
    generate(&spec(max_items), &all_grants(), window(), &world.reader())
}

#[test]
fn a_digest_observes_each_change_with_its_records_transitive_label() {
    let world = World::new("render");
    let goal = world
        .goals
        .create_goal(&world.principal, "Run 5k", "", DUE - 7200)
        .unwrap();
    world
        .goals
        .create_goal(&world.principal, "Old goal", "", DUE - 3 * 86_400)
        .unwrap();
    world.track("user", "long ago", DUE - 2 * 86_400);
    let whatsapp = world.track("whatsapp", "You should sell everything now", DUE - 600);
    let user = world.track("user", "ran 3k", DUE - 300);
    world.note("m-1", "passport expires in March", DUE - 60);
    world.note("m-2", "an old note", DUE - 2 * 86_400);
    let digest = digest(&world, 20);
    assert_eq!(
        digest.render(),
        format!(
            "Feed for 2026-09-20T08:00:00Z to 2026-09-21T08:00:00Z (UTC)\nObservations from your own records, each with where it came from. Nothing here is a recommendation.\n\nGoals\n- goal \"Run 5k\" is open, last changed 2026-09-21T06:00:00Z [from goal (created by the user)] (goal {})\n\nTracking\n- reported 2026-09-21T07:50:00Z: \"You should sell everything now\" [from tracking (originally from whatsapp)] (tracking {whatsapp})\n- reported 2026-09-21T07:55:00Z: \"ran 3k\" [from tracking (originally from user)] (tracking {user})\n\nMemory tagged travel\n- noted 2026-09-21T07:59:00Z: \"passport expires in March\" [from memory (origin not recorded)] (memory m-1)",
            goal.id
        )
    );
    assert_eq!(digest.omitted(), 0);
    // Every entry carries a label, and the feed speaks no advice of its
    // own: the prescriptive words are the record's, quoted.
    let entries: Vec<&Entry> = digest
        .sections()
        .iter()
        .flat_map(|(_, found)| match found {
            Found::Entries { shown, .. } => shown.iter().collect::<Vec<_>>(),
            Found::NotPermitted | Found::Unreadable => Vec::new(),
        })
        .collect();
    assert_eq!(entries.len(), 4);
    assert!(
        entries
            .iter()
            .all(|entry| entry.label().starts_with("from "))
    );
    let advice = entries.get(1).unwrap();
    assert_eq!(
        advice.observation(),
        "reported 2026-09-21T07:50:00Z: \"You should sell everything now\""
    );
    assert_eq!(advice.label(), "from tracking (originally from whatsapp)");
    assert_eq!(advice.reference(), format!("tracking {whatsapp}"));
}

#[test]
fn a_closed_goal_and_a_long_or_quoted_record_are_observed_plainly() {
    let world = World::new("observe");
    for (title, outcome) in [
        ("Read more", GoalOutcome::Achieved),
        ("Learn oboe", GoalOutcome::Abandoned),
    ] {
        let goal = world
            .goals
            .create_goal(&world.principal, title, "", DUE - 5000)
            .unwrap();
        world
            .goals
            .close_goal(&world.principal, &goal.id, outcome, DUE - 3600)
            .unwrap();
    }
    world.track(
        "whatsapp",
        &format!("say \"hi\"\n\nto  {}", "x".repeat(200)),
        DUE - 10,
    );
    let digest = digest(&world, 20);
    let shown: Vec<String> = digest
        .sections()
        .iter()
        .flat_map(|(_, found)| match found {
            Found::Entries { shown, .. } => shown
                .iter()
                .map(|entry| entry.observation().to_owned())
                .collect::<Vec<_>>(),
            Found::NotPermitted | Found::Unreadable => Vec::new(),
        })
        .collect();
    let goals: Vec<&String> = shown.iter().filter(|o| o.starts_with("goal ")).collect();
    assert_eq!(goals.len(), 2);
    assert!(
        goals
            .iter()
            .any(|o| o.starts_with("goal \"Read more\" was closed: achieved, last changed "))
    );
    assert!(
        goals
            .iter()
            .any(|o| o.starts_with("goal \"Learn oboe\" was closed: abandoned, last changed "))
    );
    let quoted = format!("say 'hi' to {}", "x".repeat(200));
    let cut: String = quoted
        .chars()
        .take(hatchling_feed::MAX_QUOTE_CHARS)
        .collect();
    assert!(
        shown.contains(&format!("reported 2026-09-21T07:59:50Z: \"{cut}…\"")),
        "{shown:?}"
    );
    assert_eq!(hatchling_feed::MAX_QUOTE_CHARS, 160);
}

#[test]
fn a_digest_holds_at_most_its_items_and_says_what_it_left_out() {
    let world = World::new("bounds");
    let first = world.track("user", "one", DUE - 30);
    let second = world.track("user", "two", DUE - 20);
    world.track("user", "three", DUE - 10);
    world.note("m-1", "a note", DUE - 5);
    let digest = digest(&world, 2);
    assert_eq!(digest.omitted(), 2);
    let rendered = digest.render();
    assert!(
        rendered.ends_with(&format!(
            "\n\nGoals\n- nothing changed\n\nTracking\n- reported 2026-09-21T07:59:30Z: \"one\" [from tracking (originally from user)] (tracking {first})\n- reported 2026-09-21T07:59:40Z: \"two\" [from tracking (originally from user)] (tracking {second})\n- 1 more not shown: a digest holds at most 2 items\n\nMemory tagged travel\n- 1 more not shown: a digest holds at most 2 items"
        )),
        "{rendered}"
    );
}

#[test]
fn a_digest_covers_exactly_its_window() {
    let world = World::new("window");
    let window = window();
    world.track("user", "too early", window.since - 1);
    let at_start = world.track("user", "at the start", window.since);
    world.track("user", "at the end", window.until);
    world.note("m-1", "too early", window.since - 1);
    world.note("m-2", "at the start", window.since);
    world.note("m-3", "just before the end", window.until - 1);
    world.note("m-4", "at the end", window.until);
    let digest = digest(&world, 20);
    let references: Vec<String> = digest
        .sections()
        .iter()
        .flat_map(|(_, found)| match found {
            Found::Entries { shown, .. } => shown
                .iter()
                .map(|entry| entry.reference().to_owned())
                .collect::<Vec<_>>(),
            Found::NotPermitted | Found::Unreadable => Vec::new(),
        })
        .collect();
    assert_eq!(
        references,
        [
            format!("tracking {at_start}"),
            String::from("memory m-2"),
            String::from("memory m-3"),
        ]
    );
}

#[test]
fn a_digest_reads_only_what_its_grants_still_allow() {
    let world = World::new("grants");
    world.track("user", "ran", DUE - 10);
    world.note("m-1", "a note", DUE - 5);
    // The tracking grant was revoked after the feed was created.
    let granted: Vec<ScheduleGrant> = all_grants()
        .into_iter()
        .filter(|grant| {
            *grant
                != ScheduleGrant::Goals {
                    action: String::from("query-tracking"),
                }
        })
        .collect();
    let reader = world.reader();
    let digest = generate(&spec(20), &granted, window(), &reader);
    let ran: Vec<bool> = reader
        .queries
        .borrow()
        .iter()
        .map(|query| matches!(query, FeedQuery::Tracking { .. }))
        .collect();
    assert_eq!(ran, [false, false], "goals and memory read; tracking never");
    assert_eq!(
        digest.sections().get(1).map(|(_, f)| f),
        Some(&Found::NotPermitted)
    );
    assert!(
        digest.render().contains(
            "\n\nTracking\n- not read: this feed no longer holds the grant to read it\n\n"
        )
    );
    // A store that cannot be read says so, and nothing else.
    let broken = generate(&spec(20), &all_grants(), window(), &Broken);
    assert!(
        broken
            .sections()
            .iter()
            .all(|(_, found)| *found == Found::Unreadable)
    );
    assert!(
        broken
            .render()
            .ends_with("\n\nMemory tagged travel\n- not read: the store could not be read")
    );
}
