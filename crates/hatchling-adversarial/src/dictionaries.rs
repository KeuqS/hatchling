//! Path dictionaries the in-cell probes sweep.
//!
//! The probe cannot list directories (`getdents64` is denied by the
//! seccomp profile) and has no `/proc`, so every sweep is
//! dictionary-based: the probe stats/connects each compiled-in path
//! and reports what existed. The committed inventory pins the exact
//! observed set, so a construction change that adds a file fails
//! loudly instead of slipping past the sweep.
//!
//! [`LIB_CANDIDATES`] is special: a test asserts it covers the probe
//! binary's actual `ldd` output, so a toolchain change that adds a
//! library fails the suite instead of silently narrowing the sweep.

/// Filesystem socket candidates beyond the broker socket and the decoy
/// convention path: ambient names that must not resolve in the cell.
pub const FS_SOCKET_CANDIDATES: &[&str] = &[
    "/tmp/hatchling-ambient.sock",
    "/var/run/hatchling-ambient.sock",
    "/run/hatchling-ambient.sock",
    "/tmp/dbus-ambient",
];

/// Abstract socket candidates: well-known names plus guesses. The
/// cell has no `/proc/net/unix` (no `/proc` at all), so the sweep is
/// dictionary-based; the inventory pins the exact attempted set.
pub const ABSTRACT_CANDIDATES: &[&str] = &[
    "hatchling-broker",
    "hatchling-ambient",
    "dbus-system",
    "dbus-session",
    "snapd",
];

/// Shared-library paths the probe stats for writability and
/// setuid/setgid bits, `x86_64`. `tests/dictionary_coverage.rs` asserts
/// this covers the probe binary's actual `ldd` output.
///
/// Both layouts are listed: usrmerged distros (Ubuntu 26.04, the test
/// box) report every library under `/usr/lib`, older ones under `/lib`.
#[cfg(target_arch = "x86_64")]
pub const LIB_CANDIDATES: &[&str] = &[
    "/lib64/ld-linux-x86-64.so.2",
    "/lib/x86_64-linux-gnu/libc.so.6",
    "/lib/x86_64-linux-gnu/libm.so.6",
    "/lib/x86_64-linux-gnu/libgcc_s.so.1",
    "/lib/x86_64-linux-gnu/libpthread.so.0",
    "/lib/x86_64-linux-gnu/libdl.so.2",
    "/lib/x86_64-linux-gnu/librt.so.1",
    "/usr/lib/x86_64-linux-gnu/libc.so.6",
    "/usr/lib/x86_64-linux-gnu/libm.so.6",
    "/usr/lib/x86_64-linux-gnu/libgcc_s.so.1",
    "/usr/lib/x86_64-linux-gnu/libpthread.so.0",
    "/usr/lib/x86_64-linux-gnu/libdl.so.2",
    "/usr/lib/x86_64-linux-gnu/librt.so.1",
    "/usr/lib64/ld-linux-x86-64.so.2",
];

/// Non-x86_64: the suite is x86_64-only (the syscall table is), so the
/// dictionary is empty and the driver skips before spawning.
#[cfg(not(target_arch = "x86_64"))]
pub const LIB_CANDIDATES: &[&str] = &[];

/// Paths that must not exist in the cell: schedulers, service
/// managers, ambient `PATH` entries, temp dirs, device/proc/sys.
pub const ABSENT_PATHS: &[&str] = &[
    "/etc/crontab",
    "/etc/cron.d",
    "/etc/cron.daily",
    "/etc/cron.hourly",
    "/var/spool/cron",
    "/var/spool/cron/crontabs",
    "/usr/lib/systemd",
    "/etc/systemd",
    "/run/systemd",
    "/lib/systemd",
    "/root/.config/systemd",
    "/home",
    "/root",
    "/bin",
    "/usr/bin",
    "/sbin",
    "/usr/sbin",
    "/usr/local/bin",
    "/tmp",
    "/var/tmp",
    "/dev",
    "/proc",
    "/sys",
    "/etc",
    "/var",
];

/// The decoy socket path, by convention with the driver: the driver
/// binds it only for the `peer-decoy` attack.
pub const DECOY_SOCKET: &str = "/sock/decoy.sock";
