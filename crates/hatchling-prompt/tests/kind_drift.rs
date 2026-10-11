//! The kind-drift trap: no caller can take a prompt without
//! naming its kind.
//!
//! There is no default prompt to fall back on: `Loop::new` requires a
//! [`PromptKind`](hatchling_prompt::PromptKind), and the pack has no
//! kind-less assembler. This test keeps it that way across the whole
//! workspace, sources and tests alike: no member calls or defines a
//! kind-less `system_prompt` function. The patterns are assembled from
//! fragments so this file never matches itself.

#![allow(
    clippy::unwrap_used,
    reason = "walking the workspace is test scaffolding: an unreadable file should fail the test"
)]

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, skipping build output.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Whether `text` calls `call` as a name of its own, not as the tail
/// of a longer identifier (a test named `..._the_system_prompt()`).
fn calls(text: &str, call: &str) -> bool {
    text.match_indices(call).any(|(at, _)| {
        !text
            .get(..at)
            .and_then(|before| before.chars().next_back())
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    })
}

#[test]
fn the_scan_tells_a_call_from_a_longer_name() {
    let call = concat!("system_prompt", "()");
    let bare = format!("let p = {call};");
    let pathed = format!("hatchling_prompt::{call}");
    let longer = format!("fn below_the_{call} {{");
    assert!(calls(&bare, call));
    assert!(calls(&pathed, call));
    assert!(calls(call, call));
    assert!(!calls(&longer, call));
    assert!(!calls("system_prompt_for(kind)", call));
}

#[test]
fn no_member_takes_a_prompt_without_naming_its_kind() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    rust_files(crates, &mut files);
    // The workspace, not a stray directory: every member is scanned.
    for member in [
        "hatchling-prompt",
        "hatchling-loop",
        "hatchling-agent",
        "hatchling-research",
        "hatchling-broker",
    ] {
        assert!(
            files
                .iter()
                .any(|file| file.starts_with(crates.join(member))),
            "{member} was not scanned"
        );
    }
    let call = concat!("system_prompt", "()");
    let definition = concat!("fn system_prompt", "(");
    let offenders: Vec<String> = files
        .iter()
        .filter(|file| {
            let text = std::fs::read_to_string(file).unwrap();
            calls(&text, call) || text.contains(definition)
        })
        .map(|file| file.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "a kind-less system prompt is called or defined in {offenders:?}: name the kind with system_prompt_for"
    );
}
