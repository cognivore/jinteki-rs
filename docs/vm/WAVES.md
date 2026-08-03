# CR-VM campaign state — wave ledger

Working state for the kernel campaign (DESIGN.md P1.5, DP-7a/b/c). Each
handoff updates this file; the successor agent reads it FIRST, then
ARCHITECTURE.md, then the code. Odometers are enforced by tests in
`crates/jinteki-cr/tests/` — this file is the narrative, the tests are the
truth.

## Odometers (after W12f)

- **DP-7a: 217/243** CR examples as example-named passing tests (89.3%).
- **DP-7b: 573/1420** distinct rules cited (40.4%); traceability test fails
  on any cited id absent from `docs/rules/cr-index.json`
- Full workspace: 16 suites green; jinteki-core/-server untouched by VM work
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
2. **The odometer, wave after wave, to 243/243.** This is the deliverable.
   Resume the queued clusters below and keep going through every remaining
   example in `docs/rules/examples.json`, one commit per coherent sub-wave.
3. DP-7c (jinteki-reference corpus port, triaged against the CR), then the
   two decks (estrike Andromeda, Gauntlet NTM) from printed oracle text.
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
4. **Procedure-step surplus checkpoints** — traces (10.8.6), installs
   (8.5.16), and plays (8.6.7) expand into per-step instructions inside the
   ability loop, so each step gets a post-instruction checkpoint and
   interrupt-window point where the strict reading has only the explicitly
   called-for checkpoints (10.8.6b, 8.5.16d, 8.6.7b/e). Defended in-code as
   10.3.4/10.3.5 checkpoints; harmless surplus — revisit only if an example
   distinguishes them.
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

18. **The division of a credit payment is not put to the player** (W6a,
    `Vm::spend_flexible`) — 1.10.3c says a player spending credits "chooses
    how to divide the credits they are spending from among the allowed
    locations"; the kernel spends the credit pool first and then hosted
    credits in object order. Every tested case is forced (an empty pool, or
    hosted credits that are the only way to pay), so no example distinguishes
    them; a real choice means suspending payment, which `pay_cost` cannot do
    (see deviation 11).

19. **The 10.12.3a sabotage floor is completed, not refused** (W6c,
    `DecisionCtx::Sabotage`) — the Decision carries `min`, and a Corp answer
    below it is topped up from the front of HQ instead of being rejected as
    illegal. The kernel has no "your answer was illegal, choose again" path
    anywhere; every other Decision clamps the same way.
20. **`Instruction::Sabotage` trashes without a Decision per card** (W6c) —
    10.12.2b ("the Corp cannot look at cards trashed from R&D until after
    making all decisions") is satisfied trivially because the R&D cards are
    never shown to the answering side; the kernel has no per-side visibility
    model to violate yet. §10.2 information rules are their own wave.

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
14. **1.13.13 counter-trashing is one checkpoint early** (W5b, an instance of
    deviation 4) — `example_rule_trash_hosted_objects_when_host_trashed_2`
    says the hosted agenda counter goes "after step 8.5.16d"; the kernel
    gives every install step its own checkpoint, and the card leaves the
    score area at 8.5.16a, so the counter goes after (a). The observable
    claim — gone during the installation, before the card becomes installed
    at 8.5.16f, unpreventable — holds and is what the test asserts.
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
17. **No basic install or play actions** (5.2.6d/5.2.7a/d) — the action
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
26. **`Object::generation` bumps on zone CLASS *and* on unknown-location
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
29. **8.8.4a is applied only where a card becomes (un)installed** (W8b) —
    "each of the swapped cards enters its destination zone in the same state
    that a card would normally enter that zone" is implemented for the 8.8.4b
    case (a Corp card entering the play area enters unrezzed). A swap between
    two hidden zones does not re-derive faceup status, and the "facedown into
    Archives unless it was visible" clause needs the per-side visibility model
    §10.2 does not have yet (see deviation 20).
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
36. **1.18.3's "you can advance" is not modeled** (W9b) — `AdvanceCard`
    advances whatever it names. The basic advance action (5.2.6f) is still
    missing too (deviation 17), so nothing yet needs the "can be advanced"
    permission that 9.1.8f keeps active on unrezzed cards.
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

44. **The forfeit cost's choice is not put to the payer** (W11a,
    `Cost::forfeit_agenda`, annotated on the field) — deviation 11's class:
    `pay_cost` is synchronous everywhere, so the front of the score area is
    taken. No example distinguishes them; the 9.6.14d example makes its real
    choice afterwards, in the 1.15.2 announcement of whose ability to
    resolve, which is why its test puts THREE agendas in the score area.
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

## Next targets — 26 examples left, re-measured after W12f

Re-run the count before choosing a cluster:

```
python3 - <<'EOF'
import json,re
v=json.load(open('docs/rules/examples.json'))
src=open('crates/jinteki-cr/tests/cr_examples.rs').read()
i=src.index('const IMPLEMENTED'); j=src.index('];', i)
impl=set(re.findall(r'"(example_[a-z0-9_+]+)"', src[i:j]))
missing=[(e['section_number'], e['id']) for e in v['examples'] if e['id'] not in impl]
from collections import Counter
print(len(missing)); [print(s, n) for s, n in Counter(s for s,_ in missing).most_common()]
[print(s, i) for s, i in sorted(missing)]
EOF
```

Remaining by section (26 after W12f): 1.16 Costs 5 · 6.7 and 10.2 two each ·
rest 1.

CLUSTER RANKING (measured after W12f, best first):

1. **§1.16's last 5 — the biggest live cluster, and four of them share ONE
   missing primitive: COST PAYMENT AS A PHASE OF THE ABILITY FRAME.**
   `Vm::pay_cost` is synchronous everywhere, which is what deviations 11, 18
   and 44 are all instances of. Give `AbilityPhase` a `PayCost`-style
   suspension that can `ask` and resume (the shape `Instruction::Damage`
   acquired in W12e is the template — `ask` in `apply_imminent`, finish in
   `answer`) and the following fall out at once:
   - `alternate_payment_1` (1.16.2e Biawak) — the payment DIVISION put to the
     payer, retiring deviations 11/18/44 together.
   - `cost_x_1` (1.16.2c) — X announced before payment, a numeric Decision
     like W8c's `DivideCostReduction`.
   - `cost_restrictions_2` (1.16.1c) — an additional cost to SCORE; the
     advancement-requirement machinery is W9b's and already there, and
     `StaticDecl::AdditionalStealCost` is the shape to copy.
   - `additional_cost_checkpoint_1` (1.16.10c).
   `inherent_cost_aggregates_1` (1.16.4d) additionally wants the basic play
   action (deviation 17).
2. **§6.7's two — `if_successful` as a real property of the initiating
   effect.** `if_successful_tied_to_server_1` (6.7.4a) needs
   `Instruction::InitiateRun` to carry the SET of servers the initiating
   effect allowed, so that 6.1.2d moving the run to another server inside
   that set keeps the "if successful" clause and moving it outside drops it;
   `if_successful_ability_optional_1` (6.7.4c) is the same instruction
   growing an optional clause the Runner may resolve after the 6.9.5a window
   has played out. Together they are one field plus one Decision.
3. **The rest of §6, one each.** `run_ends_other_priority_windows_1` (6.8.2c
   — Formicary: a window completed normally except that no new timing
   structure may be initiated: a flag on `WindowFrame` consulted by
   `push_encounter`/`initiate_run`); `abilities_during_a_run_1` (6.3.4 —
   wants an additional cost on the basic run action); `run_phase_after_1`
   (6.1.3e, hardest — the phase-after query).
4. **§9.8's last one.** `replace_subroutine_resolution_1` (9.8.9) — a
   replacement effect applying at `AbilityPhase::SubImminent` (which exists
   as a no-op placeholder) that swaps WHICH subroutine resolves, plus a
   Persephone-class "the Runner passes an ice whose subroutines resolved"
   trigger. `GameChange::SubroutineResolved { ice, .. }` is already recorded
   with the ICE before the frame push, which is exactly the "count as
   resolving from Bloop" the rule asserts, so the trigger is a scan over the
   encounter's window.
5. **Two singletons riding machinery that now exists.**
   - `replacement_effect_only_applies_once_per_effect_1` (9.9.9c, Project
     Vacheron) — deviation 7 already gives once-per-effect; what is missing
     is a `ReplacementTransform` arm that adds the agenda to the score area
     with hosted counters.
   - `facedown_set_aside_distinct_groups_1` (4.8.5) — W12b's
     `Object::set_aside_from` is the hook; what it needs is set-aside GROUPS
     that stay distinct.
6. **`must_cannot_force_additional_cost_1` (9.12.3e)** — deviation 41: a
   "must" over a decision other than the mid-access window's pass. Needs the
   basic run action to carry an additional cost, so it pairs with
   `abilities_during_a_run_1`.

Hard/blocked, with reasons (unchanged unless noted):
- `target_3` (Trick of Light, 1.15.1) — the targets are ADVANCEMENT COUNTERS.
  Counters are a `BTreeMap<CounterKind, u32>` on the object, not
  `ObjectId`-addressed objects, so they cannot be announced. This is 1.12.1's
  counters-as-objects and it is now the ONLY §1.12 work left; W12f cleared
  the rest of that section.
- `bluffing_1`, `cannot_hide_open_info_1`, `arrange_and_other_effect_1`,
  `visibility_after_access_1` — §10.2 information: the kernel has NO per-side
  visibility model (deviations 20, 29). One wave of its own; it unlocks four.
- `mandatory_infinite_loop_1` (10.1.6a draws the game).
- `step_sequences_1` (9.11.2a — asserts installing has exactly ONE checkpoint,
  which deviation 4 says the kernel does not honour; take it WITH the
  deviation-4 fix or not at all).
- `object_move_location_1` (1.12.3) — see deviation 26: the looked-at set has
  to live on the ability frame before a shuffle can strand it.
- `drawn_card_swapped_1` (8.4.3b) — 8.8.4d (swapping with a SET-ASIDE card)
  plus the §8.4 drawing procedure's set-aside step. Take it with §8.4.
- `defferent_actions_1` (5.2.5b) and `inherent_cost_aggregates_1` (1.16.4d)
  both want the basic play/install actions (deviation 17).
- `active_exception_catchall_1` (9.1.8b) — I've Had Worse: needs damage-trash
  marking, an ability active IN THE HEAP because its condition can only ever
  be met there, AND a Skorpios-class replacement of the trash movement with a
  removal from the game. Take it with §8.2.2's movement replacements
  (`sec_replacing_movements_1`), which also needs 8.1.4's facedown installed
  Runner cards (a facedown Runner card is blank and inactive — `card_active`
  currently ignores `faceup` for Runner cards).

## Discipline (unchanged, binding)

- Every CR example from `docs/rules/examples.json` lands as a test named
  after its example id, in `crates/jinteki-cr/tests/cr_examples.rs`.
- `cite!(rule_id)` on every mechanism; traceability green at all times.
- Full `cargo test --workspace` green before every commit.
- Stage ONLY `crates/jinteki-cr` (+ `Cargo.lock` if it moves, + this file);
  never `git add -A`. Other agents share the tree.
- Commit per coherent sub-wave; hand off by updating this file when context
  nears its end.
