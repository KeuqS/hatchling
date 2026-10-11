//! Component: a socket the broker binds never carries a
//! permissive mode, not even between `bind` and the mode change.
//!
//! This binary holds one test on purpose. It runs the whole process
//! under a hostile `000` umask, which would give any file another test
//! created meanwhile a world-writable mode.
#![allow(
    clippy::unwrap_used,
    reason = "fixture setup: a failed bind or stat must fail the test loudly"
)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use hatchling_broker::{BIND_UMASK, Error, Ingress, OwnerOnlyListener, bind_socket};
use hatchling_core::SocketPath;
use rustix::fs::Mode;

fn path(name: &str) -> SocketFile {
    let path = PathBuf::from(format!("/tmp/hlum-{}-{name}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    SocketFile(path)
}

/// A test socket under `/tmp`, removed when the test ends, pass or fail
/// .
struct SocketFile(PathBuf);

impl std::ops::Deref for SocketFile {
    type Target = PathBuf;

    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn mode_of(path: &PathBuf) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_hostile_umask_never_reaches_a_broker_socket() {
    // The hostile umask: without a socket is born 0777.
    let saved = rustix::process::umask(Mode::from_bits_truncate(0o000));
    assert_eq!(hatchling_sys::current_umask(), 0o000);
    assert_eq!(BIND_UMASK, 0o077);

    // 1. The bind step alone, observed before any mode is applied: the
    //    socket is born owner-only, and the hostile umask is back.
    let first = path("owner-only");
    let owner_only = OwnerOnlyListener::bind(&first).unwrap();
    assert_eq!(mode_of(&first), 0o700, "born with no group or other bits");
    assert_eq!(hatchling_sys::current_umask(), 0o000, "umask restored");
    // 2. Then, and only through it, the configured mode.
    let _listener = owner_only.with_mode(0o640).unwrap();
    assert_eq!(mode_of(&first), 0o640);

    // 3. The broker's helper end to end, and the ingress through it.
    let second = path("helper");
    let socket: SocketPath = second.to_str().unwrap().parse().unwrap();
    let _listener = bind_socket(&socket, 0o600).unwrap();
    assert_eq!(mode_of(&second), 0o600);
    let third = path("ingress");
    let socket: SocketPath = third.to_str().unwrap().parse().unwrap();
    let _ingress = Ingress::bind(&socket, hatchling_channel::OutputGate::disabled()).unwrap();
    assert_eq!(mode_of(&third), 0o600);
    assert_eq!(hatchling_sys::current_umask(), 0o000);

    // 4. A failed bind restores the umask too: no 077 leaks into the
    //    process.
    let nowhere = PathBuf::from(format!(
        "/tmp/hlum-{}-missing-dir/broker.sock",
        std::process::id()
    ));
    assert!(matches!(
        OwnerOnlyListener::bind(&nowhere),
        Err(Error::SocketSetup { .. })
    ));
    assert_eq!(hatchling_sys::current_umask(), 0o000, "restored on failure");

    let _ = rustix::process::umask(saved);
}
