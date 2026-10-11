//! The [`Transport`] trait: the provider's only path to the network.
//!
//! The provider translates between the loop's [`Model`][crate::provider]
//! interface and provider JSON; it never performs I/O itself. The host
//! implements this trait — in production, over the broker-mediated
//! egress path — and injects it. Tests inject a canned transport.

use thiserror::Error;

/// How the provider reaches the model endpoint.
///
/// Synchronous, matching the v0.1 [`Model`][hatchling_loop::Model]
/// trait. `destination` is `host:port`, `path` the full request path
/// starting with `/` — the same split the broker's egress operation
/// uses, so the agent's implementation is a direct mapping.
pub trait Transport: Send + Sync {
    /// POST `body` to `destination` + `path` with `headers`.
    ///
    /// Returns the raw response body on HTTP success.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the request cannot be made or
    /// the response status is not success.
    fn post(
        &self,
        destination: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<u8>, TransportError>;
}

/// What can go wrong on the transport leg.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The request failed before or during transport.
    #[error("transport failed: {0}")]
    Failed(String),
    /// The endpoint answered with a non-success status.
    #[error("endpoint returned HTTP status {0}")]
    HttpStatus(u16),
}
