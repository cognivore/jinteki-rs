//! The format guard: what stops the deck metadata from lying.
//!
//! We serve Eternal and only Eternal. Every deck we carry nevertheless records
//! the narrowest tournament format its printed list would be legal in, so that
//! a Standard or Startup shelf can be built later by reading a field instead
//! of re-deriving a card pool. A recorded fact nothing checks is a rumour, so
//! this file recomputes all of it from NSG's own data on every build.
//!
//! Format legality is a TOURNAMENT construct. The Comprehensive Rules settle
//! it before the game begins (CR 1.4.2) and then say nothing more about it, so
//! nothing here cites a CR rule — there is no CR rule to cite for any of it.
//!
//! ## The data, and why it is vendored
//!
//! `crates/jinteki-server/data/nsg-v2/` holds `formats/`, `card_pools/`,
//! `restrictions/`, `printings/` and `card_sets.json`, copied verbatim and
//! byte-identical from NSG's `netrunner-cards-json` `v2/` tree (upstream
//! 51e7c6d9 — the same commit `crates/jinteki-core/carddata/formats.json`
//! records in its `_provenance`). Vendored for the same reason
//! `crates/jinteki-cr/data/card_subtypes.json` is: nothing in this workspace
//! may reach outside it, and a path to `~/Github/jinteki/netrunner-cards-json`
//! would break the moment the card data consolidates into the crates. To
//! refresh: `cp -R` the five paths over these and rerun this file.
//!
//! ## Which snapshot is "current"
//!
//! A format is a dated series of snapshots, each naming a card pool and a
//! restriction list. Nothing in the files says "this one is now" in a way
//! worth trusting on its own, so this file takes TWO independent readings and
//! insists they agree about the pool:
//!
//!   * the snapshot with the greatest `date_start` that is not explicitly
//!     `"active": false`, and
//!   * the snapshot flagged `"active": true`.
//!
//! They disagree about the SNAPSHOT for `standard` and `startup` — the flag is
//! behind the dates, which is why the reading is not simply "trust the flag"
//! (`tools/gen-carddata.py` does trust it, and gets away with it only because
//! `eternal`'s flag is on its latest snapshot). They agree about the CARD POOL
//! for every format, which is the only thing this computation needs, and
//! [`the_two_readings_of_current_agree_about_the_pool`] is what keeps that
//! true.

use jinteki_server::cr::{self, DeckSpec};
use jinteki_server::format::Format;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("nsg-v2")
}

fn read_json(path: &Path) -> Value {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("vendored {} is readable: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("vendored {} is valid JSON: {e}", path.display()))
}

fn read_dir_json(sub: &str) -> Vec<(String, Value)> {
    let dir = data_dir().join(sub);
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("vendored {} is a directory: {e}", dir.display()));
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            continue;
        }
        if p.extension().is_some_and(|x| x == "json") {
            let stem = p.file_stem().unwrap().to_string_lossy().to_string();
            out.push((stem, read_json(&p)));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn str_at<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_else(|| panic!("{v} has a string {key:?}"))
}

// ---------------------------------------------------------------------------
// NSG's format list, and the snapshot in force
// ---------------------------------------------------------------------------

/// Every format NSG defines, as `(id, name)`, read from `formats/`.
fn canonical_formats() -> Vec<(String, String)> {
    read_dir_json("formats")
        .into_iter()
        .map(|(stem, v)| {
            let id = str_at(&v, "id").to_string();
            assert_eq!(id, stem, "formats/{stem}.json declares a different id");
            (id, str_at(&v, "name").to_string())
        })
        .collect()
}

/// One snapshot, reduced to what decides a pool.
#[derive(Debug, Clone)]
struct Snapshot {
    id: String,
    date_start: String,
    card_pool_id: String,
    restriction_id: Option<String>,
}

fn snapshots(format_id: &str) -> Vec<Snapshot> {
    let v = read_json(&data_dir().join("formats").join(format!("{format_id}.json")));
    v["snapshots"]
        .as_array()
        .expect("a format has an array of snapshots")
        .iter()
        .map(|s| Snapshot {
            id: str_at(s, "id").to_string(),
            date_start: str_at(s, "date_start").to_string(),
            card_pool_id: str_at(s, "card_pool_id").to_string(),
            restriction_id: s["restriction_id"].as_str().map(str::to_string),
        })
        .collect()
}

/// Reading 1: the latest snapshot that has not been withdrawn. `date_start`
/// is ISO-8601, so lexicographic order IS chronological order; the id breaks
/// a same-day tie deterministically.
fn latest_by_date(format_id: &str) -> Snapshot {
    let v = read_json(&data_dir().join("formats").join(format!("{format_id}.json")));
    let live: Vec<(usize, Snapshot)> = v["snapshots"]
        .as_array()
        .unwrap()
        .iter()
        .zip(snapshots(format_id))
        .filter(|(raw, _)| raw["active"] != Value::Bool(false))
        .map(|(_, s)| s)
        .enumerate()
        .collect();
    assert!(!live.is_empty(), "{format_id} has no snapshot that is not withdrawn");
    live.into_iter()
        .max_by(|a, b| (&a.1.date_start, &a.1.id).cmp(&(&b.1.date_start, &b.1.id)))
        .unwrap()
        .1
}

/// Reading 2: the snapshot NSG flags. `None` when the flag is absent or
/// ambiguous rather than guessing.
fn flagged_active(format_id: &str) -> Option<Snapshot> {
    let v = read_json(&data_dir().join("formats").join(format!("{format_id}.json")));
    let flagged: Vec<Snapshot> = v["snapshots"]
        .as_array()
        .unwrap()
        .iter()
        .zip(snapshots(format_id))
        .filter(|(raw, _)| raw["active"] == Value::Bool(true))
        .map(|(_, s)| s)
        .collect();
    (flagged.len() == 1).then(|| flagged.into_iter().next().unwrap())
}

// ---------------------------------------------------------------------------
// A pool, expanded to concrete card ids
// ---------------------------------------------------------------------------

/// `card_set_id → the v2 card ids printed in it`. A card is in a pool if ANY
/// of its printings is in a set the pool names, which is why this is the
/// printings join and not `cards/`: a v2 card file carries no set membership.
fn cards_by_set() -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (stem, v) in read_dir_json("printings") {
        let e = out.entry(stem.clone()).or_default();
        for p in v.as_array().expect("a printings file is an array") {
            assert_eq!(
                str_at(p, "card_set_id"),
                stem,
                "printings/{stem}.json holds a printing from another set"
            );
            e.insert(str_at(p, "card_id").to_string());
        }
    }
    out
}

/// The set ids a pool covers: the ones it names, plus every set belonging to
/// a cycle it names.
fn sets_in_pool(format_id: &str, card_pool_id: &str) -> BTreeSet<String> {
    let pools = read_json(&data_dir().join("card_pools").join(format!("{format_id}.json")));
    let pool = pools
        .as_array()
        .expect("a card-pool file is an array")
        .iter()
        .find(|p| str_at(p, "id") == card_pool_id)
        .unwrap_or_else(|| panic!("card pool {card_pool_id:?} is not in card_pools/{format_id}.json"));
    let mut set_ids: BTreeSet<String> = pool["card_set_ids"]
        .as_array()
        .map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect())
        .unwrap_or_default();
    let cycles: BTreeSet<&str> = pool["card_cycle_ids"]
        .as_array()
        .map(|a| a.iter().map(|s| s.as_str().unwrap()).collect())
        .unwrap_or_default();
    for s in read_json(&data_dir().join("card_sets.json")).as_array().unwrap() {
        if cycles.contains(str_at(s, "card_cycle_id")) {
            set_ids.insert(str_at(s, "id").to_string());
        }
    }
    set_ids
}

fn pool_cards(format_id: &str, card_pool_id: &str, by_set: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for sid in sets_in_pool(format_id, card_pool_id) {
        let cards = by_set
            .get(&sid)
            .unwrap_or_else(|| panic!("pool {card_pool_id:?} names set {sid:?} with no printings file"));
        out.extend(cards.iter().cloned());
    }
    out
}

/// The current pool of every format NSG defines, keyed by format id.
fn current_pools() -> BTreeMap<String, BTreeSet<String>> {
    let by_set = cards_by_set();
    canonical_formats()
        .into_iter()
        .map(|(id, _)| {
            let snap = latest_by_date(&id);
            let cards = pool_cards(&id, &snap.card_pool_id, &by_set);
            (id, cards)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1 + 2 + 3: the enum and NSG's list are the same list.
// ---------------------------------------------------------------------------

#[test]
fn every_variant_is_spelled_exactly_as_nsg_spells_it() {
    let canon = canonical_formats();
    let ids: BTreeSet<&str> = canon.iter().map(|(i, _)| i.as_str()).collect();
    for f in Format::ALL {
        assert!(
            ids.contains(f.as_str()),
            "Format::{f:?} spells itself {:?}, which is not a format id in NSG's \
             formats/ directory. That id is the vocabulary card_pools/, \
             restrictions/ and every decklist's mwl_code speak, so a spelling \
             NSG does not use joins to nothing.",
            f.as_str()
        );
        let (_, name) = canon.iter().find(|(i, _)| i == f.as_str()).unwrap();
        assert_eq!(
            f.display_name(),
            name,
            "Format::{f:?} shows itself as {:?}, but NSG names it {name:?}",
            f.display_name()
        );
    }
}

#[test]
fn every_format_nsg_defines_has_a_variant() {
    let have: BTreeSet<&str> = Format::ALL.iter().map(|f| f.as_str()).collect();
    let missing: Vec<String> = canonical_formats()
        .into_iter()
        .map(|(i, _)| i)
        .filter(|i| !have.contains(i.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "NSG defines formats with no Format variant: {missing:?}. Until one \
         exists, no deck can be recorded as belonging to them. Add a variant \
         per id to crates/jinteki-server/src/format.rs — and if the new pool \
         is on the containment chain, add it to Format::CHAIN too.",
    );
}

#[test]
fn the_variant_count_matches_the_vendored_list() {
    assert_eq!(
        Format::ALL.len(),
        canonical_formats().len(),
        "Format::ALL and the vendored formats/ directory disagree on how many \
         formats exist; one of them was changed without the other",
    );
}

#[test]
fn as_str_round_trips_through_from_canonical() {
    for f in Format::ALL {
        assert_eq!(
            Format::from_canonical(f.as_str()),
            Some(*f),
            "Format::{f:?} must parse back from its own canonical id",
        );
    }
    // Deliberately case-sensitive, and deliberately not the display name:
    // "Standard" is what a human reads, "standard" is what the data says.
    assert_eq!(
        Format::from_canonical("Standard"),
        None,
        "from_canonical is exact-case over NSG's id, not its display name",
    );
    assert_eq!(
        Format::from_canonical("System Gateway"),
        None,
        "the id is system_gateway; the display name is not an id",
    );
    assert_eq!(Format::from_canonical("modern"), None, "and a format that does not exist");
}

// ---------------------------------------------------------------------------
// 4: which snapshot is current
// ---------------------------------------------------------------------------

#[test]
fn the_two_readings_of_current_agree_about_the_pool() {
    let mut notes = Vec::new();
    for (id, _) in canonical_formats() {
        let by_date = latest_by_date(&id);
        let Some(flag) = flagged_active(&id) else {
            notes.push(format!("{id}: no single snapshot is flagged active"));
            continue;
        };
        assert_eq!(
            by_date.card_pool_id, flag.card_pool_id,
            "{id}: the latest snapshot by date ({} of {}) is on card pool {:?}, but the \
             snapshot flagged active ({} of {}) is on {:?}. Every pool in this file is \
             derived from the first reading; while the two agreed, the choice did not \
             matter. It does now — decide which reading is right before trusting any \
             computed format.",
            by_date.id, by_date.date_start, by_date.card_pool_id,
            flag.id, flag.date_start, flag.card_pool_id,
        );
        if by_date.id != flag.id {
            notes.push(format!(
                "{id}: flag is on {} ({}, restriction {:?}) but {} ({}, restriction {:?}) \
                 has started — same pool, different banlist",
                flag.id, flag.date_start, flag.restriction_id,
                by_date.id, by_date.date_start, by_date.restriction_id,
            ));
        }
    }
    // Not an assertion: the flag lagging the dates is upstream's business and
    // costs us nothing while the pools agree. Printed so a `--nocapture` run
    // shows it rather than it being discovered again from scratch.
    for n in &notes {
        println!("note: {n}");
    }
}

// ---------------------------------------------------------------------------
// 5: the containment chain
// ---------------------------------------------------------------------------

#[test]
fn the_constructed_pools_form_the_chain_format_declares() {
    let pools = current_pools();

    // Every neighbouring pair on the chain, narrow inside wide.
    for pair in Format::CHAIN.windows(2) {
        let (narrow, wide) = (pair[0], pair[1]);
        let n = &pools[narrow.as_str()];
        let w = &pools[wide.as_str()];
        let escapees: Vec<&String> = n.difference(w).take(8).collect();
        assert!(
            n.is_subset(w),
            "Format::CHAIN claims {narrow} is narrower than {wide}, but {} of {}'s {} \
             cards are outside {}'s pool, e.g. {escapees:?}. The chain is what makes \
             \"narrowest format\" well defined; if it has broken, every recorded deck \
             format is meaningless until it is repaired.",
            n.difference(w).count(),
            narrow,
            n.len(),
            wide,
        );
        assert!(
            n.len() < w.len(),
            "{narrow} and {wide} have the same pool, so \"narrowest\" cannot choose \
             between them"
        );
    }

    // And the formats left OFF the chain are off it because they are not on
    // it — each a subset of exactly one chain member (eternal, the top) and
    // comparable to nothing else, so no total order includes them.
    let on_chain: BTreeSet<&str> = Format::CHAIN.iter().map(|f| f.as_str()).collect();
    for f in Format::ALL.iter().filter(|f| !on_chain.contains(f.as_str())) {
        let p = &pools[f.as_str()];
        let comparable: Vec<&str> = Format::CHAIN
            .iter()
            .filter(|c| p.is_subset(&pools[c.as_str()]) || pools[c.as_str()].is_subset(p))
            .map(|c| c.as_str())
            .collect();
        assert_eq!(
            comparable,
            vec![Format::Eternal.as_str()],
            "{f} is off Format::CHAIN because its pool is comparable only to eternal's, \
             but it now compares to {comparable:?}. If it has become a link in the \
             chain, put it on the chain; classification is defined over the chain only."
        );
    }
}

// ---------------------------------------------------------------------------
// 6: THE GUARD — every recorded deck format is the computed one
// ---------------------------------------------------------------------------

/// A deck's cards as NSG v2 ids: the list (identity included — the list
/// carries it) plus CR 1.5.4a's pile of additional identities, which a player
/// brings along with their deck and which is therefore as much a part of what
/// they put on the table as the deck is.
///
/// Titles are resolved through the card database's own title index, the same
/// join `eternal_decks::builtin_cards` uses. A title that does not resolve is
/// a failure, not a skip: a deck list naming a card nothing can find is a
/// defect whatever its format.
fn deck_card_ids(spec: &DeckSpec) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut unresolved = Vec::new();
    for title in spec.list.iter().map(|(t, _)| *t).chain(spec.pile.iter().copied()) {
        match jinteki_server::carddata::by_title(title).and_then(|c| c.nsg_id.clone()) {
            Some(id) => {
                out.insert(id);
            }
            None => unresolved.push(title),
        }
    }
    assert!(
        unresolved.is_empty(),
        "{}: these titles carry no NSG v2 id in the card database, so their format \
         cannot be computed at all: {unresolved:?}. Either the title is misspelled \
         (check the apostrophe — the card database is generated from the EDN, which \
         writes them straight) or the card data needs regenerating.",
        spec.key,
    );
    out
}

/// The narrowest format on the chain whose current pool holds every card.
///
/// Cards printed in NO constructed pool are set aside first, and named. The
/// class exemplar is Boris "Syfr" Kovac, whose first printed line is "Draft
/// format only." and who is in Andromeda's 1.5.4a pile: he exists in the
/// `draft` set, which no format's pool contains. Counting him would answer
/// "this deck belongs to no format", which is not a format — and CR 1.4.2
/// settles legality before the game begins, so he is simply not among the
/// identities brought to a constructed table. `cr::eternal_pile` already
/// drops him for exactly this reason.
fn narrowest_format(
    cards: &BTreeSet<String>,
    pools: &BTreeMap<String, BTreeSet<String>>,
) -> (Option<Format>, Vec<String>) {
    let mut universe = BTreeSet::new();
    for f in Format::CHAIN {
        universe.extend(pools[f.as_str()].iter().cloned());
    }
    let unconstructed: Vec<String> = cards.difference(&universe).cloned().collect();
    let considered: BTreeSet<String> = cards.intersection(&universe).cloned().collect();
    let found = Format::CHAIN
        .iter()
        .copied()
        .find(|f| considered.is_subset(&pools[f.as_str()]));
    (found, unconstructed)
}

#[test]
fn every_carried_decks_recorded_format_is_the_computed_one() {
    let pools = current_pools();
    let mut report = Vec::new();
    for spec in cr::carried_decks() {
        let cards = deck_card_ids(spec);
        let (computed, unconstructed) = narrowest_format(&cards, &pools);
        let computed = computed.unwrap_or_else(|| {
            panic!(
                "{}: no format on the chain holds every card, which cannot happen — \
                 eternal is the top of the chain and everything printed for \
                 constructed play is in it",
                spec.key
            )
        });
        assert_eq!(
            spec.format, computed,
            "{} ({}) records format {:?}, but its {} distinct cards fit {:?}. \
             Outside the recorded pool: {:?}. The recorded format is metadata a UI \
             will trust; it has to be the answer the cards give.",
            spec.key,
            spec.display_name,
            spec.format.as_str(),
            cards.len(),
            computed.as_str(),
            cards.difference(&pools[spec.format.as_str()]).take(8).collect::<Vec<_>>(),
        );
        report.push(format!(
            "{:<24} {:<9} {:>3} cards, {} set aside as unconstructed, mwl_code {:?}",
            spec.key,
            computed.as_str(),
            cards.len(),
            unconstructed.len(),
            spec.mwl_code,
        ));
    }
    for line in &report {
        println!("{line}");
    }
}

/// A card set aside as "in no constructed pool" must really be in no pool's
/// SETS — not merely absent from the pools we happened to look at. Otherwise
/// the set-aside step could quietly swallow a card that belongs somewhere.
#[test]
fn cards_set_aside_as_unconstructed_are_printed_only_outside_every_pool() {
    let pools = current_pools();
    let by_set = cards_by_set();
    let mut pooled_sets = BTreeSet::new();
    for (id, _) in canonical_formats() {
        pooled_sets.extend(sets_in_pool(&id, &latest_by_date(&id).card_pool_id));
    }
    for spec in cr::carried_decks() {
        let (_, unconstructed) = narrowest_format(&deck_card_ids(spec), &pools);
        for card in unconstructed {
            let printed_in: BTreeSet<&String> = by_set
                .iter()
                .filter(|(_, cards)| cards.contains(&card))
                .map(|(sid, _)| sid)
                .collect();
            assert!(
                !printed_in.is_empty(),
                "{}: {card:?} is in no printings file at all",
                spec.key
            );
            let pooled: Vec<&&String> = printed_in.iter().filter(|s| pooled_sets.contains(**s)).collect();
            assert!(
                pooled.is_empty(),
                "{}: {card:?} was set aside as belonging to no constructed pool, but it \
                 is printed in {pooled:?}, which some format's pool does contain. Setting \
                 it aside hid a card that should have decided the format.",
                spec.key,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7: the author's claim against the deck's contents
// ---------------------------------------------------------------------------

/// `mwl_code → the format the restriction it names belongs to`. NRDB writes
/// the code in kebab-case and the restriction files in snake_case; `format_id`
/// inside the file is what settles the format, not the directory it sits in
/// (upstream files the startup restrictions under `restrictions/standard/`).
fn format_of_mwl_code(code: &str) -> Option<Format> {
    let wanted = code.replace('-', "_");
    for sub in ["eternal", "snapshot", "standard", "startup"] {
        for (stem, v) in read_dir_json(&format!("restrictions/{sub}")) {
            if stem == wanted {
                assert_eq!(str_at(&v, "id"), stem, "restriction {stem} declares another id");
                return Format::from_canonical(str_at(&v, "format_id"));
            }
        }
    }
    None
}

#[test]
fn no_authors_banlist_is_narrower_than_the_deck_it_was_built_for() {
    let pools = current_pools();
    let rank = |f: Format| Format::CHAIN.iter().position(|c| *c == f);
    for spec in cr::carried_decks() {
        let Some(code) = spec.mwl_code else { continue };
        let claimed = format_of_mwl_code(code).unwrap_or_else(|| {
            panic!(
                "{}: mwl_code {code:?} names no restriction in the vendored \
                 restrictions/ tree, so the author's claim cannot be checked",
                spec.key
            )
        });
        let (computed, _) = narrowest_format(&deck_card_ids(spec), &pools);
        let computed = computed.expect("every deck fits eternal");
        let (Some(c), Some(w)) = (rank(computed), rank(claimed)) else {
            panic!("{}: {computed} or {claimed} is off the chain", spec.key)
        };
        assert!(
            w >= c,
            "{} was built against {code:?}, which is a {claimed} banlist, but its cards \
             need {computed}. The author's intent and the deck's contents contradict \
             each other; one of the two is wrong and the contents are the fact.",
            spec.key,
        );
        if w > c {
            // The interesting normal case, and not an error: a deck legal in a
            // narrower pool than its author's banlist covers.
            println!(
                "note: {} is {computed}-legal though built against a {claimed} banlist",
                spec.key
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8: no third stringly-typed piece of card vocabulary
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels under the workspace root")
        .to_path_buf()
}

fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&workspace_root().join("crates"), &mut out);
    out.sort();
    assert!(out.len() > 50, "the source walk found {} files, which cannot be right", out.len());
    out
}

/// The declaration side, in the spirit of
/// `crates/jinteki-cr/tests/subtypes.rs`'s
/// `no_subtype_position_in_the_typed_crates_is_declared_as_a_string`.
///
/// A `format`-named field or parameter that is a string is how the subtype
/// defect began, and `faction` is already the second instance in this tree.
/// The one string spelled `format` that survives is `decks::DeckRow.format`,
/// a mirror of a SQLite column — persistence, not vocabulary — and it is
/// named here rather than exempted silently. Everything else must be
/// [`Format`].
#[test]
fn no_format_position_in_the_card_vocabulary_is_declared_as_a_string() {
    /// `<file>:<line>` sites where a `format`-shaped string is persistence
    /// rather than vocabulary. Shrinking this list is progress; growing it
    /// needs a reason written down.
    const PERSISTENCE: &[&str] = &["crates/jinteki-server/src/decks.rs"];

    let root = workspace_root();
    let mut offences = Vec::new();
    for path in rust_sources() {
        let rel = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
        if rel.ends_with("tests/formats.rs") || PERSISTENCE.contains(&rel.as_str()) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        for (i, line) in src.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || code.starts_with("///") {
                continue;
            }
            for key in ["format:", "formats:"] {
                let Some(off) = code.find(key) else { continue };
                // `json!({"format": …})` is a wire payload, not a declaration.
                if off > 0 && code.as_bytes()[off - 1] == b'"' {
                    continue;
                }
                let ty = type_after(&code[off + key.len()..]);
                if ty.contains("str") || ty.contains("String") {
                    offences.push(format!("{rel}:{}: {} — the format is typed {ty:?}", i + 1, code.trim()));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "these format positions are typed as strings — use jinteki_server::format::Format, \
         which is the only spelling of a format in the tree. A string here is how CR \
         2.16's subtypes went wrong: a spelling nothing checks joins to nothing and \
         fails silently.\n  {}",
        offences.join("\n  "),
    );
}

/// The type written after a `name:` binder — up to the comma that ends this
/// field or parameter, ignoring commas nested inside generics or brackets.
fn type_after(s: &str) -> &str {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth <= 0 => return s[..i].trim(),
            _ => {}
        }
        if depth < 0 {
            return s[..i].trim();
        }
    }
    s.trim()
}
