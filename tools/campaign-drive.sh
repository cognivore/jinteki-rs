#!/usr/bin/env bash
# campaign-drive.sh — the outer loop for the whole card campaign.
#
# Phase 1: every identity in the game   (docs/vm/IDENTITY-QUEUE.md)
# Phase 2: the mandated deck queue      (docs/vm/DECK-QUEUE.md)
#
# WHY AN OUTER LOOP. A single agent context cannot span 150 identities plus
# five decks; it runs out. docs/vm/WAVES.md describes what actually works —
# "each handoff updates this file; the successor agent reads it FIRST" — so
# progress lives in the LEDGER and each wave is a fresh context that reads it.
# This script is that succession, automated: it keeps launching fresh waves
# until the ledgers are empty, and switches brief when phase 1 finishes.
#
# Usage:  tools/campaign-drive.sh          # run the whole campaign
#         MAX_STALLS=5 tools/campaign-drive.sh
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 1

IDQ="docs/vm/IDENTITY-QUEUE.md"
MAX_STALLS="${MAX_STALLS:-4}"
QUIET="${QUIET:-5}"                 # minutes of stillness before a successor
STALE_AFTER="${STALE_AFTER:-25}"    # minutes of a dirty, frozen tree = orphan
LOG="$REPO/.overnight/campaign-$(date +%Y%m%d).log"
mkdir -p "$REPO/.overnight"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$LOG"; }

ids_left() { grep -c '^- \[ \]' "$IDQ" 2>/dev/null || echo 0; }
ids_done() { grep -c '^- \[x\]' "$IDQ" 2>/dev/null || echo 0; }

# Phase 2 is finished when every deck in the queue is complete. The deck
# odometer is the test's own line, which is also what a human reads.
decks_left() {
  nix develop --command cargo test -p jinteki-cards --test decks -- --nocapture 2>/dev/null \
    | sed -n 's/.*priority decks: [0-9]* cards, \([0-9]*\) complete, \([0-9]*\) partial.*/\2/p' | head -1
}

phase() { [[ "$(ids_left)" -gt 0 ]] && echo identities || echo decks; }

log "campaign starting: $(ids_done)/150 identities, $(ids_left) to go"
stalls=0
while true; do
  ph="$(phase)"
  if [[ "$ph" == decks ]]; then
    left="$(decks_left)"
    if [[ -n "${left:-}" && "$left" == "0" ]]; then
      log "IDENTITIES AND DECKS BOTH COMPLETE. Stopping."
      break
    fi
  fi

  if pgrep -f "overnight-drive.sh" >/dev/null; then sleep 60; continue; fi

  # Wait for a still repo: another agent may be working this checkout and
  # takes no lock. A working agent trips this constantly; a finished one
  # never does. A tree that stays DIRTY and FROZEN is an orphaned batch from
  # an agent that died — salvage it rather than deadlocking here forever,
  # which is exactly what the previous keeper did.
  quiet=0; frozen=0
  while [[ "$quiet" -lt "$QUIET" ]]; do
    h="$(git rev-parse HEAD)"
    if [[ -n "$(git status --porcelain)" ]]; then
      quiet=0; frozen=$((frozen + 1))
      if [[ "$frozen" -ge "$STALE_AFTER" ]]; then
        log "tree dirty and frozen ${frozen}m — stashing an orphaned batch"
        git stash push -u -m "campaign: orphaned batch $(date +%H:%M)" >/dev/null 2>&1
        frozen=0
      fi
    else
      quiet=$((quiet + 1)); frozen=0
    fi
    sleep 60
    [[ "$(git rev-parse HEAD)" != "$h" ]] && { quiet=0; frozen=0; }
  done

  before_head="$(git rev-parse HEAD)"
  before_ids="$(ids_done)"
  case "$ph" in
    identities) BF="$REPO/tools/briefs/identities.txt" ;;
    decks)      BF="$REPO/tools/briefs/decks.txt" ;;
  esac
  log "wave: phase=${ph} ($(ids_done)/150 identities) brief=$(basename "$BF")"
  BRIEF_FILE="$BF" DEADLINE=23:58 MAX_ITERS=0 nohup "$REPO/tools/overnight-drive.sh" >/dev/null 2>&1 &
  sleep 45
  while pgrep -f "overnight-drive.sh" >/dev/null; do sleep 60; done

  # Assess for DRIFT after every wave. Unattended agents are fast and
  # confident, which is what makes drift expensive: a wrong pattern is copied
  # into the next batch before anyone reads it. Every check in the assessor
  # is one that has already been violated here at least once. If it fires,
  # STOP — waving another agent on top of drifted work multiplies it.
  if ! "$REPO/tools/assess-drift.sh" >>"$LOG" 2>&1; then
    log "DRIFT DETECTED after this wave — stopping the campaign for review"
    break
  fi

  if [[ "$(git rev-parse HEAD)" == "$before_head" && "$(ids_done)" -le "$before_ids" ]]; then
    stalls=$((stalls + 1))
    log "no progress this wave (stall ${stalls}/${MAX_STALLS})"
    [[ "$stalls" -ge "$MAX_STALLS" ]] && { log "giving up — a human should look"; break; }
    sleep 30
  else
    stalls=0
    log "progress: identities $(ids_done)/150"
  fi
done
log "campaign finished: $(ids_done)/150 identities, $(ids_left) to go"
git --no-pager log --oneline -8 | tee -a "$LOG"
