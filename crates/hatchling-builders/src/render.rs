//! Rendering a draft: a Markdown document, or a static HTML page.
//!
//! **A document** is the title as a level-one heading, then each section,
//! headed sections under a level-two heading. Section text is kept as
//! the author wrote it.
//!
//! **A page** is one static HTML5 document. Every piece of input text is
//! escaped, and the page holds no script, no event handler, and nothing
//! embedded: section text is read as plain blocks (paragraphs, `-`/`*`
//! list items, `#` headings, and ```` ``` ```` fenced code), and nothing in
//! it becomes markup of its own.

use std::fmt::Write as _;

use crate::draft::{Draft, Section};

/// The page's stylesheet: static, and the same for every page.
const STYLE: &str = "body{font:16px/1.6 system-ui,sans-serif;max-width:42rem;margin:2rem auto;padding:0 1rem;color:#1d1d1f;background:#fff}pre{overflow-x:auto;padding:.75rem;background:#f4f4f5}@media (prefers-color-scheme:dark){body{color:#e8e8ea;background:#18181b}pre{background:#27272a}}";

/// `draft` as a Markdown document.
#[must_use]
pub fn document(draft: &Draft) -> String {
    let mut out = format!("# {}\n", draft.title);
    for Section { heading, body } in &draft.sections {
        out.push('\n');
        if let Some(heading) = heading {
            let _ = write!(out, "## {heading}\n\n");
        }
        out.push_str(body);
        out.push('\n');
    }
    out
}

/// `text` with every HTML-significant character escaped.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// The blocks of section text as they are being read.
#[derive(Default)]
struct Blocks {
    out: String,
    paragraph: Vec<String>,
    list: Vec<String>,
    code: Option<Vec<String>>,
}

impl Blocks {
    fn flush(&mut self) {
        if !self.paragraph.is_empty() {
            let _ = writeln!(self.out, "<p>{}</p>", self.paragraph.join("\n"));
            self.paragraph.clear();
        }
        if !self.list.is_empty() {
            self.out.push_str("<ul>\n");
            for item in self.list.drain(..) {
                let _ = writeln!(self.out, "<li>{item}</li>");
            }
            self.out.push_str("</ul>\n");
        }
    }

    fn close_code(&mut self) {
        if let Some(lines) = self.code.take() {
            let _ = writeln!(self.out, "<pre><code>{}</code></pre>", lines.join("\n"));
        }
    }

    fn line(&mut self, line: &str) {
        let fence = line.trim_start().starts_with("```");
        if let Some(code) = self.code.as_mut() {
            if fence {
                self.close_code();
            } else {
                code.push(escape(line));
            }
            return;
        }
        if fence {
            self.flush();
            self.code = Some(Vec::new());
        } else if line.trim().is_empty() {
            self.flush();
        } else if let Some((level, text)) = heading(line) {
            self.flush();
            let _ = writeln!(self.out, "<h{level}>{}</h{level}>", escape(text));
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            if !self.paragraph.is_empty() {
                self.flush();
            }
            self.list.push(escape(item.trim()));
        } else {
            if !self.list.is_empty() {
                self.flush();
            }
            self.paragraph.push(escape(line.trim()));
        }
    }

    fn finish(mut self) -> String {
        self.flush();
        self.close_code();
        self.out
    }
}

/// A `#` heading line in section text, as a page heading level (the
/// title is the only `h1`, so `#` is `h2`, down to `###` as `h4`) and its
/// text.
fn heading(line: &str) -> Option<(u8, &str)> {
    [("### ", 4), ("## ", 3), ("# ", 2)]
        .into_iter()
        .find_map(|(marker, level)| line.strip_prefix(marker).map(|text| (level, text.trim())))
        .filter(|(_, text)| !text.is_empty())
}

/// `draft` as a static HTML5 page.
#[must_use]
pub fn page(draft: &Draft) -> String {
    let title = escape(&draft.title);
    let mut out = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n<h1>{title}</h1>\n"
    );
    for Section { heading, body } in &draft.sections {
        if let Some(heading) = heading {
            let _ = writeln!(out, "<h2>{}</h2>", escape(heading));
        }
        let mut blocks = Blocks::default();
        for line in body.lines() {
            blocks.line(line);
        }
        out.push_str(&blocks.finish());
    }
    out.push_str("</main>\n</body>\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(sections: &[(Option<&str>, &str)]) -> Draft {
        Draft {
            title: String::from("Weekly <report>"),
            sections: sections
                .iter()
                .map(|(heading, body)| Section {
                    heading: heading.map(str::to_owned),
                    body: (*body).to_owned(),
                })
                .collect(),
        }
    }

    /// The part of a page between `<main>` and `</main>`.
    fn main_of(page: &str) -> String {
        page.split_once("<main>\n")
            .and_then(|(_, rest)| rest.split_once("</main>"))
            .map(|(main, _)| main.to_owned())
            .unwrap_or_default()
    }

    #[test]
    fn a_document_is_the_title_then_its_sections() {
        assert_eq!(
            document(&draft(&[
                (None, "All quiet."),
                (Some("risks"), "None.\n\n- one")
            ])),
            "# Weekly <report>\n\nAll quiet.\n\n## risks\n\nNone.\n\n- one\n"
        );
        assert_eq!(document(&draft(&[])), "# Weekly <report>\n");
    }

    #[test]
    fn every_html_significant_character_is_escaped() {
        assert_eq!(
            escape(r#"<a href="x" title='y'>&amp;</a> plain"#),
            "&lt;a href=&quot;x&quot; title=&#39;y&#39;&gt;&amp;amp;&lt;/a&gt; plain"
        );
        assert_eq!(escape(""), "");
    }

    #[test]
    fn a_page_is_one_static_document() {
        let page = page(&draft(&[(None, "Hello.")]));
        assert!(page.starts_with("<!doctype html>\n<html lang=\"en\">\n<head>\n"));
        assert!(page.contains("<meta charset=\"utf-8\">\n"));
        assert!(page.contains("<title>Weekly &lt;report&gt;</title>\n"));
        assert!(page.contains(&format!(
            "<style>{STYLE}</style>\n</head>\n<body>\n<main>\n"
        )));
        assert!(page.ends_with("</main>\n</body>\n</html>\n"));
        assert_eq!(
            main_of(&page),
            "<h1>Weekly &lt;report&gt;</h1>\n<p>Hello.</p>\n"
        );
        assert!(!STYLE.contains('<') && !STYLE.contains("url("));
    }

    #[test]
    fn section_text_becomes_plain_blocks() {
        let body = "First line\n second line\n\n- one\n* two\nafter list\n# Part\n## Sub\n### Deep\n#### Four\n#\n\n```\n<b>code</b>\n\n  kept\n```\nlast";
        assert_eq!(
            main_of(&page(&draft(&[(Some("notes & more"), body)]))),
            "<h1>Weekly &lt;report&gt;</h1>\n<h2>notes &amp; more</h2>\n<p>First line\nsecond line</p>\n<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n<p>after list</p>\n<h2>Part</h2>\n<h3>Sub</h3>\n<h4>Deep</h4>\n<p>#### Four\n#</p>\n<pre><code>&lt;b&gt;code&lt;/b&gt;\n\n  kept</code></pre>\n<p>last</p>\n"
        );
    }

    #[test]
    fn a_list_after_a_paragraph_and_an_unclosed_fence_are_closed() {
        assert_eq!(
            main_of(&page(&draft(&[(None, "para\n- item\n```\nopen <x>")]))),
            "<h1>Weekly &lt;report&gt;</h1>\n<p>para</p>\n<ul>\n<li>item</li>\n</ul>\n<pre><code>open &lt;x&gt;</code></pre>\n"
        );
        assert_eq!(
            main_of(&page(&draft(&[(None, "```\n```")]))),
            "<h1>Weekly &lt;report&gt;</h1>\n<pre><code></code></pre>\n"
        );
    }

    #[test]
    fn markup_in_the_inputs_never_becomes_markup() {
        let hostile = "<script>alert(1)</script>\n<img src=x onerror=go()>\n[x](javascript:go())\n<iframe src=x>";
        let page = page(&draft(&[(Some("<b onclick=go()>"), hostile)]));
        let lower = page.to_ascii_lowercase();
        for active in ["<script", "<img", "<iframe", "<b "] {
            assert!(!lower.contains(active), "{active}");
        }
    }
}
