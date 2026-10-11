//! The broker's connection cap.
//!
//! The kernel peer gate bounds *who* may connect: the one authorized
//! uid. [`ConnectionSlots`] bounds *how many* connections that uid may
//! hold at once, so a misbehaving or tight-looping peer cannot exhaust
//! the broker's threads.
//!
//! Policy: **refuse, silently.** A connection accepted past the cap is
//! closed at once, before a byte is read or written, which is the same
//! posture as an unauthorized peer. It learns nothing about the cap or
//! the load. The accept loop never blocks on a full cap, so it keeps
//! accepting and refusing, and admits again the moment a slot frees.
//! One warning is logged per saturation episode, not one per refusal.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A fixed number of connection slots, taken and released exactly.
#[derive(Debug)]
pub struct ConnectionSlots {
    max: usize,
    in_flight: AtomicUsize,
    /// Whether the last attempt was refused: set by a refusal, cleared
    /// by an admission, so each run of refusals is one episode.
    saturated: AtomicBool,
}

/// One held slot. Dropping it releases the slot.
#[derive(Debug)]
pub struct Slot<'a>(&'a ConnectionSlots);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A refused connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused {
    /// The first refusal since the last admission: the one to log.
    pub first_of_episode: bool,
}

impl ConnectionSlots {
    /// `max` slots, all free.
    #[must_use]
    pub const fn new(max: usize) -> Self {
        Self {
            max,
            in_flight: AtomicUsize::new(0),
            saturated: AtomicBool::new(false),
        }
    }

    /// Takes a slot, if one is free.
    ///
    /// The check and the take are one atomic step, so no two callers can
    /// both take the last slot.
    ///
    /// # Errors
    ///
    /// Returns [`Refused`] when all `max` slots are held.
    pub fn try_acquire(&self) -> Result<Slot<'_>, Refused> {
        let taken = self
            .in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                (held < self.max).then_some(held + 1)
            })
            .is_ok();
        if taken {
            self.saturated.store(false, Ordering::Release);
            return Ok(Slot(self));
        }
        Err(Refused {
            first_of_episode: !self.saturated.swap(true, Ordering::AcqRel),
        })
    }

    /// How many slots are held right now.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Acquire)
    }

    /// The cap.
    #[must_use]
    pub const fn max(&self) -> usize {
        self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cap_of_n_admits_n_and_refuses_the_next() {
        let slots = ConnectionSlots::new(3);
        assert_eq!(slots.max(), 3);
        let held: Vec<Slot<'_>> = (0..3).map(|_| slots.try_acquire().unwrap()).collect();
        assert_eq!(slots.in_flight(), 3);
        assert!(slots.try_acquire().is_err());
        assert_eq!(slots.in_flight(), 3, "a refusal takes nothing");
        drop(held);
        assert_eq!(slots.in_flight(), 0, "every slot came back");
    }

    #[test]
    fn a_released_slot_admits_again() {
        let slots = ConnectionSlots::new(1);
        let first = slots.try_acquire().unwrap();
        assert!(slots.try_acquire().is_err());
        drop(first);
        let second = slots.try_acquire().unwrap();
        assert_eq!(slots.in_flight(), 1);
        drop(second);
        assert_eq!(slots.in_flight(), 0);
    }

    #[test]
    fn a_zero_cap_admits_nothing() {
        let slots = ConnectionSlots::new(0);
        assert!(slots.try_acquire().is_err());
        assert_eq!(slots.in_flight(), 0);
    }

    #[test]
    fn one_warning_per_saturation_episode() {
        let slots = ConnectionSlots::new(1);
        let held = slots.try_acquire().unwrap();
        assert_eq!(
            slots.try_acquire().err(),
            Some(Refused {
                first_of_episode: true
            })
        );
        for _ in 0..3 {
            assert_eq!(
                slots.try_acquire().err(),
                Some(Refused {
                    first_of_episode: false
                }),
                "later refusals in the episode are quiet"
            );
        }
        drop(held);
        let again = slots.try_acquire().unwrap();
        assert_eq!(
            slots.try_acquire().err(),
            Some(Refused {
                first_of_episode: true
            }),
            "an admission ends the episode"
        );
        drop(again);
    }

    #[test]
    fn concurrent_takers_never_exceed_the_cap() {
        let slots = ConnectionSlots::new(5);
        let barrier = std::sync::Barrier::new(32);
        let admitted = std::thread::scope(|scope| {
            #[allow(
                clippy::needless_collect,
                reason = "every taker must be spawned before any is joined, or the barrier never fills"
            )]
            let takers: Vec<_> = (0..32)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        let slot = slots.try_acquire().ok();
                        // Hold every slot until all have tried.
                        barrier.wait();
                        slot.is_some()
                    })
                })
                .collect();
            takers
                .into_iter()
                .map(std::thread::ScopedJoinHandle::join)
                .filter(|taken| matches!(taken, Ok(true)))
                .count()
        });
        assert_eq!(admitted, 5);
        assert_eq!(slots.in_flight(), 0);
    }
}
