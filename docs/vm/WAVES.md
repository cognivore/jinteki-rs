# CR-VM campaign state — wave ledger

Working state for the kernel campaign (DESIGN.md P1.5, DP-7a/b/c). Each
handoff updates this file; the successor agent reads it FIRST, then
ARCHITECTURE.md, then the code. Odometers are enforced by tests in
`crates/jinteki-cr/tests/` — this file is the narrative, the tests are the
truth.

## Odometers (after W2)

- **DP-7a: 52/243** CR examples as example-named passing tests (21.4%)
- **DP-7b: 326/1420** distinct rules cited (23.0%); traceability test fails
  on any cited id absent from `docs/rules/cr-index.json`
- Full workspace: 16 suites green; jinteki-core/-server untouched by VM work

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

## Open deviations (documented in code; retire deliberately)

1. **Mid-checkpoint minimal-set resume** — 10.3.1e Decisions suspend and
   complete the trash on answer, single-pass; exact for single-set cases and
   the tested multi-set examples. Revisit if an example demands true
   mid-procedure resume.
2. **10.3.1j auto-candidate declaration** — mid-breach root entries
   auto-declared; the real Runner-declaration Decision needs mid-breach
   installs (§8.5 work unblocks this — do it then).
3. **9.12.1d/e dependency analysis** — covers the ability-removal dependency
   class (Hush/Magnet exact), not arbitrary predicates.
4. **The Cleaners dual modeling** — CR 9.9.7a example flow (triggered
   interrupt) AND `StaticDecl::DamageBonus`; both tested.
5. **Trace-internal checkpoints** — steps (c)/(d)/(e) of 10.8.6 each get a
   post-instruction checkpoint, defended in-code as 10.3.4 cost-paid
   checkpoints; the strict reading has only 10.8.6b. Harmless surplus
   checkpoints; revisit only if an example distinguishes them.
6. **9.8.2c order-declaration** for simultaneous multi-source subroutine
   grants not implemented (single-source grants only).

Retired: W1's "persistent-ability expiry plumbed but unarmed" (W2b armed it).

## W3 recommended targets (from W2 close-out, leverage-ordered)

1. **Install/play instructions §8.5/8.6** — unblocks the LARGEST remaining
   example cluster (9.6.5b, No One Home, mid-breach installs, and retires
   deviation (2) properly via 10.3.1j).
2. Replacement-ordering pair 9.9.11a — needs an order Decision at
   imminence-open; plumbing sketched in W2 in-code notes (grep `9.9.11`).
3. Vacuous truth 9.12.2d (fully-break tracking).
4. Run-ends conditions 6.8.5 (prevention-shield lingering).
5. Candidate examples 7.4.3/7.4.7 (access-replacement effects).

## Discipline (unchanged, binding)

- Every CR example from `docs/rules/examples.json` lands as a test named
  after its example id, in `crates/jinteki-cr/tests/cr_examples.rs`.
- `cite!(rule_id)` on every mechanism; traceability green at all times.
- Full `cargo test --workspace` green before every commit.
- Stage ONLY `crates/jinteki-cr` (+ `Cargo.lock` if it moves, + this file);
  never `git add -A`. Other agents share the tree.
- Commit per coherent sub-wave; hand off by updating this file when context
  nears its end.
