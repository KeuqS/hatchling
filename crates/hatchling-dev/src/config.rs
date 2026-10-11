//! Floors, budgets, and pins the checks enforce.
//!
//! These are the ratchets: change them only deliberately, with a commit
//! message that says why. The mutation and coverage floors start at the
//! baselines and may only move upward.

/// Minimum viable-mutant kill rate. Baseline: 1.0 — 47 caught,
/// 0 missed, 20 unviable, 2 timeouts (timeouts count as kills: the
/// suite does not pass under those mutants).
pub const MUTANT_KILL_FLOOR: f64 = 1.0;

/// Minimum line coverage, as a fraction.
///
/// Baseline: 0.8153 measured via `cargo llvm-cov --workspace`;
/// the floor sits at 0.80 to leave headroom for measurement noise while
/// catching real regressions. Ratchet policy: when sustained coverage
/// exceeds the floor by five points, move the floor up to the new level
/// rounded down to a whole point.
pub const COVERAGE_FLOOR: f64 = 0.80;

/// Maximum lines for any runtime binary's `main.rs`. Thin-binary-edge
/// rule: `main` parses config, installs logging, runs the lifecycle,
/// returns an `ExitCode`. Logic lives in the library.
pub const MAIN_RS_BUDGET: usize = 80;

/// Pinned stable toolchain; must match `rust-toolchain.toml`.
pub const PINNED_TOOLCHAIN: &str = "1.98.1";

/// Name of this crate, for the dependency-direction guard.
pub const DEV_CRATE_NAME: &str = "hatchling-dev";
