//! `hatchling-builders`: Hatchling's document and page builders
//! .
//!
//! Each builder is a skill: a signed package whose manifest declares the
//! `artifact.build` capability, run by the broker in its own skill cell
//! through the pipeline. It reads the pinned build request as
//! JSON on stdin and writes a builder result on stdout:
//!
//! - `hatchling-build-document` builds the `document` kind: Markdown.
//! - `hatchling-build-page` builds the `page` kind: one static HTML5
//!   page with every piece of input text escaped.
//!
//! A builder only *claims*: the broker hashes the output, inspects it
//! against the kind's profile, and gates any delivery itself. So a
//! builder's result is just the content, a summary that names no input
//! text, and the claim that it is deterministic, which it is: the same
//! pinned request always gives the same bytes.
//!
//! A builder that cannot build exits non-zero with nothing on stdout,
//! and the broker records the build as failed.

pub mod draft;
pub mod render;

use hatchling_core::MAX_SKILL_OUTPUT_BYTES;
use serde_json::json;

use crate::draft::{Draft, DraftError, Request};

/// Why a builder could not build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The request is not a build request for this builder's kind.
    BadRequest,
    /// The request has no inputs, or no title can be found in them.
    Unreadable(DraftError),
    /// The result would not fit in a skill's output.
    TooLarge,
}

impl Failure {
    /// The builder's exit status for this failure.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::BadRequest => 2,
            Self::Unreadable(_) => 3,
            Self::TooLarge => 4,
        }
    }
}

/// One builder: the kind it builds and how it renders a draft.
#[derive(Debug, Clone, Copy)]
pub struct Builder {
    /// The kind it builds.
    pub kind: &'static str,
    /// How it renders a draft.
    pub render: fn(&Draft) -> String,
}

/// The document builder.
pub const DOCUMENT: Builder = Builder {
    kind: "document",
    render: render::document,
};

/// The page builder.
pub const PAGE: Builder = Builder {
    kind: "page",
    render: render::page,
};

impl Builder {
    /// Builds from `raw`, the pinned request as the broker sends it, and
    /// returns the builder result to write.
    ///
    /// # Errors
    ///
    /// Returns why it could not build.
    pub fn run(self, raw: &str) -> Result<String, Failure> {
        let request: Request = serde_json::from_str(raw).map_err(|_| Failure::BadRequest)?;
        if request.kind != self.kind {
            return Err(Failure::BadRequest);
        }
        let draft = Draft::read(&request).map_err(Failure::Unreadable)?;
        let content = (self.render)(&draft);
        let sections = draft.sections.len();
        let result = json!({
            "content": content,
            "summary": format!(
                "a {} with {sections} section{}",
                self.kind,
                if sections == 1 { "" } else { "s" }
            ),
            "deterministic": true,
        })
        .to_string();
        if result.len() > MAX_SKILL_OUTPUT_BYTES {
            return Err(Failure::TooLarge);
        }
        Ok(result)
    }
}

/// Runs `builder` as a skill process: the request on stdin, the result
/// on stdout. Returns the exit status.
#[must_use]
pub fn main_for(builder: Builder) -> std::process::ExitCode {
    use std::io::{Read as _, Write as _};
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return std::process::ExitCode::from(Failure::BadRequest.exit_code());
    }
    let result = match builder.run(&raw) {
        Ok(result) => result,
        Err(failure) => return std::process::ExitCode::from(failure.exit_code()),
    };
    let mut out = std::io::stdout().lock();
    if out
        .write_all(result.as_bytes())
        .and_then(|()| out.flush())
        .is_err()
    {
        return std::process::ExitCode::from(Failure::BadRequest.exit_code());
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn request(kind: &str, inputs: &[(&str, &str)]) -> String {
        let inputs: Vec<Value> = inputs
            .iter()
            .map(|(name, content)| json!({ "name": name, "sha256": "00", "content": content }))
            .collect();
        json!({ "kind": kind, "inputs": inputs }).to_string()
    }

    fn result(raw: &str) -> Value {
        serde_json::from_str(raw).unwrap()
    }

    #[test]
    fn the_document_builder_writes_a_strict_builder_result() {
        let out = DOCUMENT
            .run(&request("document", &[("title", "Plan"), ("body", "Go.")]))
            .unwrap();
        assert_eq!(
            result(&out),
            json!({
                "content": "# Plan\n\nGo.\n",
                "summary": "a document with 1 section",
                "deterministic": true,
            })
        );
        // Only the fields a builder result may carry.
        let fields: Vec<String> = result(&out).as_object().unwrap().keys().cloned().collect();
        assert_eq!(fields, ["content", "deterministic", "summary"]);
    }

    #[test]
    fn the_page_builder_builds_pages_and_counts_sections() {
        let out = PAGE
            .run(&request(
                "page",
                &[("title", "Plan"), ("body", "Go."), ("risks", "None.")],
            ))
            .unwrap();
        let content = result(&out)
            .get("content")
            .and_then(Value::as_str)
            .unwrap()
            .to_owned();
        assert!(content.starts_with("<!doctype html>"));
        assert!(content.contains("<h2>risks</h2>\n<p>None.</p>"));
        assert_eq!(
            result(&out).get("summary").and_then(Value::as_str),
            Some("a page with 2 sections")
        );
        let none = PAGE.run(&request("page", &[("title", "Plan")])).unwrap();
        assert_eq!(
            result(&none).get("summary").and_then(Value::as_str),
            Some("a page with 0 sections")
        );
    }

    #[test]
    fn the_same_request_always_gives_the_same_bytes() {
        let raw = request("page", &[("notes", "# A\n\nb\n- c")]);
        assert_eq!(PAGE.run(&raw), PAGE.run(&raw));
    }

    #[test]
    fn a_builder_refuses_what_it_cannot_build() {
        assert_eq!(DOCUMENT.run("not json"), Err(Failure::BadRequest));
        assert_eq!(
            DOCUMENT.run(r#"{"kind":"document"}"#),
            Err(Failure::BadRequest)
        );
        // Each builder builds only its own kind.
        assert_eq!(
            DOCUMENT.run(&request("page", &[("title", "x")])),
            Err(Failure::BadRequest)
        );
        assert_eq!(
            PAGE.run(&request("document", &[("title", "x")])),
            Err(Failure::BadRequest)
        );
        assert_eq!(
            DOCUMENT.run(&request("document", &[])),
            Err(Failure::Unreadable(DraftError::NoInputs))
        );
        assert_eq!(
            DOCUMENT.run(&request("document", &[("body", "\n")])),
            Err(Failure::Unreadable(DraftError::NoTitle))
        );
    }

    #[test]
    fn a_result_too_large_for_a_skill_output_is_refused() {
        // Escaping grows a page: every `<` is four bytes.
        let body = "<".repeat(MAX_SKILL_OUTPUT_BYTES / 4);
        assert_eq!(
            PAGE.run(&request("page", &[("title", "t"), ("body", &body)])),
            Err(Failure::TooLarge)
        );
        // The bound itself: find the largest body that fits, and check
        // one more byte does not.
        let fits = |n: usize| {
            DOCUMENT
                .run(&request(
                    "document",
                    &[("title", "t"), ("body", &"x".repeat(n))],
                ))
                .map(|out| out.len())
        };
        let overhead = fits(1).unwrap() - 1;
        let largest = MAX_SKILL_OUTPUT_BYTES - overhead;
        assert_eq!(fits(largest), Ok(MAX_SKILL_OUTPUT_BYTES));
        assert_eq!(fits(largest + 1), Err(Failure::TooLarge));
    }

    #[test]
    fn each_failure_has_its_own_exit_status() {
        assert_eq!(Failure::BadRequest.exit_code(), 2);
        assert_eq!(Failure::Unreadable(DraftError::NoTitle).exit_code(), 3);
        assert_eq!(Failure::TooLarge.exit_code(), 4);
        assert_eq!((DOCUMENT.kind, PAGE.kind), ("document", "page"));
    }
}
