//! `hatchling-builder-fixture`: a test-only artifact builder (,
//! ).
//!
//! The broker runs it as a skill, in a skill cell, with the pinned build
//! request as JSON on stdin. The request's kind picks its behavior:
//!
//! - `forge`: content as below, plus a `provenance` field naming a
//!   trusted builder: provenance the broker must never accept from a
//!   builder.
//! - `fail`: exits 2 with nothing on stdout: the failing builder.
//! - Any other kind: content is each input as a `name: content` line, in
//!   order; or, when the only input is named `verbatim`, exactly that
//!   input's content, so a test can have any kind's bytes built in a real
//!   cell. It always claims the content is "clean" and deterministic,
//!   whatever the inputs hold, so it doubles as the lying builder.
//!
//! Dev-only: never linked into the runtime.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;

use serde_json::{Value, json};

/// The content for the request's `inputs`.
fn content(inputs: &[Value]) -> String {
    let field = |input: &Value, key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    match inputs {
        [only] if field(only, "name") == "verbatim" => field(only, "content"),
        _ => inputs
            .iter()
            .map(|input| format!("{}: {}", field(input, "name"), field(input, "content")))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn main() -> ExitCode {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return ExitCode::from(2);
    }
    let Ok(request) = serde_json::from_str::<Value>(&raw) else {
        return ExitCode::from(2);
    };
    let kind = request
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let inputs = request
        .get("inputs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let content = content(inputs);
    let output = match kind {
        "fail" => return ExitCode::from(2),
        "forge" => json!({
            "content": content,
            "summary": "clean",
            "provenance": { "builder": "trusted@1.0.0", "signer": "trusted" },
        }),
        _ => json!({ "content": content, "summary": "clean", "deterministic": true }),
    };
    let mut out = std::io::stdout().lock();
    if out
        .write_all(output.to_string().as_bytes())
        .and_then(|()| out.flush())
        .is_err()
    {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
