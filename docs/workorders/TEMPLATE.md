# WO-NNN: <title>

**Status:** QUEUED | IN-PROGRESS | CLOSED | DROPPED
**Area:** <crates or frontend paths>
**Issue:** <link, if one exists>
**Trust boundary:** yes | no <!-- isolation, vault, egress, grants, auth, API: "yes" means extra review -->

## Problem

One or two paragraphs: what is wrong or missing, and why it matters to someone running Hatchling. Define any term a newcomer wouldn't know.

## Scope

**In scope:** the files and modules this WO changes.

**Not in scope:** the explicit non-goals. Anything not listed in scope is a different WO.

## Acceptance criteria

Concrete, checkable statements of the observable outcome, not the implementation. "Improve X" and "refactor Y" are rejected at review.

- [ ] <observable behavior or artifact>
- [ ] <observable behavior or artifact>

## Gates

All must pass before the WO closes:

- [ ] `bash scripts/check.sh` green (fmt, clippy with `-D warnings`, build, tests, seed check, `cargo deny`), on the pinned toolchain (`rust-toolchain.toml`) **and** the 1.85.0 MSRV (`RUSTUP_TOOLCHAIN=1.85.0 bash scripts/check.sh`).
- [ ] Real-cell tests ran rather than skipped: the gate run with `HATCHLING_REQUIRE_CELL=1`, on a Linux 5.11+ kernel with unprivileged user namespaces (or WSL2).
- [ ] **Mutation, kill floor 1.0:** `scripts/mutant-run.sh --base <base commit> -- <each changed .rs file>`. Its pre-flight refuses the run if any changed non-test function isn't called by name from a test (the delegation check). Every surviving mutant is killed by a new test, or proven equivalent in writing in `.cargo/mutants.toml`.
- [ ] If `frontend/` changed: `cd frontend && npm ci && npm run check`.
- [ ] If wire types changed: `cargo run -p hatchling-dev -- protocol-schema --write`, then `cd frontend && npm run protocol:generate`, with both outputs committed. (The `ProtocolAdapter` contract in `frontend/docs/PROTOCOL-LOCK.json` is frozen: changing it is a decision this WO must state and a maintainer must approve, never a regeneration to make a check pass.)
- [ ] Docs that describe the changed behavior are updated in the same change.

## Evidence

Filled in at close. Exact commands, environment (OS, kernel, toolchain versions), and counts: tests passed/failed, mutants caught/missed/timed out/unviable, and each equivalence proof. If it isn't reproducible, it didn't happen.

## Notes

Design rationale that doesn't fit above. Keep it factual.
