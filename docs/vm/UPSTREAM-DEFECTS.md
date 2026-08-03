# Upstream defects and out-of-scope tests (DP-7c triage ledger)

Every failure of a ported jinteki-reference test is triaged against the CR,
which is the specification, and lands in exactly one of three places
(`docs/vm/CORPUS.md` §3.4):

- **(a) our defect** → the kernel is fixed and the rule cited; the fix is
  recorded in `docs/vm/WAVES.md`'s commit ledger, not here.
- **(b) the reference diverges from the CR** → an entry in §1 below. We keep
  our behaviour and the port asserts the CR's outcome.
- **(c) the card is not yet expressible** → `CORPUS.md` §5's gap list.

A fourth category is not a failure at all but must be recorded so that the
odometer stays honest: a reference test that measures the reference's own
plumbing rather than the rules (§2).

## 1. Reference divergences from the CR

*No entries yet.* Through DP-7c sub-wave 2 (23 tests ported), every
divergence found has been ours, not the reference's. The one substantive
disagreement so far — whether the Corp may still score after playing a
terminal operation — was **our** defect: see W15b, `Instruction::
EndActionPhase` used to take the player's clicks (so 5.6.2's loop returned to
step (a) and opened one more paid window with the (S) option) where CR 5.6.2
makes ending the phase a jump to step (d). The reference was right.

When an entry does appear it carries: the reference test name and file, the
rule id and quoted CR text, the divergent behaviour on both sides, and the
reason the CR settles it our way.

## 2. Out of scope: tests of the reference's own plumbing

These are not ports we owe. They exercise the reference implementation's
internals — its prompt queue, its log strings, its async macros, its Clojure
helper functions — not the rules of Netrunner, and a second implementation
has nothing to answer for them.

Measured (`docs/vm/tools/corpus_survey.py` plus a body scan): **1041 of 3717
tests call the reference's internal API inside the test body** — 93 engine
tests and 948 card tests. That is not the same as being out of scope: most of
them poke state that a port can express as *setup* (`(core/gain state :corp
:click 1)` is a starting click count; `(core/command-counter …)` is a counter
placed before the script runs), and those are ported normally, with the poke
re-expressed. Out of scope is the narrower set below.

| file | tests | why |
|---|---|---|
| `core/async_test.clj` | 8 | tests the reference's `wait-for`/`continue-ability` macros |
| `core/say_test.clj` | 3 | chat commands and log strings |
| `core/stats_test.clj` | 4 | the reference's per-game statistics counters |
| `core/effects_test.clj` | 3 | `gather-effects`/`sum-effects`, internal functions |
| `core/card_test.clj` | 3 | `has-subtype?` and friends, internal predicates |
| `core/abilities_test.clj` | 3 | that every ability in the card DB carries a `:label`, an implementation convention |
| `core/access_test.clj` | 11 | `core/num-cards-to-access` returns an internal map shape |
| `core/actions_test.clj` (undo) | 1 | the reference's undo/rollback feature |
| `quotes_test.clj`, `web/deck_test.clj` | 2 | flavour text and deckbuilding web endpoints |

The *rules* content behind several of these is covered elsewhere: the access
counts by DP-7a's §7.3/§7.4 examples, the ability model by §9.1's, the
subtype predicates by §2.16's.

One narrower case, recorded because it looks like a port and is not:
`core/runs_test.clj::etr-outside-of-runs-does-not-prevent-new-runs` fires a
subroutine outside any run through `core/process-action "subroutine"` — a
driver backdoor with no counterpart in a rules-legal game. The CR content
(6.1.4c: "end the run" with no run does nothing that lingers) is already a
DP-7a test (`example_rule_end_run_no_run_or_encounter_1`, W7e).
