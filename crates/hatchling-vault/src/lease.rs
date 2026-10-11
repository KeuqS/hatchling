//! The DEK's lifetime in memory ( phase E): the lingering-secret
//! trap.
//!
//! A [`KeyLease`] holds the unlocked DEK for as long as it is used, and no
//! longer. [`KeyLease::lock`] zeroizes the key bytes in place at once, and
//! the lease locks itself the first time it is asked for the key after
//! being idle longer than its timeout. A locked lease refuses every use:
//! the vault must be unlocked again, through its tier. Dropping the lease
//! zeroizes the key too (`MasterKey` is `ZeroizeOnDrop`).
//!
//! Secrets fetched with the key are themselves zeroized on drop
//! ([`crate::Secret`]), so plaintext lives only as long as the code that
//! asked for it holds it.

use std::time::{Duration, Instant};

use zeroize::Zeroize as _;

use crate::{Error, MasterKey};

/// The unlocked DEK, held for a limited time.
pub struct KeyLease {
    key: MasterKey,
    locked: bool,
    idle_limit: Option<Duration>,
    last_used: Instant,
}

impl KeyLease {
    /// Holds `key`, locking after `idle_limit` without use (`None`: only
    /// when [`lock`](Self::lock) is called, or the lease dropped).
    #[must_use]
    pub fn new(key: MasterKey, idle_limit: Option<Duration>) -> Self {
        Self {
            key,
            locked: false,
            idle_limit,
            last_used: Instant::now(),
        }
    }

    /// Runs `use_key` with the key, at `now`, unless the lease is locked or
    /// has been idle past its limit, in which case it locks (zeroizing the
    /// key) and refuses.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Locked`] once the lease is locked.
    pub fn with_key<T>(
        &mut self,
        now: Instant,
        use_key: impl FnOnce(&MasterKey) -> T,
    ) -> Result<T, Error> {
        if self
            .idle_limit
            .is_some_and(|limit| now.saturating_duration_since(self.last_used) > limit)
        {
            self.lock();
        }
        if self.locked {
            return Err(Error::Locked);
        }
        self.last_used = now;
        Ok(use_key(&self.key))
    }

    /// Locks the lease: the key bytes are zeroized now, not at drop.
    pub fn lock(&mut self) {
        self.key.zeroize();
        self.locked = true;
    }

    /// Whether the lease is locked.
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.locked
    }
}

impl std::fmt::Debug for KeyLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyLease")
            .field("locked", &self.locked)
            .field("idle_limit", &self.idle_limit)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locking_zeroizes_the_key_at_once_and_refuses_every_use_after() {
        let mut lease = KeyLease::new(MasterKey::from_bytes([7; 32]), None);
        let now = Instant::now();
        assert_eq!(
            lease
                .with_key(now, |key| key.expose().first().copied())
                .unwrap(),
            Some(7)
        );
        assert!(!lease.is_locked());
        lease.lock();
        assert!(lease.is_locked());
        assert_eq!(
            lease.key.expose(),
            &[0u8; 32],
            "the bytes are gone, not just unreachable"
        );
        assert!(matches!(lease.with_key(now, |_| ()), Err(Error::Locked)));
        lease.lock();
        assert!(lease.is_locked(), "locking twice is fine");
        assert_eq!(
            format!("{lease:?}"),
            "KeyLease { locked: true, idle_limit: None, .. }"
        );
    }

    #[test]
    fn an_idle_lease_locks_itself_and_use_keeps_it_open() {
        let start = Instant::now();
        let limit = Duration::from_secs(60);
        let mut lease = KeyLease::new(MasterKey::from_bytes([9; 32]), Some(limit));
        lease.last_used = start;
        // Used within the limit, again and again: it stays open, each use resetting the clock.
        for step in 1..=3 {
            let at = lease.last_used + limit / 2;
            assert!(lease.with_key(at, |_| ()).is_ok(), "step {step}");
        }
        assert!(
            lease.last_used > start + limit,
            "three half-limits later, still open"
        );
        let last = lease.last_used;
        assert!(
            lease.with_key(last + limit, |_| ()).is_ok(),
            "exactly at the limit is still in time"
        );
        let last = lease.last_used;
        assert!(matches!(
            lease.with_key(last + limit + Duration::from_secs(1), |_| ()),
            Err(Error::Locked)
        ));
        assert!(lease.is_locked());
        assert_eq!(lease.key.expose(), &[0u8; 32]);
        // A clock that runs backwards does not unlock or lock anything.
        let mut steady = KeyLease::new(MasterKey::from_bytes([1; 32]), Some(limit));
        let later = steady.last_used + limit;
        assert!(steady.with_key(later, |_| ()).is_ok());
        assert!(
            steady
                .with_key(later.checked_sub(limit).unwrap(), |_| ())
                .is_ok()
        );
    }
}
