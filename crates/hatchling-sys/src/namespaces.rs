//! Thin wrappers over namespace creation.
//!
//! Each wrapper is one syscall with a named error; there is no policy
//! here. The caller (the broker's cell spawner) decides the order:
//! user namespace first, id maps written by the parent, then the mount,
//! network, and IPC namespaces, then (after the mount tree is built) the
//! pid namespace via [`unshare_pid`] followed by a fork.

use crate::Error;

/// Enters a new user namespace.
///
/// After this returns, the process has full capabilities inside the new
/// namespace but no id map yet: the parent must write the uid/gid maps
/// before the child does anything that depends on identity.
///
/// # Errors
///
/// Returns [`Error::Namespace`] if the `unshare` syscall fails.
pub fn unshare_user() -> Result<(), Error> {
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWUSER).map_err(|source| {
        Error::Namespace {
            operation: "unshare(user)",
            source: source.into(),
        }
    })
}

/// Enters a new mount namespace.
///
/// # Errors
///
/// Returns [`Error::Namespace`] if the `unshare` syscall fails.
pub fn unshare_mount() -> Result<(), Error> {
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNS).map_err(|source| {
        Error::Namespace {
            operation: "unshare(mount)",
            source: source.into(),
        }
    })
}

/// Enters a new network namespace: its own loopback (down), nothing else.
///
/// No host interface, no host TCP or UDP, no host netlink, and no host
/// abstract unix socket is reachable from it: abstract socket names live
/// in the network namespace. Pathname unix sockets are unaffected: they
/// are found through the filesystem, i.e. the mount namespace.
///
/// Needs `CAP_SYS_ADMIN` over the current user namespace, which a process
/// has right after [`unshare_user`].
///
/// # Errors
///
/// Returns [`Error::Namespace`] if the `unshare` syscall fails.
pub fn unshare_net() -> Result<(), Error> {
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNET).map_err(|source| {
        Error::Namespace {
            operation: "unshare(net)",
            source: source.into(),
        }
    })
}

/// Enters a new IPC namespace: its own System V IPC objects (shared
/// memory, semaphores, message queues) and POSIX message queues, none of
/// the host's.
///
/// # Errors
///
/// Returns [`Error::Namespace`] if the `unshare` syscall fails.
pub fn unshare_ipc() -> Result<(), Error> {
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWIPC).map_err(|source| {
        Error::Namespace {
            operation: "unshare(ipc)",
            source: source.into(),
        }
    })
}

/// Unshares the pid namespace.
///
/// The new namespace applies to the *children* of this process: the
/// caller must fork afterwards, and the child becomes PID 1 in the new
/// namespace.
///
/// # Errors
///
/// Returns [`Error::Namespace`] if the `unshare` syscall fails.
pub fn unshare_pid() -> Result<(), Error> {
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWPID).map_err(|source| {
        Error::Namespace {
            operation: "unshare(pid)",
            source: source.into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ForkResult, child_exit_code, exit_child, fork};
    use std::time::Duration;

    fn read_ns_link(kind: &str) -> String {
        std::fs::read_link(format!("/proc/self/ns/{kind}"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[test]
    fn unshare_user_enters_a_new_user_namespace() {
        // Mutant: unshare_user replaced with Ok(()). Fork a child,
        // record the user ns, unshare, and verify the ns changed. If
        // the function is a no-op, the link is identical.
        let child = match fork().expect("fork") {
            ForkResult::Parent { child } => child,
            ForkResult::Child => {
                let before = read_ns_link("user");
                if unshare_user().is_err() {
                    exit_child(99);
                }
                let after = read_ns_link("user");
                if after != before && !after.is_empty() {
                    exit_child(0);
                } else {
                    exit_child(1);
                }
            }
        };
        let status = child
            .wait_deadline(Duration::from_secs(30))
            .expect("bounded wait");
        let code = child_exit_code(status);
        if code == 99 {
            // Environmental: cannot create user namespaces here.
            return;
        }
        assert_eq!(code, 0, "user namespace must change after unshare_user");
    }

    #[test]
    fn unshare_mount_enters_a_new_mount_namespace() {
        // Mutant: unshare_mount replaced with Ok(()). Same pattern
        // as the user-namespace test, via /proc/self/ns/mnt.
        let child = match fork().expect("fork") {
            ForkResult::Parent { child } => child,
            ForkResult::Child => {
                let before = read_ns_link("mnt");
                if unshare_mount().is_err() {
                    exit_child(99);
                }
                let after = read_ns_link("mnt");
                if after != before && !after.is_empty() {
                    exit_child(0);
                } else {
                    exit_child(1);
                }
            }
        };
        let status = child
            .wait_deadline(Duration::from_secs(30))
            .expect("bounded wait");
        let code = child_exit_code(status);
        if code == 99 {
            return;
        }
        assert_eq!(code, 0, "mount namespace must change after unshare_mount");
    }

    /// Exit code for "the environment forbids this namespace": a skip,
    /// like 99 (no user namespaces at all).
    const DENIED: i32 = 97;

    /// Whether `err` is the environment refusing to create the namespace,
    /// not a bug: a runner that allows user namespaces but denies the
    /// capabilities inside them (an `AppArmor` user-namespace restriction,
    /// as on GitHub's runners) answers `EPERM` or `EACCES`. Any other
    /// failure is a real failure. Under `HATCHLING_REQUIRE_CELL=1` (the
    /// mutation runs on a box that can build cells) nothing is a skip.
    fn denied_by_environment(err: &Error) -> bool {
        if std::env::var("HATCHLING_REQUIRE_CELL").is_ok_and(|value| value == "1") {
            return false;
        }
        matches!(
            err,
            Error::Namespace { source, .. }
                if matches!(source.raw_os_error(), Some(libc::EPERM | libc::EACCES))
        )
    }

    /// The child's exit for a failed unshare: a skip when the environment
    /// forbids it, fatal otherwise.
    fn unshare_failed(err: &Error) -> ! {
        exit_child(if denied_by_environment(err) {
            DENIED
        } else {
            98
        })
    }

    /// Forks a child that enters a user namespace, then runs `unshare`
    /// and reports whether the `/proc/self/ns/<kind>` link changed.
    fn namespace_changes(kind: &'static str, unshare: fn() -> Result<(), Error>) -> Option<bool> {
        let child = match fork().expect("fork") {
            ForkResult::Parent { child } => child,
            ForkResult::Child => {
                if unshare_user().is_err() {
                    exit_child(99);
                }
                let before = read_ns_link(kind);
                if let Err(err) = unshare() {
                    unshare_failed(&err);
                }
                let after = read_ns_link(kind);
                exit_child(i32::from(after == before || after.is_empty()));
            }
        };
        let status = child
            .wait_deadline(Duration::from_secs(30))
            .expect("bounded wait");
        match child_exit_code(status) {
            // Environmental: no user namespaces here, or the runner forbids
            // this namespace inside one.
            99 | DENIED => None,
            code => Some(code == 0),
        }
    }

    #[test]
    fn unshare_net_enters_a_new_network_namespace() {
        if let Some(changed) = namespace_changes("net", unshare_net) {
            assert!(changed, "network namespace must change after unshare_net");
        }
    }

    #[test]
    fn unshare_ipc_enters_a_new_ipc_namespace() {
        if let Some(changed) = namespace_changes("ipc", unshare_ipc) {
            assert!(changed, "IPC namespace must change after unshare_ipc");
        }
    }

    /// The hole proved, closed at the namespace layer alone (no
    /// seccomp): a host abstract socket and a host TCP listener on
    /// 127.0.0.1, both reachable from a child before `unshare_net` (the
    /// control) and unreachable after it.
    #[test]
    fn a_new_network_namespace_cannot_reach_host_abstract_or_loopback_sockets() {
        use std::os::linux::net::SocketAddrExt as _;
        use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
        let name = format!("hatchling-sys-netns-control-{}", std::process::id());
        let addr = SocketAddr::from_abstract_name(&name).expect("abstract address");
        let _abstract = UnixListener::bind_addr(&addr).expect("bind abstract control");
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback control");
        let port = tcp.local_addr().expect("local addr").port();
        let child = match fork().expect("fork") {
            ForkResult::Parent { child } => child,
            ForkResult::Child => {
                let reach = |addr: &SocketAddr| UnixStream::connect_addr(addr).is_ok();
                let reach_tcp = || std::net::TcpStream::connect(("127.0.0.1", port)).is_ok();
                // Control: both are reachable from the host namespace.
                if !reach(&addr) || !reach_tcp() {
                    exit_child(10);
                }
                if unshare_user().is_err() {
                    exit_child(99);
                }
                if let Err(err) = unshare_net() {
                    unshare_failed(&err);
                }
                exit_child(match (reach(&addr), reach_tcp()) {
                    (false, false) => 0,
                    (true, _) => 1,
                    (false, true) => 2,
                });
            }
        };
        let status = child
            .wait_deadline(Duration::from_secs(30))
            .expect("bounded wait");
        let code = child_exit_code(status);
        if code == 99 || code == DENIED {
            return;
        }
        assert_eq!(
            code, 0,
            "exit 10: the control could not reach the host sockets; \
             1: host abstract socket reachable after unshare_net; \
             2: host loopback TCP reachable after unshare_net"
        );
    }

    #[test]
    fn only_a_permission_denial_is_environmental() {
        let denied = |errno| Error::Namespace {
            operation: "unshare(net)",
            source: std::io::Error::from_raw_os_error(errno),
        };
        let strict = std::env::var("HATCHLING_REQUIRE_CELL").is_ok_and(|value| value == "1");
        // Under the strict mode even a denial is a failure.
        assert_eq!(denied_by_environment(&denied(libc::EPERM)), !strict);
        assert_eq!(denied_by_environment(&denied(libc::EACCES)), !strict);
        for errno in [libc::EINVAL, libc::ENOSPC, libc::ENOMEM, libc::EUSERS] {
            assert!(!denied_by_environment(&denied(errno)), "errno {errno}");
        }
        let other = Error::Process {
            operation: "fork",
            source: std::io::Error::from_raw_os_error(libc::EPERM),
        };
        assert!(!denied_by_environment(&other));
    }

    #[test]
    fn without_a_user_namespace_unshare_net_fails_and_names_itself() {
        // Unprivileged, and outside a user namespace, CLONE_NEWNET needs a
        // capability this process lacks: the error must say which
        // namespace failed (the cell reports it verbatim). Root would
        // succeed, so there is nothing to assert then.
        if crate::process::euid() == 0 {
            return;
        }
        let child = match fork().expect("fork") {
            ForkResult::Parent { child } => child,
            ForkResult::Child => match unshare_net() {
                Ok(()) => exit_child(1),
                Err(err) if err.to_string().contains("unshare(net)") => exit_child(0),
                Err(_) => exit_child(2),
            },
        };
        let status = child
            .wait_deadline(Duration::from_secs(30))
            .expect("bounded wait");
        assert_eq!(child_exit_code(status), 0);
    }

    #[test]
    fn unshare_pid_makes_the_next_child_pid_1() {
        // Mutant: unshare_pid replaced with Ok(()). The pid namespace
        // applies to children: after unsharing, fork a grandchild and
        // check it sees pid 1. If unshare_pid is a no-op, the
        // grandchild gets a normal pid.
        let child = match fork().expect("fork") {
            ForkResult::Parent { child } => child,
            ForkResult::Child => {
                if unshare_pid().is_err() {
                    exit_child(99);
                }
                match fork().expect("fork grandchild") {
                    ForkResult::Parent { child } => {
                        let status = child
                            .wait_deadline(Duration::from_secs(30))
                            .expect("wait grandchild");
                        exit_child(child_exit_code(status));
                    }
                    ForkResult::Child => {
                        // SAFETY: getpid takes no args.
                        let pid = unsafe { libc::getpid() };
                        if pid == 1 {
                            exit_child(0);
                        } else {
                            exit_child(1);
                        }
                    }
                }
            }
        };
        let status = child
            .wait_deadline(Duration::from_secs(30))
            .expect("bounded wait");
        let code = child_exit_code(status);
        if code == 99 {
            return;
        }
        assert_eq!(code, 0, "grandchild must be pid 1 in the new pid namespace");
    }
}
