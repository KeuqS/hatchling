//! The report: claims bound to sources, disagreements kept apart, and
//! freshness stated up front.
//!
//! The model drafts; the host decides what the draft is worth:
//!
//! - **A citation binds only to a source the run read.** A cite to a
//!   source that was not read is dropped and flagged; a claim left with
//!   no citation is flagged **uncited**, never passed silently. A report
//!   in which no finding is bound to a source is a **draft**, and says so.
//! - **Citation depth is recorded:** primary (the source makes the claim)
//!   or secondary (it reports someone else's). A report whose cited
//!   findings rest only on secondary citations says so up front.
//! - **Disagreements have their own section,** always present, each side
//!   with its own citations. There is no field for a resolution, so there
//!   is no smoothed-over middle to render.
//! - **Freshness is stated up front:** when the sources were fetched, and
//!   that nothing was re-fetched. Every source carries its fetch time.

use std::fmt::Write as _;

use serde::Deserialize;

use crate::gather::{Fetch, Gathered, MAX_EVIDENCE_BYTES, StopReason};
use crate::plan::{MAX_FIELD_BYTES, json_object};

/// The most findings a report may have.
pub const MAX_FINDINGS: usize = 32;
/// The most disagreements a report may have.
pub const MAX_DISAGREEMENTS: usize = 8;
/// The most sides a disagreement may have.
pub const MAX_SIDES: usize = 4;
/// The most citations one claim may have.
pub const MAX_CITES: usize = 8;
/// The longest title, in characters; a longer one is cut.
pub const MAX_TITLE_CHARS: usize = 120;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftReply {
    title: String,
    summary: String,
    #[serde(default)]
    findings: Vec<ClaimReply>,
    #[serde(default)]
    disagreements: Vec<DisagreementReply>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimReply {
    claim: String,
    #[serde(default)]
    cites: Vec<CiteReply>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CiteReply {
    source: String,
    depth: DepthReply,
    #[serde(default)]
    via: Option<String>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum DepthReply {
    Primary,
    Secondary,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisagreementReply {
    topic: String,
    sides: Vec<SideReply>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SideReply {
    position: String,
    #[serde(default)]
    cites: Vec<CiteReply>,
}

/// How directly a source supports a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Depth {
    /// The source itself makes the claim.
    Primary,
    /// The source reports someone else's claim.
    Secondary {
        /// Whose, as the draft named it.
        via: Option<String>,
    },
}

/// A citation bound to a source the run read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cite {
    /// The source's id.
    pub source: String,
    /// How directly it supports the claim.
    pub depth: Depth,
}

/// Something wrong with a claim's citations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flag {
    /// No citation binds it to a source the run read.
    Uncited,
    /// It cites a source the run did not read.
    NotRead {
        /// The id cited.
        source: String,
    },
}

/// One claim, as the report keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// What it says.
    pub text: String,
    /// The citations that bind it to sources the run read.
    pub cites: Vec<Cite>,
    /// What is wrong with its citations, if anything.
    pub flags: Vec<Flag>,
}

impl Claim {
    /// Whether any citation binds it to a source.
    #[must_use]
    pub fn is_cited(&self) -> bool {
        !self.cites.is_empty()
    }

    /// Whether a source itself makes it.
    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.cites.iter().any(|cite| cite.depth == Depth::Primary)
    }
}

/// Where the sources disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    /// About what.
    pub topic: String,
    /// Each side, with its own citations.
    pub sides: Vec<Claim>,
}

/// Whether the report's findings are bound to sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// At least one finding is bound to a source.
    Cited,
    /// No finding is bound to a source.
    Draft,
}

/// A research report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Its title.
    pub title: String,
    /// The model's overview.
    pub summary: String,
    /// Its findings, in order.
    pub findings: Vec<Claim>,
    /// Where the sources disagree.
    pub disagreements: Vec<Disagreement>,
    /// Whether its findings are bound to sources.
    pub status: Status,
}

/// Why a drafted report cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReportError {
    /// The reply holds no report object.
    #[error("the report is not the JSON object asked for")]
    NotJson,
    /// The draft is past a bound: too many items, or a field too long.
    #[error("the report is past its bounds")]
    TooLarge,
}

impl Report {
    /// Reads the model's drafted report against what was `gathered`.
    ///
    /// # Errors
    ///
    /// Returns why the reply is not a usable draft.
    pub fn from_reply(reply: &str, gathered: &Gathered) -> Result<Self, ReportError> {
        let draft: DraftReply = json_object(reply).ok_or(ReportError::NotJson)?;
        if !within_bounds(&draft) {
            return Err(ReportError::TooLarge);
        }
        let read: Vec<&str> = gathered
            .records
            .iter()
            .filter(|record| matches!(record.fetch, Fetch::Read { .. }))
            .map(|record| record.id.as_str())
            .collect();
        let findings: Vec<Claim> = draft
            .findings
            .into_iter()
            .map(|reply| claim(&reply.claim, reply.cites, &read))
            .collect();
        let disagreements = draft
            .disagreements
            .into_iter()
            .map(|reply| Disagreement {
                topic: one_line(&reply.topic),
                sides: reply
                    .sides
                    .into_iter()
                    .map(|side| claim(&side.position, side.cites, &read))
                    .collect(),
            })
            .collect();
        let status = if findings.iter().any(Claim::is_cited) {
            Status::Cited
        } else {
            Status::Draft
        };
        Ok(Self {
            title: title(&draft.title),
            summary: draft.summary.trim().to_owned(),
            findings,
            disagreements,
            status,
        })
    }

    /// The report as the document builder's inputs, in order: the
    /// `title`, then its sections, each headed by its input's name.
    #[must_use]
    pub fn inputs(&self, gathered: &Gathered) -> Vec<(&'static str, String)> {
        vec![
            ("title", self.title.clone()),
            ("status", self.status_section(gathered)),
            ("summary", self.summary_section()),
            ("findings", self.findings_section()),
            ("disagreements", self.disagreements_section()),
            ("plan", plan_section(gathered)),
            ("sources", sources_section(gathered)),
        ]
    }

    fn status_section(&self, gathered: &Gathered) -> String {
        let status = match self.status {
            Status::Cited => "Status: cited report.",
            Status::Draft => "Status: draft. No finding is bound to a source the run read.",
        };
        let primary = self
            .findings
            .iter()
            .filter(|claim| claim.is_primary())
            .count();
        let cited = self
            .findings
            .iter()
            .filter(|claim| claim.is_cited())
            .count();
        let secondary = cited.saturating_sub(primary);
        let uncited = self.findings.len().saturating_sub(cited);
        let mut out = format!(
            "{status}\n\n{}\n\nEvidence: of {} findings, {primary} rest on a primary source, {secondary} only on secondary reports, and {uncited} are uncited.",
            freshness(gathered),
            self.findings.len(),
        );
        if cited > 0 && primary == 0 {
            out.push_str("\n\nThis report is built entirely on secondary citations: every cited finding rests on a source reporting someone else's claim.");
        }
        out.push_str("\n\n");
        out.push_str(&stopped(gathered));
        out
    }

    fn summary_section(&self) -> String {
        format!(
            "{}\n\nThis overview is the model's. Its claims are bound to sources only under findings.",
            self.summary
        )
    }

    fn findings_section(&self) -> String {
        if self.findings.is_empty() {
            return String::from("No findings were drafted.");
        }
        let lines: Vec<String> = self
            .findings
            .iter()
            .enumerate()
            .map(|(at, claim)| format!("{}. {}", at.saturating_add(1), render_claim(claim)))
            .collect();
        lines.join("\n")
    }

    fn disagreements_section(&self) -> String {
        if self.disagreements.is_empty() {
            return String::from("The draft records no disagreement between the sources.");
        }
        let blocks: Vec<String> = self
            .disagreements
            .iter()
            .map(|disagreement| {
                let mut block = format!("On {}:", disagreement.topic);
                for side in &disagreement.sides {
                    let _ = write!(block, "\n- {}", render_claim(side));
                }
                if disagreement.sides.len() < 2 {
                    block.push_str("\n  Flag: only one side was given.");
                }
                block
            })
            .collect();
        blocks.join("\n\n")
    }
}

/// `text`, its citations, and its flags.
fn claim(text: &str, cites: Vec<CiteReply>, read: &[&str]) -> Claim {
    let mut bound = Vec::new();
    let mut flags = Vec::new();
    for cite in cites {
        if read.contains(&cite.source.as_str()) {
            let depth = match cite.depth {
                DepthReply::Primary => Depth::Primary,
                DepthReply::Secondary => Depth::Secondary {
                    via: cite
                        .via
                        .as_deref()
                        .map(one_line)
                        .filter(|via| !via.is_empty()),
                },
            };
            bound.push(Cite {
                source: cite.source,
                depth,
            });
        } else {
            flags.push(Flag::NotRead {
                source: cite.source,
            });
        }
    }
    if bound.is_empty() {
        flags.insert(0, Flag::Uncited);
    }
    Claim {
        text: one_line(text),
        cites: bound,
        flags,
    }
}

/// Whether every count and field of `draft` is within its bound.
fn within_bounds(draft: &DraftReply) -> bool {
    let fits = |text: &str| text.len() <= MAX_FIELD_BYTES;
    let cites_fit = |cites: &[CiteReply]| {
        cites.len() <= MAX_CITES
            && cites
                .iter()
                .all(|cite| fits(&cite.source) && cite.via.as_deref().is_none_or(fits))
    };
    draft.findings.len() <= MAX_FINDINGS
        && draft.disagreements.len() <= MAX_DISAGREEMENTS
        && fits(&draft.title)
        && fits(&draft.summary)
        && draft
            .findings
            .iter()
            .all(|claim| fits(&claim.claim) && cites_fit(&claim.cites))
        && draft.disagreements.iter().all(|disagreement| {
            fits(&disagreement.topic)
                && disagreement.sides.len() <= MAX_SIDES
                && disagreement
                    .sides
                    .iter()
                    .all(|side| fits(&side.position) && cites_fit(&side.cites))
        })
}

/// The title: its first line, without control characters, cut to
/// [`MAX_TITLE_CHARS`], or "Research report" if that leaves nothing.
fn title(raw: &str) -> String {
    let cut: String = raw
        .trim()
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TITLE_CHARS)
        .collect();
    let cut = cut.trim();
    if cut.is_empty() {
        String::from("Research report")
    } else {
        cut.to_owned()
    }
}

/// `text` on one line: its words, single-spaced.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A claim with its citations and flags, as the report shows it.
fn render_claim(claim: &Claim) -> String {
    let mut out = claim.text.clone();
    for cite in &claim.cites {
        match &cite.depth {
            Depth::Primary => {
                let _ = write!(out, " [{}]", cite.source);
            }
            Depth::Secondary { via: Some(via) } => {
                let _ = write!(out, " [{}, secondary, reporting {via}]", cite.source);
            }
            Depth::Secondary { via: None } => {
                let _ = write!(out, " [{}, secondary]", cite.source);
            }
        }
    }
    for flag in &claim.flags {
        match flag {
            Flag::Uncited => {
                out.push_str("\n   Flag: uncited. No citation binds it to a source the run read.");
            }
            Flag::NotRead { source } => {
                let _ = write!(
                    out,
                    "\n   Flag: cites {source}, which the run did not read."
                );
            }
        }
    }
    out
}

/// When the evidence was fetched, stated up front.
fn freshness(gathered: &Gathered) -> String {
    let times = gathered
        .records
        .iter()
        .filter(|record| matches!(record.fetch, Fetch::Read { .. }))
        .map(|record| record.fetched_at);
    match times.clone().min().zip(times.max()) {
        Some((first, last)) => format!(
            "Freshness: the sources were fetched from {} to {} (UTC). Nothing was re-fetched; to freshen them, run the research again.",
            crate::timestamp(first),
            crate::timestamp(last)
        ),
        None => String::from("Freshness: no source was read."),
    }
}

/// Why gathering ended.
fn stopped(gathered: &Gathered) -> String {
    match gathered.stop {
        StopReason::Finished => String::from("Gathering ended when the model was done."),
        StopReason::SourceBound => String::from("Gathering stopped at the run's source bound."),
        StopReason::TimeBound => String::from("Gathering stopped at the run's time bound."),
        StopReason::StepBound => String::from("Gathering stopped at its step bound."),
    }
}

/// The approved plan and every amendment asked for.
fn plan_section(gathered: &Gathered) -> String {
    let mut out = gathered.plan.render();
    if gathered.amendments.is_empty() {
        out.push_str("\n\nNo amendment was asked for during gathering.");
        return out;
    }
    out.push_str("\n\nAmendments asked for during gathering:");
    for amendment in &gathered.amendments {
        let outcome = match &amendment.outcome {
            Ok(id) => format!("approved by the user, added as {id}"),
            Err(reason) => format!("declined by the user: {}", one_line(reason)),
        };
        let _ = write!(
            out,
            "\n- {} ({}): {outcome}",
            amendment.url,
            one_line(&amendment.why)
        );
    }
    out
}

/// Every planned source: what became of it, and when.
fn sources_section(gathered: &Gathered) -> String {
    let lines: Vec<String> = gathered
        .plan
        .sources
        .iter()
        .map(|source| {
            let record = gathered
                .records
                .iter()
                .find(|record| record.id == source.id);
            let what = record.map_or_else(
                || String::from("not fetched"),
                |record| {
                    let at = crate::timestamp(record.fetched_at);
                    match &record.fetch {
                        Fetch::Read {
                            sha256,
                            bytes,
                            truncated,
                            ..
                        } => {
                            let cut = if *truncated {
                                format!("; synthesis saw its first {MAX_EVIDENCE_BYTES} bytes")
                            } else {
                                String::new()
                            };
                            format!("read {at}, {bytes} bytes, sha256 {sha256}{cut}")
                        }
                        Fetch::Status { status } => {
                            format!("fetched {at}: status {status}, not evidence")
                        }
                        Fetch::Denied => format!("refused {at}: denied by egress"),
                        Fetch::Failed => format!("failed {at}: the fetch did not complete"),
                    }
                },
            );
            format!("- [{}] {}: {what}", source.id, source.url.as_str())
        })
        .collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gather::SourceRecord;
    use crate::plan::Plan;
    use crate::url::SourceUrl;

    fn text(len: usize) -> String {
        "x".repeat(len)
    }

    fn cite() -> CiteReply {
        CiteReply {
            source: String::from("s1"),
            depth: DepthReply::Primary,
            via: Some(String::from("v")),
        }
    }

    fn side() -> SideReply {
        SideReply {
            position: String::from("p"),
            cites: vec![cite()],
        }
    }

    fn valid() -> DraftReply {
        DraftReply {
            title: String::from("t"),
            summary: String::from("s"),
            findings: vec![ClaimReply {
                claim: String::from("c"),
                cites: vec![cite()],
            }],
            disagreements: vec![DisagreementReply {
                topic: String::from("d"),
                sides: vec![side()],
            }],
        }
    }

    fn with(change: impl FnOnce(&mut DraftReply)) -> bool {
        let mut draft = valid();
        change(&mut draft);
        within_bounds(&draft)
    }

    fn first_claim(draft: &mut DraftReply) -> &mut ClaimReply {
        draft.findings.first_mut().unwrap()
    }

    fn first_disagreement(draft: &mut DraftReply) -> &mut DisagreementReply {
        draft.disagreements.first_mut().unwrap()
    }

    #[test]
    fn every_count_of_a_draft_is_bounded_at_its_limit() {
        assert!(with(|_| ()));
        // Counts: at the bound, and one past it.
        for (count, ok) in [(MAX_FINDINGS, true), (MAX_FINDINGS + 1, false)] {
            assert_eq!(
                with(|d| d.findings = (0..count)
                    .map(|_| ClaimReply {
                        claim: String::from("c"),
                        cites: vec![]
                    })
                    .collect()),
                ok
            );
        }
        for (count, ok) in [(MAX_DISAGREEMENTS, true), (MAX_DISAGREEMENTS + 1, false)] {
            assert_eq!(
                with(|d| d.disagreements = (0..count)
                    .map(|_| DisagreementReply {
                        topic: String::from("d"),
                        sides: vec![]
                    })
                    .collect()),
                ok
            );
        }
        for (count, ok) in [(MAX_CITES, true), (MAX_CITES + 1, false)] {
            assert_eq!(
                with(|d| first_claim(d).cites = (0..count).map(|_| cite()).collect()),
                ok
            );
            let side_cites = with(|d| {
                if let Some(side) = first_disagreement(d).sides.first_mut() {
                    side.cites = (0..count).map(|_| cite()).collect();
                }
            });
            assert_eq!(side_cites, ok);
        }
        for (count, ok) in [(MAX_SIDES, true), (MAX_SIDES + 1, false)] {
            assert_eq!(
                with(|d| first_disagreement(d).sides = (0..count).map(|_| side()).collect()),
                ok
            );
        }
    }

    #[test]
    fn every_field_of_a_draft_is_bounded_at_its_limit() {
        for (len, ok) in [(MAX_FIELD_BYTES, true), (MAX_FIELD_BYTES + 1, false)] {
            assert_eq!(with(|d| d.title = text(len)), ok, "title {len}");
            assert_eq!(with(|d| d.summary = text(len)), ok, "summary {len}");
            assert_eq!(
                with(|d| first_claim(d).claim = text(len)),
                ok,
                "claim {len}"
            );
            assert_eq!(
                with(|d| first_disagreement(d).topic = text(len)),
                ok,
                "topic {len}"
            );
            assert_eq!(
                with(|d| {
                    if let Some(cite) = first_claim(d).cites.first_mut() {
                        cite.source = text(len);
                    }
                }),
                ok,
                "source {len}"
            );
            assert_eq!(
                with(|d| {
                    if let Some(cite) = first_claim(d).cites.first_mut() {
                        cite.via = Some(text(len));
                    }
                }),
                ok,
                "via {len}"
            );
            assert_eq!(
                with(|d| {
                    if let Some(side) = first_disagreement(d).sides.first_mut() {
                        side.position = text(len);
                    }
                }),
                ok,
                "position {len}"
            );
            assert_eq!(
                with(|d| {
                    if let Some(cite) = first_disagreement(d)
                        .sides
                        .first_mut()
                        .and_then(|side| side.cites.first_mut())
                    {
                        cite.source = text(len);
                    }
                }),
                ok,
                "side source {len}"
            );
        }
        assert!(with(|d| {
            if let Some(cite) = first_claim(d).cites.first_mut() {
                cite.via = None;
            }
        }));
        assert_eq!(
            (
                MAX_FINDINGS,
                MAX_DISAGREEMENTS,
                MAX_SIDES,
                MAX_CITES,
                MAX_TITLE_CHARS
            ),
            (32, 8, 4, 8, 120)
        );
    }

    fn gathered(records: Vec<SourceRecord>, stop: StopReason) -> Gathered {
        let reply = serde_json::json!({"strategy": "s", "sources": [
            {"url": "https://a.example/", "why": "w"},
            {"url": "https://b.example/", "why": "w"},
            {"url": "https://c.example/", "why": "w"},
            {"url": "https://d.example/", "why": "w"},
            {"url": "https://e.example/", "why": "w"}
        ]})
        .to_string();
        Gathered {
            plan: Plan::from_reply("Q", &reply).unwrap(),
            records,
            amendments: Vec::new(),
            stop,
        }
    }

    fn record(id: &str, url: &str, at: i64, fetch: Fetch) -> SourceRecord {
        SourceRecord {
            id: id.to_owned(),
            url: SourceUrl::parse(url).unwrap(),
            fetched_at: at,
            fetch,
        }
    }

    #[test]
    fn a_draft_past_its_bounds_is_refused() {
        let reply =
            serde_json::json!({"title": text(MAX_FIELD_BYTES + 1), "summary": "s"}).to_string();
        assert_eq!(
            Report::from_reply(&reply, &gathered(vec![], StopReason::Finished)),
            Err(ReportError::TooLarge)
        );
    }

    #[test]
    fn every_source_says_what_became_of_it_and_freshness_spans_the_read_ones() {
        let read = |truncated| Fetch::Read {
            sha256: String::from("ab"),
            bytes: 9,
            text: String::new(),
            truncated,
        };
        let world = gathered(
            vec![
                record("s1", "https://a.example/", 1_790_000_060, read(true)),
                record("s2", "https://b.example/", 1_790_000_000, read(false)),
                record(
                    "s3",
                    "https://c.example/",
                    1_789_000_000,
                    Fetch::Status { status: 404 },
                ),
                record("s4", "https://d.example/", 1_789_000_000, Fetch::Denied),
                record("s5", "https://e.example/", 1_791_000_000, Fetch::Failed),
            ],
            StopReason::TimeBound,
        );
        assert_eq!(
            sources_section(&world),
            "- [s1] https://a.example/: read 2026-09-21T14:14:20Z, 9 bytes, sha256 ab; synthesis saw its first 12288 bytes\n- [s2] https://b.example/: read 2026-09-21T14:13:20Z, 9 bytes, sha256 ab\n- [s3] https://c.example/: fetched 2026-09-10T00:26:40Z: status 404, not evidence\n- [s4] https://d.example/: refused 2026-09-10T00:26:40Z: denied by egress\n- [s5] https://e.example/: failed 2026-10-03T04:00:00Z: the fetch did not complete"
        );
        // Only read sources count toward freshness.
        assert_eq!(
            freshness(&world),
            "Freshness: the sources were fetched from 2026-09-21T14:13:20Z to 2026-09-21T14:14:20Z (UTC). Nothing was re-fetched; to freshen them, run the research again."
        );
        assert_eq!(
            stopped(&world),
            "Gathering stopped at the run's time bound."
        );
        assert_eq!(
            stopped(&gathered(vec![], StopReason::SourceBound)),
            "Gathering stopped at the run's source bound."
        );
        assert_eq!(
            stopped(&gathered(vec![], StopReason::StepBound)),
            "Gathering stopped at its step bound."
        );
        assert_eq!(
            stopped(&gathered(vec![], StopReason::Finished)),
            "Gathering ended when the model was done."
        );
    }

    #[test]
    fn a_title_is_one_clean_line_of_at_most_its_bound() {
        assert_eq!(title("\n\n  A title \nsecond"), "A title");
        assert_eq!(title("A\u{7}b\tc"), "Abc");
        assert_eq!(
            title(&"y".repeat(MAX_TITLE_CHARS + 5)),
            "y".repeat(MAX_TITLE_CHARS)
        );
        assert_eq!(title(" \n "), "Research report");
        assert_eq!(one_line(" a\n b\t\tc "), "a b c");
    }
}
