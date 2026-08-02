# docs/rules — normative rules reference for jinteki-rs

Machine-readable, traceable extraction of the Null Signal Games **Netrunner
Comprehensive Rules** (CR). This directory is the ground truth the engine is
built against: VM primitives cite rule ids, traceability tests consult
`cr-index.json`, the state machine mirrors `timing-structures.json`, and CR
examples in `examples.json` are the seed corpus for regression tests.

## Provenance

- Source: https://rules.nullsignal.games/ — Comprehensive Rules **v26.03**,
  saved as HTML (`Netrunner Comprehensive Rules (v26.03).html`).
- Extracted by `tools/extract-cr.py` (stdlib-only Python 3). All ids
  (`sec_*`, `rule_*`, `subsec_*`, `step_*`) are the HTML element ids from the
  source document, unmodified — they are NSG's own stable anchors, so
  citations remain valid across our re-extractions.
- The rules text is the property of Null Signal Games / its respective
  owners; it is reproduced here solely for implementation-conformance
  purposes.

## Regenerating

```sh
python3 tools/extract-cr.py "/path/to/Netrunner Comprehensive Rules (vXX.YY).html" --out docs/rules
```

- No third-party dependencies; system `python3` suffices.
- The CR version is auto-detected from the document `<title>`, falling back
  to a `vXX.YY` pattern in the filename. The markdown output name embeds it
  (`CR-v26.03.md`), so a new CR version lands beside the old one — review the
  diff of the JSON artifacts, then remove the superseded markdown.
- The script prints verification counts (chapters, sections, rules,
  sub-rules, examples, per-chapter distributions, timing-structure step
  counts) and `REVIEW` warnings wherever the §11 appendix wording diverges
  far enough from the §5/§6/§7 prose step text that the positional
  cross-reference deserves a human glance. For v26.03 all 10 such warnings
  were manually verified correct (the appendix is NSG's compressed paraphrase
  of the prose steps; lettering is guaranteed aligned by the document
  structure).

## Artifacts

| file | what it is | consumed by |
|---|---|---|
| `CR-v26.03.md` | The complete CR as markdown: every chapter/section heading, every rule and sub-rule with its number and `<a id>` anchor (the HTML id), examples inline as block quotes, §11 step lists as ordered lists with prose-rule links. | humans; code review |
| `cr-index.json` | Flat array `rules[]` of every rule and sub-rule: `{id, number, label, kind, level, is_header, section_id, section_number, section_title, text, parent_id, children[], terms[], card_refs[], examples[]}`. `number` is the full composed number (sub-rule `a.` of `10.3.1.` → `10.3.1.a.`). `is_header` marks the 35 subsection-heading rules (they have a title instead of body text). | traceability tests; every VM primitive cites an `id` from here |
| `timing-structures.json` | The five §11 timing structures (`corp_turn`, `runner_turn`, `run`, `breach`, `access`) as ordered step trees `{number, id, text, substeps[]}`. `id` is the corresponding **prose** rule id from §5.6/§5.7/§6.9/§7.5/§7.2 (positional cross-reference; decision branches (`If yes…/If no…`) have no prose anchor and carry `id: null`). `prose_section_id` points at the authoritative prose section. | the engine's state machine |
| `examples.json` | Every worked example in the CR, verbatim (with symbol tokens and plain card names): `{id, example_number_or_label, rule_id, rule_number, section_id, section_number, section_title, text, cards_referenced[], terms[]}`. | executable regression-test corpus |
| `cr-glossary.json` | Every `<span class="Term">` occurrence: `{term, rule_ids[]}` — the rules in which the CR marks that term as being defined/introduced. | card DSL & docs tooling |
| `ability-model.md` | Hand-written engine-kernel digest of §9 (abilities), §10.3 (checkpoints), §1.16 (costs), §6/§7 (runs/breach/access) and §10.4–10.14 subsystems, with a rule-id citation for every claim and MECHANISM vs CONVENTION marking. Not generated; update by hand when the CR version changes. | implementers |

## Stable ids

- Rules/sub-rules/sections: NSG's own HTML ids, verbatim (`rule_checkpoints`,
  `step_corp_turn_mandatory_draw`, `sec_costs`, …).
- Examples have no ids in the HTML. Synthesised deterministically as
  `example_<rule_id>_<n>` where `<rule_id>` is the id of the rule or sub-rule
  the example is attached to and `<n>` is its 1-based position among that
  rule's examples in document order (e.g. `example_rule_chain_reaction_1`).
  Re-runs over the same document produce identical ids; a future CR version
  only shifts `<n>` if NSG inserts/reorders examples under the same rule.

## Symbol tokens

SVG glyphs in the source are rendered as bracketed tokens everywhere (text,
JSON, markdown): `[click] [credit] [link] [mu] [subroutine] [trash]
[trash-cost] [recurring] [interrupt]`.

## Known representational choices

- Card thumbnails are rendered as plain card names; the NetrunnerDB card id
  is captured in `examples.json`/`cr-index.json` only as names in
  `card_refs`/`cards_referenced` (ids available in the extractor if needed).
- Defined terms are **bold** in the markdown; subtypes are *italic*.
- The site chrome of the saved page (table of contents, header menu, "recent
  changes" changelog, link/check icons) is intentionally not extracted.
- Internal cross-references (`see section 9.6`) become markdown links to the
  in-document anchors; the same targets are not separately listed in the
  JSON.
