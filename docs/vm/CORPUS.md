# DP-7c — the jinteki-reference corpus port

The third rung of the verification ladder: implement the CR into a VM →
every CR worked example as a regression test (DP-7a, **done, 243/243**) →
**every card-interaction test the reference has, ported and passing here** →
the two priority decks.

The corpus is the pinned Clojure reference checkout
`~/Github/jinteki/jinteki-reference` at commit `4054730` (read-only; never
modified, never run in Docker except as an oracle for a genuinely ambiguous
behaviour). Re-measure with `python3 docs/vm/tools/corpus_survey.py`; the
numbers below are that script's output and this file is the narrative.

## 1. The measurement

```
tests: 3717   is-assertions: 13193   files: 43
by kind: card 3516 | engine 180 | basic 19 | other 2
distinct cards referenced: 1929 (of 2065 printed in data/cards.edn)
tests referencing no card at all: 35
```

By file (top ten, all under `test/clj/game/`):

| tests | file |
|---|---|
| 454 | `cards/programs_test.clj` |
| 444 | `cards/ice_test.clj` |
| 400 | `cards/events_test.clj` |
| 386 | `cards/resources_test.clj` |
| 347 | `cards/operations_test.clj` |
| 333 | `cards/identities_test.clj` |
| 329 | `cards/assets_test.clj` |
| 298 | `cards/hardware_test.clj` |
| 295 | `cards/agendas_test.clj` |
| 230 | `cards/upgrades_test.clj` |
| 199 | `core/*_test.clj` + `cards/basic_test.clj` (the engine slice) |

**The load-bearing fact of this whole rung**: the corpus is a *card* corpus,
not a rules corpus. A test is portable exactly when every card it names is
expressible in the kernel vocabulary, and the distribution has a very long
tail — implementing the most-used cards buys tests slowly:

| cards implemented (by corpus frequency) | tests fully covered |
|---|---|
| top 10 | 52 (1%) |
| top 25 | 71 (2%) |
| top 50 | 120 (3%) |
| top 100 | 218 (5%) |
| top 200 | 460 (12%) |
| top 400 | 1029 (27%) |
| top 800 | 2026 (54%) |
| top 1200 | 2828 (76%) |
| all 1929 | 3682 (99%) |

So DP-7c is not one wave; it is the axis the campaign now moves along, and
its odometer is *tests ported and passing*, not *files touched*. The card
layer it forces is the same artifact the deck work (P4) consumes and the same
artifact ARCHITECTURE §12 rule 4's re-derivation gate demands — one build,
three payers.

## 2. The helper vocabulary, mapped

The reference drives its VM procedurally through ~25 helpers in
`test/clj/game/test_framework.clj`. We drive ours declaratively: tests are
*plans* (ARCHITECTURE §12 rule 5), one per player, folded by `plan::Script`.
The port is therefore a translation of *procedure* into *policy* — each
helper becomes a plan rule (`when <Match> → <Reply>`) or an assertion on the
final state / `Transcript`, never a step loop.

| reference helper (uses) | jinteki-cr equivalent |
|---|---|
| `do-game` (3646) | `plan::Script::run(&mut vm, corp_plan, runner_plan)` |
| `new-game {:corp {:hand … :deck … :credits …}}` (3577) | `corpus::Setup` — hands, decks, credits, click counts as data; cards come from `cards::` (printed text), never from testkit shapes |
| `play-from-hand … "New remote"` (3290) | `Pick::InstallCard` (5.2.6d/5.2.7d basic install action) + a destination reply |
| `play-from-hand` (operation/event) | `Pick::PlayCard` (5.2.6e/5.2.7e) |
| `take-credits` (2978) | `Plan::ends_turn_with_credits()` — take basic credit actions until the action phase ends (5.6.2b) |
| `click-prompt "label"` (2221) | `Reply::ChooseNamed` / `Reply::take(Pick::Labeled)` / `Reply::Optional` — which one depends on the decision kind, and that is the translation's one judgement call |
| `qty` (1859) | a repeat count in the setup data |
| `rez` (1609) | `Reply::take(Pick::Rez(card))` in the paid window (9.2.7c) |
| `click-card` (1266) | `Reply::Targets(vec![…])` (1.15.2 announcement) |
| `get-ice` / `get-content` / `get-program` / `get-hardware` / `get-resource` / `get-scored` / `get-discarded` / `get-rfg` (≈3300) | assertions over `vm.st` zones (`corpus::at`, `corpus::in_zone`) |
| `run-on` (991) / `run-empty-server` (744) | `Pick::Run(server)` plus a pass policy through the run's windows |
| `run-continue` (1018) / `encounter-continue` / `run-continue-until` | plan rules that pass in the run's windows; our run structure asks only where the CR gives a decision |
| `card-ability` (950) | `Reply::take(Pick::Labeled(label))` in the paid window |
| `no-prompt?` (747) | `Plan::forbidding_the_rest()` or a `Transcript` absence assertion |
| `changed?` (712) | snapshot before / assert after around a `Reply::Halt` |
| `card-subroutine` (219) / `fire-subs` (150) | subroutine resolution in the Encounter Ice Phase; `Reply::Subroutines` where the ability targets them |
| `last-log-contains?` (148) | a `GameChange` assertion — the change buffer is our log |
| `advance` (100) / `click-advance` (62) | `Pick::Advance(card)` (5.2.6f basic advance action) |
| `gain-tags` (115) | no equivalent, deliberately: the reference reaches into state, we make tags only through effects (a card or a testkit shape) |
| `trash-resource` (11) | `Pick::TrashResource` (5.2.6g) |
| `purge` (9) | `Pick::Purge` (5.2.6h) |
| `starting-hand` (153) / `stack-deck` | setup data |
| `score-agenda` / `play-and-score` (388) | install + advance to the requirement + `Pick::Score` (9.2.7d) |

Three helpers have no honest equivalent and never will: `gain-tags`,
`damage`, `move` reach into game state directly. Where a test uses them for
*setup*, the port supplies the same state through a card that produces it;
where a test uses them as the *thing under test*, the test belongs to the
rule, not to the card, and the CR example suite already covers it.

## 3. The porting method

1. **Cards are data, re-derived from printed text.** `crates/jinteki-cr/src/
   cards.rs` holds real cards built exclusively through `PrintedCard` +
   `AbilityDef` + `Instruction` (§12 rule 3), each with the oracle text quoted
   verbatim in its doc comment, taken from the reference's own
   `data/cards.edn`. No kernel source branches on a card name (§12 rule 1);
   `cards.rs` is a *dictionary*, the same way `testkit.rs` is, and the kernel
   cannot tell the difference between a card from either.
2. **Tests are plans.** Ported tests live in
   `crates/jinteki-cr/tests/corpus.rs`, one Rust test per reference
   `deftest`, **named after the reference test** (`corp_basic_actions_purge`),
   so the two suites can be diffed by name. `tests/traceability.rs` already
   fails the build on a hand-rolled loop or an injection in any test file.
3. **The manifest is a ratchet.** `corpus.rs` carries `const PORTED: &[&str]`
   and a test asserting every entry names a real reference `deftest` and a
   real Rust test. A port cannot be quietly dropped.
4. **Every failure is triaged against the CR, and only three outcomes exist**
   — recorded, never skipped:
   - **(a) our defect** → fix the kernel, cite the rule;
   - **(b) the reference is wrong per the CR** → `docs/vm/UPSTREAM-DEFECTS.md`
     gets the rule citation and the divergent behaviour, and we keep ours;
   - **(c) the card is not yet expressible** → §5's gap list, prioritised by
     corpus frequency. Never faked.
5. **The CR wins.** If a port would drop a DP-7a example, the port is wrong;
   `dp7a_complete` is a ratchet and stays green.

## 4. Triage state

| bucket | tests | note |
|---|---|---|
| **ported and passing** | 31 | `tests/corpus.rs`, manifest-ratcheted (`dp7c_odometer`) |
| **blocked on the card layer** | 3508 | card tests; unblocked card by card, in frequency order |
| **blocked on kernel gaps** | 176 | the rest of the engine slice; §5 lists the machinery |
| **out of scope** | ~40 | `quotes_test`, `web/deck_test` and the reference-plumbing tests listed in `UPSTREAM-DEFECTS.md` §2 |

Ported so far: all 19 of `cards/basic_test.clj`; `run-timing-with-no-ice`,
`run-timing-with-an-ice`, `no-scoring-after-terminal` and `purge-corp` from
the `core/` slice; and `corroder`, `hedge-fund`, `beanstalk-royalties`, `ipo`,
`sure-gamble`, `hostile-takeover`, `pad-campaign`, `ice-wall` from the card
files. 19 real cards live in `crates/jinteki-cr/src/cards.rs`; the parallel
`crates/jinteki-cards` DSL (another agent's work, same session) is where the
card layer scales, and the two must be reconciled — see §7.

The wave order followed from the measurement. `cards/basic_test.clj` (19) and
the `core/` slice (180) came first because they name the *basic actions* —
and the action window offered no install, advance, purge or trash-resource
action at all (deviation 17's remaining half), so the first sub-wave of DP-7c
was kernel work, not translation work. The card layer starts immediately
after, in corpus-frequency order, because §1's table says nothing else moves
the odometer.

**One measured obstacle worth planning around: 1041 of the 3717 tests (28%)
— 93 engine, 948 card — call the reference's own internal API inside the test
body** (`(core/gain …)`, `(core/process-action "subroutine" …)`,
`core/num-cards-to-access`, `card-def`, …). Those are not translations of game
actions; they are pokes at the reference's implementation. A port either
re-expresses the poke as setup (usually possible: `(core/gain state :corp
:click 1)` is a starting click count) or the test is measuring the
reference's plumbing rather than the rules, in which case it is **out of
scope and recorded as such**, not counted as blocked. `docs/vm/
UPSTREAM-DEFECTS.md` carries that ledger alongside genuine divergences.

## 5. The gap list (prioritised; this is what the deck work consumes)

**Kernel machinery the corpus needs and the CR examples never did:**

1. ~~basic install action (5.2.6d/5.2.7d)~~ — **done, W15a**, with 8.5.16b's
   destination declaration as a real decision
2. ~~basic advance action (5.2.6f) + 1.18.3 "can be advanced"~~ — **done, W15a**
3. ~~basic trash-resource action (5.2.6g)~~ — **done, W15a**
4. ~~basic purge action (5.2.6h) + 10.1.2 purging~~ — **done, W15a**
5. mandatory/basic draw routed through `Instruction::Draw` (deviation 66) —
   the 8.4.5 procedure exists but the basic action does not use it
6. `Vm::view_of` over the change log (deviation 63) — the reference asserts on
   its log constantly (`last-log-contains?`, 148 uses)
7. 8.5.6's optional "may first trash any number" (deviation 8)
8. the 4.6.8f remote-server limit's second half (deviation 39)

**Card-text machinery, by corpus frequency of the cards that need it** (the
top cards are counted by how many tests name them):

**The card-layer machinery the next slice needs**, in the order the corpus
asks for it: the icebreaker class (break-with-subtype-restriction + strength
pump + the "interface" timing restriction); "the Runner loses [click]" as an
instruction (Enigma, #7 by frequency); an agenda-point modification (2.5 —
Project Beale's second sentence); a rez-cost modification scoped to a server
(Breaker Bay Grid); a movement into a score area for a non-agenda (Fan Site);
and a prohibition on SCORING scoped by when the agenda was installed (Clot).

| rank | card | tests | what it needs |
|---|---|---|---|
| 1 | Hedge Fund | 1021 | gain 9[credit] — one instruction |
| 2 | Sure Gamble | 601 | gain 9[credit] — one instruction |
| 3 | Ice Wall | 582 | ETR subroutine, "can be advanced" (1.18.3), strength per advancement counter |
| 4 | Corroder | 210 | icebreaker: break-subroutine ability with a subtype restriction, strength pump |
| 5 | Hostile Takeover | 187 | when-scored: gain 7[credit], take 1 bad publicity |
| 6 | PAD Campaign | 164 | start-of-turn conditional: gain 1[credit] |
| 7 | Enigma | 143 | "lose [click]" subroutine + ETR |
| 8 | Vanilla | 138 | ETR subroutine |
| 9 | Cache | 89 | virus counters + paid ability spending them |
| 10 | IPO | 79 | gain 13[credit] |

The icebreaker class (rank 4) is the single highest-value card-layer
mechanism left: `auto-pump-and-break` and `card-ability` on a breaker appear
in roughly a fifth of the card corpus.

## 6. What "ported" means here, exactly

A ported test asserts the same *observable outcome* as the reference test,
through our vocabulary. It does not assert the reference's internal state
shapes, its prompt text, or its click-by-click prompt sequence — those are
implementation artifacts of a different engine, and copying them would be the
overfit §12 exists to prevent. Where the reference test's assertion is about
the CR's behaviour, we assert it. Where it is about the reference's own
plumbing, the port drops it and says so in the test's doc comment.

## 7. Two card layers, and how they reconcile

`crates/jinteki-cr/src/cards.rs` (this rung, W15a/W15c) and
`crates/jinteki-cards` (the deck rung, `docs/cards/DSL.md`) are the same idea
built from opposite ends: printed text → `PrintedCard` through the public
vocabulary, with every unexpressible sentence marked rather than
approximated. `cards.rs` is Rust and hand-written for the tests that need it;
`jinteki-cards` is a designer-facing DSL with a parser, a denotation pass and
its own `unimplemented:` ledger.

They must not stay parallel. The reconciliation is one-directional and cheap:
**`cards.rs` becomes `.cards` files** once the DSL can say what those 19 cards
say (it already says most of it), and `corpus.rs` loads them through
`jinteki-cards`. The DSL's `unimplemented:` marker and this file's
`UNIMPLEMENTED:` doc comment then become one ledger, counted by one test.
Nothing about the port's assertions changes — the cards are data either way,
and the VM cannot tell them apart. Do this before the card count grows past
what a hand port can carry; the corpus wants ~1200 cards to reach 76%.
