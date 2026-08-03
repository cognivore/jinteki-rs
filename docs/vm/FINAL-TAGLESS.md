# The CR VM, final tagless — the algebra cut

Normative for `crates/jinteki-cr`, subordinate to DESIGN.md and paired with
ARCHITECTURE.md (which says *what* the CR machinery is; this says *how it is
structured*). Where the two disagree, this document wins on structure and
ARCHITECTURE.md wins on rules semantics.

## 1. Why: the measured problem

W1–W4 built a correct kernel the fastest available way — one concrete `Vm`
with a growing instruction enum. Measured at 82/243 examples:

| | count | what it means |
|---|---|---|
| `Instruction` variants | 125 | card-shaped, not CR-shaped: `PumpStrengthSelf`, `ReduceRunnerMemoryThisTurn`, `CorpDiscards`, `RestrictAccessToSelf` |
| match arms in `vm.rs` | 254 | one evaluator, growing linearly with card text |
| `vm.rs` lines | 5,845 | the whole semantics in one file |
| `unreachable!`/`todo!` | 36 | frame-shape assertions the type system should make impossible |
| implementations of anything | 1 | nothing forces the cut |

That last row causes the rest. With one implementation, "narrow algebra" is a
preference; the cheapest way to express any new card is a new variant plus a
new arm, and the cost only shows up 160 examples later. It already produced
three defects the user caught by eye: `SelfStrengthPerServerIce`,
`TraceSurveyorX`, `tk::inject_*`. The vocabulary must be forced narrow by
something structural, and in final tagless that something is **a second
honest interpreter that could not implement a card-shaped operation**.

DESIGN.md already requires this: SYS-C-3 (narrow algebras, no `unreachable!`
in an instance — "the moment you write unreachable, the algebra is wrong"),
SYS-D-6 (every DSL construct denotes into the algebras, no direct state
access), SYS-F-3/4/5/8 and SYS-S-1 (redaction, replay, speculative, test
interpreters), SYS-Q-1 (sans-IO core). The CR pivot changed the *specification
of the rules*, not the *architecture of the engine*. This document puts the
architecture back.

## 2. The cut: three layers, and what may live in each

**Data (initial encoding).** Cards, abilities, declarations, instructions,
quantities, target filters, the §11 step tables. Inert values. Auditable:
`cite!` coverage, the odometer, the §9.11 transcriber, replay logs and the
re-derivation gate all consume them as data. Nothing here is a closure.

**Algebras (final encoding).** Narrow Rust traits — the *capabilities* the CR
needs. Card semantics are expressed only as calls through these. This is the
layer that gets multiple implementations.

**Evaluator (the kernel).** Checkpoints §10.3, windows §9.2, frames, imminence
§9.9, the step tables §11. Pure sequencing logic, generic over the algebras.
It is not itself an algebra: it is the one thing that knows *when* card
semantics run, and it is the same under every interpreter.

Rule of placement: if a card can observe it, it is data or algebra; if only
the rules can observe it, it is evaluator.

## 3. The algebra catalog

Narrow, CR-shaped, and named for rules constructs. Each trait lists the CR
sections it serves. Methods take already-evaluated arguments (a `Quantity` is
evaluated by `Quantities` first, so no algebra evaluates selectors itself).

```rust
pub trait Zones {                        // §1.8, §8.5, §8.8
    fn move_object(&mut self, o: ObjectId, to: ZoneRef, at: Placement) -> ObjectId;
    fn install(&mut self, o: ObjectId, dest: InstallDest) -> ObjectId;
    fn swap(&mut self, a: ObjectId, b: ObjectId);
    fn host(&mut self, o: ObjectId, host: ObjectId);
}

pub trait Pools {                        // §1.11, §5.6, §10.4-10.7
    fn adjust_credits(&mut self, side: Side, delta: i64);
    fn adjust_clicks(&mut self, side: Side, delta: i64);
    fn adjust_tags(&mut self, side: Side, delta: i64);
    fn adjust_bad_publicity(&mut self, delta: i64);
}

pub trait Counters {                     // §1.12
    fn place(&mut self, on: ObjectId, kind: CounterKind, n: u32);
    fn remove(&mut self, on: ObjectId, kind: CounterKind, n: u32) -> u32;
}

pub trait Cards {                        // §8.1-8.4, §8.7
    fn draw(&mut self, side: Side, n: u32) -> Vec<ObjectId>;
    fn reveal(&mut self, os: &[ObjectId], to: Audience);
    fn search(&mut self, zone: ZoneRef, filter: &Filter) -> Vec<ObjectId>;
    fn shuffle(&mut self, zone: ZoneRef);
}

pub trait Damage { fn deal(&mut self, kind: DamageKind, n: u32); }   // §10.5

pub trait Runs {                         // §6
    fn initiate(&mut self, server: ServerRef, source: ObjectId);
    fn end_run(&mut self);
    fn breach(&mut self, server: ServerRef);
    fn access(&mut self, o: ObjectId);
    fn bypass_current_ice(&mut self);
}

pub trait Statuses {                     // §1.13, §8.9
    fn rez(&mut self, o: ObjectId, ignore_cost: bool);
    fn derez(&mut self, o: ObjectId);
    fn expose(&mut self, o: ObjectId);
    fn trash(&mut self, o: ObjectId, cause: TrashCause);
}

pub trait Effects {                      // §9.9, §9.10, §9.12.5
    fn linger(&mut self, e: LingeringEffect) -> EffectId;
    fn expire(&mut self, id: EffectId);
    fn modify_imminent(&mut self, sel: ValueSelector, delta: i64) -> Applied;
    fn prevent(&mut self, sel: ValueSelector, amount: Prevention) -> Applied;
    fn replace(&mut self, sel: ValueSelector, with: Vec<Instruction>) -> Applied;
}

pub trait Subroutines {                  // §9.8
    fn grant(&mut self, to: ObjectId, sub: AbilityDef, at: SubPosition) -> SubId;
    fn break_subs(&mut self, on: ObjectId, which: SubSelector);
    fn resolve_sub(&mut self, id: SubId);
}

pub trait Decide {                       // §9.2, §9.11.4f/g, §10.8, §10.14
    fn ask(&mut self, spec: DecisionSpec) -> DecisionAnswer;
}

pub trait Quantities { fn eval(&self, q: &Quantity) -> i64; }        // §9.12.2

pub trait Query {                        // §9.12.1, and SYS-S-1
    fn characteristic(&self, o: ObjectId, c: Characteristic) -> Value;
    fn objects(&self, f: &Filter) -> Vec<ObjectId>;
    fn zone(&self, z: ZoneRef) -> ZoneView;      // viewpoint-indexed
}

pub trait Rng { fn permutation(&mut self, n: usize) -> Vec<usize>; } // SYS-F-4

pub trait Log { fn record(&mut self, ev: RulesEvent); }             // derived, never hand-written
```

**Narrowness has teeth**: a card's denotation names only the traits it needs,
so a pretty-printer that implements `Log + Query` can denote "gain 2
[Credits]" without having anything to say about `Runs`. The moment a
denotation needs a bound it should not need, the vocabulary is wrong.

## 4. Denotation: cards are data that call the algebras

`Instruction` splits into families, each with a denotation function whose
bounds are exactly its needs (SYS-D-6):

```rust
pub fn denote_pool<M: Pools + Quantities>(m: &mut M, i: &PoolInstr) -> Outcome;
pub fn denote_zone<M: Zones + Query + Quantities>(m: &mut M, i: &ZoneInstr) -> Outcome;
pub fn denote_run<M: Runs + Decide + Query>(m: &mut M, i: &RunInstr) -> Outcome;
// …one per family; `denote` dispatches families and is the ONLY entry point
// the evaluator uses to run card text.
```

The evaluator owns *when* a denotation runs (imminence, interrupt points,
checkpoints); the denotation owns *what it means*. No card text may reach
state except through these functions.

## 5. Interpreters — at least two honest ones per algebra

| interpreter | implements | honest use | DESIGN.md |
|---|---|---|---|
| `Prod` | all | the game | — |
| `Plan` (scripted) / `Bot` | `Decide` | tests and the random-walk opponent | F-8 |
| `Replay` | `Rng`, `Decide` | reproduce from log alone: no RNG, no clock | F-4, DP-2 |
| `Legality` | `Query`, `Quantities`, + no-op mutators | "is there a legal way to do this?" without mutating | F-2 |
| `Viewpoint` | `Query` only | the ONLY path to client payloads | S-1, DP-5 |
| `Speculative` | `Rng`, `Query` | fork + resample hidden zones from public knowledge only | F-5/6 |
| `Narrate` | `Log`, `Query` | log lines derived from denotation, not written by hand | D-4 |

The harness migration already in flight is the first of these: **plans are the
second interpreter of `Decide`**, and the bot is the third. That work is not a
detour from this refactor, it is step zero of it.

## 6. What may be an `Instruction` variant

The vocabulary is CR primitives, not card shapes. A variant is legitimate only
if it names a construct the Comprehensive Rules names. Everything specific to
a card is expressed by *parameters*: `Quantity` selectors (§12 rule 6),
`Filter` target expressions, `Duration`, and composition.

Composition operators (from §9.11's own grammar) carry what used to be
bespoke variants: `Seq`, `Optional` (9.6.9), `ChooseOne` (9.11.4g),
`ForEach` (9.12.2b), `NestedCost` (9.11.4f), `IfThen` (conditional text).

Worked collapses, each replacing a current variant:

- `PumpStrengthSelf { amount }` → `ModifyCharacteristic { target: Self_, characteristic: Strength, delta: Quantity, duration: Duration }`
- `ReduceRunnerMemoryThisTurn(n)` → the same with `characteristic: MemoryLimit`, `duration: EndOfTurn`
- `CorpDiscards { count }` → `Discard { who: Corp, count: Quantity, chooser: Controller }`
- `RestrictAccessToSelf` → `Effects::linger(Restriction { … })`
- `GrantSubroutinesToSelf { … }` → `Subroutines::grant(target: Self_, …)`
- `TraceSurveyorX` (already retired) → `Trace { base: Quantity, … }`

Target: **≤45 variants** across all families at FT-2 exit, with every deleted
variant's example test passing unchanged.

## 7. Staged migration — each stage green, odometer never regresses

| stage | content | exit gate |
|---|---|---|
| **FT-0** | plan-driver harness; all tests declarative; injections retired (in flight) | 82/243 green, no `vm.step()` loops, no `tk::inject_*` |
| **FT-1** | extract the traits; `Vm` implements them; move the 254 arms into family denotations. **No behavior change.** | 82/243 green; `vm.rs` no longer contains card semantics |
| **FT-2** | collapse the vocabulary per §6; `Filter`/`Duration` languages land | ≤45 variants; every example still green; zero card-shaped variants |
| **FT-3** | second interpreters: `Legality`, `Viewpoint`, `Replay` | each trait has ≥2 impls; DP-2/DP-5 wired; CT compile-fail tests |
| **FT-4** | resume the odometer on the clean substrate → 243/243 | DP-7a green, DP-7b coverage climbing |
| **FT-5** | DP-7c corpus port, then the two decks | DP-7c triaged green |

FT-1 and FT-2 are mechanical, large, and boring; that is the point. They are
cheap at 82 examples and ruinous at 243.

## 8. Enforcement (make the structure unfakeable)

1. **No `unreachable!`/`panic!`/`todo!` in any algebra implementation** (SYS-C-3).
   A test greps the impl blocks; a hit is a factoring defect, not a lint waiver.
   The evaluator may assert its own frame invariants only where the frame type
   cannot express them — each such site carries a comment saying why.
2. **≥2 implementations per trait**, asserted by a test that enumerates impls.
   A trait with one impl is a trait that has not been designed.
3. **No card names in the vocabulary** and **no bespoke quantities** —
   ARCHITECTURE §12 rules 1–2 and 6, unchanged.
4. **Denotation bound audit**: no denotation function may take a bound its
   family does not need; reviewed per commit, and naturally caught because
   the narrow interpreters will not compile against a wrong bound.
5. **Differential gates**: DP-1 (pure ≍ production), DP-2 (record/replay),
   DP-5 (redaction fuzz), DP-7a/b/c. The interpreters exist to be compared —
   agreement is the price of admission (SYS-C-4).

## 9. What this does not change

The CR remains the specification: ARCHITECTURE.md's reading of §9–§11, the
citation discipline, the 243 examples, the deviation ledger in WAVES.md, and
the cutover plan (old engine stays deployed until DP-7a/b/c are green). This
document changes only *how the semantics are expressed* — from one concrete
evaluator that knows every card shape, to narrow algebras that no single card
can widen.
