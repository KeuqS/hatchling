//! Gathering: the approved sources only, each at most once, within the
//! run's bounds.
//!
//! The gather phase is the agent loop with two tools over one shared
//! [`Gather`]:
//!
//! - **`fetch_source`** takes a source *id* from the approved plan, never
//!   a URL, so the phase cannot reach anything the user did not approve.
//!   Each source is fetched at most once: nothing is re-fetched to
//!   freshen it. The model learns only how the fetch went (status, size,
//!   hash), **never the content**: what a source says cannot steer which
//!   sources are read. Content is shown only at synthesis, as evidence.
//! - **`amend_plan`** asks to add a source. The user decides
//!   ([`Approver::review_amendment`]), and every amendment, approved or
//!   declined, is in the report.
//!
//! Both stop at the bounds: past the source bound or the deadline,
//! nothing more is fetched.

use std::sync::{Mutex, MutexGuard, PoisonError};

use hatchling_loop::{Error as LoopError, Tool, ToolOutput};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::bounds::{Bounds, MAX_SOURCES};
use crate::plan::{Origin, Plan, field};
use crate::url::SourceUrl;

/// The most bytes of one source's text the synthesis phase is shown.
/// Its hash and size are always of the whole body.
pub const MAX_EVIDENCE_BYTES: usize = 12 * 1024;

/// A fetched body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The HTTP status.
    pub status: u16,
    /// The body.
    pub body: Vec<u8>,
}

/// Why a fetch got no response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchFailure {
    /// Egress refused the destination: not on the allowlist, or not
    /// granted.
    Denied,
    /// The fetch failed on the way.
    Failed,
}

/// Fetches a source. In the agent, through the egress proxy.
pub trait Fetcher: Send + Sync {
    /// Fetches `url`.
    ///
    /// # Errors
    ///
    /// Returns why there is no response.
    fn fetch(&self, url: &SourceUrl) -> Result<Fetched, FetchFailure>;
}

/// The time, in Unix seconds.
pub trait Clock: Send + Sync {
    /// Now.
    fn now(&self) -> i64;
}

/// The system clock.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
            })
    }
}

/// A user's decision on a plan or an amendment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Go ahead.
    Approve,
    /// Don't.
    Decline {
        /// Why, in the user's words.
        reason: String,
    },
}

/// The user, as the run asks them: the plan before anything is
/// fetched, and each amendment as it is asked for.
pub trait Approver: Send + Sync {
    /// Decides on the proposed plan.
    fn review_plan(&self, plan: &Plan) -> Decision;

    /// Decides on adding `url` to `plan`, for the reason `why`.
    fn review_amendment(&self, plan: &Plan, url: &SourceUrl, why: &str) -> Decision;
}

/// What became of one fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetch {
    /// Read, with a 2xx status: evidence.
    Read {
        /// SHA-256 of the whole body, hex.
        sha256: String,
        /// The whole body's size.
        bytes: usize,
        /// Its text as synthesis sees it: at most [`MAX_EVIDENCE_BYTES`].
        text: String,
        /// Whether the text was cut at the bound.
        truncated: bool,
    },
    /// Answered with a status outside 2xx: not evidence.
    Status {
        /// The status.
        status: u16,
    },
    /// Refused by egress.
    Denied,
    /// Failed on the way.
    Failed,
}

/// One fetched source, as the run records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    /// The source's plan id.
    pub id: String,
    /// Where it was fetched from.
    pub url: SourceUrl,
    /// When, in Unix seconds.
    pub fetched_at: i64,
    /// What came of it.
    pub fetch: Fetch,
}

/// An amendment the model asked for, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amendment {
    /// The source asked for.
    pub url: String,
    /// Why, as the model gave it.
    pub why: String,
    /// The user's decision: the id it was added as, or why not.
    pub outcome: Result<String, String>,
}

/// Why gathering ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The model was done.
    Finished,
    /// The source bound was reached.
    SourceBound,
    /// The time bound was reached.
    TimeBound,
    /// The phase ran out of model turns.
    StepBound,
}

/// What the gather phase gathered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gathered {
    /// The plan, with any approved amendments.
    pub plan: Plan,
    /// Every fetch, in order.
    pub records: Vec<SourceRecord>,
    /// Every amendment asked for, in order.
    pub amendments: Vec<Amendment>,
    /// Why gathering ended.
    pub stop: StopReason,
}

/// The gather phase's state, shared by its tools.
pub struct Gather<'a> {
    plan: Plan,
    bounds: Bounds,
    deadline: i64,
    fetcher: &'a dyn Fetcher,
    approver: &'a dyn Approver,
    clock: &'a dyn Clock,
    records: Vec<SourceRecord>,
    amendments: Vec<Amendment>,
    out_of_time: bool,
}

impl std::fmt::Debug for Gather<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gather")
            .field("plan", &self.plan)
            .field("records", &self.records)
            .finish_non_exhaustive()
    }
}

impl<'a> Gather<'a> {
    /// Gathering for the approved `plan`, until `deadline`.
    #[must_use]
    pub const fn new(
        plan: Plan,
        bounds: Bounds,
        deadline: i64,
        fetcher: &'a dyn Fetcher,
        approver: &'a dyn Approver,
        clock: &'a dyn Clock,
    ) -> Self {
        Self {
            plan,
            bounds,
            deadline,
            fetcher,
            approver,
            clock,
            records: Vec::new(),
            amendments: Vec::new(),
            out_of_time: false,
        }
    }

    /// Fetches the approved source `id`, and says how it went, without
    /// its content.
    ///
    /// # Errors
    ///
    /// Returns why it was not fetched: not in the plan, already fetched,
    /// or a bound reached.
    pub fn fetch(&mut self, id: &str) -> Result<Value, String> {
        let Some(source) = self.plan.source(id) else {
            return Err(format!(
                "{id} is not in the approved plan; to read another source, ask for an amendment with amend_plan"
            ));
        };
        if let Some(done) = self.records.iter().find(|record| record.id == id) {
            return Err(format!(
                "{id} was already fetched at {}; sources are never re-fetched",
                crate::timestamp(done.fetched_at)
            ));
        }
        if self.records.len() >= self.bounds.sources() {
            return Err(format!(
                "the source bound ({}) is reached: gathering is over",
                self.bounds.sources()
            ));
        }
        let now = self.clock.now();
        if now >= self.deadline {
            self.out_of_time = true;
            return Err(String::from("the time bound is reached: gathering is over"));
        }
        let url = source.url.clone();
        let fetch = read(self.fetcher.fetch(&url));
        let reply = describe(id, &fetch);
        self.records.push(SourceRecord {
            id: id.to_owned(),
            url,
            fetched_at: now,
            fetch,
        });
        Ok(reply)
    }

    /// Asks the user to add `raw` to the plan, for the reason `why`.
    ///
    /// # Errors
    ///
    /// Returns why it was not added: not a usable URL, already planned,
    /// the plan full, no reason given, or the user declined.
    pub fn amend(&mut self, raw: &str, why: &str) -> Result<Value, String> {
        let Some(url) = SourceUrl::parse(raw.trim()) else {
            return Err(String::from(
                "not a usable https URL (no query, fragment, or userinfo)",
            ));
        };
        if let Some(known) = self.plan.find(&url) {
            return Err(format!("already in the plan as {}", known.id));
        }
        if self.plan.sources.len() >= MAX_SOURCES {
            return Err(format!(
                "the plan is full ({MAX_SOURCES} sources): nothing more can be added"
            ));
        }
        let why = why.trim();
        let Some(why) = field(why).filter(|why| !why.is_empty()) else {
            return Err(String::from("give a reason, of at most 4096 bytes"));
        };
        let decision = self.approver.review_amendment(&self.plan, &url, &why);
        let outcome = match decision {
            Decision::Approve => Ok(self.plan.push(url.clone(), why.clone(), Origin::Amendment)),
            Decision::Decline { reason } => Err(reason),
        };
        self.amendments.push(Amendment {
            url: url.as_str().to_owned(),
            why,
            outcome: outcome.clone(),
        });
        match outcome {
            Ok(id) => Ok(json!({ "added": id })),
            Err(reason) => Err(format!("the user declined: {reason}")),
        }
    }

    /// Ends gathering. `step_bound` says the phase ran out of turns.
    #[must_use]
    pub fn finish(self, step_bound: bool) -> Gathered {
        let stop = if self.out_of_time {
            StopReason::TimeBound
        } else if self.records.len() >= self.bounds.sources() {
            StopReason::SourceBound
        } else if step_bound {
            StopReason::StepBound
        } else {
            StopReason::Finished
        };
        Gathered {
            plan: self.plan,
            records: self.records,
            amendments: self.amendments,
            stop,
        }
    }
}

/// A fetch's result, as the record keeps it.
fn read(fetched: Result<Fetched, FetchFailure>) -> Fetch {
    match fetched {
        Ok(Fetched { status, body }) if (200..300).contains(&status) => {
            let (text, truncated) = evidence_text(&body);
            Fetch::Read {
                sha256: format!("{:x}", Sha256::digest(&body)),
                bytes: body.len(),
                text,
                truncated,
            }
        }
        Ok(Fetched { status, .. }) => Fetch::Status { status },
        Err(FetchFailure::Denied) => Fetch::Denied,
        Err(FetchFailure::Failed) => Fetch::Failed,
    }
}

/// `body` as text, cut to [`MAX_EVIDENCE_BYTES`] at a character
/// boundary, and whether it was cut.
fn evidence_text(body: &[u8]) -> (String, bool) {
    let text = String::from_utf8_lossy(body);
    if text.len() <= MAX_EVIDENCE_BYTES {
        return (text.into_owned(), false);
    }
    let cut = (0..=MAX_EVIDENCE_BYTES)
        .rev()
        .find(|at| text.is_char_boundary(*at))
        .unwrap_or(0);
    (text.get(..cut).unwrap_or_default().to_owned(), true)
}

/// How a fetch went, for the model: never the content.
fn describe(id: &str, fetch: &Fetch) -> Value {
    match fetch {
        Fetch::Read { sha256, bytes, .. } => json!({
            "source": id,
            "read": true,
            "bytes": bytes,
            "sha256": sha256,
            "content": "withheld until synthesis",
        }),
        Fetch::Status { status } => json!({ "source": id, "read": false, "status": status }),
        Fetch::Denied => json!({ "source": id, "read": false, "refused": "denied by egress" }),
        Fetch::Failed => json!({ "source": id, "read": false, "refused": "the fetch failed" }),
    }
}

/// Locks the shared state, poisoned or not.
fn lock<'g, 'a>(gather: &'g Mutex<Gather<'a>>) -> MutexGuard<'g, Gather<'a>> {
    gather.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `result` as a tool's output.
fn output(result: Result<Value, String>) -> ToolOutput {
    match result {
        Ok(value) => ToolOutput::Ok(value),
        Err(reason) => ToolOutput::Err(reason),
    }
}

/// The text argument `key`, or empty.
fn text<'v>(arguments: &'v Value, key: &str) -> &'v str {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// The `fetch_source` tool.
#[derive(Debug)]
pub struct FetchSource<'g, 'a> {
    gather: &'g Mutex<Gather<'a>>,
}

impl<'g, 'a> FetchSource<'g, 'a> {
    /// The tool over `gather`.
    #[must_use]
    pub const fn new(gather: &'g Mutex<Gather<'a>>) -> Self {
        Self { gather }
    }
}

impl Tool for FetchSource<'_, '_> {
    fn name(&self) -> &'static str {
        "fetch_source"
    }

    fn description(&self) -> &'static str {
        "Fetch one source from the approved plan, by its id (s1, s2, ...). Each source is fetched at most once. The reply says how the fetch went; the content is shown only at synthesis."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "description": "The source's id in the approved plan." }
            },
            "required": ["source"],
            "additionalProperties": false
        })
    }

    fn execute(&self, arguments: Value) -> Result<ToolOutput, LoopError> {
        Ok(output(lock(self.gather).fetch(text(&arguments, "source"))))
    }
}

/// The `amend_plan` tool.
#[derive(Debug)]
pub struct AmendPlan<'g, 'a> {
    gather: &'g Mutex<Gather<'a>>,
}

impl<'g, 'a> AmendPlan<'g, 'a> {
    /// The tool over `gather`.
    #[must_use]
    pub const fn new(gather: &'g Mutex<Gather<'a>>) -> Self {
        Self { gather }
    }
}

impl Tool for AmendPlan<'_, '_> {
    fn name(&self) -> &'static str {
        "amend_plan"
    }

    fn description(&self) -> &'static str {
        "Ask the user to add a source to the approved plan. The user decides; if they approve, the reply gives the new source's id to fetch."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The source's https URL, without a query or fragment." },
                "why": { "type": "string", "description": "Why the run needs it." }
            },
            "required": ["url", "why"],
            "additionalProperties": false
        })
    }

    fn execute(&self, arguments: Value) -> Result<ToolOutput, LoopError> {
        Ok(output(
            lock(self.gather).amend(text(&arguments, "url"), text(&arguments, "why")),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    const HELLO_SHA: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    /// A fixture page: its URL, and its status and body or why not.
    type Page<'s> = (&'s str, Result<(u16, &'s str), FetchFailure>);

    /// Pages by URL; records every fetch.
    struct Pages {
        pages: HashMap<String, Result<Fetched, FetchFailure>>,
        calls: Mutex<Vec<String>>,
    }

    impl Pages {
        fn new(pages: &[Page<'_>]) -> Self {
            Self {
                pages: pages
                    .iter()
                    .map(|(url, page)| {
                        let page = page.map(|(status, body)| Fetched {
                            status,
                            body: body.as_bytes().to_vec(),
                        });
                        ((*url).to_owned(), page)
                    })
                    .collect(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Fetcher for Pages {
        fn fetch(&self, url: &SourceUrl) -> Result<Fetched, FetchFailure> {
            self.calls.lock().unwrap().push(url.as_str().to_owned());
            self.pages
                .get(url.as_str())
                .cloned()
                .unwrap_or(Err(FetchFailure::Failed))
        }
    }

    /// Decides amendments from a queue; records what it was asked.
    struct User {
        decisions: Mutex<VecDeque<Decision>>,
        asked: Mutex<Vec<(String, String)>>,
    }

    impl User {
        fn deciding(decisions: Vec<Decision>) -> Self {
            Self {
                decisions: Mutex::new(decisions.into()),
                asked: Mutex::new(Vec::new()),
            }
        }

        fn asked(&self) -> Vec<(String, String)> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl Approver for User {
        fn review_plan(&self, _plan: &Plan) -> Decision {
            Decision::Approve
        }

        fn review_amendment(&self, _plan: &Plan, url: &SourceUrl, why: &str) -> Decision {
            self.asked
                .lock()
                .unwrap()
                .push((url.as_str().to_owned(), why.to_owned()));
            self.decisions
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Decision::Approve)
        }
    }

    /// A clock set by the test.
    struct At(AtomicI64);

    impl Clock for At {
        fn now(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn plan(count: usize) -> Plan {
        let sources: Vec<Value> = (1..=count)
            .map(|n| {
                let url = format!("https://s{n}.example/");
                json!({"url": url, "why": "w"})
            })
            .collect();
        let reply = json!({"strategy": "s", "sources": sources}).to_string();
        Plan::from_reply("Q", &reply).unwrap()
    }

    fn pages() -> Pages {
        Pages::new(&[
            ("https://s1.example/", Ok((200, "hello"))),
            ("https://s2.example/", Ok((404, "gone"))),
            ("https://s3.example/", Err(FetchFailure::Denied)),
            ("https://s4.example/", Err(FetchFailure::Failed)),
        ])
    }

    #[test]
    fn only_planned_sources_are_fetched_each_once_up_to_the_source_bound() {
        let (pages, user, clock) = (pages(), User::deciding(vec![]), At(AtomicI64::new(500)));
        let bounds = Bounds::new(2, 100).unwrap();
        let mut gather = Gather::new(plan(4), bounds, 1000, &pages, &user, &clock);
        assert_eq!(
            gather.fetch("s9"),
            Err(String::from(
                "s9 is not in the approved plan; to read another source, ask for an amendment with amend_plan"
            ))
        );
        assert!(gather.fetch("https://s1.example/").is_err());
        assert!(pages.calls().is_empty());
        assert_eq!(
            gather.fetch("s1"),
            Ok(
                json!({"source": "s1", "read": true, "bytes": 5, "sha256": HELLO_SHA, "content": "withheld until synthesis"})
            )
        );
        assert_eq!(
            gather.fetch("s1"),
            Err(String::from(
                "s1 was already fetched at 1970-01-01T00:08:20Z; sources are never re-fetched"
            ))
        );
        assert_eq!(
            gather.fetch("s2"),
            Ok(json!({"source": "s2", "read": false, "status": 404}))
        );
        assert_eq!(
            gather.fetch("s3"),
            Err(String::from(
                "the source bound (2) is reached: gathering is over"
            ))
        );
        assert_eq!(
            pages.calls(),
            ["https://s1.example/", "https://s2.example/"]
        );
        let gathered = gather.finish(false);
        assert_eq!(gathered.stop, StopReason::SourceBound);
        let ids: Vec<(&str, i64)> = gathered
            .records
            .iter()
            .map(|r| (r.id.as_str(), r.fetched_at))
            .collect();
        assert_eq!(ids, [("s1", 500), ("s2", 500)]);
        assert_eq!(
            gathered.records.first().map(|r| r.fetch.clone()),
            Some(Fetch::Read {
                sha256: String::from(HELLO_SHA),
                bytes: 5,
                text: String::from("hello"),
                truncated: false,
            })
        );
    }

    #[test]
    fn nothing_is_fetched_from_the_deadline_on() {
        let (pages, user, clock) = (pages(), User::deciding(vec![]), At(AtomicI64::new(999)));
        let mut gather = Gather::new(plan(4), Bounds::standard(), 1000, &pages, &user, &clock);
        assert!(gather.fetch("s1").is_ok());
        clock.0.store(1000, Ordering::SeqCst);
        assert_eq!(
            gather.fetch("s2"),
            Err(String::from("the time bound is reached: gathering is over"))
        );
        assert_eq!(pages.calls().len(), 1);
        assert_eq!(gather.finish(true).stop, StopReason::TimeBound);
    }

    #[test]
    fn gathering_says_why_it_ended() {
        let (pages, user, clock) = (pages(), User::deciding(vec![]), At(AtomicI64::new(0)));
        let fresh = || Gather::new(plan(4), Bounds::standard(), 1000, &pages, &user, &clock);
        assert_eq!(fresh().finish(false).stop, StopReason::Finished);
        assert_eq!(fresh().finish(true).stop, StopReason::StepBound);
        let mut denied = fresh();
        assert_eq!(
            denied.fetch("s3"),
            Ok(json!({"source": "s3", "read": false, "refused": "denied by egress"}))
        );
        assert_eq!(
            denied.fetch("s4"),
            Ok(json!({"source": "s4", "read": false, "refused": "the fetch failed"}))
        );
        let gathered = denied.finish(false);
        assert_eq!(gathered.stop, StopReason::Finished);
        let fetches: Vec<Fetch> = gathered.records.into_iter().map(|r| r.fetch).collect();
        assert_eq!(fetches, [Fetch::Denied, Fetch::Failed]);
    }

    #[test]
    fn only_a_2xx_status_is_evidence() {
        let at = |status: u16| {
            read(Ok(Fetched {
                status,
                body: b"hello".to_vec(),
            }))
        };
        assert_eq!(at(199), Fetch::Status { status: 199 });
        assert_eq!(at(300), Fetch::Status { status: 300 });
        for status in [200, 299] {
            assert!(
                matches!(at(status), Fetch::Read { bytes: 5, .. }),
                "{status}"
            );
        }
    }

    #[test]
    fn evidence_text_is_cut_at_its_bound_on_a_character_boundary() {
        let exact = "a".repeat(MAX_EVIDENCE_BYTES);
        assert_eq!(evidence_text(exact.as_bytes()), (exact.clone(), false));
        let over = format!("{exact}b");
        assert_eq!(evidence_text(over.as_bytes()), (exact, true));
        let split = format!("{}\u{e9}", "a".repeat(MAX_EVIDENCE_BYTES - 1));
        let (text, truncated) = evidence_text(split.as_bytes());
        assert!(truncated);
        assert_eq!(text, "a".repeat(MAX_EVIDENCE_BYTES - 1));
        assert_eq!(MAX_EVIDENCE_BYTES, 12 * 1024);
        // The record keeps the whole body's size and hash.
        let Fetch::Read { bytes, .. } = read(Ok(Fetched {
            status: 200,
            body: split.into_bytes(),
        })) else {
            unreachable!("a 200 is read")
        };
        assert_eq!(bytes, MAX_EVIDENCE_BYTES + 1);
    }

    #[test]
    fn an_amendment_is_the_users_decision_and_is_recorded_either_way() {
        let pages = pages();
        let user = User::deciding(vec![
            Decision::Approve,
            Decision::Decline {
                reason: String::from("not that one"),
            },
        ]);
        let clock = At(AtomicI64::new(0));
        let mut gather = Gather::new(plan(4), Bounds::standard(), 1000, &pages, &user, &clock);
        // Refused before the user is asked.
        assert_eq!(
            gather.amend("http://x.example/", "w"),
            Err(String::from(
                "not a usable https URL (no query, fragment, or userinfo)"
            ))
        );
        assert_eq!(
            gather.amend("https://s2.example:443/", "w"),
            Err(String::from("already in the plan as s2"))
        );
        assert_eq!(
            gather.amend("https://new.example/", "  "),
            Err(String::from("give a reason, of at most 4096 bytes"))
        );
        let long = "w".repeat(crate::plan::MAX_FIELD_BYTES + 1);
        assert!(gather.amend("https://new.example/", &long).is_err());
        assert!(user.asked().is_empty());
        // Asked, and approved.
        assert_eq!(
            gather.amend(" https://new.example/a ", " cited by s1 "),
            Ok(json!({"added": "s5"}))
        );
        let added = gather.plan.source("s5").unwrap();
        assert_eq!(added.origin, Origin::Amendment);
        assert_eq!(added.url.as_str(), "https://new.example/a");
        // Asked, and declined.
        assert_eq!(
            gather.amend("https://other.example/", "more"),
            Err(String::from("the user declined: not that one"))
        );
        assert_eq!(
            user.asked(),
            [
                (
                    String::from("https://new.example/a"),
                    String::from("cited by s1")
                ),
                (String::from("https://other.example/"), String::from("more")),
            ]
        );
        let gathered = gather.finish(false);
        assert_eq!(
            gathered.amendments,
            [
                Amendment {
                    url: String::from("https://new.example/a"),
                    why: String::from("cited by s1"),
                    outcome: Ok(String::from("s5")),
                },
                Amendment {
                    url: String::from("https://other.example/"),
                    why: String::from("more"),
                    outcome: Err(String::from("not that one")),
                },
            ]
        );
        assert_eq!(gathered.plan.sources.len(), 5);
    }

    #[test]
    fn a_full_plan_takes_no_amendment() {
        let (pages, user, clock) = (pages(), User::deciding(vec![]), At(AtomicI64::new(0)));
        let mut gather = Gather::new(
            plan(MAX_SOURCES - 1),
            Bounds::standard(),
            1000,
            &pages,
            &user,
            &clock,
        );
        assert!(gather.amend("https://last.example/", "w").is_ok());
        assert_eq!(
            gather.amend("https://more.example/", "w"),
            Err(String::from(
                "the plan is full (32 sources): nothing more can be added"
            ))
        );
        assert_eq!(user.asked().len(), 1);
    }

    #[test]
    fn the_tools_drive_the_shared_gather() {
        let (pages, user, clock) = (pages(), User::deciding(vec![]), At(AtomicI64::new(0)));
        let state = Mutex::new(Gather::new(
            plan(4),
            Bounds::standard(),
            1000,
            &pages,
            &user,
            &clock,
        ));
        let fetch = FetchSource::new(&state);
        let amend = AmendPlan::new(&state);
        assert_eq!((fetch.name(), amend.name()), ("fetch_source", "amend_plan"));
        assert!(fetch.description().contains("by its id"));
        assert!(amend.description().contains("The user decides"));
        assert_eq!(
            fetch.parameters_schema().get("required"),
            Some(&json!(["source"]))
        );
        assert_eq!(
            amend.parameters_schema().get("required"),
            Some(&json!(["url", "why"]))
        );
        assert!(matches!(
            fetch.execute(json!({"source": "s1"})),
            Ok(ToolOutput::Ok(value)) if value.get("read") == Some(&json!(true))
        ));
        assert!(matches!(
            fetch.execute(json!({})),
            Ok(ToolOutput::Err(reason)) if reason.starts_with(" is not in the approved plan")
        ));
        assert!(matches!(
            amend.execute(json!({"url": "https://n.example/", "why": "w"})),
            Ok(ToolOutput::Ok(value)) if value == json!({"added": "s5"})
        ));
        assert_eq!(pages.calls(), ["https://s1.example/"]);
    }

    #[test]
    fn the_system_clock_tells_the_time() {
        assert!(SystemClock.now() > 1_700_000_000);
    }

    #[test]
    fn a_gather_shows_its_plan_and_records_and_nothing_it_borrows() {
        let (pages, user, clock) = (pages(), User::deciding(vec![]), At(AtomicI64::new(0)));
        let shown = format!(
            "{:?}",
            Gather::new(plan(1), Bounds::standard(), 1000, &pages, &user, &clock)
        );
        assert!(
            shown.starts_with("Gather { plan: Plan { question: \"Q\""),
            "{shown}"
        );
        assert!(shown.ends_with("records: [], .. }"), "{shown}");
    }
}
