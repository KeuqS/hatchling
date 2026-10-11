//! Typed errors for the goals and tracking store.

/// Failures of the goals store, the tracking log, and the feed reader.
///
/// Backend failures are opaque at the API boundary: the broker logs
/// them and replies with an opaque error, so the agent learns that an
/// op failed, not why.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The shared directory discipline refused or failed (an insecure
    /// or non-directory component, or a filesystem error beneath it),
    /// or the memory store failed a memory query.
    #[error("store backend failed: {source}")]
    Store {
        /// The underlying directory-discipline failure.
        #[source]
        source: hatchling_memory::Error,
    },
    /// Filesystem failure on a tracking log.
    #[error("goals store I/O error")]
    Io {
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A goal id or source label failed validation.
    #[error("invalid goals name: {0}")]
    InvalidName(String),
    /// A title, detail, or tracking text failed validation.
    #[error("invalid {field}: {reason}")]
    InvalidText {
        /// Which field was rejected.
        field: &'static str,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// An update named no field to change.
    #[error("goal update names no field to change")]
    EmptyEdit,
    /// The user already holds the maximum number of goals.
    #[error("goal limit reached: at most {max} goals per user")]
    GoalLimit {
        /// The bound.
        max: usize,
    },
    /// The user's tracking log holds the maximum number of entries.
    #[error("tracking log full: at most {max} entries per user")]
    TrackingFull {
        /// The bound.
        max: u64,
    },
    /// A time range whose start is after its end.
    #[error("invalid time range: since {since} is after until {until}")]
    InvalidRange {
        /// The range start.
        since: u64,
        /// The range end.
        until: u64,
    },
    /// A memory query reached a reader that holds no memory store.
    #[error("memory is not available to this reader")]
    MemoryUnavailable,
    /// A feed item's stated provenance does not match its contents.
    #[error("feed item provenance does not match its contents")]
    ProvenanceMismatch,
    /// A stored record is corrupt, not valid JSON, or not where its
    /// own contents say it belongs.
    #[error("goals store record is corrupt")]
    Corrupt,
}

impl From<hatchling_memory::Error> for Error {
    fn from(source: hatchling_memory::Error) -> Self {
        Self::Store { source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_display_without_leaking_content() {
        let err = Error::InvalidText {
            field: "title",
            reason: "must not be empty",
        };
        assert_eq!(err.to_string(), "invalid title: must not be empty");
        assert_eq!(
            Error::GoalLimit { max: 256 }.to_string(),
            "goal limit reached: at most 256 goals per user"
        );
        assert_eq!(
            Error::InvalidRange { since: 9, until: 3 }.to_string(),
            "invalid time range: since 9 is after until 3"
        );
    }

    #[test]
    fn a_directory_failure_converts_into_a_store_error() {
        let err = Error::from(hatchling_memory::Error::Corrupt);
        assert!(matches!(
            err,
            Error::Store {
                source: hatchling_memory::Error::Corrupt
            }
        ));
    }
}
