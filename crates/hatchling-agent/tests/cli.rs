//! Binary-level CLI tests: subcommand dispatch and usage failures.
//!
//! These pin the thin binary's behavior without a broker — every case
//! here fails before configuration is even read, so no fixture is
//! needed. In particular they kill a mutant that replaces the
//! binary's `run_task` with `Ok(())`: the usage cases must exit
//! non-zero.

use std::process::Command;

const fn agent_binary() -> &'static str {
    env!("CARGO_BIN_EXE_hatchling-agent")
}

#[test]
fn run_without_task_fails_closed() {
    let output = Command::new(agent_binary())
        .arg("run")
        .output()
        .expect("spawn agent binary");
    assert!(
        !output.status.success(),
        "`run` with no task must fail, not silently succeed"
    );
}

#[test]
fn run_with_blank_task_fails_closed() {
    let output = Command::new(agent_binary())
        .args(["run", "   "])
        .output()
        .expect("spawn agent binary");
    assert!(
        !output.status.success(),
        "`run` with a blank task must fail, not silently succeed"
    );
}

#[test]
fn unknown_subcommand_fails_closed() {
    let output = Command::new(agent_binary())
        .arg("frobnicate")
        .output()
        .expect("spawn agent binary");
    assert!(
        !output.status.success(),
        "unknown subcommand must fail, not silently succeed"
    );
}
