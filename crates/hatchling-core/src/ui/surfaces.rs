//! The interactive surfaces: the agent's status, artifacts and
//! files, and the live browser.
//!
//! Everything a surface shows is data. Artifact content arrives as text
//! or as an inline image, never as something to execute: an HTML artifact
//! is its source text. Images are inline bytes of a named image type, so
//! showing one never fetches anything. Titles, narration and page titles
//! are untrusted text.

use serde::{Deserialize, Serialize};

use super::control::Actor;

/// What the agent is doing, as the avatar and the status line show it.
/// The UI shows exactly the state the broker last reported, never one it
/// infers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// Nothing is running.
    Idle,
    /// Working on a task.
    Working,
    /// Waiting for the user to approve or deny something.
    AwaitingApproval,
    /// The user paused the work.
    Paused,
    /// Waiting on something outside the agent: a source, a service.
    Stalled,
}

/// The kinds of file a surface can show.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    /// Markdown text.
    Markdown,
    /// Plain text.
    Text,
    /// Comma-separated values.
    Csv,
    /// Source code.
    Code,
    /// HTML, shown as its source text, never rendered.
    Html,
    /// An image.
    Image,
    /// A PDF.
    Pdf,
    /// A word-processor document.
    Document,
    /// A spreadsheet.
    Spreadsheet,
}

/// The image types a surface shows inline.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ImageType {
    /// `image/png`.
    Png,
    /// `image/jpeg`.
    Jpeg,
    /// `image/webp`.
    Webp,
    /// `image/gif`.
    Gif,
}

/// An image carried inline, so showing it fetches nothing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct InlineImage {
    /// Its type.
    pub media_type: ImageType,
    /// Its bytes, standard base64.
    pub base64: String,
}

/// One artifact as the UI lists it: every revision kept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct UiArtifact {
    /// Stable across revisions.
    pub artifact_id: String,
    /// Its title. Untrusted text.
    pub title: String,
    /// What kind of file it is.
    pub format: FileFormat,
    /// Its revisions, oldest first. Never empty.
    pub revisions: Vec<ArtifactRevision>,
    /// The revision last shared, if one was.
    pub shared_revision: Option<u32>,
}

/// One revision of an artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ArtifactRevision {
    /// From 1, increasing.
    pub revision: u32,
    /// The build that produced it.
    pub build_id: String,
    /// When, RFC 3339.
    pub created_at: String,
    /// Its size.
    pub bytes: u64,
    /// Who built it, as the broker can vouch for now, if it can.
    pub attribution: Option<String>,
}

/// An artifact revision's content, as a surface may show it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArtifactBody {
    /// Text: markdown, plain text, CSV, code, or HTML source. Untrusted.
    Text {
        /// The text.
        text: String,
    },
    /// An image.
    Image(InlineImage),
    /// A file that has no inline preview (a PDF, a document, a
    /// spreadsheet).
    Unpreviewable {
        /// Its size.
        bytes: u64,
    },
    /// Too large to send to the UI.
    TooLarge {
        /// Its size.
        bytes: u64,
        /// The most the broker sends.
        limit: u64,
    },
}

/// A file the user adds from their device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileContent {
    /// Text.
    Text {
        /// The text.
        text: String,
    },
    /// An image.
    Image(InlineImage),
}

/// Why an artifact action found nothing to act on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRefusal {
    /// No artifact has that id.
    UnknownArtifact,
    /// The artifact has no such revision.
    UnknownRevision,
}

/// Why a file was not saved.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum FileRefusal {
    /// Larger than the broker accepts.
    TooLarge {
        /// The most it accepts.
        limit: u64,
    },
    /// Not a kind of file the broker accepts.
    UnsupportedType,
    /// The store already holds the most it keeps.
    Full,
}

/// A browser session's state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BrowserState {
    /// Waiting for the browser.
    Queued,
    /// Running.
    Running,
    /// Waiting for the user before it goes on.
    Parked,
    /// Finished.
    Completed,
    /// Failed.
    Failed,
    /// Stopped.
    Cancelled,
}

/// A browser session, as the UI shows it. Its control state is not here:
/// it moves only by handoff events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BrowserSession {
    /// Stable for the session.
    pub session_id: String,
    /// The chat that asked for it, if one did.
    pub chat_id: Option<String>,
    /// What it is for. Untrusted text.
    pub title: String,
    /// The site, as a host name. Untrusted text.
    pub site: String,
    /// Where it stands.
    pub state: BrowserState,
    /// What is happening now, in the user's words. Untrusted text.
    pub step: String,
    /// The page state last confirmed. Untrusted text.
    pub last_confirmed: String,
}

/// A snapshot of the page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BrowserFrame {
    /// The session.
    pub session_id: String,
    /// Increasing within the session.
    pub sequence: u64,
    /// When it was captured, RFC 3339.
    pub captured_at: String,
    /// The page's title. Untrusted text.
    pub title: String,
    /// The picture of the page, if there is one.
    pub image: Option<InlineImage>,
}

/// One action on the page, for the action log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BrowserAction {
    /// The session.
    pub session_id: String,
    /// Who acted. The UI shows it only if they were driving.
    pub by: Actor,
    /// What they did, in the user's words. Untrusted text.
    pub narration: String,
}

/// Why a control action was refused.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ControlRefusal {
    /// No session has that id.
    UnknownSession,
    /// The session has finished.
    Finished,
    /// Control is already changing hands.
    Switching,
    /// The handoff is not allowed from the current state.
    NotAllowed,
    /// The frame the user returned control from is not the latest.
    StaleFrame,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn wire<T: Serialize>(value: &T) -> Value {
        serde_json::to_value(value).unwrap()
    }

    #[test]
    fn agent_states_and_file_formats_are_pinned() {
        let states = [
            AgentState::Idle,
            AgentState::Working,
            AgentState::AwaitingApproval,
            AgentState::Paused,
            AgentState::Stalled,
        ];
        assert_eq!(
            wire(&states),
            json!(["idle", "working", "awaiting_approval", "paused", "stalled"])
        );
        let formats = [
            FileFormat::Markdown,
            FileFormat::Text,
            FileFormat::Csv,
            FileFormat::Code,
            FileFormat::Html,
            FileFormat::Image,
            FileFormat::Pdf,
            FileFormat::Document,
            FileFormat::Spreadsheet,
        ];
        assert_eq!(
            wire(&formats),
            json!([
                "markdown",
                "text",
                "csv",
                "code",
                "html",
                "image",
                "pdf",
                "document",
                "spreadsheet"
            ])
        );
        assert_eq!(
            wire(&[
                ImageType::Png,
                ImageType::Jpeg,
                ImageType::Webp,
                ImageType::Gif
            ]),
            json!(["png", "jpeg", "webp", "gif"])
        );
    }

    #[test]
    fn artifact_bodies_are_data_never_markup_to_run() {
        let image = InlineImage {
            media_type: ImageType::Png,
            base64: String::from("iVBORw0KGgo="),
        };
        assert_eq!(
            wire(&ArtifactBody::Text {
                text: String::from("# Notes")
            }),
            json!({ "type": "text", "text": "# Notes" })
        );
        assert_eq!(
            wire(&ArtifactBody::Image(image.clone())),
            json!({ "type": "image", "media_type": "png", "base64": "iVBORw0KGgo=" })
        );
        assert_eq!(
            wire(&ArtifactBody::Unpreviewable { bytes: 9 }),
            json!({ "type": "unpreviewable", "bytes": 9 })
        );
        assert_eq!(
            wire(&ArtifactBody::TooLarge { bytes: 9, limit: 4 }),
            json!({ "type": "too_large", "bytes": 9, "limit": 4 })
        );
        assert_eq!(
            wire(&FileContent::Image(image)),
            json!({ "type": "image", "media_type": "png", "base64": "iVBORw0KGgo=" })
        );
        assert_eq!(
            wire(&FileContent::Text {
                text: String::from("x")
            }),
            json!({ "type": "text", "text": "x" })
        );
    }

    #[test]
    fn refusals_are_pinned() {
        assert_eq!(
            wire(&[
                ArtifactRefusal::UnknownArtifact,
                ArtifactRefusal::UnknownRevision
            ]),
            json!(["unknown_artifact", "unknown_revision"])
        );
        assert_eq!(
            wire(&FileRefusal::TooLarge { limit: 10 }),
            json!({ "refusal": "too_large", "limit": 10 })
        );
        assert_eq!(
            wire(&[FileRefusal::UnsupportedType, FileRefusal::Full]),
            json!([{ "refusal": "unsupported_type" }, { "refusal": "full" }])
        );
        let refusals = [
            ControlRefusal::UnknownSession,
            ControlRefusal::Finished,
            ControlRefusal::Switching,
            ControlRefusal::NotAllowed,
            ControlRefusal::StaleFrame,
        ];
        assert_eq!(
            wire(&refusals),
            json!([
                "unknown_session",
                "finished",
                "switching",
                "not_allowed",
                "stale_frame"
            ])
        );
        let states = [
            BrowserState::Queued,
            BrowserState::Running,
            BrowserState::Parked,
            BrowserState::Completed,
            BrowserState::Failed,
            BrowserState::Cancelled,
        ];
        assert_eq!(
            wire(&states),
            json!([
                "queued",
                "running",
                "parked",
                "completed",
                "failed",
                "cancelled"
            ])
        );
    }
}
