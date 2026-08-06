//! The subtype guard: what stops the next mechanic repeating the last one.
//!
//! CR 2.16 subtypes were `&'static str` everywhere. The card layer spelled
//! them as printed ("Region", "Icebreaker", "Code Gate"); several kernel
//! sites spelled them lowercase. A lowercase literal matches no real card, so
//! the rule reading it silently never fired, and the kernel tests passed
//! because the testkit fixtures spelled them lowercase too — the fixture
//! agreed with the defect. Two rules were dead that way (CR 3.6.5 and CR
//! 3.9.5b) and nothing in the build could have said so.
//!
//! [`Subtype`] is now a closed enum, so the typed crates cannot express the
//! bug at all. This file pins the three things the type system alone does not:
//!
//! 1. that every variant is spelled exactly as NSG spells it,
//! 2. that every subtype NSG defines HAS a variant — so when Trojan, Bioroid,
//!    Alliance or whatever ships next appears upstream, adding it is forced
//!    rather than remembered,
//! 3. that no subtype is named as a string in a subtype position ANYWHERE in
//!    the workspace — including `jinteki-core`, which still stores subtypes as
//!    strings and cannot see this enum.
//!
//! ## The canonical list, and how to refresh it
//!
//! `crates/jinteki-cr/data/card_subtypes.json` is vendored verbatim from
//! NSG's `netrunner-cards-json`, `v2/card_subtypes.json`. It is vendored
//! rather than read from `~/Github/jinteki/netrunner-cards-json` because this
//! crate must not reach outside the workspace — a path out of the tree would
//! break the moment the card data consolidates into the crates. To refresh:
//! copy that file over this one, run these tests, and add the variants the
//! JSON-to-enum test names.

use jinteki_cr::Subtype;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The vendored list, as `(id, name)` pairs. Parsed with `serde_json` rather
/// than a derive so the test reads the file the way a human checks it.
fn canonical() -> Vec<(String, String)> {
    let raw = include_str!("../data/card_subtypes.json");
    let v: serde_json::Value = serde_json::from_str(raw).expect("vendored card_subtypes.json parses");
    v.as_array()
        .expect("card_subtypes.json is an array")
        .iter()
        .map(|e| {
            (
                e["id"].as_str().expect("every entry has an id").to_string(),
                e["name"].as_str().expect("every entry has a name").to_string(),
            )
        })
        .collect()
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is <workspace>/crates/jinteki-cr.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels under the workspace root")
        .to_path_buf()
}

/// Every `.rs` file under `crates/`, skipping build output.
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

// ---------------------------------------------------------------------------
// 1 + 2: the enum and NSG's list are the same list.
// ---------------------------------------------------------------------------

#[test]
fn every_variant_is_spelled_exactly_as_nsg_spells_it() {
    let names: BTreeSet<&str> = canonical().iter().map(|(_, n)| n.clone()).collect::<Vec<_>>().leak().iter().map(|s| s.as_str()).collect();
    for t in Subtype::ALL {
        assert!(
            names.contains(t.as_str()),
            "Subtype::{t:?} spells itself {:?}, which is not a subtype in NSG's \
             card_subtypes.json. CR 2.16 subtypes are matched by exact string \
             against real card data, so a spelling NSG does not use can never \
             match a printed card.",
            t.as_str()
        );
    }
}

#[test]
fn every_subtype_nsg_defines_has_a_variant() {
    let have: BTreeSet<&str> = Subtype::ALL.iter().map(|t| t.as_str()).collect();
    let missing: Vec<String> = canonical()
        .into_iter()
        .map(|(_, n)| n)
        .filter(|n| !have.contains(n.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "card_subtypes.json defines subtypes with no Subtype variant: {missing:?}. \
         A card layer cannot name these at all, so any rule about them is \
         unwritable. Add a variant per name to crates/jinteki-cr/src/subtype.rs.",
    );
}

#[test]
fn the_variant_count_matches_the_vendored_list() {
    assert_eq!(
        Subtype::ALL.len(),
        canonical().len(),
        "Subtype::ALL and the vendored card_subtypes.json disagree on how many \
         subtypes exist; one of them was edited without the other",
    );
}

#[test]
fn as_str_round_trips_through_from_canonical() {
    for t in Subtype::ALL {
        assert_eq!(
            Subtype::from_canonical(t.as_str()),
            Some(*t),
            "Subtype::{t:?} must parse back from its own canonical spelling",
        );
    }
    // Deliberately case-sensitive: "icebreaker" is NOT a subtype. This is the
    // exact literal that made CR 3.9.5b dead code, and it must stay rejected.
    assert_eq!(
        Subtype::from_canonical("icebreaker"),
        None,
        "from_canonical is exact-case: a lowercase spelling matches no printed card",
    );
    assert_eq!(Subtype::from_canonical("region"), None, "likewise for CR 3.6.5's subtype");
    assert_eq!(Subtype::from_canonical("Nonsense"), None, "and for a subtype that does not exist");
}

// ---------------------------------------------------------------------------
// 3: no subtype named as a string in a subtype position, workspace-wide.
// ---------------------------------------------------------------------------

/// The syntax that puts a value in a CR 2.16 subtype position. `jinteki-cr`
/// and `jinteki-cards` are typed and so cannot put a string here at all; this
/// list exists for `jinteki-core`, which still stores subtypes as
/// `&'static [&'static str]`, and as a tripwire for any new API that takes a
/// subtype by string.
const SUBTYPE_POSITIONS: &[&str] = &[
    // storage
    "subtypes: &[",
    "subtypes: vec![",
    "subtypes = vec![",
    "subtypes.push(",
    "of_subtypes: vec![",
    // criteria and stipulations
    "has_subtype(",
    "HasSubtype(",
    "HasAnySubtype(",
    "required_subtype:",
    "with_subtype(",
    "with_any_subtype(",
    // trigger conditions
    "PlayOperationWithSubtype(",
    "encounters_a(",
    "plays_a_subtyped(",
    "installs_a_subtyped(",
    "corp_rezzes_a_subtyped(",
    "can_interface_with_the_encountered(",
    // modification and naming
    "AddSubtype(",
    "RemoveSubtype(",
    "gains_subtypes(",
    "name_the_subtype(",
    "name_one_of_these_subtypes(",
    "name_one_of_these_subtypes_for(",
    "ChoiceSpec::Subtype(",
    "ChoiceValue::Subtype(",
    // Going around the type by comparing the canonical spelling to a string.
    // `has_subtype(t, "icebreaker")` no longer compiles, so this is the only
    // shape the original defect could still be written in.
    "as_str() ==",
    "as_str()==",
    "eq_ignore_ascii_case(",
    // testkit shapes
    "subtyped_ice(",
    "subtyped_etr_ice(",
    "morph_ice(",
    "pelangi_like(",
    "warden_fatuma_like(",
    "paid_once_per_turn_during_encounters_with(",
];

/// From `at`, the text up to the point the brackets opened there balance —
/// capped, so a marker that opens nothing reads only its own line.
fn position_text(src: &str, at: usize) -> &str {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut i = at;
    let cap = (at + 600).min(src.len());
    while i < cap {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => {
                depth -= 1;
                if depth <= 0 {
                    i += 1;
                    break;
                }
            }
            b'\n' if depth <= 0 => break,
            _ => {}
        }
        i += 1;
    }
    while !src.is_char_boundary(i) {
        i += 1;
    }
    &src[at..i]
}

/// Every string literal in `s`, naive but sufficient: subtype positions hold
/// plain literals, never escapes.
fn string_literals(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find('"') {
        let after = &rest[a + 1..];
        let Some(b) = after.find('"') else { break };
        out.push(&after[..b]);
        rest = &after[b + 1..];
    }
    out
}

/// THE GUARD. Every subtype named anywhere in the workspace exists in NSG's
/// list with exact casing.
///
/// A string in a subtype position is judged only if it LOOKS like a subtype —
/// it matches a canonical name ignoring case. That is what makes the test
/// precise: a card name passed alongside a subtype ("Lycan-like") is ignored,
/// while "icebreaker", "region", "code gate" — the literals that made real
/// rules dead — are exactly what it rejects.
#[test]
fn every_subtype_named_anywhere_in_the_workspace_is_canonical() {
    let canon = canonical();
    let exact: BTreeSet<&str> = canon.iter().map(|(_, n)| n.as_str()).collect();
    let lower: Vec<(String, &str)> =
        canon.iter().map(|(_, n)| (n.to_lowercase(), n.as_str())).collect();

    let mut offences: Vec<String> = Vec::new();
    let mut positions_seen = 0usize;

    for path in rust_sources() {
        // This file names bad spellings on purpose.
        if path.ends_with("tests/subtypes.rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let rel = path.strip_prefix(workspace_root()).unwrap_or(&path).display().to_string();

        for marker in SUBTYPE_POSITIONS {
            let mut from = 0usize;
            while let Some(off) = src[from..].find(marker) {
                let at = from + off + marker.len() - 1;
                from = from + off + marker.len();
                positions_seen += 1;
                let text = position_text(&src, at);
                for lit in string_literals(text) {
                    if exact.contains(lit) {
                        continue;
                    }
                    let low = lit.to_lowercase();
                    if let Some((_, want)) = lower.iter().find(|(l, _)| *l == low) {
                        let line = src[..at].matches('\n').count() + 1;
                        offences.push(format!(
                            "{rel}:{line}: {marker}… names the subtype {lit:?}, but NSG \
                             spells it {want:?}. A subtype is matched by exact string \
                             against printed card data, so {lit:?} matches no card and \
                             the rule reading it silently never fires."
                        ));
                    }
                }
                from = from.max(at + 1);
            }
        }
    }

    assert!(
        positions_seen > 100,
        "only {positions_seen} subtype positions found — the scan is not looking at the tree"
    );
    assert!(
        offences.is_empty(),
        "{} subtype(s) named with the wrong casing:\n  {}",
        offences.len(),
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

/// The other half of the guard: a subtype-named field or parameter in the
/// TYPED crates must not be a string again. Adding `fn foo(subtype: &str)`
/// would reopen the whole class one call site at a time.
#[test]
fn no_subtype_position_in_the_typed_crates_is_declared_as_a_string() {
    let root = workspace_root();
    let mut offences = Vec::new();
    for path in rust_sources() {
        let rel = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
        if !(rel.starts_with("crates/jinteki-cr/") || rel.starts_with("crates/jinteki-cards/")) {
            continue;
        }
        if rel.ends_with("tests/subtypes.rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        for (i, line) in src.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            // A declaration whose binder names a subtype, judged on the TYPE
            // that follows that binder and not on the rest of the line — a
            // helper may perfectly well take a card name or a choice key as a
            // string alongside a properly typed subtype.
            for key in ["subtype:", "subtypes:", "of_subtypes:", "required_subtype:"] {
                let Some(off) = code.find(key) else { continue };
                let ty = type_after(&code[off + key.len()..]);
                if ty.contains("str") || ty.contains("String") {
                    offences.push(format!("{rel}:{}: {} — the subtype is typed {ty:?}", i + 1, code.trim()));
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "these subtype positions are typed as strings again — use jinteki_cr::Subtype, \
         which is the only spelling of a subtype in the tree:\n  {}",
        offences.join("\n  "),
    );
}
