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

BRIEF='You are continuing the jinteki-rs priority-deck card drive, unattended.

Read FIRST: docs/vm/WAVES.md, then docs/cards/EDSL.md, then
crates/jinteki-cards/src/decks/{andromeda,gauntlet}.rs.

GOAL: raise the count in `cargo test -p jinteki-cards --test decks --
--nocapture` ("priority decks: N cards, M complete"). Finish ONE partial
card this run, then stop.

METHOD:
1. Pick the partial card needing the least new kernel vocabulary.
2. Check whether its `UNIMPLEMENTED:` doc comment is STALE — the vocabulary
   has repeatedly grown past what those comments claim. Several cards were
   already sayable and nobody had revisited them.
3. Implement it in the EMBEDDED DSL (typed Rust builders in
   crates/jinteki-cards). This is NOT a text format and nothing is parsed.
4. Add a behaviour test in crates/jinteki-cards/tests/behaviour.rs, driven
   by a PLAN. Never add a *_for_test backdoor to the VM.
5. Run BOTH gate halves:
     nix develop --command cargo test --workspace
     nix build .#default && rm -f result
6. Commit with a message in the established style (see git log).

HARD RULES (docs/vm/ARCHITECTURE.md §12):
- No card names in kernel vocabulary. Thresholds, polarity, scope and
  windows are CONTENT on one atom, never a new atom each.
- A clause the vocabulary cannot express gets .unimplemented("<sentence>").
  NEVER approximate or fake it.
- Odometers never regress: DP-7a stays 243/243.
- Any cite!("rule_...") id must exist in docs/rules/cr-index.json —
  tests/traceability.rs enforces this.
- Do NOT touch crates/jinteki-core (the legacy engine the server runs).

If the workspace is red when you start, FIX THAT FIRST and commit nothing
else. If you cannot finish a card cleanly, revert your changes and stop —
leave the tree green.'

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
