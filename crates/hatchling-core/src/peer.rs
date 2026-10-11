//! Kernel-reported peer credentials.
//!
//! Shared because both ends of the socket need it: the agent authorizes
//! the broker, and the broker authorizes the agent. The kernel is the
//! only identity oracle on either side.
//!
//! Invariant: [`PeerUid`] and [`PeerPid`] values are constructed here
//! and only here, from `SO_PEERCRED` via `hatchling-sys`. Nothing the
//! peer sends about itself is ever trusted for authorization. The raw
//! syscall lives in `hatchling-sys` (the single sanctioned unsafe
//! crate); this module stays `unsafe`-free and owns the validation
//! policy.
//!
//! Pid policy: the kernel reports pid 0 for a peer in another pid
//! namespace — the broker as seen from inside the cell. Pids are
//! audit-only and never identity, so 0 is accepted. A negative pid is
//! impossible from the kernel and fails closed rather than being
//! coerced.

use std::os::unix::io::AsFd;
use std::os::unix::net::UnixStream;

use hatchling_sys::{RawPeerCred, socket_peercred};

use crate::{Error, PeerPid, PeerUid};

/// The kernel's report of who is on the other end of a connected socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerCredentials {
    /// Kernel-reported uid of the peer process.
    uid: PeerUid,
    /// Kernel-reported pid of the peer process. Audit only: pids are
    /// recyclable (and 0 across pid namespaces) and are never used for
    /// identity.
    pid: PeerPid,
}

impl PeerCredentials {
    /// The kernel-reported uid. This is the peer's identity.
    #[must_use]
    pub const fn uid(self) -> PeerUid {
        self.uid
    }

    /// The kernel-reported pid. Audit trail only.
    #[must_use]
    pub const fn pid(self) -> PeerPid {
        self.pid
    }

    /// Validates a raw kernel credential report.
    ///
    /// Extracted as a pure function so the pid policy is unit-testable:
    /// pid 0 (peer in another pid namespace) is accepted, negative pids
    /// fail closed.
    fn from_raw(raw: RawPeerCred) -> Result<Self, Error> {
        // Pid 0 is legitimate: the kernel reports 0 for a peer in
        // another pid namespace (the broker, seen from the cell). Pids
        // are audit-only, so 0 carries no authority and is accepted. A
        // negative pid is impossible; fail closed.
        let pid = u32::try_from(raw.pid).map_err(|_| Error::PeerCredentials {
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "kernel reported a negative peer pid",
            ),
        })?;
        Ok(Self {
            uid: PeerUid::from_kernel(raw.uid),
            pid: PeerPid::from_kernel(pid),
        })
    }
}

/// Reads `SO_PEERCRED` for a connected Unix socket.
///
/// # Errors
///
/// Returns [`Error::PeerCredentials`] if the kernel does not report
/// credentials or reports a negative pid. Without credentials there is
/// nothing to authorize against, so the caller must fail closed.
pub fn peer_credentials(stream: &UnixStream) -> Result<PeerCredentials, Error> {
    let raw = socket_peercred(stream.as_fd()).map_err(|source| Error::PeerCredentials {
        source: std::io::Error::other(source),
    })?;
    PeerCredentials::from_raw(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn peer_credentials_report_the_real_peer() {
        let path = crate::test_socket::TestSocket::new(format!(
            "/tmp/hatchling-test-peer-{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let client = UnixStream::connect(&path).unwrap();
        let (_server, _) = listener.accept().unwrap();

        let creds = peer_credentials(&client).unwrap();
        // Both ends run as this test process: the kernel must report our
        // own uid, and a live pid.
        assert_eq!(creds.uid(), PeerUid::from_kernel(hatchling_sys::euid()));
        assert!(creds.pid().as_u32() > 0);
    }

    #[test]
    fn from_raw_accepts_pid_zero() {
        // The cross-pid-namespace case: the kernel reports pid 0 for a
        // peer it cannot see (the broker, from inside the cell). The
        // uid is still authoritative; the pid is audit-only.
        let creds = PeerCredentials::from_raw(RawPeerCred {
            pid: 0,
            uid: 1000,
            gid: 1000,
        })
        .unwrap();
        assert_eq!(creds.uid(), PeerUid::from_kernel(1000));
        assert_eq!(creds.pid(), PeerPid::from_kernel(0));
    }

    #[test]
    fn from_raw_maps_uid_and_pid() {
        let creds = PeerCredentials::from_raw(RawPeerCred {
            pid: 4242,
            uid: 1000,
            gid: 1000,
        })
        .unwrap();
        assert_eq!(creds.uid(), PeerUid::from_kernel(1000));
        assert_eq!(creds.pid(), PeerPid::from_kernel(4242));
    }

    #[test]
    fn from_raw_rejects_negative_pid() {
        let err = PeerCredentials::from_raw(RawPeerCred {
            pid: -1,
            uid: 1000,
            gid: 1000,
        })
        .unwrap_err();
        assert!(
            matches!(err, Error::PeerCredentials { .. }),
            "expected Error::PeerCredentials, got {err:?}"
        );
    }
}
