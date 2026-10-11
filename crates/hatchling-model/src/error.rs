//! Typed errors for the model provider.
//!
//! Everything the provider reports converts into
//! [`hatchling_loop::Error::ModelFailed`] at the [`Model`][hatchling_loop::Model]
//! trait boundary, with the reason preserved.

use thiserror::Error;

use crate::transport::TransportError;

/// What can go wrong building or driving the provider.
#[derive(Debug, Error)]
pub enum Error {
    /// The configured endpoint is not a usable `https://` URL.
    #[error("invalid endpoint: {reason}")]
    InvalidEndpoint {
        /// Why the endpoint was rejected.
        reason: String,
    },
    /// The configured model name is empty.
    #[error("invalid model name: must be non-empty")]
    EmptyModelName,
    /// The transport leg failed.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The model cannot do what the loop needs: it does not
    /// call tools, or its declared context window is too small. Refused
    /// loudly before a task, never discovered mid-run.
    #[error("the model cannot do what the loop needs: {reason}")]
    Incapable {
        /// What it cannot do.
        reason: String,
    },
    /// The provider answered, but the response cannot be interpreted
    /// as a chat-completions reply.
    #[error("uninterpretable provider response: {reason}")]
    BadResponse {
        /// Why the response was rejected.
        reason: String,
    },
}

impl Error {
    /// Renders the error for the loop's `ModelFailed` reason string.
    #[must_use]
    pub fn reason(&self) -> String {
        self.to_string()
    }
}
