//! Space admission: what is checked, factor by factor.
//!
//! `Join` admits the session's principal to one space and mints a ticket.
//! Every later op on that space presents the ticket, and [`Admissions::check`]
//! re-checks every factor, in this order, on every call:
//!
//! | Factor | What is checked | Refusal |
//! | --- | --- | --- |
//! | Possession | The ticket is one the broker minted and still holds | `not-admitted` |
//! | Liveness | It was minted in *this* live session: a ticket from another or an ended session is a replayed or stale credential | `stale-admission` |
//! | Peer pinning | The session's kernel-attested peer (uid and pid) is the one pinned at admission | `peer-changed` |
//! | Freshness | It has not expired ([`ADMISSION_TTL_SECS`]) | `expired` |
//! | Name binding | The space the request names is byte-for-byte the space the ticket admits | `not-admitted` |
//!
//! The broker then re-reads the space and re-checks **membership** of the
//! session's broker-asserted principal: a ticket outlives nothing it was
//! not granted.
//!
//! A session's tickets die with it ([`Admissions::end_session`]), and at
//! most [`MAX_ADMISSIONS_PER_SESSION`] are live per session. A refused
//! factor means joining again, which re-checks membership from scratch:
//! a changed peer re-authenticates.
//!
//! On Linux a connection's peer credentials are fixed when it connects, so
//! within the broker a different peer is always a different session; the
//! pin still holds independently of the session check, so neither factor
//! silently stands in for the other.

use std::collections::BTreeMap;

use hatchling_core::SpaceRefusal;

/// How long an admission lasts, in seconds.
pub const ADMISSION_TTL_SECS: i64 = 15 * 60;

/// Most live admissions one session may hold.
pub const MAX_ADMISSIONS_PER_SESSION: usize = 16;

/// The kernel-attested peer of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerPin {
    /// The peer's uid.
    pub uid: u32,
    /// The peer's pid.
    pub pid: u32,
}

/// The session asking: its broker-minted id and its kernel-attested peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPin {
    /// The broker-minted session id.
    pub session: String,
    /// The session's peer.
    pub peer: PeerPin,
}

/// One live admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    /// The space it admits to.
    pub space: String,
    /// The session it was minted in.
    pub session: String,
    /// The peer it was pinned to.
    pub peer: PeerPin,
    /// When it expires, in Unix seconds.
    pub expires_at: i64,
}

/// Every live admission, by ticket.
#[derive(Debug)]
pub struct Admissions {
    live: BTreeMap<String, Admission>,
}

impl Admissions {
    /// No admissions.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            live: BTreeMap::new(),
        }
    }

    /// Admits `pin`'s session to `space` under `ticket` at `now`, after
    /// the broker has checked membership. Returns the admission.
    ///
    /// # Errors
    ///
    /// [`SpaceRefusal::Full`] when the session already holds
    /// [`MAX_ADMISSIONS_PER_SESSION`].
    pub fn admit(
        &mut self,
        ticket: String,
        space: &str,
        pin: &SessionPin,
        now: i64,
    ) -> Result<Admission, SpaceRefusal> {
        let held = self
            .live
            .values()
            .filter(|admission| admission.session == pin.session)
            .count();
        if held >= MAX_ADMISSIONS_PER_SESSION {
            return Err(SpaceRefusal::Full);
        }
        let admission = Admission {
            space: space.to_owned(),
            session: pin.session.clone(),
            peer: pin.peer,
            expires_at: now.saturating_add(ADMISSION_TTL_SECS),
        };
        self.live.insert(ticket, admission.clone());
        Ok(admission)
    }

    /// Checks every admission factor for `ticket`, presented by `pin`'s
    /// session for `space` at `now` (see the module docs for the order).
    ///
    /// # Errors
    ///
    /// The refusal for the first factor that fails.
    pub fn check(
        &self,
        ticket: &str,
        space: &str,
        pin: &SessionPin,
        now: i64,
    ) -> Result<(), SpaceRefusal> {
        let admission = self.live.get(ticket).ok_or(SpaceRefusal::NotAdmitted)?;
        if admission.session != pin.session {
            return Err(SpaceRefusal::StaleAdmission);
        }
        if admission.peer != pin.peer {
            return Err(SpaceRefusal::PeerChanged);
        }
        if now >= admission.expires_at {
            return Err(SpaceRefusal::Expired);
        }
        if admission.space.as_bytes() != space.as_bytes() {
            return Err(SpaceRefusal::NotAdmitted);
        }
        Ok(())
    }

    /// Drops every admission minted in `session`: it ended.
    pub fn end_session(&mut self, session: &str) {
        self.live
            .retain(|_, admission| admission.session != session);
    }

    /// Drops expired admissions as of `now`.
    pub fn sweep(&mut self, now: i64) {
        self.live.retain(|_, admission| now < admission.expires_at);
    }

    /// How many admissions are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Whether none are.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(session: &str, uid: u32, pid: u32) -> SessionPin {
        SessionPin {
            session: session.to_owned(),
            peer: PeerPin { uid, pid },
        }
    }

    const NOW: i64 = 1_790_000_000;

    #[test]
    fn an_admission_is_good_for_its_space_session_peer_and_lifetime() {
        let mut admissions = Admissions::empty();
        assert!(admissions.is_empty());
        let a = pin("sess-a", 1000, 42);
        let admission = admissions
            .admit(String::from("adm-1"), "spc-1", &a, NOW)
            .unwrap();
        assert_eq!(
            admission,
            Admission {
                space: String::from("spc-1"),
                session: String::from("sess-a"),
                peer: PeerPin { uid: 1000, pid: 42 },
                expires_at: NOW + ADMISSION_TTL_SECS,
            }
        );
        assert_eq!(admissions.len(), 1);
        assert!(!admissions.is_empty());
        assert_eq!(admissions.check("adm-1", "spc-1", &a, NOW), Ok(()));
        assert_eq!(
            admissions.check("adm-1", "spc-1", &a, NOW + ADMISSION_TTL_SECS - 1),
            Ok(())
        );
    }

    #[test]
    fn each_factor_refuses_on_its_own() {
        let mut admissions = Admissions::empty();
        let a = pin("sess-a", 1000, 42);
        admissions
            .admit(String::from("adm-1"), "spc-1", &a, NOW)
            .unwrap();
        let cases = [
            // Possession: a ticket the broker never minted.
            (
                "adm-forged",
                "spc-1",
                a.clone(),
                NOW,
                SpaceRefusal::NotAdmitted,
            ),
            // Liveness: replayed in another session, same peer.
            (
                "adm-1",
                "spc-1",
                pin("sess-b", 1000, 42),
                NOW,
                SpaceRefusal::StaleAdmission,
            ),
            // Peer pinning: same session, another peer.
            (
                "adm-1",
                "spc-1",
                pin("sess-a", 1000, 43),
                NOW,
                SpaceRefusal::PeerChanged,
            ),
            (
                "adm-1",
                "spc-1",
                pin("sess-a", 1001, 42),
                NOW,
                SpaceRefusal::PeerChanged,
            ),
            // Freshness: at and after expiry.
            (
                "adm-1",
                "spc-1",
                a.clone(),
                NOW + ADMISSION_TTL_SECS,
                SpaceRefusal::Expired,
            ),
            // Name binding: the ticket's space, not another.
            ("adm-1", "spc-2", a.clone(), NOW, SpaceRefusal::NotAdmitted),
            ("adm-1", "spc-1 ", a.clone(), NOW, SpaceRefusal::NotAdmitted),
            ("adm-1", "SPC-1", a, NOW, SpaceRefusal::NotAdmitted),
        ];
        for (ticket, space, who, at, refusal) in cases {
            assert_eq!(
                admissions.check(ticket, space, &who, at),
                Err(refusal),
                "{ticket} {space} {who:?} {at}"
            );
        }
    }

    #[test]
    fn a_sessions_tickets_end_with_it_and_are_bounded() {
        let mut admissions = Admissions::empty();
        let a = pin("sess-a", 1000, 42);
        let b = pin("sess-b", 1000, 42);
        for n in 0..MAX_ADMISSIONS_PER_SESSION {
            admissions
                .admit(format!("adm-a{n}"), "spc-1", &a, NOW)
                .unwrap();
        }
        assert_eq!(
            admissions.admit(String::from("adm-over"), "spc-1", &a, NOW),
            Err(SpaceRefusal::Full)
        );
        // The bound is per session: another session still joins.
        admissions
            .admit(String::from("adm-b"), "spc-1", &b, NOW)
            .unwrap();
        assert_eq!(admissions.len(), MAX_ADMISSIONS_PER_SESSION + 1);
        admissions.end_session("sess-a");
        assert_eq!(admissions.len(), 1);
        assert_eq!(
            admissions.check("adm-a0", "spc-1", &a, NOW),
            Err(SpaceRefusal::NotAdmitted),
            "an ended session's ticket is gone"
        );
        assert_eq!(admissions.check("adm-b", "spc-1", &b, NOW), Ok(()));
        assert_eq!((ADMISSION_TTL_SECS, MAX_ADMISSIONS_PER_SESSION), (900, 16));
    }

    #[test]
    fn a_sweep_drops_only_expired_admissions() {
        let mut admissions = Admissions::empty();
        let a = pin("sess-a", 1000, 42);
        admissions
            .admit(String::from("adm-old"), "spc-1", &a, NOW)
            .unwrap();
        admissions
            .admit(String::from("adm-new"), "spc-1", &a, NOW + 10)
            .unwrap();
        admissions.sweep(NOW + ADMISSION_TTL_SECS - 1);
        assert_eq!(admissions.len(), 2);
        admissions.sweep(NOW + ADMISSION_TTL_SECS);
        assert_eq!(admissions.len(), 1);
        assert_eq!(
            admissions.check("adm-new", "spc-1", &a, NOW + ADMISSION_TTL_SECS),
            Ok(())
        );
    }
}
