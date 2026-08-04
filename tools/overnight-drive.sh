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

BRIEF='You are continuing the jinteki-rs DECK QUEUE, unattended.

Read FIRST: docs/vm/DECK-QUEUE.md (the mandated deck order and the exact
lists), then docs/vm/WAVES.md, then docs/cards/EDSL.md, then an existing deck
module such as crates/jinteki-cards/src/decks/gauntlet.rs for house style.

GOAL: finish the decks in DECK-QUEUE.md order. Deck 1 is done. Work the
FIRST deck in that file that is not yet complete, and do ONE unit of work
this run, then stop:

  * If the deck has no module yet: create
    crates/jinteki-cards/src/decks/<key>.rs with one function per DISTINCT
    card in printed order, register it in decks/mod.rs, and add its DeckSpec
    (list, copy counts, CR 1.5.4a pile) to crates/jinteki-server/src/cr.rs.
    Cards already implemented for an earlier deck are REUSED, never copied.
    Stub every not-yet-written card with its printed text from
    crates/jinteki-core/carddata/cards.json plus .unimplemented(...) for each
    printed sentence, so the odometer counts it honestly. Commit that.
  * Otherwise: implement the next incomplete card of that deck, whole.
    Commit that.

METHOD:
1. Read the exact printed text from crates/jinteki-core/carddata/cards.json.
   Never work from memory.
2. VERIFY any UNIMPLEMENTED: doc comment before believing it — the kernel
   vocabulary has repeatedly grown past what those comments claim, and stale
   ones have already yielded several free cards.
3. Write it in the EMBEDDED DSL: typed Rust builders in crates/jinteki-cards.
   Nothing is parsed; .text(...) is data for the SYS-D-10 agreement test.
4. Add a behaviour test in crates/jinteki-cards/tests/behaviour.rs driven by
   a PLAN (plan::play / plan::Script). Never add a *_for_test backdoor to the
   VM and never write a vm.step() loop.
5. Run: nix develop --command cargo test --workspace  (must be fully green)
6. Commit in the established style (see git log).

HARD RULES (docs/vm/ARCHITECTURE.md section 12):
- No card names in kernel vocabulary. Thresholds, polarity, scope, windows
  and namespaces are CONTENT on one atom, never a new atom per card.
- A clause the vocabulary genuinely cannot express keeps
  .unimplemented("<exact printed sentence>"). NEVER approximate or fake it.
- Odometers never regress: DP-7a stays 247/247.
- Every cite!("rule_...") id must exist in docs/rules/cr-index.json.
- Do NOT touch crates/jinteki-core (the legacy engine the live server uses).
- Do NOT run nix build, do NOT push, and do NOT deploy. Commit locally only.

If the workspace is red when you start, FIX THAT FIRST and commit nothing
else. If you cannot finish cleanly, revert your changes and stop, leaving
the tree green.'

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
