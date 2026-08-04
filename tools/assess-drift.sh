#!/usr/bin/env bash
# assess-drift.sh — does the campaign's work still meet the campaign's rules?
#
# Unattended waves are fast and confident, which is exactly what makes drift
# expensive: a wrong pattern gets copied into the next batch before anyone
# reads it. Every check here is one that has ALREADY been violated at least
# once in this repo, so none of them is hypothetical.
#
# Exit 0 = clean. Exit 1 = drift found; the campaign should stop and a human
# should look. Prints one line per finding, nothing when clean.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; cd "$REPO" || exit 1
IDQ="docs/vm/IDENTITY-QUEUE.md"
bad=0
say() { echo "DRIFT: $*"; bad=1; }

# 1. A ticked box over a faked card. The whole odometer is worthless if a
#    box can be ticked while the card still carries .unimplemented(...).
#    An identity is allowed to be incomplete — it just may not be TICKED.
ticked=$(grep -c '^- \[x\]' "$IDQ" 2>/dev/null || echo 0)
while IFS= read -r name; do
  [[ -z "$name" ]] && continue
  # find the card fn carrying this printed name, then look for a marker in it.
  # Printed names contain quotes (Ken "Express" Tenma) and asterisks (NBN: The
  # World is Yours*), which are ESCAPED or literal in Rust source — so match
  # the source form, not the markdown form, or every such card reads missing.
  src=$(printf '%s' "$name" | sed 's/"/\\"/g')
  f=$(grep -rlF "card(\"$src\")" crates/jinteki-cards/src/decks/ 2>/dev/null | head -1)
  # A printed title ending in '*' (NBN: The World is Yours*) collides with the
  # queue's **bold** markers, so the asterisk is lost on the way out. Retry
  # with it before believing the card is missing.
  if [[ -z "$f" ]]; then
    src="${src}*"
    f=$(grep -rlF "card(\"$src\")" crates/jinteki-cards/src/decks/ 2>/dev/null | head -1)
  fi
  [[ -z "$f" ]] && { say "queue ticks \"$name\" but no card function defines it"; continue; }
  if awk -v n="card(\"$src\")" 'index($0,n){f=1} f&&/\.build\(\)/{exit} f&&/\.unimplemented\(/{print;exit}' "$f" | grep -q .; then
    say "\"$name\" is ticked in the queue but still carries .unimplemented(...)"
  fi
done < <(grep '^- \[x\]' "$IDQ" 2>/dev/null | sed 's/^- \[x\] \*\*//; s/\*\*.*//')

# 2. Invented rule ids. tests/traceability.rs enforces this, but it has been
#    broken by a wave before and the failure is easy to skim past.
if ! nix develop --command cargo test -p jinteki-cr --test traceability >/dev/null 2>&1; then
  say "traceability fails — an invented cite!(\"rule_...\") id, or a stale scan path"
fi

# 3. The DP-7a ratchet. 247 worked examples; a wave must never regress it.
n=$(nix develop --command cargo test -p jinteki-cr --test cr_examples 2>/dev/null \
      | sed -n 's/^test result: ok\. \([0-9]*\) passed.*/\1/p' | tail -1)
[[ -n "${n:-}" && "$n" -ge 247 ]] || say "DP-7a regressed or did not run: got '${n:-none}', expected >= 247"

# 4. CR 9.11.3 — one sentence is one instruction. This exact mistake has been
#    made twice; the guard test pins the three known cards by shape.
if ! nix develop --command cargo test -p jinteki-cards --test behaviour an_and_sentence >/dev/null 2>&1; then
  say "the 9.11.3 guard fails — an \"X and Y\" sentence split into two instructions again"
fi

# 5. Tests that assert nothing a reader can understand. The brief requires a
#    message on every assertion; a bare assert_eq! is not reviewable.
# A RATCHET, not a threshold: 39 predate the rule and are not worth churning,
# but the count must never grow. A new wave adding message-less assertions is
# drift; the old ones are debt.
BARE_MAX="${BARE_MAX:-39}"
bare=$(grep -c 'assert_eq!([^,]*, [^,]*);$' crates/jinteki-cards/tests/behaviour.rs 2>/dev/null || echo 0)
[[ "$bare" -le "$BARE_MAX" ]] || say "message-less assertions grew ${BARE_MAX} -> ${bare}; the brief requires a message on every assertion"

# 6. The live gate: both decks still playable. Enlisting an incomplete
#    identity into a pile takes the lobby down.
if ! nix develop --command cargo test -p jinteki-server the_two_eternal_decks_are_playable >/dev/null 2>&1; then
  say "the two priority decks are no longer playable — a pile probably took an incomplete card"
fi

# 7. The artifact build. A workspace-green tree can still fail this because
#    nix/package.nix filters the source tree (WAVES.md, W7e). Skipped when
#    the tree is dirty, since that measures someone else's half-edit.
if [[ -z "$(git status --porcelain)" ]]; then
  if ! nix build .#default >/tmp/assess-build.log 2>&1; then
    say "nix build .#default FAILS on the committed tree (see /tmp/assess-build.log)"
  fi
  rm -f result
fi

[[ "$bad" -eq 0 ]] && echo "clean: ${ticked}/150 ticked, no drift found"
exit "$bad"
