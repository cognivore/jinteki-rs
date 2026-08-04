#!/usr/bin/env bash
# identity-drive.sh — keep implementing identities until ALL of them are done.
#
# The finish condition is the queue itself, not a clock: every checkbox in
# docs/vm/IDENTITY-QUEUE.md ticked. `overnight-drive.sh` does one unit of work
# per iteration and stops at its DEADLINE or on an iteration that produced no
# commit; this relaunches it until the queue is empty, so a single stalled
# iteration does not end the run.
#
# It gives up only if several consecutive relaunches make no progress at all —
# that is a stuck agent, not slow work, and spinning on it wastes tokens.
#
# Usage:
#   tools/identity-drive.sh                  # until 150/150
#   MAX_STALLS=5 tools/identity-drive.sh     # tolerate more no-progress rounds
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 1
QUEUE="docs/vm/IDENTITY-QUEUE.md"
MAX_STALLS="${MAX_STALLS:-3}"
LOG="$REPO/.overnight/identity-keeper-$(date +%Y%m%d).log"
mkdir -p "$REPO/.overnight"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$LOG"; }

# Ticked checkboxes in the queue. The queue is the single source of truth for
# progress precisely so this loop and a human read the same number.
done_count() { grep -c '^- \[x\]' "$QUEUE" 2>/dev/null || echo 0; }
todo_count() { grep -c '^- \[ \]' "$QUEUE" 2>/dev/null || echo 0; }

log "identity keeper starting: $(done_count) done, $(todo_count) to go"
stalls=0
while true; do
  todo="$(todo_count)"
  if [[ "$todo" -eq 0 ]]; then
    log "ALL IDENTITIES IMPLEMENTED ($(done_count)). Stopping."
    break
  fi
  if pgrep -f "overnight-drive.sh" >/dev/null; then sleep 60; continue; fi

  before_done="$(done_count)"
  before_head="$(git rev-parse HEAD)"
  log "relaunching drive — $before_done done, $todo to go"
  # A late deadline so the drive itself rarely ends a round; the keeper is
  # what decides when to stop.
  DEADLINE=23:58 MAX_ITERS=0 nohup "$REPO/tools/overnight-drive.sh" >/dev/null 2>&1 &
  sleep 45
  # Wait out this drive run.
  while pgrep -f "overnight-drive.sh" >/dev/null; do sleep 60; done

  after_done="$(done_count)"
  if [[ "$after_done" -le "$before_done" && "$(git rev-parse HEAD)" == "$before_head" ]]; then
    stalls=$((stalls + 1))
    log "no progress this round (stall ${stalls}/${MAX_STALLS})"
    if [[ "$stalls" -ge "$MAX_STALLS" ]]; then
      log "giving up after ${stalls} stalled rounds — a human should look"
      break
    fi
    sleep 30
  else
    stalls=0
    log "progress: ${before_done} -> ${after_done} identities"
  fi
done
log "keeper finished: $(done_count) done, $(todo_count) to go"
git --no-pager log --oneline -5 | tee -a "$LOG"
