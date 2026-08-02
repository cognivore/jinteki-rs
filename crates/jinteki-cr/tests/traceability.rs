//! SYS-F-10 traceability: every `cite!("rule_id")` in the crate must name a
//! rule id that exists in docs/rules/cr-index.json; coverage is the DP-7b
//! odometer.

use std::collections::BTreeSet;

const CR_INDEX: &str = include_str!("../../../docs/rules/cr-index.json");

fn index_ids() -> BTreeSet<String> {
    let v: serde_json::Value = serde_json::from_str(CR_INDEX).expect("cr-index.json parses");
    let mut ids = BTreeSet::new();
    for r in v["rules"].as_array().expect("rules array") {
        if let Some(id) = r["id"].as_str() {
            ids.insert(id.to_string());
        }
        // Section ids are legitimate citation targets too (headers carry
        // section_id).
        if let Some(id) = r["section_id"].as_str() {
            ids.insert(id.to_string());
        }
        if let Some(id) = r["parent_id"].as_str() {
            ids.insert(id.to_string());
        }
    }
    ids
}

/// Every cited id exists in the extracted rule index. Fails naming each
/// offender and its module.
#[test]
fn all_cited_rule_ids_exist() {
    let ids = index_ids();
    let mut bad: Vec<String> = Vec::new();
    for c in jinteki_cr::cite::registry() {
        if !ids.contains(&c.rule_id) {
            bad.push(format!("{}: cite!(\"{}\")", c.module, c.rule_id));
        }
    }
    bad.sort();
    bad.dedup();
    assert!(
        bad.is_empty(),
        "citations of nonexistent CR rule ids:\n{}",
        bad.join("\n")
    );
}

/// The DP-7b odometer: distinct rules cited / total rules in the index.
#[test]
fn report_rule_coverage() {
    let v: serde_json::Value = serde_json::from_str(CR_INDEX).unwrap();
    let total = v["rules"].as_array().unwrap().len();
    let ids = index_ids();
    let cited: BTreeSet<String> = jinteki_cr::cite::cited_rule_ids()
        .into_iter()
        .filter(|c| ids.contains(c))
        .collect();
    println!(
        "DP-7b odometer: {} distinct CR rules cited / {} rules in index ({:.1}%)",
        cited.len(),
        total,
        100.0 * cited.len() as f64 / total as f64
    );
    // Birth floor: the kernel wave must cite a real body of rules.
    assert!(cited.len() >= 100, "kernel wave cites at least 100 rules");
}

/// Every step id in timing-structures.json has executable semantics, and the
/// tables load (SYS-F-9: step tables as data).
#[test]
fn timing_tables_fully_mapped() {
    for table in jinteki_cr::timing::load_tables() {
        for step in &table.steps {
            // Panics inside step_op on an unmapped id.
            let _ = jinteki_cr::timing::step_op(table.kind, &step.id);
        }
    }
}
