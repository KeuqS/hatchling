//! Durable session records: the loop's state across restarts.
//!
//! A [`SessionRecord`] captures what a session was doing: open tasks,
//! pending confirmations, and in-flight tool calls. It survives loop
//! restarts because it lives in the broker-side store, not in the
//! agent's memory.
//!
//! The critical invariant: a crashed-then-resumed session
//! **reconciles rather than replays**. [`SessionStore::reconcile`]
//! marks in-flight tool calls [`Interrupted`](ToolCallOutcome::Interrupted)
//! — it never returns them for re-execution. A state-changing tool
//! that was interrupted must be re-proposed (and re-confirmed) by the
//! agent; the record cannot cause a double-execution on its own.

use serde::{Deserialize, Serialize};

use crate::item::SessionId;
use crate::{Error, MemoryStore};

/// One open task tracked by the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    /// Human-readable task description.
    pub description: String,
    /// Whether the task is done.
    pub done: bool,
}

/// One confirmation the agent is waiting on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationRecord {
    /// What is being confirmed (e.g. "delete file X").
    pub action: String,
    /// Unix epoch seconds when the request was made.
    pub requested_at_secs: u64,
}

/// The outcome of a tool call tracked by the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallOutcome {
    /// Started but not yet completed when the session ended.
    /// Reconcile converts these to `Interrupted`, never to
    /// `Completed`: the tool may or may not have run, so the only
    /// safe assumption is "unknown, do not replay blindly".
    InFlight,
    /// Completed and its result recorded.
    Completed,
    /// Was in-flight at crash/resume; must be re-proposed, never
    /// auto-replayed.
    Interrupted,
    /// Failed with the recorded reason.
    Failed,
}

/// One tool call the session tracked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// The tool that was called.
    pub tool: String,
    /// The JSON-encoded arguments.
    pub args_json: String,
    /// Unix epoch seconds when the call started.
    pub started_at_secs: u64,
    /// The call's outcome.
    pub outcome: ToolCallOutcome,
}

/// Lifecycle status of the session record itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Currently running.
    Active,
    /// Resumed after a crash or restart (via [`SessionStore::reconcile`]).
    Resumed,
    /// Cleanly closed.
    Closed,
}

/// The durable record of one session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Broker-assigned session identity.
    pub session_id: SessionId,
    /// The peer uid that owns this session.
    pub peer_uid: u32,
    /// Unix epoch seconds, broker-stamped at creation.
    pub created_at_secs: u64,
    /// Unix epoch seconds, broker-stamped at last update.
    pub updated_at_secs: u64,
    /// Lifecycle status.
    pub status: SessionStatus,
    /// Tasks the session had open.
    pub open_tasks: Vec<TaskRecord>,
    /// Confirmations awaiting a decision.
    pub pending_confirmations: Vec<ConfirmationRecord>,
    /// Tool calls the session tracked, including in-flight ones.
    pub tool_calls: Vec<ToolCallRecord>,
}

/// Broker-side store for session records.
///
/// Records live under `<memory-root>/sessions/<session-id>.json`,
/// beside (not inside) the memory item tree: they are broker
/// bookkeeping, not agent-readable memory. There is no agent-facing
/// read op for session records in v0.1 — the broker uses them
/// internally at session start.
#[derive(Debug)]
pub struct SessionStore<'s> {
    /// The memory store root; session records live in a `sessions/`
    /// subdirectory of it.
    store: &'s MemoryStore,
}

impl<'s> SessionStore<'s> {
    /// Borrow the memory store's root for session records.
    #[must_use]
    pub const fn new(store: &'s MemoryStore) -> Self {
        Self { store }
    }

    /// Creates a fresh active session record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem failure.
    pub fn create(
        &self,
        session_id: &SessionId,
        peer_uid: u32,
        now_secs: u64,
    ) -> Result<SessionRecord, Error> {
        let record = SessionRecord {
            session_id: session_id.clone(),
            peer_uid,
            created_at_secs: now_secs,
            updated_at_secs: now_secs,
            status: SessionStatus::Active,
            open_tasks: Vec::new(),
            pending_confirmations: Vec::new(),
            tool_calls: Vec::new(),
        };
        self.persist(&record)?;
        Ok(record)
    }

    /// Fetches a session record. `Ok(None)` when unknown.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Corrupt`] for an unreadable record, or
    /// [`Error::Io`].
    pub fn get(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, Error> {
        self.read_record(session_id)
    }

    /// Persists an updated record (tasks, confirmations, tool calls).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem failure.
    pub fn update(&self, record: &SessionRecord, now_secs: u64) -> Result<(), Error> {
        let mut record = record.clone();
        record.updated_at_secs = now_secs;
        self.persist(&record)
    }

    /// Reconciles a session record after a crash or restart.
    ///
    /// Every [`InFlight`](ToolCallOutcome::InFlight) tool call becomes
    /// [`Interrupted`](ToolCallOutcome::Interrupted): the tool may have
    /// run, may not have — the record cannot know, so it records the
    /// honest uncertainty instead of replaying. The status becomes
    /// [`Resumed`](SessionStatus::Resumed).
    ///
    /// This function is pure state transition: it executes nothing.
    /// The agent re-proposes interrupted calls (subject to
    /// confirmation rails); nothing in this record auto-runs.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] for an unknown session,
    /// [`Error::Corrupt`] for an unreadable record, or [`Error::Io`].
    pub fn reconcile(&self, session_id: &SessionId, now_secs: u64) -> Result<SessionRecord, Error> {
        let mut record = self.read_record(session_id)?.ok_or(Error::NotFound)?;
        let mut interrupted = 0;
        for call in &mut record.tool_calls {
            if call.outcome == ToolCallOutcome::InFlight {
                call.outcome = ToolCallOutcome::Interrupted;
                interrupted += 1;
            }
        }
        record.status = SessionStatus::Resumed;
        record.updated_at_secs = now_secs;
        self.persist(&record)?;
        tracing::info!(
            session = %session_id,
            interrupted,
            "session reconciled after restart"
        );
        Ok(record)
    }

    /// Reconciles every still-`Active` session record owned by
    /// `peer_uid`: sessions that never closed cleanly (crash, kill,
    /// reboot). Returns the number reconciled.
    ///
    /// The broker calls this when a new session starts for the peer,
    /// before creating the fresh record: it is how a crashed session's
    /// in-flight tool calls become [`Interrupted`](ToolCallOutcome::Interrupted)
    /// instead of lingering as in-flight forever.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem failure.
    pub fn reconcile_stale(&self, peer_uid: u32, now_secs: u64) -> Result<usize, Error> {
        let dir = self.store.sessions_dir()?;
        let mut reconciled = 0;
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::Io { source })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io { source })?;
            let path = entry.path();
            let is_file = entry
                .file_type()
                .map_err(|source| Error::Io { source })?
                .is_file();
            if !is_file || path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Some(record): Option<SessionRecord> = MemoryStore::read_raw(&path)? else {
                continue;
            };
            // Only `Active` records: a `Resumed` record can never gain
            // new in-flight calls (each connection gets a fresh
            // record), so re-reconciling it would just churn
            // `updated_at_secs`.
            if record.peer_uid == peer_uid && record.status == SessionStatus::Active {
                self.reconcile(&record.session_id, now_secs)?;
                reconciled += 1;
            }
        }
        if reconciled > 0 {
            tracing::info!(reconciled, "reconciled stale sessions");
        }
        Ok(reconciled)
    }

    /// Marks a session cleanly closed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] for an unknown session, or
    /// [`Error::Io`].
    pub fn close(&self, session_id: &SessionId, now_secs: u64) -> Result<(), Error> {
        let mut record = self.read_record(session_id)?.ok_or(Error::NotFound)?;
        record.status = SessionStatus::Closed;
        record.updated_at_secs = now_secs;
        self.persist(&record)
    }

    fn persist(&self, record: &SessionRecord) -> Result<(), Error> {
        let dir = self.store.sessions_dir()?;
        let path = dir.join(format!("{}.json", record.session_id.as_str()));
        let bytes = serde_json::to_vec(record).map_err(|_| Error::Corrupt)?;
        MemoryStore::write_raw(&dir, &path, &bytes)
    }

    fn read_record(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, Error> {
        let dir = self.store.sessions_dir()?;
        let path = dir.join(format!("{}.json", session_id.as_str()));
        MemoryStore::read_raw(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(name: &str) -> (MemoryStore, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "hatchling-memory-session-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = MemoryStore::open(&root).unwrap();
        (store, root)
    }

    fn session_id(id: &str) -> SessionId {
        id.parse().unwrap()
    }

    #[test]
    fn create_get_roundtrip() {
        let (store, root) = test_store("roundtrip");
        let sessions = SessionStore::new(&store);
        let id = session_id("sess-1");
        let created = sessions.create(&id, 1000, 1_000).unwrap();
        assert_eq!(created.status, SessionStatus::Active);
        assert_eq!(created.peer_uid, 1000);
        let fetched = sessions.get(&id).unwrap().unwrap();
        assert_eq!(fetched, created);
        assert!(sessions.get(&session_id("nope")).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_marks_in_flight_interrupted_never_completed() {
        // The no-double-execution core: a tool call that was
        // in-flight at crash time must come back Interrupted, never
        // Completed. Nothing in reconcile executes anything — it is a
        // pure state transition, so there is no path to a replay.
        let (store, root) = test_store("reconcile");
        let sessions = SessionStore::new(&store);
        let id = session_id("sess-crash");
        let mut record = sessions.create(&id, 1000, 1_000).unwrap();
        record.tool_calls.push(ToolCallRecord {
            tool: String::from("delete_file"),
            args_json: String::from(r#"{"path":"/tmp/x"}"#),
            started_at_secs: 1_001,
            outcome: ToolCallOutcome::InFlight,
        });
        record.tool_calls.push(ToolCallRecord {
            tool: String::from("read_file"),
            args_json: String::from(r#"{"path":"/tmp/y"}"#),
            started_at_secs: 1_002,
            outcome: ToolCallOutcome::Completed,
        });
        sessions.update(&record, 1_003).unwrap();

        // "Crash": drop everything, reopen, reconcile.
        drop(record);
        let sessions = SessionStore::new(&store);
        let resumed = sessions.reconcile(&id, 2_000).unwrap();

        assert_eq!(resumed.status, SessionStatus::Resumed);
        assert_eq!(resumed.tool_calls.len(), 2);
        // The state-changing in-flight call is Interrupted...
        assert!(
            resumed
                .tool_calls
                .first()
                .is_some_and(|call| call.outcome == ToolCallOutcome::Interrupted)
        );
        // ...the completed one is untouched (no `==`/`!=` confusion:
        // Completed stays Completed).
        assert!(
            resumed
                .tool_calls
                .get(1)
                .is_some_and(|call| call.outcome == ToolCallOutcome::Completed)
        );
        // And the persisted record agrees — a second reconcile is a
        // no-op on outcomes (idempotent).
        let again = sessions.reconcile(&id, 3_000).unwrap();
        assert!(
            again
                .tool_calls
                .first()
                .is_some_and(|call| call.outcome == ToolCallOutcome::Interrupted)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_unknown_session_is_not_found() {
        let (store, root) = test_store("unknown");
        let sessions = SessionStore::new(&store);
        let err = sessions.reconcile(&session_id("ghost"), 1_000).unwrap_err();
        assert!(matches!(err, Error::NotFound));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_stale_only_touches_this_peers_open_sessions() {
        let (store, root) = test_store("stale");
        let sessions = SessionStore::new(&store);
        // Peer's stale session: in-flight tool, never closed.
        let stale = session_id("stale-1");
        let mut record = sessions.create(&stale, 1000, 1_000).unwrap();
        record.tool_calls.push(ToolCallRecord {
            tool: String::from("x"),
            args_json: String::from("{}"),
            started_at_secs: 1_001,
            outcome: ToolCallOutcome::InFlight,
        });
        sessions.update(&record, 1_002).unwrap();
        // Peer's cleanly closed session: untouched.
        let closed = session_id("closed-1");
        sessions.create(&closed, 1000, 1_000).unwrap();
        sessions.close(&closed, 1_005).unwrap();
        // Another peer's stale session: untouched.
        let other = session_id("other-1");
        sessions.create(&other, 2000, 1_000).unwrap();

        assert_eq!(sessions.reconcile_stale(1000, 2_000).unwrap(), 1);
        let stale_record = sessions.get(&stale).unwrap().unwrap();
        assert_eq!(stale_record.status, SessionStatus::Resumed);
        assert!(
            stale_record
                .tool_calls
                .first()
                .is_some_and(|call| call.outcome == ToolCallOutcome::Interrupted)
        );
        // Closed and other-peer's records are unchanged.
        assert_eq!(
            sessions.get(&closed).unwrap().unwrap().status,
            SessionStatus::Closed
        );
        assert_eq!(
            sessions.get(&other).unwrap().unwrap().status,
            SessionStatus::Active
        );
        // Second run is a no-op: reconciled sessions are Resumed, not
        // re-reconciled.
        assert_eq!(sessions.reconcile_stale(1000, 3_000).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn close_marks_closed() {
        let (store, root) = test_store("close");
        let sessions = SessionStore::new(&store);
        let id = session_id("sess-end");
        sessions.create(&id, 1000, 1_000).unwrap();
        sessions.close(&id, 2_000).unwrap();
        let fetched = sessions.get(&id).unwrap().unwrap();
        assert_eq!(fetched.status, SessionStatus::Closed);
        assert_eq!(fetched.updated_at_secs, 2_000);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_bumps_updated_at() {
        let (store, root) = test_store("update-ts");
        let sessions = SessionStore::new(&store);
        let id = session_id("sess-ts");
        let mut record = sessions.create(&id, 1000, 1_000).unwrap();
        record.open_tasks.push(TaskRecord {
            description: String::from("write report"),
            done: false,
        });
        sessions.update(&record, 1_500).unwrap();
        let fetched = sessions.get(&id).unwrap().unwrap();
        assert_eq!(fetched.updated_at_secs, 1_500);
        assert_eq!(fetched.created_at_secs, 1_000);
        assert_eq!(fetched.open_tasks.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reconcile_stale_skips_stray_files() {
        let (store, root) = test_store("stray");
        let sessions = SessionStore::new(&store);
        let dir = store.sessions_dir().unwrap();
        std::fs::write(dir.join("notes.txt"), b"not a session record").unwrap();
        let result = sessions.reconcile_stale(1000, 1);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(result.ok(), Some(0));
    }
}
