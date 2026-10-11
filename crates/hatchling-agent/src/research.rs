//! The agent side of research runs: every source is fetched
//! through the broker's egress proxy.
//!
//! A fetch mints a surrogate for the source's destination, then makes a
//! `GET` with it. The broker checks the peer's grant and the egress
//! allowlist on both; a public source on the allowlist is read without
//! any credential. A destination the broker refuses is
//! [`FetchFailure::Denied`], and nothing is fetched from it.

use std::os::unix::net::UnixStream;

use hatchling_research::{FetchFailure, Fetched, Fetcher, SourceUrl};

use crate::{Error, egress};

/// How long a source's surrogate lives, in seconds: one fetch.
pub const SOURCE_SURROGATE_TTL_SECS: u64 = 60;

/// Fetches research sources through the broker session's egress proxy.
#[derive(Debug)]
pub struct EgressFetcher<'s> {
    stream: &'s UnixStream,
}

impl<'s> EgressFetcher<'s> {
    /// A fetcher over the broker session.
    #[must_use]
    pub const fn new(stream: &'s UnixStream) -> Self {
        Self { stream }
    }
}

impl Fetcher for EgressFetcher<'_> {
    fn fetch(&self, url: &SourceUrl) -> Result<Fetched, FetchFailure> {
        let destination = url.destination();
        let surrogate = egress::mint_surrogate(self.stream, destination, SOURCE_SURROGATE_TTL_SECS)
            .map_err(|error| failure(&error))?;
        let response = egress::egress_request(
            self.stream,
            &surrogate,
            "GET",
            destination,
            url.path(),
            Vec::new(),
            &[],
        )
        .map_err(|error| failure(&error))?;
        Ok(Fetched {
            status: response.status,
            body: response.body,
        })
    }
}

/// An egress error as a fetch failure: refused by the broker, or failed.
const fn failure(error: &Error) -> FetchFailure {
    if matches!(error, Error::EgressDenied { .. }) {
        FetchFailure::Denied
    } else {
        FetchFailure::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_is_denied_and_anything_else_failed() {
        assert_eq!(
            failure(&Error::EgressDenied {
                destination: String::from("evil.example:443"),
            }),
            FetchFailure::Denied
        );
        assert_eq!(failure(&Error::EgressFailed), FetchFailure::Failed);
        assert_eq!(SOURCE_SURROGATE_TTL_SECS, 60);
    }
}
