//! Binary edge for `hatchling-agent`.
//!
//! Thin by design: parse config and arguments, install logging, run
//! the cell lifecycle, and translate the outcome to a process exit
//! code. No logic lives here; everything testable is in the library.
//!
//! Note the denied lints: no `println!`/`eprint!`/`process::exit`.
//! Startup diagnostics go to stderr via `io::Write`, the task answer
//! goes to stdout the same way, and the process outcome is a returned
//! [`ExitCode`].

use std::io::Write as _;
use std::process::ExitCode;

use hatchling_agent::{Cell, Config, Error, run_command};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "hatchling-agent: failed to start: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatches on the subcommand. Bare invocation keeps the historical
/// behavior: connect, handshake, report readiness, exit.
fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => run_lifecycle(),
        Some("run") => run_task(&args.into_iter().skip(1).collect::<Vec<_>>()),
        Some(other) => Err(Error::Usage(format!(
            "unknown subcommand {other:?}; usage: hatchling-agent run \"task\""
        ))),
    }
}

/// Parses configuration, checks preconditions, connects to the broker,
/// authorizes the broker's kernel peer credentials, and completes the
/// handshake.
///
/// # Errors
///
/// Propagates [`Error`] from config parsing, precondition checks, the
/// broker connection, peer authorization, or the handshake. Any failure
/// is fail-closed: the cell does not run.
fn run_lifecycle() -> Result<(), Error> {
    let config = Config::from_env()?;
    tracing_subscriber::fmt()
        .with_max_level(config.log_level.to_tracing())
        .init();
    let cell = Cell::new(config)
        .check_preconditions()?
        .connect_broker()?
        .authorize()?
        .handshake()?;
    tracing::info!(cell_id = %cell.config().cell_id, "cell authorized, handshake complete, ready");
    Ok(())
}

/// Prints the final answer of one agent task to stdout.
///
/// The library owns the whole subcommand flow (see [`run_command`]);
/// the binary only moves the answer to stdout. Logs already went to
/// stderr, so the answer stays machine-readable.
///
/// # Errors
///
/// Propagates [`Error`] from [`run_command`]: empty task, missing or
/// invalid model configuration, mint failure, or loop failure.
fn run_task(task_args: &[String]) -> Result<(), Error> {
    let answer = run_command(task_args)?;
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{answer}");
    Ok(())
}
