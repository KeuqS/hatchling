//! Agent side of the broker handshake.
//!
//! The framing and message vocabulary live in [`hatchling_core`]; this
//! module is the agent's behavior: send `HELLO`, require a well-formed
//! `WELCOME` speaking [`PROTOCOL_VERSION`](hatchling_core::PROTOCOL_VERSION).
//! Must only be called after the peer passed authorization — the
//! typestate enforces this.

use std::os::unix::net::UnixStream;

use hatchling_core::framing::{read_frame, write_frame};
use hatchling_core::{AgentMessage, BrokerMessage, CellId, PROTOCOL_VERSION};

use crate::Error;

/// Runs the handshake over an authorized broker connection.
///
/// Sends `HELLO` and requires a well-formed `WELCOME` speaking
/// [`PROTOCOL_VERSION`]. Must only be called after the peer passed
/// authorization — the typestate enforces this.
///
/// # Errors
///
/// Returns [`Error::HandshakeIo`] on transport failure,
/// [`Error::HandshakeRejected`] if the broker refuses, or
/// [`Error::HandshakeProtocol`] on any framing, encoding, or version
/// violation.
pub fn perform_handshake(stream: &UnixStream, cell_id: &CellId) -> Result<(), Error> {
    let hello = AgentMessage::Hello {
        v: PROTOCOL_VERSION,
        cell_id: cell_id.as_str().to_owned(),
    };
    let payload = serde_json::to_vec(&hello).map_err(|e| Error::HandshakeProtocol {
        reason: format!("failed to encode hello: {e}"),
    })?;
    write_frame(stream, &payload)?;

    let reply = read_frame(stream)?;
    let message: BrokerMessage =
        serde_json::from_slice(&reply).map_err(|e| Error::HandshakeProtocol {
            reason: format!("broker reply is not a valid handshake message: {e}"),
        })?;
    match message {
        BrokerMessage::Welcome { v } => {
            if v != PROTOCOL_VERSION {
                return Err(Error::HandshakeProtocol {
                    reason: format!("broker protocol version {v}, expected {PROTOCOL_VERSION}"),
                });
            }
            Ok(())
        }
        BrokerMessage::Reject { v: _, reason } => Err(Error::HandshakeRejected { reason }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use std::time::Duration;

    use crate::test_socket::TestSocket;

    fn test_path(name: &str) -> TestSocket {
        TestSocket::new(format!(
            "/tmp/hatchling-test-hs-{}-{name}.sock",
            std::process::id()
        ))
    }

    fn bind_listener(name: &str) -> (UnixListener, TestSocket) {
        let path = test_path(name);
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        (listener, path)
    }

    /// Connect a test client/server pair with short read timeouts. A
    /// missing frame (e.g. a mutant that skips `write_frame`, or one
    /// that skips the handshake entirely) must fail the peer's read
    /// in seconds, not hang the suite past the mutation runner's
    /// per-test timeout.
    fn connected_pair(name: &str) -> (UnixStream, UnixStream, TestSocket) {
        let (listener, path) = bind_listener(name);
        let client = UnixStream::connect(&path).unwrap();
        let (server, _) = listener.accept().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        (client, server, path)
    }

    fn cell_id() -> CellId {
        "cell-01".parse::<CellId>().unwrap()
    }

    fn encode(message: &BrokerMessage) -> Vec<u8> {
        serde_json::to_vec(message).unwrap()
    }

    #[test]
    fn handshake_succeeds_on_welcome() {
        let (client, server, _path) = connected_pair("welcome");

        let broker = thread::spawn(move || {
            let frame = read_frame(&server).unwrap();
            let hello: AgentMessage = serde_json::from_slice(&frame).unwrap();
            match hello {
                AgentMessage::Hello { v, cell_id } => {
                    assert_eq!(v, PROTOCOL_VERSION);
                    assert_eq!(cell_id, "cell-01");
                }
            }
            write_frame(
                &server,
                &encode(&BrokerMessage::Welcome {
                    v: PROTOCOL_VERSION,
                }),
            )
            .unwrap();
        });

        perform_handshake(&client, &cell_id()).unwrap();
        broker.join().unwrap();
    }

    #[test]
    fn handshake_fails_closed_on_reject() {
        let (client, server, _path) = connected_pair("reject");

        let broker = thread::spawn(move || {
            let _ = read_frame(&server).unwrap();
            write_frame(
                &server,
                &encode(&BrokerMessage::Reject {
                    v: PROTOCOL_VERSION,
                    reason: String::from("unknown cell"),
                }),
            )
            .unwrap();
        });

        let err = perform_handshake(&client, &cell_id()).unwrap_err();
        broker.join().unwrap();
        assert!(matches!(err, Error::HandshakeRejected { .. }));
        if let Error::HandshakeRejected { reason } = err {
            assert_eq!(reason, "unknown cell");
        }
    }

    #[test]
    fn handshake_fails_closed_on_garbage() {
        let (client, server, _path) = connected_pair("garbage");

        let broker = thread::spawn(move || {
            let _ = read_frame(&server).unwrap();
            write_frame(&server, b"this is not json").unwrap();
        });

        let err = perform_handshake(&client, &cell_id()).unwrap_err();
        broker.join().unwrap();
        assert!(
            matches!(err, Error::HandshakeProtocol { .. }),
            "expected HandshakeProtocol, got {err:?}"
        );
    }

    #[test]
    fn handshake_fails_closed_on_version_mismatch() {
        let (client, server, _path) = connected_pair("version");

        let broker = thread::spawn(move || {
            let _ = read_frame(&server).unwrap();
            write_frame(&server, &encode(&BrokerMessage::Welcome { v: 999 })).unwrap();
        });

        let err = perform_handshake(&client, &cell_id()).unwrap_err();
        broker.join().unwrap();
        assert!(
            matches!(err, Error::HandshakeProtocol { .. }),
            "expected HandshakeProtocol, got {err:?}"
        );
    }

    #[test]
    fn handshake_fails_closed_when_broker_closes_early() {
        let (client, server, _path) = connected_pair("early-close");

        // Broker reads HELLO, then vanishes without a reply.
        let broker = thread::spawn(move || {
            let _ = read_frame(&server).unwrap();
            drop(server);
        });

        let err = perform_handshake(&client, &cell_id()).unwrap_err();
        broker.join().unwrap();
        assert!(
            matches!(err, Error::HandshakeIo { .. }),
            "expected HandshakeIo, got {err:?}"
        );
    }
}
