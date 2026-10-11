//! Raw `SO_PEERCRED` reads.
//!
//! rustix's typed `get_socket_peercred` cannot represent a peer pid of
//! 0: its `Pid` is a `NonZeroI32` newtype, so the call fails when the
//! kernel reports pid 0. The kernel *does* report pid 0 — for a peer in
//! another pid namespace, which is exactly the broker as seen from
//! inside the cell (the broker lives outside the cell's pid namespace).
//! This module reads the kernel's `struct ucred` as plain integers
//! instead, so pid 0 is representable. Pids are audit-only and never
//! identity; uids are the identity.

use std::os::fd::BorrowedFd;
use std::os::unix::io::AsRawFd;

use crate::Error;

/// The kernel's `struct ucred`, as plain integers.
///
/// The layout matches the kernel definition (`pid_t pid; uid_t uid;
/// gid_t gid`), so `getsockopt(SO_PEERCRED)` can fill it directly.
/// `pid` stays signed because the kernel reports 0 for a peer in
/// another pid namespace; callers must validate it before use — it is
/// audit-only, never identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct RawPeerCred {
    /// Peer pid, or 0 when the peer is in another pid namespace.
    pub pid: libc::pid_t,
    /// Peer uid: the identity.
    pub uid: libc::uid_t,
    /// Peer gid.
    pub gid: libc::gid_t,
}

/// Rejects a short `getsockopt` read.
///
/// A short read would leave `struct ucred` fields uninitialized, so it
/// is a hard error, never trusted. Extracted as a pure helper so the
/// branch is unit-testable: the kernel never produces a short read on
/// success, so no integration test could exercise it.
fn check_optlen(len: libc::socklen_t, expected: libc::socklen_t) -> Result<(), Error> {
    if len != expected {
        return Err(Error::PeerCredentials {
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "SO_PEERCRED returned fewer bytes than struct ucred",
            ),
        });
    }
    Ok(())
}

/// Reads the kernel peer credentials of a connected Unix socket.
///
/// Returns the raw `struct ucred` with pid 0 preserved: a zero pid
/// means the peer lives in another pid namespace (the normal case for
/// the broker as seen from inside the cell) and must be accepted as
/// audit data, not treated as a lookup failure.
///
/// # Errors
///
/// Returns [`Error::PeerCredentials`] if the `getsockopt` call fails
/// or the kernel returns fewer bytes than `struct ucred`.
pub fn socket_peercred(socket: BorrowedFd<'_>) -> Result<RawPeerCred, Error> {
    let expected = libc::socklen_t::try_from(size_of::<RawPeerCred>()).map_err(|_| {
        Error::PeerCredentials {
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "struct ucred size does not fit in socklen_t",
            ),
        }
    })?;
    let mut cred = std::mem::MaybeUninit::<RawPeerCred>::uninit();
    let mut len = expected;
    // SAFETY: `getsockopt` with `SO_PEERCRED` writes at most
    // `sizeof(struct ucred)` bytes into the buffer when it succeeds;
    // the buffer is `size_of::<RawPeerCred>()` bytes and `len`
    // advertises exactly that size, so the write cannot overflow.
    let ret = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            cred.as_mut_ptr().cast::<libc::c_void>(),
            &raw mut len,
        )
    };
    if ret != 0 {
        return Err(Error::PeerCredentials {
            source: std::io::Error::last_os_error(),
        });
    }
    check_optlen(len, expected)?;
    // SAFETY: `getsockopt` succeeded and wrote exactly
    // `size_of::<RawPeerCred>()` bytes (checked above), so every field
    // of `cred` is initialized.
    let cred = unsafe { cred.assume_init() };
    Ok(cred)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsFd as _;
    use std::os::unix::net::UnixListener;
    use std::os::unix::net::UnixStream;

    fn test_path(name: &str) -> crate::test_socket::TestSocket {
        crate::test_socket::TestSocket::new(format!(
            "/tmp/hatchling-test-pc-{}-{name}.sock",
            std::process::id()
        ))
    }

    #[test]
    fn reads_peer_credentials_off_a_live_socket() {
        let path = test_path("live");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let client = UnixStream::connect(&path).unwrap();
        let (server, _) = listener.accept().unwrap();

        let cred = socket_peercred(server.as_fd()).unwrap();
        assert_eq!(cred.uid, crate::euid());
        // SAFETY: getegid takes no arguments and always succeeds.
        assert_eq!(cred.gid, unsafe { libc::getegid() });
        let pid = libc::pid_t::try_from(std::process::id()).unwrap();
        assert_eq!(cred.pid, pid);

        drop(client);
    }

    #[test]
    fn non_socket_fd_fails() {
        // getsockopt(SO_PEERCRED) on a non-socket fails with ENOTSOCK:
        // the ret != 0 branch, exercised for real.
        let file = std::fs::File::open("/dev/null").unwrap();
        let err = socket_peercred(file.as_fd()).unwrap_err();
        assert!(
            matches!(err, Error::PeerCredentials { .. }),
            "expected Error::PeerCredentials, got {err:?}"
        );
    }

    #[test]
    fn check_optlen_accepts_exact_size() {
        assert!(check_optlen(12, 12).is_ok());
    }

    #[test]
    fn check_optlen_rejects_short_read() {
        let err = check_optlen(0, 12).unwrap_err();
        assert!(
            matches!(err, Error::PeerCredentials { .. }),
            "expected Error::PeerCredentials, got {err:?}"
        );
    }
}
