//! A run's step ledger: the steps it may still take, shared
//! between the loop and the tools that delegate.
//!
//! Steps only ever leave the ledger. A model turn spends one; giving a
//! child agent a budget spends that many at once, whether or not the
//! child uses them. Nothing comes back, so a run cannot mint budget by
//! spawning children, or launder it through serial ones: everything its
//! children do counts against what it was given.

use std::sync::atomic::{AtomicU32, Ordering};

/// The steps a run may still take. Only ever decreases.
#[derive(Debug)]
pub struct StepLedger {
    remaining: AtomicU32,
}

impl StepLedger {
    /// A ledger holding `steps`.
    #[must_use]
    pub const fn new(steps: u32) -> Self {
        Self {
            remaining: AtomicU32::new(steps),
        }
    }

    /// The steps left.
    #[must_use]
    pub fn remaining(&self) -> u32 {
        self.remaining.load(Ordering::Acquire)
    }

    /// Spends one step on a model turn: `false` when none is left.
    #[must_use]
    pub fn take_step(&self) -> bool {
        self.allocate(1).is_ok()
    }

    /// Spends `steps` at once, on a child's budget: all of them or none.
    ///
    /// # Errors
    ///
    /// The steps left, when they cannot cover `steps`; nothing is spent
    /// then.
    pub fn allocate(&self, steps: u32) -> Result<(), u32> {
        self.remaining
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |left| {
                left.checked_sub(steps)
            })
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_only_leave_and_an_allocation_is_all_or_nothing() {
        let ledger = StepLedger::new(5);
        assert_eq!(ledger.remaining(), 5);
        assert!(ledger.take_step());
        assert_eq!(ledger.remaining(), 4);
        assert_eq!(ledger.allocate(5), Err(4));
        assert_eq!(ledger.remaining(), 4);
        assert_eq!(ledger.allocate(4), Ok(()));
        assert_eq!(ledger.remaining(), 0);
        assert!(!ledger.take_step());
        assert_eq!(ledger.allocate(1), Err(0));
        assert_eq!(ledger.allocate(0), Ok(()));
        assert_eq!(ledger.remaining(), 0);
    }
}
