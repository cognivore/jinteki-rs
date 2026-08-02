# PARITY.md — testing parity against the reference server, by playing

The same UI drives two backends through one JSON envelope:

- **Local mode** (`/ws/local`): the jinteki-rs engine, bot opponent,
  legality-enumerator affordances.
- **Bridge mode** (`/ws/bridge`): a real jinteki.net-protocol server. The
  bridge speaks sente over `/chsk` with the MessagePack packer (ext type 0
  keywords, per-frame key cache, ext 100 timestamps), performs the CSRF
  scrape + login dance, patches `differ` diffs, and relays jnet-shaped state
  JSON to the UI. Implemented from extracted sente 1.22.0 / differ 0.3.3
  source — see `crates/jinteki-server/src/bridge/`.

Because both backends feed the UI the same state shape, **playing the same
line of play in both modes and watching for divergence IS the parity test.**
Every bridge session also appends every wire event to
`parity-logs/bridge-<epoch>.jsonl` for offline comparison.

## Running the reference server (the oracle)

Everything through nix + the reference repo's own docker-compose:

```sh
cd ../jinteki-reference
nix develop ../jinteki-rs   # provides colima, docker, docker-compose
colima start                # once per boot
docker-compose build database server   # once (~minutes)
docker-compose up -d database server
docker-compose exec -T server lein fetch --no-card-images  # seed card DB (once)
docker-compose exec -T server lein create-indexes
docker-compose restart server
```

Reference is then at `http://localhost:1042`. Register two accounts there
(first account created becomes admin), build decks — for honest comparison,
build the decks from the jinteki-rs pool (see `crates/jinteki-core/src/carddb.rs`).

Be gentle: the oracle is LOCAL. Never point automated drivers at the real
jinteki.net.

## The parity loop

1. `nix develop --command cargo run -p jinteki-server`, open
   `http://localhost:7787`.
2. Play a scripted line in **local mode** (fixed seed, note the log).
3. Connect **bridge mode** to `http://localhost:1042`, create a game with the
   pool decks, play the same line (a second browser tab with the second
   account can seat the opponent through the same UI).
4. Compare: in-game logs side by side, and the bridge's
   `parity-logs/*.jsonl` against the local engine's log output.

Divergences worth recording become allowlist entries per DESIGN.md SYS-F-1 —
each with a reason and, when the reference is the one that's wrong, an
upstream issue link.

## Known deliberate deviations of the playable milestone

These are simplifications the milestone accepts; each is visible in play and
none changes card outcomes in the 28-card pool:

1. Approach rez windows are modeled as an explicit Corp prompt instead of
   jnet's dual no-action latch; encounters auto-fire unbroken subroutines on
   Continue instead of requiring the Corp's fire click.
2. Runner breach of Archives auto-resolves non-agenda cards (jnet clicks
   through each).
3. Corp paid abilities outside its own turn (rezzing assets mid-run, e.g.)
   are not yet offered; nothing in the pool needs them.
4. Installing over a card in a remote auto-trashes it without a confirm.
5. Run events go to the heap at play time rather than after resolution.
6. The `:timer` lobby option and inactivity timeouts are not implemented
   locally (the milestone's fuse work is native-mode design, per DESIGN.md
   SYS-I-11).
