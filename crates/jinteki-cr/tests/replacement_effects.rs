//! CR 9.9.2 — "instead of" means the original DOES NOT HAPPEN.
//!
//! A replacement effect removes the atom it replaces (9.9.2b) and resolves
//! its own instructions in that atom's place. `SuppressAndResolve` is the
//! only transform whose replacement is INSTRUCTIONS, so it is the only one
//! that puts a resolution frame on the stack while the effect it replaced is
//! still imminent — and that is where the kernel got it wrong. A production
//! game played Pinhole Threading ("instead of breaching the attacked server,
//! access 1 card in the root of another server"), got the Pinhole access AND
//! then `BreachBegan { server: Rnd }` right behind it, and stole an agenda
//! off R&D that the card never let the Runner see.
//!
//! The cause was not the transform and not the breach. Making an instruction
//! imminent can push a frame — the replacement's own — and both callers of
//! `push_imminent` then advanced "the frame on top" instead of THEIR frame.
//! With a mandatory replacement the frame on top was the replacement's, so
//! the step never left its Enter phase; when the replacement finished, the
//! step ran Enter a SECOND time, built a FRESH imminence with a live atom,
//! and did the very thing it had just been told not to do. Account Siphon
//! escaped only because its replacement is a "you may": the decision suspends
//! the flow before any frame is pushed, so the phase advance landed right.
//!
//! These tests hold the whole class to the rule, on both sides of the split:
//! a replaced STEP (the breach) and a replaced INSTRUCTION inside an ability
//! (a credit gain).

use jinteki_cr::change::GameChange;
use jinteki_cr::effects::EffectClass;
use jinteki_cr::instr::{Instruction, Quantity};
use jinteki_cr::object::{ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// A MANDATORY "instead of breaching, <instructions>" — Pinhole Threading's
/// half of the class, with the instructions reduced to something an assertion
/// can see. The breach must not happen.
#[test]
fn mandatory_instead_of_breaching_suppresses_the_breach() {
    let mut vm = Vm::empty(9001);
    tk::install_rig(
        &mut vm,
        tk::instead_resolve_card(
            "Pinhole-like",
            "pinhole: gain 7 instead of breaching",
            EffectClass::Breach,
            vec![Instruction::GainCredits(Side::Runner, Quantity::c(7))],
        ),
    );
    // An agenda on top of R&D: if the breach happens at all, the Runner
    // steals it, and no assertion can miss that.
    let agenda = vm.new_object(tk::vanilla_agenda("Untouchable", 3, 3), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("pinhole:"))
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 7, "the replacement resolved: {}", t.tail(16));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "9.9.2b: the breach it replaced did NOT also happen: {}",
        t.tail(16)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { .. })),
        "and nothing in R&D was accessed: {}",
        t.tail(16)
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::Deck(Side::Corp), "the agenda is still in R&D");
    assert_eq!(vm.score(Side::Runner), 0);
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::RunDeclaredSuccessful { server: ServerId::Rnd, .. }
        )),
        "6.8.4: a replaced breach still leaves the run successful"
    );
}

/// The replacement is spent by applying (9.9.9c is about one effect; the
/// kernel's replacements are one-shot). A SECOND run in the same turn
/// breaches normally — proof that the first run's silence was the
/// replacement and not a broken breach step.
#[test]
fn the_breach_returns_once_the_replacement_is_spent() {
    let mut vm = Vm::empty(9002);
    tk::install_rig(
        &mut vm,
        tk::instead_resolve_card(
            "Pinhole-like",
            "pinhole: gain 7 instead of breaching",
            EffectClass::Breach,
            vec![Instruction::GainCredits(Side::Runner, Quantity::c(7))],
        ),
    );
    tk::fill_deck(&mut vm, Side::Corp, 4);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("pinhole:"))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    let breaches = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::BreachBegan { server: ServerId::Rnd }))
        .count();
    assert_eq!(breaches, 1, "the first breach was replaced, the second was not: {}", t.tail(24));
    assert_eq!(vm.st.runner.credits, 7, "the replacement paid out exactly once");
}

/// The other side of the split: a replaced INSTRUCTION, inside an ability
/// frame rather than a timing-structure step. "Instead of gaining credits,
/// draw a card" — the gain must not happen either.
#[test]
fn mandatory_instead_of_an_instruction_suppresses_that_instruction() {
    let mut vm = Vm::empty(9003);
    tk::install_rig(
        &mut vm,
        tk::instead_resolve_card(
            "Instead-like",
            "instead: draw 1 instead of gaining credits",
            EffectClass::GainCredits,
            vec![Instruction::Draw(Side::Runner, Quantity::c(1))],
        ),
    );
    tk::install_rig(&mut vm, tk::gain_credits_card("Payday", "payday: gain 4", 4));
    tk::fill_deck(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("instead:"))
            .when(Match::paid().once(), Reply::take("payday:"))
            .stop_at_action(),
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 1, "the replacement drew a card: {}", t.tail(16));
    assert_eq!(
        vm.st.runner.credits, 0,
        "9.9.2b: the credit gain it replaced did NOT also happen: {}",
        t.tail(16)
    );
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CreditsGained { side: Side::Runner, .. })),
        "no gain was ever recorded: {}",
        t.tail(16)
    );
}

/// The replacing instructions still get to ANNOUNCE their own targets. This
/// is what a mandatory replacement lost when its resolution frame was handed
/// its parent's phase: forced straight to Resolve, it would never ask.
/// Pinhole's "access 1 card in the root of another server" is exactly such a
/// choice, so the class needs the property, not just the card.
#[test]
fn a_replacements_instructions_announce_their_own_targets() {
    let mut vm = Vm::empty(9004);
    let a = tk::install_root(&mut vm, tk::vanilla_asset("Target A", 0, 2), ServerId::Remote(1), true);
    let b = tk::install_root(&mut vm, tk::vanilla_asset("Target B", 0, 2), ServerId::Remote(2), true);
    tk::install_rig(
        &mut vm,
        tk::instead_resolve_card(
            "Pinhole-like",
            "pinhole: trash a card instead of breaching",
            EffectClass::Breach,
            vec![Instruction::TrashCards(jinteki_cr::instr::TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![jinteki_cr::instr::TargetFilter::InstalledCorpCard],
                up_to: false,
            })],
        ),
    );
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("pinhole:"))
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .when(Match::targets().once(), Reply::Targets(vec![b]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&b].zone,
        Zone::Discard(Side::Corp),
        "the chosen target was trashed: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.objects[&a].zone, Zone::Root(ServerId::Remote(1)), "the other one was not");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "and the breach still did not happen: {}",
        t.tail(20)
    );
}
