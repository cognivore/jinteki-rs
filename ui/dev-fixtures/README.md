# dev-fixtures — the deck-builder API, as static files

A tiny static stub of the deck-builder backend contract, served by the same
`ServeDir` fallback that serves the rest of `ui/`. The client probes
`GET /api/catalog?format=eternal` once; while that route does not exist it
falls back to these files (reads) plus a `localStorage` store (writes), and
the moment the real endpoints land the stub steps aside untouched — nothing
here shadows an `/api/*` path.

- `catalog-eternal.json` — `GET /api/catalog?format=eternal` response shape.
- `decks.json` — `GET /api/decks` response shape (the two builtins).
- `decks/<key>.json` — `GET /api/decks/<key>` response shape.

Card attributes are real (extracted from
`crates/jinteki-core/carddata/cards.json`); the two builtin lists are
`ANDROMEDA_LIST` (`crates/jinteki-server/src/cr.rs`) and the Making Stars
list (`docs/vm/DECK-QUEUE.md` §4). The eternal `points` values are DEV-STUB
PLACEHOLDERS to exercise the points meter — they are not the official
Eternal Points List; the server owns the real one.

Regeneration: the extraction script lives in the session scratchpad
(`gen_fixtures.py`); it filters the pool to the two builtin lists plus a
handful of extra cards per side so search, filters and off-faction influence
have something to bite on.
