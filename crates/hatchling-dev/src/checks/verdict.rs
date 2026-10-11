//! Suspicious-outcome classifier for the mutation cache.
//!
//! cargo-mutants' outcome records can carry verdicts that must never
//! be frozen into the cache: a build that died from a signal, a build
//! that failed in a crate the mutant cannot have affected, a timeout,
//! or a malformed record. This module classifies each outcome as
//! [`VerdictClass::Reusable`] (safe to cache) or
//! [`VerdictClass::NonReusable`] (must be re-tested, never cached).
//!
//! Fail-closed ordering: suspicious conditions are checked first;
//! `Reusable` is the last resort, never the default. An outcome that
//! cannot be parsed is `NonReusable`.
//!
//! Wrong-crate attribution is conservative by design: it requires
//! POSITIVE evidence (a `-p`/`--package` in the phase argv naming
//! crates the mutant's package is not among and does not feed). A
//! workspace-wide build failure carries no such evidence, so it is
//! treated as same-crate. A false `NonReusable` costs one retest; a
//! false `Reusable` would freeze a lie into the seed.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// Whether an outcome record may be cached and reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictClass {
    /// Safe to freeze into the seed/cache: a genuine verdict.
    Reusable,
    /// Must be re-tested, never cached: signal death, timeout,
    /// wrong-crate failure, malformed record.
    NonReusable,
}

/// Verdict strings that may be served from cache.
///
/// This is an allowlist: anything else (`MissedMutant`, `Timeout`,
/// `Failure`, or an unknown string) forces a re-test.
/// Belt-and-suspenders — the seed builder ([`classify`]) never writes
/// those verdicts, but a hand-edited seed must not slip through
/// either.
#[must_use]
pub fn is_reusable_verdict(verdict: &str) -> bool {
    verdict == "CaughtMutant" || verdict == "Unviable"
}

/// How one phase of an outcome record classifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseClass {
    /// A build/check phase that failed to compile: needs wrong-crate
    /// attribution before it can be trusted.
    BuildFailure,
    /// Signal death, timeout, or a malformed/unknown status: never
    /// reusable, no further questions.
    Suspicious,
    /// Success, or a test-phase failure (the mutant was exercised by
    /// tests — a genuine verdict, not a build problem).
    Ok,
}

/// Classify one phase's `process_status`.
fn phase_class(phase: &Value) -> PhaseClass {
    // Malformed record: fail closed.
    let Some(phase_name) = phase.get("phase").and_then(Value::as_str) else {
        return PhaseClass::Suspicious;
    };
    let is_build = phase_name == "Build" || phase_name == "Check";
    let Some(status) = phase.get("process_status") else {
        return PhaseClass::Suspicious;
    };
    if let Some(text) = status.as_str() {
        return match text {
            "Success" => PhaseClass::Ok,
            // "Timeout", "Other", or any unknown string.
            _ => PhaseClass::Suspicious,
        };
    }
    if let Some(fields) = status.as_object() {
        if fields.contains_key("Signalled") {
            return PhaseClass::Suspicious;
        }
        if fields.contains_key("Failure") {
            return if is_build {
                PhaseClass::BuildFailure
            } else {
                // Test-phase failure: the tests ran and the mutant
                // changed their outcome — a genuine verdict.
                PhaseClass::Ok
            };
        }
        // Unknown object shape.
        return PhaseClass::Suspicious;
    }
    // Bool, number, array, null: not a known process_status shape.
    PhaseClass::Suspicious
}

/// Did this build/check failure happen in a crate the mutant cannot
/// have affected?
///
/// Positive evidence only: scan the phase argv for `-p`/`--package`
/// targets. If the argv names specific crates and the mutant's package
/// is neither among them nor a (transitive) dependency of them — i.e.
/// none of the built crates depends on the mutant's package, per
/// `rdeps` — the failure is in an unrelated crate. Without `-p`
/// targets (workspace-wide build) there is no positive evidence, so
/// this returns false (conservatively reusable on this criterion).
fn wrong_crate_failure(
    phase: &Value,
    package: &str,
    rdeps: &HashMap<String, HashSet<String, RandomState>, RandomState>,
) -> bool {
    let Some(argv) = phase.get("argv").and_then(Value::as_array) else {
        return false;
    };
    let mut targets: Vec<&str> = Vec::new();
    let mut arg_iter = argv.iter();
    while let Some(arg) = arg_iter.next() {
        let Some(text) = arg.as_str() else {
            continue;
        };
        if text == "-p" || text == "--package" {
            if let Some(target) = arg_iter.next().and_then(Value::as_str) {
                targets.push(target);
            }
        }
    }
    if targets.is_empty() {
        return false;
    }
    let dependents = rdeps.get(package);
    !targets
        .iter()
        .any(|target| *target == package || dependents.is_some_and(|deps| deps.contains(*target)))
}

/// Classify one cargo-mutants outcome record.
///
/// `rdeps` maps each workspace member to the set of members that
/// transitively depend on it (see `mutant_cache::reverse_deps`): a
/// mutant in package `P` can only break the build of `P` itself or of
/// a crate that depends on `P`.
///
/// # Fail-closed ordering
///
/// Signal death, timeout, and malformed records are rejected first;
/// wrong-crate build failures next; `Reusable` is returned only for
/// `CaughtMutant`/`Unviable` summaries with no suspicious phase.
#[must_use]
pub fn classify(
    outcome: &Value,
    rdeps: &HashMap<String, HashSet<String, RandomState>, RandomState>,
) -> VerdictClass {
    // Without the mutant's package no attribution is possible.
    let Some(package) = outcome
        .pointer("/scenario/Mutant/package")
        .and_then(Value::as_str)
    else {
        return VerdictClass::NonReusable;
    };
    // Summary-level timeout: never reused (matches the partition's
    // historical rule; kept here so the classifier is total).
    if outcome.get("summary").and_then(Value::as_str) == Some("Timeout") {
        return VerdictClass::NonReusable;
    }
    let Some(phases) = outcome.get("phase_results").and_then(Value::as_array) else {
        return VerdictClass::NonReusable;
    };
    for phase in phases {
        match phase_class(phase) {
            PhaseClass::Suspicious => return VerdictClass::NonReusable,
            PhaseClass::BuildFailure => {
                if wrong_crate_failure(phase, package, rdeps) {
                    return VerdictClass::NonReusable;
                }
            }
            PhaseClass::Ok => {}
        }
    }
    match outcome.get("summary").and_then(Value::as_str) {
        Some("CaughtMutant" | "Unviable") => VerdictClass::Reusable,
        // "MissedMutant", "Failure", "Success", missing, or unknown:
        // not a cacheable verdict.
        Some(_) | None => VerdictClass::NonReusable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minimal reverse-deps fixture: `app` depends on `mid`, `mid`
    /// depends on `base`; `lone` depends on nothing.
    fn fixture_rdeps() -> HashMap<String, HashSet<String>> {
        [
            (
                String::from("base"),
                [String::from("mid"), String::from("app")]
                    .into_iter()
                    .collect(),
            ),
            (
                String::from("mid"),
                std::iter::once(String::from("app")).collect(),
            ),
            (String::from("app"), HashSet::new()),
            (String::from("lone"), HashSet::new()),
        ]
        .into_iter()
        .collect()
    }

    fn outcome(summary: &str, package: &str, phases: &Value) -> Value {
        json!({
            "scenario": {
                "Mutant": {
                    "name": "src/lib.rs:1:1: replace foo with bar",
                    "package": package,
                    "file": "src/lib.rs",
                }
            },
            "summary": summary,
            "phase_results": phases,
        })
    }

    fn build_phase(status: &Value, argv: &[&str]) -> Value {
        json!({
            "phase": "Build",
            "process_status": status,
            "argv": argv,
        })
    }

    #[test]
    fn signal_death_is_not_reusable() {
        // Build killed by SIGKILL: not a genuine verdict.
        let o = outcome(
            "Failure",
            "base",
            &json!([build_phase(
                &json!({"Signalled": 9}),
                &["cargo", "test", "--no-run", "--workspace"]
            )]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn test_phase_signal_death_is_not_reusable() {
        let o = outcome(
            "Failure",
            "base",
            &json!([
                build_phase(&json!("Success"), &["cargo", "test", "--no-run", "--workspace"]),
                {
                    "phase": "Test",
                    "process_status": {"Signalled": 15},
                    "argv": ["cargo", "test", "--workspace"],
                },
            ]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn phase_timeout_is_not_reusable() {
        let o = outcome(
            "Timeout",
            "base",
            &json!([
                build_phase(&json!("Success"), &["cargo", "test", "--no-run", "--workspace"]),
                {
                    "phase": "Test",
                    "process_status": "Timeout",
                    "argv": ["cargo", "test", "--workspace"],
                },
            ]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn wrong_crate_build_failure_is_not_reusable() {
        // Mutant in `base`; the build targeted `-p lone`, which does
        // not depend on `base`: the mutant cannot have caused this.
        let o = outcome(
            "Unviable",
            "base",
            &json!([build_phase(
                &json!({"Failure": 101}),
                &["cargo", "test", "--no-run", "-p", "lone"],
            )]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn same_crate_build_failure_is_reusable() {
        // Mutant in `base`; the build failed: genuine unviable.
        let o = outcome(
            "Unviable",
            "base",
            &json!([build_phase(
                &json!({"Failure": 101}),
                &["cargo", "test", "--no-run", "--workspace"],
            )]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::Reusable);
    }

    #[test]
    fn dependent_crate_build_failure_is_reusable() {
        // Mutant in `base`; the build targeted `-p app`, which
        // transitively depends on `base`: the mutant can break it.
        let o = outcome(
            "Unviable",
            "base",
            &json!([build_phase(
                &json!({"Failure": 101}),
                &["cargo", "test", "--no-run", "-p", "app"],
            )]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::Reusable);
    }

    #[test]
    fn workspace_wide_build_failure_is_conservatively_reusable() {
        // No `-p` in argv: no positive evidence of wrong-crate, so
        // conservatively reusable on this criterion.
        let o = outcome(
            "Unviable",
            "lone",
            &json!([build_phase(
                &json!({"Failure": 101}),
                &["cargo", "test", "--no-run", "--workspace"],
            )]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::Reusable);
    }

    #[test]
    fn malformed_process_status_is_not_reusable() {
        for status in [
            json!("Other"),
            json!("Weird"),
            json!({"Unknown": 1}),
            json!(42),
            json!(null),
        ] {
            let o = outcome(
                "CaughtMutant",
                "base",
                &json!([build_phase(&status, &["cargo", "test", "--no-run"])]),
            );
            assert_eq!(
                classify(&o, &fixture_rdeps()),
                VerdictClass::NonReusable,
                "status must be rejected"
            );
        }
    }

    #[test]
    fn missing_phase_results_is_not_reusable() {
        let o = json!({
            "scenario": {"Mutant": {"package": "base"}},
            "summary": "CaughtMutant",
        });
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn missing_package_is_not_reusable() {
        let o = json!({
            "scenario": {"Mutant": {}},
            "summary": "CaughtMutant",
            "phase_results": [],
        });
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn clean_caught_mutant_is_reusable() {
        let o = outcome(
            "CaughtMutant",
            "base",
            &json!([
                build_phase(&json!("Success"), &["cargo", "test", "--no-run", "--workspace"]),
                {
                    "phase": "Test",
                    "process_status": {"Failure": 101},
                    "argv": ["cargo", "test", "--workspace"],
                },
            ]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::Reusable);
    }

    #[test]
    fn missed_mutant_is_not_reusable() {
        let o = outcome(
            "MissedMutant",
            "base",
            &json!([
                build_phase(&json!("Success"), &["cargo", "test", "--no-run", "--workspace"]),
                {
                    "phase": "Test",
                    "process_status": "Success",
                    "argv": ["cargo", "test", "--workspace"],
                },
            ]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn failure_summary_is_not_reusable() {
        // Summary "Failure" without a signalling phase on record:
        // still not a cacheable verdict.
        let o = outcome(
            "Failure",
            "base",
            &json!([build_phase(
                &json!({"Failure": 101}),
                &["cargo", "test", "--no-run", "--workspace"],
            )]),
        );
        assert_eq!(classify(&o, &fixture_rdeps()), VerdictClass::NonReusable);
    }

    #[test]
    fn reusable_verdict_allowlist() {
        assert!(is_reusable_verdict("CaughtMutant"));
        assert!(is_reusable_verdict("Unviable"));
        for v in ["MissedMutant", "Timeout", "Failure", "Success", "Bogus", ""] {
            assert!(!is_reusable_verdict(v), "{v} must not be reusable");
        }
    }

    /// Pins the `==` in `is_build`: the `!=` mutant classifies a Build
    /// (or Check) failure as `Ok` — a genuine verdict — letting a
    /// broken-build mutant be cached as reusable.
    #[test]
    fn build_and_check_failures_are_build_failures() {
        for phase_name in ["Build", "Check"] {
            let phase = json!({
                "phase": phase_name,
                "process_status": {"Failure": 1},
                "argv": ["cargo", "test", "--workspace"],
            });
            assert_eq!(
                phase_class(&phase),
                PhaseClass::BuildFailure,
                "phase {phase_name}"
            );
        }
        // A Test-phase failure is a genuine verdict.
        let phase = json!({
            "phase": "Test",
            "process_status": {"Failure": 1},
            "argv": ["cargo", "test", "--workspace"],
        });
        assert_eq!(phase_class(&phase), PhaseClass::Ok);
    }
}
