//! Two `CLOEXEC` pipes for the broker/child spawn handshake.
//!
//! The spawn protocol needs exactly three messages: the child signals
//! readiness (`R`) after entering its namespaces, the parent releases
//! it (`D`) after writing the id maps, and a failing child reports the
//! reason (`E` + message) before exiting.
//!
//! These travel on two *unidirectional* pipes, not one bidirectional
//! pipe: with a single pipe both ends can read, so the child could
//! consume its own `R` byte before the parent reads it (or the parent
//! could consume the child's failure report). Two pipes make each
//! direction unambiguous:
//!
//! - [`ChildToParent`]: child writes `R` / `E...`; parent reads.
//! - [`ParentToChild`]: parent writes `D`; child reads.
//!
//! Both pipes are created `CLOEXEC` so the exec'd agent image closes
//! them; the parent reads the child→parent pipe to EOF after the cell
//! exits to collect any failure report.
//!
//! Ownership across `fork`: parent and child each hold a copy of the
//! pair. Each side explicitly closes the ends it no longer needs
//! ([`SignalPipe::close_read`]/[`close_write`]); the `Option` fields
//! make a double close impossible within one process.

use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use crate::Error;

/// Whether a `poll` `revents` bitmask means the pipe is readable.
///
/// Pure and table-tested: `POLLIN` means data is available; `POLLHUP`
/// means every writer closed, so the next read returns EOF; `POLLERR`
/// is treated like readable so the read surfaces the real error
/// instead of spinning. Anything else — including a spurious wake
/// with no bits, or `POLLOUT`/`POLLNVAL` — means "not ready".
#[must_use]
const fn readiness(revents: i16) -> bool {
    revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
}

/// How [`SignalPipe::wait_readable`] classifies a failed `poll(2)`.
///
/// Pure and table-tested: extracting the `EINTR` check from the poll
/// loop lets a table test pin the `==`/`!=` mutants on the
/// `Interrupted` classification, which the real-signal integration
/// test cannot time reliably.
#[derive(Debug)]
enum PollStep {
    /// `poll` was interrupted by a signal: retry against the deadline.
    Retry,
    /// `poll` failed: fail closed with the errno.
    Fatal(io::Error),
}

/// Classifies a `poll(2)` error for [`SignalPipe::wait_readable`].
#[must_use]
fn poll_error_step(error: io::Error) -> PollStep {
    if error.kind() == io::ErrorKind::Interrupted {
        PollStep::Retry
    } else {
        PollStep::Fatal(error)
    }
}

/// One end-pair of the spawn handshake pipe.
#[derive(Debug)]
pub struct SignalPipe {
    rd: Option<OwnedFd>,
    wr: Option<OwnedFd>,
}

impl SignalPipe {
    /// Creates the pipe. Both ends are `CLOEXEC`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if `pipe2` fails.
    fn new() -> Result<Self, Error> {
        let [rd_raw, wr_raw] = {
            let mut fds: [libc::c_int; 2] = [0, 0];
            // SAFETY: `pipe2` fills both slots on success; the only flag
            // is `O_CLOEXEC`.
            let ret = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
            if ret != 0 {
                return Err(Error::Process {
                    operation: "pipe2",
                    source: io::Error::last_os_error(),
                });
            }
            fds
        };
        // The read end is non-blocking: every read is preceded by
        // `wait_readable` (poll), so a blocking read would only hang if
        // the poll was skipped or lied. Non-blocking converts that
        // hang into an immediate `EAGAIN`, which `read_byte_deadline`
        // treats as a fail-closed error. (Mutant:
        // `wait_readable → Ok(true)`.)
        // SAFETY: `fcntl` with `F_GETFL` on a freshly created pipe fd
        // we own; the flag word is only read here.
        let flags = unsafe { libc::fcntl(rd_raw, libc::F_GETFL) };
        if flags < 0 {
            // SAFETY: `close` on freshly created fds we own; they have
            // not been wrapped yet, so this is the only owner.
            unsafe { libc::close(rd_raw) };
            // SAFETY: same ownership argument, for the write end.
            unsafe { libc::close(wr_raw) };
            return Err(Error::Process {
                operation: "fcntl F_GETFL",
                source: io::Error::last_os_error(),
            });
        }
        // SAFETY: `fcntl` with `F_SETFL` on a fd we own; `O_NONBLOCK`
        // is a valid file-status flag, and the read-modify-write
        // preserves all other flags.
        if unsafe { libc::fcntl(rd_raw, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            // SAFETY: `close` on freshly created fds we own; they have
            // not been wrapped yet, so this is the only owner.
            unsafe { libc::close(rd_raw) };
            // SAFETY: same ownership argument, for the write end.
            unsafe { libc::close(wr_raw) };
            return Err(Error::Process {
                operation: "fcntl F_SETFL O_NONBLOCK",
                source: io::Error::last_os_error(),
            });
        }
        // SAFETY: each fd is freshly owned by this process and wrapped
        // exactly once; the `Option` fields below are the only owners.
        let rd = unsafe { OwnedFd::from_raw_fd(rd_raw) };
        // SAFETY: same ownership argument, for the write end.
        let wr = unsafe { OwnedFd::from_raw_fd(wr_raw) };
        Ok(Self {
            rd: Some(rd),
            wr: Some(wr),
        })
    }

    /// Writes one byte. Used for the `R`/`D` handshake bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the write end is closed or the
    /// write fails.
    pub fn write_byte(&self, byte: u8) -> Result<(), Error> {
        self.write_all(std::slice::from_ref(&byte))
    }

    /// Writes the whole buffer, retrying short writes and `EINTR`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the write end is closed or a write
    /// fails unrecoverably.
    pub fn write_all(&self, mut buf: &[u8]) -> Result<(), Error> {
        let wr = self.wr.as_ref().ok_or_else(|| Error::Process {
            operation: "pipe write",
            source: io::Error::new(io::ErrorKind::BrokenPipe, "write end is closed"),
        })?;
        while !buf.is_empty() {
            // `rustix::io::write` retries nothing itself; the loop here
            // handles short writes. `EINTR` surfaces as
            // `Errno::INTR`, which is retryable, not fatal.
            match rustix::io::write(wr, buf) {
                Ok(0) => {
                    return Err(Error::Process {
                        operation: "pipe write",
                        source: io::Error::new(
                            io::ErrorKind::WriteZero,
                            "pipe write returned zero",
                        ),
                    });
                }
                Ok(n) => {
                    let (_, rest) = buf.split_at(n);
                    buf = rest;
                }
                Err(rustix::io::Errno::INTR) => {}
                Err(source) => {
                    return Err(Error::Process {
                        operation: "pipe write",
                        source: io::Error::from_raw_os_error(source.raw_os_error()),
                    });
                }
            }
        }
        Ok(())
    }

    /// Reads one byte. `Ok(None)` is EOF — the peer died or closed its
    /// end without writing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the read end is closed or a read
    /// fails unrecoverably.
    pub fn read_byte(&self) -> Result<Option<u8>, Error> {
        let rd = self.rd.as_ref().ok_or_else(|| Error::Process {
            operation: "pipe read",
            source: io::Error::new(io::ErrorKind::BrokenPipe, "read end is closed"),
        })?;
        let mut byte = [0u8; 1];
        loop {
            match rustix::io::read(rd, &mut byte) {
                Ok(0) => return Ok(None),
                Ok(_) => return Ok(Some(byte[0])),
                Err(rustix::io::Errno::INTR) => {}
                Err(source) => {
                    return Err(Error::Process {
                        operation: "pipe read",
                        source: io::Error::from_raw_os_error(source.raw_os_error()),
                    });
                }
            }
        }
    }

    /// Reads one byte, waiting at most `timeout` for it. `Ok(None)` is
    /// EOF — the peer died or closed its end without writing.
    ///
    /// Unbounded pipe reads are how a stuck child hangs the broker
    /// forever: every production read path uses this or
    /// [`SignalPipe::read_to_end_deadline`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProcessTimeout`] if no byte (or EOF) arrives
    /// within `timeout`, or [`Error::Process`] if the read end is
    /// closed or a read fails unrecoverably.
    pub fn read_byte_deadline(&self, timeout: Duration) -> Result<Option<u8>, Error> {
        let rd = self.rd.as_ref().ok_or_else(|| Error::Process {
            operation: "pipe read",
            source: io::Error::new(io::ErrorKind::BrokenPipe, "read end is closed"),
        })?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| Error::Process {
                operation: "pipe read deadline",
                source: io::Error::new(io::ErrorKind::InvalidInput, "deadline overflowed"),
            })?;
        loop {
            if !Self::wait_readable(rd, deadline)? {
                return Err(Error::ProcessTimeout {
                    operation: "pipe read_byte",
                    timeout,
                });
            }
            let mut byte = [0u8; 1];
            match rustix::io::read(rd, &mut byte) {
                Ok(0) => return Ok(None),
                Ok(_) => return Ok(Some(byte[0])),
                // The fd was readable but the read was interrupted:
                // re-poll with the remaining budget.
                Err(rustix::io::Errno::INTR) => {}
                // The read end is non-blocking, so `EAGAIN` means the
                // preceding `wait_readable` reported readiness that was
                // not there (or was skipped). Fail closed instead of
                // spinning: looping here would busy-wait under the
                // `wait_readable → Ok(true)` mutant.
                Err(rustix::io::Errno::AGAIN) => {
                    return Err(Error::Process {
                        operation: "pipe read",
                        source: io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "read end reported readable but had no data",
                        ),
                    });
                }
                Err(source) => {
                    return Err(Error::Process {
                        operation: "pipe read",
                        source: io::Error::from_raw_os_error(source.raw_os_error()),
                    });
                }
            }
        }
    }

    /// Waits until `rd` is readable (data or EOF) or `deadline`
    /// passes. Returns `true` when a `read` will not block.
    ///
    /// `EINTR` retries against the same deadline, so a signal storm
    /// cannot stretch the wait past the bound.
    fn wait_readable(rd: &OwnedFd, deadline: Instant) -> Result<bool, Error> {
        // SAFETY: `pfd` is a valid single `pollfd`; `poll` writes only
        // `revents`. The fd is borrowed from `rd`, which outlives the
        // call.
        let mut pfd = libc::pollfd {
            fd: rd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Ok(false);
            }
            // `poll` takes milliseconds; saturate rather than wrap,
            // and never pass 0 (a zero timeout would report "not
            // ready" up to a millisecond early).
            let ms = i32::try_from((deadline - now).as_millis())
                .unwrap_or(i32::MAX)
                .max(1);
            // SAFETY: `pfd` is a valid single `pollfd`; `poll` writes
            // only `revents`. The fd is borrowed from `rd`, which
            // outlives the call.
            let ret = unsafe { libc::poll(&raw mut pfd, 1, ms) };
            if ret < 0 {
                match poll_error_step(io::Error::last_os_error()) {
                    PollStep::Retry => continue,
                    PollStep::Fatal(source) => {
                        return Err(Error::Process {
                            operation: "pipe poll",
                            source,
                        });
                    }
                }
            }
            if ret == 0 {
                // `poll` may return up to a millisecond before its
                // timeout elapses (timer granularity): loop and let the
                // deadline check at the top decide, so a timeout is
                // never reported before the deadline passes.
                continue;
            }
            // `POLLIN`: data; `POLLHUP`: every writer closed, so the
            // next read returns EOF; `POLLERR`: treat like readable so
            // the read surfaces the real error instead of spinning.
            if readiness(pfd.revents) {
                return Ok(true);
            }
            // Woke without a readable bit (spurious): re-poll against
            // the unchanged deadline.
        }
    }

    /// Reads to EOF, collecting every byte. The parent uses this after
    /// the cell exits to collect a child's `E`-report, if any. Read
    /// errors end collection; a partial report is still a report.
    #[must_use]
    pub fn read_to_end(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let Some(rd) = self.rd.as_ref() else {
            return out;
        };
        let mut buf = [0u8; 1024];
        loop {
            match rustix::io::read(rd, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let (chunk, _) = buf.split_at(n);
                    out.extend_from_slice(chunk);
                }
                Err(rustix::io::Errno::INTR) => {}
                Err(_) => break,
            }
        }
        out
    }

    /// Reads to EOF, collecting every byte. The parent uses this after
    /// the cell exits to collect a child's `E`-report, if any.
    ///
    /// Bounded sibling of [`SignalPipe::read_to_end`]: returns
    /// [`Error::ProcessTimeout`] instead of hanging when EOF never
    /// arrives. Read errors end collection (a partial report is still
    /// a report); only the timeout is loud, because a missing EOF
    /// after the child is gone means a leaked writer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProcessTimeout`] if EOF is not reached within
    /// `timeout`, or [`Error::Process`] if the read end is closed.
    pub fn read_to_end_deadline(&self, timeout: Duration) -> Result<Vec<u8>, Error> {
        let rd = self.rd.as_ref().ok_or_else(|| Error::Process {
            operation: "pipe read",
            source: io::Error::new(io::ErrorKind::BrokenPipe, "read end is closed"),
        })?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| Error::Process {
                operation: "pipe read deadline",
                source: io::Error::new(io::ErrorKind::InvalidInput, "deadline overflowed"),
            })?;
        let mut out = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            if !Self::wait_readable(rd, deadline)? {
                return Err(Error::ProcessTimeout {
                    operation: "pipe read_to_end",
                    timeout,
                });
            }
            match rustix::io::read(rd, &mut buf) {
                Ok(0) => return Ok(out),
                Ok(n) => {
                    let (chunk, _) = buf.split_at(n);
                    out.extend_from_slice(chunk);
                }
                Err(rustix::io::Errno::INTR) => {}
                Err(_) => return Ok(out),
            }
        }
    }

    /// Closes this process's copy of the read end. Idempotent.
    /// Private: external users go through [`ChildToParent`] or
    /// [`ParentToChild`], which expose only the ends their direction
    /// needs.
    fn close_read(&mut self) {
        self.rd.take();
    }

    /// Closes this process's copy of the write end. Idempotent.
    /// Private: see [`SignalPipe::close_read`].
    fn close_write(&mut self) {
        self.wr.take();
    }
}

/// Child -> parent direction: the child writes `R` (ready) or
/// `E<message>` (setup failure); the parent reads.
#[derive(Debug)]
pub struct ChildToParent(SignalPipe);

/// Parent -> child direction: the parent writes `D` (release into
/// mount construction); the child reads.
#[derive(Debug)]
pub struct ParentToChild(SignalPipe);

/// Creates the two handshake pipes. Both ends of both pipes are
/// `CLOEXEC`.
///
/// # Errors
///
/// Returns [`Error::Process`] if `pipe2` fails.
pub fn handshake_pipes() -> Result<(ChildToParent, ParentToChild), Error> {
    Ok((
        ChildToParent(SignalPipe::new()?),
        ParentToChild(SignalPipe::new()?),
    ))
}

impl ChildToParent {
    /// The pipe ends this process still holds: for fd hygiene, the
    /// descriptors a process keeps.
    pub fn fds(&self) -> impl Iterator<Item = BorrowedFd<'_>> {
        self.0.rd.iter().chain(self.0.wr.iter()).map(AsFd::as_fd)
    }

    /// Writes one byte. Used for the `R` ready byte.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the write end is closed or the
    /// write fails.
    pub fn write_byte(&self, byte: u8) -> Result<(), Error> {
        self.0.write_byte(byte)
    }

    /// Writes the whole buffer, retrying short writes and `EINTR`.
    /// Used for `E<message>` failure reports.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the write end is closed or a write
    /// fails unrecoverably.
    pub fn write_all(&self, buf: &[u8]) -> Result<(), Error> {
        self.0.write_all(buf)
    }

    /// Reads one byte. `Ok(None)` is EOF — the peer died or closed its
    /// end without writing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the read end is closed or a read
    /// fails unrecoverably.
    pub fn read_byte(&self) -> Result<Option<u8>, Error> {
        self.0.read_byte()
    }

    /// Reads one byte, waiting at most `timeout`. See
    /// [`SignalPipe::read_byte_deadline`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProcessTimeout`] if no byte (or EOF) arrives
    /// within `timeout`, or [`Error::Process`] if the read end is
    /// closed or a read fails unrecoverably.
    pub fn read_byte_deadline(&self, timeout: Duration) -> Result<Option<u8>, Error> {
        self.0.read_byte_deadline(timeout)
    }

    /// Reads to EOF, collecting every byte. The parent uses this after
    /// the cell exits to collect a child's `E`-report, if any.
    #[must_use]
    pub fn read_to_end(&self) -> Vec<u8> {
        self.0.read_to_end()
    }

    /// Bounded sibling of [`ChildToParent::read_to_end`]. See
    /// [`SignalPipe::read_to_end_deadline`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProcessTimeout`] if EOF is not reached within
    /// `timeout`, or [`Error::Process`] if the read end is closed.
    pub fn read_to_end_deadline(&self, timeout: Duration) -> Result<Vec<u8>, Error> {
        self.0.read_to_end_deadline(timeout)
    }

    /// Closes this process's copy of the read end. Idempotent.
    /// The parent calls this once it has the ready byte; the child
    /// calls it when it no longer reports failures.
    pub fn close_read(&mut self) {
        self.0.close_read();
    }

    /// Closes this process's copy of the write end. Idempotent.
    /// The child calls this after reporting readiness; the parent
    /// calls it on spawn failure.
    pub fn close_write(&mut self) {
        self.0.close_write();
    }
}

impl ParentToChild {
    /// Writes one byte. Used for the `D` release byte.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the write end is closed or the
    /// write fails.
    pub fn write_byte(&self, byte: u8) -> Result<(), Error> {
        self.0.write_byte(byte)
    }

    /// Reads one byte. `Ok(None)` is EOF — the peer died or closed its
    /// end without writing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Process`] if the read end is closed or a read
    /// fails unrecoverably.
    pub fn read_byte(&self) -> Result<Option<u8>, Error> {
        self.0.read_byte()
    }

    /// Reads one byte, waiting at most `timeout`. See
    /// [`SignalPipe::read_byte_deadline`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProcessTimeout`] if no byte (or EOF) arrives
    /// within `timeout`, or [`Error::Process`] if the read end is
    /// closed or a read fails unrecoverably.
    pub fn read_byte_deadline(&self, timeout: Duration) -> Result<Option<u8>, Error> {
        self.0.read_byte_deadline(timeout)
    }

    /// Closes this process's copy of the read end. Idempotent.
    /// The child calls this once released; the parent calls it on
    /// spawn failure.
    pub fn close_read(&mut self) {
        self.0.close_read();
    }

    /// Closes this process's copy of the write end. Idempotent.
    /// The parent calls this after releasing the child; the child
    /// never writes on this pipe.
    pub fn close_write(&mut self) {
        self.0.close_write();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_pipe_names_exactly_the_ends_it_still_holds() {
        let (mut report, _release) = handshake_pipes().unwrap();
        let both: Vec<i32> = report.fds().map(|fd| fd.as_raw_fd()).collect();
        assert_eq!(both.len(), 2);
        assert_ne!(both.first(), both.last());
        report.close_read();
        assert_eq!(report.fds().count(), 1);
        report.close_write();
        assert_eq!(report.fds().count(), 0);
    }

    /// Waits, at most 10 s, for EOF on a pipe whose write end this test
    /// closed. Another test forking in this process meanwhile (many do)
    /// leaves its child a copy of the write end until it exits or execs;
    /// until then the non-blocking read end says `EAGAIN`, not EOF. EOF
    /// reads consume nothing, so the plain read after this still sees it.
    fn await_eof(read: impl Fn(Duration) -> Result<Option<u8>, Error>) {
        assert_eq!(read(Duration::from_secs(10)).unwrap(), None);
    }

    /// The directionality pin: the two pipes are independent. Bytes
    /// written on one are never visible on the other — this is what
    /// makes the child unable to consume its own `R` byte and the
    /// parent unable to consume the child's failure report.
    #[test]
    fn pipes_are_independent_directions() {
        let (mut c2p, mut p2c) = handshake_pipes().unwrap();
        // Child -> parent: the parent reads what the child wrote.
        c2p.write_byte(b'R').unwrap();
        assert_eq!(c2p.read_byte().unwrap(), Some(b'R'));
        // Parent -> child: the child reads what the parent wrote.
        p2c.write_byte(b'D').unwrap();
        assert_eq!(p2c.read_byte().unwrap(), Some(b'D'));
        // Closing one pipe's write end EOFs only that pipe.
        p2c.close_write();
        await_eof(|timeout| p2c.read_byte_deadline(timeout));
        assert_eq!(p2c.read_byte().unwrap(), None);
        c2p.write_byte(b'E').unwrap();
        assert_eq!(c2p.read_byte().unwrap(), Some(b'E'));
        c2p.close_read();
        c2p.close_write();
    }

    #[test]
    fn read_after_close_is_eof() {
        let (mut c2p, _) = handshake_pipes().unwrap();
        c2p.close_write();
        await_eof(|timeout| c2p.read_byte_deadline(timeout));
        assert_eq!(c2p.read_byte().unwrap(), None);
        assert!(c2p.read_to_end().is_empty());
    }

    #[test]
    fn write_after_close_is_an_error() {
        let (mut c2p, _) = handshake_pipes().unwrap();
        c2p.close_write();
        let err = c2p.write_byte(b'R').unwrap_err();
        assert!(matches!(err, Error::Process { .. }));
    }

    #[test]
    fn read_after_close_read_is_an_error() {
        let (mut c2p, _) = handshake_pipes().unwrap();
        c2p.close_read();
        let err = c2p.read_byte().unwrap_err();
        assert!(matches!(err, Error::Process { .. }));
    }

    #[test]
    fn close_is_idempotent() {
        let (mut c2p, mut p2c) = handshake_pipes().unwrap();
        c2p.close_read();
        c2p.close_read();
        c2p.close_write();
        c2p.close_write();
        p2c.close_read();
        p2c.close_write();
    }

    #[test]
    fn write_all_round_trips_a_report() {
        let (mut c2p, _) = handshake_pipes().unwrap();
        let report = b"Eexec agent: not found";
        c2p.write_all(report).unwrap();
        c2p.close_write();
        assert_eq!(c2p.read_to_end(), report);
    }

    #[test]
    fn parent_to_child_close_read_closes_the_read_end() {
        // Mutant: ParentToChild::close_read replaced with (). The
        // existing close_read tests only cover ChildToParent; this
        // pins the ParentToChild direction: after close_read, reading
        // must fail (not return data or block).
        let (_, mut p2c) = handshake_pipes().unwrap();
        p2c.close_read();
        let err = p2c.read_byte().unwrap_err();
        assert!(matches!(err, Error::Process { .. }));
    }

    #[test]
    fn child_to_parent_close_read_drops_the_read_fd() {
        // Mutants: `SignalPipe::close_read` / `ChildToParent::close_read`
        // replaced with `()`. Observe the fd state directly instead of
        // writing to a readerless pipe: a write would raise `SIGPIPE`,
        // which needs process-global disposition to assert on, and
        // reading after close only proves the `None` check fires — the
        // mutant's still-open read end would also fail a read with
        // `EAGAIN`. A surviving read end stays `Some`; only a real
        // close makes it `None`.
        let (mut c2p, _p2c) = handshake_pipes().unwrap();
        assert!(c2p.0.rd.is_some());
        c2p.close_read();
        assert!(c2p.0.rd.is_none(), "close_read must drop the read fd");
    }

    #[test]
    fn parent_to_child_close_read_drops_the_read_fd() {
        // Same mutant class as above, for `ParentToChild::close_read`:
        // the read fd must be gone from the struct after the call.
        let (_c2p, mut p2c) = handshake_pipes().unwrap();
        assert!(p2c.0.rd.is_some());
        p2c.close_read();
        assert!(p2c.0.rd.is_none(), "close_read must drop the read fd");
    }

    #[test]
    fn read_byte_deadline_returns_a_ready_byte() {
        let (c2p, _) = handshake_pipes().unwrap();
        c2p.write_byte(b'R').unwrap();
        assert_eq!(
            c2p.read_byte_deadline(Duration::from_secs(5)).unwrap(),
            Some(b'R')
        );
    }

    #[test]
    fn read_byte_deadline_times_out_on_silence() {
        // The writer stays open but silent: the read must fail with
        // ProcessTimeout after ~the deadline, not hang and not return
        // instantly.
        let (c2p, _) = handshake_pipes().unwrap();
        let timeout = Duration::from_millis(200);
        let start = Instant::now();
        let err = c2p.read_byte_deadline(timeout).unwrap_err();
        assert!(
            matches!(err, Error::ProcessTimeout { .. }),
            "expected ProcessTimeout, got {err:?}"
        );
        assert!(
            start.elapsed() >= timeout,
            "deadline read returned before its timeout"
        );
    }

    #[test]
    fn read_byte_deadline_sees_eof_without_waiting() {
        // Closed writer: EOF must arrive immediately, not after the
        // full timeout. (Mutant: wait_readable's POLLHUP arm removed —
        // poll would sit out the whole deadline on a dead pipe.) The
        // 10 s deadline is short on purpose: under the
        // `close_write → ()` mutant no EOF ever arrives, and the test
        // must fail in seconds, not at cargo-mutants' 32 s timeout.
        let (mut c2p, _) = handshake_pipes().unwrap();
        c2p.close_write();
        let start = Instant::now();
        assert_eq!(
            c2p.read_byte_deadline(Duration::from_secs(10)).unwrap(),
            None
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "EOF took too long: POLLHUP not treated as readable"
        );
    }

    #[test]
    fn parent_to_child_read_byte_deadline_delegates() {
        // Mutants: `ParentToChild::read_byte_deadline` replaced with
        // `Ok(None)` / `Ok(Some(0))` / `Ok(Some(1))`. The
        // `ChildToParent` twin is pinned by the tests above; nothing
        // called this handle's deadline read directly, leaving the
        // one-line delegation to `SignalPipe::read_byte_deadline`
        // uncovered. A distinctive byte kills all three replacements.
        let (_, p2c) = handshake_pipes().unwrap();
        p2c.write_byte(b'D').unwrap();
        assert_eq!(
            p2c.read_byte_deadline(Duration::from_secs(5)).unwrap(),
            Some(b'D')
        );
    }

    #[test]
    fn read_to_end_deadline_collects_then_eofs() {
        let (mut c2p, _) = handshake_pipes().unwrap();
        c2p.write_all(b"Eboom").unwrap();
        c2p.close_write();
        let out = c2p
            .read_to_end_deadline(Duration::from_secs(5))
            .expect("bounded read_to_end");
        assert_eq!(out, b"Eboom");
    }

    #[test]
    fn read_to_end_deadline_times_out_on_a_leaked_writer() {
        // The writer stays open but silent: EOF never arrives. The
        // bounded read must fail loud instead of hanging the broker.
        let (c2p, _) = handshake_pipes().unwrap();
        let timeout = Duration::from_millis(200);
        let start = Instant::now();
        let err = c2p.read_to_end_deadline(timeout).unwrap_err();
        assert!(
            matches!(err, Error::ProcessTimeout { .. }),
            "expected ProcessTimeout, got {err:?}"
        );
        assert!(
            start.elapsed() >= timeout,
            "deadline read returned before its timeout"
        );
    }

    #[test]
    fn readiness_interprets_revents_bits() {
        // Table test over the `poll` revents bitmask: kills the `&`/`|`
        // mutants in the mask, the `!=`/`==` inversion, and the
        // whole-function `true`/`false` replacements.
        for revents in [libc::POLLIN, libc::POLLHUP, libc::POLLERR] {
            assert!(readiness(revents), "revents {revents} must read as ready");
        }
        assert!(readiness(libc::POLLIN | libc::POLLHUP));
        assert!(readiness(libc::POLLIN | libc::POLLERR));
        // No bits (spurious wake), write-ready, and invalid fd are all
        // "not readable": `POLLOUT` must not count, and `POLLNVAL`
        // must not be mistaken for data.
        for revents in [0, libc::POLLOUT, libc::POLLNVAL] {
            assert!(
                !readiness(revents),
                "revents {revents} must not read as ready"
            );
        }
    }

    #[test]
    fn poll_error_step_classifies() {
        // Table test over the `poll` error classification: `EINTR`
        // must retry, any other errno must fail closed. Pins the
        // `==`/`!=` mutants on the `Interrupted` check in both
        // directions — the real-signal integration test cannot time
        // the race reliably.
        assert!(matches!(
            poll_error_step(io::Error::new(io::ErrorKind::Interrupted, "interrupted")),
            PollStep::Retry
        ));
        assert!(matches!(
            poll_error_step(io::Error::other("boom")),
            PollStep::Fatal(_)
        ));
    }
}
