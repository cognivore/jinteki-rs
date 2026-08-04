#!/usr/bin/env bash
# drive-until-ready.sh — keep the card drive going until BOTH priority decks
# are playable, i.e. crates/jinteki-server/src/cr.rs::readiness() reports
# complete == total. Below that the CR lobby REFUSES to start a game
# (SYS-D-12: strict decks), so there is no partial credit — 42/50 is as
# unplayable as 0/50. That is why this loops rather than stopping at a time.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 1
HARD_STOP="${HARD_STOP:-18:00}"
LOG="$REPO/.overnight/keeper-$(date +%Y%m%d).log"
mkdir -p "$REPO/.overnight"
log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$LOG"; }

remaining() {
  nix develop --command cargo test -p jinteki-cards --test decks -- --nocapture 2>/dev/null \
    | sed -n 's/.*priority decks: \([0-9]*\) cards, \([0-9]*\) complete.*/\1 \2/p' | head -1
}

log "keeper starting; hard stop ${HARD_STOP}"
while [[ "$(date +%H:%M)" < "$HARD_STOP" ]]; do
  if ! pgrep -f "overnight-drive.sh" >/dev/null; then
    read -r total complete <<<"$(remaining)"
    if [[ -n "${total:-}" && "${complete:-0}" == "${total}" ]]; then
      log "ALL ${total} CARDS COMPLETE — both decks are playable. Stopping."
      break
    fi
    log "drive not running (${complete:-?}/${total:-?} complete) — relaunching"
    DEADLINE="$HARD_STOP" MAX_ITERS=0 nohup "$REPO/tools/overnight-drive.sh" >/dev/null 2>&1 &
    sleep 30
  fi
  sleep 60
done
log "keeper finished"
