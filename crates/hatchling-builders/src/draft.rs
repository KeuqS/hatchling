//! A build request's inputs, read as a draft: a title and sections.
//!
//! Both builders read their inputs the same way:
//!
//! - **The title** is the `title` input if there is one. Otherwise it is
//!   the first non-blank line of the first input, without any leading
//!   `#`s, and that line leaves the input's body.
//! - **The sections** are the other inputs, in request order. An input
//!   named `body` or `idea`, or the first input when the title came from
//!   it, is untitled; any other input is headed by its name. An empty
//!   section is left out.
//!
//! Text is normalized: line endings become `\n`, trailing whitespace
//! goes, leading and trailing blank lines go, and runs of blank lines
//! become one. A title is one line, without control characters, at most
//! [`MAX_TITLE_CHARS`].

use serde::Deserialize;

/// Longest title, in characters; a longer one is cut.
pub const MAX_TITLE_CHARS: usize = 120;

/// One pinned input, as a builder receives it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Input {
    /// The input's name.
    pub name: String,
    /// Its content.
    pub content: String,
}

/// The pinned request, as a builder receives it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Request {
    /// The build kind.
    pub kind: String,
    /// The pinned inputs, in request order.
    pub inputs: Vec<Input>,
}

/// One section of a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Its heading, if it has one.
    pub heading: Option<String>,
    /// Its normalized text.
    pub body: String,
}

/// A request's inputs, read as a title and sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The title.
    pub title: String,
    /// The sections, in request order, empty ones left out.
    pub sections: Vec<Section>,
}

/// Why a request cannot be read as a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftError {
    /// The request has no inputs.
    NoInputs,
    /// No title can be found.
    NoTitle,
}

/// `text` with `\n` line endings, trailing whitespace removed, leading
/// and trailing blank lines removed, and runs of blank lines made one.
#[must_use]
pub fn normalize(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = Vec::new();
    for line in unified.lines().map(str::trim_end) {
        let after_blank = lines.last().is_none_or(|last| last.is_empty());
        if !(line.is_empty() && after_blank) {
            lines.push(line);
        }
    }
    if lines.last().is_some_and(|last| last.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// `raw` as a title: one line of words separated by single spaces, no
/// control characters, at most [`MAX_TITLE_CHARS`].
#[must_use]
pub fn clean_title(raw: &str) -> String {
    let words: Vec<String> = raw
        .split_whitespace()
        .map(|word| word.chars().filter(|c| !c.is_control()).collect())
        .filter(|word: &String| !word.is_empty())
        .collect();
    words.join(" ").chars().take(MAX_TITLE_CHARS).collect()
}

/// Whether an input named `name` is an untitled section.
fn untitled(name: &str) -> bool {
    matches!(name, "body" | "idea")
}

impl Draft {
    /// Reads `request`'s inputs as a draft.
    ///
    /// # Errors
    ///
    /// [`DraftError::NoInputs`] for a request with none, and
    /// [`DraftError::NoTitle`] when no title can be found.
    pub fn read(request: &Request) -> Result<Self, DraftError> {
        let first = request.inputs.first().ok_or(DraftError::NoInputs)?;
        let named_title = request.inputs.iter().find(|input| input.name == "title");
        let (title, first_rest) = named_title.map_or_else(
            || {
                let text = normalize(&first.content);
                let (line, rest) = text.split_once('\n').unwrap_or((&text, ""));
                (
                    clean_title(line.trim_start_matches('#')),
                    Some(rest.to_owned()),
                )
            },
            |input| (clean_title(&input.content), None),
        );
        if title.is_empty() {
            return Err(DraftError::NoTitle);
        }
        let mut sections = Vec::new();
        for (at, input) in request.inputs.iter().enumerate() {
            let (heading, body) = match (&first_rest, at) {
                (Some(rest), 0) => (None, normalize(rest)),
                _ if input.name == "title" => continue,
                _ => (
                    (!untitled(&input.name)).then(|| input.name.clone()),
                    normalize(&input.content),
                ),
            };
            if !body.is_empty() {
                sections.push(Section { heading, body });
            }
        }
        Ok(Self { title, sections })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(inputs: &[(&str, &str)]) -> Request {
        Request {
            kind: String::from("document"),
            inputs: inputs
                .iter()
                .map(|(name, content)| Input {
                    name: (*name).to_owned(),
                    content: (*content).to_owned(),
                })
                .collect(),
        }
    }

    fn section(heading: Option<&str>, body: &str) -> Section {
        Section {
            heading: heading.map(str::to_owned),
            body: body.to_owned(),
        }
    }

    #[test]
    fn text_is_normalized() {
        assert_eq!(
            normalize("\n\n a  \r\nb\t\r\rc\n\n\n\nd\n\n"),
            " a\nb\n\nc\n\nd"
        );
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("\n \n\t\n"), "");
        assert_eq!(normalize("one"), "one");
    }

    #[test]
    fn a_title_is_one_clean_bounded_line() {
        assert_eq!(clean_title("  Weekly \t report\n"), "Weekly report");
        assert_eq!(clean_title("a\u{7}b \u{1b} c"), "ab c");
        assert_eq!(clean_title(" \u{0} "), "");
        let long = "x".repeat(MAX_TITLE_CHARS + 5);
        assert_eq!(clean_title(&long).chars().count(), MAX_TITLE_CHARS);
        assert_eq!(clean_title(&"é".repeat(130)), "é".repeat(MAX_TITLE_CHARS));
        assert_eq!(MAX_TITLE_CHARS, 120);
    }

    #[test]
    fn a_title_input_heads_the_draft_and_other_inputs_are_sections() {
        let draft = Draft::read(&request(&[
            ("body", "All quiet.\r\n"),
            ("title", " Weekly report "),
            ("risks", "\nNone.\n\n"),
            ("idea", "Ship it."),
            ("empty", " \n"),
            ("body", ""),
        ]))
        .unwrap();
        assert_eq!(draft.title, "Weekly report");
        assert_eq!(
            draft.sections,
            [
                section(None, "All quiet."),
                section(Some("risks"), "None."),
                section(None, "Ship it."),
            ]
        );
    }

    #[test]
    fn without_a_title_input_the_first_line_is_the_title() {
        let draft = Draft::read(&request(&[
            ("notes", "\n## Launch plan\n\nStep one.\n"),
            ("budget", "Small."),
        ]))
        .unwrap();
        assert_eq!(draft.title, "Launch plan");
        assert_eq!(
            draft.sections,
            [
                section(None, "Step one."),
                section(Some("budget"), "Small.")
            ]
        );
        let single = Draft::read(&request(&[("idea", "Just a title")])).unwrap();
        assert_eq!(single.title, "Just a title");
        assert!(single.sections.is_empty());
    }

    #[test]
    fn a_draft_needs_inputs_and_a_title() {
        assert_eq!(Draft::read(&request(&[])), Err(DraftError::NoInputs));
        for no_title in [
            vec![("title", " \t "), ("body", "text")],
            vec![("body", "###\ntext")],
            vec![("body", "\n\n")],
        ] {
            assert_eq!(
                Draft::read(&request(&no_title)),
                Err(DraftError::NoTitle),
                "{no_title:?}"
            );
        }
    }
}
