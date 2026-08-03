# The CR Virtual Machine — architecture

Status: NORMATIVE for the P1.5 rules VM (`crates/jinteki-cr`). Derived from a
close reading of the Comprehensive Rules v26.03 (see `docs/rules/`); every
design decision below cites the rule that forces it. If code and this document
disagree, the CR wins, then this document, then the code.

## 0. The one-paragraph model

The CR specifies a virtual machine: **timing structures** are programs whose
steps are data (§11 gives them as numbered step tables); **abilities** are
procedures made of **instructions**; **priority windows** are the scheduler;
the **checkpoint** (§10.3) is the innermost loop that watches a **change
buffer** and turns state deltas into **pending ability instances**; and
**interrupts/replacements** (§9.9) are a modification stage that rewrites an
instruction's **expected effects** after it becomes **imminent** and before it
resolves. Every player decision suspends the machine — the VM is a coroutine
that yields typed `Decision` requests and never blocks. That last property is
what lets the existing prompt-driven server/UI/bot sit on top unchanged.

## 1. Objects, zones, characteristics

- `ObjectId`-addressed objects: cards and counters (9.1.1 covers counter
  abilities; 10.1.4 cards-as-counters conversions). Zones per §4.
- **Characteristics pipeline** (9.12.1d–e): an object's effective
  characteristics (subtypes, strength, abilities present, X definitions) are
  computed from printed values by applying active static/lingering effects in
  dependency order: apply independent effects first; on a dependency loop,
  hosted objects' effects ignore their dependence on their host's effects
  (the Hush/Magnet rule). Value stacking: set → increase → decrease
  (9.12.1a); subtype add/remove by counting (9.12.1b).
- Ability **activity** (9.1.7) with the §9.1.8 exceptions table (access
  abilities, zone-scoped abilities, cost/req modifiers, advance-permitters,
  trigger-met-then-moved 9.1.8g, encounters-while-uninstalled 9.1.8h,
  persistent 9.12.5). Activity is computed, never cached wrongly: a trashed
  Hostile Infrastructure keeps its ability active until its instances resolve.

## 2. The change buffer and checkpoints (§10.3)

- Every mutation the VM performs appends a `GameChange` record (credits
  gained, card trashed, card installed, run declared successful, damage
  suffered, …) to a buffer.
- **Checkpoint procedure** implements 10.3.1 steps (a)–(l) in order, byte for
  byte: (a) each active conditional ability examines *the changes since the
  beginning of the last checkpoint* (9.6.5a) and creates pending instances —
  one per occurrence (9.6.4b), with occurrence multiplicity governed by the
  aggregation rules (9.12.2: Hostile Infrastructure sees 3 trashes, Warroid
  Tracker sees 1 event); (b) expire lingering effects; (c) 7-point win/draw;
  (d) unique ◆ / console trashing; (e) restriction violations trashed by the
  **minimal appropriate set** algorithm with controller/active-player choice;
  (f)(g) hosted-orphan trashing with the 9.5.5 set-aside exemption, repeated
  to fixpoint; (h) empty remotes cease; (i) unoccupied ice positions cease
  (except the Runner's current position); (j) breach-candidate declaration
  for cards newly in the breached server's root; (k)(l) discard-pile
  conversion/counter cleanup.
- The buffer is snapshotted at each checkpoint: "had"-conditions read the
  state as of the *previous* checkpoint's step (a) (9.6.6a — Built to Last).
- Checkpoints run: whenever a player would receive priority (10.3.3), after
  every cost payment (10.3.4), after every instruction (10.3.5), and at the
  §9.11.1 mid-instruction points (structure opened, draw step 8.4.5b, play
  step 8.6.7e, trace step 10.8.6b). **No checkpoint nests inside a
  checkpoint** (9.11.2a).
- 10.3.2: if a checkpoint marked instances pending, a **reaction window**
  opens immediately, nested if necessary.
- **10.3.6 (subtle, load-bearing):** the checkpoint after a timing
  structure's last step runs *outside* that structure — the frame pops
  first, then the checkpoint and its reaction window run (the
  AMAZE/Jesminder example). Frames must pop before their closing checkpoint.

## 3. Frames: the control stack

- `StructureFrame` — one per timing structure (turn, phase, run, breach,
  access; 9.2.2). Steps come from `docs/rules/timing-structures.json` (§11)
  as DATA, not control flow. Each step is a single instruction: preceded by
  an interrupt window, followed by a checkpoint (9.11.2). Structures nest
  (turn → run → breach → access). Procedures that are NOT structures
  (installing, traces — 9.2.2e) do not get per-step windows (9.11.2a): their
  checkpoints occur only where explicitly called for.
- `AbilityFrame` — the shared resolution loop of 9.5.7 / 9.6.15 / 9.7.2 /
  9.8.10: per instruction: announce targets → instruction becomes imminent →
  interrupt window → resolve atomically → checkpoint → next. Chain reactions
  are nested frames resolved LIFO (9.1.2a).
- `WindowFrame` — see §4.
- The VM's public face: `step() -> Yield` where `Yield` is
  `Decision(side, DecisionSpec)` | `Progressed` | `GameOver(result)`.
  Decisions include: which pending ability to trigger (or pass), targets,
  modes, nested-cost choices, bids (hidden until both collected, 10.14),
  breach candidate choice (11.5 — the Runner chooses), replacement-effect
  order (9.9.11), minimal-set selection (10.3.1e), subroutine order
  declarations (9.8.2c–d), paid-window actions (trigger/rez/score/pass).

## 4. Priority windows (§9.2)

Five kinds, one engine:
- **Action** (9.2.6): active player only, must act, no pass, closes after one
  action.
- **Paid ability** (9.2.7): both players, active first; pass → other player;
  closes when a player passes right after receiving priority from a pass.
  Option classes per window: (P) paid abilities, (R) rez asset/upgrade,
  (S) score agenda, plus the approach-ice special: rez the approached ice
  (9.2.7e). Any number of uses before passing (9.2.7f).
- **Reaction** (9.2.8): bound to a FIXED set of pending instances captured at
  open; both players, active first; mandatory instances must be triggered
  before their controller may pass (9.2.8e); closing drops remaining
  pendings; if the window's originating structure ends mid-window, the
  window closes immediately, dropping even mandatory pendings (9.2.8f —
  Femme/Tollbooth).
- **Interrupt** (9.2.9, §9.9): opens when an instruction becomes imminent
  and anything could modify it. At open: compute expected effects → apply
  active replacement effects (order chosen per 9.9.11, each applies at most
  once per effect 9.9.9c, and only if its target effect is still expected
  9.9.11a) → mark relevant conditional interrupts pending (a fixed set;
  abilities activated later can NOT join — 9.9.4b/c, No One Home) → priority
  exchange. Paid-ability interrupts may join even if they weren't active at
  open (9.9.4d — the double-Decoy example). Relevance is re-evaluated
  continuously (9.9.3, Sacrificial Construct example).
- **Mid-access** (9.2.10): Runner only, at access step 7.2.2, one ability
  (including the basic trash ability) or pass, once.

Receiving priority always runs a checkpoint first (9.2.4e). Windows nest
LIFO (9.2.4d).

## 5. Imminent instructions, expected effects, values (§9.9)

- `Imminent { instruction, targets, expected: Vec<EffectAtom>, applied_replacements: Set<EffectId>, }`.
- `EffectAtom`s carry **values** (damage amount, tag count, cost amount,
  base trace strength) that are *continuously recomputed* while imminent
  (9.9.7): interrupts add/subtract; values may go below 0 while imminent
  (9.9.7a — Mr. Stone/Cleaners); at resolve time, must-be-positive values
  ≤ 0 drop that part of the effect (9.9.7d); "prevent all X" removes the
  atom entirely so nothing remains to modify (9.9.7b — Chrome Parlor kills
  The Cleaners).
- Ordinal "would" trackers: "the first time you would X" counts
  imminent-becomings, not resolutions (9.9.5a — Tori Hanzō). Per-period
  counters keyed by effect class, bumped when an atom becomes imminent.
- Aggregation (9.12.2b–c): the listed effect classes (credits, clicks, tags,
  bad pub, look/reveal, draw, trash-from-locations, shuffle-back) aggregate
  into ONE atom per instruction ("2 net + 1 per advancement" = one 5-damage
  atom, Prāna applies once); anything else stays per-occurrence (realloc()/
  NASX). This same distinction drives trigger multiplicity in checkpoints.

## 6. Instructions and the card DSL (§9.11, §9.3)

- `Instruction` is the DSL's unit. Card text is transcribed at card-compile
  time into ability lists using the §9.11 segmentation grammar: one sentence
  = one instruction (9.11.3) EXCEPT: restriction/clarification sentences are
  not instructions (9.11.4a); multi-play/install/access sentences split per
  card (9.11.4b — Shipment from MirrorMorph); bare target-choice sentences
  merge into the next (9.11.4c — Tinkering); legacy search/reveal sentences
  split at the shuffle/visibility boundary (9.11.4d–e); nested-cost choices
  end an instruction (9.11.4f); option choices end an instruction and each
  option is its own instruction chain (9.11.4g — Data Raven's tag-or-ETR,
  which is why you can take the tag then Decoy it: 9.12.3d).
- Text classes (9.3): conditions (cost/trigger/static), restrictions (active
  even when inactive per 9.1.8), instructions, declarations (static only),
  and the six ability flags: access, interface, interrupt (↳), persistent,
  threat N, once-per-turn (9.3.6).
- Ability taxonomy (9.3.7): static (declarations, never resolve), paid
  (cost: effect), conditional (condition + instructions; optional iff it
  could have no effect at all — 9.6.9), play, subroutine.

## 7. Conditional abilities (§9.6)

- Instances: created pending at checkpoint step (a); one per occurrence;
  static-condition conditionals repeat-while-true with the no-effect
  throttle (9.6.7d — Parasite vs Architect); pending lost on window close,
  or when the ability goes inactive (9.6.10 — Aesop's/Drug Dealer).
- Independence: paid abilities detach from source at cost payment (9.5.4),
  conditionals when triggered (9.6.12), subroutines when first instruction
  becomes imminent (9.8.8); after detachment the ability cannot act on a
  source that changed zones (9.1.4 — Compile/Mayfly).
- Delayed conditionals are lingering effects; implicit duration = until
  first resolution (9.6.13c — Joshua B.); "when this run ends" without a run
  → never created (9.6.13d — Mayfly outside a run).
- "When scored/installed/encountered" classes and forced-resolution
  semantics (9.6.14d — 24/7 News Cycle re-firing AstroScript).

## 8. Subroutines (§9.8)

Ordered list with provenance categories (9.8.3a–e) determining order;
broken/unbroken per encounter (9.8.4); resolution at encounter step 6.9.3c
is mandatory, no priority window between subs (9.8.7a), encounter-end stops
the loop (9.8.7c — Little Engine); sub-level imminence with its own
prevent-the-subroutine interrupt point before per-instruction interrupts
(9.8.10a–b).

## 9. Lingering effects (§9.10)

`{ id, source, payload, duration }` where payload ∈ {value modifier, granted
ability, replacement effect, maintained choice, delayed conditional}.
Durations only exist on lingering effects (9.10.5 — Gebrselassie interacts
with paid-pump lingering effects, never with static abilities). Duration
referencing a structure not in progress → expires at next checkpoint
(9.10.4). Icebreaker strength pumps default to end-of-encounter (9.10.4a).
Maintained choices get durations from 9.10.3 (Security Testing's chosen
server dies with the turn; Femme's chosen ice lives while she's active).

## 10. What this replaces, and the cutover

The W0 `ir.rs` fire-event pipeline dispatches triggers directly at mutation
sites; the CR requires change-buffer accumulation + checkpoint scanning +
windows (a Decoy must be able to interpose between Snare!'s imminence and
its resolution — direct dispatch cannot express that). `runs.rs`'s hand-rolled
state machine becomes the §11 step tables. `engine.rs` command handling
becomes the Decision surface. **The old engine stays running and deployed
until the VM passes DP-7a (the CR's own ~438 worked examples), DP-7b, and
DP-7c — then the cutover deletes it.** The server/UI protocol, redaction
view, printed DB, deck registry, and session persistence carry over intact.

## 11. Citation discipline (SYS-F-10)

Every primitive carries `cite!("rule_id")` registering into a static table;
a test loads `docs/rules/cr-index.json` and fails on any citation of a
nonexistent rule id, and reports rule-coverage (cited/total) as the DP-7b
number. Wave agents: cite as you implement; an uncited primitive is a
review rejection.

## 12. Encodings, the final-tagless boundary, and the anti-overfit contract

How DESIGN.md's dual-encoding rule ("initial surface, final core") lands in
this crate — and the discipline that keeps CR examples from bending the
kernel around themselves.

**Rules content is initial-encoded, deliberately.** Cards, abilities,
declarations, instructions, and the §11 step tables are DATA
(`PrintedCard`/`AbilityDef`/`StaticDecl`/`Instruction`,
`timing-structures.json`) — never callbacks — because everything that
verifies this campaign consumes them as data: `cite!` coverage, the
odometers, the future §9.11 transcriber, redaction, replay diffing. You can
audit data; you cannot audit a closure.

**The semantics are final-encoded: see [FINAL-TAGLESS.md](FINAL-TAGLESS.md),
which is normative.** W1–W4 built the evaluator concrete-first, on the
argument that trait-abstraction before the second interpreter is ceremony.
That argument was wrong in one specific way, and the measurements show it:
with a single implementation, nothing forces the algebra narrow, so the
cheapest way to express any card is a new `Instruction` variant plus a new
match arm — 125 variants and 254 arms later, three card-shaped defects had
to be caught by a human reading diffs. Narrow algebras are not paid for by
polymorphism-for-its-own-sake; they are paid for by a second honest
interpreter that *could not implement* a card-shaped operation. That is
DESIGN.md SYS-C-3's actual argument, and it applies at 82 examples, not at
cutover. FINAL-TAGLESS.md carries the algebra catalog, the denotation rule
(SYS-D-6), the interpreter roster, and the staged migration FT-0…FT-5.

**The anti-overfit contract** (binding on wave agents, extends §11):

1. Kernel sources never branch on card or example identity. Card names
   appear in `src/` only inside comments, as *class exemplars* naming the
   motivating class of a general mechanism ("Ashigaru class").
2. Vocabulary variants are named for the rule concept and parameterized so
   the entire class is expressible — `AdditionalStealCost(Cost)`, never a
   Strongbox special. A variant that can express exactly one printed card
   is a defect.
3. `testkit` shapes construct cards exclusively through the public
   vocabulary (`PrintedCard` + `AbilityDef` + `Instruction`) — no
   privileged kernel hooks. A simplification inside a shape (a fixed
   installee where the example doesn't exercise targeting) must be
   annotated in the shape's doc comment and is legitimate only while
   orthogonal to every example using that shape.
4. **The re-derivation gate** (DP-7c entry criterion): when the §9.11
   transcriber lands, every testkit shape used by a CR-example test is
   re-derived from the corresponding real card's printed text, and the
   DP-7a suite must pass unchanged. Divergence means harvested overfit —
   either the example test is wrong or the kernel is, and the CR decides
   which.
5. **Tests are plans, not loops.** An example test declares setup (cards,
   hands, credits — data), one *plan per player* (data: ordered
   `when <decision-matcher> → <answer>` rules plus a default policy), and
   assertions on the outcome; a single shared driver folds
   `Vm × CorpPlan × RunnerPlan → FinalState`. The player seam is the
   final-tagless boundary done honestly: the scripted plan and the
   random-walk bot are two real interpreters of the same player algebra
   (the server/human driver joins at cutover). Hand-rolled `while
   vm.step()` loops in tests are the named defect. `tk::inject_*`-style
   state manufacture — effects appearing by test fiat instead of being
   created by a card through the vocabulary — is forbidden; where it exists
   as scaffolding for machinery not yet built, it must be annotated and it
   cannot survive the re-derivation gate (no printed text stands behind an
   injection).
6. **Quantity positions never get bespoke variants.** Everywhere card text
   computes a number (strength X, damage per something, credits per
   something, trace base, "for each" counts) there is ONE `Quantity`
   selector language: a pure data expression evaluated against world state
   returning an int, with its dependencies readable from the expression
   itself — which is what makes reactive re-evaluation through the
   characteristics pipeline (9.12.1) and calculated-quantity timing
   (9.12.2) possible. Selectors are data, not closures, for the same
   auditability reasons as everything else in this section.
   `SelfStrengthPerServerIce { per }`-style variants are the named defect
   class: the variant should be the *position* (`SelfStrength(Quantity)`),
   the selector should be the *content*.
