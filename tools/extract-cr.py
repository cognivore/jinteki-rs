#!/usr/bin/env python3
"""extract-cr.py — Null Signal Games Comprehensive Rules extractor.

Parses the saved HTML of the NSG Netrunner Comprehensive Rules
(https://rules.nullsignal.games/) into normative, machine-readable
artifacts used by the jinteki-rs engine:

  docs/rules/CR-v<ver>.md            complete rules as markdown (stable anchors)
  docs/rules/cr-index.json           flat index of every rule/sub-rule
  docs/rules/timing-structures.json  ordered step lists from section 11
  docs/rules/examples.json           every worked example, verbatim
  docs/rules/cr-glossary.json        defined terms -> rule ids

Usage:
    python3 tools/extract-cr.py "<path-to-html>" --out docs/rules

Stdlib only (html.parser). Re-runnable for future CR versions: the version
is taken from the document <title> when possible, else from the filename
(pattern v<major>.<minor>).

HTML structure this parser understands (verified against v26.03):
  <h1 class="Chapter" id="chpt_*">N. Title</h1>
  <h2 class="Section" id="sec_*">N.M. Title</h2>
  <ol class="Rules">    <li class="Rule" id="...">   (top-level; NOT nested)
  <ol class="SubRules"> <li class="SubRule" id="..."> (sibling ol following
                         its parent Rule's ol — parenthood is by sequence)
  rule number in <a class="RuleLink">N.M.K.</a> / sub-rule letter "a."
  rule text in <span class="RuleText">; "header" rules instead carry
  <span class="SubSection">Title</span>
  <ol class="Examples ..."> <li class="Example"> — inside a Rule/SubRule li,
  or top-level right after the li it belongs to (ExamplesSubSection)
  <ol class="TimingStructureList"> <li class="TimingStructureL1|L2|L3 ..."> —
  section 11 step tables, nested via anonymous <ol>
  <img class="Symbol" alt="click|credit|..."> — game symbols
  <a class="Thumbnail Card" href="netrunnerdb...">Name<span
    class="ThumbnailImageContainer">…</span></a> — card references
  <span class="Term"> — defined terms;  <span class="SubType"> — subtypes
"""

import argparse
import json
import os
import re
import sys
from html.parser import HTMLParser

SOURCE_URL = "https://rules.nullsignal.games/"

# img.Symbol alt attribute -> readable token
SYMBOL_TOKENS = {
    "click": "[click]",
    "credit": "[credit]",
    "link": "[link]",
    "mu": "[mu]",
    "sub": "[subroutine]",
    "trash": "[trash]",
    "trashcost": "[trash-cost]",
    "recurring": "[recurring]",
    "interrupt": "[interrupt]",
}

# section-id suffix of the five §11 timing structures -> engine structure key
STRUCTURE_KEYS = [
    ("corps_turn", "corp_turn"),
    ("runners_turn", "runner_turn"),
    ("of_a_run", "run"),
    ("breaching_a_server", "breach"),
    ("accessing_a_card", "access"),
]

VOID_TAGS = {"img", "br", "hr", "meta", "link", "input", "source", "wbr"}


def norm_ws(s: str) -> str:
    return re.sub(r"\s+", " ", s).strip()


# ---------------------------------------------------------------------------
# Token stream rendering
# Tokens: ("t", text) ("sym", name) ("card", name, nrdb_id)
#         ("term_open",) ("term_close",) ("subtype_open",) ("subtype_close",)
#         ("ref_open", target_or_None) ("ref_close",)
# ---------------------------------------------------------------------------

def render_plain(tokens):
    out = []
    for tok in tokens:
        k = tok[0]
        if k == "t":
            out.append(tok[1])
        elif k == "sym":
            out.append(SYMBOL_TOKENS.get(tok[1], "[%s]" % tok[1]))
        elif k == "card":
            out.append(tok[1])
        # markers render as nothing in plain text
    return norm_ws("".join(out))


def render_md(tokens, known_ids):
    """Render a token stream to markdown. Emphasis (term/subtype) and links
    (internal refs) are built as nested groups so whitespace never sits
    inside the markers (``** agenda **`` would not render as bold)."""
    OPENERS = {"term_open": "term", "subtype_open": "subtype", "ref_open": "ref"}
    CLOSERS = {"term_close": "term", "subtype_close": "subtype", "ref_close": "ref"}
    root = {"kind": "root", "parts": [], "target": None}
    stack = [root]
    for tok in tokens:
        k = tok[0]
        if k in OPENERS:
            g = {"kind": OPENERS[k], "parts": [],
                 "target": tok[1] if k == "ref_open" else None}
            stack[-1]["parts"].append(g)
            stack.append(g)
        elif k in CLOSERS:
            # pop until matching kind (tolerate mis-nesting)
            while len(stack) > 1:
                top = stack.pop()
                if top["kind"] == CLOSERS[k]:
                    break
        elif k == "t":
            stack[-1]["parts"].append(tok[1].replace("*", r"\*"))
        elif k == "sym":
            stack[-1]["parts"].append(SYMBOL_TOKENS.get(tok[1], "[%s]" % tok[1]))
        elif k == "card":
            stack[-1]["parts"].append(tok[1])

    def flatten(g):
        s = "".join(flatten(p) if isinstance(p, dict) else p for p in g["parts"])
        if g["kind"] == "root":
            return s
        # keep leading/trailing whitespace OUTSIDE the markers
        lead = s[:len(s) - len(s.lstrip())]
        trail = s[len(s.rstrip()):]
        core = s.strip()
        if not core:
            return s
        if g["kind"] == "term":
            core = "**%s**" % core
        elif g["kind"] == "subtype":
            core = "*%s*" % core
        elif g["kind"] == "ref" and g["target"] and g["target"] in known_ids:
            core = "[%s](#%s)" % (core, g["target"])
        return lead + core + trail

    return norm_ws(flatten(root))


def harvest(tokens, kind):
    """Collect term or card strings from a token stream."""
    found = []
    if kind == "card":
        for tok in tokens:
            if tok[0] == "card":
                nm = norm_ws(tok[1])
                if nm and nm not in found:
                    found.append(nm)
        return found
    # terms: text between term_open/term_close
    depth = 0
    buf = []
    for tok in tokens:
        if tok[0] == "term_open":
            depth += 1
            buf = []
        elif tok[0] == "term_close":
            depth -= 1
            t = norm_ws("".join(buf))
            if t and t not in found:
                found.append(t)
            buf = []
        elif depth > 0 and tok[0] == "t":
            buf.append(tok[1])
        elif depth > 0 and tok[0] == "sym":
            buf.append(SYMBOL_TOKENS.get(tok[1], tok[1]))
        elif depth > 0 and tok[0] == "card":
            buf.append(tok[1])
    return found


# ---------------------------------------------------------------------------
# Parser
# ---------------------------------------------------------------------------

class Frame:
    __slots__ = ("tag", "cls", "id", "kind", "tokens", "label", "header_tokens",
                 "examples", "children", "card_name", "card_id", "targets")

    def __init__(self, tag, cls, id_, kind=None):
        self.tag = tag
        self.cls = cls
        self.id = id_
        self.kind = kind
        self.tokens = []
        self.label = []
        self.header_tokens = None
        self.examples = []
        self.children = []
        self.card_name = []
        self.card_id = None
        self.targets = []


def cls_has(cls, name):
    return name in (cls or "").split()


def internal_target(href):
    """Extract a rules-document anchor id from an href, if any."""
    if not href:
        return None
    m = re.search(r"#([A-Za-z0-9_]+)$", href)
    if m and not m.group(1).startswith("icon"):
        return m.group(1)
    m = re.search(r"[?&]r=([A-Za-z0-9_]+)", href)
    if m:
        return m.group(1)
    return None


class CRParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack = []
        self.chapters = []      # {id, number, title, sections:[...]}
        self.sections = []      # {id, number, title, chapter_id, rules:[...]}
        self.rules = []         # rule + subrule records, document order
        self.timing = []        # {section_id, items:[...]} per TimingStructureList
        self.cur_chapter = None
        self.cur_section = None
        self.cur_rule = None    # last closed li.Rule record
        self.last_item = None   # last closed rule OR subrule record
        self.warnings = []

    # -- helpers ---------------------------------------------------------

    def _find(self, pred):
        for fr in reversed(self.stack):
            if pred(fr):
                return fr
        return None

    def _target_frame(self):
        """The frame whose token list should receive text/markers now."""
        for fr in reversed(self.stack):
            if fr.kind in ("svg", "thumbcontainer"):
                return None
            if fr.kind == "rulelink":
                return fr           # number label buffer
            if fr.kind == "linkwrap":
                return None
            if fr.kind == "card":
                return fr           # card name buffer
            if fr.kind == "subsec_span":
                return fr
            if fr.kind == "ruletext":
                # rule body text belongs to the enclosing Rule/SubRule li
                return self._find(lambda f: f.kind == "rule_li")
            if fr.kind in ("rule_li", "example_li", "timing_li", "h1", "h2"):
                if fr.kind == "rule_li":
                    # direct text outside span.RuleText is chrome; ignore
                    return None
                return fr
        return None

    def _emit(self, tok):
        fr = self._target_frame()
        if fr is None:
            return
        if fr.kind == "rulelink":
            # a.RuleLink holds the rule number ("1.1.1." / "a.") inside a
            # Rule/SubRule li, but holds the full numbered TITLE inside h1/h2
            host = self._find(lambda f: f.kind in ("rule_li", "h1", "h2"))
            if host is None:
                return
            if host.kind == "rule_li":
                if tok[0] == "t":
                    host.label.append(tok[1])
            else:
                host.tokens.append(tok)
            return
        if fr.kind == "card":
            if tok[0] == "t":
                fr.card_name.append(tok[1])
            elif tok[0] == "sym":
                fr.card_name.append(SYMBOL_TOKENS.get(tok[1], tok[1]))
            return
        fr.tokens.append(tok)

    # -- tag events ------------------------------------------------------

    def handle_starttag(self, tag, attrs):
        if tag in VOID_TAGS:
            self.handle_startendtag(tag, attrs)
            return
        a = dict(attrs)
        cls = a.get("class", "")
        id_ = a.get("id")
        fr = Frame(tag, cls, id_)

        if tag == "svg":
            fr.kind = "svg"
        elif tag == "h1" and cls_has(cls, "Chapter"):
            fr.kind = "h1"
        elif tag == "h2" and cls_has(cls, "Section"):
            fr.kind = "h2"
        elif tag == "li" and (cls_has(cls, "Rule") or cls_has(cls, "SubRule")):
            fr.kind = "rule_li"
        elif tag == "li" and cls_has(cls, "Example"):
            fr.kind = "example_li"
        elif tag == "li" and "TimingStructure" in cls:
            fr.kind = "timing_li"
        elif tag == "ol" and cls_has(cls, "TimingStructureList"):
            fr.kind = "timing_ol"
            fr.children = []
        elif tag == "span":
            if cls_has(cls, "RuleText"):
                fr.kind = "ruletext"
            elif cls_has(cls, "SubSection"):
                fr.kind = "subsec_span"
            elif cls_has(cls, "RuleLinkWrapper"):
                fr.kind = "linkwrap"
            elif cls_has(cls, "ThumbnailImageContainer"):
                fr.kind = "thumbcontainer"
            elif cls_has(cls, "Term"):
                self._emit(("term_open",))
                fr.kind = "term"
            elif cls_has(cls, "SubType"):
                self._emit(("subtype_open",))
                fr.kind = "subtype"
        elif tag == "a":
            href = a.get("href", "")
            if cls_has(cls, "RuleLink"):
                fr.kind = "rulelink"
            elif cls_has(cls, "RuleAnchor"):
                fr.kind = "ruleanchor"
            elif cls_has(cls, "Thumbnail") or "netrunnerdb.com/en/card/" in href:
                fr.kind = "card"
                m = re.search(r"/card/(\d+)", href)
                fr.card_id = m.group(1) if m else None
            else:
                tgt = internal_target(href)
                # record cross-reference targets on the enclosing h2 (used to
                # link §11 timing structures to their prose sections)
                h2 = self._find(lambda f: f.kind == "h2")
                if tgt and h2 is not None:
                    h2.targets.append(tgt)
                if tgt:
                    fr.kind = "ref"
                    self._emit(("ref_open", tgt))
                # external non-card links are transparent text
        self.stack.append(fr)

    def handle_startendtag(self, tag, attrs):
        a = dict(attrs)
        cls = a.get("class", "")
        if tag == "img" and cls_has(cls, "Symbol"):
            alt = a.get("alt", "")
            if not alt:
                m = re.search(r"/([a-z0-9]+)\.svg", a.get("src", ""))
                alt = m.group(1) if m else "?"
            self._emit(("sym", alt))

    def handle_endtag(self, tag):
        if tag in VOID_TAGS:
            return
        # find matching open frame (generated HTML is well-formed; guard anyway)
        idx = None
        for i in range(len(self.stack) - 1, -1, -1):
            if self.stack[i].tag == tag:
                idx = i
                break
        if idx is None:
            return
        # close any frames left open above (shouldn't happen)
        for fr in reversed(self.stack[idx + 1:]):
            self.warnings.append("implicitly closed <%s class=%r>" % (fr.tag, fr.cls))
        frames = self.stack[idx:]
        del self.stack[idx:]
        fr = frames[0]
        self._close(fr)

    def _close(self, fr):
        k = fr.kind
        if k == "term":
            self._emit(("term_close",))
        elif k == "subtype":
            self._emit(("subtype_close",))
        elif k == "ref":
            self._emit(("ref_close",))
        elif k == "card":
            name = norm_ws("".join(fr.card_name))
            if name:
                self._emit(("card", name, fr.card_id))
        elif k == "h1":
            text = render_plain(fr.tokens)
            m = re.match(r"^(\d+)\.\s*(.*)$", text)
            ch = {
                "id": fr.id,
                "number": m.group(1) if m else "",
                "title": m.group(2) if m else text,
                "sections": [],
            }
            self.chapters.append(ch)
            self.cur_chapter = ch
        elif k == "h2":
            text = render_plain(fr.tokens)
            m = re.match(r"^(\d+\.\d+)\.\s*(.*)$", text)
            sec = {
                "id": fr.id,
                "number": m.group(1) if m else "",
                "title": m.group(2) if m else text,
                "chapter_id": self.cur_chapter["id"] if self.cur_chapter else None,
                "link_targets": [t for t in fr.targets if t],
                "rules": [],
            }
            self.sections.append(sec)
            if self.cur_chapter:
                self.cur_chapter["sections"].append(sec)
            self.cur_section = sec
        elif k == "rule_li":
            is_sub = cls_has(fr.cls, "SubRule")
            label = norm_ws("".join(fr.label))
            rec = {
                "id": fr.id,
                "kind": "subrule" if is_sub else "rule",
                "label": label,
                "tokens": fr.tokens,
                "header_tokens": fr.header_tokens,
                "examples": fr.examples,
                "section": self.cur_section,
                "parent": None,
                "children": [],
            }
            if is_sub:
                if self.cur_rule is None:
                    self.warnings.append("SubRule %s with no preceding Rule" % fr.id)
                else:
                    rec["parent"] = self.cur_rule
                    self.cur_rule["children"].append(rec)
            else:
                self.cur_rule = rec
                if self.cur_section:
                    self.cur_section["rules"].append(rec)
            self.rules.append(rec)
            self.last_item = rec
        elif k == "example_li":
            # attach to nearest enclosing rule li, else to the last closed item
            host = self._find(lambda f: f.kind == "rule_li")
            ex = {"tokens": fr.tokens}
            if host is not None:
                host.examples.append(ex)
            elif self.last_item is not None:
                self.last_item["examples"].append(ex)
            else:
                self.warnings.append("orphan example: %s" %
                                     render_plain(fr.tokens)[:60])
        elif k == "timing_li":
            m = re.search(r"TimingStructureL(\d)", fr.cls)
            item = {
                "level": int(m.group(1)) if m else 0,
                "bold": "TimingStructureBold" in fr.cls,
                "tokens": fr.tokens,
                "children": fr.children,
            }
            host = self._find(lambda f: f.kind in ("timing_li", "timing_ol"))
            if host is not None:
                host.children.append(item)
            else:
                self.warnings.append("orphan timing li")
        elif k == "timing_ol":
            self.timing.append({
                "section": self.cur_section,
                "items": fr.children,
            })
        elif k == "subsec_span":
            host = self._find(lambda f: f.kind == "rule_li")
            if host is not None:
                host.header_tokens = fr.tokens

    def handle_data(self, data):
        if data:
            self._emit(("t", data))


# ---------------------------------------------------------------------------
# Post-processing
# ---------------------------------------------------------------------------

def assign_numbers(parser):
    for rec in parser.rules:
        label = rec["label"].rstrip()
        if rec["kind"] == "rule":
            rec["number"] = label            # e.g. "10.3.1."
        else:
            parent = rec["parent"]
            pnum = parent["number"] if parent else "?"
            rec["number"] = "%s%s" % (pnum, label)   # "10.3.1." + "a." -> "10.3.1.a."


def sim(a, b):
    stop = {"the", "a", "an", "of", "to", "is", "are", "and", "or", "in",
            "if", "any", "their", "s"}
    ta = set(re.findall(r"[a-z0-9]+", a.lower())) - stop
    tb = set(re.findall(r"[a-z0-9]+", b.lower())) - stop
    if not ta or not tb:
        return 0.0
    return len(ta & tb) / len(ta | tb)


LETTERS = "abcdefghijklmnopqrstuvwxyz"
ROMANS = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"]


def build_timing(parser, warn):
    """Convert the five TimingStructureList trees into engine step tables,
    cross-referenced against the prose step rules in sections 5/6/7."""
    sec_by_id = {s["id"]: s for s in parser.sections}
    structures = []
    for t in parser.timing:
        sec = t["section"]
        skey = None
        for suffix, key in STRUCTURE_KEYS:
            if sec["id"].endswith(suffix):
                skey = key
        if skey is None:
            warn("timing structure in unrecognized section %s" % sec["id"])
            continue
        prose_id = next((x for x in sec["link_targets"] if x in sec_by_id), None)
        prose_sec = sec_by_id.get(prose_id)
        prose_rules = prose_sec["rules"] if prose_sec else []

        phase_mode = (len(t["items"]) == len(prose_rules)
                      and all(r["children"] for r in prose_rules))

        steps = []
        for i, item in enumerate(t["items"]):
            text = render_plain(item["tokens"])
            step = {"number": str(i + 1), "id": None, "text": text,
                    "substeps": []}
            # The appendix lists are NSG's own compressed rendering of the
            # prose step rules, with identical (phase, letter) positions —
            # positional linkage is therefore authoritative. The similarity
            # score is only an informational tripwire for future CR versions:
            # a low score is reported for human review but the link is kept.
            prule = prose_rules[i] if i < len(prose_rules) else None
            if prule is not None:
                ptext = (render_plain(prule["header_tokens"])
                         if prule["header_tokens"] else render_plain(prule["tokens"]))
                step["id"] = prule["id"]
                if not phase_mode and sim(text, ptext) < 0.18:
                    warn("REVIEW %s step %d %r positionally linked to %s %r "
                         "(low text similarity)"
                         % (skey, i + 1, text[:40], prule["id"], ptext[:40]))
            prose_subs = prule["children"] if (prule and phase_mode) else []
            for j, sub in enumerate(item["children"]):
                stext = render_plain(sub["tokens"])
                sstep = {"number": LETTERS[j], "id": None, "text": stext,
                         "substeps": []}
                psub = prose_subs[j] if j < len(prose_subs) else None
                if psub is not None:
                    sstep["id"] = psub["id"]
                    if sim(stext, render_plain(psub["tokens"])) < 0.15:
                        warn("REVIEW %s step %d.%s %r positionally linked to "
                             "%s %r (low text similarity)"
                             % (skey, i + 1, LETTERS[j], stext[:40], psub["id"],
                                render_plain(psub["tokens"])[:40]))
                for kk, sub2 in enumerate(sub["children"]):
                    sstep["substeps"].append({
                        "number": ROMANS[kk],
                        "id": None,
                        "text": render_plain(sub2["tokens"]),
                        "substeps": [],
                    })
                step["substeps"].append(sstep)
            steps.append(step)
        structures.append({
            "structure": skey,
            "source_section": sec["number"],
            "source_section_id": sec["id"],
            "title": sec["title"],
            "prose_section": prose_sec["number"] if prose_sec else None,
            "prose_section_id": prose_id,
            "steps": steps,
        })
    return structures


# ---------------------------------------------------------------------------
# Output writers
# ---------------------------------------------------------------------------

def rule_plain_text(rec):
    if rec["header_tokens"] is not None:
        return render_plain(rec["header_tokens"])
    return render_plain(rec["tokens"])


def build_index(parser):
    entries = []
    ex_counter = {}
    examples = []
    for rec in parser.rules:
        sec = rec["section"] or {}
        terms = harvest(rec["tokens"], "term")
        cards = harvest(rec["tokens"], "card")
        is_header = rec["header_tokens"] is not None
        ex_ids = []
        for ex in rec["examples"]:
            n = ex_counter.get(rec["id"], 0) + 1
            ex_counter[rec["id"]] = n
            eid = "example_%s_%d" % (rec["id"], n)
            ex_ids.append(eid)
            examples.append({
                "id": eid,
                "example_number_or_label": "Example",
                "rule_id": rec["id"],
                "rule_number": rec["number"],
                "section_id": sec.get("id"),
                "section_number": sec.get("number"),
                "section_title": sec.get("title"),
                "text": render_plain(ex["tokens"]),
                "cards_referenced": harvest(ex["tokens"], "card"),
                "terms": harvest(ex["tokens"], "term"),
            })
        entries.append({
            "id": rec["id"],
            "number": rec["number"],
            "label": rec["label"],
            "kind": rec["kind"],
            "level": 1 if rec["kind"] == "rule" else 2,
            "is_header": is_header,
            "section_id": sec.get("id"),
            "section_number": sec.get("number"),
            "section_title": sec.get("title"),
            "text": rule_plain_text(rec),
            "parent_id": rec["parent"]["id"] if rec["parent"] else None,
            "children": [c["id"] for c in rec["children"]],
            "terms": terms,
            "card_refs": cards,
            "examples": ex_ids,
        })
    return entries, examples


def build_glossary(index_entries):
    gl = {}
    for e in index_entries:
        for t in e["terms"]:
            gl.setdefault(t, []).append(e["id"])
    return [{"term": t, "rule_ids": ids} for t, ids in sorted(gl.items())]


def write_markdown(path, parser, timing_structs, version):
    known_ids = set()
    for s in parser.sections:
        known_ids.add(s["id"])
    for r in parser.rules:
        known_ids.add(r["id"])
    for c in parser.chapters:
        known_ids.add(c["id"])

    L = []
    L.append("# Netrunner Comprehensive Rules v%s" % version)
    L.append("")
    L.append("> Source: [%s](%s) — Null Signal Games, Comprehensive Rules v%s." % (SOURCE_URL, SOURCE_URL, version))
    L.append("> Reproduced in-repo for implementation-conformance purposes: jinteki-rs")
    L.append("> engine code cites the stable rule anchors below (e.g. `rule_checkpoints`),")
    L.append("> and traceability tests consult `cr-index.json`, which is generated from the")
    L.append("> same HTML by `tools/extract-cr.py`. Netrunner and its rules text are the")
    L.append("> property of their respective owners; this copy is for reference only.")
    L.append("")

    def emit_rule(rec, depth):
        ind = "    " * depth
        anchor = '<a id="%s"></a>' % rec["id"]
        if rec["header_tokens"] is not None:
            title = render_md(rec["header_tokens"], known_ids)
            L.append("%s- %s**%s** ***%s***" % (ind, anchor, rec["label"], title))
        else:
            body = render_md(rec["tokens"], known_ids)
            L.append("%s- %s**%s** %s" % (ind, anchor, rec["label"], body))
        for ex in rec["examples"]:
            t = render_md(ex["tokens"], known_ids)
            t = re.sub(r"^Example:\s*", "", t)
            L.append("%s    - > *Example:* %s" % (ind, t))
        for ch in rec["children"]:
            emit_rule(ch, depth + 1)

    timing_by_sec = {t["source_section_id"]: t for t in timing_structs}

    for ch in parser.chapters:
        L.append("")
        L.append('# <a id="%s"></a>%s. %s' % (ch["id"], ch["number"], ch["title"]))
        for sec in ch["sections"]:
            L.append("")
            L.append('## <a id="%s"></a>%s. %s' % (sec["id"], sec["number"], sec["title"]))
            L.append("")
            for rec in sec["rules"]:
                emit_rule(rec, 0)
            ts = timing_by_sec.get(sec["id"])
            if ts:
                def emit_step(st, depth):
                    ind = "    " * depth
                    ref = " *(→ [%s](#%s))*" % (st["id"], st["id"]) if st["id"] else ""
                    L.append("%s1. %s%s" % (ind, st["text"], ref))
                    for ss in st["substeps"]:
                        emit_step(ss, depth + 1)
                for st in ts["steps"]:
                    emit_step(st, 0)
    L.append("")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(L))


# ---------------------------------------------------------------------------

def detect_version(html, html_path):
    m = re.search(r"<title>([^<]*)</title>", html)
    if m:
        vm = re.search(r"v(\d+\.\d+)", m.group(1))
        if vm:
            return vm.group(1)
    vm = re.search(r"v(\d+\.\d+)", os.path.basename(html_path))
    if vm:
        return vm.group(1)
    return "unknown"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("html", help="path to saved Comprehensive Rules HTML")
    ap.add_argument("--out", default="docs/rules", help="output directory")
    args = ap.parse_args()

    with open(args.html, encoding="utf-8") as f:
        html = f.read()
    version = detect_version(html, args.html)

    parser = CRParser()
    parser.feed(html)
    parser.close()
    assign_numbers(parser)

    warnings = list(parser.warnings)
    timing_structs = build_timing(parser, warnings.append)
    index_entries, examples = build_index(parser)
    glossary = build_glossary(index_entries)

    os.makedirs(args.out, exist_ok=True)
    meta = {
        "source_url": SOURCE_URL,
        "version": version,
        "generator": "tools/extract-cr.py",
    }

    def jdump(name, payload):
        p = os.path.join(args.out, name)
        with open(p, "w", encoding="utf-8") as f:
            json.dump(payload, f, indent=1, ensure_ascii=False)
            f.write("\n")
        return p

    p_index = jdump("cr-index.json", {**meta, "rules": index_entries})
    p_timing = jdump("timing-structures.json", {**meta, "structures": timing_structs})
    p_examples = jdump("examples.json", {**meta, "examples": examples})
    p_gloss = jdump("cr-glossary.json", {**meta, "glossary": glossary})
    p_md = os.path.join(args.out, "CR-v%s.md" % version)
    write_markdown(p_md, parser, timing_structs, version)

    # ------------------------------------------------------------------
    # verification: re-load our own JSON and report counts
    # ------------------------------------------------------------------
    with open(p_index, encoding="utf-8") as f:
        idx = json.load(f)["rules"]
    n_rules = sum(1 for e in idx if e["kind"] == "rule")
    n_subs = sum(1 for e in idx if e["kind"] == "subrule")
    print("version: %s" % version)
    print("chapters: %d  sections: %d" % (len(parser.chapters), len(parser.sections)))
    print("rules: %d  subrules: %d  total index entries: %d"
          % (n_rules, n_subs, len(idx)))
    print("examples: %d" % len(examples))
    print("glossary terms: %d" % len(glossary))
    empty = [e["id"] for e in idx if not e["text"]]
    if empty:
        print("EMPTY TEXT entries: %s" % empty)
    for t in timing_structs:
        n1 = len(t["steps"])
        n2 = sum(len(s["substeps"]) for s in t["steps"])
        n3 = sum(len(ss["substeps"]) for s in t["steps"] for ss in s["substeps"])
        linked = sum(1 for s in t["steps"] if s["id"]) + \
            sum(1 for s in t["steps"] for ss in s["substeps"] if ss["id"])
        print("timing %-11s steps L1=%d L2=%d L3=%d  prose-linked=%d"
              % (t["structure"], n1, n2, n3, linked))
    per_ch = {}
    for e in idx:
        chn = (e["section_number"] or "?").split(".")[0]
        per_ch[chn] = per_ch.get(chn, 0) + 1
    print("index entries per chapter:",
          " ".join("%s:%d" % kv for kv in sorted(per_ch.items(), key=lambda x: int(x[0]))))
    exch = {}
    for e in examples:
        chn = (e["section_number"] or "?").split(".")[0]
        exch[chn] = exch.get(chn, 0) + 1
    print("examples per chapter:",
          " ".join("%s:%d" % kv for kv in sorted(exch.items(), key=lambda x: int(x[0]))))
    if warnings:
        print("\nWARNINGS (%d):" % len(warnings))
        for w in warnings:
            print("  -", w)
    print("\nwrote:")
    for p in (p_md, p_index, p_timing, p_examples, p_gloss):
        print("  %s (%d bytes)" % (p, os.path.getsize(p)))


if __name__ == "__main__":
    main()
