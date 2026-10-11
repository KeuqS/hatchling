//! The process umask, set for exactly one scope.
//!
//! The umask is per process, not per thread, so a scoped change has two
//! hazards: another thread's change interleaving with it (two saves and
//! two restores in the wrong order leave the wrong mask behind), and the
//! restore being skipped on an early return or an unwind. [`with_umask`]
//! closes both: every change goes through one process-wide lock, and the
//! restore is a drop guard that runs on every exit from the scope while
//! the lock is still held.
//!
//! Threads that create files while a scope is open see its mask. The
//! broker only ever narrows it (to `077`), so they can only get stricter
//! modes than they asked for, never looser.

use std::sync::{Mutex, MutexGuard, PoisonError};

use rustix::fs::Mode;

/// Serializes every umask change in the process. A panic inside a scope
/// cannot corrupt the mask (the guard restores it during the unwind),
/// so a poisoned lock is still safe to take.
static UMASK: Mutex<()> = Mutex::new(());

/// Holds the process-wide umask lock.
fn serial() -> MutexGuard<'static, ()> {
    UMASK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Restores the saved umask when dropped.
struct Restore(Mode);

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = rustix::process::umask(self.0);
    }
}

/// Runs `f` with the process umask set to `mask` (permission bits only),
/// and puts the previous umask back afterwards on every path: a normal
/// return, an early return inside `f`, or an unwind out of it.
pub fn with_umask<T>(mask: u32, f: impl FnOnce() -> T) -> T {
    let _serial = serial();
    // Declared after the lock, so dropped before it: the restore runs
    // while this scope still holds the lock.
    let _restore = Restore(rustix::process::umask(Mode::from_bits_truncate(mask)));
    f()
}

/// The current process umask. Reading it means setting it, so this
/// takes the same lock and puts the value straight back.
#[must_use]
pub fn current_umask() -> u32 {
    let _serial = serial();
    let current = rustix::process::umask(Mode::from_bits_truncate(0o077));
    let _ = rustix::process::umask(current);
    current.bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mask_holds_inside_the_scope_and_is_restored_after() {
        let before = current_umask();
        let inside = with_umask(0o077, || {
            // Observed from inside, without the lock (it is held).
            let seen = rustix::process::umask(Mode::from_bits_truncate(0o077));
            let _ = rustix::process::umask(seen);
            seen.bits()
        });
        assert_eq!(inside, 0o077);
        assert_eq!(current_umask(), before);
        let other = with_umask(0o027, || {
            let seen = rustix::process::umask(Mode::from_bits_truncate(0o027));
            let _ = rustix::process::umask(seen);
            seen.bits()
        });
        assert_eq!(other, 0o027);
        assert_eq!(current_umask(), before);
    }

    #[test]
    fn the_scope_returns_what_its_closure_returns() {
        assert_eq!(with_umask(0o077, || 7), 7);
        let early: Result<(), &str> = with_umask(0o077, || Err("early"));
        assert_eq!(early, Err("early"));
    }

    #[test]
    fn an_unwind_out_of_the_scope_still_restores_the_mask() {
        let before = current_umask();
        let unwound = std::panic::catch_unwind(|| {
            with_umask(0o077, || std::panic::resume_unwind(Box::new("boom")));
        });
        assert!(unwound.is_err());
        assert_eq!(current_umask(), before, "the unwind restored the mask");
        // And the lock is usable afterwards.
        assert_eq!(with_umask(0o077, || 1), 1);
    }

    #[test]
    fn concurrent_scopes_never_leave_a_scope_mask_behind() {
        let before = current_umask();
        std::thread::scope(|scope| {
            for mask in [0o077, 0o027, 0o077, 0o027, 0o077, 0o027, 0o077, 0o027] {
                scope.spawn(move || {
                    for _ in 0..200 {
                        with_umask(mask, || std::hint::black_box(()));
                    }
                });
            }
        });
        assert_eq!(current_umask(), before);
    }

    /// The kernel's own view of the mask, from `/proc/self/status`.
    fn proc_umask() -> u32 {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        let line = status
            .lines()
            .find_map(|line| line.strip_prefix("Umask:"))
            .unwrap();
        u32::from_str_radix(line.trim(), 8).unwrap()
    }

    #[test]
    fn current_umask_reports_what_the_kernel_holds() {
        // Read the kernel's value under the lock, so no parallel test's
        // scope is open while it is read.
        let base = {
            let _serial = serial();
            proc_umask()
        };
        assert_eq!(current_umask(), base);
        assert_eq!(with_umask(0o027, proc_umask), 0o027);
        assert_eq!(with_umask(0o077, proc_umask), 0o077);
        assert_eq!(current_umask(), base);
    }
}
