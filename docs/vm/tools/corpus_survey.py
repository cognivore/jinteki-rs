#!/usr/bin/env python3
"""DP-7c corpus survey: measure the jinteki-reference kaocha test corpus.

Reads the pinned reference checkout (default ~/Github/jinteki/jinteki-reference,
commit 4054730) and emits a machine-readable index of every `deftest`, plus the
aggregate counts that `docs/vm/CORPUS.md` quotes.

Usage:
    python3 docs/vm/tools/corpus_survey.py [--ref PATH] [--json OUT]

The index is a JSON array, one object per test:
    file, name, line, lines, helpers[], cards[], asserts, kind
where `kind` is engine (test/clj/game/core/**), card (test/clj/game/cards/**),
basic (cards/basic_test.clj) or web (everything else).
"""

import argparse
import json
import os
import re
import sys
from collections import Counter

HELPER_RE = re.compile(r"\(([a-z][a-z0-9?!*<>=-]*)")
STRING_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')
DEFTEST_RE = re.compile(r"^\(deftest\s+([^\s\)]+)", re.M)
TITLE_RE = re.compile(r':title "((?:[^"\\]|\\.)*)"')


def card_titles(ref):
    path = os.path.join(ref, "data", "cards.edn")
    with open(path, encoding="utf-8") as fh:
        return set(TITLE_RE.findall(fh.read()))


def deftests(text):
    """Split a file into (name, line, body) triples by paren balance."""
    out = []
    for m in DEFTEST_RE.finditer(text):
        start = m.start()
        depth, i, in_str, esc = 0, start, False, False
        while i < len(text):
            c = text[i]
            if in_str:
                if esc:
                    esc = False
                elif c == "\\":
                    esc = True
                elif c == '"':
                    in_str = False
            elif c == '"':
                in_str = True
            elif c == ";":
                while i < len(text) and text[i] != "\n":
                    i += 1
            elif c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
                if depth == 0:
                    i += 1
                    break
            i += 1
        body = text[start:i]
        out.append((m.group(1), text.count("\n", 0, start) + 1, body))
    return out


def classify(rel):
    if rel.endswith("cards/basic_test.clj"):
        return "basic"
    if "/core/" in rel:
        return "engine"
    if "/cards/" in rel:
        return "card"
    return "other"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ref", default=os.path.expanduser("~/Github/jinteki/jinteki-reference"))
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    titles = card_titles(args.ref)
    root = os.path.join(args.ref, "test", "clj", "game")
    framework = os.path.join(root, "test_framework.clj")
    with open(framework, encoding="utf-8") as fh:
        ftext = fh.read()
    known = set(re.findall(r"^\(def(?:n|macro)-?\s+([a-z0-9?!*<>=-]+)", ftext, re.M))

    index = []
    for dirpath, _dirs, files in os.walk(root):
        for name in sorted(files):
            if not name.endswith("_test.clj"):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, args.ref)
            with open(path, encoding="utf-8") as fh:
                text = fh.read()
            for tname, line, body in deftests(text):
                helpers = sorted({h for h in HELPER_RE.findall(body) if h in known})
                cards = sorted({s for s in STRING_RE.findall(body) if s in titles})
                index.append({
                    "file": rel,
                    "name": tname,
                    "line": line,
                    "lines": body.count("\n") + 1,
                    "helpers": helpers,
                    "cards": cards,
                    "asserts": len(re.findall(r"\(is\b", body)),
                    "kind": classify(rel),
                })

    if args.json:
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(index, fh, indent=1, sort_keys=True)

    kinds = Counter(t["kind"] for t in index)
    helpers = Counter(h for t in index for h in t["helpers"])
    cards = Counter(c for t in index for c in t["cards"])
    files = Counter(t["file"] for t in index)
    print(f"tests: {len(index)}  asserts: {sum(t['asserts'] for t in index)}")
    print("by kind:", dict(kinds))
    print(f"distinct cards referenced: {len(cards)} (of {len(titles)} printed)")
    print(f"tests with no card reference: {sum(1 for t in index if not t['cards'])}")
    print(f"tests referencing 1 card: {sum(1 for t in index if len(t['cards']) == 1)}")
    print("\n-- files (top 20) --")
    for f, n in files.most_common(20):
        print(f"{n:5d}  {f}")
    print("\n-- helpers (top 40) --")
    for h, n in helpers.most_common(40):
        print(f"{n:5d}  {h}")
    print("\n-- cards (top 30) --")
    for c, n in cards.most_common(30):
        print(f"{n:5d}  {c}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
