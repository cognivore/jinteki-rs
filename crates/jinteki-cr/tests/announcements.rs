//! CR 1.15.2 announcements, made structural.
//!
//! Five separate instructions shipped with the same defect — `MoveToDeck`
//! (W14b), the counter family (W17b), `ModifyStrength` (W17c), `RevealCards`
//! (W20) and `IfMet` (W21) — each of them silently resolving a
//! `TargetSpec::Choose` position to nothing because the VM's hand-maintained
//! list of announcing instructions did not mention it. Five instances of one
//! mistake is a design defect, not five bugs.
//!
//! The mechanism that closes it has three layers, and this file is the third:
//!
//! 1. **Compile.** [`Instruction::target_positions`] and
//!    [`Instruction::contains`] are exhaustive matches with no wildcard arm,
//!    so a new `Instruction` variant does not compile until its target
//!    positions and contained instructions are declared.
//! 2. **Derivation.** The VM reads the announcement obligation off those two
//!    functions instead of a list, so a declared position is announced with
//!    no VM code written anywhere. Forgetting to *use* a position is no
//!    longer possible, because nothing uses one.
//! 3. **This test.** A variant can still be declared with a position it
//!    really has left out (`Instruction::Foo { .. } => Vec::new()`), which
//!    compiles. So the enum's own fields are read out of the source and
//!    checked against the arms that are supposed to bind them.
//!
//! Layer 3 is source reflection because Rust has none at runtime and the
//! alternative — a proc macro deriving the two functions from the fields —
//! cannot know announcement ORDER or that a description is not a target.
//! An exemption is therefore allowed, but it must be written down with its
//! reason, and a stale one fails too.

use jinteki_cr::decision::DecisionAnswer;
use jinteki_cr::instr::{Contained, Instruction, Quantity, TargetFilter, TargetSpec};
use jinteki_cr::object::{CardType, Side, Zone};
use jinteki_cr::plan::{self, Kind, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

const INSTR_RS: &str = include_str!("../src/instr.rs");

/// (variant, field) pairs whose type carries a `TargetSpec` but which are NOT
/// target positions, with the rule that says so. 1.15.2 scopes announcement
/// to the objects an INSTRUCTION acts on; a description evaluated later by
/// something else is not one.
const NOT_A_TARGET_POSITION: &[(&str, &str, &str)] = &[(
    "CreateLingeringEffect",
    "payload",
    "9.10.1: the payload's criteria describe the objects a lingering EFFECT \
     applies to, re-read while it lasts (9.5.3a's 'cannot use those cards' \
     abilities'). They are not targets of this instruction, and 1.15.2f's \
     'targets cannot be changed' would be wrong about them.",
)];

/// A variant of the `Instruction` enum, as the source declares it.
struct Variant {
    name: String,
    /// Field names (or `"0"` for a tuple position) whose type mentions
    /// `TargetSpec`, directly or through another type declared in this module
    /// that carries one.
    target_fields: Vec<String>,
    /// Field names whose type mentions `Instruction`.
    instruction_fields: Vec<String>,
    tuple: bool,
}

/// The body of a `{ … }` block starting at `from`.
fn block_at(src: &str, from: usize) -> &str {
    let start = src[from..].find('{').expect("a block") + from;
    let mut depth = 0;
    for (i, c) in src[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[start + 1..start + i];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated block");
}

/// Strip `///` and `//` comment lines — they mention type names in prose.
fn without_comments(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split an enum body into its variants at depth 0.
fn split_variants(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in body.chars() {
        match c {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            _ => {}
        }
        if c == ',' && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// Every type declared in `instr.rs` that carries a `TargetSpec`, to a
/// fixpoint — `ChoiceSpec` and `SubroutineGrant` hold one inside them, and a
/// future one will too. `Instruction` itself is excluded: an instruction
/// field is the CONTAINED half of the question, checked separately.
fn target_carrying_types(src: &str) -> Vec<String> {
    let mut carriers: Vec<String> = vec!["TargetSpec".to_string()];
    let mut grew = true;
    while grew {
        grew = false;
        for (i, _) in src.match_indices("\npub enum ").chain(src.match_indices("\npub struct ")) {
            let head = &src[i..];
            let name: String = head
                .trim_start_matches('\n')
                .split_whitespace()
                .nth(2)
                .unwrap_or("")
                .trim_end_matches('{')
                .trim()
                .to_string();
            if name.is_empty() || name == "Instruction" || carriers.contains(&name) {
                continue;
            }
            let body = without_comments(block_at(src, i));
            if carriers.iter().any(|c| body.contains(c.as_str())) {
                carriers.push(name);
                grew = true;
            }
        }
    }
    carriers
}

fn instruction_variants() -> Vec<Variant> {
    let src = INSTR_RS;
    let carriers = target_carrying_types(src);
    let at = src.find("pub enum Instruction {").expect("the Instruction enum");
    let body = without_comments(block_at(src, at));
    let mut out = Vec::new();
    for v in split_variants(&body) {
        let v = v.split_whitespace().collect::<Vec<_>>().join(" ");
        if v.is_empty() {
            continue;
        }
        let name: String = v.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if name.is_empty() {
            continue;
        }
        let rest = v[name.len()..].trim().to_string();
        let tuple = rest.starts_with('(');
        let mut target_fields = Vec::new();
        let mut instruction_fields = Vec::new();
        if tuple {
            if carriers.iter().any(|c| rest.contains(c.as_str())) {
                target_fields.push("0".to_string());
            }
            if rest.contains("Instruction") {
                instruction_fields.push("0".to_string());
            }
        } else {
            // `name: Type` pairs at depth 0 of the braces.
            let inner = rest.trim_start_matches('{').trim_end_matches('}');
            for field in split_variants(inner) {
                let Some((fname, ftype)) = field.split_once(':') else { continue };
                let fname = fname.trim().to_string();
                if fname.is_empty() {
                    continue;
                }
                if carriers.iter().any(|c| ftype.contains(c.as_str())) {
                    target_fields.push(fname.clone());
                }
                if ftype.contains("Instruction") {
                    instruction_fields.push(fname);
                }
            }
        }
        out.push(Variant { name, target_fields, instruction_fields, tuple });
    }
    out
}

/// The pattern text of every arm of `fn <name>` that mentions this variant:
/// from `Instruction::<Variant>` up to the next `Instruction::` or the arm's
/// `=>`, which is where a pattern stops binding.
fn arm_patterns(func: &str, variant: &str) -> Vec<String> {
    let needle = format!("Instruction::{variant}");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = func[from..].find(&needle) {
        let at = from + i;
        from = at + needle.len();
        // Not a prefix of a longer variant name (`Trace` vs `TraceInitiate`).
        if func[from..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let tail = &func[at..];
        let end = tail
            .find("=>")
            .map(|e| tail[..e].find(" | ").map(|p| p.min(e)).unwrap_or(e))
            .unwrap_or(tail.len());
        out.push(tail[..end].to_string());
    }
    out
}

fn function_body(name: &str) -> String {
    let at = INSTR_RS.find(&format!("pub fn {name}")).unwrap_or_else(|| panic!("fn {name}"));
    without_comments(block_at(INSTR_RS, at))
}

/// Every `TargetSpec` field of every `Instruction` variant is declared as a
/// target position — or exempted, with its reason.
#[test]
fn every_target_spec_field_is_a_declared_position() {
    let body = function_body("target_positions");
    let mut bad: Vec<String> = Vec::new();
    for v in instruction_variants() {
        for f in &v.target_fields {
            if NOT_A_TARGET_POSITION.iter().any(|(n, fl, _)| *n == v.name && fl == f) {
                continue;
            }
            let arms = arm_patterns(&body, &v.name);
            let bound = arms.iter().any(|a| {
                if v.tuple {
                    // A tuple position is bound unless the arm wildcards it.
                    !a.contains("(..)")
                } else {
                    a.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .any(|w| w == f.as_str())
                }
            });
            if !bound {
                bad.push(format!(
                    "Instruction::{}.{} is a TargetSpec position that \
                     `target_positions` never binds — a `TargetSpec::Choose` \
                     there would resolve to nothing, with no error",
                    v.name, f
                ));
            }
        }
    }
    assert!(bad.is_empty(), "CR 1.15.2 announcement holes:\n{}", bad.join("\n"));
}

/// Every field holding instructions is declared in `contains`, so the VM can
/// say whose announcements they are (1.15.2 scopes them to an instruction).
#[test]
fn every_contained_instruction_is_declared() {
    let body = function_body("contains");
    let mut bad: Vec<String> = Vec::new();
    for v in instruction_variants() {
        for f in &v.instruction_fields {
            let arms = arm_patterns(&body, &v.name);
            let bound = arms.iter().any(|a| {
                if v.tuple {
                    !a.contains("(..)")
                } else {
                    a.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .any(|w| w == f.as_str())
                }
            });
            if !bound {
                bad.push(format!(
                    "Instruction::{}.{} holds instructions that `contains` \
                     never declares — their target announcements belong to \
                     nobody",
                    v.name, f
                ));
            }
        }
    }
    assert!(bad.is_empty(), "CR 9.11.3 containment holes:\n{}", bad.join("\n"));
}

/// An exemption that no longer names a real field is a lie about the kernel.
#[test]
fn no_stale_announcement_exemptions() {
    let variants = instruction_variants();
    let mut bad: Vec<String> = Vec::new();
    for (name, field, reason) in NOT_A_TARGET_POSITION {
        assert!(reason.len() > 40, "an exemption states its rule: {name}.{field}");
        let live = variants
            .iter()
            .any(|v| v.name == *name && v.target_fields.iter().any(|f| f == field));
        if !live {
            bad.push(format!("{name}.{field} is exempted but is no longer a TargetSpec field"));
        }
    }
    assert!(bad.is_empty(), "stale exemptions:\n{}", bad.join("\n"));
}

/// The reflection above is about declarations; this is about behaviour. Every
/// declared position of every variant, put into a `Choose`, must make the
/// instruction owe an announcement — the property the five defects violated.
#[test]
fn a_choose_in_any_declared_position_owes_an_announcement() {
    let choose = TargetSpec::Choose {
        count: Quantity::c(1),
        criteria: vec![TargetFilter::InstalledRunnerCard],
        up_to: false,
    };
    assert_eq!(choose.announcement_slots(), 1, "1.15.2: a chosen position is announced");
    assert_eq!(TargetSpec::SelfSource.announcement_slots(), 0, "1.15.1: a named object is not");
    assert_eq!(
        TargetSpec::Each(vec![choose.clone(), TargetSpec::SelfSource, choose.clone()])
            .announcement_slots(),
        2,
        "1.15.2: one announcement per choosing element of an `Each`"
    );
    // Every instruction that reaches the announcement machinery goes through
    // these two functions, so the property holds for every variant by
    // construction; the samples below are the ones the five defects were.
    for instr in [
        Instruction::MoveToDeck { card: choose.clone(), top: true },
        Instruction::PlaceCounters {
            target: choose.clone(),
            kind: jinteki_cr::object::CounterKind::Advancement,
            amount: Quantity::c(1),
        },
        Instruction::ModifyStrength {
            target: choose.clone(),
            amount: Quantity::c(1),
            duration: None,
        },
        Instruction::RevealCards { cards: choose.clone() },
        Instruction::PlayCard {
            card: choose.clone(),
            ignore_costs: false,
            then_remove_from_game: false,
        },
        Instruction::IfMet {
            requires: Vec::new(),
            then: vec![Instruction::TrashCards(choose.clone())],
            otherwise: Vec::new(),
        },
        Instruction::Combined(vec![Instruction::TrashCards(choose.clone())]),
    ] {
        let owed = instr.chooses_targets()
            || matches!(instr.contains(), Contained::Inline(_) | Contained::Deferred(_) | Contained::Branches(_));
        assert!(owed, "{instr:?} must owe an announcement for its chosen position");
    }
}

/// The sixth instance, found by this wave's audit and fixed with the rest:
/// `Instruction::PlayCard`'s card position was never announced, so an ability
/// playing a CHOSEN card from the grip played nothing at all.
#[test]
fn a_played_card_chosen_from_the_grip_is_announced() {
    let mut vm = Vm::empty(41);
    let event = vm.new_object(
        tk::event("Gain-3-like", 0, vec![Instruction::GainCredits(Side::Runner, Quantity::c(3))]),
        Zone::Hand(Side::Runner),
    );
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(event);
    let other = vm.new_object(
        tk::event("Gain-1-like", 0, vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))]),
        Zone::Hand(Side::Runner),
    );
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(other);
    tk::install_rig(&mut vm, tk::play_chosen_event_button("Play-from-grip"));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("play-chosen-event"))
            .when(Match::targets().once(), Reply::target(event))
            .stop_at_action(),
    );
    let picks: Vec<Vec<jinteki_cr::ObjectId>> = t
        .windows(Kind::Targets, Side::Runner)
        .iter()
        .filter_map(|e| match &e.answer {
            Some(DecisionAnswer::Targets(v)) => Some(v.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        picks,
        vec![vec![event]],
        "1.15.1/1.15.2: the card a play instruction chooses is announced"
    );
    assert_eq!(vm.st.runner.credits, 3, "the announced event is the one that was played");
    assert!(
        matches!(vm.st.objects[&event].zone, Zone::Discard(Side::Runner)),
        "8.6.7g: the played event is trashed"
    );
    assert_eq!(vm.st.objects[&other].zone, Zone::Hand(Side::Runner), "the other card stayed");
}

/// A card type the reflection cannot see: the shape above is built from the
/// public vocabulary, so it proves the fix is reachable from printed text.
#[test]
fn the_play_from_grip_shape_uses_only_public_vocabulary() {
    let c = tk::play_chosen_event_button("probe");
    assert_eq!(c.card_type, CardType::Resource);
    assert!(matches!(
        c.abilities[0].instructions[0],
        Instruction::PlayCard { card: TargetSpec::Choose { .. }, .. }
    ));
    assert_eq!(c.abilities[0].instructions.len(), 1);
    let _ = Side::Runner;
}
