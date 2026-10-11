//! The plan: what the run will read, and why.
//!
//! The model proposes a strategy and a list of candidate sources, best
//! first (at most [`MAX_SOURCES`]); the user approves it before anything
//! is fetched, and the gather phase can fetch only what it lists, and at
//! most the run's source bound of those. A source added later is an **amendment** the user
//! decides on, and the plan says which sources came that way.
//!
//! Source ids (`s1`, `s2`, …) are the host's, assigned in order. The
//! model never chooses one.

use std::fmt::Write as _;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::bounds::MAX_SOURCES;
use crate::url::SourceUrl;

/// The longest text the model may give for any one field, in bytes.
pub const MAX_FIELD_BYTES: usize = 4096;

/// How a source came to be in the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Listed in the plan the user approved.
    Plan,
    /// Added mid-run by an amendment the user approved.
    Amendment,
}

/// One source the plan lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSource {
    /// Its id: `s1`, `s2`, …, in plan order.
    pub id: String,
    /// Where it is.
    pub url: SourceUrl,
    /// Why the run reads it.
    pub why: String,
    /// How it came to be in the plan.
    pub origin: Origin,
}

/// A research plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The question the run answers.
    pub question: String,
    /// How the run means to answer it.
    pub strategy: String,
    /// The sources it may read, in order.
    pub sources: Vec<PlannedSource>,
}

/// Why a proposed plan cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// The reply holds no plan object.
    #[error("the plan is not the JSON object asked for")]
    NotJson,
    /// The strategy is blank.
    #[error("the plan has no strategy")]
    NoStrategy,
    /// No source is listed.
    #[error("the plan lists no sources")]
    NoSources,
    /// More than [`MAX_SOURCES`] sources are listed.
    #[error("the plan lists more than {MAX_SOURCES} sources")]
    TooManySources,
    /// A source's URL is not fetchable.
    #[error("source {index} is not a usable https URL")]
    BadUrl {
        /// Its position in the list, from 0.
        index: usize,
    },
    /// A source repeats an earlier one.
    #[error("source {index} repeats an earlier one")]
    Repeated {
        /// Its position in the list, from 0.
        index: usize,
    },
    /// A field is longer than [`MAX_FIELD_BYTES`].
    #[error("a field is longer than {MAX_FIELD_BYTES} bytes")]
    TooLong,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanReply {
    strategy: String,
    sources: Vec<SourceReply>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceReply {
    url: String,
    why: String,
}

impl Plan {
    /// Reads the model's proposed plan for `question`.
    ///
    /// # Errors
    ///
    /// Returns why the reply is not a usable plan.
    pub fn from_reply(question: &str, reply: &str) -> Result<Self, PlanError> {
        let parsed: PlanReply = json_object(reply).ok_or(PlanError::NotJson)?;
        let strategy = parsed.strategy.trim();
        if strategy.is_empty() {
            return Err(PlanError::NoStrategy);
        }
        if parsed.sources.is_empty() {
            return Err(PlanError::NoSources);
        }
        if parsed.sources.len() > MAX_SOURCES {
            return Err(PlanError::TooManySources);
        }
        let mut plan = Self {
            question: question.to_owned(),
            strategy: field(strategy).ok_or(PlanError::TooLong)?,
            sources: Vec::new(),
        };
        for (index, source) in parsed.sources.iter().enumerate() {
            let url = SourceUrl::parse(source.url.trim()).ok_or(PlanError::BadUrl { index })?;
            if plan.find(&url).is_some() {
                return Err(PlanError::Repeated { index });
            }
            let why = field(source.why.trim()).ok_or(PlanError::TooLong)?;
            plan.push(url, why, Origin::Plan);
        }
        Ok(plan)
    }

    /// The source with `id`.
    #[must_use]
    pub fn source(&self, id: &str) -> Option<&PlannedSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    /// The source at `url`.
    #[must_use]
    pub fn find(&self, url: &SourceUrl) -> Option<&PlannedSource> {
        self.sources.iter().find(|source| source.url == *url)
    }

    /// Adds a source under the next id, and returns the id.
    pub fn push(&mut self, url: SourceUrl, why: String, origin: Origin) -> String {
        let id = format!("s{}", self.sources.len().saturating_add(1));
        self.sources.push(PlannedSource {
            id: id.clone(),
            url,
            why,
            origin,
        });
        id
    }

    /// The plan as the user approves it and the model reads it.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!(
            "Question: {}\nStrategy: {}\nApproved sources:",
            self.question, self.strategy
        );
        for source in &self.sources {
            let added = match source.origin {
                Origin::Plan => "",
                Origin::Amendment => "; added by amendment",
            };
            let _ = write!(
                out,
                "\n- {} {} ({}{added})",
                source.id,
                source.url.as_str(),
                source.why
            );
        }
        out
    }
}

/// `text`, if it is no longer than [`MAX_FIELD_BYTES`].
pub(crate) fn field(text: &str) -> Option<String> {
    (text.len() <= MAX_FIELD_BYTES).then(|| text.to_owned())
}

/// The JSON object in a model's reply: the text from its first `{` to
/// its last `}`, so a reply wrapped in a code fence still reads.
pub(crate) fn json_object<T: DeserializeOwned>(reply: &str) -> Option<T> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    serde_json::from_str(reply.get(start..=end)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(sources: &[(&str, &str)]) -> String {
        let listed: Vec<serde_json::Value> = sources
            .iter()
            .map(|(url, why)| serde_json::json!({"url": url, "why": why}))
            .collect();
        serde_json::json!({"strategy": "Read the specs.", "sources": listed}).to_string()
    }

    #[test]
    fn a_plan_reads_from_the_reply_with_host_assigned_ids() {
        let text = format!(
            "Here is the plan:\n```json\n{}\n```",
            reply(&[
                ("https://a.example/x", " the spec "),
                ("https://b.example/y", "a survey"),
            ])
        );
        let plan = Plan::from_reply("Why?", &text).unwrap();
        assert_eq!(plan.question, "Why?");
        assert_eq!(plan.strategy, "Read the specs.");
        let ids: Vec<&str> = plan.sources.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["s1", "s2"]);
        assert_eq!(plan.source("s1").unwrap().why, "the spec");
        assert_eq!(
            plan.source("s2").unwrap().url.as_str(),
            "https://b.example/y"
        );
        assert_eq!(plan.source("s3"), None);
        assert!(plan.sources.iter().all(|s| s.origin == Origin::Plan));
        assert_eq!(
            plan.render(),
            "Question: Why?\nStrategy: Read the specs.\nApproved sources:\n- s1 https://a.example/x (the spec)\n- s2 https://b.example/y (a survey)"
        );
    }

    #[test]
    fn an_amended_source_is_named_as_such() {
        let mut plan = Plan::from_reply("Q", &reply(&[("https://a.example/", "w")])).unwrap();
        let url = SourceUrl::parse("https://c.example/").unwrap();
        assert_eq!(
            plan.push(url.clone(), String::from("found"), Origin::Amendment),
            "s2"
        );
        assert_eq!(plan.find(&url).map(|s| s.id.as_str()), Some("s2"));
        assert!(
            plan.render()
                .ends_with("\n- s2 https://c.example/ (found; added by amendment)")
        );
    }

    /// A plan of `count` distinct sources.
    fn many(count: usize) -> String {
        let urls: Vec<String> = (0..count)
            .map(|n| format!("https://s{n}.example/"))
            .collect();
        let pairs: Vec<(&str, &str)> = urls.iter().map(|url| (url.as_str(), "w")).collect();
        reply(&pairs)
    }

    #[test]
    fn a_plan_may_list_as_many_candidates_as_the_ceiling() {
        let plan = Plan::from_reply("Q", &many(MAX_SOURCES)).unwrap();
        assert_eq!(plan.sources.len(), 32);
        assert_eq!(plan.sources.last().map(|s| s.id.as_str()), Some("s32"));
    }

    #[test]
    fn an_unusable_plan_says_why() {
        let cases = [
            (String::from("no json here"), PlanError::NotJson),
            (
                String::from("{\"strategy\": 1, \"sources\": []}"),
                PlanError::NotJson,
            ),
            (
                String::from("{\"strategy\": \"s\", \"sources\": [], \"extra\": 1}"),
                PlanError::NotJson,
            ),
            (
                String::from(
                    "{\"strategy\": \"  \", \"sources\": [{\"url\": \"https://a.example/\", \"why\": \"w\"}]}",
                ),
                PlanError::NoStrategy,
            ),
            (
                String::from("{\"strategy\": \"s\", \"sources\": []}"),
                PlanError::NoSources,
            ),
            (many(MAX_SOURCES + 1), PlanError::TooManySources),
            (
                reply(&[("https://a.example/", "w"), ("http://b.example/", "w")]),
                PlanError::BadUrl { index: 1 },
            ),
            (
                reply(&[("https://a.example/", "w"), ("https://a.example:443/", "w")]),
                PlanError::Repeated { index: 1 },
            ),
        ];
        for (text, error) in cases {
            assert_eq!(Plan::from_reply("Q", &text), Err(error), "{text}");
        }
    }

    #[test]
    fn every_field_is_bounded() {
        let long = "x".repeat(MAX_FIELD_BYTES + 1);
        let at = "x".repeat(MAX_FIELD_BYTES);
        let why_long = reply(&[("https://a.example/", long.as_str())]);
        assert_eq!(Plan::from_reply("Q", &why_long), Err(PlanError::TooLong));
        let why_at = reply(&[("https://a.example/", at.as_str())]);
        assert!(Plan::from_reply("Q", &why_at).is_ok());
        let strategy_long = serde_json::json!({
            "strategy": long,
            "sources": [{"url": "https://a.example/", "why": "w"}]
        })
        .to_string();
        assert_eq!(
            Plan::from_reply("Q", &strategy_long),
            Err(PlanError::TooLong)
        );
        assert_eq!(MAX_FIELD_BYTES, 4096);
    }
}
