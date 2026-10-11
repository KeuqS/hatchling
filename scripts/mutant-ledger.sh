#!/usr/bin/env bash
# The mutation run ledger: one JSON file per run,
# written once, under .mutation-state/ledger/<run-id>.json. "What did we
# run, and what did it say" is a file read, not a reconstruction.
#
# Usage:
#   scripts/mutant-ledger.sh record --id ID --status STATUS --base COMMIT
#       --log PATH [--exit N] [--started TIME] [--out DIR] [--] [FILE...]
#   scripts/mutant-ledger.sh list
#   scripts/mutant-ledger.sh show ID
#   scripts/mutant-ledger.sh tree-hash [DIR]
#
# A run id is `<UTC start, YYYYmmddTHHMMSSZ>-<first 8 of the tree hash>`.
# The run wrapper (mutant-run.sh) names the run's log, PID files and
# ledger entry with it, so they never disagree.
#
# record writes one entry, refusing to overwrite one that exists. With
# --out (a cargo-mutants output dir) it takes the counts from
# mutant-report.sh and keeps a copy of outcomes.json beside the ledger.
# A `complete` status is refused unless mutant-report.sh agrees.
#
# Entry schema, version 1 (every key is always present):
#   schema         1
#   run_id         the run id
#   status         complete | partial | refused
#   started_at     UTC, ISO 8601: when the run started (--started),
#                  or recorded_at
#   recorded_at    UTC, ISO 8601: when this entry was written
#   exit_status    the run's exit status (128+N: killed by signal N),
#                  or null (refused, never run)
#   base           the WO base commit the run was checked against
#   head           `git rev-parse HEAD`, or null
#   tree_hash      the tree hash (below) of the tree that was run
#   dirty          whether tracked files differed from HEAD
#   files          the files mutated (`--file`s); [] means the default
#                  list of scripts/mutate-targeted.sh
#   discovered, tested, caught, missed, timeout, unviable
#                  mutant-report.sh's counts, or null without a report
#   complete       mutant-report.sh's verdict, or false
#   missed_names, timeout_names
#                  the named mutants, or []
#   log            the run's log (or the pre-flight log, if refused)
#   outcomes       the kept copy of outcomes.json, or null
#
# tree-hash prints the sha256 of `git ls-files -z | xargs -0 sha256sum`
# run in DIR (default: the repository root): the tracked files'
# working-tree contents, the scheme WSL2's hand-backs report.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.mutation-state"
LEDGER="$STATE/ledger"
RUNS="$STATE/runs"
ID_PATTERN='^[0-9]{8}T[0-9]{6}Z-[0-9a-f]{8}$'

usage() {
  sed -n '6,11s/^# \{0,1\}//p' "$0" >&2
  exit 64
}

tree_hash() {
  (cd "${1:-$ROOT}" && git ls-files -z | xargs -0 -r sha256sum | sha256sum | cut -d' ' -f1)
}

record() {
  local id="" status="" base="" log="" exit_status="" started="" out=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --id | --status | --base | --log | --exit | --started | --out)
        [ "$#" -ge 2 ] || usage
        case "$1" in
          --id) id="$2" ;;
          --status) status="$2" ;;
          --base) base="$2" ;;
          --log) log="$2" ;;
          --exit) exit_status="$2" ;;
          --started) started="$2" ;;
          --out) out="$2" ;;
        esac
        shift 2
        ;;
      --) shift; break ;;
      -*) usage ;;
      *) break ;;
    esac
  done
  [[ "$id" =~ $ID_PATTERN ]] || { echo "mutant-ledger: bad run id '$id'" >&2; exit 64; }
  case "$status" in complete | partial | refused) ;; *) echo "mutant-ledger: bad status '$status'" >&2; exit 64 ;; esac
  [ -n "$base" ] && [ -n "$log" ] || usage
  [ -z "$exit_status" ] || [[ "$exit_status" =~ ^[0-9]+$ ]] || usage

  local entry="$LEDGER/$id.json"
  if [ -e "$entry" ]; then
    echo "mutant-ledger: $entry exists; ledger entries are never rewritten" >&2
    exit 1
  fi
  mkdir -p "$LEDGER" "$RUNS"

  local report="" outcomes=""
  if [ -n "$out" ] && [ -f "$out/outcomes.json" ]; then
    local rc=0
    report="$("$ROOT/scripts/mutant-report.sh" --json "$out")" || rc=$?
    [ "$rc" -le 2 ] || report=""
    outcomes="$RUNS/$id.outcomes.json"
  fi

  local head dirty=0
  head="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
  [ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no 2>/dev/null)" ] || dirty=1

  RUN_ID="$id" STATUS="$status" BASE="$base" LOG="$log" EXIT="$exit_status" \
    STARTED="$started" RECORDED="$(date -u +%Y-%m-%dT%H:%M:%SZ)" HEAD="$head" \
    TREE="$(tree_hash)" DIRTY="$dirty" REPORT="$report" OUTCOMES="$outcomes" \
    ENTRY="$entry" python3 - "$@" <<'PY'
import json
import os
import sys

e = os.environ
report = json.loads(e["REPORT"]) if e["REPORT"] else None
complete = bool(report and report["complete"])
if e["STATUS"] == "complete" and not complete:
    print("mutant-ledger: a run is complete only when mutant-report.sh says every "
          "discovered mutant was tested", file=sys.stderr)
    sys.exit(1)
entry = {
    "schema": 1,
    "run_id": e["RUN_ID"],
    "status": e["STATUS"],
    "started_at": e["STARTED"] or e["RECORDED"],
    "recorded_at": e["RECORDED"],
    "exit_status": int(e["EXIT"]) if e["EXIT"] else None,
    "base": e["BASE"],
    "head": e["HEAD"] or None,
    "tree_hash": e["TREE"] or None,
    "dirty": e["DIRTY"] == "1",
    "files": sys.argv[1:],
    "complete": complete,
    "missed_names": report["missed_names"] if report else [],
    "timeout_names": report["timeout_names"] if report else [],
    "log": e["LOG"],
    "outcomes": e["OUTCOMES"] or None,
}
for key in ("discovered", "tested", "caught", "missed", "timeout", "unviable"):
    entry[key] = report[key] if report else None
try:
    with open(e["ENTRY"], "x") as f:
        json.dump(entry, f, indent=2, sort_keys=True)
        f.write("\n")
except FileExistsError:
    print(f"mutant-ledger: {e['ENTRY']} exists; ledger entries are never rewritten", file=sys.stderr)
    sys.exit(1)
os.chmod(e["ENTRY"], 0o444)
print(e["ENTRY"])
PY
  if [ -n "$outcomes" ]; then
    cp "$out/outcomes.json" "$outcomes"
    chmod 0444 "$outcomes"
  fi
}

list_runs() {
  shopt -s nullglob
  local entries=("$LEDGER"/*.json)
  if [ "${#entries[@]}" -eq 0 ]; then
    echo "no runs recorded"
    return
  fi
  python3 - "${entries[@]}" <<'PY'
import json
import sys

def shown(value):
    return "-" if value is None else value

for path in sys.argv[1:]:
    with open(path) as f:
        e = json.load(f)
    print(f"{e['run_id']}  {e['status']:<8}  tested {shown(e['tested'])}/{shown(e['discovered'])}  "
          f"caught {shown(e['caught'])} missed {shown(e['missed'])} timeout {shown(e['timeout'])}")
PY
}

case "${1:-}" in
  record) shift; record "$@" ;;
  list) list_runs ;;
  show)
    [[ "${2:-}" =~ $ID_PATTERN ]] || usage
    [ -f "$LEDGER/$2.json" ] || { echo "mutant-ledger: no run $2" >&2; exit 1; }
    cat "$LEDGER/$2.json"
    ;;
  tree-hash) tree_hash "${2:-}" ;;
  -h | --help) sed -n '2,/^set -euo/{/^#/s/^# \{0,1\}//p}' "$0" ;;
  *) usage ;;
esac
