//! Media intake: typed, bounded, and typed by the broker.
//!
//! The uploader says only what *kind* of media it is sending (`image` or
//! `audio`). The broker decodes the bytes, bounds their size, and reads
//! their type from their own leading bytes ([`sniff`]). The bytes must be
//! a type the broker admits, of the kind the uploader said. The content
//! type stored and served is the broker's reading, never the uploader's
//! word. Nothing that can carry script is admitted: no SVG, no HTML.
//!
//! This is the first gate, not the only one: media is served with
//! headers the broker sets ([`SERVED_HEADERS`]), so a client treats it as
//! a download of exactly that type, never as a page.

use base64::Engine as _;
use hatchling_core::{MediaHeader, SpaceRefusal};

/// Largest media, in bytes: one sealed store entry, and one session frame
/// once encoded.
pub const MAX_MEDIA_BYTES: usize = 32 * 1024;

/// The media types the broker admits, by kind, with the leading bytes
/// that identify each: `(kind, content type, signature)`, where a `None`
/// byte is any byte.
const SIGNATURES: &[(&str, &str, &[Option<u8>])] = &[
    (
        "image",
        "image/png",
        &[
            Some(0x89),
            Some(b'P'),
            Some(b'N'),
            Some(b'G'),
            Some(0x0d),
            Some(0x0a),
            Some(0x1a),
            Some(0x0a),
        ],
    ),
    ("image", "image/jpeg", &[Some(0xff), Some(0xd8), Some(0xff)]),
    (
        "image",
        "image/gif",
        &[Some(b'G'), Some(b'I'), Some(b'F'), Some(b'8')],
    ),
    (
        "image",
        "image/webp",
        &[
            Some(b'R'),
            Some(b'I'),
            Some(b'F'),
            Some(b'F'),
            None,
            None,
            None,
            None,
            Some(b'W'),
            Some(b'E'),
            Some(b'B'),
            Some(b'P'),
        ],
    ),
    (
        "audio",
        "audio/wav",
        &[
            Some(b'R'),
            Some(b'I'),
            Some(b'F'),
            Some(b'F'),
            None,
            None,
            None,
            None,
            Some(b'W'),
            Some(b'A'),
            Some(b'V'),
            Some(b'E'),
        ],
    ),
    (
        "audio",
        "audio/ogg",
        &[Some(b'O'), Some(b'g'), Some(b'g'), Some(b'S')],
    ),
    (
        "audio",
        "audio/flac",
        &[Some(b'f'), Some(b'L'), Some(b'a'), Some(b'C')],
    ),
    ("audio", "audio/mpeg", &[Some(b'I'), Some(b'D'), Some(b'3')]),
];

/// The headers every piece of served media carries, after its content
/// type: never sniffed, never rendered inline, never able to run.
pub const SERVED_HEADERS: [(&str, &str); 3] = [
    ("X-Content-Type-Options", "nosniff"),
    ("Content-Disposition", "attachment"),
    ("Content-Security-Policy", "default-src 'none'; sandbox"),
];

/// The kind and content type of `bytes`, read from their leading bytes,
/// if they are a type the broker admits.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    SIGNATURES
        .iter()
        .find(|(_, _, signature)| {
            bytes.len() >= signature.len()
                && signature
                    .iter()
                    .zip(bytes)
                    .all(|(want, got)| want.is_none_or(|want| want == *got))
        })
        .map(|(kind, content_type, _)| (*kind, *content_type))
}

/// Admits media the uploader says is `kind`, sent as base64 `data`:
/// returns the bytes and the content type the broker assigns.
///
/// # Errors
///
/// [`SpaceRefusal::BadMedia`] for data that is not base64, or empty;
/// [`SpaceRefusal::MediaTooLarge`] past [`MAX_MEDIA_BYTES`];
/// [`SpaceRefusal::WrongMediaType`] for bytes of no admitted type, or not
/// of the kind said.
pub fn admit(kind: &str, data: &str) -> Result<(Vec<u8>, &'static str), SpaceRefusal> {
    // Bound before decoding: the encoded form of the largest media.
    if data.len() > MAX_MEDIA_BYTES.div_ceil(3).saturating_mul(4) {
        return Err(SpaceRefusal::MediaTooLarge);
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| SpaceRefusal::BadMedia)?;
    if bytes.is_empty() {
        return Err(SpaceRefusal::BadMedia);
    }
    if bytes.len() > MAX_MEDIA_BYTES {
        return Err(SpaceRefusal::MediaTooLarge);
    }
    match sniff(&bytes) {
        Some((sniffed, content_type)) if sniffed == kind => Ok((bytes, content_type)),
        Some(_) | None => Err(SpaceRefusal::WrongMediaType),
    }
}

/// The headers to serve media of `content_type` with.
#[must_use]
pub fn served_headers(content_type: &str) -> Vec<MediaHeader> {
    std::iter::once(("Content-Type", content_type))
        .chain(SERVED_HEADERS)
        .map(|(name, value)| MediaHeader {
            name: name.to_owned(),
            value: value.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn with(head: &[u8]) -> Vec<u8> {
        let mut bytes = head.to_vec();
        bytes.extend_from_slice(b"rest of the file");
        bytes
    }

    #[test]
    fn each_admitted_type_is_read_from_its_bytes() {
        for (head, kind, content_type) in [
            (&b"\x89PNG\r\n\x1a\n"[..], "image", "image/png"),
            (&b"\xff\xd8\xff\xe0"[..], "image", "image/jpeg"),
            (&b"GIF89a"[..], "image", "image/gif"),
            (&b"GIF87a"[..], "image", "image/gif"),
            (&b"RIFF\x10\x00\x00\x00WEBPVP8 "[..], "image", "image/webp"),
            (&b"RIFF\x10\x00\x00\x00WAVEfmt "[..], "audio", "audio/wav"),
            (&b"OggS\x00"[..], "audio", "audio/ogg"),
            (&b"fLaC"[..], "audio", "audio/flac"),
            (&b"ID3\x04"[..], "audio", "audio/mpeg"),
        ] {
            assert_eq!(
                sniff(&with(head)),
                Some((kind, content_type)),
                "{content_type}"
            );
            let (bytes, assigned) = admit(kind, &b64(&with(head))).unwrap();
            assert_eq!((bytes, assigned), (with(head), content_type));
        }
    }

    #[test]
    fn script_bearing_or_unknown_bytes_are_never_admitted() {
        for bytes in [
            &b"<svg xmlns='http://www.w3.org/2000/svg'><script>go()</script></svg>"[..],
            b"<!doctype html><script>go()</script>",
            b"#!/bin/sh\nrm -rf /",
            b"RIFF\x10\x00\x00\x00AVI LIST",
            b"\x89PNG",
            b"PK\x03\x04",
        ] {
            assert_eq!(sniff(bytes), None, "{bytes:?}");
            assert_eq!(
                admit("image", &b64(bytes)),
                Err(SpaceRefusal::WrongMediaType)
            );
            assert_eq!(
                admit("audio", &b64(bytes)),
                Err(SpaceRefusal::WrongMediaType)
            );
        }
    }

    #[test]
    fn the_uploader_cannot_choose_the_kind_the_bytes_are_not() {
        let png = with(b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            admit("audio", &b64(&png)),
            Err(SpaceRefusal::WrongMediaType)
        );
        assert_eq!(
            admit("video", &b64(&png)),
            Err(SpaceRefusal::WrongMediaType)
        );
        assert_eq!(
            admit("image/png", &b64(&png)),
            Err(SpaceRefusal::WrongMediaType)
        );
    }

    #[test]
    fn media_is_bounded_before_and_after_decoding() {
        let mut at_bound = with(b"GIF89a");
        at_bound.resize(MAX_MEDIA_BYTES, 0);
        assert!(admit("image", &b64(&at_bound)).is_ok());
        let mut over = at_bound.clone();
        over.push(0);
        assert_eq!(
            admit("image", &b64(&over)),
            Err(SpaceRefusal::MediaTooLarge)
        );
        // Over the encoded bound, refused before any decoding.
        let huge = "A".repeat(MAX_MEDIA_BYTES.div_ceil(3) * 4 + 4);
        assert_eq!(admit("image", &huge), Err(SpaceRefusal::MediaTooLarge));
        assert_eq!(admit("image", ""), Err(SpaceRefusal::BadMedia));
        assert_eq!(admit("image", "not base64!"), Err(SpaceRefusal::BadMedia));
        assert_eq!(MAX_MEDIA_BYTES, 32768);
    }

    #[test]
    fn served_media_carries_the_brokers_headers() {
        let headers: Vec<(String, String)> = served_headers("image/png")
            .into_iter()
            .map(|header| (header.name, header.value))
            .collect();
        assert_eq!(
            headers,
            [
                (String::from("Content-Type"), String::from("image/png")),
                (
                    String::from("X-Content-Type-Options"),
                    String::from("nosniff")
                ),
                (
                    String::from("Content-Disposition"),
                    String::from("attachment")
                ),
                (
                    String::from("Content-Security-Policy"),
                    String::from("default-src 'none'; sandbox")
                ),
            ]
        );
    }
}
