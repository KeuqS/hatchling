//! `hatchling-dev`: the development-only audit binary.
//!
//! Thin dispatcher over the checks in the library: `audit` runs the
//! fast checks (also covered by the crate's unit tests, so they gate
//! every push), `mutants` and `coverage` run the slow scheduled checks,
//! `mutants-cached` runs the mutation gate through the content-hash
//! cache. Exit code is the contract: 0 means every check passed.

use std::io::Write as _;
use std::process::ExitCode;

use hatchling_dev::{Error, dispatch, workspace_root};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "hatchling-dev: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Error> {
    let root = workspace_root();
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| String::from("audit"));
    let rest: Vec<String> = args.collect();

    let findings = dispatch(command.as_str(), &rest, &root)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if findings.is_empty() {
        let _ = writeln!(out, "hatchling-dev {command}: all checks passed");
        Ok(())
    } else {
        for finding in &findings {
            let _ = writeln!(out, "[{}] {}", finding.check, finding.message);
        }
        Err(Error::ToolFailed {
            tool: format!("hatchling-dev {command}"),
            status: format!("{} finding(s)", findings.len()),
        })
    }
}
