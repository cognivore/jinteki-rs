//! The modal guard: every printed option is one a test has actually taken.
//!
//! CR 9.11.4g — "if an effect directs a player to choose between a set of
//! options that would create different effects, that choice ends an
//! instruction; each option begins its own instruction or set of
//! instructions, and the one chosen will resolve next".
//!
//! Predictive Planogram shipped resolving to NOTHING. Its "Resolve 1 of the
//! following" sat inside an `IfMet` branch, `IfMet` resolved its branch
//! inline, and a choice resolved inline is a choice nobody is ever asked. It
//! compiled, it was ticked complete, and it had a test — the test asserted the
//! card was playable and never looked at what the play did.
//!
//! That defect has a structural guard already:
//! `hardening::no_card_writes_a_choice_where_the_kernel_could_never_put_it`
//! walks the corpus and fails on a choice written where the kernel could not
//! put it. This file is the other half, and it is about the tests rather than
//! the cards: a modal card can be written correctly, offer its options, and
//! still have a mode that resolves to nothing, because no test ever took it.
//! Auditing the class by hand found four cards in exactly that state and one —
//! Data Raven — where the choice went to the wrong player, undetected because
//! the neutral policy's "answer with option 0" happened to be the answer the
//! test wanted.
//!
//! ## What is checked, and why that is enough
//!
//! Every `Instruction::ChooseOne` the corpus builds, at any depth and on
//! either face of a double-sided card, must have EVERY option taken by name
//! in `behaviour.rs`. Taking an option by name goes through
//! [`jinteki_cr::plan::Reply::ChooseNamed`], which panics unless the label is
//! among the options the VM offered — so "every option is driven" entails
//! "every option is offered", and the test that drives it is where the
//! resulting board state is asserted. The three things a modal card owes are
//! then one check.
//!
//! ## Source reflection
//!
//! Which options a test drives is not a runtime fact this test can observe —
//! it is a fact about another test binary. So the drives are read out of
//! `behaviour.rs`, the way
//! `jinteki_cr::tests::announcements` reads `instr.rs` to prove every declared
//! target position owes an announcement. The card side is NOT reflected: it is
//! the real `Instruction` tree of the real built cards, so a choice hidden in
//! an `IfMet`, a `Combined`, a subroutine or a flip face is found by
//! construction.

use jinteki_cr::instr::{Contained, Instruction};
use std::collections::BTreeSet;

/// The behaviour suite, read as text. The drives live in another test binary,
/// so the only way to ask what it takes is to read what it says.
const BEHAVIOUR_RS: &str = include_str!("behaviour.rs");

/// A modal option that no test needs to drive, with the rule that says so.
/// `(card, option label, reason)`.
///
/// Empty, and it should stay that way: an option a player can be offered and
/// cannot be shown to do anything is a card that does nothing, which is the
/// defect this file exists for. 9.12.3c's "no option can be fully resolved"
/// case is NOT an exemption — that is a board state, and a test reaches it by
/// building that board (see
/// `behaviour::fairchild_3_0_subroutines_are_a_mandatory_choice`).
const NOT_DRIVEN_BY_A_TEST: &[(&str, &str, &str)] = &[];

// ---------------------------------------------------------------------------
// The card side: every choice the corpus builds
// ---------------------------------------------------------------------------

/// One choice found in a built card: 9.11.4g's optioned effect, or 9.11.4f's
/// nested cost with more than one door.
struct Modal {
    /// The card a test would name to reach it. A flip face's choice is
    /// reached by naming the FRONT — `card("Cyber Bureau: Keeping the Peace")`
    /// is how Detective's Bureau's `[click]` ability is put on a board — so
    /// both names are carried and either satisfies the guard.
    names: Vec<String>,
    /// The face and ability it sits on, for the failure message.
    where_: String,
    /// 9.11.4g's option labels. Empty for a 9.11.4f cost choice, whose doors
    /// carry no labels at all.
    options: Vec<&'static str>,
    /// 9.11.4f: how many doors this nested cost prints. 0 for a `ChooseOne`.
    doors: usize,
    /// 9.11.4g's "options that would create DIFFERENT EFFECTS". See
    /// [`creates_different_effects`].
    different_effects: bool,
}

/// CR 9.11.4g asks whether the options "would create different effects". A
/// branch whose every instruction only RECORDS a named value creates no
/// effect at all: "name a card type" is one instruction with ten contents,
/// enumerated from 2.15.2's closed list of card types rather than printed on
/// the card as bullets, and the effect happens later when something READS the
/// name. Ten arms of the same instruction prove nothing the first arm does not.
///
/// A printed modal is the other kind, and every one of its options is its own
/// instruction chain — so every one is its own promise, and owes its own arm.
fn creates_different_effects(options: &[(&'static str, Vec<Instruction>)]) -> bool {
    options
        .iter()
        .any(|(_, is)| is.iter().any(|i| !matches!(i, Instruction::MaintainChoice { .. })))
}

/// Every `ChooseOne` reachable from an instruction, at any depth.
///
/// [`Instruction::contains`] is the authority on what an instruction holds,
/// and `announcements::every_contained_instruction_is_declared` is what keeps
/// it exhaustive — so a new container variant cannot hide a choice from this
/// walk without failing that test first.
fn choices_in<'a>(i: &'a Instruction, out: &mut Vec<&'a Vec<(&'static str, Vec<Instruction>)>>) {
    if let Instruction::ChooseOne { options } = i {
        out.push(options);
    }
    walk_contained(i, out, choices_in);
}

/// CR 9.11.4f: a nested cost with more than one door — "end the run unless
/// they either spend [click][click] or pay 5[credit]" (Manegarm Skunkworks).
/// A printed door nobody has ever walked through is the same defect as a
/// printed option nobody has ever taken; the rule and the decision kind are
/// different, so it is counted separately.
fn cost_doors_in<'a>(i: &'a Instruction, out: &mut Vec<&'a [jinteki_cr::ability::Cost]>) {
    match i {
        Instruction::NestedCostUnless { costs, .. } if costs.len() > 1 => out.push(costs),
        _ => {}
    }
    walk_contained(i, out, cost_doors_in);
}

/// The shared recursion of the two walks above.
fn walk_contained<'a, T>(
    i: &'a Instruction,
    out: &mut Vec<T>,
    each: fn(&'a Instruction, &mut Vec<T>),
) {
    match i.contains() {
        Contained::Nothing => {}
        Contained::Inline(l) | Contained::Deferred(l) => {
            for k in l {
                each(k, out);
            }
        }
        Contained::Branches(bs) => {
            for (_, effects) in bs {
                for k in effects {
                    each(k, out);
                }
            }
        }
    }
}

/// Every modal in every card the card layer defines.
fn corpus() -> Vec<Modal> {
    let mut out = Vec::new();
    for c in jinteki_cards::all_cards() {
        let front = c.printed.name.to_string();
        // CR 1.4: a double-sided card's back is printed characteristics of
        // its own, abilities included.
        let faces = std::iter::once(&c.printed).chain(c.printed.flip_faces.iter());
        for face in faces {
            let mut names = vec![face.name.to_string()];
            if face.name != front {
                names.push(front.clone());
            }
            for ability in &face.abilities {
                let mut found = Vec::new();
                for i in &ability.instructions {
                    choices_in(i, &mut found);
                }
                let mut doors = Vec::new();
                for i in &ability.instructions {
                    cost_doors_in(i, &mut doors);
                }
                for costs in doors {
                    out.push(Modal {
                        names: names.clone(),
                        where_: format!(
                            "{} / ability {:?} / 9.11.4f nested cost",
                            face.name, ability.label
                        ),
                        options: Vec::new(),
                        doors: costs.len(),
                        different_effects: true,
                    });
                }
                for options in found {
                    out.push(Modal {
                        names: names.clone(),
                        doors: 0,
                        where_: format!("{} / ability {:?}", face.name, ability.label),
                        options: options.iter().map(|(l, _)| *l).collect(),
                        different_effects: creates_different_effects(options),
                    });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The test side: what `behaviour.rs` drives
// ---------------------------------------------------------------------------

/// One `#[test] fn` of `behaviour.rs`: the cards it puts on a board, and the
/// option labels it is in a position to take.
struct TestFn {
    name: String,
    /// Every `card("…")`/`card_partial("…")` argument in the body.
    cards: BTreeSet<String>,
    /// The strings it could hand to `Reply::ChooseNamed`. Empty unless the
    /// body calls it at all, which is what keeps a test that merely MENTIONS
    /// a phrase from being counted as driving it.
    drives: BTreeSet<String>,
    /// Whether the body pays a nested cost by index at all
    /// (`Reply::PayCostWith(…)`). WHICH door it names is not legible — see
    /// [`every_printed_choice_of_costs_has_a_test_that_pays_one`].
    doors: bool,
}

/// Every string literal in a slice of source, un-escaped only as far as the
/// suite needs (no `\"` appears inside a label).
fn string_literals(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            if j <= b.len() {
                if let Some(s) = src.get(start..j.min(src.len())) {
                    out.push(s.to_string());
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// Split `behaviour.rs` into its test functions by brace depth.
fn test_functions() -> Vec<TestFn> {
    let mut out = Vec::new();
    let lines: Vec<&str> = BEHAVIOUR_RS.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let Some(rest) = lines[i].strip_prefix("fn ") else {
            i += 1;
            continue;
        };
        let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let mut depth = 0i32;
        let mut j = i;
        let mut body = String::new();
        loop {
            if j >= lines.len() {
                break;
            }
            body.push_str(lines[j]);
            body.push('\n');
            depth += lines[j].matches('{').count() as i32;
            depth -= lines[j].matches('}').count() as i32;
            if depth <= 0 && j > i {
                break;
            }
            j += 1;
        }
        let literals = string_literals(&body);
        let mut cards = BTreeSet::new();
        for m in ["card(\"", "card_partial(\""] {
            let mut from = 0;
            while let Some(p) = body[from..].find(m) {
                let at = from + p + m.len();
                if let Some(end) = body[at..].find('"') {
                    cards.insert(body[at..at + end].to_string());
                }
                from = at;
            }
        }
        // A body that never calls `ChooseNamed` drives no option, whatever it
        // says in its assertion messages.
        let drives = if body.contains("Reply::ChooseNamed") {
            literals.iter().filter(|s| s.len() >= 2).cloned().collect()
        } else {
            BTreeSet::new()
        };
        let doors = body.contains("Reply::PayCostWith");
        out.push(TestFn { name, cards, drives, doors });
        i = j + 1;
    }
    out
}

/// Whether some test both puts this card on a board and takes this label.
///
/// The match is `label.contains(needle)`, which is exactly what
/// `jinteki_cr::plan`'s `ChooseNamed` does when it turns a name into an
/// option index — so a label this says is driven is a label that reply would
/// really select, and a label it says is not is one that reply would panic on.
fn driven_by<'a>(tests: &'a [TestFn], names: &[String], label: &str) -> Option<&'a str> {
    tests
        .iter()
        .find(|t| {
            names.iter().any(|n| t.cards.contains(n))
                && t.drives.iter().any(|d| label.contains(d.as_str()))
        })
        .map(|t| t.name.as_str())
}

// ---------------------------------------------------------------------------
// The guard
// ---------------------------------------------------------------------------

/// CR 9.11.4g: each option "begins its own instruction or set of
/// instructions". A printed option no test has ever taken is an instruction
/// chain nothing has ever run — which is the state Predictive Planogram
/// shipped in, for both of its options at once.
#[test]
fn every_printed_option_of_every_modal_card_is_taken_by_a_test() {
    let tests = test_functions();
    let mut bad: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for m in corpus().iter().filter(|m| m.different_effects && m.doors == 0) {
        for label in &m.options {
            checked += 1;
            if NOT_DRIVEN_BY_A_TEST
                .iter()
                .any(|(c, o, _)| m.names.iter().any(|n| n == c) && o == label)
            {
                continue;
            }
            if driven_by(&tests, &m.names, label).is_none() {
                bad.push(format!(
                    "{}: the option {label:?} is never taken by any test naming {} — \
                     9.11.4g gives it its own instruction chain, and nothing has run it",
                    m.where_,
                    m.names.join(" / ")
                ));
            }
        }
    }
    assert!(
        checked >= 30,
        "the corpus still holds the modal options this guard is about — {checked} found, \
         so the walk has not been quietly emptied by a refactor"
    );
    assert!(
        bad.is_empty(),
        "CR 9.11.4g: {} printed modal option(s) that no behaviour test resolves. Add an \
         arm per option — `behaviour::predictive_planogram_offers_the_untagged_choice_\
         and_resolves_it` is the worked template — or exempt it in \
         `NOT_DRIVEN_BY_A_TEST` with the rule that says so:\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}

/// CR 9.11.4f: "if an ability contains a nested cost, the choice of whether or
/// not to pay that cost ends an instruction". A cost printed as two doors —
/// "end the run unless they either spend [click][click] or pay 5[credit]"
/// (Manegarm Skunkworks) — promises the payer both.
///
/// This guard is DELIBERATELY WEAKER than the 9.11.4g one above, and the
/// reason is a limit of the technique rather than a judgement about the rule.
/// 9.11.4g's options carry printed labels, so "which option a test took" is
/// legible in the test's own source. A nested cost's doors carry no labels at
/// all — `Reply::PayCostWith(i)` names one by INDEX — and an integer literal
/// in a test body is indistinguishable from every other integer in it, so
/// "which door a test walked through" is not a fact source reflection can
/// read. Guessing at it would produce a check that passes on any test with a
/// `0` and a `1` in it, which is not a guard.
///
/// So what is pinned is the part that IS legible: a card printing a choice of
/// costs has a test that names it and pays a door by index. That catches the
/// regression this file exists for — a new multi-door card shipping with no
/// test that ever pays either door — and stops short of claiming more.
/// Manegarm's own test covers the rest by hand: it walks both doors, asserts
/// the offered list holds two, and covers 1.16.1's third outcome where
/// neither is affordable and no choice is put at all.
#[test]
fn every_printed_choice_of_costs_has_a_test_that_pays_one() {
    let tests = test_functions();
    let mut bad: Vec<String> = Vec::new();
    let mut doors = 0usize;
    for m in corpus().iter().filter(|m| m.doors > 0) {
        doors += m.doors;
        let walked =
            tests.iter().any(|t| m.names.iter().any(|n| t.cards.contains(n)) && t.doors);
        if !walked {
            bad.push(format!(
                "{}: {} printed doors, and no test naming {} ever pays one",
                m.where_,
                m.doors,
                m.names.join(" / ")
            ));
        }
    }
    assert!(doors > 0, "the corpus still prints a multi-door cost (Manegarm Skunkworks)");
    assert!(
        bad.is_empty(),
        "CR 9.11.4f: printed cost choice(s) no behaviour test pays with \
         `Reply::PayCostWith(…)`:\n  {}",
        bad.join("\n  ")
    );
}

/// The naming family, which 9.11.4g does not reach: "name a card type" is one
/// instruction over 2.15.2's closed list, not a printed set of bullets, so its
/// options do not each owe an arm. What they do owe is that the decision is
/// raised and answerable at all — a naming choice nobody ever answers is a
/// card whose whole ability is dead.
#[test]
fn every_naming_choice_is_answered_by_a_test() {
    let tests = test_functions();
    let mut bad: Vec<String> = Vec::new();
    let mut naming = 0usize;
    for m in corpus().iter().filter(|m| !m.different_effects) {
        naming += 1;
        if !m.options.iter().any(|l| driven_by(&tests, &m.names, l).is_some()) {
            bad.push(format!(
                "{}: no test ever names one of {:?}, so this choice has never been \
                 answered by anything",
                m.where_, m.options
            ));
        }
    }
    assert!(naming > 0, "the naming family still exists (Azmari EdTech, Wari, …)");
    assert!(bad.is_empty(), "CR 1.15.1b naming choices never exercised:\n  {}", bad.join("\n  "));
}

/// An exemption that no longer names a real option is a lie about the corpus:
/// the card may have been rewritten, or deleted, and the entry would then
/// silently excuse a DIFFERENT option that shares the name.
#[test]
fn no_stale_modal_exemptions() {
    let all = corpus();
    let mut bad: Vec<String> = Vec::new();
    for (card, option, reason) in NOT_DRIVEN_BY_A_TEST {
        assert!(reason.len() > 40, "an exemption states the rule that allows it: {card}/{option}");
        let live = all.iter().any(|m| {
            m.different_effects
                && m.names.iter().any(|n| n == card)
                && m.options.iter().any(|o| o == option)
        });
        if !live {
            bad.push(format!("{card} no longer prints a modal option {option:?}"));
        }
    }
    assert!(bad.is_empty(), "stale exemptions:\n  {}", bad.join("\n  "));
}

/// The reflection above trusts one thing about the test side: that
/// `Reply::ChooseNamed` selects the option whose label CONTAINS the name, so
/// that [`driven_by`]'s `label.contains(needle)` asks the driver's own
/// question. If the driver ever matched some other way — equality, a prefix,
/// a normalised comparison — this guard would keep passing while measuring
/// something else, and options it called driven would panic in a real test.
///
/// The reply resolver is private to `plan`, so the contract is pinned where
/// it is written.
#[test]
fn the_guard_asks_the_same_question_the_reply_driver_asks() {
    const PLAN_RS: &str = include_str!("../../jinteki-cr/src/plan.rs");
    let at = PLAN_RS.find("Reply::ChooseNamed(n) =>").expect("plan.rs resolves `ChooseNamed`");
    let arm = &PLAN_RS[at..at + 300];
    assert!(
        arm.contains("choices(spec)"),
        "the driver reads the option labels off the `ChooseOption` spec, which is what \
         this guard walks the corpus for: {arm}"
    );
    assert!(
        arm.contains(".position(|l| l.contains(n))"),
        "9.11.4g: the driver picks the option whose label CONTAINS the name. \
         `driven_by` matches the same way, and would be measuring nothing if this \
         changed: {arm}"
    );
    // And the panic is what makes "driven" entail "offered": a test that takes
    // an option the VM did not put on the table does not quietly pass.
    assert!(
        arm.contains("panic!"),
        "…and a name matching no offered option is a panic, not a silent default — \
         which is why driving every option proves every option was offered: {arm}"
    );
}
