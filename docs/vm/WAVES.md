# CR-VM campaign state — wave ledger

Working state for the kernel campaign (DESIGN.md P1.5, DP-7a/b/c). Each
handoff updates this file; the successor agent reads it FIRST, then
ARCHITECTURE.md, then the code. Odometers are enforced by tests in
`crates/jinteki-cr/tests/` — this file is the narrative, the tests are the
truth.

## Odometers (after W8b)

- **DP-7a: 155/243** CR examples as example-named passing tests (63.8%).
- **DP-7b: 495/1420** distinct rules cited (34.9%); traceability test fails
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
| `pending` | W8b | **§8.8 swapping cards**: 8.8.2 destination legality (`Vm::swap_legal` / `may_occupy` — card type per location, and 4.6.6e/3.6.1 root limits with the vacating card discounted) applied BOTH as a gate on the swap and as a filter on the two 1.15.2 announcements a `SwapCards { Choose, Choose }` requires; 8.8.4b's mixed installed/uninstalled case (the leaver uninstalls and everything hosted on it is trashed, the joiner becomes installed in the exact position with no install procedure, entering unrezzed per 8.8.4a, and `Card{Un,}Installed` are recorded so the trigger conditions meet at the next checkpoint); `TriggerCond::SelfPassed`. Deviation 15 retired | 155 |

## Open deviations (documented in code; retire deliberately)

1. **Mid-checkpoint minimal-set resume** — 10.3.1e Decisions suspend and
   complete the trash on answer, single-pass; exact for single-set cases and
   the tested multi-set examples. The 10.3.1j candidacy Decision (W3a) uses
   the same single-pass pattern: steps (k)/(l) run before the answer, which
   is unobservable (they only clean counters). Revisit if an example demands
   true mid-procedure resume.
2. **9.12.1d/e dependency analysis** — covers the ability-removal dependency
   class (Hush/Magnet exact), not arbitrary predicates.
3. **The Cleaners dual modeling** — CR 9.9.7a example flow (triggered
   interrupt) AND `StaticDecl::DamageBonus`; both tested.
4. **Procedure-step surplus checkpoints** — traces (10.8.6), installs
   (8.5.16), and plays (8.6.7) expand into per-step instructions inside the
   ability loop, so each step gets a post-instruction checkpoint and
   interrupt-window point where the strict reading has only the explicitly
   called-for checkpoints (10.8.6b, 8.5.16d, 8.6.7b/e). Defended in-code as
   10.3.4/10.3.5 checkpoints; harmless surplus — revisit only if an example
   distinguishes them.
5. **9.8.2c order-declaration** for simultaneous multi-source subroutine
   grants not implemented (single-source grants only).
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
26. **`Object::generation` bumps on zone CLASS, and nothing reads history**
    (W7e/W7f) — 1.12.3's new object is a `(ObjectId, generation)` pair, with
    the whole play area one class so 1.12.4 moves keep the object. What is
    NOT built: 1.12.3's "moved to an unknown location" cases (a shuffle or a
    rearrangement makes new objects — `object_move_location_1/2`), and
    1.12.6's game-history queries (`previous_object_1`,
    `previous_object_source_1`), which want the change log indexed by
    object rather than scanned.
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

## Next targets — 98 examples left, re-measured after W7

Re-run the count before choosing a cluster:

```
python3 - <<'EOF'
import json,re
v=json.load(open('docs/rules/examples.json'))
src=open('crates/jinteki-cr/tests/cr_examples.rs').read()
i=src.index('const IMPLEMENTED'); j=src.index('];', i)
impl=set(re.findall(r'"(example_[a-z0-9_+]+)"', src[i:j]))
missing=[e['id'] for e in v['examples'] if e['id'] not in impl]
print(len(missing)); [print(m) for m in missing]
EOF
```

Remaining by section: 1.16 Costs 8 · 6.2 Position 8 · 9.12 Other 7 ·
9.1 General 6 · 1.12 Objects 5 · 4.6 Play Area 4 · 7.3 Breaching 4 ·
6.1/6.5/8.8/9.8/9.9/9.10/9.11/10.1 three each · rest ≤ 2.

1. **§6.2 positions + §8.8 swaps (~11)** — the biggest single cluster and
   the one with a clear design. `count_positions_1/2`,
   `no_position_after_approach_server_1`, `ice_change_outward_1`,
   `_inward_1`, `_encounter_move_swap_1`, `_during_movement_1/2`,
   `swap_become_installed_1`, `swap_installed_cards_preserves_hosting_1`,
   `swap_only_to_valid_location_1`, plus `drawn_card_swapped_1`.
   **6.2.6 is explicit that the Runner's position is "a specific element of
   the sequence of positions, not an index into that sequence"**, and
   `RunCtx.position` is an index today — so this cluster is a position-id
   refactor: give each server a list of position ids alongside its ice list
   (6.2.2 creates a position, 6.2.4 destroys it, 6.2.2f swaps do neither),
   and make `RunCtx.position: Option<u64>`. 6.2.3's "same position" then
   reads as "same number of positions inward", which is what
   `TargetFilter::IceAtPosition { n }` (Rook) wants. §8.8 rides along and
   retires deviation 15.
2. **§1.16 costs (~8)** — `cost_x_1` (X chosen at announce, 1.16.2c),
   `alternate_payment_1` (1.16.2e), `cost_quantities_1`,
   `cost_restrictions_2`, `inherent_cost_aggregates_1`,
   `additional_cost_checkpoint_1`, `install_and_rez_reducing_total_1`,
   `cost_interrupt_static_mandatory_1`. X-costs and alternate payments are
   the two new mechanisms; note deviations 11 and 18 both say the same
   thing — `pay_cost` is synchronous and cannot suspend for a Decision.
   Taking this cluster probably means making cost payment a phase of the
   ability frame, which would retire 11, 18 and part of 8 at once.
3. **§1.12's remaining 5 + §4.6 "this server" (~9)** — `previous_object_1`,
   `previous_object_source_1`, `object_move_location_1/2`,
   `object_move_known_location_1`, `this_server_1/2/3`,
   `limit_remote_servers_1`. `Object::generation` (W7e/f) is the stamp;
   what is missing is (a) unknown-location moves bumping it (shuffle,
   rearrange) and (b) a game-history query keyed by object, which 1.12.6
   needs and which `previous_object_source_1` shares with 4.6.6i's "this
   server" (the server a card was in when it left). `MaintainedChoice`
   exists but nothing creates one — `object_move_known_location_1` and the
   three `lingering_effect_maintaining_choice_*` examples all want
   `Instruction::MaintainChoice`, which is one more instruction.
4. **Forced encounters outside a run (~5)** — `forced_encounter_1`,
   `forced_encounter_during_run_1`, `end_encounter_outside_run_1`,
   `active_exception_encounter_not_installed_1`,
   `no_position_after_approach_server_1`, plus
   `ice_strength_modification_duration_1` (W6b built the duration half and
   could not test it for want of a standalone encounter — the union-of-
   durations model already gives the right answer). `EncounterState` is
   already a `Vm` field rather than a run-frame field and its doc comment
   already says encounters can exist without a run; what is missing is an
   encounter TIMING STRUCTURE (§11) to push, and `Instruction::
   ForceEncounter { ice }`.
5. **§9.12.1's dependency examples (~4)** — `independent_effects_1/2`
   (Mother Goddess/Warden Fatuma/Hush; Hush/Magnet), `modify_ability_
   with_choice_1`, `calculated_quantity_3`. Deviation 2 says the dependency
   analysis covers the ability-removal class only; `independent_effects_1`
   is the subtype-grant class, and W7d just added the subtype ops the
   pipeline needs, so this got cheaper.
6. **§9.11's last three** — `step_sequences_1` contradicts deviation 4 head
   on (it says the ONLY checkpoint while installing is 8.5.16d), so taking
   it means retiring that deviation; `use_restrictions_1` wants a
   payment restriction ("only by spending credits from a stealth card");
   `look_reveal_instruction_1` wants `Instruction::LookAt { zone, count }`
   — with W7c's `TargetFilter::TopOfDeckOf` the second instruction is
   already expressible.

Cheap singletons worth taking opportunistically: `dividends_1` +
`dividends_timing_1` (2, one keyword — `PrintedCard.dividends` plus a
score-time snapshot, since 10.13.2 reads the values as the agenda began to
be scored); `bluffing_1` + `cannot_hide_open_info_1` (§10.2 information —
the kernel has no per-side visibility model at all, see deviation 20);
`multiple_damage_selected_sequentially_1`; `mandatory_infinite_loop_1`
(hard — 10.1.6a draws the game).

**Blocked / deferred, with reasons:**
- `target_3` (Trick of Light, 1.15.1) — the targets are ADVANCEMENT
  COUNTERS. Counters are a `BTreeMap<CounterKind, u32>` on the object, not
  `ObjectId`-addressed objects, so they cannot be announced. Deferred to
  the §1.12 cluster, which is where counters-as-objects belongs (1.12.1
  says counters are objects).
- `candidates_leaving_server_1` was blocked on object identity and is now
  DONE (W7e) — the generation stamp was what it needed.

## Discipline (unchanged, binding)

- Every CR example from `docs/rules/examples.json` lands as a test named
  after its example id, in `crates/jinteki-cr/tests/cr_examples.rs`.
- `cite!(rule_id)` on every mechanism; traceability green at all times.
- Full `cargo test --workspace` green before every commit.
- Stage ONLY `crates/jinteki-cr` (+ `Cargo.lock` if it moves, + this file);
  never `git add -A`. Other agents share the tree.
- Commit per coherent sub-wave; hand off by updating this file when context
  nears its end.
