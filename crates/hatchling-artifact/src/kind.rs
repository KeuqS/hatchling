//! Build kinds, and the inspector.
//!
//! **A kind is admitted by registering a profile.** A [`KindProfile`]
//! names the kind, the content type its output must conform to, its size
//! bound, and its own shape checks. The broker builds only kinds its
//! [`Kinds`] registry holds, and inspects every completed build against
//! its kind's profile. Nothing else in the pipeline knows any kind: a new
//! kind, media included, is a new profile and no pipeline change.
//!
//! **The inspector is advisory.** [`inspect`] runs the generic checks
//! (empty, size, control characters) and then the profile's own, and
//! reports what it found. The broker records the report in provenance
//! and surfaces it; it never decides a delivery. The share-review gate
//! alone does that, so an inspector pass cannot let blocked bytes out,
//! and an inspector failure cannot stop bytes the gate passed.
//!
//! **Findings never quote the content.** A finding names the check and
//! describes the problem in the inspector's own words, so a surfaced
//! report cannot carry an artifact's bytes around the gate.

use std::fmt;
use std::sync::Arc;

use hatchling_core::{
    ArtifactInspection, InspectionFinding, InspectionVerdict, MAX_SKILL_OUTPUT_BYTES,
};
use serde_json::Value;

use crate::request::PinnedRequest;

/// The content type of a media build's output: a render manifest, not
/// the media itself (rendering is a later builder's job).
pub const MEDIA_MANIFEST_TYPE: &str = "application/vnd.hatchling.media-manifest+json";

/// What the pipeline knows about one build kind.
pub trait KindProfile: Send + Sync {
    /// The kind, a token (`document`, `page`, `video`, …).
    fn kind(&self) -> &str;

    /// The content type the kind's output must conform to.
    fn content_type(&self) -> &str;

    /// The kind's size bound, in bytes.
    fn max_bytes(&self) -> usize {
        MAX_SKILL_OUTPUT_BYTES
    }

    /// The kind's own shape checks on `content`, built from `request`.
    /// Each problem is one finding; a finding never quotes the content.
    fn check(&self, request: &PinnedRequest, content: &str) -> Vec<InspectionFinding>;
}

/// A finding from `check`, described as `detail`.
#[must_use]
pub fn finding(check: &str, detail: &str) -> InspectionFinding {
    InspectionFinding {
        check: check.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Inspects `content`, built from `request`, against `profile`: the
/// generic checks first, then the profile's own.
#[must_use]
pub fn inspect(
    profile: &dyn KindProfile,
    request: &PinnedRequest,
    content: &str,
) -> ArtifactInspection {
    let mut findings = Vec::new();
    if content.trim().is_empty() {
        findings.push(finding("empty", "the output is empty"));
    }
    if content.len() > profile.max_bytes() {
        findings.push(finding(
            "size",
            &format!(
                "the output is {} bytes, over the kind's {}",
                content.len(),
                profile.max_bytes()
            ),
        ));
    }
    if content
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        findings.push(finding("control", "the output holds control characters"));
    }
    findings.extend(profile.check(request, content));
    ArtifactInspection {
        verdict: if findings.is_empty() {
            InspectionVerdict::Pass
        } else {
            InspectionVerdict::Fail
        },
        content_type: profile.content_type().to_owned(),
        findings,
    }
}

/// The kinds a broker builds: one profile per kind.
#[derive(Clone)]
pub struct Kinds {
    profiles: Vec<Arc<dyn KindProfile>>,
}

impl Kinds {
    /// No kinds: every build is refused.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    /// The standard kinds: `document`, `page`, and the `video` media
    /// kind.
    #[must_use]
    pub fn standard() -> Self {
        Self::empty()
            .with(DocumentProfile)
            .with(PageProfile)
            .with(MediaProfile::new("video", "video/mp4"))
    }

    /// These kinds plus `profile`, which replaces any profile for the
    /// same kind.
    #[must_use]
    pub fn with(mut self, profile: impl KindProfile + 'static) -> Self {
        self.profiles.retain(|held| held.kind() != profile.kind());
        self.profiles.push(Arc::new(profile));
        self
    }

    /// The profile for `kind`, if it is registered.
    #[must_use]
    pub fn get(&self, kind: &str) -> Option<&dyn KindProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.kind() == kind)
            .map(Arc::as_ref)
    }

    /// The registered kinds, in registration order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.profiles.iter().map(|profile| profile.kind()).collect()
    }
}

impl fmt::Debug for Kinds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Kinds").field(&self.names()).finish()
    }
}

/// The inside of each tag in `lower`: the text between a `<` and the
/// next `>`.
fn tags(lower: &str) -> impl Iterator<Item = &str> {
    lower
        .split('<')
        .skip(1)
        .filter_map(|after| after.split('>').next())
}

/// A tag's name: its first word.
fn tag_name(tag: &str) -> &str {
    tag.split(|c: char| c.is_ascii_whitespace() || c == '/')
        .find(|word| !word.is_empty())
        .unwrap_or_default()
}

/// Whether any tag in `lower` is named one of `names`.
fn has_tag(lower: &str, names: &[&str]) -> bool {
    tags(lower).any(|tag| names.contains(&tag_name(tag)))
}

/// Whether any tag in `lower` carries an inline event handler: an
/// attribute named `on` plus letters.
fn has_event_handler(lower: &str) -> bool {
    tags(lower).any(|tag| {
        tag.split(|c: char| c.is_ascii_whitespace() || c == '/')
            .filter(|word| !word.is_empty())
            .skip(1)
            .any(|attribute| {
                let name = attribute.split('=').next().unwrap_or_default();
                name.strip_prefix("on").is_some_and(|event| {
                    !event.is_empty() && event.chars().all(|c| c.is_ascii_lowercase())
                })
            })
    })
}

/// A `document`: Markdown, opening with a level-one heading, with no
/// embedded script.
#[derive(Debug, Clone, Copy)]
pub struct DocumentProfile;

impl KindProfile for DocumentProfile {
    fn kind(&self) -> &'static str {
        "document"
    }

    fn content_type(&self) -> &'static str {
        "text/markdown"
    }

    fn check(&self, _request: &PinnedRequest, content: &str) -> Vec<InspectionFinding> {
        let mut findings = Vec::new();
        let titled = content
            .lines()
            .find(|line| !line.trim().is_empty())
            .and_then(|line| line.strip_prefix("# "))
            .is_some_and(|title| !title.trim().is_empty());
        if !titled {
            findings.push(finding(
                "title",
                "a document opens with a level-one heading",
            ));
        }
        if has_tag(&content.to_ascii_lowercase(), &["script"]) {
            findings.push(finding("script", "the document embeds a script"));
        }
        findings
    }
}

/// A `page`: one static HTML5 page. A doctype, a non-empty title, closed,
/// and nothing active: no script, no inline event handlers, no
/// `javascript:` URLs, no embedded frames or objects.
#[derive(Debug, Clone, Copy)]
pub struct PageProfile;

impl KindProfile for PageProfile {
    fn kind(&self) -> &'static str {
        "page"
    }

    fn content_type(&self) -> &'static str {
        "text/html"
    }

    fn check(&self, _request: &PinnedRequest, content: &str) -> Vec<InspectionFinding> {
        let lower = content.to_ascii_lowercase();
        let mut findings = Vec::new();
        if !lower.trim_start().starts_with("<!doctype html>") {
            findings.push(finding("doctype", "a page opens with <!doctype html>"));
        }
        let titled = lower
            .split_once("<title>")
            .and_then(|(_, rest)| rest.split_once("</title>"))
            .is_some_and(|(title, _)| !title.trim().is_empty());
        if !titled {
            findings.push(finding("title", "a page has a non-empty <title>"));
        }
        if !lower.trim_end().ends_with("</html>") {
            findings.push(finding("closed", "a page ends with </html>"));
        }
        if has_tag(&lower, &["script"]) {
            findings.push(finding("script", "the page runs a script"));
        }
        if has_event_handler(&lower) {
            findings.push(finding("handler", "the page has an inline event handler"));
        }
        if tags(&lower).any(|tag| tag.contains("javascript:")) {
            findings.push(finding(
                "javascript-url",
                "the page links a javascript: URL",
            ));
        }
        if has_tag(&lower, &["iframe", "frame", "object", "embed"]) {
            findings.push(finding("embed", "the page embeds a frame or an object"));
        }
        findings
    }
}

/// A media kind (`video`, …): the media build interface.
///
/// A media build's output is a render manifest, a JSON object:
///
/// - `media_type`: the media type it renders to; must be the profile's.
/// - `sources`: the pinned inputs it renders from, by name; at least
///   one, each pinned in this build, none twice.
/// - `steps`: the render steps, in order; each an object with an `op`.
///
/// Rendering the manifest is a later builder's job. Any media kind is
/// one `MediaProfile`; none needs a pipeline change.
#[derive(Debug, Clone)]
pub struct MediaProfile {
    kind: String,
    media_type: String,
}

impl MediaProfile {
    /// The media kind `kind`, rendering to `media_type`.
    #[must_use]
    pub fn new(kind: &str, media_type: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            media_type: media_type.to_owned(),
        }
    }

    /// The media type the kind renders to.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }
}

impl KindProfile for MediaProfile {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn content_type(&self) -> &str {
        MEDIA_MANIFEST_TYPE
    }

    fn check(&self, request: &PinnedRequest, content: &str) -> Vec<InspectionFinding> {
        let Ok(Value::Object(manifest)) = serde_json::from_str::<Value>(content) else {
            return vec![finding("manifest", "a media build outputs a JSON object")];
        };
        let mut findings = Vec::new();
        if manifest.get("media_type").and_then(Value::as_str) != Some(self.media_type.as_str()) {
            findings.push(finding(
                "media-type",
                &format!("the manifest renders to {}", self.media_type),
            ));
        }
        let sources: Option<Vec<&str>> = manifest
            .get("sources")
            .and_then(Value::as_array)
            .and_then(|sources| sources.iter().map(Value::as_str).collect());
        let pinned = |name: &&str| request.inputs.iter().any(|input| input.name == *name);
        let sources_ok = sources.is_some_and(|sources| {
            !sources.is_empty()
                && sources.iter().all(pinned)
                && sources
                    .iter()
                    .enumerate()
                    .all(|(at, name)| !sources.iter().skip(at + 1).any(|other| other == name))
        });
        if !sources_ok {
            findings.push(finding(
                "sources",
                "the manifest names its sources: pinned inputs, at least one, none twice",
            ));
        }
        let steps_ok = manifest
            .get("steps")
            .and_then(Value::as_array)
            .is_some_and(|steps| {
                steps
                    .iter()
                    .all(|step| step.get("op").and_then(Value::as_str).is_some())
            });
        if !steps_ok {
            findings.push(finding(
                "steps",
                "the manifest lists its steps, each with an op",
            ));
        }
        findings
    }
}

/// A plain text kind with no shape of its own: only the generic checks.
#[derive(Debug, Clone)]
pub struct TextProfile {
    kind: String,
    content_type: String,
}

impl TextProfile {
    /// The kind `kind`, of `content_type`.
    #[must_use]
    pub fn new(kind: &str, content_type: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            content_type: content_type.to_owned(),
        }
    }
}

impl KindProfile for TextProfile {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn content_type(&self) -> &str {
        &self.content_type
    }

    fn check(&self, _request: &PinnedRequest, _content: &str) -> Vec<InspectionFinding> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hatchling_core::ArtifactInput;

    fn request(names: &[&str]) -> PinnedRequest {
        let inputs: Vec<ArtifactInput> = names
            .iter()
            .map(|name| ArtifactInput {
                name: (*name).to_owned(),
                content: String::from("x"),
            })
            .collect();
        PinnedRequest::pin("any", &inputs).unwrap()
    }

    fn checks(inspection: &ArtifactInspection) -> Vec<&str> {
        inspection
            .findings
            .iter()
            .map(|finding| finding.check.as_str())
            .collect()
    }

    const PAGE: &str = "<!doctype html>\n<html lang=\"en\">\n<head>\n<title>Notes</title>\n</head>\n<body>\n<p>Turn the dial on=off; onboarding=done.</p>\n</body>\n</html>\n";

    #[test]
    fn the_generic_checks_run_for_every_kind() {
        let text = TextProfile::new("note", "text/plain");
        let clean = inspect(&text, &request(&[]), "hello\n\tworld\r\n");
        assert_eq!(clean.verdict, InspectionVerdict::Pass);
        assert!(clean.findings.is_empty());
        assert_eq!(clean.content_type, "text/plain");
        assert_eq!(checks(&inspect(&text, &request(&[]), " \n ")), ["empty"]);
        assert_eq!(
            checks(&inspect(&text, &request(&[]), "a\u{0}b")),
            ["control"]
        );
        assert_eq!(
            checks(&inspect(&text, &request(&[]), "a\u{1b}[31mb")),
            ["control"]
        );
        let at_bound = "x".repeat(MAX_SKILL_OUTPUT_BYTES);
        assert!(inspect(&text, &request(&[]), &at_bound).findings.is_empty());
        let over = inspect(&text, &request(&[]), &format!("{at_bound}x"));
        assert_eq!(checks(&over), ["size"]);
        assert_eq!(over.verdict, InspectionVerdict::Fail);
        assert_eq!(
            over.findings.first().map(|f| f.detail.as_str()),
            Some("the output is 8193 bytes, over the kind's 8192")
        );
    }

    #[test]
    fn findings_never_quote_the_content() {
        let canary = "canary-7f3e9";
        let video = MediaProfile::new("video", "video/mp4");
        let cases: [(&dyn KindProfile, String); 3] = [
            (
                &DocumentProfile,
                format!("{canary}\n<script>{canary}</script>"),
            ),
            (&PageProfile, format!("<p onclick=\"{canary}\">{canary}")),
            (
                &video,
                format!(r#"{{"media_type":"{canary}","sources":["{canary}"],"steps":[{{}}]}}"#),
            ),
        ];
        for (profile, content) in cases {
            let inspection = inspect(profile, &request(&[]), &content);
            assert_eq!(inspection.verdict, InspectionVerdict::Fail);
            for finding in &inspection.findings {
                assert!(
                    !finding.detail.contains(canary) && !finding.check.contains(canary),
                    "{finding:?}"
                );
            }
        }
    }

    #[test]
    fn a_document_opens_with_a_title_and_embeds_no_script() {
        let good = inspect(
            &DocumentProfile,
            &request(&[]),
            "\n# Weekly report\n\nAll quiet. <b>bold</b> is fine.\n",
        );
        assert_eq!(good.verdict, InspectionVerdict::Pass, "{good:?}");
        assert_eq!(good.content_type, "text/markdown");
        for untitled in [
            "Weekly report\n",
            "## Weekly report\n",
            "#Weekly report\n",
            "# \n",
            "#   \nbody",
        ] {
            assert_eq!(
                checks(&inspect(&DocumentProfile, &request(&[]), untitled)),
                ["title"],
                "{untitled:?}"
            );
        }
        assert_eq!(
            checks(&inspect(
                &DocumentProfile,
                &request(&[]),
                "# T\n<SCRIPT src=x></SCRIPT>"
            )),
            ["script"]
        );
        // Prose about scripts is not a script.
        assert!(
            inspect(
                &DocumentProfile,
                &request(&[]),
                "# T\nThe script runs nightly."
            )
            .findings
            .is_empty()
        );
    }

    #[test]
    fn a_page_is_one_static_html_document() {
        let good = inspect(&PageProfile, &request(&[]), PAGE);
        assert_eq!(good.verdict, InspectionVerdict::Pass, "{good:?}");
        assert_eq!(good.content_type, "text/html");
        assert!(
            inspect(&PageProfile, &request(&[]), &PAGE.to_ascii_uppercase())
                .findings
                .is_empty(),
            "case-insensitive"
        );
        let swap =
            |from: &str, to: &str| inspect(&PageProfile, &request(&[]), &PAGE.replace(from, to));
        assert_eq!(checks(&swap("<!doctype html>\n", "")), ["doctype"]);
        assert_eq!(checks(&swap("<title>Notes</title>", "")), ["title"]);
        assert_eq!(
            checks(&swap("<title>Notes</title>", "<title> </title>")),
            ["title"]
        );
        assert_eq!(
            checks(&swap("<title>Notes</title>", "<title>Notes")),
            ["title"]
        );
        assert_eq!(checks(&swap("</html>\n", "")), ["closed"]);
        assert_eq!(checks(&swap("<p>", "<script>go()</script><p>")), ["script"]);
        assert_eq!(checks(&swap("<p>", "<p onclick=\"go()\">")), ["handler"]);
        assert_eq!(checks(&swap("<p>", "<body/onload=go()><p>")), ["handler"]);
        assert_eq!(
            checks(&swap("<p>", "<img src=x onerror=go()><p>")),
            ["handler"]
        );
        assert!(
            swap("<p>", "<p on=\"2\" title=\"onboarding notes\">")
                .findings
                .is_empty()
        );
        assert_eq!(
            checks(&swap("<p>", "<a href=\"javascript:go()\">x</a><p>")),
            ["javascript-url"]
        );
        for embed in [
            "<iframe src=x>",
            "<frame src=x>",
            "<object data=x>",
            "<embed src=x>",
        ] {
            assert_eq!(checks(&swap("<p>", embed)), ["embed"], "{embed}");
        }
        // Words in the text are not tags.
        assert!(
            swap("Notes</title>", "Notes</title><!-- iframe -->")
                .findings
                .is_empty()
        );
        let everything = inspect(&PageProfile, &request(&[]), "<p onclick=x><script>");
        assert_eq!(
            checks(&everything),
            ["doctype", "title", "closed", "script", "handler"]
        );
    }

    #[test]
    fn a_media_build_outputs_a_render_manifest_over_its_pinned_sources() {
        let video = MediaProfile::new("video", "video/mp4");
        assert_eq!((video.kind(), video.media_type()), ("video", "video/mp4"));
        let pinned = request(&["footage-a", "footage-b"]);
        let good = r#"{"media_type":"video/mp4","sources":["footage-a","footage-b"],"steps":[{"op":"trim","from_ms":0,"to_ms":900},{"op":"concat"}]}"#;
        let inspection = inspect(&video, &pinned, good);
        assert_eq!(
            inspection.verdict,
            InspectionVerdict::Pass,
            "{inspection:?}"
        );
        assert_eq!(inspection.content_type, MEDIA_MANIFEST_TYPE);
        let no_steps = r#"{"media_type":"video/mp4","sources":["footage-a"],"steps":[]}"#;
        assert!(inspect(&video, &pinned, no_steps).findings.is_empty());
        for (bad, expected) in [
            ("[1]", vec!["manifest"]),
            ("not json", vec!["manifest"]),
            (
                r#"{"media_type":"audio/ogg","sources":["footage-a"],"steps":[]}"#,
                vec!["media-type"],
            ),
            (
                r#"{"sources":["footage-a"],"steps":[]}"#,
                vec!["media-type"],
            ),
            (
                r#"{"media_type":"video/mp4","sources":[],"steps":[]}"#,
                vec!["sources"],
            ),
            (
                r#"{"media_type":"video/mp4","sources":["elsewhere"],"steps":[]}"#,
                vec!["sources"],
            ),
            (
                r#"{"media_type":"video/mp4","sources":["footage-a","footage-a"],"steps":[]}"#,
                vec!["sources"],
            ),
            (
                r#"{"media_type":"video/mp4","sources":["footage-a",3],"steps":[]}"#,
                vec!["sources"],
            ),
            (r#"{"media_type":"video/mp4","steps":[]}"#, vec!["sources"]),
            (
                r#"{"media_type":"video/mp4","sources":["footage-a"],"steps":[{"op":"trim"},{"to":1}]}"#,
                vec!["steps"],
            ),
            (
                r#"{"media_type":"video/mp4","sources":["footage-a"],"steps":[{"op":7}]}"#,
                vec!["steps"],
            ),
            (
                r#"{"media_type":"video/mp4","sources":["footage-a"]}"#,
                vec!["steps"],
            ),
        ] {
            assert_eq!(checks(&inspect(&video, &pinned, bad)), expected, "{bad}");
        }
    }

    #[test]
    fn the_registry_admits_exactly_its_profiles() {
        let standard = Kinds::standard();
        assert_eq!(standard.names(), ["document", "page", "video"]);
        assert_eq!(
            standard.get("page").map(KindProfile::content_type),
            Some("text/html")
        );
        assert_eq!(
            standard.get("video").map(KindProfile::content_type),
            Some(MEDIA_MANIFEST_TYPE)
        );
        assert!(standard.get("audio-clip").is_none());
        assert!(Kinds::empty().get("document").is_none());
        assert!(Kinds::empty().names().is_empty());
        // A new kind is one more profile; one for a held kind replaces it.
        let extended = standard
            .with(MediaProfile::new("audio-clip", "audio/ogg"))
            .with(TextProfile::new("page", "text/plain"));
        assert_eq!(
            extended.names(),
            ["document", "video", "audio-clip", "page"]
        );
        assert_eq!(
            extended.get("page").map(KindProfile::content_type),
            Some("text/plain")
        );
        assert_eq!(
            format!("{extended:?}"),
            r#"Kinds(["document", "video", "audio-clip", "page"])"#
        );
        assert_eq!(TextProfile::new("t", "text/plain").max_bytes(), 8192);
    }
}
