#!/usr/bin/env bash
# Report on a cargo-mutants run: progress, the
# named missed and timed-out mutants, and whether the run is complete.
#
# Usage: scripts/mutant-report.sh [--json] [OUT_DIR]
#
# OUT_DIR (default: mutants.out) holds outcomes.json and, from the
# start of a run, mutants.json: every mutant the run discovered.
#
# Complete means every discovered mutant was tested: by name when
# mutants.json is there, by count (`total_mutants`) otherwise. A run
# that is not complete is never called complete, whatever its counts
# say: a run killed at 482 of 678 still has a clean-looking report.
#
# The schema is handled here, once. outcomes.json (cargo-mutants 27)
# has top-level counts (`caught`, `missed`, `timeout`, `unviable`) and
# one entry per scenario: the baseline (`"scenario": "Baseline"`) or a
# mutant (`"scenario": {"Mutant": {"name": ...}}`), whose `summary` is
# `CaughtMutant`, `MissedMutant`, `Timeout` or `Unviable`. The count
# key is `missed`, the summary `MissedMutant`. Counts here come from
# the summaries, and a top-level count that disagrees makes the report
# inconsistent.
#
# --json prints the verdict as JSON (for mutant-run.sh, mutant-ledger.sh
# and verify-results.sh): complete, discovered, tested, the four counts,
# and missed_names, timeout_names, untested_names, extra_names and
# duplicate_names.
#
# Exit status:
#   0   complete, nothing missed or timed out
#   1   complete, with missed or timed-out mutants (both fail the floor)
#   2   incomplete: some discovered mutants were never tested
#   3   no report, or an unreadable or inconsistent one
#   64  usage
set -euo pipefail

JSON=0
if [ "${1:-}" = "--json" ]; then
  JSON=1
  shift
fi
case "${1:-}" in
  -h | --help)
    sed -n '2,/^set -euo/{/^#/s/^# \{0,1\}//p}' "$0"
    exit 0
    ;;
esac
if [ "$#" -gt 1 ]; then
  echo "usage: $0 [--json] [OUT_DIR]" >&2
  exit 64
fi
OUT="${1:-mutants.out}"

exec python3 - "$OUT" "$JSON" <<'PY'
import collections
import json
import os
import sys

out, as_json = sys.argv[1], sys.argv[2] == "1"
KINDS = {
    "CaughtMutant": "caught",
    "MissedMutant": "missed",
    "Timeout": "timeout",
    "Unviable": "unviable",
}


def fail(message):
    print(f"mutant-report: {message}", file=sys.stderr)
    sys.exit(3)


def load(name):
    path = os.path.join(out, name)
    try:
        with open(path) as f:
            return json.load(f)
    except FileNotFoundError:
        return None
    except (OSError, ValueError) as e:
        fail(f"{path} is unreadable: {e}")


report = load("outcomes.json")
if not isinstance(report, dict):
    fail(f"{os.path.join(out, 'outcomes.json')} is missing or not a report")

counts = {kind: 0 for kind in KINDS.values()}
names = {kind: [] for kind in KINDS.values()}
tested = []
for entry in report.get("outcomes", []):
    scenario = entry.get("scenario")
    if scenario == "Baseline":
        continue
    mutant = scenario.get("Mutant") if isinstance(scenario, dict) else None
    name = mutant.get("name") if isinstance(mutant, dict) else None
    kind = KINDS.get(entry.get("summary"))
    if not name or kind is None:
        fail(f"unrecognized outcome: summary {entry.get('summary')!r}, scenario {str(scenario)[:80]}")
    counts[kind] += 1
    names[kind].append(name)
    tested.append(name)

for kind, count in counts.items():
    if report.get(kind) != count:
        fail(f"top-level {kind!r} is {report.get(kind)!r}, but {count} outcomes say {kind}")

listed = load("mutants.json")
seen = collections.Counter(tested)
duplicates = sorted(name for name, n in seen.items() if n > 1)
if listed is not None:
    if not isinstance(listed, list) or not all(
        isinstance(m, dict) and isinstance(m.get("name"), str) for m in listed
    ):
        fail("mutants.json is not a list of named mutants")
    found = collections.Counter(m["name"] for m in listed)
    discovered = len(listed)
    untested = sorted((found - seen).elements())
    extra = sorted((seen - found).elements())
else:
    discovered = report.get("total_mutants")
    if not isinstance(discovered, int):
        fail("there is no mutants.json, and outcomes.json has no total_mutants")
    untested, extra = [], []
complete = not untested and not extra and not duplicates and len(tested) == discovered

verdict = {
    "complete": complete,
    "discovered": discovered,
    "tested": len(tested),
    **counts,
    "missed_names": names["missed"],
    "timeout_names": names["timeout"],
    "untested_names": untested,
    "extra_names": extra,
    "duplicate_names": duplicates,
    "cargo_mutants_version": report.get("cargo_mutants_version"),
}
floor_failed = counts["missed"] > 0 or counts["timeout"] > 0
status = 2 if not complete else 1 if floor_failed else 0

if as_json:
    print(json.dumps(verdict, indent=2, sort_keys=True))
    sys.exit(status)


def section(title, items, limit=50):
    if items:
        print(f"{title} ({len(items)}):")
        for item in items[:limit]:
            print(f"  {item}")
        if len(items) > limit:
            print(f"  ... and {len(items) - limit} more")


percent = 100 * len(tested) // discovered if discovered else 100
print(f"progress: {len(tested)} of {discovered} discovered mutants tested ({percent}%)")
print("caught {caught}, missed {missed}, timeout {timeout}, unviable {unviable}".format(**counts))
section("missed", names["missed"])
section("timeout", names["timeout"])
section("never tested", untested)
section("tested but never discovered", extra)
section("tested more than once", duplicates)
if not complete:
    print(f"INCOMPLETE: {len(tested)} of {discovered} discovered mutants tested; "
          "this run is not complete, and its counts cover only what it tested.")
elif floor_failed:
    print(f"COMPLETE, FLOOR FAILED: {counts['missed']} missed, {counts['timeout']} timed out.")
else:
    print("COMPLETE: every discovered mutant was tested; nothing missed or timed out.")
sys.exit(status)
PY
