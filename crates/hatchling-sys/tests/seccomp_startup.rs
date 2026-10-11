//! The agent binary must survive its own startup under the installed
//! agent seccomp profile.
//!
//! The profile is fail-closed (`KILL_PROCESS`): any syscall the real
//! agent needs at startup but the profile omits kills it with SIGSYS
//! before `main`. This test forks a child, installs
//! [`SeccompProfile::agent()`], and execs the real agent binary with a
//! dead broker socket — the dynamic loader, the Rust runtime, config
//! parsing, tracing init, and the clean broker-connect failure must
//! all complete without tripping the filter. That is the exact
//! install-then-exec ordering the real cell spawn uses
//! (`hatchling-broker`'s `child_main`), minus the namespaces.
//!
//! Needs no user namespace — fork + seccomp + exec only — so it runs
//! on every Linux worker, including ones where the cell boundary
//! tests skip. The first test expects exit 1 (the agent starts, then
//! fails cleanly to connect); the second drives a full HELLO/WELCOME
//! over a real socket and expects exit 0. 128+31 = 159 means SIGSYS,
//! i.e. the profile is missing a syscall.

use std::os::unix::net::UnixListener;
use std::os::unix::process::CommandExt as _;

use std::time::Duration;

use hatchling_sys::process::{ForkResult, child_exit_code, exit_child, fork, set_no_new_privs};
use hatchling_sys::seccomp::{SeccompProfile, install};

/// Locates the agent binary: `HATCHLING_AGENT_BIN` wins, otherwise the
/// workspace `target/debug` build. Panics (never skips) when absent —
/// a skipping test is how the `access` startup kill went unnoticed on
/// machines that couldn't run the cell path.
fn agent_binary() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("HATCHLING_AGENT_BIN") {
        let path = std::path::PathBuf::from(path);
        assert!(
            path.is_file(),
            "HATCHLING_AGENT_BIN points nowhere: {}; run `cargo build -p hatchling-agent`",
            path.display()
        );
        return path;
    }
    let candidate = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/hatchling-agent");
    assert!(
        candidate.is_file(),
        "hatchling-agent binary not found at {}: run `cargo build -p hatchling-agent` first (or set HATCHLING_AGENT_BIN)",
        candidate.display()
    );
    candidate
}

#[test]
fn agent_survives_startup_under_its_seccomp_profile() {
    let agent = agent_binary();
    let child = match fork().expect("fork for seccomp startup test") {
        ForkResult::Parent { child } => child,
        ForkResult::Child => {
            // Child: NoNewPrivs first (unprivileged seccomp install
            // requires it), then the filter, then exec. Mirrors the
            // real spawn's ordering.
            if set_no_new_privs().is_err() {
                exit_child(99);
            }
            let Ok(profile) = SeccompProfile::agent() else {
                exit_child(98);
            };
            if install(&profile).is_err() {
                exit_child(97);
            }
            // Enough config for the agent to get through startup and
            // fail cleanly on the dead broker socket. The socket path
            // is deliberately nonexistent: connect fails with ENOENT,
            // the agent reports the error and exits 1.
            //
            // Env is passed on the Command (safe `.env`), not via
            // set_var: this crate's tests deny unsafe_code, and the
            // explicit env mirrors the real spawn's exec anyway.
            let _err = std::process::Command::new(&agent)
                .env("HATCHLING_CELL_ID", "seccomp-startup-test")
                .env(
                    "HATCHLING_BROKER_SOCKET",
                    "/nonexistent/hatchling-seccomp-test-broker.sock",
                )
                .env("HATCHLING_BROKER_UID", "1000")
                .env("HATCHLING_LOG_LEVEL", "error")
                .exec();
            // `exec` only returns on failure; the parent's exit-code
            // contract reports that as 96.
            exit_child(96);
        }
    };

    let status = child
        .wait_deadline(Duration::from_secs(60))
        .expect("bounded wait for seccomp startup test");
    let code = child_exit_code(status);
    // Exit-code contract:
    //   1   agent survived the filter, failed cleanly to connect (pass)
    //   159 SIGSYS: the filter killed the agent at startup (FAIL —
    //       the profile is missing a startup syscall)
    //   99/98/97/96 harness or environment failure (fail loudly)
    assert!(
        code == 1,
        "agent under its seccomp profile exited {code}, expected 1 (clean connect failure): \
         159 = SIGSYS, the profile is missing a startup syscall; \
         99/98/97/96 = test harness failure"
    );
}

/// The test broker's socket under `/tmp`, removed when the test ends,
/// pass or fail. The forked child never drops it: it execs or
/// exits without unwinding.
struct SocketFile(String);

impl Drop for SocketFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn agent_completes_handshake_under_its_seccomp_profile() {
    use hatchling_core::framing::{read_frame, write_frame};
    use hatchling_core::{AgentMessage, BrokerMessage, PROTOCOL_VERSION};

    // A full HELLO/WELCOME over a plain Unix socket, with the agent
    // under the installed filter: reading HELLO exercises recvfrom,
    // writing WELCOME exercises sendto, and the agent's teardown
    // exercises fcntl. The startup test above cannot catch a missing
    // handshake syscall — the agent dies only after connect.
    let sock_path = SocketFile(format!(
        "/tmp/hatchling-test-sechandshake-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sock_path.0);
    let listener = UnixListener::bind(&sock_path.0).expect("bind test broker socket");
    let agent = agent_binary();
    let broker_uid = hatchling_sys::euid().to_string();

    let child = match fork().expect("fork for seccomp handshake test") {
        ForkResult::Parent { child } => child,
        ForkResult::Child => {
            // Same install-then-exec ordering as the real spawn.
            if set_no_new_privs().is_err() {
                exit_child(99);
            }
            let Ok(profile) = SeccompProfile::agent() else {
                exit_child(98);
            };
            if install(&profile).is_err() {
                exit_child(97);
            }
            // The broker socket is real this time; the peer uid is the
            // test process's own, so the agent's peer gate passes.
            let _err = std::process::Command::new(&agent)
                .env("HATCHLING_CELL_ID", "seccomp-handshake-test")
                .env("HATCHLING_BROKER_SOCKET", &sock_path.0)
                .env("HATCHLING_BROKER_UID", &broker_uid)
                .env("HATCHLING_LOG_LEVEL", "error")
                .exec();
            exit_child(96);
        }
    };

    // Parent plays the broker. Accept with a deadline so a SIGSYS'd
    // agent fails the test instead of hanging it.
    listener
        .set_nonblocking(true)
        .expect("set listener nonblocking");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) => {
                assert_eq!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock,
                    "broker accept failed: {e}"
                );
                assert!(
                    std::time::Instant::now() <= deadline,
                    "timed out waiting for the agent to connect: it likely died under the filter"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    // The agent speaks first: HELLO. The frame must be non-empty —
    // an empty read would mean the agent died before writing.
    let frame = read_frame(&stream).expect("read HELLO from the agent");
    assert!(!frame.is_empty(), "agent closed without sending HELLO");
    let hello: AgentMessage = serde_json::from_slice(&frame).expect("parse HELLO");
    let AgentMessage::Hello { v, .. } = hello;
    assert_eq!(v, PROTOCOL_VERSION, "agent must speak the current protocol");
    // Answer WELCOME; the agent exits 0 after a clean handshake.
    let welcome = serde_json::to_vec(&BrokerMessage::Welcome {
        v: PROTOCOL_VERSION,
    })
    .expect("encode WELCOME");
    write_frame(&stream, &welcome).expect("write WELCOME to the agent");
    drop(stream);

    let status = child
        .wait_deadline(Duration::from_secs(60))
        .expect("bounded wait for seccomp handshake test");
    let code = child_exit_code(status);
    // Exit-code contract:
    //   0   agent completed the handshake under the filter (pass)
    //   159 SIGSYS: the profile is missing a handshake syscall (FAIL —
    //       sendto, recvfrom, or fcntl)
    //   1   agent failed the handshake for a non-filter reason (FAIL)
    //   99/98/97/96 harness or environment failure (fail loudly)
    assert!(
        code == 0,
        "agent under its seccomp profile exited {code}, expected 0 (clean handshake): \
         159 = SIGSYS, the profile is missing a handshake syscall; \
         99/98/97/96 = test harness failure"
    );
}
