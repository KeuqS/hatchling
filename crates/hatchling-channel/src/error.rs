//! Channel errors.

use thiserror::Error;

/// Channel failures: backend errors, never protocol violations (those
/// are the session layer's job).
#[derive(Debug, Error)]
pub enum Error {
    /// The channel backend failed (I/O, closed, unavailable).
    #[error("channel backend failed: {0}")]
    Backend(String),
    /// No channel is registered under this id.
    #[error("unknown channel: {0}")]
    UnknownChannel(String),
    /// The output gate redacted the message: its notice was
    /// delivered in its place. The flagged text is never here.
    #[error("the output gate redacted the message ({category}, event {event_id})")]
    Redacted {
        /// The category it was redacted under.
        category: String,
        /// The redaction's event id, for the operator's review.
        event_id: String,
    },
}
