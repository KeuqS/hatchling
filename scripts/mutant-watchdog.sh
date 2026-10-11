#!/bin/bash
# Watchdog for a cargo-mutants run. Alerts (by exiting with a message, which
# the runtime delivers) if the run dies silently or stalls. Also backs up
# outcomes.json every 10 checks so records survive even mutants.out damage.
# Usage: mutant-watchdog.sh <expected_total> &
#
# Environment (scripts/mutant-run.sh sets the first three):
#   MUTANT_ROOT               the repository (default: ~/workspace/hatchling)
#   MUTANT_PIDFILE            watch the PID in this file, instead of
#                             matching any cargo-mutants process by name
#   MUTANT_LOG                a run log to check for the completion line
#   MUTANT_WATCHDOG_INTERVAL  seconds between checks (default 60)
cd "${MUTANT_ROOT:-$HOME/workspace/hatchling}" || exit 1
EXPECTED="${1:-0}"
INTERVAL="${MUTANT_WATCHDOG_INTERVAL:-60}"
mkdir -p .mutation-state
LAST=0
STALLED=0
alive() {
  if [ -n "${MUTANT_PIDFILE:-}" ]; then
    kill -0 "$(cat "$MUTANT_PIDFILE" 2>/dev/null)" 2>/dev/null
  else
    # Bracket trick: pgrep pattern must not match its own command line.
    pgrep -f "[c]argo-mutants" >/dev/null 2>&1
  fi
}
for i in $(seq 1 240); do
  sleep "$INTERVAL"
  if ! alive; then
    # The process being gone can mean success: check the log for the summary line first.
    if grep -q "mutants tested in" ${MUTANT_LOG:+"$MUTANT_LOG"} .mutation-state/shard*.log .mutation-state/full-run.log 2>/dev/null; then
      echo "WATCHDOG: cargo-mutants process is gone and the log shows a completed summary line. Verify the summary counts, then merge."
      exit 0
    fi
    N=$(python3 -c "import json;print(len(json.load(open('mutants.out/outcomes.json')).get('outcomes',[])))" 2>/dev/null || echo "?")
    echo "WATCHDOG: cargo-mutants process is GONE with no completion signal. outcomes recorded: $N of ~$EXPECTED. Check for VM reboot (uptime) before relaunching."
    exit 0
  fi
  if [ $((i % 10)) -eq 0 ]; then
    cp mutants.out/outcomes.json .mutation-state/outcomes-backup.json 2>/dev/null
  fi
  N=$(python3 -c "import json;print(len(json.load(open('mutants.out/outcomes.json')).get('outcomes',[])))" 2>/dev/null || echo 0)
  if [ "$N" = "$LAST" ]; then
    STALLED=$((STALLED+1))
    if [ "$STALLED" -ge 10 ]; then
      echo "WATCHDOG: no new outcomes for 10 checks ($((10 * INTERVAL))s; stuck at $N). Process alive but stalled; investigate."
      exit 0
    fi
  else
    STALLED=0
    LAST="$N"
  fi
done
echo "WATCHDOG: $((240 * INTERVAL / 60)) minutes elapsed, run still going; check manually."
