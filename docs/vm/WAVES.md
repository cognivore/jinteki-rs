# CR-VM campaign state — wave ledger

Working state for the kernel campaign (DESIGN.md P1.5, DP-7a/b/c). Each
handoff updates this file; the successor agent reads it FIRST, then
ARCHITECTURE.md, then the code. Odometers are enforced by tests in
`crates/jinteki-cr/tests/` — this file is the narrative, the tests are the
truth.

## Odometers (during W4)

- **DP-7a: 82/243** CR examples as example-named passing tests (33.7%).
  **FROZEN by the user for the duration of the W4 plan-driver migration**:
  no new example tests until every existing test is a plan. Same tests,
  better bodies.
- **DP-7b: 384/1420** distinct rules cited (27.0%); traceability test fails
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
  file before building or it is invisible to the sandbox.

## Stage: FT-0 (the algebra cut, `docs/vm/FINAL-TAGLESS.md`)

FINAL-TAGLESS.md (commit `1e51327`) is NORMATIVE for `crates/jinteki-cr` and
ARCHITECTURE §12's concrete-first stance is retracted in its favour. The
staged migration is FT-0 → FT-1 → FT-2 → FT-3 → FT-4 → FT-5, strictly in
order, odometer never regressing.

**W4 IS FT-0**: the plan-driver harness makes the scripted plan the second
honest interpreter of the `Decide` algebra (the bot is the third; the
server/human driver joins at cutover). FT-0's exit gate: 82/243 green, no
`vm.step()` loops anywhere in tests, no `tk::inject_*` state manufacture.
Do NOT resume adding CR examples until FT-2 is complete — FT-1/FT-2 are
mechanical and cheap at 82 examples, ruinous at 243.

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
9. **Ob-class search elision** — 8.5.13c's example is tested with a fixed
   R&D card; §8.7 search/find/shuffle is not yet implemented.

Retired: W1's "persistent-ability expiry plumbed but unarmed" (W2b armed
it); W2's "10.3.1j auto-candidate declaration" (W3a implemented the real
Runner declaration Decision with 7.4.6a declined-tracking).

## W4 recommended targets (from W3 close-out, leverage-ordered)

1. **§8.7 search/find/shuffle** — unblocks the search example cluster
   (8.7.2b Artist Colony/SMC/Tucana, 8.7.3 Tech Startup shuffle-before-use,
   8.7.5 search-condition pend timing) and retires deviation (9). The
   InstallCard machinery is ready to consume found cards.
2. **9.12.2b calculated_quantity_3** — realloc()/NASX: a "for each" whose
   effects are NOT all aggregated classes resolves as separate instances
   (needs a derez primitive + per-occurrence change groups; NASX-class
   credits-gained triggers).
3. **§8.8 swaps** — Metamorph/Thimblerig/A Teia/Tatu-Bola examples
   (position-preserving moves, hosted-relationship maintenance, 8.8.4b
   install/uninstall condition timing).
4. **Chain/independence residue** — 9.1.2a example 2 (Zahya/Direct Access
   ability-removed-during-resolution), 9.1.4 Compile/Mayfly stranding
   (source_moved_since is still a stub — see vm.rs).
5. **Encounter/bypass residue** — 6.5.8c bypass-after-subs-broken states,
   8.5.10 mid-encounter ice uninstall (currently position arithmetic only).

## Discipline (unchanged, binding)

- Every CR example from `docs/rules/examples.json` lands as a test named
  after its example id, in `crates/jinteki-cr/tests/cr_examples.rs`.
- `cite!(rule_id)` on every mechanism; traceability green at all times.
- Full `cargo test --workspace` green before every commit.
- Stage ONLY `crates/jinteki-cr` (+ `Cargo.lock` if it moves, + this file);
  never `git add -A`. Other agents share the tree.
- Commit per coherent sub-wave; hand off by updating this file when context
  nears its end.
