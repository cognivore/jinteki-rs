# CAMPAIGN.md — the full-fidelity port of every card

Mandate: ALL cards ported COMPLETELY and FAITHFULLY to the reference
implementation. No vanilla fallback in real games — the local server refuses
decks containing unimplemented cards, loudly, by title. The fallback plumbing
exists solely so the browser/deckbuilder can show any card and so coverage is
measurable; it is not a way to play.

## Fidelity bar (non-negotiable, per card)

1. Behavior mirrors the reference defcard at the pinned commit (`DESIGN.md`
   Appendix B pin), including its jank, unless the divergence ledger in
   PARITY.md says otherwise with a reason.
2. Every card with a reference test gets that test ported (same assertions,
   our harness vocabulary). Cards without reference tests get at least one
   behavior test written from the card text + defcard.
3. No `unreachable!()`, no silent no-ops. If the IR can't express a card,
   grow the IR (preferred) or use a registered Rust escape hatch (tracked,
   budgeted per DESIGN.md SYS-D-4).
4. `python3 tools/gen-carddata.py` rerun after each wave so
   `docs/CARD-COVERAGE.md` counts stay truthful. The rs-behavior count is the
   campaign's public odometer.

## Wave structure

- **W0 — mechanics pack 1 (enabler):** tags (incl. corp trash-resource
  action), traces (base/boost/link), psi games, expose, on-access/ambush,
  on-rez hooks, generic counters (power/virus/agenda + purge), and the
  generalization of hardcoded hooks into an event-driven ability IR
  (trigger × effect-sequence). Migrate the existing 28 behaviors onto the IR;
  prove each new mechanic with 12–18 reference-faithful cards + ported tests.
- **W1–W4 — PRIORITY DECKS (user-directed):** the four netrunnerdb meta
  decks in `tools/priority-decks.json`, one per wave, in order:
  W1 "estrike Regular Andromeda" (Andromeda), W2 "Gauntlet" (Nebula Talent
  Management), W3 "Wack" (Valencia), W4 "post flood asa" (Asa Group).
  A wave is DONE when its whole deck — identity included — loads under
  strict mode and plays: every card natively implemented at the fidelity
  bar, tests ported. These decks force currents, heap breakers, damage
  prevention, hosting, recurring credits, upgrades + access rules,
  encounter events, bioroids, and variable subs — the engine grows its real
  vocabulary here. On completion, register each deck as a selectable
  decklist in the local UI.
- **W5..Wn — remaining pool:** ~30 cards per wave, one reference file at a
  time in file order (agendas → assets → ice → operations → upgrades →
  events → hardware → programs → resources → identities). Each wave: extend
  IR only as its cards demand; port tests; regenerate coverage; commit.
- **Mechanics packs interleave** when a batch hits a wall (hosting, currents,
  MU-modifying programs, damage prevention, replacement effects, ...).

Waves run as dedicated background agents, sequential (they share engine
files). Every wave commits on green; `docs/CARD-COVERAGE.md` is regenerated
in the same commit. The campaign is resumable from any point: read this
file, read the coverage report, take the next unported block.

## Ground rules for wave agents

- Build/test ONLY via `nix develop --command cargo ...`.
- Touch only `crates/jinteki-core`, `tools/`, `docs/CARD-COVERAGE.md`.
- All existing tests stay green — the 30 pool tests, self-play fuzz,
  printed-db tests, server tests.
- The self-play fuzz must keep terminating: new mechanics that add prompts
  must be enumerable (`enumerate_actions` covers every new decision point).
- Reference sources: defcards in
  `../jinteki-reference/src/clj/game/cards/*.clj`, engine semantics in
  `../jinteki-reference/src/clj/game/core/`, tests in
  `../jinteki-reference/test/clj/game/cards/*_test.clj`.

## Status

See `docs/CARD-COVERAGE.md` (generated) for live counts. Waves completed are
recorded in git history (`feat(cards): W<n> ...` commits).
