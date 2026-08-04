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

BRIEF='You are implementing EVERY identity in jinteki-rs, unattended.

Read FIRST: docs/vm/IDENTITY-QUEUE.md (the full list, grouped, with printed
text and a checkbox each), then docs/vm/WAVES.md, then docs/cards/EDSL.md,
then crates/jinteki-cards/src/decks/gauntlet.rs (Nebula/Gemilang show the
double-sided pattern) and decks/andromeda.rs.

GOAL: work the queue top to bottom. Runner identities first — they are the
ones Rebirth and DJ Fenris can reach. Do ONE unit of work this run, then
stop:

  * If the faction module does not exist yet, create
    crates/jinteki-cards/src/decks/identities/<name>.rs, register it in
    decks/mod.rs, and implement the FIRST few identities of that faction in
    it. Commit that.
  * Otherwise implement the next 8-12 unchecked identities of the faction
    currently in progress, whole. Commit that. Batch them: the full workspace
    suite runs once per commit and is most of the wall-clock, so a bigger
    batch is far cheaper than more commits. If an identity in the batch needs
    kernel vocabulary you would rather not rush, SKIP it, leave its box
    unticked, and take the next one — do not block the batch on the hardest
    card.

Tick the boxes in docs/vm/IDENTITY-QUEUE.md as you go and update the
"Implemented: N / 150" line, in the same commit.

METHOD:
1. Read the exact printed text from crates/jinteki-core/carddata/cards.json.
   Never work from memory.
2. Write it in the EMBEDDED DSL: typed Rust builders in crates/jinteki-cards.
   Nothing is parsed; .text(...) is data for the SYS-D-10 agreement test.
2a. CR 9.11.3: one SENTENCE is one INSTRUCTION. "draw 2 cards and take 1 tag"
   is combined([draw(...), give_tags(...)]) — NOT two instructions. Splitting
   it invents a checkpoint, a reaction window and a second interrupt window
   the card does not have, so a prevention effect gets two chances where the
   card gives one. 9.12.2c is about aggregating a CALCULATED QUANTITY ("for
   each"), NOT about whether a sentence splits — do not cite it for that. The
   only splits are 9.11.4b-g. See docs/cards/EDSL.md.
3. Add a behaviour test per identity in
   crates/jinteki-cards/tests/behaviour.rs, driven by a PLAN. Never add a
   *_for_test backdoor to the VM and never write a vm.step() loop.
4. Run: nix develop --command cargo test --workspace  (must be fully green)
5. Commit in the established style (see git log).

ENLISTING INTO A PILE — the rule that must not be broken:
An identity may be added to a deck spec pile in crates/jinteki-server/src/cr.rs
ONLY when it is is_complete(). readiness() holds pile cards to the same bar as
deck cards, so enlisting a partial identity makes BOTH priority decks
unplayable and the live site refuses to start a game. When you finish a batch
of Criminal identities, you MAY add the complete ones to ANDROMEDA_PILE — and
if you do, verify `cargo test -p jinteki-server` still passes
cr::tests::the_two_eternal_decks_are_playable before committing.

HARD RULES (docs/vm/ARCHITECTURE.md section 12):
- No card names in kernel vocabulary. Thresholds, polarity, scope, windows
  and namespaces are CONTENT on one atom, never a new atom per card.
- A clause the vocabulary genuinely cannot express keeps
  .unimplemented("<exact printed sentence>"). NEVER approximate or fake it.
  An identity with an unimplemented clause simply does not join a pile yet.
- Odometers never regress: DP-7a stays 247/247.
- Every cite!("rule_...") id must exist in docs/rules/cr-index.json.
- Do NOT touch crates/jinteki-core (the legacy engine the live server uses).
- Do NOT run nix build, do NOT push, and do NOT deploy. Commit locally only.

If the workspace is red when you start, FIX THAT FIRST and commit nothing
else. If you cannot finish cleanly, revert your changes and stop, leaving the
tree green.'

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
