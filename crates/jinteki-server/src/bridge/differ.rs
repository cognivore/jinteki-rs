//! differ 0.3.3 `patch`, ported line-for-line from differ/patch.cljc +
//! differ/core.cljc (extracted source; see the wire spec in this repo's
//! design notes). A diff is `[alterations, removals]`; patch applies
//! removals FIRST, then alterations.
//!
//! Vector alterations are flat `[idx v idx v …]` pairs where a non-integer
//! index slot (the JSON string "+" after cheshire serialization) means
//! append. Vector removals are `[count-delta, idx sub …]`. Map removal
//! marker is the number 0.

use serde_json::Value as Js;

pub fn patch(state: &Js, diff: &Js) -> Js {
    let Js::Array(parts) = diff else {
        return state.clone();
    };
    if parts.len() != 2 {
        return state.clone();
    }
    let after_removals = removals(state, &parts[1]);
    alterations(&after_removals, &parts[0])
}

fn alterations(state: &Js, diff: &Js) -> Js {
    match (state, diff) {
        (Js::Object(s), Js::Object(d)) => {
            let mut out = s.clone();
            for (k, dv) in d {
                let old = out.get(k).cloned().unwrap_or(Js::Null);
                out.insert(k.clone(), alterations(&old, dv));
            }
            Js::Object(out)
        }
        (Js::Array(s), Js::Array(d)) => {
            // Lockstep walk: pairs (diff-idx, diff-val).
            let mut out: Vec<Js> = Vec::with_capacity(s.len());
            let mut idx: usize = 0;
            let mut old_iter = s.iter();
            let mut old_next = old_iter.next();
            let mut pairs = d.chunks(2).peekable();
            loop {
                let old_empty = old_next.is_none();
                let diff_empty = pairs.peek().is_none();
                if old_empty && diff_empty {
                    break;
                }
                if diff_empty {
                    out.push(old_next.unwrap().clone());
                    old_next = old_iter.next();
                    idx += 1;
                    continue;
                }
                let pair = *pairs.peek().unwrap();
                let diff_idx = pair.first().and_then(|v| v.as_u64());
                let diff_val = pair.get(1).cloned().unwrap_or(Js::Null);
                let idx_matches = diff_idx == Some(idx as u64);
                if idx_matches || old_empty {
                    let old = old_next.cloned().unwrap_or(Js::Null);
                    out.push(alterations(&old, &diff_val));
                    pairs.next();
                    if !old_empty {
                        old_next = old_iter.next();
                    }
                    idx += 1;
                } else {
                    out.push(old_next.unwrap().clone());
                    old_next = old_iter.next();
                    idx += 1;
                }
            }
            Js::Array(out)
        }
        _ => diff.clone(),
    }
}

fn removals(state: &Js, diff: &Js) -> Js {
    match (state, diff) {
        (Js::Object(s), Js::Object(d)) => {
            let mut out = s.clone();
            for (k, dv) in d {
                if dv == &Js::Number(0.into()) {
                    out.remove(k);
                } else {
                    let old = out.get(k).cloned().unwrap_or(Js::Null);
                    out.insert(k.clone(), removals(&old, dv));
                }
            }
            Js::Object(out)
        }
        (Js::Array(s), Js::Array(d)) => {
            if d.is_empty() {
                return state.clone();
            }
            let delta = d[0].as_i64().unwrap_or(0);
            let max_index = (s.len() as i64 - delta).max(0) as usize;
            let mut out: Vec<Js> = Vec::with_capacity(max_index);
            let mut pairs = d[1..].chunks(2).peekable();
            for (index, old_val) in s.iter().enumerate() {
                if index == max_index {
                    break;
                }
                let matched = pairs
                    .peek()
                    .and_then(|p| p.first())
                    .and_then(|v| v.as_u64())
                    == Some(index as u64);
                if matched {
                    let sub = pairs.next().unwrap().get(1).cloned().unwrap_or(Js::Null);
                    out.push(removals(old_val, &sub));
                } else {
                    out.push(old_val.clone());
                }
            }
            Js::Array(out)
        }
        _ => state.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn map_alter_and_remove() {
        let state = json!({"a": 1, "b": {"c": 2, "d": 3}, "e": 4});
        // b.c -> 9, e removed.
        let diff = json!([{"b": {"c": 9}}, {"e": 0}]);
        assert_eq!(patch(&state, &diff), json!({"a":1,"b":{"c":9,"d":3}}));
    }

    #[test]
    fn vector_append_with_plus_marker() {
        let state = json!({"log": [1, 2]});
        // old [1 2] -> new [1 9 3]: alterations [1 9 "+" 3].
        let diff = json!([{"log": [1, 9, "+", 3]}, {}]);
        assert_eq!(patch(&state, &diff), json!({"log": [1, 9, 3]}));
    }

    #[test]
    fn vector_truncation() {
        let state = json!({"v": [1, 2, 3, 4]});
        // remove trailing 2 elements: removals [2] (delta only).
        let diff = json!([{}, {"v": [2]}]);
        assert_eq!(patch(&state, &diff), json!({"v": [1, 2]}));
    }

    #[test]
    fn nested_vector_of_maps() {
        let state = json!({"hand": [{"cid": 1}, {"cid": 2}]});
        let diff = json!([{"hand": [1, {"cid": 5}]}, {}]);
        assert_eq!(
            patch(&state, &diff),
            json!({"hand": [{"cid": 1}, {"cid": 5}]})
        );
    }

    #[test]
    fn removal_then_append_order() {
        // Patch applies removals first, then alterations.
        let state = json!({"v": [1, 2, 3]});
        let diff = json!([{"v": [2, 9]}, {"v": [1]}]); // truncate to [1,2], then set idx2 -> append 9
        assert_eq!(patch(&state, &diff), json!({"v": [1, 2, 9]}));
    }
}
