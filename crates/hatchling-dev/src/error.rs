//! Typed errors for the dev tooling.

use std::path::PathBuf;
use thiserror::Error;

/// Errors the dev checks can produce. A check that finds a violation
/// reports it as a [`Finding`](crate::checks::Finding), not an error:
/// `Error` is for when the check itself could not run.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem or subprocess IO failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A manifest or config file did not parse.
    #[error("could not parse {path}: {reason}")]
    Parse {
        /// File that failed to parse.
        path: PathBuf,
        /// Why parsing failed.
        reason: String,
    },

    /// A required file is missing.
    #[error("missing required file: {0}")]
    MissingFile(PathBuf),

    /// An external tool exited non-zero.
    #[error("tool `{tool}` failed with status {status}")]
    ToolFailed {
        /// Tool that was run.
        tool: String,
        /// Its exit status.
        status: String,
    },

    /// A required environment variable is missing or empty.
    #[error("missing required environment variable: {0}")]
    MissingEnv(&'static str),

    /// A bounded tool invocation exceeded its deadline. The
    /// child and its process group were `SIGKILL`ed; the tool never
    /// finished, so its output cannot be trusted.
    #[error("tool `{tool}` timed out after {timeout:?}")]
    ToolTimeout {
        /// Tool that was run.
        tool: String,
        /// The deadline it exceeded.
        timeout: std::time::Duration,
    },
}
