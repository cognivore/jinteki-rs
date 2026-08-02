# jinteki-rs

A Rust rebuild of the [jinteki.net](https://jinteki.net) backend — rules engine, card system, lobby, wire protocol — **final tagless**, wire-compatible with the existing frontend, built so a hot mobile client can finally exist.

**Need:** make netrunner modern and attractive by enabling people to play games of netrunner faster.

## Why rebuild a backend that works?

Because the end goal is a mobile-friendly, animated, optionally fuse-timed client — and the current backend can't carry it: the diff stream says *what* changed but never *why* (animation needs why), there is no server-side clock at all (the lobby "timer" is a client-side decoration), and hidden-information redaction is enforced by discipline instead of types. Meanwhile every game of netrunner deserves to start faster, run faster, and be trusted more. So: new backend, same wire. The unmodified jinteki.net frontend keeps working against jinteki-rs the entire time — it is our conformance oracle, not our victim.

## The idea in one table

One card text, five meanings — `shuffle` under each interpreter:

| production | test | replay | legality | speculative (bots) |
|---|---|---|---|---|
| fresh CSPRNG permutation | seeded, `stack-deck`-able | recorded permutation, RNG doesn't exist | "order now unknown" constraint | resample from public knowledge — the truth is unreachable **by type** |

Narrow algebras (`Deck`, `Reveal`, `Prompt`, `Clock`, `Persist`, …), interpreters that implement only what they can honestly denote, and differential tests as the price of admission. Cards are DATA a game designer can write — no nerdy coding skills required, that's a hard requirement with a usability trial attached, not a vibe.

## Status

Specification stage. Read **[DESIGN.md](DESIGN.md)** — INCOSE-shaped: need → stakeholders → requirements → differential verification → mandated plan (P0–P4), with the wire-contract ICD generated from pinned reference commit `4054730` and the architecture mandates confined to the appendix where they belong. No code yet; the plan says protocol capture comes first and the plan is the boss.

## License

[WTFPL](LICENSE). Do what the fuck you want to. :D
