#!/usr/bin/env bash
# next-todo.sh — run one ready entry of docs/todo.md § Next headless.
#
# The local form of the scheduled runner (docs/workflow-c.md § 9): one entry per
# run, three hours of wall clock, nothing asked of a human. Edits are accepted
# (`--permission-mode acceptEdits`); a print-mode run has nobody to answer a
# prompt, so a command outside .claude/settings.json's allow list is denied and
# the run fails instead of waiting.
#
# Use:  just next-todo          (or from a timer: bash scripts/next-todo.sh)
# Exit: 0 done or nothing to do · 1 preconditions · 2 the run failed or timed out
set -euo pipefail

PROJECT_ROOT="$(git rev-parse --show-toplevel)"
cd "$PROJECT_ROOT"
LOG_DIR="$PROJECT_ROOT/logs/next-todo"
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/$(date -u +%Y%m%dT%H%M%SZ).log"
BUDGET="${NEXT_TODO_BUDGET:-3h}"

log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*" | tee -a "$LOG"; }

# One run at a time: a second timer tick while a run is open must not start over.
exec 9>"$LOG_DIR/.lock"
if ! flock -n 9; then
    log "another run holds the lock — skipped"
    exit 0
fi

for tool in claude git gh just flock timeout; do
    command -v "$tool" >/dev/null 2>&1 || { log "$tool not found"; exit 1; }
done

if [[ -n "$(git status --short)" ]]; then
    log "working tree is not clean — refusing to run"
    exit 1
fi
if [[ "$(git branch --show-current)" != "main" ]]; then
    log "not on main — refusing to run"
    exit 1
fi
git pull --ff-only --quiet origin main || { log "main could not be fast-forwarded"; exit 1; }

# The queue: references (#NNN or TD-NNN) under § Next, comments ignored.
queued=$(awk '/^## Next/{f=1; next} /^## /{f=0} f' docs/todo.md | grep -v '^<!--' | grep -Eo '#[0-9]{3}|TD-[0-9]{3}' || true)
if [[ -z "$queued" ]]; then
    log "nothing queued"
    exit 0
fi
log "queued: $(tr '\n' ' ' <<<"$queued")"

log "run starts (budget $BUDGET, log $LOG)"
set +e
timeout --signal=INT --kill-after=60 "$BUDGET" \
    claude -p "/next-todo" --permission-mode acceptEdits --output-format text >>"$LOG" 2>&1
status=$?
set -e
case "$status" in
    0) log "run finished" ;;
    124|137) log "run stopped at the $BUDGET budget"; exit 2 ;;
    *) log "run failed (exit $status)"; exit 2 ;;
esac
