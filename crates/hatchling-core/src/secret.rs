//! Secret payload codec: arbitrary bytes inside framed JSON.
//!
//! The wire protocol is framed JSON, but secrets are arbitrary bytes.
//! This module is the single place that converts between the two:
//! base64, one pinned engine ([`base64::engine::general_purpose::STANDARD`]).
//! The agent encodes before sending; the broker decodes after receiving.
//! Neither end hand-rolls the conversion.
//!
//! [`MAX_SECRET_BYTES`] is the largest secret either end will carry. It
//! is sized so the base64 expansion of a maximum secret still fits
//! comfortably inside one [`MAX_FRAME_BYTES`](crate::MAX_FRAME_BYTES)
//! frame with room for the JSON envelope: 32 KiB of secret becomes
//! ~43 KiB of base64, well under the 64 KiB frame cap.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::Error;

/// Maximum size of one secret, in bytes. A secret must fit in a single
/// protocol frame; this bound keeps the base64 expansion inside
/// [`MAX_FRAME_BYTES`](crate::MAX_FRAME_BYTES).
pub const MAX_SECRET_BYTES: usize = 32 * 1024;

/// Encodes secret bytes for a protocol payload.
#[must_use]
pub fn encode(secret: &[u8]) -> String {
    STANDARD.encode(secret)
}

/// Decodes a secret payload back to bytes.
///
/// # Errors
///
/// Returns [`Error::InvalidSecretEncoding`] if the payload is not valid
/// base64. Callers treat this as a protocol violation: the peer sent
/// bytes that are not a well-formed secret.
pub fn decode(payload: &str) -> Result<Vec<u8>, Error> {
    STANDARD
        .decode(payload)
        .map_err(|e| Error::InvalidSecretEncoding(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_secret_bytes_fits_a_frame() {
        // The value pin and the fit argument, together: a `*` -> `+`
        // mutant on the constant breaks the pin; the inequality proves
        // the bound actually serves the frame cap.
        assert_eq!(MAX_SECRET_BYTES, 32 * 1024);
        let encoded = encode(&vec![0xABu8; MAX_SECRET_BYTES]);
        assert!(
            encoded.len() < crate::MAX_FRAME_BYTES,
            "base64 of a maximum secret must fit in one frame"
        );
    }

    #[test]
    fn codec_round_trips_arbitrary_bytes() {
        // Arbitrary bytes, including non-UTF-8: the whole point of
        // base64 over raw JSON strings.
        let raw = vec![0x00, 0xFF, 0x80, b'a', 0x1F, 0x7F];
        assert_eq!(decode(&encode(&raw)).unwrap(), raw);
    }

    #[test]
    fn decode_rejects_garbage() {
        // Pins the error mapping: undecodable input must surface as the
        // typed protocol error, not a panic or a silent empty vec.
        let err = decode("!!! not base64 !!!").unwrap_err();
        assert!(
            matches!(err, Error::InvalidSecretEncoding(_)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn encode_is_standard_base64() {
        // Pins the engine choice: standard alphabet with padding.
        assert_eq!(encode(b"hello"), "aGVsbG8=");
    }
}
