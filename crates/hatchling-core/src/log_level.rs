//! Log verbosity, shared by every hatchling binary.
//!
//! One vocabulary for `--log-level`-style configuration so operator
//! tooling sees the same values everywhere.

use std::str::FromStr;

use tracing::Level;

use crate::Error;

/// Log verbosity for a hatchling binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Designates very low verbosity.
    Error,
    /// Designates lower verbosity.
    Warn,
    /// Designates normal verbosity.
    Info,
    /// Designates higher verbosity.
    Debug,
    /// Designates very high verbosity.
    Trace,
}

impl LogLevel {
    /// Maps to the `tracing` level used by the subscriber.
    #[must_use]
    pub const fn to_tracing(self) -> Level {
        match self {
            Self::Error => Level::ERROR,
            Self::Warn => Level::WARN,
            Self::Info => Level::INFO,
            Self::Debug => Level::DEBUG,
            Self::Trace => Level::TRACE,
        }
    }
}

impl FromStr for LogLevel {
    type Err = Error;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        // The `_` arm is the fail-closed default: anything unrecognized is
        // rejected rather than guessed at.
        match raw.to_ascii_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(Error::InvalidLogLevel(format!("unknown log level {raw:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_accepted_spelling() {
        // One entry per match arm: deleting an arm must fail the suite.
        let cases = [
            ("error", LogLevel::Error),
            ("warn", LogLevel::Warn),
            ("warning", LogLevel::Warn),
            ("info", LogLevel::Info),
            ("debug", LogLevel::Debug),
            ("trace", LogLevel::Trace),
            // Case-insensitive: operators shout in env files.
            ("ERROR", LogLevel::Error),
            ("Warn", LogLevel::Warn),
            ("INFO", LogLevel::Info),
        ];
        for (raw, expected) in cases {
            assert_eq!(raw.parse::<LogLevel>().unwrap(), expected, "input {raw:?}");
        }
    }

    #[test]
    fn rejects_unknown_levels() {
        for raw in ["verbose", "", "info ", "err"] {
            assert!(
                matches!(raw.parse::<LogLevel>(), Err(Error::InvalidLogLevel(_))),
                "input {raw:?} should be rejected"
            );
        }
    }

    #[test]
    fn maps_to_tracing_levels() {
        assert_eq!(LogLevel::Error.to_tracing(), Level::ERROR);
        assert_eq!(LogLevel::Warn.to_tracing(), Level::WARN);
        assert_eq!(LogLevel::Info.to_tracing(), Level::INFO);
        assert_eq!(LogLevel::Debug.to_tracing(), Level::DEBUG);
        assert_eq!(LogLevel::Trace.to_tracing(), Level::TRACE);
    }
}
