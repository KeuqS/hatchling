//! Names in the goals store: goal ids and tracking source labels.
//!
//! Both share the memory store's name rule: ASCII alphanumeric, `-`,
//! or `_`, bounded length. The alphabet has no path separator, so
//! traversal, absolute paths, and NUL bytes are unrepresentable — a
//! goal id is joined onto a store path only after it parses.
//!
//! Unlike the memory names, these validate on deserialize too: a
//! record read back from disk (or a feed item read off the wire) with
//! a malformed name fails to decode instead of carrying it forward.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;

/// Maximum goal id length, in bytes.
const MAX_ID_LEN: usize = 128;
/// Maximum source label length, in bytes. Matches the memory tag
/// bound: a label names where something came from, not what it says.
const MAX_SOURCE_LEN: usize = 64;

/// Validates the shared name alphabet.
fn valid_name(raw: &str, max_len: usize) -> bool {
    if raw.is_empty() || raw.len() > max_len {
        return false;
    }
    raw.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

macro_rules! define_name {
    ($(#[$meta:meta])* $name:ident, $max:expr, $what:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// The validated name.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(raw: &str) -> Result<Self, Self::Err> {
                if valid_name(raw, $max) {
                    Ok(Self(raw.to_owned()))
                } else {
                    Err(Error::InvalidName(format!(
                        concat!($what, " must be 1-{} bytes of [A-Za-z0-9_-], got {:?}"),
                        $max, raw
                    )))
                }
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;

            fn try_from(raw: String) -> Result<Self, Self::Error> {
                raw.parse()
            }
        }

        impl From<$name> for String {
            fn from(name: $name) -> Self {
                name.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

define_name!(
    /// Identifies one goal.
    ///
    /// Minted by the store at creation, never chosen by the agent: a
    /// create can never overwrite a goal.
    GoalId,
    MAX_ID_LEN,
    "goal id"
);
define_name!(
    /// Where a tracking entry came from (`user`, `whatsapp`, …).
    ///
    /// Stated by whoever appended the entry. The feed shows it as the
    /// entry's original source, so a label is carried verbatim and
    /// never rewritten.
    SourceLabel,
    MAX_SOURCE_LEN,
    "source label"
);

impl GoalId {
    /// Mints a fresh goal id from 16 random bytes, hex-encoded.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the system RNG cannot be read.
    pub fn generate() -> Result<Self, Error> {
        use std::io::Read as _;
        let mut bytes = [0u8; 16];
        let mut urandom =
            std::fs::File::open("/dev/urandom").map_err(|source| Error::Io { source })?;
        urandom
            .read_exact(&mut bytes)
            .map_err(|source| Error::Io { source })?;
        Ok(Self(hex(&bytes)))
    }
}

/// Lowercase hex of `bytes`, built from nibbles (the lint set denies
/// both `format!`-in-`collect` and `expect`).
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_accept_exactly_the_shared_alphabet() {
        for good in ["a", "Z9", "run-5k", "x_y", "whatsapp"] {
            assert!(good.parse::<SourceLabel>().is_ok(), "{good}");
            assert!(good.parse::<GoalId>().is_ok(), "{good}");
        }
        for bad in ["", "a/b", "..", "a b", "a.b", "a\0", "é", "a:b"] {
            assert!(bad.parse::<SourceLabel>().is_err(), "{bad:?}");
            assert!(bad.parse::<GoalId>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn length_bounds_are_exact() {
        assert!("a".repeat(64).parse::<SourceLabel>().is_ok());
        assert!("a".repeat(65).parse::<SourceLabel>().is_err());
        assert!("a".repeat(128).parse::<GoalId>().is_ok());
        assert!("a".repeat(129).parse::<GoalId>().is_err());
    }

    #[test]
    fn names_validate_on_deserialize() {
        let ok: SourceLabel = serde_json::from_str(r#""user""#).unwrap();
        assert_eq!(ok.as_str(), "user");
        assert_eq!(serde_json::to_string(&ok).unwrap(), r#""user""#);
        assert!(serde_json::from_str::<SourceLabel>(r#""../etc""#).is_err());
        assert!(serde_json::from_str::<GoalId>(r#""""#).is_err());
    }

    #[test]
    fn generated_ids_are_valid_distinct_hex() {
        let a = GoalId::generate().unwrap();
        let b = GoalId::generate().unwrap();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 32);
        assert!(a.as_str().bytes().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a.as_str().parse::<GoalId>().unwrap(), a);
        assert_eq!(a.to_string(), a.as_str());
    }

    #[test]
    fn hex_encodes_both_nibbles_in_order() {
        assert_eq!(hex(&[0x00, 0x0f, 0xf0, 0xa5, 0xff]), "000ff0a5ff");
    }
}
