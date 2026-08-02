# jinteki-rs

A Rust rebuild of the [jinteki.net](https://jinteki.net) backend — rules engine, card system, lobby, wire protocol — **final tagless**, wire-compatible with the existing frontend, built so a hot mobile client can finally exist.

**Need:** make netrunner modern and attractive by enabling people to play games of netrunner faster.

## Play it now

```sh
nix develop --command cargo run -p jinteki-server
# open http://localhost:7787 — phone on the same LAN works too
```

- **Play vs Bot** — the jinteki-rs engine with a random-walk opponent. 28-card
  pool (Weyland BABW vs The Catalyst), full runs/ice/breakers/access, mobile
  UI schooled by MTG Arena and Hearthstone mobile ([docs/UX.md](docs/UX.md)).
- **Reference Server** — connect the SAME UI to a real jinteki.net-protocol
  server through the built-in bridge (sente + msgpack + differ, implemented
  from source) and test parity by playing: [PARITY.md](PARITY.md).

Everything builds through `nix develop` — the flake is the only blessed
toolchain. Tests: `nix develop --command cargo test` (30 per-card behavior
tests mirroring the reference corpus, self-play fuzz across 120 seeded
bot-vs-bot games with zone audits, msgpack/differ codec tests).

## Why rebuild a backend that works?

Because the end goal is a mobile-friendly, animated, optionally fuse-timed client — and the current backend can't carry it: the diff stream says *what* changed but never *why* (animation needs why), there is no server-side clock at all (the lobby "timer" is a client-side decoration), and hidden-information redaction is enforced by discipline instead of types. Meanwhile every game of netrunner deserves to start faster, run faster, and be trusted more. So: new backend, same wire. The unmodified jinteki.net frontend keeps working against jinteki-rs the entire time — it is our conformance oracle, not our victim.

## The idea in one table

One card text, five meanings — `shuffle` under each interpreter:

| production | test | replay | legality | speculative (bots) |
|---|---|---|---|---|
| fresh CSPRNG permutation | seeded, `stack-deck`-able | recorded permutation, RNG doesn't exist | "order now unknown" constraint | resample from public knowledge — the truth is unreachable **by type** |

Narrow algebras (`Deck`, `Reveal`, `Prompt`, `Clock`, `Persist`, …), interpreters that implement only what they can honestly denote, and differential tests as the price of admission. Cards are DATA a game designer can write — no nerdy coding skills required, that's a hard requirement with a usability trial attached, not a vibe.

## Repo map

- **[DESIGN.md](DESIGN.md)** — the INCOSE-shaped specification: need →
  requirements → differential verification → mandated plan P0–P4, with the
  wire ICD pinned to reference commit `4054730` and architecture mandates in
  the appendix. The plan is the boss.
- `crates/jinteki-core` — sans-IO engine: seeded state, jnet-shaped
  commands/prompts, run state machine, legality enumerator, random-walk bot,
  declarative card table (printed stats verified against netrunner-data).
- `crates/jinteki-server` — axum server: local games, static UI, and the
  reference bridge.
- `ui/` — dependency-free mobile web client, one renderer for both backends.
- `PARITY.md`, `docs/UX.md` — how to test parity by playing; which MTGA/HS
  mobile lessons the UI assimilates.

## Status

Playable milestone (DESIGN.md P0/P1 slice): complete games vs the bot in the
browser, per-card tests green, self-play fuzz green, bridge implemented
against extracted sente/differ internals. Next per the plan: golden
transcripts, full compat event catalog, engine algebra split, card pool
growth.

## License

[WTFPL](LICENSE). Do what the fuck you want to. :D
