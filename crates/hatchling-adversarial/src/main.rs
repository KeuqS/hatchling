//! `hatchling-probe`: the adversarial probe binary.
//!
//! Thin entry point: all attack logic lives in
//! [`hatchling_adversarial::probe`], which runs inside a real
//! production-spawned cell under the production mount plan and
//! seccomp profile.

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    match hatchling_adversarial::probe::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(std::io::stderr(), "hatchling-probe: {message}");
            ExitCode::FAILURE
        }
    }
}
