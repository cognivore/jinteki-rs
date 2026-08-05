# CR-VM campaign state — wave ledger

Working state for the kernel campaign (DESIGN.md P1.5, DP-7a/b/c). Each
handoff updates this file; the successor agent reads it FIRST, then
ARCHITECTURE.md, then the code. Odometers are enforced by tests in
`crates/jinteki-cr/tests/` — this file is the narrative, the tests are the
truth.

## Odometers (after W22)

- **DP-7a: 247/247** — **COMPLETE.** Every worked example in
  `docs/rules/examples.json` is an example-named passing test in
  `crates/jinteki-cr/tests/cr_examples.rs` (100.0%). No blockers, no
  elisions, no example unimplemented. `dp7a_complete` is a ratchet.
- **DP-7b: 888/1420** distinct rules cited (62.5%); traceability test fails
  on any cited id absent from `docs/rules/cr-index.json`
- **Priority decks: 68/68 cards complete**, 0 printed sentences still
  unsayable (`cargo test -p jinteki-cards --test decks -- --nocapture`,
  ratcheted by `the_gap_list_is_measurable_and_honest`). Cards 51–68 are
  CR 1.5.4a's additional-identities pile, which a player brings "along with
  their deck" and which the readiness gate therefore counts with it — it
  GROWS as the identity queue (`docs/vm/IDENTITY-QUEUE.md`) completes
  identities, and an identity joins it only when it is complete, since a
  partial pile card would make both decks unplayable.
  `decks/unlisted.rs` carries 15 further cards no deck lists, outside the
  odometer; **DJ Fenris is complete there since W22d**, and five sentences
  across four other unlisted cards remain (see the gap list).
- **Identity queue: 100/150** (`docs/vm/IDENTITY-QUEUE.md`, the list and the
  ledger — it is the ledger, this line is only the headline).
  `decks/identities/<faction>.rs`, outside the deck odometer until enlisted in
  a pile. Runner Criminal 20/22, Shaper 13/21, Anarch 12/19, Neutral 3/3,
  Sunny 1/1; Corp Haas-Bioroid 15/19, Jinteki 10/21, NBN 12/19, Weyland
  11/19, Neutral 3/4. **Every complete
  Criminal is enlisted** in `cr::ANDROMEDA_PILE`, so `priority_decks()` is 68
  cards; nothing of another faction is, because 1.5.4b's "another identity
  from the same faction" makes the only pile in the priority decks Criminal —
  and a CORP identity can never join it at all, which is why W23c's eight
  changed nothing about the deck odometer.
  Every Runner identity the queue still has open is a recorded blocker, which
  is why W23c is the first Corp batch.
- **DP-7c: 70/3717** reference tests ported and passing
  (`crates/jinteki-cr/tests/corpus.rs`, manifest-ratcheted by
  `dp7c_odometer`). The plan, the measurement and the triage are
  `docs/vm/CORPUS.md`; the divergence ledger is
  `docs/vm/UPSTREAM-DEFECTS.md`.
- Full workspace: 28 suites green; jinteki-core/-server untouched by VM work
- **Commit gate (both, every time):** `nix develop --command cargo test
  --workspace` AND `nix build .#default` (then `rm -f result`). Workspace
  green does NOT imply the artifact builds — `nix/package.nix` filters the
  source tree, so a new compile-time file dependency (`include_str!`,
  `include_bytes!`, `build.rs` path) outside `crates/` can break the release
  build while every test passes. Flag any such new dependency in the commit
  message and to the coordinator; do NOT edit `nix/package.nix` (outside the
  staging paths). Also: `nix build` reads the git tree — `git add` a NEW
  file before building or it is invisible to the sandbox. (W7e hit this
  exactly, with `crates/jinteki-cr/src/deck.rs`.)

## Stage: FT-0 (the algebra cut, `docs/vm/FINAL-TAGLESS.md`)

FINAL-TAGLESS.md (commit `1e51327`) is NORMATIVE for `crates/jinteki-cr` and
ARCHITECTURE §12's concrete-first stance is retracted in its favour. The
staged migration is FT-0 → FT-1 → FT-2 → FT-3 → FT-4 → FT-5, strictly in
order, odometer never regressing.

**W4 IS FT-0, and FT-0 IS DONE** (`W4f`): the plan-driver harness makes the
scripted plan the second honest interpreter of the `Decide` algebra (the bot
is the third; the server/human driver joins at cutover). Exit gate met —
82/243 green, no `vm.step()` loop or `vm.answer` call anywhere in tests, no
`tk::inject_*` state manufacture — and `tests/traceability.rs::
tests_are_plans_not_loops` now enforces all three so they cannot come back.

**PRIORITY RE-ORDER (user directive, supersedes the FT-1…FT-5 sequencing).**
The CR implementation is completed FIRST; the architecture work follows.

1. FT-0 — done (W4b–W4f).
2. **The odometer, wave after wave, to 243/243** — **DONE (W14i).** Every
   CR example is a passing test.
3. **DP-7c (jinteki-reference corpus port, triaged against the CR), then the
   two decks (estrike Andromeda, Gauntlet NTM) from printed oracle text. THIS
   IS NOW THE FRONT OF THE QUEUE.** ARCHITECTURE §12 rule 4's re-derivation
   gate is its entry criterion: every testkit shape used by a CR-example test
   is re-derived from the corresponding real card's printed text, and the
   DP-7a suite must pass unchanged.
4. **FT-1/FT-2/FT-3 (algebra extraction, vocabulary collapse,
   Legality/Viewpoint/Replay interpreters) are DEFERRED until after that.**
   `FINAL-TAGLESS.md` stays normative as the TARGET architecture — do not
   delete or contradict it — but do not spend wave time on the mechanical
   refactors now.

Still binding while implementing examples, because these are cheap in flight
and expensive to retrofit (ARCHITECTURE §12): no card names in kernel
vocabulary; quantity positions take `Quantity` selectors, never bespoke
per-card variants; testkit shapes go through the public vocabulary with any
elision annotated in the shape's doc comment; **new tests are plans, never
step loops** — the enforcement test fails the build otherwise. "In a pure
way" (the user's phrase) means no side-channel state manufacture and no
test-only backdoors into the VM: where an example tempts you into a hack,
prefer a slightly larger honest primitive and note it here.

## Commit ledger

| commit | wave | delivered | DP-7a |
|---|---|---|---|
| `-` | W23e | **eight identities, and the four declarations a permanent fact needed.** Jinteki 7→10, NBN 9→12, Weyland 9→11 (queue 92→100/150). **`StaticCond::StateRequirement(Vec<TriggerRequirement>)`** is 9.3.7a's "**While** <state>, …" written in the SAME words 9.6.5c's `requires` uses on a trigger condition — one vocabulary for "the Runner is tagged", asked continuously instead of at an occurrence. It is what makes Jinteki: Replicating Perfection ONE static ability rather than a declaration plus a suppression: "ignore this ability until the end of the turn whenever the Runner runs on a central server" and "while the Runner has not run on a central server this turn" are the same span read from the two ends, so `TriggerRequirement::RunnerMadeRun` grows `on: Vec<ServerId>` (the server stipulation `RunEnds` and `RunBegins` already carry) and the second printed sentence becomes the first's condition. **`StaticDecl::CannotInitiateRunOn(RunServerSet)`** is the general form of `CannotInitiateRunOnSourceServer` — the sentence names the servers itself — and 6.3.2a keeps it on the ANNOUNCEMENT, so 6.1.2d can still move a run in progress onto a remote. **`StaticDecl::HandRevealed { whose }`** lifts 4.3.2, which is the only reason a hand is hidden at all, for one hand and no other; nothing is revealed card by card, so no reveal condition is met by it. **`StaticDecl::StealsPerTurnAtMost(u32)`** is 1.2.2's absolute "cannot" at 7.2.3's steal step: the agenda is accessed and simply not stolen, and the count comes from the turn's history (10.2.1). **`StaticDecl::TrashCostMod { criteria, amount }`** puts every trash cost through one `Vm::effective_trash_cost`; 7.1.5a is what bounds it — a card with no printed trash cost never gains one, however large the modifier. **`GameChange::RemoteServerCreated`** is 4.6.8d's server coming into existence at install step 8.5.16e, recorded there because that move is what creates it — an install into a remote that already exists creates nothing, which is the whole of Near-Earth Hub. **`GameChange::CreditsGained.source`** is 9.1.4's "through an ability on <a card>" (The Zwicky Group); `None` is 5.2.6b's basic credit action, which came through no card, so `TriggerCond::PlayerGainsCredits` grows `criteria` and a sentence stipulating a source is not met by the basic action. **`GameChange::CardRevealed.by`** is the reveal's DOER — 9.1.1a's controller, not the card's owner — so Hyoubu Institute is paid for showing a card out of the Runner's grip; **`Instruction::RevealRandomFromHand`** is beside `TrashRandomFromHand` for the same reason 10.4.3 needed that one: 1.15.2b puts an announcement to a player and "at random" takes it from both, so the cards cannot be a `TargetSpec`. Also **`Quantity::AgendaPointsOf(Vec<TargetFilter>)`** and **`TargetFilter::IsTriggeringCard`** — a quantity position is READ and never announced, so it describes with filters, and 8.2.5 has already removed the forfeited agenda from the game by the time Jemison reads its printed points. Cards: **Jemison Astronautics, The Zwicky Group, Near-Earth Hub, Haarpsichord Studios, Harishchandra Ent., Hyoubu Institute, Industrial Genomics, Jinteki: Replicating Perfection — all COMPLETE**, 8 behaviour tests. Two blockers recorded: a sentence joined by "and" cannot refer to the card its own other half chose (`Combined` splices a target-choosing half out of the merge, so it resolves AFTER — Blue Sun), and nothing modifies the NUMBER of tags the Runner is considered to have (Acme Consulting). No card joined a pile: the pile is the Runner's (1.5.4b), so `priority_decks()` is 68 cards unchanged | 247 |
| `-` | W23d | **eight identities, and the clause every draft-format identity opens with.** Criminal 19→20, Shaper 12→13, Haas-Bioroid 14→15, Jinteki 5→7, NBN 7→9, Weyland 8→9 (queue 84→92/150). **`TriggerRequirement::LargestFactionGroupIs { faction, criteria }`** is "if you have more **[criminal]** cards installed than any other faction" — a comparison across the FACTION PARTITION (2.13) of a described set, which `BoardHasMatching` cannot say because that one measures a description against a printed number and this sentence prints none. The described set is the ordinary filter vocabulary (§12 rule 5), so "installed" and "rezzed" are the same atom with 8.1.2's stipulation added, and the comparison is strict: a tie is not "more than". It unblocks six identities at once — Boris "Syfr" Kovac, Jamie "Bzzz" Micken, Strategic Innovations, Fringe Applications, Information Dynamics, Synthetic Systems — leaving Wyvern alone on the queue for a reason of its own (nothing maintains the ORDER of the heap). **9.6.5c's `requires` joins `TriggerCond::{TurnEnds, CorpScoresAgenda, RunnerStealsAgenda, CardInstalledBy}`**, beside the eight conditions that already carried it: the leading "if" of each draft identity is part of the CONDITION, so it must be asked when the condition would be met and not while the ability resolves — and on Jamie "Bzzz" Micken it is asked together with the printed ordinal, which is what stops a Runner who was behind at their first install of the turn from banking the ordinal for a later one. **`Instruction::PlayCard` gains an EDSL word** (`play_card`): New Angeles Sol's "play 1 **current** from HQ **or** Archives" is one description with `TargetFilter::AnyOf` across the two zones — where the card is played from is a criterion about the card, not a property of the instruction, which is the only way one sentence names two zones. Cards: **Boris "Syfr" Kovac, Jamie "Bzzz" Micken, Strategic Innovations, Fringe Applications, Information Dynamics, New Angeles Sol, Synthetic Systems, PT Untaian — all COMPLETE**, 8 behaviour tests, each driving both readings of its requirement. Skipped, with reasons recorded on the queue: A Teia, AU Co., AgInfusion, Chronos Protocol (Selective Mind-mapping), Hyoubu Institute, Industrial Genomics, Jinteki: Replicating Perfection, Mti Mwekundu, Saraswati Mnemonics | 247 |
| `-` | W23c | **eight identities — the first CORP batch — and six kernel words the Runner queue never needed.** Haas-Bioroid 8→14, NBN 6→7, Weyland 7→8 (queue 76→84/150); the queue's Runner half is now nothing but recorded blockers, which is why this batch is Corp. **A trigger condition describes the card the occurrence named**, in the same filter vocabulary a target announcement uses (§12 rule 5): `criteria` joins `TriggerCond::{IcePassed, CorpRezzesCard, EncounterEnds}`, which is what lets "a **rezzed** piece of **bioroid** ice" (Architects of Tomorrow), "a piece of **AP** or **destroyer** ice" (Thunderbolt Armaments — a printed *or*, so the conjunctive `of_subtypes` could never have said it) and "an **advanced** piece of ice" (Builder of Nations) be conditions rather than approximations. `CorpRezzesCard` gains 9.6.5c's `requires` beside it for Thunderbolt's "during a run". **`Instruction::RezCard.reduce`** is 1.16.2a's cost reduction on the rez, the same content `InstallCard` already carried on the install — floored at zero by the rule, and the payment still happens, so a Corp who cannot afford the remainder still cannot rez (Architects of Tomorrow). **`StaticDecl::MaxHandSizeIs { whose, to: Quantity }`** is 9.12.1a's effect that SETS a value, applied before every effect that moves it — written as a modifier Cerebral Imaging would have been five plus the pool rather than the pool, and stacking it with Cybernetics Division now gives "credits, minus one" in the order the rule states. **`TargetFilter::SameNameAsTriggeringCard`** is 2.1.4's "another copy of that ice" beside `SameCardTypeAsTriggeringCard`, and **`TargetFilter::InAttackedServer`** is 4.6.6b's whole server — root AND ice — where `IceProtectingAttackedServer` was only half of it, which is what makes LEO Construction's cost unpayable outside a run without a timing restriction being read into the card. **`InstallDest::DeclaredByInstallerInServerOfTriggeringCard`** is Asa Group's "the same server": 1.15.4 fixes the server, 8.5.16b still lets the Corp declare which half of it. Retired one queue blocker (max hand size could be moved but not set); recorded three more that the batch found — nothing happens before the first turn (NEXT Design, Cyber Bureau, Jinteki Biotech, Ayla), nothing is met by trashing a card TO PAY for damage (Chronos Protocol: Haas-Bioroid), and no instruction takes an action (MirrorMorph). No card joined a pile: the pile is the Runner's (1.5.4b), so `priority_decks()` is 68 cards unchanged. | 247/247 |
| `-` | W23b | **six identities, and CR 1.16.4a's inherent costs made movable by a declaration.** Criminal 17→19, Shaper 10→12, Anarch 10→12 (queue 70→76/150); Az McCaffrey and Khan are complete, so they join `cr::ANDROMEDA_PILE` and both eternal decks stay playable at 68 cards. **`StaticDecl::InherentCostMod { which, criteria, amount, first_each_turn }`** is the automatic modification of an install or rez cost — the McCaffreys lower one, Reina Roja raises one, and all three are the same sentence about a different number, so 1.16.4a's cost SITE is content on one declaration (§12 rule 2) rather than a declaration each. It is deliberately not `InstallDiscount`, which is Patchwork's 1.16.6 reduction: that one must be PAID for and chosen, this one applies of its own accord wherever the cost is calculated — including 8.7.2b's affordability query, which is what makes it able to bring an unaffordable card into reach. The printed ordinal rides on it as `first_each_turn` and is read from the change log (10.2.1), and the cost is always calculated BEFORE the occurrence is recorded, so the card being priced is never counted among the earlier ones. `Vm::rez_cost_credits` is now the one place a rez cost comes from, through all four sites that used to read `printed.cost` directly. **`TriggerCond::IcePassed { this_ice, fully_broken, subs_resolved }`** replaces `SelfPassed`, `PassedIceAfterFullyBreaking` and `PassedIceWithResolvedSubroutines`: all three `false` is Khan's plain "you pass a piece of ice", which each of the old three said something more than — the same merge `CardTrashed` got in W23a. **`TriggerCond::CardTrashed.while_accessed`** is 7.1.2's "a card you are accessing" (René "Loup" Arcemont); it is recorded ON `GameChange::CardTrashed` rather than asked of the state later, because 9.6.5c's ordinal is answered of the PAST and the access an earlier trash happened inside is long over by the time the count is taken. Also **`TriggerRequirement::EncounteredIceRezzedDuringApproach`** (6.9.2b, read from the log so an ice rezzed on an EARLIER approach does not qualify) and **`Quantity::RezCostOfEncounteredIce`** — 1.16.4a's inherent cost printed on the card, so Nasir Meidan pays out even for an ice the Corp rezzed for free. Cards: **Az McCaffrey, Khan, Kate "Mac" McCaffrey, Nasir Meidan, Reina Roja, René "Loup" Arcemont — all COMPLETE**, 8 behaviour tests. Skipped with reasons added to the queue: Sebastião Souza Pessoa (5.2.6g pays before it announces the resource, so a target-dependent additional cost has nowhere to be paid), Apex (nothing INSTALLS a card facedown), Omar Keung (`DeclareRunSuccessful` carries only a structural atom, so no "would" condition can be relevant to it) | 247 |
| `-` | W23a | **eight identities across three Runner factions, and three defects the queue's leftovers were hiding.** Criminal 14→17, Shaper 7→10, Anarch 8→10 (queue 62→70/150); the three new Criminals join `cr::ANDROMEDA_PILE`, so both eternal decks stay playable at 66 cards. **A printed "or" between two SERVERS is content on one condition, not two abilities**: `TriggerCond::RunEnds` grows `on: Vec<ServerId>` and `BreachesServer`'s `server` becomes `servers`, because Mercury and Zahya Sadeghi each state 9.3.6g's flag ONCE and a pair of abilities would carry a flag each and fire twice a turn. **`TargetFilter::AnyOf`** is the printed "or" between whole DESCRIPTIONS (MuslihaT's "an **icebreaker** or a **run** event" — a subtype alone, or a type AND a subtype), which `CardTypeIsAny`/`HasAnySubtype` cannot say because they are the "or" between single words of one kind; it participates in `names_zone` only when EVERY alternative names one, and in `stipulates_characteristic` when ANY does. **`TriggerCond::CardTrashed`** replaces `InstalledCardTrashed` with every stipulation a sentence can make about a trash as content (§12 rule 2) — `owner` (1.14.1), `by` (1.14.5), `of_types`, and `installed_only`, whose `false` is Hiram "0mission" Svensson's printed "(from any location)". **`TriggerCond::{RunBegins, SameActionInARow}`**, **`TriggerRequirement::{QuantityAtMost, RunInProgress}`**, **`Quantity::{SubroutinesBrokenThisRun, SubroutinesResolvedThisRun}`** (the 1.12.6 history window `DistinctIcePassedThisRun` already reads), **`TargetFilter::HasCounters`** (the charge keyword's "a card that already has one"), **`MakesSuccessfulRun.requires`** (Ryō "Phoenix" Ōno's "after a subroutine resolved during that run" is 9.6.5c's requirement and must be part of the CONDITION, or the printed ordinal is spent by a run that never met it). **Defect fixed: `Instruction::chooses_targets` did not look through a `Contained::Inline` wrapper**, so "gain 1[credit] and the Corp trashes 1 card from HQ" — a `PerformedBy` inside a `Combined`, which declares no target position of its own because the wrapper only names WHO chooses — was not spliced out of the merge and trashed nothing. The same class as W14b's `MoveToDeck`, one level down. **Defect fixed: 9.6.5c's ordinal re-ran only `trigger_matches` over the earlier changes**, not the scan's state narrowing, so The Collective's first identical action spent the ordinal and the third could never fire; `same_action_run_at` asks the question of the history AS IT STOOD at each earlier point, beside the `during_run` special case that was already there for the same reason. **Defect fixed: a `trash_matching` cost only ever offered INSTALLED cards**, ignoring 1.15.2c's "unless an instruction explicitly specifies the zone" — so "trash 1 card from your grip" (Null: Whistleblower) could not be said with the payer choosing; the same `names_zone` read the target machinery already uses now lifts it. Cards: **Mercury, MuslihaT, Zahya Sadeghi, Captain Padma Isbister, Hiram "0mission" Svensson, The Collective, Null: Whistleblower, Ryō "Phoenix" Ōno — all COMPLETE**, 11 behaviour tests. Skipped, with reasons on the queue: Az McCaffrey, Boris "Syfr" Kovac, Khan, Nero Severn, Steve Cambridge and eight Shapers/Anarchs whose blockers are listed there, plus every double-sided identity — `carddata/cards.json` carries FRONT faces only, so a back face has no oracle text to copy and SYS-D-10 forbids writing one from memory | 247 |
| `-` | W22d | **DJ Fenris, whole — CR 9.1.9 in both directions, and 9.6.13 for the clause that looked unreachable.** **Gaining abilities**: an object's abilities were a presence MASK over `printed.abilities` (`Effective::ability_present`) and `AbilityRef` indexed the same list, so a card could LOSE an ability and never gain one. 9.1.9b says the abilities an object actually has come out of 9.12.1d/e's procedure, so `Effective` now carries a computed `gained_abilities` list beside the loss mask, `CharOp::CopyAbilitiesFrom` fills it by re-entering the pipeline for the copied-from card (the same shape `CopySubtypesFrom` has, with the same cycle guard), and **`Vm::abilities_of`** is the ONE accessor every enumeration site reads — printed abilities keep their printed index so every existing `AbilityRef` still names the same thing (and 9.1.9c's play-ability/subroutine order is the printed order), gained ones are indexed above them. 23 sites in `vm.rs` and `checkpoint.rs` went through it: static declarations, conditional scanning, paid/action/interrupt/access offering, the cost-blocked interrupt scans, hosting, and the `AbilityRef` lookups (`Vm::ability_at`). `char_effects` needed a second pass, since which abilities exist is itself computed by the pipeline it feeds — it runs only when something on the board declares a gain, and cascades one level (annotated). **`StaticDecl::GainAbilitiesOf { criteria }`** is the declaration, the mirror of `RemoveAbilitiesOfMatching` and the house shape of `GainSubtypesOf`; 9.1.9a still wins, because 9.12.1d applies the removal before the copy that depends on it. **Removing the hosted identity**: not a destination override at all — the sentence belongs to the same ability as the hosting, so it is a 9.6.13 DELAYED conditional created when the hosting happens (`TriggerCond::SelfUninstalled` + 9.6.13c's until-it-resolves duration). 9.10.1 keeps the effect alive after DJ Fenris has left, and 1.15.4's `bind_targets` — the Howler mechanism — is what still knows WHICH identity after 1.13.13 severed the hosting; a printed conditional would stay active under 9.1.8g and have nothing left to name, which is what made the clause look unreachable. **DJ Fenris is COMPLETE.** New tests: `crates/jinteki-cr/tests/gained_abilities.rs` (a gained static, a gained paid ability offered in a paid window, a gained conditional pending at a checkpoint, and 9.1.9a beating the gain) and two card-level ones (the memory limit moves by exactly one — the hosted identity's own copy is inactive under 1.13.2a/4.6.5h — and the identity is removed from the game rather than returning to 1.5.4b's pile) | 247 |
| `-` | W22c | **8.6.6d's removal is a nested CONDITIONAL ABILITY, and now resolves where one does — the deviation is retired, not annotated.** The rule says a playing ability that "also contains the nested conditional ability 'After it resolves, remove it from the game'" leaves the card in the play area "until the conditional ability removes it from the game"; the kernel did the removal as the play step's own last act, one step early, so a card able to act on the played operation in the reaction window after 8.6.7h would have found it already gone. Taken literally instead: at 8.6.7g the card is not trashed and a 9.6.13 delayed conditional is created ON THE PLAYED CARD (`TriggerCond::SelfPlayResolved`, `Instruction::RemoveSelfFromGame`, 9.6.13c's implicit until-it-resolves duration), which 8.6.7h's `CardPlayResolved` then meets — so the removal becomes a pending instance at the following checkpoint and resolves in the 10.3.2 reaction window, alongside every other ability whose condition (h) met, in the 9.1.2a order its controller chooses. The source is the played card, which is what makes the condition name the right card and keeps 9.1.4 from stranding the removal. Test: `the_8_6_6d_removal_resolves_in_the_reaction_window_after_the_play` — an Oppo-Research-class rider on the played operation resolves FIRST out of the same window, which it could only do while its source is still in the play area | 247 |
| `-` | W22b | **CR 9.1.6: a mandatory ability is never USED, so nothing expends its 9.3.6g flag.** 9.3.6g says an ability with the once-per-turn flag "can only be used once per turn" and points at 9.1.6 for what using is; 9.1.6's second sentence — "players do not 'use' abilities that are entirely mandatory" — was not implemented, so the kernel spent the flag on RESOLUTION whatever the ability was. Harmless while every card relying on it was unique, and wrong on a non-unique one: 1.12.2's Vaporframe Fabricator example makes the flag per OBJECT, so a mandatory ability written with the flag comes back fresh the moment its card is reinstalled. **The flag is now spent only by a USE**: 9.1.6a moves the paid-ability marking from 9.5.7a's announcement to the moment the trigger cost is paid, 9.1.6b keeps the conditional marking but gates it on the ability being OPTIONAL, and 9.6.9d's optional-component path (the Zahya Sadeghi example, `example_rule_once_per_turn_flag_1`) is unchanged. **The printed sentence for a mandatory ordinal is 9.6.5c's stipulation about the OCCURRENCE**, and it is now ONE mechanism for every condition: `AbilityDef::first_each_turn` + `AbilityDef::first_time_each_turn()`, checked in the checkpoint scan against the turn's change log (10.2.1 open information). `TriggerCond::CardPlayed`'s per-variant `first_each_turn` field is gone into it (§12 rule 2 — one stipulation beside the condition, not one per variant), and the card layer's `when_first_each_turn` sets the stipulation instead of the flag, so Ken Tenma, Bukhgalter, Nebula Talent Management, Azmari EdTech, Wari and RNG Key all stop relying on a rule that does not apply to them. New: `tests/using_abilities.rs`, three plan-driven tests on a NON-UNIQUE object — the flag on a mandatory ability never expends, "the first time each turn" fires once for every copy in play, and a copy installed AFTER the first occurrence gets no fresh first time | 247 |
| `-` | W22a | **the announcement obligation becomes STRUCTURAL — the defect class of `MoveToDeck` (W14b), the counters (W17b), `ModifyStrength` (W17c), `RevealCards` (W20) and `IfMet` (W21) is closed, and a sixth instance was found and fixed.** Five instructions shipped with a `TargetSpec::Choose` position that nothing ever announced, each silently resolving to nothing, because `Vm::targets_needed` carried a HAND-MAINTAINED list of announcing variants. The list is gone. **`Instruction::target_positions`** declares each variant's target positions in announcement order and **`Instruction::contains`** declares the instructions it contains and whose choices those are (`Contained::{Nothing,Inline,Deferred,Branches}`); both are exhaustive matches with NO wildcard arm, so a new variant does not COMPILE until it is declared. `Vm::announcements_owed` / `targets_needed_at` are then DERIVED from those two functions — a declared position is announced with no VM code written anywhere, which is why the sixth instance (**`PlayCard`'s card position**: an ability playing a chosen event from the grip played nothing) needed no new arm. The third layer is `tests/announcements.rs`, which reads the enum's own fields out of the source and fails when a variant is declared without a position it really has (`Instruction::Foo { .. } => Vec::new()` compiles); exemptions are allowed, must state their rule, and a stale one fails too (one entry: 9.10.1's `CreateLingeringEffect` payload DESCRIBES objects, it does not target them). Both gates were verified by negative control. The audit found five more silent holes and fixed them all: **`GrantSubroutines`' `to`**; **`MoveCounters` whose destination was not a choice** never announced its counters at all; **9.11.4a `Combined`** merged a choosing sub-instruction with no targets of its own, so 9.11.3 splices it out to announce for itself; **9.6.9 `DeclineableChoice`** never announced its component's targets after the yes; and an **`IfMet` branch whose FIRST instruction announced nothing swallowed the announcements of the rest**, because slots were counted with a floor of 1 per step instead of by what each owes. **1.15.2 several-position instructions are now read positionally**: `AbilityFrame::target_spans` records how many objects each announcement named and `Vm::position_targets` reads one position back, so a hosting instruction that chooses both its guests and its host stops taking the union of the two for each (`SwapCards`' ad-hoc positional read is now the general one) | 247 |
| `-` | W21 | **the last six cards of the priority decks — 45 -> 51/51, and both decks are PLAYABLE.** Six cards, and not one of them wanted a card-shaped variant; three of the six had `UNIMPLEMENTED:` comments that were simply stale. **9.1.8b's FIRST sentence** — "abilities STATING that they are active in a particular zone are active in that zone" — is the wave's biggest single rule, and nothing had read it: a 9.6.5c requirement naming a zone IS that statement, so `ability::requirement_states_zone` reads it off the requirement list and Subliminal Messaging talks from Archives, Petty Cash's [click] ability works from Archives and nowhere else (9.3.3c's `TimingRestriction::SourceInZone`, checked at every offering site including the ACTION window, which checked no restriction at all before), and Paperclip installs itself out of the heap while the same ability stays silent in the grip. **"The first time each turn you play a copy of X" is NOT 9.3.6g's flag**: the flag is per OBJECT (the CR's own Vaporframe Fabricator examples), so a second copy carries a fresh one, and 9.1.6 never counts a MANDATORY ability as "used" at all — it is a 9.6.5c stipulation about the OCCURRENCE, counted from the change log since the turn began, exactly as `SuccessfulRunOnMark` already counted its own. **5.2.2a defines a FINISHED action** ("it must be completed before the game can advance to the next step or open another action window"), so `GameChange::ActionCompleted` is recorded where the action step reaches its own closing checkpoint — which is also where 5.2.2d puts the reaction window an "action finishing" condition resolves in. **8.6.6d is the flashback rule**: a playing ability that "also contains the nested conditional ability 'After it resolves, remove it from the game'" does not trash the card at 8.6.7g at all, and it has to be content on `PlayCard` because 9.1.4 stops an ability acting on a source that changed zones. **1.10.3a settles Miss Bones**: credits taken from a card ENTER the pool, so the sentence is 1.10.3c's — what a card ALLOWS is content (`CreditUse::{AnyPayment, TrashingCards}`), and the purpose of a payment is read off `PaymentCont`, which already recorded it. **9.10.3 as a RESTRICTION**: `TimingRestriction::EncounterOnly` grows `required_choice`, so Boomerang's "only during encounters with that ice" keys on the object it remembers. **3.9.5g as a REQUIREMENT, not the interface flag**: Paperclip asks after "+X strength" has resolved, and the flag is checked when the ability is offered. **Defect fixed: `Instruction::IfMet` never announced its branch's targets** — the fifth of that family after `MoveToDeck`, counters, `ModifyStrength` and `RevealCards`; `targets_needed_at` splits the announcement slot out of the frame so an instruction that CONTAINS instructions can rebase it. Also: `Quantity::LargestGroupSharingCardType` + `TargetFilter::RevealedThisEncounter` + `TriggerRequirement::QuantityAtLeast` (Slot Machine's two subroutines are ONE selector asked with two numbers), `TriggerRequirement::{ActionsFinishedThisTurn, SourcePlayedFrom, SourceInDiscard, CanInterfaceWithEncounteredIce}`, `RunnerMadeRun` gains its polarity, `TriggerCond::{TurnBegins, EncounterBegins}` gain `requires`, `CardPlayed` gains `criteria` and `first_each_turn`. Cards: **Subliminal Messaging, Petty Cash, Slot Machine, Miss Bones, Boomerang, Paperclip — all COMPLETE**; `cr::readiness()` reports ready and `cr::eternal_setup` returns a game | 243 |
| `-` | W20 | **naming a card / type / subtype / number — CR 1.15.1b; 44 -> 45/51.** 1.15.1b is the rule the whole wave hangs on: "only objects and subroutines are announced as targets. If an instruction directs a player to choose (or 'name') a number, a card type, a subtype, a card name, a server, or one of a specified set of effects, that choice is not made until the instruction resolves." Naming is therefore NOT a 1.15.2 announcement, and `Instruction::MaintainChoice` (9.10.3) — already built for the server and the object — is the mechanism for all six. **The split between them is whether the namespace is ENUMERABLE**, and 9.11.4g decides it: a choice between the servers, between the subtypes a card prints, or between the ten card types 2.15.2 lists is a `ChooseOne` whose branches each maintain a different value (the Pelangi pattern, unchanged), so **`ChoiceSpec::CardType(CardType)`** joins `Server` and `Subtype` and Embezzle's four types and Azmari EdTech's unrestricted "name a card type" become the same shape with a different list. A card NAME and a NUMBER have no branches to write: **`ChoiceSpec::Named { of: NameSpace, excluding }`** → **`DecisionSpec::NameValue`** → **`DecisionAnswer::NamedValue(NamedValue)`**, asked where 1.15.1b puts it, at the instruction's RESOLUTION. **The decision offers no candidate list, deliberately**: the namespace is open, and the only list the kernel could build from its own state is the union of both decks, which §10.2 does not entitle the naming player to see — resolving a player's input to a real printed card is the DRIVER's job (the plan answers `Reply::Name(&'static str)`, the server offers `jinteki_cards::all_cards()`). That is also why a name is `&'static str` and not a `String`: it is what `PrintedCard::name` and `TargetFilter::HasName` already are, it keeps `ChoiceValue` `Copy`, and the kernel never manufactures a title, so nothing is interned and nothing leaks. **Reclamation Order's exclusion needs no card name** — 10.1.5 reads a card's own name, used without the word "copy", as "this object", so it is `NameExclusion::SourceName`. **`TargetFilter::MatchesMaintainedChoice(key)`** is the matching half in ONE atom: "a copy of that card", "all cards with the chosen name", "1 card of the named type", "if it has the named subtype" — WHICH characteristic is compared is content on the maintained VALUE (2.1.4 a name, 2.15.2 a type, 2.16 a subtype through the 9.12.1b pipeline), and `Vm::object_matches_maintained_choice` is the single answer shared by the filter and by the trigger condition. **`TriggerCond::CardPlayed` grows `also_installed` and `matching_choice`**: "plays or installs a copy of that card" is ONE condition, because the sentence is one and its 9.3.6g "first time each turn" must be spent by whichever of 8.6.1's play and 8.5.1's install happens first. **`TriggerRequirement::EarlierTargetMatches`** is 1.15.4 asked as a question ("if the exposed card has the named card type"), **`TargetSpec::EarlierTargets`** is 1.15.4 in the plural ("add THEM to HQ"), and **`Quantity::Count`** widens to a criteria LIST (§12 rule 5) so "every copy of the named card in the heap" is one selector and "all X" is a count equal to how many there are. **CR 2.1.5's "cards with different names"** is deliberately NOT naming: it constrains the SET, so `TargetFilter::DistinctNames` is the one criterion that is not a per-object predicate (`is_set_criterion` keeps it out of candidacy) and rides both places 2.1.5 names — the announcement and the search's find — as `distinct_names` on `DecisionSpec::ChooseTargets`. **Defect fixed: `Instruction::RevealCards` never announced its targets** — missing from `Vm::targets_needed`, the same class as W14b's `MoveToDeck`, W17b's counters and W17c's `ModifyStrength`, so a reveal whose card position was a `Choose` silently revealed nothing; latent because every revealing card so far revealed its own source or what a search found. Cards: **Targeted Marketing COMPLETE** (Gauntlet), and 13 more in `decks/unlisted.rs` — Ark Lockdown, Reclamation Order, Salem's Hospitality, Azmari EdTech, Falsified Credentials, Ibrahim Salem, Wari, Harmony AR Therapy and Asmund Pudlat complete; Whistleblower, RNG Key, Complete Image and Embezzle honestly partial (see the gap list) | 243 |
| `-` | W19 | **the additional identities (CR 1.5.4) — Rebirth complete, DJ Fenris partial; 42/50 → 44/51.** **`Zone::OutsideGame(Side)`** is 1.5.4a's pile: a player's additional identities are ORDINARY objects in an ordinary zone, so `TargetSpec`/`TargetFilter`/`ChooseTargets` reach them unchanged and no parallel value-selection vocabulary is needed. Deliberately NOT `RemovedFromGame` — 4.9.5 is gone for good, 1.5.4a is available — and `card_active` has no arm for it, so a pile identity is inactive (1.8.3d). Its viewpoint entitlement is 1.5.4a's own words: open to the player who brought them, to nobody else. **Faction is an in-game characteristic** (2.13), not deckbuilding metadata: `PrintedCard::faction`, `.faction("Criminal")`, populated for all 51 cards from `carddata/cards.json`, and **`TargetFilter::FactionMatchesIdentityOf { side, same }`** — Rebirth's "same faction" and DJ Fenris's "does not match" are ONE atom with the polarity as content (§12 rule 2), measured against the CURRENT identity and re-read each time. **`TargetFilter::InIdentityPileOf(Side)`** names the zone, which is what lifts 1.15.2c. **`Instruction::SwitchIdentity { side, with }`** is 1.5.4b: the described identity takes the play area (3.1.1), the one it replaces goes back to the pile, and 1.5.4d puts a double-sided arrival front side faceup. Not an 8.8 swap — 3.1.1b, identities are not installed. **1.5.4b as a movement rule**: `trash_card` sends an identity leaving the play area to the pile, not to a discard pile, so 1.13.13's sweep of DJ Fenris's hosted identity lands where the CR says. **Defect fixed: "the player's identity" was any object of card type Identity on that side** — `starting_hand_size` and `FlipIdentity` scanned by type and side, so a second identity anywhere in the play area (hosted on a card, 1.13.12) would have been read as the player's; `Vm::identity_of` reads 3.1.1's LOCATION instead. `decks/unlisted.rs` opens for cards no priority deck lists (Chaos Theory, DJ Fenris), excluded from the deck odometer. DJ Fenris hosts correctly and is honestly partial on its other two sentences — see the gap list | 243 |
| `-` | W18b | **the interrupt that reads the draw — The Class Act; 41 → 42/50.** **BREAKING: `TargetSpec::TopOfDeck(Side, u32)` → `TopOfDeck { side, count: Quantity }`** (§12 rule 6), **`TriggerCond::DiscardPhaseEnds.side` → `Option<Side>`**, **`TriggerCond::WouldDraw` grows `by: Option<Side>`**. **`Quantity::ImminentValueOf(EffectClass)`** is 9.9.6's modifiable value as a selector — "X is equal to the number of cards you would draw plus 1" reads the value of the very instruction the interrupt window was opened over, as it now STANDS (9.9.7a/b, so an earlier interrupt's modification is seen), and 0 outside an imminence, the treatment 1.16.2d gives an X outside its payment. `Vm::imminent_damage_value` is now one call to the same `imminent_value_of`, so prevention and this selector cannot disagree about what a modifiable value is. **Defect fixed: the basic draw action was not a draw instruction at all** — `ActionOption::BasicDraw` called `draw_cards` straight, so 5.2.6c/5.2.7c's draw never became imminent: no interrupt window, and no 8.4.2 ability could act on the commonest draw in the game. It now runs the §8.4 procedure in a rules ability frame, the shape the basic play/install/advance actions already used. **Defect fixed: 9.9.5a's ordinal counted both players together** — "the first time each turn YOU would draw" was spent by the Corp's mandatory draw, so a Runner card fired on the wrong turn and then not on its own; `WouldCounters` is keyed by the atom's side as well as its class (a no-op for damage and tags, which only ever name the Runner). **Whose discard phase is content** (§12 rule 2): 5.5.4's condition takes `None` for a sentence naming no player ("when A discard phase ends" — The Class Act, Breaking News) and `Some(s)` for one that does ("when YOUR discard phase ends" — Citadel Sanctuary); Breaking News had been reading the Corp's, which its "this turn" requirement made harmless and inexact. `.interrupt(…)` on the card builder is MANDATORY now, as "[interrupt] → …" with no "you may" is, with `.may_interrupt(…)` for the other wording | 243 |
| `-` | W18a | **the current class, said whole — Employee Strike; 40 → 41/50.** **BREAKING: `StaticDecl::PlayedNotTrashedUntilAgendaSteal` → `PlayedNotTrashedUntil { until: Vec<TriggerCond> }`.** CR 3.5.1b and 3.7.1b print the same sentence with one word different — a current OPERATION is not trashed until another current is played or the Runner STEALS an agenda, a current EVENT until another current is played or the Corp SCORES one — so the ending occurrences are content on one declaration (§12 rule 2), stated in the vocabulary that already names occurrences. **Gap closed, not just widened: "another current is played" was never implemented at all**, so Targeted Marketing's whole first sentence was riding on the steal half; the shield now expires through the same `trigger_matches` a conditional's condition goes through, with the shielded card as the source. **`TriggerCond::CardPlayed` grows the rest of its stipulations** — `by: Option<Side>` (`None` is a sentence naming no player), `of_subtypes` (2.16, read through the 9.12.1b pipeline, so a list is a conjunction where the type list is a disjunction) and `other_than_source`, the word "another", the same reading `TargetFilter::OtherThanSource` gives "other". **`TargetFilter::ControlledBy(Side)`** is 1.14.2's controller: "the Corp's identity" needs a side-scoped criterion that does NOT require the card to be installed, since an identity never is — and that is exactly why it leaves the Runner's identity alone. Employee Strike's second sentence is then 9.1.9a with nothing left over: `Effective::ability_present` is a mask over `printed.abilities`, so removing all of them removes exactly the PRINTED ones the card names | 243 |
| `93fceec` | W17d | **DP-7c sub-wave 12 — the coordinator's gap requests, and the `Combined` defect.** **Defect: `Instruction::Combined` silently dropped STRUCTURAL sub-instructions.** `Combined` exists because the CR forces it (Snare!'s "3 net damage and 1 tag" is ONE instruction, so one interrupt window sees both) and works by MERGING the sub-instructions' expected atoms — a sub-instruction whose expected effect is structural carries no value to merge and resolved as nothing (Earthrise Hotel removed no counter). Those are 9.11.3's separate instructions and are now spliced in after the merged one (DEVIATION, annotated: a spliced sub-instruction resolves after every merged one, so printed order is not preserved between the two kinds). **`TriggerRequirement::RunnerTagsAtLeast(u32)`** replaces `RunnerTagged` — the threshold is content (§12 rule 2), so BOOM!'s "at least 2 tags" is sayable and `RunnerTagsAtLeast(1)` IS "tagged". **`TriggerCond::DiscardPhaseEnds { side, requires }`** — 5.5.4's condition can now carry a 9.6.5c state requirement (Citadel Sanctuary needs no new predicate, only this field). **`TriggerCond::RunnerAccessesCard { of_types }`** — a card-type stipulation, mirroring `CorpRezzesCard` (Film Critic's "whenever you access an agenda"); `trigger_matches` takes the printed type through a lookup closure. **`TargetFilter::CanBeAdvanced`** — 1.18.3's permission as a criterion, derived from the SAME `Vm::advanceable_cards` the basic advance action reads, so criterion and action cannot disagree | 243 |
| `5e3004f` | W17c | **DP-7c sub-wave 11 — BREAKING: quantity positions on `ModifyStrength.amount` and `LoseCredits`.** Both signature changes in ONE commit so the card layer takes one break: `LoseCredits(Side, u32)` → `LoseCredits(Side, Quantity)` and `ModifyStrength { amount: i32 }` → `{ amount: Quantity }`. `Quantity::CreditsInPoolOf(Side)` (1.10.2 — the credit POOL, which 1.13.3 keeps distinct from credits hosted on cards) is Closed Accounts' "loses all credits in their credit pool"; `Quantity::AnnouncedX` in a strength modification is Corporate Troubleshooter's and Paperclip's "+X strength". `crates/jinteki-cards/src/edsl.rs`'s two-helper compile fix (the cards agent's) is committed in the same atomic break. **Defect fixed: 1.16.2c keyed the X announcement on the wrong thing** — "some costs CONTAIN the variable X", but the kernel asked only when the ability also stated a RESTRICTION, so a cost of plain X silently announced 0. The announcement is now owed by the cost's SHAPE (`Quantity::mentions_announced_x`) and `Vm::x_bound` is the bound: the stated restriction where there is one, and in every case what 1.16.1c leaves. **Defect fixed: `ModifyStrength` never announced its target** (same class as W14b's `MoveToDeck`, W17b's `PlaceCounters`) — 1.15.1/9.11.4c make the ice the target of "choose 1 rezzed piece of ice … that ice gets +X strength". Cards: Closed Accounts, Corporate Troubleshooter, Quandary. Ported: `closed-accounts`, `corporate-troubleshooter` | 243 |
| `b31eac8` | W17b | **DP-7c sub-wave 10 — "installed this turn", and a prohibition on scoring.** **`TargetFilter::InstalledThisTurn(bool)`** — CR 1.12.6, a GAME HISTORY query over the change log since the turn began (10.2.1 makes the history open information), the polarity as content (§12 rule 2) so one atom says both Clot's "an agenda installed this turn" and Seamless Launch's "1 installed card that you did not install this turn". **`StaticDecl::CannotScoreMatching { criteria }`** — CR 1.2.2: "if a rule or ability directs something to happen, but another effect states that it cannot happen, the 'cannot' ability takes precedence", so the declaration removes the (S) option (9.2.7d) rather than competing with it; scoring is not an ability (1.17.3c), so 9.1.9's restriction machinery could never have reached it. The description is re-read whenever a paid window opens, which is why the prohibition lifts by itself next turn and why installing Clot AFTER the agenda still blocks. **Defect fixed: a counter instruction's card position never announced its targets** — `PlaceCounters`, `LoadCounters`, `RemoveCounters`, `TakeHostedCredits` and `AdvanceCard` were missing from `targets_needed`, so a `TargetSpec::Choose` position silently did nothing (W14b's `MoveToDeck` bug again). Cards: Seamless Launch; Clot COMPLETE. Ported: `clot`, `seamless-launch` | 243 |
| `533a748` | W17a | **DP-7c sub-wave 9 — agenda points as a modifiable value (2.5), and adding a card to a score area (10.1.3).** `Effective` grows `agenda_points`; `CharOp::{IncreaseAgendaPoints, DecreaseAgendaPoints}` are 9.12.1a's second and third stages for it; `StaticDecl::SelfAgendaPointsMod(Quantity)` is the one declaration behind Project Beale's "1 more for each hosted agenda counter", Merger's "1 additional … while in the Runner's score area" and Global Food Initiative's negative. `Vm::score` reads the pipeline, so 1.17.2's win condition and the score/steal records report the value the agenda has WHERE IT NOW IS. `StaticCond::SourceInScoreAreaOf(Side)` is the stated condition (9.3.7a). **Defect fixed: CR 4.5.4** — "agendas in the Corp's score area are active; agendas in the Runner's score area are inactive unless stated otherwise" — `card_active` returned true for BOTH; it is now Corp-only, and the "unless stated otherwise" half is 9.1.8b in `ability_active` (`condition_only_met_in_zone` for 10.4.2's damage trash, 1.17.7's steal and 1.17.6's score, plus an ability STATING a zone). Without the second half a Clone-Retirement-class "when you steal this agenda" would never resolve, and the DP-7a suite caught exactly that. **`Instruction::AddToScoreArea { cards, to, as_agenda }`** — 1.17.3e/f: not scored, not stolen, so nothing that watches for either can fire; `as_agenda: Some(n)` is 10.1.3's conversion, carried on `Object::converted_agenda` and cleared by `move_card` the moment the card leaves a score area. `TriggerCond::CorpScoresAgenda` is 1.17.6's twin of `RunnerStealsAgenda`. Also `Quantity::PerEvery(q, n)` ("1 for every N", the complement of `Times` — Beale prints a rate the dividends keyword cannot say). Cards: Merger, Global Food Initiative; Project Beale and Fan Site COMPLETE. Ported: `global-food-initiative`, `merger`, `project-beale`, `fan-site` | 243 |
| `1c0fd24` | W15a | **DP-7c sub-wave 1 — the basic actions the corpus needs, and the card layer.** `ActionOption::{BasicInstall, BasicAdvance, BasicTrashResource, BasicPurge}` (5.2.6d/f/g/h, 5.2.7d) resolve the ordinary procedures in rules ability frames; the install's destination is declared where the CR puts it, step 8.5.16b, through `InstallDest::DeclaredByInstaller` + `DecisionSpec::DeclareInstallDestination` — ONE declaration listing every legal location "including any host relationships" — with `InstallDest::NewRemoteProtecting` for 8.5.2a's other half. **1.18.3** is real: `StaticDecl::CanBeAdvancedSelf` + `Vm::advanceable_cards`, active while installed-but-inactive per 9.1.8f (an unrezzed Ice Wall can be advanced). **10.1.2** is `Instruction::PurgeVirusCounters` + `GameChange::VirusCountersPurged` + `TriggerCond::CorpPurgesVirusCounters`. **`src/cards.rs`** is the card layer: real cards re-derived from printed text (oracle: the reference's `data/cards.edn`), unexpressible clauses marked `UNIMPLEMENTED:` and counted by a test. Deviation 17 retired. Fixed: `BasicPlayOperation` cited 5.2.7d, which is the Runner's INSTALL action | 243 |
| `db77549` | W15b | **DP-7c sub-wave 2 — 5.6.2 ends a phase properly.** Porting `no-scoring-after-terminal` found OUR defect: `Instruction::EndActionPhase` took the player's clicks, so 5.6.2's loop returned to step (a) — a paid window offering (P)(R)(S) — and only then skipped to (d), leaving the Corp a window to score after a TERMINAL operation. Ending the action phase is a jump to step (d); 5.2.2a keeps the action itself intact. Ported: `run-timing-with-{no-ice,an-ice}`, `no-scoring-after-terminal`, `purge-corp`. `docs/vm/UPSTREAM-DEFECTS.md` opens the triage ledger | 243 |
| `4f88dd4` | W16e | **DP-7c sub-wave 8 — three more deck-gate instructions, additive.** **1.21.3** `Instruction::RevealCards` (Mutual Favor, Archangel, Slot Machine, Subliminal Messaging) — revealing is showing a front face and returning the card to its previous state, so 1.21.3a keeps a facedown card facedown and the whole effect lands on what each player has SEEN (10.2.2b). **1.10.3a** `Instruction::TakeHostedCredits` — hosted credits ENTER a pool, so taking them is a gain (Daily Casts, the Crowdfunding class); `MoveSetAsideCounters` is 9.5.5's set-aside move and was never this. **1.9.2** `Instruction::RemoveCounters` — the mandatory-effect counterpart of `Cost::spend_counters` (Earthrise Hotel). Card + port: Daily Casts (a deck card), `daily-casts`, which also proves 10.9.1/10.9.2 end to end — the card is LOADED, so "when it is empty" has a kind to be linked to, and the first draft using `PlaceCounters` correctly failed to trash it | 243 |
| `d3783b7` | W16d | **DP-7c sub-wave 7 — the DECK-GATE capabilities.** Priority re-pointed by the coordinator at what the two priority decks need. **9.1.8c "Play only if <state>"** — six cards: `StaticDecl::PlayOnlyIf(Vec<TriggerRequirement>)`, a static ability that modifies WHEN its source may be played and is therefore active while the card is inactive in hand; `Vm::play_permitted` removes an illegal play from the basic play action (5.2.6e/5.2.7e) and from a multi-play effect's candidates (8.6.3). `TriggerRequirement` becomes the shared state-predicate vocabulary — `RunnerTagged` plus `RunnerMadeRunLastTurn { successful_only }`, read from the change log's last COMPLETED Runner turn (10.2.1: the history is open information) — and `trigger_requirements_met` evaluates through the same function. **5.5.4/5.1.4b "when a discard phase ends"** — three deck cards: `TriggerCond::DiscardPhaseEnds(Side)`, met at the formal end of the turn because 5.1.4b says so in as many words. **Defect fixed: `char_effects` ignored `ability_active`** — it gathered characteristic declarations behind `card_active` alone, so 9.3.6f's `[threat N]` flag and every 9.1.8 exception were ignored for strength and subtype modification (Shibboleth's "Threat 4 → −2 strength" applied at threat 0). Card + port: Neural EMP, `neural-emp` | 243 |
| `b47cec3` | W16c | **DP-7c sub-wave 6 — three cards, six tests, no new kernel.** Extract, Infiltration and Rashida Jaheem are all sayable with the vocabulary already built: 1.16.11a's optional cost with a `trash_matching` payment (and 1.16.1b keeping the Corp from being ASKED when nothing is installed), 9.11.4g's optioned "or", and an optional turn-begins conditional whose cost is trashing its own source. Ported: `extract-{trash-to-gain-9,skip-trash,nothing-to-trash}`, `infiltration-{gain-2,expose}`, `rashida-jaheem-when-there-are-enough-cards-in-r-d` | 243 |
| `63f3c2c` | W16b | **DP-7c sub-wave 5 — "Run any server", and 7.1.5b.** `InitiateRun.server` is now `Option<ServerId>`: `None` is an effect that named no server, and **6.9.1a** ("the Runner announces the attacked server") becomes a real decision — `DecisionSpec::DeclareAttackedServer`, offered over 6.7.4a's allowed set minus the servers 6.3.2a forbids, answered by `plan::Reply::Server`, rewriting the instruction's server position exactly as 8.5.16b's declaration rewrites an install destination. **Defect found and fixed: 7.1.5b** — "the Runner cannot trash or pay the trash cost of a card in the Corp's discard pile, either with the basic trash ability or with other mid-access abilities" was cited in a doc comment and never implemented, so the basic trash ability was re-offered for a card the same access had just trashed, and Imp could trash a card already in Archives. The "other mid-access abilities" half is derived from the instructions (`instr::could_trash_accessed_card`), not from card names. New: **6.5.7a** `TriggerCond::SelfFullyBroken` ("when the Runner fully breaks THIS ice"). Cards: Dirty Laundry, Paper Wall, Hostile Infrastructure. Ported: `dirty-laundry`, `paper-wall`, `hostile-infrastructure-basic-behavior`, `imp-vs-cards-in-archives` (the test W16a deferred — the reference was right and the CR says so in 7.1.5b). **One line of `crates/jinteki-cards/src/edsl.rs` was fixed mechanically** (`server: Some(server)`), forced by the `InitiateRun` signature change; that crate's semantics are untouched | 243 |
| `820b8ad` | W16a | **DP-7c sub-wave 4 — three real defects, found by porting.** (1) **6.2.1**: `occupy_ice_position` gave a POSITION to any card moving into a server's ice zone, so a program hosted on a piece of ice (Botulus) became a position of its own — the Runner approached and "encountered" the program, vacuously fully breaking it, before ever reaching the ice. Only a piece of ice occupies a position, and 6.2.1a takes the position away from ice that becomes hosted. (2) **9.5.6a** ("a paid ability that contains an instruction that could break 1 or more subroutines can only be used during an encounter") was not implemented at all: it was approximated per-card by `TimingRestriction::EncounterOnly`, so a breaker whose text names no ice — Botulus's "break 1 subroutine on host ice" — was offered in every paid window. Now derived from the INSTRUCTIONS (`instr::could_break_subroutines`) and applied at all three paid-ability offer sites. (3) **1.16.2c**: the announced X died with the payment record, so Misdirection's "remove X tags" removed 0 — the announcement belongs to the USE of the ability (`AbilityFrame::announced_x`, `PaymentCont::TriggerCost`), and 1.16.2d stays exact for a cost that is not being paid. New vocabulary: `Instruction::{GainClicks, LoseClicks}` (1.11.3a/b, aggregated per 9.12.2c). 10 new cards (Enigma, Tithe, Pup, Government Takeover, Easy Mark, Diesel, Magnum Opus, Rezeki, Mimic, Cache); 17 tests ported, DP-7c 31 → 48 | 243 |
| `edce92b` | W15c | **DP-7c sub-wave 3 — the icebreaker class.** Corroder needed NO new kernel machinery: 9.3.6c's [interface] strength gate, 9.5.6c's "this barrier" encounter restriction and 3.9.5b's implicit pump duration were all built during the CR wave, so the class gating ~a fifth of the card corpus is expressible today. Ported: `corroder`, `hedge-fund`, `beanstalk-royalties`, `ipo`, `sure-gamble`, `hostile-takeover`, `pad-campaign`, `ice-wall`. (This wave's diff landed inside another agent's commit `edce92b` — shared tree, staged index; the code is intact, only the message is theirs) | 243 |
| `6eb2b59` | W13a | **cost payment as a PROCEDURE (§1.16)**: `Vm::begin_payment` gathers every choice the payer gets — 1.16.2c's X (`Cost::x_restriction` + `Quantity::AnnouncedX`, 0 outside a payment, which IS 1.16.2d; `PrintedCard::cost_x`), 1.16.2e's alternate payments (`StaticDecl::AlternatePaymentForSelf`, with 9.1.8d now REAL so the declaration is active while the source is unrezzed), 1.10.3c's division of the credits among the allowed locations (`DecisionSpec::DivideCreditPayment`), and which cards/agendas are spent (`DecisionSpec::PaymentCards` — cards paying a cost are NOT targets, so it does not collide with a 1.15.2 announcement) — then pays the whole cost at once and resumes `PaymentCont`. 1.16.1c is `PaymentRestriction` + `advancement_requirement_without`; 1.16.10c is `PrintedCard::additional_score_cost` + `Instruction::ScoreSelfAgenda` through an ability frame's PayCost phase. Deviations 11, 18, 44 retired. **Bug fixed:** the (R) rez offer read the credit pool alone, so hosted credits could never pay a rez cost | 221 |
| `ef6904a` | W13b | **§6.7 "If successful" is a property of the initiating effect**: `Instruction::InitiateRun` grows `allowed: RunServerSet` (6.7.4a, a selector — "a remote server" names a computed set) and `if_successful: Vec<Instruction>`, carried on `RunCtx` and pended by `Vm::pend_if_successful` at step 6.9.5a as an ordinary conditional instance. 6.7.4c is `optional` on `Payload::ReplacementEffect`: an optional replacement asks before applying, and replacements apply as the replaced instruction's interrupt window opens — which for the breach step IS 6.9.5b, after the 6.9.5a reaction window's Ash-class trace. New `TriggerCond::SuccessfulRunOnServer` | 223 |
| `d75ac6a` | W13c | **additional costs on the basic run action**: `StaticDecl::AdditionalRunActionCost` (6.3.4/1.16.10) paid with the [click] to INITIATE the run, so 6.3.4 falls out of where `current_run` is already set (step 6.9.1c); `TriggerCond::PlayerSpendsClick { during_run }` reads it. `StaticDecl::MustRunWithFirstClick` is 9.12.3a stated over a DECISION (the action window offers only runs), and 9.12.3e is one line: being OFFERED the additional cost discharges the requirement | 225 |
| `120c8a2` | W13d | **9.9.9c**: `ReplacementTransform::StealWithHostedCounters` (Project Vacheron) — the agenda still enters the score area, with counters, and the replacement cannot apply to its own result; `TriggerCond::WouldStealSelfAgenda`, a `StealAgenda` atom on `Instruction::StealIfAgenda`, and `interrupt_relevant` for an interrupt that CREATES a replacement (9.9.8c). **Three bugs fixed:** `after_window_closed` sent a structure frame from Enter/Exec straight to Checkpoint, so a step whose INTERRUPT window opened never executed its instruction (every interrupt on a timing-structure step silently cancelled that step); nothing applied a replacement created by an interrupt to the instruction that was imminent (9.9.10); and 1.13.13's sweep read a moved card's counters after the whole scan window, banking counters placed as part of the very move that put it there | 226 |
| `348cce0` | W13e | **the basic play action and action identity**: `ActionOption::BasicPlayOperation` (5.2.6e/5.2.7d — half of deviation 17), running the ordinary 8.6.7 procedure in a rules ability frame; `ActionIdentity` (5.2.5a/b as data — a basic action, or the (card, ability) pair, since equivalent abilities on different cards are still different actions) recorded by `GameChange::ActionTaken` with `CoreState::turn_log_start` as the history window; `CoreState::current_action` counts the clicks spent to TAKE an action, so 1.16.4d's additional-cost click still counts against it | 228 |
| `ac96779` | W13f | **1.12.3's third case**: `AbilityFrame::looked_at` stamps each looked-at card with its generation and `TargetFilter::LookedAtByThisAbility` reads them back, so a shuffle that re-makes the object simply stops matching and the ability can no longer act on the card. Deviation 26 closed | 229 |
| `d96c1e6` | W14a | **§10.2 information as a per-side view** (`src/view.rs`): `CardView::{Seen,Unseen}` — one card as one player sees it, with two `Unseen` entries EQUAL, which is what "the Runner cannot tell which" means; the distinction drawn is a card's IDENTITY (its front face) versus its PRESENCE, since an unrezzed installed card is an object both players can point at. `Vm::identity_visible_to` derives the entitlement in the order the CR gives it — a disclosure (10.2.2b), the card being accessed (7.1.2), an object accessed earlier in the breach in progress (7.3.1a), then the zone (4.3.2 own hand only, 4.2.2 decks hidden from BOTH, 4.4.6c facedown Archives is the Corp's, 4.4.7b the heap is open, 1.21.2a your own facedown cards, 4.8.7/8.3.3a a facedown set-aside group belongs to the player carrying the effect out). `Vm::view_of(side)` assembles the redacted state with 10.2.3a's open counts and 10.2.3b's maintained choices; `Sightings` is 10.2.2b's record of what an effect SHOWED a player, expired by 1.21.6 when the card moves. `Object::set_aside_group` is 4.8.7/1.21.1b. 4.4.6b is real. Verbal communication is deliberately outside the kernel — no instruction, decision or record lets a player assert something, so a claim changes neither state nor view, which IS "bluffing is allowed" | 231 |
| `5574154` | W14b | **§8.4 drawing as a PROCEDURE**: `Instruction::Draw` expands into 8.4.5's steps — `DrawStepSetAside` (8.4.5a: the cards are set aside facedown as one 4.8.7 group and are "then considered drawn", so this is the step carrying the draw's expected effect and what a `WouldDraw` interrupt modifies), the ordinary post-instruction checkpoint (8.4.5b), and `DrawStepAddToHand` (8.4.5c: whatever is STILL in the group goes, so 8.4.3a's card that left is not added and 8.4.3b's card swapped in is). `TriggerCond::PlayerDrawsCards` is per EVENT; `TargetFilter::DrawnCards` is 8.4.2a's exception to 4.8.3; `swap_cards` carries the group across the exchange, which IS 8.4.3b/8.8.4d. **Bug fixed:** `MoveToDeck`'s card position never announced its targets (missing from `targets_needed`), so "add 1 of the drawn cards to the bottom of R&D" silently moved nothing | 233 |
| `2218ce2` | W14c | **§8.3 arranging**: `SetAsideTopOfDeck` + `ArrangeSetAside` are 8.3.3's two halves with room between them, which is the whole content of 8.3.3b — `TargetFilter::SetAsideByThisAbility` now names the arranging group too, so a Cultivate-class ability trashes one and adds one to HQ while they are set aside. The order is a DECLARATION, not a target announcement (`DecisionSpec::ArrangeCards` / `plan::Kind::Arrange`), 8.3.1a skips it for ≤1 card, every arranged card becomes a new object, and the arranging player keeps seeing them (8.3.3 + 4.2.3) while 8.3.3a keeps the opponent out. That asymmetry IS 10.2.2b's example | 235 |
| `3e2028f` | W14d | **6.8.2c**: `end_the_run` PROCESSES the open priority windows instead of discarding them — (a) paid windows close, (b) a reaction window bound to a structure closes per 9.2.8f, (c) any other "is completed normally, except that new timing structures … cannot be initiated" (`WindowFrame::no_new_timing_structures`, consulted by `initiate_run` / the Encounter Ice Phase opener / `push_breach`). Also **9.1.8c**: an ability whose effect is rezzing its own source modifies WHEN that card can be rezzed, so it is active while the card is inactive — without which a Formicary-class ability could never pend. `TriggerCond::ServerApproached` | 236 |
| `3ee0fb6` | W14e | **8.2.2 / 9.9.8b**: `StaticDecl::ReplaceTrashDestination { criteria, to }` — a static ability stipulating a replacement of where a trashed card goes (`RemovedFromGame`, Skorpios class; `FacedownInPlay`, Harbinger class, 8.1.4d so the card is not uninstalled). `trash_card` records `CardTrashed` unconditionally, which is 8.2.2's "the modified effect is still an occurrence of that movement". **9.1.8b** is real for the class the CR describes (`TriggerCond::SelfTrashedByDamage` can only be met by the grip→heap move, so the ability is active in the heap and nowhere else); 8.1.4a too. **Two bugs fixed:** damage trashed with a bare `move_card`, so 10.4.2a's trash was not a trash movement at all; and the 9.1.8g hangover was granted to any source that moved to an inactive zone, though the rule begins "if an ACTIVE card moves…" | 238 |
| `429d81f` | W14f | **9.8.9**: `AbilityPhase::SubImminent` is where `StaticDecl::ReplaceSubroutineResolution` swaps the INSTRUCTION LIST while keeping the frame's source — exactly "the replaced subroutine is treated as having the same source as the original imminent subroutine". **6.1.3e/f**: `RunCtx::last_encounter` records the Encounter Ice Phase the run comes directly from (the ice, whether it was fully broken, whether any subroutine resolved), never set by a forced encounter (6.1.3c); `GameChange::IcePassed` carries all three, so `PassedIceAfterFullyBreaking` and `PassedIceWithResolvedSubroutines` are one field read each. The Mirāju case falls out of 6.1.3d + 6.2.8a | 240 |
| `59056cf` | W14g | **10.1.6a**: `Vm::loop_period` detects a repeating suffix of the ability-frame sequence — only MANDATORY loops, which is the rule's own scope (an optional loop has a priority window in it, and a window is a frame of another kind). `DecisionSpec::LoopCount { period }` puts the number to the player resolving it and `Vm::loop_budget` counts the turns down, refusing the push that would begin one too many. Also: `ResolveAbilityOf`'s subroutine class now records `SubroutineResolved` (9.8.10) | 241 |
| `f14776a` | W14h | **1.15.1 counters are targets**: `object::CounterRef { host, kind, index }` gives a counter an address; `DecisionSpec::ChooseCounters` / `AbilityFrame::counter_targets` / `ImminentWrap::counter_targets` make it the THIRD kind of target. `Instruction::MoveCounters { kind, count, up_to, to, from_criteria }` announces the destination first and the counters second, keeping only counters that share the host of the first named — "if 2 tokens are chosen, they must be hosted on the same card" enforced where the choice is made. 1.18.2 stays true | 242 |
| `0a370c4` | W14i | **9.11.2a**: the steps of installing are not instructions — step 8.5.16a is followed by NO checkpoint, so the only one during the procedure is 8.5.16d (the checkpoint after 8.5.16f IS the instruction's own post-instruction checkpoint). That makes 1.13.13 exact where deviation 14 said it was one checkpoint early. **DP-7a 243/243** | 243 |
| `-` | W14j | DP-7b sweep: ~180 rules the kernel genuinely implements but had never cited — §8.2's movement vocabulary, §9.1's ability/source/use model, §9.2's priority-window rules, §9.3's whole text classification, §4.1's zone visibility classes (public/hidden/secret, which `identity_visible_to` reads directly), §4.6's play-area and server rules, §5.2's action model, §6.2's position rules, §8.1's rez/derez, §1.9's counter types, §1.14's ownership and control, §1.16's cost taxonomy, §9.6's conditional-ability model, §7.1.4/7.1.5a-b. Rules realised structurally rather than at one call site are cited at a doc-commented **citation anchor** (`Zone::visibility_class`, `CounterKind::types`, `IcePosition::id`, `instr::movements`, `ability::{ability_model, ability_source_model, text_classification_model, ownership_and_control_model, cost_and_conditional_model}`, `Vm::{ability_use_model, action_model}`, `View::play_area_information`, `PawClasses::occurrences`), each labelled as such so nobody mistakes it for an implementation. DP-7b 643 → 820 | 243 |
| `07e386e` | W1 | kernel: objects/characteristics 9.12.1, change-buffer checkpoints 10.3.1(a–l), five windows 9.2.6–9.2.10, frames + §11 step tables as data, imminence/expected effects §9.9, decision-yielding coroutine | 15 |
| `f50c063` | W2a | cost system §1.16 (tag/damage components, all-at-once aggregation, declinable steal costs, nested Then/Unless payers, unpayability 1.16.1b), paid abilities §9.5 (set-aside 9.5.5, timing 9.5.6) | 27 |
| `713b25d` | W2b | §9.9 complete for example set (statics-shaped expected effects, unpreventable retention, replacement-at-resolution w/ relevance re-eval); persistent 9.12.5 ARMED incl. run-binding 9.12.5d | 34 |
| `e98e8eb` | W2c | traces 10.8.6 (open spends as Decisions), psi 10.14 (sealed bids, spendability-limited legality, reveal-spend atomicity) | 37 |
| `f0012b5` | W2d | subroutine origin categories 9.8.3 a/c/d/e (re-keyed by category/source/ordinal, broken-status preserved), access candidates §7.4, candidate prohibition (Ash class) | 41 |
| `1e74f82` | W2e | once-per-turn use semantics 9.3.6g, delayed-conditional durations 9.6.13, minimal-set 10.3.1e multi-set cases | 47 |
| `dbab206` | W2f | calculated quantities 9.12.2, must-choices 9.12.3 | 52 |
| `2fe0d2c` | W3a | install instructions §8.5: 8.5.16 steps as expanded instructions, one-at-a-time 8.5.5 w/ host choice + Dhegdheer discounts, install-and-rez 8.5.15 w/ declinable additional rez costs 1.16.4c, reveal rules 8.5.13a–d, invalid destinations 8.5.14; 9.6.5b activity gates (Reaper/Nico, ADT/THG); 10.3.1j Runner candidacy DECLARATION (deviation retired) | 63 |
| `0448712` | W3b | play instructions §8.6: 8.6.7 steps, one-at-a-time 8.6.3 w/ per-pick affordability, lingering independence 8.6.4, left-play-area 8.6.6a, not-trashed-until shields 8.6.6c (Targeted Marketing); 9.6.5c condition-time requirements (QPM), 9.6.5d resolution-time requirements (UC link) | 69 |
| `ac5b346` | W3c | replacement ordering 9.9.11: Breach as an effect class, one-at-a-time application with the order Decision, something-to-replace re-evaluation 9.9.11a (SecTest/Siphon/Showing Off) | 71 |
| `ba8c1d3` | W3d | vacuous truth 9.12.2d (all-subs-broken tracking, Forked/Troll), run-ends conditions 6.8.5 (prevent-all shields expiring at 6.9.6d; Chum-in-Run-Ends prevented, DRT after expiry lands) | 74 |
| `38a7bf6` | W3e | candidates: chosen-ever 7.4.3 (access replacement — Immolation; declinable access costs — Gagarin), Archives continuous derivation 7.4.6d, R&D topmost-ELIGIBLE descent 7.4.7a (Bacterial new-objects, Seidr top-inserts, Strongbox click steals) | 78 |
| `7bae1f7` | W3f | mid-window installs: conditional interrupts not pending if activated post-open 9.9.4c (No One Home) vs paid interrupts joining freely 9.9.4d (double Decoy), via TagsAvoided chains; X-values 9.12.2e (Surveyor strength-X in the char pipeline, X-traces initiating at 0 when inactive) | 82 |
| `633fa74` | W4a | the `Quantity` selector language — one data expression for every quantity position (§12 rule 6) | 82 |
| `3f31b61` | W4b | the plan-driver harness (`src/plan.rs`): plans as data, ONE `Script` driver, `Transcript` for post-hoc offer assertions; 11 examples migrated | 82 |
| `1c0099d` | W4c | 16 examples migrated: §6.8 run-ends pair, duration checkpoints, 10.3.6, the whole §1.16 cost cluster | 82 |
| `b90030c` | W4d | playable slice migrated (phase plans + `forbidding_the_rest`); `Instruction::CreateLingeringEffect { LingeringSpec, WantedDuration }` + the real cards that replace `tk::inject_*` | 82 |
| `469c09c` | W4e | 14 examples migrated (§9.9 interrupts, traces, psi, subroutine origins, access prohibition); `GrantSubroutinesToSelf` → `GrantSubroutines { to, count, sub, before, duration }` | 82 |
| `-` | W4f | 22 + 19 examples migrated — the suite is now 100% declarative; every `tk::inject_*` and `grant_external_sub` DELETED, their effects created by real cards; legacy script drivers deleted from testkit; `tests_are_plans_not_loops` enforcement test. **FT-0 exit gate met.** | 82 |
| `e044046` | W5a | §8.7 searching/finding/shuffling: `Instruction::Search { zone, criteria, count, may_fail }` as a §9.11 instruction, found cards set aside facedown (4.8.4) and addressed by `TargetSpec::FoundBySearch`, 8.7.3 shuffle-before-anything, 8.7.5/9.11.4d pend timing; the 8.7.2b legality query (`could_install_found_card` / `could_play_found_card`) incl. Patchwork-class cost reduction; `TargetFilter` extended with card-characteristic atoms; 8.5.13d reveal for an unaffordable rez. Deviation (9) retired. | 87 |
| `9d1d5c3` | W5b | §1.13 hosting: `StaticDecl::{CanHost, HostedInstallDiscount, InstallOnlyHostedOn}`, `Instruction::{HostCards, SwapCards, RemoveCountersFromPlayer}`; the 8.5.16b destination declaration now offers every eligible host (1.13.6a) and refuses the ones that host only through their own abilities (1.13.6b); 1.13.6c install-legality gate; 1.13.12 zone-following + 1.13.2a/b hosted-not-installed; 1.13.13 rebuilt as a change-driven checkpoint rule with the score-area→score-area exception (8.8.4c); 1.13.3 hosted counters (`CounterKind::BadPublicity`); 9.1.6c hosted-credit spending marks both cards used | 99 |
| `7074e36` | W6a | small rules with real machinery: §1.10 credits (1.10.3b lose-as-much-as-possible, 1.10.3c hosted-credit spending incl. psi bids, 1.10.5a/b/d recurring credits placed on becoming active and refilled UP TO the printed number), 1.14.5 `Instruction::PerformedBy` — the player named to carry an effect out makes its choices and is the one attributed (1.14.5a), 1.17.1 `Vm::score` + 1.17.1a `threat_level` with the 9.3.6f threat-flag activity gate, 1.19.4 [trash] costs, 1.20.2 memory limit, 10.4.1 suffer-vs-do attribution, 10.4.3 simultaneous damage trashes | 112 |
| `aec481c` | W6b | strength modifications and durations: `PumpStrengthSelf` generalised to `Instruction::ModifyStrength { target, amount, duration }` (the target and the duration are both positions), 3.9.5b/d implicit encounter duration and its next-checkpoint expiry, 3.9.5c/3.4.4a stated-plus-implicit durations via `LingeringEffect::also` (expires when BOTH have), 9.10.5 duration replacement as `StaticDecl::ExtendStrengthDurations` rewriting an expiring effect's duration at step 10.3.1b, 9.9.9a's "only while the replacement is active", 9.4.4 statics make no lingering effects; `TargetFilter::IceProtectingAttackedServer`, `Vm::has_subtype` | 118 |
| `f86061b` | W6c | §10.12 sabotage as `Instruction::Sabotage { count }`: the Corp's HQ choice at resolution (10.12.2, not a target announcement — 1.15.1b), the 10.12.3a floor and 10.12.3b everything-goes case carried by a new `min` on `DecisionSpec::ChooseTargets`, simultaneous facedown trashing (10.12.2a); 9.4.5 a static value modification keeps the original value's restrictions (The Cleaners' +1 on Flare's unpreventable damage) | 122 |
| `614d533` | W6d | 9.5.6c stipulations on encountered-ice references: `TimingRestriction::EncounterOnly { required_subtype }` — an ability referring to "this code gate" is unusable during any other encounter — alongside the 9.3.6c interface strength gate | 123 |
| `102919a` | W7a | §1.15.2 target announcements: `TargetSpec::Choose { count }` takes a `Quantity` (§12 rule 6 — Aggressive Secretary's X); 1.15.2c restricts every announcement to the play area unless a criterion names a zone (`TargetFilter::names_zone`); 1.15.2b/e as one `Vm::announcement` helper — the count is capped at the distinct valid targets available and is also the floor — with `clamp_announcement` validating/deduplicating/completing the answer | 125 |
| `c809288` | W7b | several announcements per instruction and SUBROUTINE targets: `TargetSpec::Each` (1.15.2 "for each time the instruction requires a player to choose") driven by `AbilityFrame::announce_slot`, `TargetSpec::EarlierTarget` + `ability_targets` for 1.15.4; `DecisionSpec::ChooseSubroutines` / `DecisionAnswer::Subroutines` / `plan::Kind::SubTargets` / `Reply::SubroutineNamed` — subroutines are targets (1.15.1) — and `BreakSubroutines { count }` generalised to `BreakSubroutines { subs: SubroutineSpec }` covering 9.8.6 chosen, 9.8.6a all, 9.8.6b all-but-N; `SubKey` moved to `ability.rs` so the decision vocabulary can name it | 128 |
| `5db3719` | W7c | 1.15.4 targets beyond a move (`bind_targets` substitutes an `EarlierTarget` reference into an ability the SAME ability creates — Howler's delayed conditional), `TargetFilter::TopOfDeckOf` as a zone-naming criterion (1.15.2c lifts for it), `Instruction::AccessCards { cards: TargetSpec }` (§7.2 as an instruction, the access structure pushed per announced card). **Bug fixed:** 9.2.8f window binding was a "look back 12 changes for an EncounterBegan" heuristic that bound a post-encounter reaction window to the encounter that had just ended and dropped its mandatory pendings; it is now the encounter actually in progress | 130 |
| `30992fe` | W7d | subtypes as modifiable characteristics and §9.11 instruction identification: `Instruction::ModifySubtypes { target, add, remove, duration }` + `Payload::SubtypeMod` + `StaticDecl::SubtypeModSelf` feeding the 9.12.1b counting pipeline (2.16.5 — a subtype is present while its adds outnumber its removals); 9.11.4c the choose-and-modify sentence pair as ONE instruction; 9.11.4b split-up instructions, 9.11.4g choice instructions and 9.11.2a's "no checkpoint inside a checkpoint" asserted against existing machinery | 135 |
| `bc1a920` | W7e | small rules riding existing machinery: §1.4 deck construction as pure functions (`src/deck.rs` — 1.4.5a influence counted by copy, 1.4.6d agenda-point requirement), `Cost::lose_clicks` so 5.2.1a's "Lose [click]" ability is used in a paid window and never offered as an action, 5.2.2b action completion, 6.1.4c "end the run" with no run and no encounter, 7.4.1a root cards are candidates for EVERY server (a real gap: Archives ignored its root), and `Object::generation` — CR 1.12.3's "a card that changes zone becomes a new object" as a stamp, which is what lets a trashed upgrade become an Archives candidate again (7.4.5) | 141 |
| `8126ef9` | W7f | §1.12 object identity: `Zone::zone_class` makes the play area ONE zone so 1.12.4 moves within it keep the object while 1.12.3 moves between zones make a new one; once-per-turn use (9.3.6g) is keyed by `(AbilityRef, generation)`, so a reinstalled card's ability is fresh (1.12.2) and a derezzed-then-rezzed one's is not (1.12.5); `Instruction::Derez { target }` | 145 |
| `613433e` | W8a | **§6.2 positions are OBJECTS, not indices** (6.2.6): `object::IcePosition { id, ice }`, `CoreState.ice: BTreeMap<ServerId, Vec<IcePosition>>` and `RunCtx.position: Option<u64>` — a position id. 6.2.2 creation (a outermost / b innermost / c directly inward / f swaps create none), 6.2.4 destruction as a REAL 10.3.1i with both its exceptions (the Runner's position, and an install in progress protecting that server), 6.2.3 "same position" as `TargetFilter::IceInSamePositionAs(PositionRef::{Source,Runner})`, 6.2.7a/c/d/e as `Vm::apply_ice_change_to_run`; `Instruction::{MoveIce, MoveRunnerToIce}` (6.2.2 / 6.2.8a-d); `swap_cards` re-occupies the existing positions (6.2.2f) and no longer no-ops on two ice protecting the SAME server; the HOST position of `HostCards` is now announceable and `announcement_for` passes the source so source-relative criteria work in announcements | 152 |
| `24020b2` | W8b | **§8.8 swapping cards**: 8.8.2 destination legality (`Vm::swap_legal` / `may_occupy` — card type per location, and 4.6.6e/3.6.1 root limits with the vacating card discounted) applied BOTH as a gate on the swap and as a filter on the two 1.15.2 announcements a `SwapCards { Choose, Choose }` requires; 8.8.4b's mixed installed/uninstalled case (the leaver uninstalls and everything hosted on it is trashed, the joiner becomes installed in the exact position with no install procedure, entering unrezzed per 8.8.4a, and `Card{Un,}Installed` are recorded so the trigger conditions meet at the next checkpoint); `TriggerCond::SelfPassed`. Deviation 15 retired | 155 |
| `22caf54` | W8c | §1.16 costs, continued: **`Cost.credits` is a `Quantity`** (§12 rule 6) evaluated when the cost is to be paid and taken as ONE aggregate (1.16.2b), with `TriggerCond::PlayerPaysCredits` to observe it; 1.16.2f's "total N less" as `Instruction::InstallCard.reduce_total` + `DecisionSpec::DivideCostReduction` declared at the top of step 8.5.16d and applied to both costs by 1.16.2a; 1.16.1b extended from tags to a DAMAGE component (`Vm::damage_cost_blocked`) and the 7.2.3 steal-cost offer now gated on `cost_payable`, so an unpayable additional cost is never a choice | 158 |
| `0b9cd6d` | W9a | **encounters as a timing structure** (§6.5): `StructKind::Encounter`, whose table IS the run table's phase-3 span (9.2.2b makes each run phase a structure), opened by the run's step 3a as a child frame parked at 4a; `Instruction::ForceEncounter { ice: TargetSpec }` for 6.5.9a, with 6.5.9c ("not finished until the encounter is complete") and "return to the effect that caused it" free from the frame stack; nested encounters (Shiro→Chrysalis) stash and restore the interrupted one; 6.1.4b unwinds exactly the phase (everything begun inside it, no Run Ends steps) and 6.5.9b unwinds it with the run; 6.5.8a/6.2.7c ABORT the phase (the aborting instruction finishes, then no further step — 9.8.7c) instead of poking the run's cursor. **Bugs fixed:** 6.2.7 was applied to any encountered ice, killing a forced encounter with a card in HQ instantly; `current_subs` never checked 9.1.7 activity, so 9.1.8h was unimplemented-but-passing | 165 |
| `7629564` | W9b | §1.18 advancing vs placing: `Instruction::AdvanceCard` + `GameChange::CardAdvanced`, so 1.18.2's distinction exists at all (`TriggerCond::AdvancesCard` keyed on the advance, 9.6.6a's "had" check moved with it); `PlaceCounters.amount` is a `Quantity` (§12 rule 6) with `Minus`/`RequirementOfSource` joining the selector language; §10.13 **dividends** as a keyword expanded into the conditional ability it denotes (`PrintedCard::with_dividends`, `TriggerCond::SelfScored`); 1.17.8/10.13.2 `Object::scored_snapshot` — the counters and requirement as the agenda began to be scored — plus `Vm::advancement_requirement` and `StaticDecl::ScoreRequirementModInSourceServer` (SanSan class) | 168 |
| `55056b3` | W9c | the attacked server: `Instruction::ChangeAttackedServer` (6.1.2d — changed DIRECTLY, so the timing step does not change and the new server's ice is never approached), `StaticDecl::CannotInitiateRunOnSourceServer` (6.3.2a — removes the basic run action for that server and reaches no further, so a run can still be moved onto it); 9.11.4a/9.3.3f as tests over machinery already right (a use-restriction gates every window the ability is offered in and resolves nothing; an X definition is a static ability with no instructions) | 172 |
| `-` | W12f | **§1.12 object identity, the last two mechanisms**: `Vm::new_objects_for_unknown_location` — a shuffle or a rearrangement moves cards to an unknown location, so each becomes a NEW object (1.12.3) even without changing zones, which is now where `CorpRearrangesRnd`'s breach-bookkeeping reset comes from rather than a bespoke retain; and `Quantity::DistinctIcePassedThisRun` + `CoreState::run_log_start` — a 1.12.6 GAME-HISTORY query, counting distinct `IcePassed` records since the run began, so an ice trashed after being passed still counts though its object has ceased to exist | 217 |
| `-` | W12e | **the damage-selection pair, one mechanism**: `StaticDecl::SelectsDamageTrashes { by, count }` modifies the damage procedure so a player SELECTS the cards trashed (10.4.3a); `Vm::do_damage` split into `do_damage_selecting`, which takes the selected cards first and fills the rest at random, still recording ONE `DamageSuffered` (10.4.3: selected sequentially, trashed simultaneously). The `Instruction::Damage` arm may now `ask` and finish in `answer` (`DecisionCtx::DamageSelection`), the shape `Sabotage` uses. 9.12.1c is `Vm::damage_trash_selector`: declarations from BOTH players mean a choice that can only be made once, so the ACTIVE player makes it — and nothing else about either ability changes, which is why the Chronos-class "look at the grip" is a SEPARATE conditional and still resolves. `Instruction::LookAtCards` now announces its targets | 215 |
| `-` | W12d | **§9.8 subroutine origins, finished**: `GrantSubroutines`'s `count`/`sub` pair became `SubroutineGrant::{Stated { count, sub }, CopiedFrom(TargetSpec)}` — ONE effect can grant SEVERAL subroutines (Loki), which share one `seq` and are ordered among themselves by a new `ord` on `Payload::GrantedSubroutine`, so 9.8.3a's "most recently added first" orders the GRANTS while the copied set keeps the order it had where it came from; the copied-from ice is a 1.15.2 announcement. **9.8.2c order declarations** (deviation 5 retired): `Payload::GrantedSubroutine.placement` is an index applied AFTER the category sort — "regardless of categories" is exactly that — declared through `DecisionSpec::DeclareSubroutineOrder { existing, granted }` / `DecisionAnswer::SubroutineOrder` / `plan::Kind::SubOrder` / `Reply::SubOrder`, with `existing` the ice's subroutines WITHOUT the ones being placed | 213 |
| `-` | W12c | **9.12.2b as a real branch**: `Instruction::ForEach { count, effects }` — the effects TIED to a calculated quantity. `instruction_aggregates` is 9.12.2c's closed list read over the instruction vocabulary and `scale_instruction` is "the values aggregated according to the quantity"; if any effect is off the list NOTHING aggregates and the group is performed once per unit, as separate occurrences a per-occurrence condition (`TriggerCond::PlayerGainsCredits`) meets separately. **9.9.6c costs as modifiable values**: `EffectClass::PayCost` on the install/play cost-payment steps, `Instruction::ReduceImminentCost` modifying it like a damage prevention modifies damage, and `TriggerCond::WouldPayCost`'s relevance test — "relevant to any instruction where a card will be played or installed and the corresponding cost paid" is literally "the imminent instruction carries a PayCost atom". **9.5.5 set-aside cards as targets**: `TargetFilter::SetAsideByThisAbility`, a zone-naming criterion (1.15.2c lifts) that reads the RESOLVING ability's own set-aside list — which is 4.8.3's "no other ability can see them" by construction. Also `Instruction::Derez` now announces its target like every other target position | 211 |
| `-` | W12b | four small rules, four mechanisms: **4.8.3** the set-aside passthrough — `Object::set_aside_from` records where a card was before it was set aside and `Vm::move_card` REPORTS that as the move's origin, so the change log (the kernel's only representation of what other abilities can see) never mentions the set-aside zone; `GameChange::CardInstalled` grew a `from` and `TriggerCond::CardInstalledFrom` reads it (Exile). **9.1.8g** example 2: `TriggerCond::SelfAddedToDeck` — the hangover machinery was already right, what was missing was a condition met by the move that makes the card inactive. **§10.9** loading and emptiness: `Instruction::LoadCounters` + `Object::loaded_kinds` + `TriggerCond::SelfEmpty { kind }`, with the 10.9.2/10.9.3 state test in the checkpoint scan (never loaded → never empty; an unloaded kind coming off meets nothing). **§10.11** the mark: `Instruction::IdentifyMark` (10.11.2/10.11.2a random central, 10.11.3 immutable), `Payload::MarkDesignation { server, since }` as the lingering effect 10.11.4 says it is, `Vm::mark`, and `TriggerCond::SuccessfulRunOnMark { first_each_turn }` whose ordinal is counted from `since` — 10.11.5's "only checks from the moment that server was designated", which is also why a successful run BEFORE the designation cannot meet the condition at all | 208 |
| `-` | W12a | **§7.3/§7.4 breaching**: `BreachCtx.zone_candidate` splits 7.4.1b/c's ONE presented card from the maintained root list, so 7.4.1a root cards are candidates during HQ/R&D breaches too and the 7.3.5 random access limit is spent only by choosing the zone candidate (7.3.5c: on CHOOSING, accessed or not — `Vm::take_candidate` is now the single place a candidate is consumed); `CoreState.run_accesses` + `Quantity::AccessesThisRun` (7.3.6, counted at `CardBecomesAccessed`, which a replaced access never reaches) and `ReplacementTransform::SuppressAccessAndRemoveChosen`; `StaticDecl::RestrictCandidatesToSelf` + `StaticCond::RunnerHasAccessedCardThisRun` — the Flagship-class prohibition is a STATIC, so 9.1.7 activity governs it and 7.4.2a's re-evaluation is free (`restrict_candidates` reads the prohibitions where the candidates are wanted); `Payload::AccessLimitThisRun` + `LingeringSpec::AccessLimit` for 7.4.2b's literal "cannot access more than N cards"; 7.3.8 delayed breaches as the conditional ability the rule says they are (`TriggerCond::BreachEnds`); §10.6 `Instruction::TakeBadPublicity` + `GameChange::BadPublicityTaken` + `TriggerCond::PlayerTakesBadPublicity`, `TriggerCond::SelfStolen` (1.17.7). **Bugs fixed:** `refresh_candidates_after_access` REPLACED the whole candidate list with the random hand/deck pick, so a card in the root of HQ or R&D was never a candidate (7.4.1a — W7e fixed exactly this for Archives) and every access spent a random access, so an upgrade in the root ate one of the Runner's HQ accesses; and 9.6.13c's "until the next time it resolves" was applied at ability-frame COMPLETION, so a delayed conditional whose instruction opens a nested timing structure (a delayed breach, or a delayed run) re-armed itself from inside its own resolution and looped forever — it is now consumed when the ability is triggered (9.6.8a) | 204 |
| `-` | W11f | 4.1.2a revealing from a hidden zone: `TargetFilter::InDiscardOf(Side)` (a zone-naming criterion, so 1.15.2c lifts for it) and `TargetFilter::stipulates_characteristic`, so `AddCardsToHand` reveals a card that is not otherwise visible exactly when the criteria stipulate something about the card itself — the criteria being the kernel's only representation of what the ability stipulated (deviation 21's reading again) | 199 |
| `-` | W11e | §9.7.1 ability taxonomy on one card: `TriggerCond::SelfPlayResolved` (8.6.7h — the condition is met after the played card has already been trashed at 8.6.7g, so 9.1.8g's hangover is what keeps the ability active to resolve) and `Instruction::EndActionPhase` (5.6.2b: the action loop takes an action only while the player has unspent [click]). The Oppo-Research-class shape carries FOUR abilities of three types — a static that is nothing but a restriction, a conditional, and two play abilities that resolve in sequence inside 8.6.7f | 198 |
| `a52c406` | W11d | **9.9.7f preventing as a trigger condition**: `TriggerCond::SourcePreventedDamage` + `GameChange::DamagePrevented`, recorded ONLY when the imminent damage value was above 0 before the interrupt applied (the whole content of the rule — removing an effect already at 0 prevents nothing, so Guru Davinder's second ability never pends); `Instruction::LookAtCards { cards, by }` (1.21.2) + `GameChange::CardLookedAt`, which makes 9.11.4e's split expressible. **Bugs fixed:** `interrupt_relevant` demanded a damage value ABOVE 0 for a *prevent all* interrupt, so a value already reduced to 0 could never be removed — which is precisely what the 9.9.7f example does, and it is observable (The Cleaners has nothing left to modify); and `DeclineableChoice` applied its inner instruction directly, bypassing imminence, so an install/play/trace inside a "you may" did NOTHING silently — those kinds are now spliced in as the next instruction, the same shape 9.11.4f uses | 197 |
| `332bcee` | W11c | `Instruction::ExposeCards` (1.21.4 — revealing, restricted to installed unrezzed cards; 1.21.5 keeps it distinct) + `TriggerCond::CardExposed`: exposing is not one of 9.12.2c's aggregated classes, so two cards exposed by ONE instruction meet a Blackguard-class condition twice (9.6.4b); `TriggerCond::InstalledCardTrashed { side, of_types }` (District 99 / Wasteland), where 8.2.2a falls straight out of the change buffer — a prevented trash records nothing; 6.8.2a as a test over W9a's frame unwinding (the paid window open when the run ends closes, and the Runner never gets another offer to spend the run's bad publicity fund) | 195 |
| `6a7bcfd` | W11b | **§9.1**: `Vm::source_moved_since` is real — the ability's source is an OBJECT, an `(id, generation)` pair (1.12.3), so `AbilityFrame`/`AbilityInstance` carry `source_generation` and a conditional's frame INHERITS the generation its instance came into being with (that is what strands Mayfly: its frame is pushed after Compile moved the program, but the instance remembers the object that met the condition); `StaticCond::SourceAbilityResolving` (9.1.2b) + `Vm::static_cond_holds`, with `active_statics` now honouring a static ability's stated condition (9.3.7a) — an interrupt window is nested ABOVE the ability frame, so it is inside the scope; `cost_payable` now honours `CannotSpendCredits`; `StaticDecl::RemoveAbilitiesOfMatching { criteria }` (9.1.9a for a described set — Direct Access class); `MoveToTopOfRnd` generalised to `MoveToDeck { card, top }`. **Bug fixed:** checkpoint step 10.3.1a never checked 9.1.9 ability PRESENCE, so a card whose abilities had been removed still pended its conditionals | 192 |
| `6bc5513` | W11a | **§9.6.14d resolving an ability by class**: `Instruction::ResolveAbilityOf { source: TargetSpec, which: AbilityClass }` — the card is the position and the class is the content (§12 rule 2), so 24/7 News Cycle's "resolve the 'when scored' ability of an agenda in your score area" and Nanisivik Grid's "resolve its first subroutine" are ONE instruction. The three conditional classes mark the ability PENDING as though the stipulation had occurred (drained into the next checkpoint's newly-pending set via `Vm::pending_from_effect`, so 10.3.2 opens the window); the subroutine class resolves where it is named (9.8.10). 9.6.5c's additional requirements became DATA on the condition (`TriggerRequirement`, `SelfScored { requires }` / `SelfAccessed { requires }`), retiring the bespoke `SelfAccessedIfRunnerTagged`; `Instruction::RezCard { target, ignore_costs }` (8.1.2b/d), `TriggerCond::SelfInstalled`, `Cost::forfeit_agenda` (8.2.5), `PrintedCard::additional_play_cost` (1.16.10b). Deviation 40 retired | 189 |
| `53c8243` | W10d | **§9.10.3 maintained choices, all three durations**: `Instruction::MaintainChoice { key, of: ChoiceSpec::{Server, Object, Subtype}, duration }` finally creates the `Payload::MaintainedChoice` that has existed since W1 with nothing to create it, and `TargetSpec::MaintainedChoice(key)` + `Vm::maintained_choice` read it back. A choice BETWEEN servers or subtypes is 9.11.4g's option choice (`ChooseOne` with one branch per value), so no new Decision was needed — only the object case announces (1.15.2). 9.10.3a rides `ThisEncounter`, 9.10.3b `ThisTurn` with `TriggerCond::SuccessfulRunOnChosenServer { key }` comparing the choice in the checkpoint scan, 9.10.3c the already-implemented `WhileSourceActive`. `object_move_known_location_1` (1.12.4) falls out: a Thimblerig-class move inside the play area does not bump `generation`, so the remembered object is still the same object | 187 |
| `070e034` | W10c | **§9.12.3 "must" as a requirement at the mid-access window**: `Instruction::MustTrashAccessedCard { means: TrashMeans::{AnyAbility, PayingTheTrashCost} }` records a requirement on the access in progress (`AccessCtx::must_trash`), and `DecisionSpec::MidAccessWindow` grows `can_pass` — false exactly while a permitted means of trashing the accessed card is among the options. 9.12.3a's "any decisions necessary, even other card abilities" is `AnyAbility` plus a shallow scan for `TrashCards(AccessedCard)`; 9.12.3b's stipulated means is `PayingTheTrashCost`, which no other ability can satisfy. Supporting vocabulary: `Cost::spend_counters` (1.9.2, Imp class), `Payload::CannotUseAbilitiesOf` + `LingeringSpec::CannotUseAbilitiesOf` (9.5.3a, Wendigo class — a prohibition on USE, which a "must" cannot reach past because paid abilities are always optional), `TriggerCond::RunnerAccessesCard`. **Bug fixed:** the basic trash ability's affordability read only the credit pool and the bad-publicity fund, so 1.10.3c hosted credits (Scrubber class) never paid a trash cost, though `cost_payable` counted them everywhere else | 183 |
| `2fad713` | W10b | **§4.6.6i "this server"**: `Object::last_server` stamped whenever a card leaves a server/root/position, and `Vm::this_server` as the ONE resolution of the phrase — host's server (4.6.6k), else the current server, else the server the card LEFT, else the central server corresponding to the zone the card is in. Every "this server" reader routes through it: `TargetFilter::IceProtectingSourceServer` (both the filter and the count), the checkpoint's trigger-condition server, `CardInstalledInSourceServer`, and the new 4.6.6i scope on `RunnerTrashesAtLeastOneCorpCard { in_this_server }`. **Bug fixed:** `IceProtectingSourceServer` matched only a source in `Zone::Ice`, so an upgrade or asset in a server's ROOT — the usual source of "ice protecting this server" text — counted ZERO ice. Also §4.6.8f `StaticDecl::RemoteServerLimit` + `Vm::{can_create_new_remote, remote_servers}` (a limit makes "a new remote server" an unidentifiable destination, 8.5.14), and §10.1.5 `TargetFilter::HasName` — naming a card is not self-reference | 180 |
| `7c4d480` | W10a | **§9.12.1d/e dependency, both classes**: `StaticDecl::RemoveHostAbilities` generalised to `RemoveAbilitiesOf(HostRelation::{Host,Hosted})`, so the Hush direction and the Magnet direction are ONE declaration and their mutual dependency is the 9.12.1e loop the hosted-beats-host tiebreak exists for; `StaticDecl::GainSubtypesOf { criteria }` + `CharOp::CopySubtypesFrom` — the copied subtypes are the source object's EFFECTIVE ones, so `compute_effective` re-enters itself (cycle-guarded by a `visiting` set) and 9.12.1d's dependency ordering is realised by construction; `StaticDecl::GrantSubroutinesTo { criteria, sub, before }` for subroutines granted by a static ability that is NOT on the ice (9.8.3a/e external categories, ordered by the source's `active_since` — and `Payload::GrantedSubroutine`'s `seq` moved onto the same clock so the two kinds of external grant sort against each other); `TargetFilter::OtherThanSource` (the word "other" in a description) | 174 |

## Open deviations (documented in code; retire deliberately)

1. **Mid-checkpoint minimal-set resume** — 10.3.1e Decisions suspend and
   complete the trash on answer, single-pass; exact for single-set cases and
   the tested multi-set examples. The 10.3.1j candidacy Decision (W3a) uses
   the same single-pass pattern: steps (k)/(l) run before the answer, which
   is unobservable (they only clean counters). Revisit if an example demands
   true mid-procedure resume.
2. **9.12.1d/e dependency analysis** — the EXPLICIT dependency graph
   (`compute_effective`'s `dep_of`) still covers only the ability-removal
   class, which is what 9.12.1e's loop tiebreak needs. Every other dependency
   is realised implicitly, by the pipeline re-entering itself: a
   `CopySubtypesFrom` reads the copied-from object's effective subtypes
   (W10a), and a criteria-scoped effect like a subtype-gated subroutine grant
   asks `has_subtype`, which runs the whole pipeline for that object. That is
   correct for a dependency FOREST; a dependency LOOP not made of
   ability-removals would recurse, and the `visiting` guard falls back to
   printed characteristics rather than applying 9.12.1e's hosting tiebreak.
   No example.
2b. **Criteria are read shallowly while gathering the pipeline's input**
   (W10a, `Vm::filter_matches_shallow`) — `StaticDecl::GainSubtypesOf`'s
   criteria are evaluated inside `char_effects`, where asking for an
   EFFECTIVE subtype would re-enter the gather forever, so `HasSubtype` there
   reads printed subtypes (2.16). Every other atom is the real one. A "gains
   the subtypes of each other ice that HAS a granted subtype" card would
   notice; nothing in the corpus does.
3. **The Cleaners dual modeling** — CR 9.9.7a example flow (triggered
   interrupt) AND `StaticDecl::DamageBonus`; both tested.
4. **Procedure-step surplus checkpoints, NARROWED (W14i)** — installing is
   now exact: 9.11.2a is implemented, step 8.5.16a is followed by no
   checkpoint, and the only one during the procedure is 8.5.16d (the one
   after 8.5.16f is the instruction's own post-instruction checkpoint, which
   9.11.2 requires). Traces (10.8.6), plays (8.6.7) and the install-and-rez
   tail still expand into per-step instructions and so still get a checkpoint
   per step where the strict reading has only the explicitly called-for ones
   (10.8.6b, 8.6.7b/e, 8.1.2e). Defended in-code as 10.3.4/10.3.5
   checkpoints; the same one-line suppression in `AbilityPhase::Checkpoint`
   extends to them when an example distinguishes them.
5. **RETIRED (W12d)** — "9.8.2c order-declaration not implemented".
   `Payload::GrantedSubroutine.placement` + `DecisionSpec::
   DeclareSubroutineOrder` implement the declaration for a grant of any size,
   applied after the category sort. What is still not modelled is the
   simultaneous resolution of two DIFFERENT abilities each declaring an
   order; each declaration is made against the list as it stands when that
   ability resolves, which is what 9.8.2c says ("each subroutine the ice has
   at that time").
6. **6.8.5 example 1, Georgia clause** — the CR says run-end conditions are
   met at 6.9.6d AND (10.3.6, AMAZE example) that "run on this server ends"
   abilities pend after the run frame pops, yet the 6.8.5 example asserts
   Georgia Emelyov meets its condition "at step 6.9.6c" with damage
   prevented. Internal CR tension; the VM keeps the 10.3.6 reading (W1 test
   `example_rule_checkpoint_after_timing_structure_1` depends on it). The
   6.8.5 example-1 test verifies the Chum/Noble-Path clause only.
7. **Kernel replacement lingering effects are one-shot** — applying a
   `Payload::ReplacementEffect` consumes it (right for Security Testing,
   Account Siphon, Showing Off, Immolation Script). Multi-application
   durations (9.9.9c across several effects) arrive with the card layer.
8. **8.5.6 may-trash** — only the MUST component of trash-like-cards is
   implemented (asset/agenda and region conflicts); the optional "may first
   trash any number" needs a Decision no current example demands. Multi-
   install affordability gates on printed cost (no discount anticipation).
9. **8.7.2b legality query scope** (W5a, annotated on
   `Vm::could_install_found_card`) — the query tests exactly the two things
   the CR names: installable card type (8.5.1/8.5.3) and payability of the
   install cost (8.5.11) net of cost-reducing abilities. It does NOT
   re-derive destination legality (8.5.14 invalid destinations, 8.5.2 server
   limits, 8.5.6c memory-limit trashing); no example turns on those. The
   play branch (8.7.2b's second sentence) is implemented but untested — no
   CR example exercises a search followed by a play.
10. **Patchwork-class cost reduction is applied, not offered** (W5a,
   annotated on `Vm::install_payment`) — `StaticDecl::InstallDiscount { cost,
   amount }` reductions are used only when the player could not otherwise
   pay, largest first, and the choice of using an affordable-anyway
   reduction is never put to them. This is the 8.7.2b example's own reading
   ("they *must* use Patchwork"), but a real Patchwork is optional and
   once-per-turn.
11. **`Cost::trash_from_hand` picks no cards** (W5a, annotated on the field)
   — "trash N cards from your grip" as a cost takes the front of the hand
   instead of asking the payer, because `pay_cost` is synchronous
   everywhere. A Decision here means suspending cost payment; revisit when
   an example distinguishes which card is trashed.

18. **RETIRED (W13a)** — "the division of a credit payment is not put to the
    player". `DecisionSpec::DivideCreditPayment` lists the credit pool and
    each card whose hosted credits are spendable, and the answer is one
    number per location. The bad-publicity fund still spends first during a
    run and is not one of the offered locations.

19. **The 10.12.3a sabotage floor is completed, not refused** (W6c,
    `DecisionCtx::Sabotage`) — the Decision carries `min`, and a Corp answer
    below it is topped up from the front of HQ instead of being rejected as
    illegal. The kernel has no "your answer was illegal, choose again" path
    anywhere; every other Decision clamps the same way.
20. **RETIRED (W14a)** — "the kernel has no per-side visibility model".
    `src/view.rs` is one: `Vm::identity_visible_to` / `Vm::view_of(side)` /
    `CardView`. 10.12.2b still holds trivially for `Instruction::Sabotage`
    (the R&D cards are never shown to the answering side, and the Corp's view
    of R&D is `Unseen` by 4.2.2 anyway), so nothing about sabotage changed —
    what changed is that the claim is now checkable.

12. **Plan-driver approximations** (W4, all annotated in `src/plan.rs`):
    `Reply::Pass` in a window where 9.2.8e forbids passing discharges the
    mandatory obligation first (that is what "nothing of my own volition"
    means there, and it is what the hand-rolled loops did). `Ordinal` counts
    PER RULE and only advances on decisions the driver actually evaluates
    that rule against, since the first applicable rule answers — so "the
    second action window" is `nth(2)` only when no earlier rule consumed
    one. `Pick::Labeled` is a substring match on the option label, so needles
    must be distinctive (`"advance target"`, not `"advance"`).

13. **The 1.13.6a host choice is declared at announce time** (W5b) — every
    `InstallCard` whose destination is not already a fixed host asks the
    installer to pick an eligible host (or decline) when the instruction
    announces its targets, not during step 8.5.16b. Nothing intervenes
    between the two points in any tested example. Consequence: ONE
    instruction gets ONE announce Decision, so `InstallCard { card:
    Choose{…} }` spends it on the card and never offers a host;
    `InstallCards` (8.5.5, one at a time) rewrites itself per card and so
    gets both, which is how the §1.13 tests install.
14. **RETIRED (W14i)** — "1.13.13 counter-trashing is one checkpoint early".
    9.11.2a is implemented: step 8.5.16a is followed by no checkpoint, so the
    counter goes at the only checkpoint the install procedure has, 8.5.16d,
    which is what `example_rule_trash_hosted_objects_when_host_trashed_2`
    says. `example_rule_step_sequences_1` asserts the ordering both ways.
15. **RETIRED (W8b)** — "§8.8 swaps are a slice" (W5b). W8a added 6.2.2f
    position preservation, W8b added 8.8.2 destination legality (as a gate
    AND as an announcement filter) and the 8.8.4b mixed installed/uninstalled
    case. What §8.8 still does not have is 8.8.4d's set-aside swap, which
    belongs with `drawn_card_swapped_1` and the §8.4 drawing procedure.
16. **Two narrow scans** (W5b): 1.13.6b's "has an ability that hosts onto
    itself" is a shallow scan of printed instruction lists for
    `HostCards { host: SelfSource }` (it does not look inside `Combined` /
    `DeclineableChoice` wrappers), and `RemoveCountersFromPlayer` is wired
    for bad publicity only — tags have their own removal path and the other
    counter kinds only ever exist hosted on cards.
17. **RETIRED (W15a)** — every basic action the CR names is now an
    `ActionOption`: credit, draw, run, remove-tag, play (W13e), install
    (5.2.6d/5.2.7d), advance (5.2.6f), trash-resource (5.2.6g) and purge
    (5.2.6h). Original text: the basic PLAY
    action (5.2.6e/5.2.7d) is
    `ActionOption::BasicPlayOperation`. What is still missing is the basic
    INSTALL action (5.2.6d/5.2.7e) and the basic advance action (5.2.6f).
    Original text: the action
    window still offers only credit/draw/run/remove-tag plus card actions,
    so "the Runner installs a connection card" and "the Corp plays
    Scapegoat" are driven by card abilities that install/play
    (`tk::runner_install_button`, `tk::play_event_action`, …). The 1.13.6a/b
    examples' phrase "through an install action" is therefore tested against
    an install EFFECT; 1.13.6c's legality gate lives in the same place either
    way (`Vm::install_destination_available`).

21. **1.15.2c is a criteria-level test** (W7a, `Vm::filter_candidates_from`)
    — the play-area restriction lifts when ANY criterion of the announcement
    names a zone (`TargetFilter::names_zone`). The CR phrases it as a
    property of the instruction; the criteria are the kernel's only
    representation of what the instruction specifies, so the two coincide
    for every shape in the vocabulary. A future instruction that names a
    zone in prose without a criterion would need a flag.
22. **`TargetSpec::Each` announcements are per-element, not per-"time"**
    (W7b) — 1.15.2's "for each time the instruction requires a player to
    choose 1 or more objects" is one Decision per element of the `Each`
    list, and `announcements_required` returns 1 for every other
    instruction. An instruction needing two announcements of a shape other
    than `TrashCards(Each(..))` needs its arm added there.
23. **Subroutine announcements have no `min`** (W7b,
    `DecisionSpec::ChooseSubroutines`) — 1.15.2e's "as many distinct as
    possible" floor is applied to object announcements
    (`Vm::announcement`) but subroutine announcements only carry `count`
    and `up_to`, since the two examples are an "up to N" break and a
    "break all but N". A mandatory "break 2 subroutines" would want the
    floor too.
24. **`bind_targets` covers the instructions that carry a target** (W7c) —
    1.15.4's cross-ability binding rewrites `EarlierTarget` inside
    `TrashCards`, `PlaceCounters`, `ModifyStrength`, `Combined` and
    `PerformedBy`. Other instructions pass through unchanged; add arms as
    shapes need them. Only `CreateDelayedConditional` calls it — a
    `GrantSubroutines` whose granted subroutine refers to an earlier target
    would need the same call.
25. **`Instruction::AccessCards` skips additional access costs** (W7c) —
    §7.2 accessing as an instruction pushes the access structure directly;
    the 7.4.3/1.16.10 additional-access-cost Decision lives only on the
    candidate path (`StepKind::AccessChosenCandidate`). No example
    exercises a Top-Hat-class access with a Gagarin-class cost.
26. **RETIRED (W13f)** — the last open half ("the looked-at set has to live
    on the ability frame") is `AbilityFrame::looked_at` +
    `TargetFilter::LookedAtByThisAbility`. Original text, for the record:
    **`Object::generation` bumps on zone CLASS *and* on unknown-location
    moves** (W7e/W7f, extended W11b, W12f) — 1.12.3's new object is a
    `(ObjectId, generation)` pair, with the whole play area one class so
    1.12.4 moves keep the object; W11b made 9.1.4 read it; W12f added
    `new_objects_for_unknown_location` for shuffles and rearrangements, and
    `Quantity::DistinctIcePassedThisRun` for 1.12.6's history queries (a
    linear scan of the change log from `run_log_start`, not an index). What
    is still NOT built: 1.12.3's third case, cards being LOOKED AT that a
    shuffle moves out from under the ability looking at them
    (`object_move_location_1` — it wants `Instruction::LookAtCards` to keep
    the looked-at set on the ability frame, the way `found_cards` does, and
    to drop entries whose object has been re-made).
27. **6.2.7 is applied at the checkpoint, not continuously** (W8a,
    `Vm::apply_ice_change_to_run`) — the CR states 6.2.7a/c/d as immediate
    consequences of a change to the ice in the Runner's position. The kernel
    notices them where it notices every state change: at the top of the
    checkpoint procedure, before step (a) (so an encounter ending there is in
    the scan window) and before step (i) (so a position the Runner has just
    left is vacant). Nothing can observe the difference without a decision
    between the change and the checkpoint, and there is none. 6.2.7b's "the
    trigger conditions of being approached are not met for the new ice" is
    free — nothing re-records `IceApproached` — and 6.2.7e needs no code at
    all, which is the point of positions-as-elements.
28. **`MoveIce` accepts the `InstallDest` vocabulary** (W8a) — 6.2.2's
    position language is the same one 8.5.16b uses, so `MoveIce` reuses it and
    implements the two arms that name a position protecting a server
    (`Protecting` = 6.2.2a outermost, `InwardFromSource` = the innermost/
    inward case). Any other destination moves nothing. 6.2.2d's "in any
    position" (a Corp choice among the gaps) and 6.2.2e's Mutate case are not
    expressible yet.
29. **8.8.4a is applied where a card becomes (un)installed or joins a
    facedown group** (W8b, extended W14b) — the 8.8.4b case (a Corp card
    entering the play area enters unrezzed) and 8.4.3b/8.8.4d (a card swapped
    into the set-aside zone joins the group that the leaver was in) are both
    implemented. 4.4.6b's "facedown into Archives unless it was visible" is
    real since W14a. A swap between two hidden zones still does not re-derive
    faceup status; no example does one.
30. **Two swap announcements, and only two** (W8b, `announcements_required`) —
    a `SwapCards { Choose, Choose }` asks once per target position, and the
    FIRST announcement is filtered to cards that have at least one legal
    partner (8.8.2's "if a swap effect would resolve while there are no legal
    exchanges possible, then that effect does nothing", read forward). Neither
    announcement can express "another piece of ice" — nothing in the filter
    vocabulary excludes the source — so a Thimblerig-class shape offers itself
    as its own partner, where the swap is a no-op.
31. **1.16.2f's split is one number, and it is not re-offered** (W8c) —
    `DecisionSpec::DivideCostReduction { total }` asks for the credits going
    on the install cost and derives the rez share, which is exactly 1.16.2f's
    "nonnegative numbers whose sum is equal to" for two costs. A modifier
    spanning three costs would need a vector. The declaration happens once, at
    the beginning of step 8.5.16d, and is not revisited if the rez is then
    declined (8.5.13d) — no example distinguishes that.
32. **1.16.1b's damage check is the prevention CLASS, not a simulation**
    (W8c, `Vm::damage_cost_blocked`) — the kernel asks "is there an active,
    mandatory, interrupt-flagged conditional whose trigger is `WouldDamage` of
    this kind and whose instructions include a prevention of that kind?", the
    same shape `tag_cost_blocked` uses. It does not run the payment as a
    hypothetical effect through the imminence pipeline, so a prevention that
    only applies under a further condition would be treated as blocking.

33. **An aborted Encounter Ice Phase finishes the instruction that aborted
    it** (W9a, `Vm::abort_encounter_phase`) — 6.5.8a's bypass and 6.2.7c's
    uninstall/derez end the encounter STATE immediately and flag the phase's
    frame, which completes without following any of its remaining steps as
    soon as it is on top again (9.8.7c: no more subroutines resolve). The
    resolving instruction is not cut short, which is what 9.11.2's atomicity
    wants and what the CR's "the phase is aborted" says about the phase, not
    about the ability. Ending the RUN is different and stays different:
    6.1.4/6.1.4b unwind every frame above the run (or above the phase),
    resolving ability included.
34. **6.2.7 is skipped entirely during a forced encounter** (W9a,
    `Vm::apply_ice_change_to_run`) — 6.2.7 governs "the piece of ice in the
    Runner's CURRENT POSITION", and a forced encounter (6.5.9a) is resolved
    outside the run's progression with an ice that need not be in that
    position or installed at all. So a forced encounter is never ended by
    6.2.7c/d. The narrow case this gives up: a forced re-encounter with the
    very ice the Runner is standing in front of (The Twins) that is then
    uninstalled mid-encounter would, on a strict reading of 6.2.7c, end.
    No example.
35. **A priority window open above an aborted encounter is not force-closed**
    (W9a) — 9.2.8f already closes reaction windows bound to the ended
    encounter, which covers every tested case. A PAID window open at step
    6.9.3b when a paid ability ends the encounter would, strictly, close too;
    the kernel lets it run out. No example.
36. **RETIRED (W15a)** — 1.18.3 is `StaticDecl::CanBeAdvancedSelf` +
    `Vm::advanceable_cards`, read by the basic advance action, with 9.1.8f
    keeping the declaration active while the card is installed and inactive.
    `Instruction::AdvanceCard` still advances whatever it names, which is
    right: an ability that says "advance a card" is not the basic action and
    1.18.3 does not restrict it.
37. **The scored snapshot is two numbers** (W9b, `Object::scored_snapshot`) —
    1.17.8 says "the agenda's last known number of advancement counters" and
    10.13.2 adds the advancement requirement, so those two are captured just
    before an agenda moves to a score area. A future ability referring to
    some other last-known characteristic of a scored agenda would want the
    same treatment, and the honest form of that is a whole last-known-state
    snapshot rather than two fields.

38. **4.6.6i's "initiated by the move" scope is not tested for** (W10b,
    `Vm::this_server`) — the rule reads the PREVIOUS server only for an
    ability initiated by a trigger condition or cost involving the source's
    move. The kernel reads it for any ability of a card that has left a
    server, on the argument that the only abilities that can resolve from
    such a card are the ones 9.1.8 keeps active across the move, which are
    exactly those. A card that left a server and then had an ability resolve
    for an unrelated reason would read the old server.
39. **The 4.6.8f remote-server limit is a creation gate only** (W10b) — the
    rule has a second half: at step 10.3.1e, if more remotes exist than an
    active limit allows, the Corp chooses which to keep and the rest are
    trashed. That wants a Decision in the minimal-set machinery (deviation 1)
    and no example demands it; `limit_remote_servers_1` is entirely about the
    creation gate.
40. **RETIRED (W11a)** — "`this_server_3` is deferred" (W10b). The missing
    delivery is `Instruction::ResolveAbilityOf`, which also lands
    `instructed_to_resolve_conditional_ability_1` (9.6.14d). What the
    primitive did NOT land is `gain_subroutines_in_any_order_1` (9.8.2c,
    an order DECLARATION — deviation 5) or `replace_subroutine_resolution_1`
    (9.8.9, a subroutine-level replacement effect); neither actually needs
    it.

41. **9.12.3's "must" is implemented for the mid-access window only** (W10c) —
    `MustTrashAccessedCard` is the shape both 9.12.3a and 9.12.3b examples
    take, and the requirement is enforced exactly where the CR puts the
    choice: the 9.2.10 window's pass. A "must" over any OTHER decision (the
    9.12.3e half about declining an additional cost to a basic action, which
    needs the basic run action to carry one) is not expressible yet, which is
    why `must_cannot_force_additional_cost_1` is still open.
42. **"Would this ability trash the accessed card?" is a shallow scan** (W10c,
    `Vm::ability_trashes_accessed_card`) — the same shape deviation 16
    describes: the ability's instruction list is scanned for
    `TrashCards(AccessedCard)` and wrappers (`Combined`, `DeclineableChoice`,
    `NestedCost*`) are not looked inside. An Imp-class ability that trashed
    the accessed card from inside a wrapper would not satisfy a 9.12.3a
    "must".

43. **A maintained choice of a server or a subtype is enumerated, not
    computed** (W10d) — `ChoiceSpec::{Server, Subtype}` name ONE value, and
    the choice between values is `Instruction::ChooseOne` with one branch
    each, which is what 9.11.4g says a choice between options that create
    different effects is. That is exact for Security Testing (five servers)
    and Pelangi (five subtypes), and it kept a new `DecisionSpec` out of the
    kernel. A card choosing from a set the kernel must COMPUTE (every server
    with ice, say) would want a real `ChooseServer` decision; nothing does.
    `ChoiceSpec::Object` already announces properly through 1.15.2.

44. **RETIRED (W13a)** — "the forfeit cost's choice is not put to the payer".
    Which agenda is forfeited is a `DecisionSpec::PaymentCards` choice
    whenever the score area holds more agendas than the cost takes.
45. **Two of the three 9.6.14 classes are implemented but untested** (W11a,
    `ability_in_class`) — `WhenScored` is exercised; `WhenEncountered`
    (9.6.14a) and `WhenInstalled` (9.6.14b, riding the new
    `TriggerCond::SelfInstalled`) have no example. They are one match arm
    each and exist so the vocabulary covers the rule, not one card.
46. **An additional cost to PLAY is not gated at the point of choosing to
    play** (W11a, `PrintedCard::additional_play_cost`) — 1.16.10b's
    combination happens at step 8.6.7b and `could_play_found_card` (8.7.2b)
    asks about it, but nothing else does, because there is no basic play
    action to gate (deviation 17) and `Instruction::PlayCard` names its card
    directly. 1.16.10a's "if the additional cost cannot be paid, the effect
    is not applied" is therefore enforced only on the search path.
47. **`RemoveAbilitiesOfMatching` reads its criteria shallowly** (W11b) —
    deviation 2b's class exactly: the declaration is gathered inside
    `char_effects`, so `filter_matches_shallow` applies and a `HasSubtype`
    criterion there reads printed subtypes.
48. **A "you may" wrapping an expanding instruction splices it in as the
    next instruction** (W11d, `Instruction::DeclineableChoice`) — an
    install/play/trace has to go back through imminence to expand into its
    step sequence and announce its own targets, so the accepted branch is
    inserted after the current instruction, exactly as 9.11.4f's nested cost
    does. The observable difference from applying it in place is one extra
    checkpoint (deviation 4's class); before W11d it silently did nothing at
    all.
49. **`InstalledCardTrashed`'s type narrowing is applied by the checkpoint
    scan** (W11c) — `trigger_matches` only receives a Corp-ness closure for
    the trashed card, so the `of_types` filter is applied where the state is
    reachable, the same way the 4.6.6i server scopes are.

Retired: W1's "persistent-ability expiry plumbed but unarmed" (W2b armed
it); W2's "10.3.1j auto-candidate declaration" (W3a implemented the real
Runner declaration Decision with 7.4.6a declined-tracking); **W3's
"Ob-class search elision" (W5a implemented §8.7 as a real instruction —
searching, finding, setting aside and shuffling are all in the kernel now,
and 8.5.13c/d reveals ride on it);** **W3's
`tk::inject_*` state manufacture and `grant_external_sub` (W4f deleted all
six; their effects are now created by real cards —
`noble_path_like`, `chum_like`, `breach_replacement_card`,
`access_replacement_card`, `additional_access_card`, `subroutine_granter` —
through `Instruction::CreateLingeringEffect` / `CreateDelayedConditional` /
`GrantSubroutines`, so nothing in the example suite stands on text no
printed card could produce, and the §12 rule 4 re-derivation gate has a
chance of passing).**

50. **The 7.4.2b access limit and `Quantity::AccessesThisRun` read ONE
    counter** (W12a, `CoreState::run_accesses`) — the count is kept for the
    run in progress and survives its end, so a "when this run ends" ability
    can still read it (6.9.6d puts those abilities after the run frame pops).
    An ability asking about an EARLIER run in the same turn would read the
    wrong number; nothing does. Accesses performed outside a run count for
    nothing at all, which is right for every "during that run" ability and
    would be wrong for a "this turn" one.
51. **`Instruction::ForEach`'s aggregated branch scales the arms that carry a
    number** (W12c, `scale_instruction`) — a set-based aggregated effect
    (trashing named cards, looking at named cards) has no per-unit value to
    multiply, so it is performed once unscaled. 9.12.2b's "if a value
    aggregated in this way is less than or equal to 0, that part of the
    effect does not take place" is left to the selector, which evaluates to
    the scaled number and is floored where the effect is applied.
52. **9.9.6c's cost value is read at the payment step** (W12c) — the
    `EffectClass::PayCost` atom is computed when `InstallStepPayCost` /
    `PlayStepPayCost` becomes imminent and applied when it resolves. The
    1.16.2f division path (`DivideCostReduction`) resumes payment through a
    Decision and does NOT re-read the modified atom, so a Patchwork-class
    interrupt on an install-and-rez that also carries a "total N less"
    modifier would be lost. No example combines them.
53. **A mark is only ever designated by `IdentifyMark`** (W12d… W12b, §10.11)
    — 10.11.2a's "any method with equal probability" is the kernel RNG, and
    10.11.4's expiry rides `Duration::Turn`. Nothing else can designate or
    clear a mark, which is all 10.11 asks for.
54. **9.8.2c declarations are made per ABILITY, against the list as it
    stands** (W12d) — which is what the rule says ("each subroutine the ice
    has at that time"). Two abilities resolving in the same window each
    declare separately, in resolution order; the kernel never has to
    reconcile two simultaneous declarations, and the CR does not say what
    would happen if it did.
55. **`SelectsDamageTrashes` has a count, not an ordinal** (W12e) — Chronos
    Protocol's "the first time each turn" and Titanium Ribs' "you choose the
    cards you trash" differ in the kernel only by `count`. The once-per-turn
    limit is elided (deviation 4's class: no example damages twice in a turn
    through it), and the selection Decision is asked once per damage
    instruction rather than once per point.

56. **A payment asks only where a real choice exists** (W13a,
    `Vm::advance_payment`) — the 1.16.2c X announcement and each 1.16.2e
    alternate payment are always offered, but a card component whose
    candidates exactly equal the number it takes, and a credit division where
    the payer is spending everything they have or has only one location, are
    completed without a Decision. Nothing is chosen in those cases; the CR
    still nominally has the payer choose, and 1.16.1d's "costs of 0 are not
    paid automatically" is the same class of elision the kernel has always
    made for forced decisions.
57. **The bad-publicity fund is not one of the 1.10.3c locations** (W13a) —
    it still spends FIRST during a run, before the division is offered, and
    the division covers only what is left. No example puts a bad-publicity
    credit and a hosted credit in the same choice.
58. **An additional cost to score is aggregated from the printed card only**
    (W13b… W13a, `Vm::score_cost_of`) — `steal_cost_of` also folds active
    `StaticDecl::AdditionalStealCost` declarations; the score side has no
    declaration form because no example has a Ben-Musashi-for-scoring.
59. **6.7.4's clause is one per run and fires once** (W13b) — `RunCtx::
    if_successful` holds a single clause, cleared when it pends. Two
    "if successful" effects on one run (a second initiating effect moving the
    run) would lose the first. 6.7.4a's set is checked when the run is
    DECLARED successful, which is the moment the rule names.
60. **`MustRunWithFirstClick` reads "first click" as "no clicks spent this
    turn"** (W13c) — `p.clicks == p.allotted_clicks`. A card that grants a
    click mid-turn would confuse it; nothing does. The discharge flag is
    per-turn state on `CoreState`, reset in `push_turn`.
61. **`ClicksSpentOnAction` counts until the next action begins** (W13e) —
    `CoreState::current_action` is set when an action is initiated and
    replaced when the next one is; it is not cleared at 5.2.2a completion,
    so a click spent outside any action (a nested cost during a run) counts
    against the action that initiated the run. 1.16.4d's example is exactly
    an action that "has no other effects", where the two coincide.
62. **`DifferentActionsThisTurn` counts EVERY action this turn** (W13e) —
    5.2.5b's identity is exact, but the condition is met only when the count
    is precisely `count`, which is the MirrorMorph reading ("the first time
    each turn you take 3 different actions"). A card asking "have your
    actions so far all been different" would want `>=`.

63. **The §10.2 view is over the STATE, not over the change log** (W14a) —
    `Vm::view_of(side)` redacts the game state as it stands; there is no
    per-side redaction of `ChangeBuffer::log`. Every §10.2 example is asserted
    by halting the plan where the information would leak and reading the two
    views, which is stronger for those examples (the claim is about what a
    player can tell APART, and two `Unseen` entries are equal). A redacted log
    is what the server's replay/redaction view will want at cutover, and
    `record` would have to take the Vm to build it as changes happen.
64. **A sighting lapses when the card moves** (W14a, `Sightings::forget`,
    1.21.6) — every other visibility is derived from the zone, so a player who
    is continuously entitled to a card keeps seeing it. The two exceptions the
    CR states are implemented: 7.1.2 (the card being accessed) and 7.3.1a (an
    object accessed earlier in the breach in progress), and the latter is
    move-immune, which is what its own example needs. A human tracking a
    revealed card into HQ would know more than the kernel says they do —
    10.2.2a lists "cards in HQ" as hidden information, which is the reading
    taken.
65. **8.3.3's secret order is a Decision whose answer is not shown** (W14c) —
    `DecisionSpec::ArrangeCards`. The kernel has no channel by which one
    player sees another's answer, so "they do not declare which cards moved to
    which locations" holds by construction; the arranging player is granted a
    sighting of the returned cards (8.3.3 + 4.2.3, they placed them) and
    8.3.3a keeps the opponent out.
66. **The draw procedure is expanded only for `Instruction::Draw`** (W14b) —
    HALF CLOSED, W18b. The basic draw action (5.2.6c/5.2.7c) now resolves
    `Instruction::Draw` in a rules ability frame and gets the whole 8.4.5
    procedure; it had been calling `draw_cards` synchronously, so its draw
    never became imminent at all and The Class Act could not see the
    commonest draw in the game. The MANDATORY draw (5.6.1) is still
    `Instruction::MandatoryDraw` resolving `draw_cards` directly: it DOES
    carry a Draw atom, so it becomes imminent and a "would draw" interrupt
    reaches it, but the cards are never set aside and a
    Daily-Business-Show-class ability does not see THEM. Same fix, same
    pattern; every test that starts a Corp turn would be re-timed by it.
67. **6.8.2b's "opened due to a phase beginning" is read as the 9.2.8f
    binding** (W14d) — `WindowFrame::originating_structure` is set for any
    reaction window opened during an encounter and `None` otherwise, so a
    window opened by a RUN phase beginning (say `step_approach_begins`) would
    be completed under 6.8.2c rather than closed under 6.8.2b. Every tested
    case is an encounter window, which the binding gets right; a finer
    predicate would flag the windows opened by the checkpoint after a phase's
    first step.
68. **A window surviving 6.8.2c is re-pushed above the run frame** (W14d) —
    if it was opened inside a breach, the breach frame below it is popped
    while the window lives on. Nothing in the vocabulary lets a pending
    ability in that window read the breach it was opened in; a delayed breach
    (7.3.8) is the case the rule names, and it is blocked by
    `no_new_timing_structures`.
69. **9.8.9 replaces EVERY imminent subroutine while the declaration is
    active** (W14f, `StaticDecl::ReplaceSubroutineResolution`) — the printed
    Bankhar class conditions the replacement on the encountered ice not having
    been fully broken; the shape leaves the condition off, and the tests scope
    it by installing the card only where the replacement should apply. The
    rule under test is what the replaced subroutine's SOURCE is, which the
    condition does not touch.
70. **A `CounterRef` is derived, not stored** (W14h) — a counter's identity is
    `(host, kind, ordinal)`, which is exact when an instruction announces it
    (1.15.2) and is what 1.15.1 needs, but it is NOT 1.12.1 identity: a
    counter that moves between cards gets a new `CounterRef`. Full counter
    objects mean replacing `Object::counters`, which reaches every counter
    reader in the kernel; no example needs it, and the corpus port should
    decide whether it does.
71. **The mandatory-loop count is per DETECTION, not per turn of the loop**
    (W14g) — the Decision is asked at the push that closes the second
    repetition and the budget counts the pushes after it, so choosing `n`
    resolves `n` further turns of the loop and then ends it. The CR's "the
    loop instantaneously resolves that many times" says nothing about where
    the counting starts; `example_rule_mandatory_infinite_loop_1` asserts the
    slope (one turn per unit chosen), which is the observable content.
73. **The 8.5.16b declaration is one Decision listing every destination**
    (W15a, `Vm::install_destinations_for`) — the basic install action offers
    servers, new remotes and eligible hosts in ONE `DeclareInstallDestination`,
    which is what 8.5.16b's "including any host relationships" says. An effect
    that STATES a destination still gets deviation 13's separate host choice;
    the two paths should converge when a card needs both.
74. **`cards.rs` partial cards are marked, not approximated** (W15a) — a
    printed sentence the vocabulary cannot say is quoted in the card's doc
    comment after `UNIMPLEMENTED:` and counted by `dp7c_odometer`. Six of the
    19 cards are partial. A partial card is legitimate only while the missing
    clause is orthogonal to every test using it; the tests that WOULD exercise
    it (`fan-site`, `clot`, `project-beale`) are triaged as blocked, not
    ported.
75. **The corpus port re-expresses reference-internal setup** (W15b) — 1041
    of 3717 reference tests poke the reference's own API in the test body
    (`core/gain`, `core/command-counter`, …). A port re-expresses the poke as
    setup where it is state (a starting click count, a counter placed before
    the script runs) and records the test as out of scope where it is
    plumbing. `docs/vm/UPSTREAM-DEFECTS.md` §2 is that ledger.
72. **The loop detector looks at ABILITY frames only, to depth 4** (W14g,
    `Vm::loop_period`) — a cycle whose period is longer than four abilities,
    or one made of timing structures rather than abilities (a run that
    initiates a run), is not detected. 10.1.6b's optional loops are out of
    scope by construction, which is right.

## The test pattern, now mandatory (ARCHITECTURE §12 rule 5)

Every new example test declares: setup (cards, hands, credits — data), ONE
`plan::Plan` per player (data: ordered `when <Match> → <Reply>` rules plus a
fallback policy), and assertions. The shared driver `plan::Script` folds
`Vm × Plan(Corp) × Plan(Runner) → Transcript`; nothing else may drive the
VM, and `tests_are_plans_not_loops` fails the build on `vm.answer(`,
`while vm.step`, `loop {`, `inject_` or `vm.lingering.push` in a test.

Assertions about what was *offered* are made on the `Transcript` after the
fold — `first_window(Kind::Reaction, Side::Corp).count("hostile-infra")`,
`ever_offered("tori")`, `times_taken("parasite")`, `Entry::{options, actions,
choices, candidates, cost, spec, side, seq, stack, step}` — never from
inside a driver. Mid-flight state assertions use `Reply::Halt`, which
suspends the driver leaving the decision unanswered; `Script::run` again to
resume with ordinal counters intact. `Plan::forbidding_the_rest()` turns "no
other decision may occur" into a checked claim (the playable slice's central
assertion). Reusable fragments: `runs(server)`, `uses(label)`,
`always_uses(label)`, `trashes_on_access()`, `otherwise_click_credit()`,
`stop_at_action()`.

New card shapes go in `testkit.rs` and are built EXCLUSIVELY through
`PrintedCard` + `AbilityDef` + `Instruction`; any simplification inside a
shape is annotated in its doc comment and is legitimate only while
orthogonal to every example using it.

W13 adds a fourth plan-driver fact: a payment now asks its own Decisions, so
a plan that pays a non-trivial cost needs rules for `Match::declare_x()`,
`Match::alternate_payment()`, `Match::payment_cards()` and
`Match::division()`. They are deliberately NOT `Kind::Targets` — CR
1.15.1/1.15.2 scope a target announcement to an instruction, and a cost is
not one — so they never collide with a 1.15.2 announcement rule.

Three plan-driver gotchas, all instances of deviation 12. The first two W11
hit repeatedly; the third is W12's, and it cost more time than anything else
in the wave: **an ordinal counts PER RULE and only advances on decisions the
driver actually EVALUATES that rule against.** Two rules that both want "the
nth decision of a kind" must therefore both be written `.once()` — a second
rule written `.nth(2)` never fires, because the first rule answered the
decision its own counter was on and the second rule's counter never saw it.
The same trap bites `stop_at_action_nth(3)` after two action rules.
And:
a `when(Match::paid(), Reply::take("x"))` rule with no `.once()` will take a
FREE ability every window forever and blow the 600-decision budget; and a
delayed conditional armed by an ability that also initiates the run is never
created at all, because 9.6.13d requires the run to be in progress when the
`CreateDelayedConditional` instruction resolves (arm it from a paid window
INSIDE the run instead). Also: a paid ability offered to BOTH sides needs
`Match::paid()` scoped by side implicitly (the plan is per player), but a
Corp-side shape installed with `tk::install_rig` is still controlled by the
CORP — check the transcript's `side` before assuming a rule will fire.

Since W9a the Encounter Ice Phase is its own timing structure, so
`Match::during(StructKind::Encounter)` joins `at_step("step_encounter_paw")`
as a way to scope a plan rule, and `Entry::stack` shows it. Note that the
encounter's paid window yields no Decision at all when neither player has a
usable paid ability — a plan that wants to halt mid-encounter must give
someone something to be offered (`tk::break_button` is the usual one).

W14 adds four decision kinds and one assertion surface:
`Match::arrange()` / `Reply::Arrange` (8.3.3's secret order),
`Match::loop_count()` / `Reply::LoopCount` (10.1.6a), `Match::counter_targets()`
/ `Reply::Counters` with `Entry::counters()` (1.15.1 counters as targets), and
the §10.2 state view — `vm.view_of(side)` with `View::{in_zone, count_in,
sees, group, choice, credits_of}` and `vm.identity_visible_to(obj, side)`,
plus `vm.set_aside_groups()` for 4.8.7. §10.2 assertions are made by halting
where the information would leak and comparing the two views: two `CardView::
Unseen` entries are EQUAL, which is what "cannot tell which" means.

Three more plan-driver facts W14 paid for:
- a conditional whose source is a Corp card is controlled by the CORP even
  when the effect moves the RUNNER (the Mirāju case, `run_phase_after_1`):
  put the rule in the right plan and check `Entry::side` first;
- `AbilityDef::conditional(.., optional: true)` makes the INSTANCE
  non-mandatory (the reaction window gains a pass), not an `OptionalEffect`
  decision — match it with `Match::reaction().offering(..)`, not
  `Match::optional()`;
- a testkit shape whose label is used by `Reply::take` must be matched by a
  distinctive substring of the LABEL as written (`"do net damage"`, not
  `"net-damage"`), and a `Take` reply carries an implicit offer guard, so a
  mistyped needle makes the rule silently never fire rather than fail.

## What is next — DP-7a is done

DP-7a is **243/243**. There is no example backlog; `dp7a_odometer` asserts it
and `dp7a_backlog_placeholder` has nothing left to list. Re-run the count
before believing this file:

```
python3 - <<'EOF'
import json,re
v=json.load(open('docs/rules/examples.json'))
src=open('crates/jinteki-cr/tests/cr_examples.rs').read()
i=src.index('const IMPLEMENTED'); j=src.index('];', i)
impl=set(re.findall(r'"(example_[a-z0-9_+]+)"', src[i:j]))
missing=[(e['section_number'], e['id']) for e in v['examples'] if e['id'] not in impl]
print(len(missing)); [print(s, i) for s, i in sorted(missing)]
EOF
```

(One id, `example_rule_54+_1`, is listed with a test function named
`example_rule_54_1` — `+` is not a Rust identifier. That is the only
divergence between the ledger and the test names, and it is deliberate.)

**The queue, in the user's stated order (DP-7c is IN PROGRESS — read
`docs/vm/CORPUS.md` first; it carries the measurement, the helper mapping,
the porting method and the triage state):**

1. **DP-7c — the jinteki-reference corpus port, triaged against the CR.**
   ARCHITECTURE §12 rule 4's re-derivation gate is its entry criterion and it
   is now the most valuable thing in the campaign: every `testkit` shape used
   by a CR-example test is re-derived from the corresponding real card's
   printed text, and the DP-7a suite must pass **unchanged**. Divergence means
   harvested overfit — either the example test is wrong or the kernel is, and
   the CR decides which. Every shape carries a doc comment naming its class
   exemplar and annotating its simplifications; those annotations are the
   worklist.
2. **The two decks** (estrike Andromeda, Gauntlet NTM) from printed oracle
   text — started in `crates/jinteki-cards` (a designer-facing DSL, both decks
   written, 46 of 51 cards partial with quoted `unimplemented:` sentences).
   **Reconcile it with `crates/jinteki-cr/src/cards.rs`** before either grows:
   CORPUS.md §7 says how, and it is one-directional.

   The DP-7c gap list (CORPUS.md §5) is what both rungs consume. The next
   card-layer mechanisms the corpus asks for, in order: "the Runner loses
   [click]" as an instruction (Enigma, #7 by frequency), an agenda-point
   modification (2.5), a rez-cost modification scoped to a server, a movement
   into a score area for a non-agenda, and a scoring prohibition scoped by
   when the agenda was installed (Clot).
3. **FT-1/FT-2/FT-3** (algebra extraction, vocabulary collapse,
   Legality/Viewpoint/Replay interpreters), which the user deferred until
   after the above. `FINAL-TAGLESS.md` stays normative as the TARGET.

**Cheap kernel work that the deck/corpus phase will want, none of which any
example needs** (the honest gap list; the DP-7c half of it is CORPUS.md §5):

- ~~the basic INSTALL action (5.2.6d/5.2.7d) and the basic ADVANCE action
  (5.2.6f)~~ — **done, W15a**, along with the trash-resource and purge actions
  and 1.18.3's "you can advance" permission (deviation 36 closed).
- routing the mandatory draw through `Instruction::Draw` so it gets the 8.4.5
  procedure (deviation 66; the basic draw action was done in W18b).
- ~~"Run any server" as a chosen `InitiateRun.server` (6.7.4a)~~ — **done,
  W16b**: `server: Option<ServerId>` plus 6.9.1a's announcement as a real
  decision (`DecisionSpec::DeclareAttackedServer`, `plan::Reply::Server`).
- `Vm::view_of` over the change log as well as the state (deviation 63) — the
  server's redaction view at cutover wants exactly that, and the corpus asserts
  on the reference's log 148 times.
- 8.5.6's optional "may first trash any number" (deviation 8).
- the 4.6.8f remote-server limit's second half (deviation 39).

### The two priority decks: what the kernel cannot yet say

Measured, not guessed: `crates/jinteki-cards` carries both decks as cards and
prints the count. At W21 it is **51 cards, 51 complete, 0 partial, 0 printed
sentences unsayable** — both decks are whole, `cr::readiness()` reports ready,
and `cr::eternal_setup` returns a game rather than a refusal. (It began at 80
unsayable sentences across 5 complete cards, on a 51-card list before Hedge
Fund left it.)

**W20's four honest partials are in `decks/unlisted.rs`, outside the
odometer**, and every one of their gaps is a general capability rather than a
card:

- **a one-shot delayed conditional on the NEXT access this run.** 9.6.13's
  delayed conditionals exist, but nothing states "the next time this run you
  access X" — met once and then gone. Whistleblower and RNG Key both need it.
- **a steal that ignores all costs.** `Instruction::StealIfAgenda` has no
  position for overriding 1.16.10's additional steal costs, and a steal that
  quietly skipped an unpaid Obokata-class cost would be worse than a marker
  (Whistleblower).
- **comparing a printed VALUE against a maintained number.**
  `TargetFilter::MatchesMaintainedChoice` compares characteristics that ARE
  the named thing; "a rez cost, play cost, or advancement requirement equal
  to the named number" compares a different characteristic to a named number
  (RNG Key).
- **cards selected from a hand AT RANDOM as targets.**
  `Instruction::TrashRandomFromHand` trashes without naming, so no later
  instruction can act on the same cards, and nothing counts "each card
  trashed this way" (the `CreditsLostThisAbility` shape, for trashes) —
  Embezzle needs both.
- **repeating a process whose repetition count is not known when it
  resolves.** Complete Image's "if you trash a card with the chosen name this
  way, repeat this process" is not `ForEach` over a computed quantity (the
  count depends on what the random damage trash turned up, and each pass
  names a NEW card) and is not 10.1.6a's loop (which is about abilities
  already resolving each other).

The card-authoring surface is now an EMBEDDED DSL — typed builders over the
kernel vocabulary, `docs/cards/EDSL.md` — so a missing verb is no longer a
reason for anything: the deck files reach the whole public vocabulary
directly. **Every entry below is therefore a real kernel gap**, a sentence a
card in these decks needs that the kernel has no way to express. ARCHITECTURE
§12 forbids a card-shaped variant, so each is stated as a GENERAL capability
with the cards that want it named. The deck modules carry the matching
`.unimplemented(…)` markers as data, so this list and the test's count move
together — and `tests/decks.rs` ratchets the count, so it cannot quietly grow.

~~**A defect, not a gap — fix this one first.** `Vm::char_effects` gathers
characteristic declarations behind `card_active(o)` alone…~~ — **fixed,
W16d**: `char_effects` now filters through `ability_active` (and honours a
9.6.7 static condition), so `[threat N]` and every 9.1.8 exception reach
strength and subtype modification. Shibboleth is complete.

*Instructions with no variant at all:*

- ~~"take N[credit] from this card"~~ — **done, W16e**:
  `Instruction::TakeHostedCredits { from, amount, to }` (1.10.3a — the credits
  enter the pool, so it is a GAIN). Daily Casts is complete in `cards.rs` and
  ported.
- ~~"remove N hosted <kind> counters" from a card, outside a cost~~ — **done,
  W16e**: `Instruction::RemoveCounters { target, kind, amount, up_to }`.
- ~~"the Runner did NOT initiate any runs during their last turn"~~ — **done,
  W21**: `TriggerRequirement::RunnerMadeRun` carries the polarity as content
  (§12 rule 2), so the negative sentence is the same question asked for the
  other answer.
- ~~"gain [click]" (Petty Cash, Subliminal Messaging)~~ — **done, W16a**:
  `Instruction::GainClicks(Side, Quantity)` and its `LoseClicks` twin
  (1.11.3a/b).
- ~~"cards that share a type when this encounter began" (Slot Machine)~~ —
  **done, W21**: 1.21.3a is why it needed anything at all — revealing puts the
  card back exactly as it was, so nothing about the card records that it
  happened. `EncounterState.revealed` does, which is also the scope the
  printed words name, and `TargetFilter::RevealedThisEncounter` is the
  description; `Quantity::LargestGroupSharingCardType(criteria)` is the amount
  (2.15: exactly one type per card), and
  `TriggerRequirement::QuantityAtLeast { amount, at_least }` compares any
  calculated amount to a printed threshold — so "2 or more" and "3 or more"
  are ONE selector asked twice (§12 rule 2). **Defect fixed: `Instruction::IfMet`
  never announced its branch's targets**, the same class as W14b's
  `MoveToDeck`, W17b's counters, W17c's `ModifyStrength` and W20's
  `RevealCards`: a targeting instruction inside "if <state>, <do this>"
  silently acted on nothing.
- ~~1.21.3 REVEAL~~ — **done, W16e**: `Instruction::RevealCards { cards }`,
  with 1.21.3a (revealing is not turning faceup) exact. **On the deck rung it
  unblocks nothing yet.** Mutual Favor's first sentence ("Search your stack
  for 1 icebreaker and reveal it") is now fully sayable — but expressing it
  ALONE would strand the found card in the set-aside zone forever, because its
  SECOND sentence ("if you made a successful run this turn, you may install
  that program; if you do not, add it to your grip") is what disposes of it,
  and that needs a "made a successful run this turn" predicate. A card that
  searches and then loses the card is worse than one that does nothing, so
  both sentences stay marked until the predicate lands.

*A shape that does not fit, found by using it:* `Instruction::Combined`
resolves by walking its effect ATOMS and matching on `EffectClass`, so an
effect whose atom is `Structural` — `RemoveCounters`, for one — is dropped
SILENTLY when it is put inside a `Combined`. Earthrise Hotel's "remove 1
hosted power counter and draw 2 cards" was written that way first and removed
no counter. It is correct as two instructions (9.11.4a: only same-class
effects aggregate), which is how the card now reads, so this is a sharp edge
rather than a blocker — but a `Combined` that silently drops what it cannot
classify will cut someone again.
- "remove <a card that is not the source> from the game" (Bloo Moose; also
  Jackson Howard's trigger cost).
- ~~"add <a card> to your score area"~~ — **done, W17a**
  (`Instruction::AddToScoreArea`). Film Critic's paid ability is expressed;
  the card is still partial on its other two sentences.

- ~~"a card you did not install this turn"~~ — **done, W17a/b**
  (`TargetFilter::InstalledThisTurn`, plus `PlaceCounters` joining the
  target-announcement dispatch in W17b, which is what actually made it work).
  Seamless Launch is complete. AstroScript's paid ability still waits on a
  criterion for 1.18.3's "a card you can advance"; Slot Machine's third
  subroutine waits on the reveal-counting its first sentence needs.
- ~~"If you played this operation from anywhere except HQ" (Petty Cash)~~ —
  **done, W21**: plays record their ORIGIN the way installs do — 8.6.7a places
  the card into the play area from somewhere — and
  `TriggerRequirement::SourcePlayedFrom { from, is }` asks about the play IN
  PROGRESS rather than about the history, with the zone and the polarity as
  content.
- ~~"[click]: Play this operation from Archives. After it resolves, remove it
  from the game." (Petty Cash)~~ — **done, W21**: CR 8.6.6d names the pair as
  ONE construction — a playing ability that "also contains the nested
  conditional ability" does not trash the card at 8.6.7g at all — so it is
  `Instruction::PlayCard { then_remove_from_game }`. Written as two
  instructions it could not work: 9.1.4 stops an ability acting on a source
  that changed zones, and playing the card moves it into the play area.
  "From Archives" is `TimingRestriction::SourceInZone`, 9.3.3c's "limits on
  when, WHERE, or how often an ability can be used", checked at every site
  that offers a paid ability — including the action window, which 5.2.1 is
  where a [click]-cost ability is offered and which checked no restriction at
  all before.
- ~~"shuffle up to N cards from Archives into R&D" (Jackson Howard; Boomerang
  shuffles from the heap into the stack).~~ — **done**:
  `Instruction::ShuffleCardsIntoDeck`. Boomerang's own "when this run ends"
  half is a 9.6.13 DELAYED conditional, which is what a card trashed to pay
  its own trigger cost needs — there is no source left to carry a conditional
  ability — and 9.6.13d keeps it from existing outside a run at all.
- ~~swapping the identity in play (Rebirth) and flipping a double-sided
  identity (Nebula Talent Management)~~ — **done**:
  `Instruction::SwitchIdentity { side, with }` over CR 1.5.4a's pile, which is
  a real zone (`Zone::OutsideGame(Side)`), so the identity switched to is an
  ordinary announced target and the one it replaces goes back by an ordinary
  `move_card` (1.5.4b). Rebirth is COMPLETE.

- ~~**gaining abilities (9.1.9)** — DJ Fenris's "gains the text of hosted
  identity"~~ — **done, W22d**: an object's abilities are no longer a
  presence MASK over its printed ones. 9.1.9b says the abilities an object
  actually has come out of 9.12.1d/e's procedure, so `Effective` carries a
  computed `gained_abilities` list beside the loss mask, `Vm::abilities_of`
  is the single accessor every enumeration site reads (an `AbilityRef` index
  above the printed list names a gained ability), and
  `StaticDecl::GainAbilitiesOf` is the declaration — the house shape
  `GainSubtypesOf` and `RemoveAbilitiesOfMatching` already had.
- ~~**a destination override that outlives its source's activity** — DJ
  Fenris's "remove hosted identity from the game if DJ Fenris is
  uninstalled"~~ — **done, W22d**, and it is not a destination override at
  all: the sentence belongs to the same ability as the hosting, so it is a
  9.6.13 DELAYED conditional created when the hosting happens. 9.10.1 keeps
  a lingering effect alive after its source has left the play area, and
  1.15.4 lets the created ability act on the card the same ability already
  chose — which is the only way to still know WHICH identity once 1.13.13
  has severed the hosting. (9.1.8g would keep a printed conditional active
  for exactly this zone change, but by the time it resolved it would have
  nothing left to name, which is what made this look unreachable.)

*Positions that exist but are not quantity/target positions (§12 rule 6):*

- ~~`InitiateRun.server` is a concrete `ServerId`~~ — **done, W16b**: it is
  `Option<ServerId>`, and `None` puts 6.9.1a's announcement to the Runner
  (Clean Getaway, Pinhole Threading, Dirty Laundry).
- ~~`Instruction::ModifyStrength.amount` is `i32`~~ — **done, W17c
  (BREAKING)**: it is a `Quantity`, so "+X strength" (Paperclip, Corporate
  Troubleshooter) is sayable, and `ModifyStrength` now ANNOUNCES its target
  (it never did — the same defect class as `MoveToDeck` and `PlaceCounters`).
  `StaticDecl::StrengthMod.delta` is still `i32`; a static "+N strength"
  reads through `SelfStrength(Quantity)` instead. (The neighbouring
  "+1 strength for each tag the Runner has" — Resistor — turned out NOT to be
  a gap: `SelfStrength(Quantity)` is how `cards.rs` already reads Ice Wall's
  "+1 strength for each hosted advancement counter", printed value included,
  and Resistor is the same sentence. It is complete.)
- ~~`Instruction::LoseCredits(Side, u32)`~~ — **done, W17c (BREAKING)**: it
  takes a `Quantity`, and `Quantity::CreditsInPoolOf(Side)` is "loses all
  credits in their credit pool". Closed Accounts is COMPLETE.
- `Cost::trash_from_hand: u32` — "trash all cards from your grip" as a cost
  (Citadel Sanctuary).
- `InstallCard::reduce_total` is evaluated only when `and_rez` is set, since
  1.16.2f's "total" needs two costs to divide between. A plain 1.16.6 install
  discount — "Install 1 resource from your grip, paying 3[credit] less"
  (Career Fair) — has nowhere to land.
- ~~`TargetSpec::TopOfDeck(Side, u32)` (The Class Act's "top X cards").~~ —
  **done, W18b**: `TopOfDeck { side, count: Quantity }` is a quantity position
  (§12 rule 6), and `Quantity::ImminentValueOf(EffectClass)` is the count The
  Class Act derives from the draw it is interrupting (9.9.6).
- ~~a general "instead of breaching, <arbitrary instructions>" replacement~~ —
  **done, 82bfd54** (`ReplacementTransform::SuppressAndResolve`), along with
  `Quantity::CreditsLostThisAbility` and `TriggerCond::MakesSuccessfulRun`.
  Account Siphon and Desperado are complete. Pinhole Threading still is not:
  it needs the chosen-server run below, an access into the root of a server
  other than the one being breached, and a per-access prohibition on stealing
  or trashing.

*Criteria the shared filter vocabulary lacks (§12 rule 5):*

- ~~"a card you can advance" (1.18.3)~~ — **done, W17d**:
  `TargetFilter::CanBeAdvanced`, derived from the same
  `Vm::advanceable_cards` the basic advance action reads.
- "a card you did not install this turn" (Seamless Launch).
- ~~"the Corp's identity" — an identity is not installed, so every side-scoped
  filter misses it (Employee Strike).~~ — **done, W18a**:
  `TargetFilter::ControlledBy(Side)`, which is 1.14.2's controller and
  therefore reaches a card that is not installed.
- criteria on `TargetSpec::AccessedCard` — "the non-agenda card you are
  accessing" (Cupellation).

*One more defect-shaped gap.* `Vm::hosts_onto_itself` derives 1.13.6b by
scanning an object's instruction lists for `HostCards { host: SelfSource }`.
That makes the exclusion depend on whether the OTHER half of the card is
expressible: Cupellation's "Limit 1 hosted card" and Film Critic's "can host
a single agenda" are exactly `StaticDecl::CanHost`, but with their hosting
abilities still unsayable the declaration turns into a 1.13.6a install
permission — measured, not guessed: `eligible_hosts_for` then offers
Cupellation as a host for any program and both cards as hosts for an agenda
being installed. Both sentences are therefore marked unimplemented even
though the words exist. 1.13.6b wants to be a property of the card, not a
scan of its instructions.

*Trigger conditions the checkpoint cannot detect:*

- ~~**"When a discard phase ends"** (5.5.4)~~ — **done, W16d**:
  `TriggerCond::DiscardPhaseEnds(Side)`, met where 5.1.4b says it is met (the
  formal end of the turn). **It unblocks none of its three cards yet**, and
  the reason is one shared shortfall rather than three: each pairs the
  condition with a state requirement, and `ability::trigger_requirements`
  reaches only `SelfAccessed`/`SelfScored`, so a `TriggerCond` variant with no
  `requires` field cannot carry one at all.
  - ~~"…while you are tagged" (Citadel Sanctuary) has nowhere to put its
    requirement~~ — **done, W17d**: `DiscardPhaseEnds { side, requires }`
    carries a 9.6.5c requirement like every other condition.
  - ~~"…if you scored this agenda this turn" (Breaking News) and "…if you
    installed this resource this turn" (The Class Act) need a predicate that
    reads what a player did this turn.~~ — **done**:
    `TriggerRequirement::{SelfScoredThisTurn, SelfInstalledThisTurn}`, read
    from the change log since the turn began. W18b also made WHOSE discard
    phase content — `side: Option<Side>`, `None` being the sentence that
    names no player, which is what both of these cards actually print.
- "Whenever the Runner breaks a printed subroutine on this ice" (Gold
  Farmer), and "the first time each turn this program fully breaks a piece of
  ice" (Bukhgalter) — `PassedIceAfterFullyBreaking` is the PASS, not the
  break. (6.5.7a's "when the Runner fully breaks THIS ice" IS now real:
  `TriggerCond::SelfFullyBroken`, W16b. 6.5.7b's "…using abilities on a
  single object" — which is what Bukhgalter needs — is not.)
- "When your action phase ends" (Nebula Talent Management).
- ~~a subtype stipulation on `EncounterBegins` — "whenever you encounter a
  barrier" (Paperclip).~~ — **done, W21**: `EncounterBegins { of_subtypes,
  requires }`, the subtype read through the 9.12.1b pipeline like every other
  subtype query and the requirements carrying 9.1.8b's zone statement, which
  is how a program in the HEAP talks at all — and what keeps the same ability
  from offering an install out of the grip, where its printed words do not
  reach. The neighbouring "can this program interface with the barrier you are
  encountering" is `TriggerRequirement::CanInterfaceWithEncounteredIce`
  (3.9.5g's strength, 3.9.5h's subtype) and deliberately NOT 9.3.6d's
  interface flag: the flag is checked when the ability is OFFERED, and the
  sentence asks after "+X strength" has resolved.
- "whenever the Runner plays or installs a copy of <a named card>" (Targeted
  Marketing), which also needs naming a card at all.
- "when an agenda is scored **or** stolen", by either player (The Source);
  `RunnerStealsAgenda` is half of it.
- ~~a condition on a card sitting in a discard pile (9.1.8b) — "when your turn
  begins, if this card is in Archives…" (Subliminal Messaging).~~ — **done,
  W21**: 9.1.8b's FIRST sentence, which nothing had read yet. A 9.6.5c
  requirement naming a zone IS the ability "stating that it is active in a
  particular zone", so `ability::requirement_states_zone` answers it from the
  requirement list and `TriggerRequirement::SourceInDiscard` is the statement
  Subliminal Messaging makes. `TriggerCond::TurnBegins` grew `requires` (the
  house shape `DiscardPhaseEnds` already had) because the stipulation has to
  be part of the CONDITION: put in the instructions (9.6.5d) it would be
  checked too late to make the ability active at all.
- ~~"the first time each turn you play a **copy of** <this card>" (Subliminal
  Messaging)~~ — **done, W21**: `TriggerCond::CardPlayed` grew `criteria`
  (the shared filter vocabulary, so 10.1.5's "a copy of" is
  `TargetFilter::HasName`) and `first_each_turn`. The ordinal is deliberately
  NOT 9.3.6g's flag: 9.3.6g is per OBJECT (the CR's own Vaporframe Fabricator
  examples), so a second copy of the card would carry a fresh one and gain a
  second [click], and 9.1.6 never counts a MANDATORY ability as "used" at
  all. It is a 9.6.5c stipulation about the occurrence, counted from the
  change log since the turn began (10.2.1) — the same shape
  `SuccessfulRunOnMark { first_each_turn }` already used.

*Restrictions and declarations:*

- ~~**"Play only if <state>"**~~ — **done, W16d**:
  `StaticDecl::PlayOnlyIf(Vec<TriggerRequirement>)` (9.1.8c) with
  `TriggerRequirement::{RunnerTagged, RunnerMadeRunLastTurn}` as the shared
  state-predicate vocabulary, enforced by `Vm::play_permitted`. Hard-Hitting
  News and Self-Growth Program are COMPLETE; Closed Accounts carries its
  restriction. Three cards still wait, and each wants one more PREDICATE
  rather than more machinery:
  - ~~"Play only if the Runner has **at least 2 tags**" (BOOM!)~~ — **done,
    W17d**: `TriggerRequirement::RunnerTagsAtLeast(u32)` replaced
    `RunnerTagged` outright; `RunnerTagsAtLeast(1)` IS "tagged".
  - ~~"Play only if you have not finished an action yet this turn" (Petty
    Cash).~~ — **done, W21**: `TriggerRequirement::ActionsFinishedThisTurn`,
    read from a change the log did not carry. 5.2.2a defines FINISHED — "once
    an action is initiated, it must be completed before the game can advance
    to the next step or open another action window" — so the action step
    reaching its own closing checkpoint IS the action having finished, and
    `GameChange::ActionCompleted` is recorded there. 5.2.2d agrees: that is
    the reaction window an "action finishing" condition resolves in, so the
    record also gives that class of card a real occurrence to meet.
  - Predictive Planogram's "if the Runner is tagged, you may resolve both
    instead" is a different sentence — a requirement on an OPTION, not on the
    play — and is still unsayable.
- "The advancement requirement of all agendas is increased by 1" (The
  Source); `ScoreRequirementModInSourceServer` is scoped to one server.
- ~~`PlayedNotTrashedUntilAgendaSteal` ends only on a steal, so a Runner
  current reading "…or an agenda is **scored**" cannot use it (Employee
  Strike). The ending condition wants to be content, not a variant name.~~ —
  **done, W18a (BREAKING)**: `PlayedNotTrashedUntil { until: Vec<TriggerCond> }`
  carries the ending occurrences as content, including the "another current is
  played" half that neither side's shield used to have.
- ~~a `TimingRestriction` keyed to a maintained choice — "use this hardware
  only during encounters with that ice" (Boomerang). The existing variants
  key on subtype.~~ — **done, W21**: `EncounterOnly` carries
  `required_choice` beside `required_subtype`, both content on one atom (§12
  rule 2) — 9.3.3c makes "use this card only during encounters with that ice"
  a restriction and 9.10.3 is what "that ice" means, so the ability is offered
  only while the ice encountered is the one this copy remembers, and never
  while it remembers nothing.
- ~~hosted credits usable for a DESCRIBED class of cost — "use these credits
  to trash installed cards" (Miss Bones). `hosted_credits_spendable` is
  all-or-nothing.~~ — **done, W21**: 1.10.3a says credits taken from a card
  ENTER the pool, so nothing about a hosted credit differs from any other and
  the restriction is on what a PAYMENT may be for. 1.10.3c is the sentence —
  "credits hosted on cards may only be spent as the card's ability allows" —
  so `PrintedCard::hosted_credits_spendable` carries what the card allows
  (`CreditUse::{AnyPayment, TrashingCards(criteria)}`) instead of a yes/no,
  and `Vm::CreditPurpose` is read off the payment's own continuation, which is
  where the kernel already recorded what the cost is being paid for. "Installed
  cards" needs no criterion: 1.15.2c already reads a description with no zone
  criterion that way.
- starting hand size: `Vm::new_game` draws 5 (1.6.6) with no hook, so an
  identity cannot change it (Andromeda).
- "while the Runner is accessing this ice in R&D, they must reveal it"
  (Archangel) and "you cannot steal or trash it during this access" (Pinhole
  Threading).

**DP-7b is 820/1420 (57.7%).** The remaining ~600 uncited rules are dominated
by §4.6 layout/orientation prose with no game effect, §1.5 setup, §2.16
subtypes, §3.x card-type prose, and the several dozen "one card, X, has the
ability…" rules the kernel has no card for yet — the corpus port is what will
move those. Raising it further is honest work — but only
where the kernel really implements the rule. Where a rule is realised
structurally rather than at one call site, W14j's convention is a doc-commented
**citation anchor** (`instr::movements`, `ability::ability_model`,
`ability::ability_source_model`, `Vm::ability_use_model`,
`View::play_area_information`), labelled as such so nobody mistakes it for an
implementation. To find candidates:

```
python3 - <<'EOF'
import json,re,os
d=json.load(open('docs/rules/cr-index.json'))
cited=set()
for f in os.listdir('crates/jinteki-cr/src'):
    if f.endswith('.rs'):
        cited |= set(re.findall(r'cite!\("([A-Za-z0-9_+]+)"\)', open('crates/jinteki-cr/src/'+f).read()))
for r in d['rules']:
    if r['id'] not in cited and not r.get('is_header'):
        print(r['number'], r['id'], '|', r['text'][:100])
EOF
```

## Discipline (unchanged, binding)

- Every CR example from `docs/rules/examples.json` lands as a test named
  after its example id, in `crates/jinteki-cr/tests/cr_examples.rs`.
- `cite!(rule_id)` on every mechanism; traceability green at all times.
- Full `cargo test --workspace` green before every commit.
- Stage ONLY `crates/jinteki-cr` (+ `Cargo.lock` if it moves, + this file);
  never `git add -A`. Other agents share the tree.
- Commit per coherent sub-wave; hand off by updating this file when context
  nears its end.
