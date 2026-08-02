//! Citation discipline (SYS-F-10, DESIGN.md Amendment 1).
//!
//! Every VM primitive cites the CR rule id it implements via `cite!("rule_id")`.
//! The registry is *static*: `lib.rs` embeds every source file with
//! `include_str!`, and [`registry`] extracts each `cite!("…")` occurrence into
//! `(module, rule_id)` records at runtime with zero I/O. A test in
//! `tests/traceability.rs` parses `docs/rules/cr-index.json` and fails on any
//! cited id that does not exist there; a second test reports coverage
//! (cited distinct rules / total rules) — the DP-7b odometer.

/// One citation record: which module cited which CR rule id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cite {
    pub module: &'static str,
    pub rule_id: String,
}

/// Mark the enclosing primitive as implementing the named CR rule.
///
/// Expands to a compile-checked const so a typo'd non-literal fails to build;
/// the id itself is validated against `cr-index.json` by the traceability
/// test. Usable in statement position.
#[macro_export]
macro_rules! cite {
    ($id:literal) => {
        const _: &str = $id;
    };
}

/// Every embedded source file of this crate: `(module name, source text)`.
/// Kept in `lib.rs` via `include_str!` so the registry is part of the binary.
pub fn sources() -> &'static [(&'static str, &'static str)] {
    crate::EMBEDDED_SOURCES
}

/// Extract all `cite!("rule_id")` occurrences from the embedded sources.
pub fn registry() -> Vec<Cite> {
    let mut out = Vec::new();
    for (module, text) in sources() {
        let mut rest = *text;
        while let Some(pos) = rest.find("cite!(\"") {
            let after = &rest[pos + "cite!(\"".len()..];
            if let Some(end) = after.find('"') {
                let id = &after[..end];
                // Skip the macro definition itself and doc examples.
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    out.push(Cite {
                        module,
                        rule_id: id.to_string(),
                    });
                }
                rest = &after[end..];
            } else {
                break;
            }
        }
    }
    out
}

/// Distinct rule ids cited anywhere in the crate.
pub fn cited_rule_ids() -> std::collections::BTreeSet<String> {
    registry().into_iter().map(|c| c.rule_id).collect()
}
