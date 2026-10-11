//! Agent side of the egress protocol.
//!
//! The framing and message vocabulary live in [`hatchling_core`]; this
//! module is the agent's behavior: mint a surrogate for a destination,
//! then perform requests with it. The agent never touches the network
//! and never holds a real credential.
//! Must only be called on a [`Ready`](crate::Ready) cell — the typestate
//! enforces this.

use std::os::unix::net::UnixStream;

use hatchling_core::framing::{read_frame, write_frame};
use hatchling_core::{
    EgressOp, EgressResult, SessionRequest, SessionResponse, decode_secret, encode_secret,
};

use crate::Error;

/// An outbound HTTP response, as returned to the agent.
#[derive(Debug, Clone)]
pub struct EgressResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: Vec<(String, String)>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

/// Mints a surrogate for `destination`.
///
/// The broker checks the peer's grant for the destination, then the
/// egress allowlist. The surrogate is bound to (`peer_uid`, `destination`,
/// `expiry`) and is valid for `ttl_secs` seconds (capped at 1 hour by the
/// broker).
///
/// # Errors
///
/// Returns [`Error::EgressDenied`] if the broker holds no grant or the
/// destination is not on the allowlist, [`Error::EgressFailed`] on a
/// broker-side failure, [`Error::EgressIo`] on transport failure, or
/// [`Error::EgressProtocol`] on a malformed broker reply.
pub fn mint_surrogate(
    stream: &UnixStream,
    destination: &str,
    ttl_secs: u64,
) -> Result<String, Error> {
    let op = EgressOp::MintSurrogate {
        destination: destination.to_owned(),
        ttl_secs,
    };
    match request(stream, &op)? {
        EgressResult::Surrogate { surrogate_b64 } => Ok(surrogate_b64),
        EgressResult::Denied => Err(Error::EgressDenied {
            destination: destination.to_owned(),
        }),
        EgressResult::EgressError => Err(Error::EgressFailed),
        other @ EgressResult::Ok { .. } => Err(unexpected_reply(&op, &other)),
    }
}

/// Performs an outbound HTTP request with a previously minted surrogate.
///
/// The surrogate must have been minted for `destination`; the broker
/// validates the surrogate's signature, peer binding, destination
/// binding, and expiry before forwarding.
///
/// # Errors
///
/// Returns [`Error::EgressDenied`] if the surrogate is invalid or the
/// destination is not allowed, [`Error::EgressFailed`] on a broker-side
/// failure, [`Error::EgressIo`] on transport failure, or
/// [`Error::EgressProtocol`] on a malformed broker reply.
pub fn egress_request(
    stream: &UnixStream,
    surrogate_b64: &str,
    method: &str,
    destination: &str,
    path: &str,
    headers: Vec<(String, String)>,
    body: &[u8],
) -> Result<EgressResponse, Error> {
    let op = EgressOp::Egress {
        method: method.to_owned(),
        destination: destination.to_owned(),
        path: path.to_owned(),
        headers,
        body_b64: encode_secret(body),
        surrogate_b64: surrogate_b64.to_owned(),
    };
    match request(stream, &op)? {
        EgressResult::Ok {
            status,
            headers,
            body_b64,
        } => {
            let body = decode_secret(&body_b64).map_err(|error| Error::EgressProtocol {
                reason: error.to_string(),
            })?;
            Ok(EgressResponse {
                status,
                headers,
                body,
            })
        }
        EgressResult::Denied => Err(Error::EgressDenied {
            destination: destination.to_owned(),
        }),
        EgressResult::EgressError => Err(Error::EgressFailed),
        other @ EgressResult::Surrogate { .. } => Err(unexpected_reply(&op, &other)),
    }
}

/// Maps a frame-layer failure onto the egress vocabulary.
fn frame_error(error: hatchling_core::Error) -> Error {
    if let hatchling_core::Error::FrameIo { source } = error {
        return Error::EgressIo { source };
    }
    Error::EgressProtocol {
        reason: error.to_string(),
    }
}

/// Sends one egress op and reads the broker's reply.
///
/// The session now speaks [`SessionRequest`]/[`SessionResponse`]
/// envelopes (v3). Every op gets exactly one reply.
fn request(stream: &UnixStream, op: &EgressOp) -> Result<EgressResult, Error> {
    let request = SessionRequest::Egress(op.clone());
    let payload = serde_json::to_vec(&request).map_err(|e| Error::EgressProtocol {
        reason: format!("failed to encode egress op: {e}"),
    })?;
    write_frame(stream, &payload).map_err(frame_error)?;
    let reply = read_frame(stream).map_err(frame_error)?;
    let response: SessionResponse =
        serde_json::from_slice(&reply).map_err(|e| Error::EgressProtocol {
            reason: format!("broker reply is not a valid session response: {e}"),
        })?;
    match response {
        SessionResponse::Egress(result) => Ok(result),
        SessionResponse::Vault(_) => Err(Error::EgressProtocol {
            reason: String::from("broker gave a vault reply to an egress op"),
        }),
        SessionResponse::Channel(_) => Err(Error::EgressProtocol {
            reason: String::from("broker gave a channel reply to an egress op"),
        }),
        SessionResponse::Memory(_) => Err(Error::EgressProtocol {
            reason: String::from("broker gave a memory reply to an egress op"),
        }),
        SessionResponse::Skill(_) => Err(Error::EgressProtocol {
            reason: String::from("broker gave a skill reply to an egress op"),
        }),
        SessionResponse::Goals(_)
        | SessionResponse::Query(_)
        | SessionResponse::Schedule(_)
        | SessionResponse::Hook(_)
        | SessionResponse::Artifact(_)
        | SessionResponse::Space(_)
        | SessionResponse::Subagent(_) => Err(Error::EgressProtocol {
            reason: String::from("broker gave a goals or query reply to an egress op"),
        }),
    }
}

/// Builds the protocol-violation error for a reply that does not match
/// the op that was sent.
fn unexpected_reply(op: &EgressOp, result: &EgressResult) -> Error {
    Error::EgressProtocol {
        reason: format!("broker gave an unexpected reply to egress op: {op:?} -> {result:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    /// Short read timeout for the fake-broker sockets. A missing frame
    /// (e.g. a mutant that skips `write_frame`) must fail the peer's
    /// read in seconds, not hang the suite.
    fn timeouts(client: &UnixStream, server: &UnixStream) {
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
    }

    /// `mint_surrogate` returns the broker's surrogate verbatim. Pins
    /// the return-value substitution mutants (`Ok(String::new())`,
    /// `Ok("xyzzy".into())`).
    #[test]
    fn mint_surrogate_returns_broker_value() {
        let (client, server) = UnixStream::pair().unwrap();
        timeouts(&client, &server);
        let expected = String::from("real-surrogate-from-broker-12345");
        let expected_clone = expected.clone();
        // Fake broker: read the op, reply with a known surrogate.
        let handle = std::thread::spawn(move || {
            let frame = read_frame(&server).unwrap();
            let request: SessionRequest = serde_json::from_slice(&frame).unwrap();
            assert!(matches!(
                request,
                SessionRequest::Egress(EgressOp::MintSurrogate { .. })
            ));
            let response = SessionResponse::Egress(EgressResult::Surrogate {
                surrogate_b64: expected_clone,
            });
            let payload = serde_json::to_vec(&response).unwrap();
            write_frame(&server, &payload).unwrap();
        });

        let got = mint_surrogate(&client, "example.com:443", 3600).unwrap();
        assert_eq!(got, expected);
        assert!(!got.is_empty());
        assert_ne!(got, "xyzzy");
        handle.join().unwrap();
    }

    /// `mint_surrogate` maps a broker `Denied` to `EgressDenied`.
    #[test]
    fn mint_surrogate_denied() {
        let (client, server) = UnixStream::pair().unwrap();
        timeouts(&client, &server);
        let handle = std::thread::spawn(move || {
            let frame = read_frame(&server).unwrap();
            let _request: SessionRequest = serde_json::from_slice(&frame).unwrap();
            let response = SessionResponse::Egress(EgressResult::Denied);
            let payload = serde_json::to_vec(&response).unwrap();
            write_frame(&server, &payload).unwrap();
        });

        let err = mint_surrogate(&client, "example.com:443", 3600).unwrap_err();
        assert!(matches!(err, Error::EgressDenied { .. }));
        handle.join().unwrap();
    }
}
