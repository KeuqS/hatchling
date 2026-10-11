//! The builder binaries as the broker runs them: the request on stdin,
//! the result on stdout, and the exit status.
#![allow(
    clippy::unwrap_used,
    reason = "fixtures: a failed setup must fail the calling test loudly"
)]

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// Runs `binary` with `stdin`; returns its exit status and stdout.
fn run(binary: &str, stdin: &str) -> (Option<i32>, String) {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code(), String::from_utf8(out.stdout).unwrap())
}

fn request(kind: &str, inputs: &[(&str, &str)]) -> String {
    let inputs: Vec<Value> = inputs
        .iter()
        .map(|(name, content)| json!({ "name": name, "sha256": "00", "content": content }))
        .collect();
    json!({ "kind": kind, "inputs": inputs }).to_string()
}

const DOCUMENT: &str = env!("CARGO_BIN_EXE_hatchling-build-document");
const PAGE: &str = env!("CARGO_BIN_EXE_hatchling-build-page");

#[test]
fn each_binary_builds_its_kind_on_stdout() {
    let (status, out) = run(
        DOCUMENT,
        &request("document", &[("title", "Plan"), ("body", "Go.")]),
    );
    assert_eq!(status, Some(0));
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap(),
        json!({
            "content": "# Plan\n\nGo.\n",
            "summary": "a document with 1 section",
            "deterministic": true,
        })
    );
    let (status, out) = run(PAGE, &request("page", &[("title", "Plan")]));
    assert_eq!(status, Some(0));
    let content = serde_json::from_str::<Value>(&out)
        .unwrap()
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap();
    assert!(content.starts_with("<!doctype html>"), "{content}");
}

#[test]
fn a_binary_that_cannot_build_exits_non_zero_with_nothing_on_stdout() {
    for (binary, stdin, status) in [
        (DOCUMENT, String::from("not json"), 2),
        (PAGE, request("document", &[("title", "x")]), 2),
        (DOCUMENT, request("document", &[("body", " ")]), 3),
        (
            PAGE,
            request("page", &[("title", "t"), ("body", &"<".repeat(2048))]),
            4,
        ),
    ] {
        let (code, out) = run(binary, &stdin);
        assert_eq!(code, Some(status), "{binary}");
        assert!(out.is_empty(), "{out}");
    }
}
