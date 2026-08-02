#!/usr/bin/env python3
"""gen-carddata.py — card database + coverage codegen for jinteki-rs.

Pipeline (see docs/CARD-COVERAGE.md):
  tools/raw_data.edn  (official card data, from
      https://raw.githubusercontent.com/NoahTheDuke/netrunner-data/master/edn/raw_data.edn)
    │  tolerant EDN reader (below)
    ▼
  crates/jinteki-core/carddata/cards.json      printed data for EVERY card
  crates/jinteki-core/carddata/coverage.json   per-title implementation coverage
  docs/CARD-COVERAGE.md                        generated human summary

Coverage sources:
  - jnet_impl / jnet_partial: (defcard "Title" ...) forms scanned from the
    reference implementation ../jinteki-reference/src/clj/game/cards/*.clj
    (partial = the defcard map carries an :implementation caveat note).
  - rs_behavior: titles present in the hand-written behavior table
    crates/jinteki-core/src/carddb.rs (the behavior overlay).

Deterministic dedupe rule: when a title has several printings, the entry with
the highest numeric :code (latest printing) wins.

Regenerate with:  python3 tools/gen-carddata.py
"""

import json
import os
import re
import sys
from collections import OrderedDict

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RAW_EDN = os.path.join(REPO, "tools", "raw_data.edn")
REFERENCE_CARDS_DIR = os.path.join(
    os.path.dirname(REPO), "jinteki-reference", "src", "clj", "game", "cards"
)
CARDDB_RS = os.path.join(REPO, "crates", "jinteki-core", "src", "carddb.rs")
OUT_CARDS = os.path.join(REPO, "crates", "jinteki-core", "carddata", "cards.json")
OUT_COVERAGE = os.path.join(REPO, "crates", "jinteki-core", "carddata", "coverage.json")
OUT_DOC = os.path.join(REPO, "docs", "CARD-COVERAGE.md")

# ── tolerant EDN reader ─────────────────────────────────────────────────────
# The input is machine-generated and regular; this reader covers the subset it
# uses (strings with escapes, keywords, numbers, booleans, nil, vectors, maps)
# plus a little slack (lists, sets, #_ discard, tagged literals, comments).

TOKEN_RE = re.compile(
    r"""
      (?P<ws>[\s,]+)
    | (?P<comment>;[^\n]*)
    | (?P<discard>\#_)
    | (?P<set>\#\{)
    | (?P<string>"(?:[^"\\]|\\.)*")
    | (?P<punct>[()\[\]{}])
    | (?P<keyword>:[^\s,()\[\]{}"';]+)
    | (?P<number>[+-]?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[MN]?)
      (?=[\s,()\[\]{}"';]|$)
    | (?P<symbol>[^\s,()\[\]{}"';]+)
    """,
    re.VERBOSE,
)

_STR_ESCAPES = {"n": "\n", "t": "\t", "r": "\r", '"': '"', "\\": "\\", "b": "\b", "f": "\f"}


def unescape_edn_string(tok):
    body = tok[1:-1]
    out = []
    i = 0
    while i < len(body):
        ch = body[i]
        if ch == "\\" and i + 1 < len(body):
            nxt = body[i + 1]
            if nxt == "u" and i + 5 < len(body):
                try:
                    out.append(chr(int(body[i + 2 : i + 6], 16)))
                    i += 6
                    continue
                except ValueError:
                    pass
            out.append(_STR_ESCAPES.get(nxt, nxt))
            i += 2
        else:
            out.append(ch)
            i += 1
    return "".join(out)


def tokenize_edn(src):
    toks = []
    pos = 0
    n = len(src)
    while pos < n:
        m = TOKEN_RE.match(src, pos)
        if not m:
            raise ValueError(f"EDN tokenizer stuck at offset {pos}: {src[pos:pos+40]!r}")
        pos = m.end()
        kind = m.lastgroup
        if kind in ("ws", "comment"):
            continue
        toks.append((kind, m.group()))
    return toks


class EdnParser:
    def __init__(self, toks):
        self.toks = toks
        self.i = 0

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def next(self):
        t = self.toks[self.i]
        self.i += 1
        return t

    def parse(self):
        kind, val = self.next()
        if kind == "string":
            return unescape_edn_string(val)
        if kind == "keyword":
            return val  # keep the leading ':' — keys look like ':title'
        if kind == "number":
            body = val.rstrip("MN")
            if any(c in body for c in ".eE"):
                return float(body)
            return int(body)
        if kind == "symbol":
            if val == "true":
                return True
            if val == "false":
                return False
            if val == "nil":
                return None
            return ("sym", val)
        if kind == "discard":
            self.parse()  # value read and dropped
            return self.parse()
        if kind == "set":
            return self.parse_seq("}")
        if kind == "punct":
            if val == "[":
                return self.parse_seq("]")
            if val == "(":
                return self.parse_seq(")")
            if val == "{":
                return self.parse_map()
            raise ValueError(f"unexpected {val!r}")
        raise ValueError(f"unexpected token {kind} {val!r}")

    def parse_seq(self, closer):
        out = []
        while True:
            kind, val = self.peek()
            if kind is None:
                raise ValueError("unterminated sequence")
            if kind == "punct" and val == closer:
                self.next()
                return out
            out.append(self.parse())

    def parse_map(self):
        out = {}
        while True:
            kind, val = self.peek()
            if kind is None:
                raise ValueError("unterminated map")
            if kind == "punct" and val == "}":
                self.next()
                return out
            k = self.parse()
            v = self.parse()
            if isinstance(k, (str, int, float, bool)) or k is None:
                out[k] = v
            else:
                out[str(k)] = v


def read_edn_file(path):
    with open(path, encoding="utf-8") as f:
        src = f.read()
    return EdnParser(tokenize_edn(src)).parse()


# ── reference defcard scanner (Clojure source) ──────────────────────────────


def parse_clj_string(src, i):
    """src[i] == '"'; returns (unescaped-string, index-after-closing-quote)."""
    assert src[i] == '"'
    j = i + 1
    out = []
    while j < len(src):
        ch = src[j]
        if ch == "\\" and j + 1 < len(src):
            out.append(_STR_ESCAPES.get(src[j + 1], src[j + 1]))
            j += 2
            continue
        if ch == '"':
            return "".join(out), j + 1
        out.append(ch)
        j += 1
    raise ValueError("unterminated Clojure string")


def form_end(src, start):
    """Index just past the balanced form opening at src[start] == '('.

    Tracks strings (with escapes), line comments, and char literals so parens
    inside them don't affect the balance.
    """
    depth = 0
    i = start
    n = len(src)
    while i < n:
        ch = src[i]
        if ch == '"':
            _, i = parse_clj_string(src, i)
            continue
        if ch == ";":
            nl = src.find("\n", i)
            i = n if nl < 0 else nl + 1
            continue
        if ch == "\\" and i + 1 < n:  # char literal: \( \" \space ...
            i += 2
            continue
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError("unbalanced form")


def scan_defcards(cards_dir):
    """Return {title: has_implementation_note} for every (defcard "Title" ...)."""
    result = {}
    for fname in sorted(os.listdir(cards_dir)):
        if not fname.endswith(".clj"):
            continue
        with open(os.path.join(cards_dir, fname), encoding="utf-8") as f:
            src = f.read()
        i = 0
        while True:
            j = src.find("(defcard ", i)
            if j < 0:
                break
            # Only accept forms that start a line (avoids matches inside
            # strings/comments; every real defcard is at column 0).
            if j > 0 and src[j - 1] != "\n":
                i = j + 1
                continue
            k = j + len("(defcard ")
            while k < len(src) and src[k] in " \t\n":
                k += 1
            if k >= len(src) or src[k] != '"':
                i = j + 1
                continue
            title, _ = parse_clj_string(src, k)
            end = form_end(src, j)
            body = src[j:end]
            # Caveat either as a literal :implementation key or attached via
            # the (implementation-note "..." def) helper (e.g. "Wendigo").
            partial = ":implementation" in body or "(implementation-note" in body
            # A title defined twice keeps partial=true if any form says so.
            result[title] = result.get(title, False) or partial
            i = end
    return result


# ── Rust behavior-table scanner ─────────────────────────────────────────────


def scan_rust_behaviors(path):
    """Titles in the CARDS behavior table of carddb.rs."""
    with open(path, encoding="utf-8") as f:
        src = f.read()
    start = src.index("pub const CARDS")
    end = src.index("];", start)
    block = src[start:end]
    titles = []
    for m in re.finditer(r'CardDef::blank\(\s*"((?:[^"\\]|\\.)*)"', block):
        t = m.group(1).replace('\\"', '"').replace("\\\\", "\\")
        titles.append(t)
    return titles


# ── card normalization ──────────────────────────────────────────────────────


def as_int(v):
    if isinstance(v, bool):
        return None
    if isinstance(v, int):
        return v
    if isinstance(v, float):
        return int(v)
    return None


def normalize_card(c):
    return OrderedDict(
        [
            ("title", c.get(":title")),
            ("code", c.get(":code")),
            ("side", c.get(":side")),
            ("type", c.get(":type")),
            ("faction", c.get(":faction")),
            ("subtypes", c.get(":subtypes") or []),
            ("text", c.get(":text")),
            ("cost", as_int(c.get(":cost"))),
            ("strength", as_int(c.get(":strength"))),
            ("memoryunits", as_int(c.get(":memoryunits"))),
            ("trash_cost", as_int(c.get(":trash"))),
            ("advancement_requirement", as_int(c.get(":advancementcost"))),
            ("agenda_points", as_int(c.get(":agendapoints"))),
            ("base_link", as_int(c.get(":baselink"))),
            ("uniqueness", bool(c.get(":uniqueness", False))),
            ("deck_limit", as_int(c.get(":deck-limit"))),
            ("set", c.get(":setname")),
            ("cycle", c.get(":cycle_code")),
            ("rotated", bool(c.get(":rotated", False))),
        ]
    )


def code_key(c):
    code = c.get(":code")
    if isinstance(code, str):
        digits = re.sub(r"\D", "", code)
        if digits:
            return int(digits)
    if isinstance(code, int):
        return code
    return -1


def main():
    print(f"reading {RAW_EDN} ...", file=sys.stderr)
    data = read_edn_file(RAW_EDN)
    raw_cards = data.get(":cards") or []
    cycles = data.get(":cycles") or []
    print(f"  parsed {len(raw_cards)} raw card entries", file=sys.stderr)

    cycle_name = {}
    cycle_pos = {}
    for cy in cycles:
        if isinstance(cy, dict):
            code = cy.get(":code")
            if code:
                cycle_name[code] = cy.get(":name") or code
                cycle_pos[code] = cy.get(":position", 9999)

    # Dedupe by title; latest printing (highest numeric code) wins.
    by_title = {}
    for c in raw_cards:
        t = c.get(":title")
        if not t:
            continue
        if t not in by_title or code_key(c) > code_key(by_title[t]):
            by_title[t] = c
    cards = [normalize_card(c) for c in sorted(by_title.values(), key=lambda c: c.get(":title"))]

    # Coverage sources.
    defcards = scan_defcards(REFERENCE_CARDS_DIR)
    rs_titles = scan_rust_behaviors(CARDDB_RS)
    print(
        f"  reference defcards: {len(defcards)}  rust behaviors: {len(rs_titles)}",
        file=sys.stderr,
    )

    titles = {c["title"] for c in cards}
    jnet_extra = sorted(t for t in defcards if t not in titles)
    rs_extra = sorted(t for t in rs_titles if t not in titles)
    if rs_extra:
        print(f"  WARNING: rust behaviors not in card data: {rs_extra}", file=sys.stderr)

    coverage_cards = OrderedDict()
    for c in cards:
        t = c["title"]
        coverage_cards[t] = OrderedDict(
            [
                ("jnet_impl", t in defcards),
                ("jnet_partial", bool(defcards.get(t, False))),
                ("rs_behavior", t in rs_titles),
            ]
        )

    coverage = OrderedDict(
        [
            (
                "_provenance",
                OrderedDict(
                    [
                        ("generator", "tools/gen-carddata.py"),
                        (
                            "card_data",
                            "tools/raw_data.edn (netrunner-data raw_data.edn)",
                        ),
                        ("reference", "jinteki-reference/src/clj/game/cards/*.clj"),
                        ("behavior_table", "crates/jinteki-core/src/carddb.rs"),
                    ]
                ),
            ),
            ("cards", coverage_cards),
            ("jnet_extra_titles", jnet_extra),
        ]
    )

    os.makedirs(os.path.dirname(OUT_CARDS), exist_ok=True)
    with open(OUT_CARDS, "w", encoding="utf-8") as f:
        json.dump(cards, f, ensure_ascii=False, indent=1)
        f.write("\n")
    with open(OUT_COVERAGE, "w", encoding="utf-8") as f:
        json.dump(coverage, f, ensure_ascii=False, indent=1)
        f.write("\n")

    # ── docs/CARD-COVERAGE.md ───────────────────────────────────────────────
    total = len(cards)
    jnet_count = sum(1 for v in coverage_cards.values() if v["jnet_impl"])
    partial_count = sum(1 for v in coverage_cards.values() if v["jnet_partial"])
    rs_count = sum(1 for v in coverage_cards.values() if v["rs_behavior"])
    isolated = sorted(t for t, v in coverage_cards.items() if not v["jnet_impl"])

    per_cycle = {}
    for c in cards:
        cy = c["cycle"] or "(none)"
        row = per_cycle.setdefault(cy, {"total": 0, "jnet": 0, "rs": 0})
        row["total"] += 1
        cov = coverage_cards[c["title"]]
        if cov["jnet_impl"]:
            row["jnet"] += 1
        if cov["rs_behavior"]:
            row["rs"] += 1

    def cycle_sort(code):
        return (cycle_pos.get(code, 9999), code)

    lines = []
    lines.append("# Card Coverage")
    lines.append("")
    lines.append("**GENERATED FILE — do not edit.** Regenerate with `python3 tools/gen-carddata.py`.")
    lines.append("")
    lines.append("## Totals")
    lines.append("")
    lines.append(f"- Cards in database: **{total}**")
    lines.append(
        f"- Implemented by the reference (jinteki.net defcard exists): **{jnet_count}**"
    )
    lines.append(
        f"- Reference implementations flagged partial (`:implementation` caveat): **{partial_count}**"
    )
    lines.append(
        f"- Not implemented anywhere — the ISOLATED set needing fresh behavior work: **{len(isolated)}**"
    )
    lines.append(f"- Rust behavior overlay (carddb.rs): **{rs_count}**")
    lines.append("")
    lines.append("### Isolated titles (no implementation even in jinteki.net)")
    lines.append("")
    if isolated:
        for t in isolated:
            side_ty = by_title[t]
            lines.append(
                f"- {t} ({side_ty.get(':side')}, {side_ty.get(':type')}, {side_ty.get(':setname')})"
            )
    else:
        lines.append("(none)")
    lines.append("")
    lines.append("### Reference defcard titles absent from the card data")
    lines.append("")
    lines.append(
        "jinteki.net pseudo-cards (basic actions and similar); they are engine-internal on our side too:"
    )
    lines.append("")
    for t in jnet_extra:
        lines.append(f"- {t}")
    lines.append("")
    lines.append("## Per-cycle coverage")
    lines.append("")
    lines.append("| Cycle | Cards | jnet impl | rs behavior |")
    lines.append("|---|---:|---:|---:|")
    for cy in sorted(per_cycle, key=cycle_sort):
        row = per_cycle[cy]
        name = cycle_name.get(cy, cy)
        lines.append(f"| {name} (`{cy}`) | {row['total']} | {row['jnet']} | {row['rs']} |")
    lines.append("")
    lines.append("## How the pipeline works")
    lines.append("")
    lines.append(
        "`tools/raw_data.edn` (official card data, vendored from "
        "[netrunner-data](https://raw.githubusercontent.com/NoahTheDuke/netrunner-data/master/edn/raw_data.edn)) "
        "is parsed by `tools/gen-carddata.py` (a small tolerant EDN reader), which emits:"
    )
    lines.append("")
    lines.append(
        "- `crates/jinteki-core/carddata/cards.json` — printed data for every card "
        "(deduped by title; the printing with the highest numeric code wins);"
    )
    lines.append(
        "- `crates/jinteki-core/carddata/coverage.json` — per-title flags: does a "
        "reference `(defcard \"Title\" ...)` exist (`jnet_impl`), does it carry an "
        "`:implementation` caveat (`jnet_partial`), and does the Rust behavior table "
        "cover it (`rs_behavior`);"
    )
    lines.append("- this document.")
    lines.append("")
    lines.append(
        "`crates/jinteki-core/src/printed.rs` embeds both JSON files via `include_str!` "
        "and exposes `printed(title)`, `all_printed()`, and `impl_status(title)` "
        "(`Behavior` / `JnetOnly` / `Unimplemented`)."
    )
    lines.append("")
    lines.append(
        "The behavior overlay stays in `crates/jinteki-core/src/carddb.rs`: a card with "
        "a `CardDef` row there plays with full rules. Every other card known to "
        "`printed()` spawns as a **vanilla** runtime definition — correct printed "
        "stats, no behavior hooks: operations and events resolve with only their cost "
        "paid (the log notes \"(no implemented effect)\"), ice has zero subroutines, "
        "assets/upgrades/hardware/resources install and sit there. Unknown titles are "
        "a clean error instead of a panic."
    )
    lines.append("")
    lines.append("Regenerate everything with: `python3 tools/gen-carddata.py`")
    lines.append("")

    os.makedirs(os.path.dirname(OUT_DOC), exist_ok=True)
    with open(OUT_DOC, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    print(
        f"wrote {OUT_CARDS} ({total} cards), {OUT_COVERAGE}, {OUT_DOC}",
        file=sys.stderr,
    )
    print(
        f"totals: cards={total} jnet={jnet_count} partial={partial_count} "
        f"isolated={len(isolated)} rs={rs_count} jnet_extra={len(jnet_extra)}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
