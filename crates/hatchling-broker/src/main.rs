//! Binary edge for `hatchling-broker`.
//!
//! Thin by design: parse config, dispatch an operator subcommand or
//! serve agents until killed. No logic lives here; everything testable
//! is in the library.
//!
//! Subcommands (operator actions, run instead of serving):
//! - `migrate-vault`: encrypt a plaintext store into sealed envelopes.
//! - `rekey-vault <new-key-file>`: rotate the master key.
//! - `vault-keyring <action>`: adopt, enroll or inspect the vault keyring.
//! - `redactions <action>`, `vault-audit`: redaction review, credential access.
//!
//! Internal: `skill-cell <spec>` is the spawner helper the broker
//! starts to run one skill cell (see `hatchling_broker::skill_run`).
//! It reads no broker configuration.
//!
//! Note the denied lints: no `println!`/`eprint!`/`process::exit`. Startup
//! diagnostics go to stderr via `io::Write`, and the process outcome is a
//! returned [`ExitCode`].

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use hatchling_broker::skill_run::{SKILL_CELL_SUBCOMMAND, skill_cell_main};
use hatchling_broker::vault_keyring::run_keyring;
use hatchling_broker::{AGENT_RUN_SUBCOMMAND, agent_run_main, run_operator};
use hatchling_broker::{Config, Error, run_migrate_vault, run_rekey_vault, run_serve};

fn main() -> ExitCode {
    // The spawner helpers: fresh, single-threaded processes the broker
    // starts for one skill cell or one scheduled run, before any config.
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some(SKILL_CELL_SUBCOMMAND) => return skill_cell_main(args.next().as_deref()),
        Some(AGENT_RUN_SUBCOMMAND) => return agent_run_main(args.next().as_deref()),
        Some(_) | None => {}
    }
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "hatchling-broker: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Parses configuration, then either runs an operator subcommand or
/// serves agents until killed.
///
/// # Errors
///
/// Propagates [`Error`] from config parsing, subcommand misuse, or the
/// dispatched library action.
fn run() -> Result<(), Error> {
    let config = Config::from_env()?;
    // Operator subcommands act on the vault directly and never bind
    // the socket. Anything else falls through to serving.
    match std::env::args().nth(1).as_deref() {
        Some("migrate-vault") => run_migrate_vault(&config),
        Some("rekey-vault") => {
            let new_key_file = std::env::args().nth(2).ok_or_else(|| Error::Usage {
                reason: String::from(
                    "rekey-vault requires a new key file path: \
                         hatchling-broker rekey-vault <new-key-file>",
                ),
            })?;
            run_rekey_vault(&config, Path::new(&new_key_file))
        }
        Some("vault-keyring") => run_keyring(&config.vault, std::env::args().nth(2).as_deref()),
        Some(other) => run_operator(other, &config, std::env::args().skip(2)),
        None => run_serve(&config),
    }
}
