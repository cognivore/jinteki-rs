# Card Coverage

**GENERATED FILE — do not edit.** Regenerate with `python3 tools/gen-carddata.py`.

## Totals

- Cards in database: **2065**
- Implemented by the reference (jinteki.net defcard exists): **2054**
- Reference implementations flagged partial (`:implementation` caveat): **56**
- Not implemented anywhere — the ISOLATED set needing fresh behavior work: **11**
- Rust behavior overlay (carddb.rs): **43**

### Isolated titles (no implementation even in jinteki.net)

- Agenda Points (Rules Insert, Rules Insert, Rules)
- Charge (Rules Insert, Rules Insert, Rules)
- Core Damage (Rules Insert, Rules Insert, Rules)
- Corp Turn (Rules Insert, Rules Insert, Rules)
- Custom Biotics: Engineered for Success (Corp, Identity, Creation and Control)
- Cyber Bureau: Keeping the Peace (Corp, Identity, NAPD Multiplayer)
- Making a Run (Rules Insert, Rules Insert, Rules)
- Mark (Rules Insert, Rules Insert, Rules)
- Runner Turn (Rules Insert, Rules Insert, Rules)
- Sabotage (Rules Insert, Rules Insert, Rules)
- Threat (Rules Insert, Rules Insert, Rules)

### Reference defcard titles absent from the card data

jinteki.net pseudo-cards (basic actions and similar); they are engine-internal on our side too:

- Corp Basic Action Card
- Runner Basic Action Card

## Per-cycle coverage

| Cycle | Cards | jnet impl | rs behavior |
|---|---:|---:|---:|
| Draft (`draft`) | 9 | 9 | 0 |
| Core (`core`) | 32 | 32 | 0 |
| Genesis (`genesis`) | 69 | 69 | 1 |
| Creation and Control (`creation-and-control`) | 46 | 45 | 0 |
| Spin (`spin`) | 90 | 90 | 0 |
| Honor and Profit (`honor-and-profit`) | 50 | 50 | 1 |
| Lunar (`lunar`) | 102 | 102 | 0 |
| Order and Chaos (`order-and-chaos`) | 55 | 55 | 0 |
| SanSan (`sansan`) | 109 | 109 | 0 |
| Data and Destiny (`data-and-destiny`) | 54 | 54 | 0 |
| Mumbad (`mumbad`) | 111 | 111 | 1 |
| Flashpoint (`flashpoint`) | 118 | 118 | 0 |
| Red Sand (`red-sand`) | 120 | 120 | 0 |
| Terminal Directive (`terminal-directive`) | 71 | 71 | 0 |
| Unreleased (`unreleased`) | 2 | 2 | 0 |
| Revised Core (`revised-core`) | 39 | 39 | 1 |
| Kitara (`kitara`) | 120 | 120 | 0 |
| Reign and Reverie (`reign-and-reverie`) | 56 | 56 | 0 |
| Magnum Opus (`magnum-opus`) | 2 | 2 | 0 |
| NAPD Multiplayer (`napd-multiplayer`) | 1 | 0 | 0 |
| System Core 2019 (`system-core-2019`) | 84 | 84 | 16 |
| Ashes (`ashes`) | 130 | 130 | 1 |
| Magnum Opus Reprint (`magnum-opus-reprint`) | 6 | 6 | 0 |
| Salvaged Memories (`salvaged-memories`) | 15 | 15 | 0 |
| System Gateway (`system-gateway`) | 77 | 77 | 7 |
| System Update 2021 (`system-update-2021`) | 82 | 82 | 15 |
| Borealis (`borealis`) | 128 | 128 | 0 |
| Liberation (`liberation`) | 130 | 130 | 0 |
| Elevation (`elevation`) | 82 | 82 | 0 |
| Vantage Point (`vantage-point`) | 66 | 66 | 0 |
| Rules (`rules`) | 9 | 0 | 0 |

## How the pipeline works

`tools/raw_data.edn` (official card data, vendored byte-for-byte from [netrunner-data](https://github.com/NoahTheDuke/netrunner-data) `edn/raw_data.edn` at the commit pinned in `tools/raw_data.edn.lock`; actualise/verify/re-fetch it with `rust-script tools/fetch-carddata.rs [verify|pinned]` — no argument moves the pin to the latest upstream commit) is parsed by `tools/gen-carddata.py` (a small tolerant EDN reader), which emits:

- `crates/jinteki-core/carddata/cards.json` — printed data for every card (deduped by title; the printing with the highest numeric code wins). Double-sided cards carry a `faces` key with each back face's title and text, copied from a local clone of NSG's [netrunner-cards-json](https://github.com/NullSignalGames/netrunner-cards-json) (`v2/cards/*.json`, commit `51e7c6d99838ca1197f27ad9f7a36d522b8204a8`) — the EDN strips that text, keeping only card-id pointers;
- `crates/jinteki-core/carddata/coverage.json` — per-title flags: does a reference `(defcard "Title" ...)` exist (`jnet_impl`), does it carry an `:implementation` caveat (`jnet_partial`), and does the Rust behavior table cover it (`rs_behavior`);
- this document.

`crates/jinteki-core/src/printed.rs` embeds both JSON files via `include_str!` and exposes `printed(title)`, `all_printed()`, and `impl_status(title)` (`Behavior` / `JnetOnly` / `Unimplemented`).

The behavior overlay stays in `crates/jinteki-core/src/carddb.rs`: a card with a `CardDef` row there plays with full rules. Every other card known to `printed()` spawns as a **vanilla** runtime definition — correct printed stats, no behavior hooks: operations and events resolve with only their cost paid (the log notes "(no implemented effect)"), ice has zero subroutines, assets/upgrades/hardware/resources install and sit there. Unknown titles are a clean error instead of a panic.

Regenerate everything with: `python3 tools/gen-carddata.py`
