#!/usr/bin/env bash
# Run one timing-sensitive test many times and count (): a
# race-dependent kill gets a number, not a re-run and a hope.
#
# Usage: scripts/stress-test.sh [-p PACKAGE] [--load K] TEST N
#
# Builds the tests once, then runs `cargo test [-p PACKAGE] -- --exact
# TEST` N times. TEST is the test's full path (`module::name`). With
# --load K, K busy loops run alongside every round: contention is what
# surfaces a race. Each round passes only if the command succeeds and
# reports at least one passed test; a round where no test ran stops
# the script, since a misspelled name would otherwise pass every time.
#
# Prints PASS or FAIL per round, then `passed P of N, failed F`. A
# failing round's output is kept in .mutation-state/stress/.
#
# STRESS_CMD, if set, is run instead of cargo (given TEST as its one
# argument), with no build step.
#
# Exit status: 0 every round passed; 1 a round failed; 64 usage, or no
# test ran.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTDIR="$ROOT/.mutation-state/stress"

usage() {
  echo "usage: $0 [-p PACKAGE] [--load K] TEST N" >&2
  exit 64
}

PACKAGE="" LOAD=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    -p) [ "$#" -ge 2 ] || usage; PACKAGE="$2"; shift 2 ;;
    --load) [ "$#" -ge 2 ] || usage; LOAD="$2"; shift 2 ;;
    -h | --help) sed -n '2,/^set -euo/{/^#/s/^# \{0,1\}//p}' "$0"; exit 0 ;;
    -*) usage ;;
    *) break ;;
  esac
done
[ "$#" -eq 2 ] || usage
TEST="$1" N="$2"
[[ "$N" =~ ^[1-9][0-9]*$ ]] || usage
[[ "$LOAD" =~ ^[0-9]+$ ]] || usage

cd "$ROOT"
mkdir -p "$OUTDIR"
NAME="$(printf '%s' "$TEST" | tr -c 'A-Za-z0-9_.-' '_')"
rm -f "$OUTDIR/$NAME".*.log

if [ -n "${STRESS_CMD:-}" ]; then
  RUN=("$STRESS_CMD" "$TEST")
else
  CARGO=(cargo test --locked)
  [ -z "$PACKAGE" ] || CARGO+=(-p "$PACKAGE")
  "${CARGO[@]}" --no-run --quiet
  RUN=("${CARGO[@]}" --quiet -- --exact "$TEST")
fi

LOADERS=()
stop_load() {
  for pid in "${LOADERS[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
}
trap stop_load EXIT
for _ in $(seq 1 "$LOAD"); do
  (while :; do :; done) &
  LOADERS+=("$!")
done

PASSED=0
FAILED=0
for round in $(seq 1 "$N"); do
  LOG="$OUTDIR/$NAME.$round.log"
  if "${RUN[@]}" >"$LOG" 2>&1; then
    if ! grep -qE 'test result: ok\. [1-9][0-9]* passed' "$LOG"; then
      echo "stress-test: no test named '$TEST' ran (give its full path, module::name)" >&2
      exit 64
    fi
    PASSED=$((PASSED + 1))
    echo "round $round: PASS"
    rm -f "$LOG"
  else
    FAILED=$((FAILED + 1))
    echo "round $round: FAIL (output: $LOG)"
  fi
done
LOADED=""
[ "$LOAD" -eq 0 ] || LOADED=" (under $LOAD busy loops)"
echo "$TEST: passed $PASSED of $N, failed $FAILED$LOADED"
[ "$FAILED" -eq 0 ]
