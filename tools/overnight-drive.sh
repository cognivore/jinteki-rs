#!/usr/bin/env bash
# overnight-drive.sh — unattended card-implementation drive.
#
# WHY THIS EXISTS: Claude Code only executes while a turn is active. Nothing
# runs between user messages, and `CronCreate` jobs are session-only (in
# memory, gone when the session exits, and they fire only while the REPL is
# idle). So "keep working overnight" is NOT achievable from inside a chat
# session — it needs an OS-level scheduler invoking the CLI headlessly.
# That is this script, driven by the launchd agent in nix/launchd/.
#
# Each iteration is one headless `claude -p` run with a self-contained brief.
# The agent is told to finish a card, run the commit gate, and commit — so
# progress is durable after every iteration and a crash costs one card, not
# the night.
#
# Usage:
#   tools/overnight-drive.sh                 # run until DEADLINE (default 12:00)
#   DEADLINE=09:30 tools/overnight-drive.sh  # stop after 09:30 local
#   MAX_ITERS=3 tools/overnight-drive.sh     # cap iterations (smoke test)
#
# Logs to tools/../.overnight/drive-<date>.log (gitignored).

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO" || exit 1

DEADLINE="${DEADLINE:-12:00}"
MAX_ITERS="${MAX_ITERS:-0}" # 0 = unlimited
LOG_DIR="$REPO/.overnight"
mkdir -p "$LOG_DIR"
LOG="$LOG_DIR/drive-$(date +%Y%m%d).log"

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" | tee -a "$LOG"; }

past_deadline() {
  # Compare HH:MM lexically — valid for zero-padded 24h times.
  [[ "$(date +%H:%M)" > "$DEADLINE" ]]
}

BRIEF_FILE="${BRIEF_FILE:-$REPO/tools/briefs/identities.txt}"
BRIEF="$(cat "$BRIEF_FILE")"

log "drive starting; deadline ${DEADLINE}; repo ${REPO}"

iter=0
while true; do
  if past_deadline; then
    log "past deadline ${DEADLINE} — stopping"
    break
  fi
  if [[ "$MAX_ITERS" != 0 && "$iter" -ge "$MAX_ITERS" ]]; then
    log "reached MAX_ITERS=${MAX_ITERS} — stopping"
    break
  fi
  iter=$((iter + 1))

  before="$(git rev-parse HEAD)"
  log "iteration ${iter} starting at ${before:0:8}"

  # --dangerously-skip-permissions: unattended runs cannot answer prompts.
  # Scoped by the brief to this repo, and every iteration is gated by the
  # test suite and left as a reviewable commit.
  claude -p "$BRIEF" \
    --permission-mode bypassPermissions \
    >>"$LOG" 2>&1
  rc=$?

  after="$(git rev-parse HEAD)"
  if [[ "$before" == "$after" ]]; then
    log "iteration ${iter} produced NO commit (exit ${rc}) — stopping to avoid a spin loop"
    break
  fi
  log "iteration ${iter} committed ${after:0:8} (exit ${rc})"
  git --no-pager log --oneline -1 | tee -a "$LOG"
done

log "drive finished after ${iter} iteration(s)"
git --no-pager log --oneline "$(git rev-parse HEAD)" -5 | tee -a "$LOG"
