//! Typed errors for the memory store.

use std::path::PathBuf;

/// Failures of the memory store, session store, and context log.
///
/// Backend failures (filesystem errors) are opaque at the API
/// boundary: the broker collapses them before replying, so the agent
/// learns that an op failed, not why.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure.
    #[error("memory store I/O error")]
    Io {
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The store root is not a directory.
    #[error("memory store root is not a directory: {}", path.display())]
    NotADirectory {
        /// The offending path.
        path: PathBuf,
    },
    /// The store root has insecure permissions or owner: fail closed.
    #[error("memory store root has insecure permissions: {}", path.display())]
    InsecurePermissions {
        /// The offending path.
        path: PathBuf,
        /// The uid the root was expected to be owned by.
        expected_uid: u32,
    },
    /// An id, tag, project name, or session id failed validation.
    #[error("invalid memory name: {0}")]
    InvalidName(String),
    /// Content exceeds [`crate::MAX_CONTENT_BYTES`].
    #[error("memory content too large: {len} bytes, max {max}")]
    ContentTooLarge {
        /// The content's byte length.
        len: usize,
        /// The bound.
        max: usize,
    },
    /// An item or context entry would not fit a session frame once
    /// encoded for a reply ([`crate::MAX_ENCODED_ITEM_BYTES`]).
    #[error("memory item too large once encoded for a reply: {len} bytes, max {max}")]
    EncodedTooLarge {
        /// The encoded length ([`crate::reply_encoded_len`]).
        len: usize,
        /// The bound.
        max: usize,
    },
    /// No item is stored under the id (or it expired).
    #[error("memory item not found")]
    NotFound,
    /// A stored item file is corrupt or not valid JSON.
    #[error("memory item is corrupt")]
    Corrupt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_display_without_leaking_paths_content() {
        // The Display impls must not leak item content; paths are
        // fine (the broker never forwards these to the agent anyway).
        let err = Error::ContentTooLarge { len: 99, max: 10 };
        assert_eq!(
            err.to_string(),
            "memory content too large: 99 bytes, max 10"
        );
        let err = Error::InvalidName(String::from("bad/name"));
        assert_eq!(err.to_string(), "invalid memory name: bad/name");
    }
}
