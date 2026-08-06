//! CR 6.5.7 — fully breaking, and WHO does it.
//!
//! 6.5.7a is one occurrence with two fully-breakers in it. The Runner always
//! fully breaks the encountered ice the first time all its subroutines are
//! broken; 6.5.7b adds a second, conditional one — *"if all its subroutines
//! were broken using abilities on a single object, that object also fully
//! breaks the ice"*. That object is not always there, and the rule says so
//! twice: the condition can simply fail when two breakers share one piece of
//! ice, and 6.5.7c settles the vacuous case in as many words — ice with no
//! subroutines is fully broken by the Runner and *"no objects fully break the
//! ice in this case"*.
//!
//! So `GameChange::AllSubsBroken` names the ice unconditionally and the
//! object optionally, and a card's "fully breaks" sentence has to say which of
//! the two it means: "when the Runner fully breaks **this ice**" (Paper Wall)
//! is the ice, "whenever **this program** fully breaks a piece of ice"
//! (Bukhgalter, Curupira, Cleaver) is the object. Both readings are one
//! condition, `TriggerCond::SelfFullyBroken { by_source }`.

use jinteki_cr::cards;
use jinteki_cr::change::GameChange;
use jinteki_cr::object::{ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// The full break recorded for `ice`, if the log holds one.
fn full_break_of(vm: &Vm, ice: jinteki_cr::object::ObjectId) -> Option<Option<jinteki_cr::object::ObjectId>> {
    vm.changes.log.iter().find_map(|c| match c {
        GameChange::AllSubsBroken { ice: i, by } if *i == ice => Some(*by),
        _ => None,
    })
}

/// CR 6.5.7b: one object broke every subroutine, so that object also fully
/// breaks the ice — and the record says which object it was.
#[test]
fn an_object_that_broke_every_subroutine_also_fully_breaks_the_ice() {
    let mut vm = Vm::empty(6570);
    let ice = tk::install_ice(&mut vm, tk::heimdall_like("Heimdall-like"), ServerId::Hq, true);
    let button = tk::install_rig(&mut vm, tk::break_button("Break-button"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // One free breaker, three subroutines, three uses of it: nothing else on
    // the board broke anything.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").times(3), Reply::take("break: 1"))
            .stop_at_action(),
    );

    assert_eq!(
        full_break_of(&vm, ice),
        Some(Some(button)),
        "6.5.7a/b: the Runner fully broke the ice, and all three subroutines were \
         broken using abilities on the one object, so it fully broke the ice too: {}",
        t.tail(24)
    );
}

/// CR 6.5.7b: "…using abilities on a SINGLE object". Two breakers sharing one
/// piece of ice is the case where the Runner fully breaks it and no object
/// does — the condition is a claim about all the subroutines at once, not
/// about whoever happened to break the last one.
#[test]
fn two_breakers_sharing_one_piece_of_ice_means_no_object_fully_breaks_it() {
    let mut vm = Vm::empty(6571);
    let ice = tk::install_ice(&mut vm, tk::heimdall_like("Heimdall-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::cleaver_like("Cleaver-like", 6));
    tk::install_rig(&mut vm, tk::break_button("Break-button"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    // Cleaver takes two of the three subroutines; the free button takes the
    // last one. Every subroutine is broken and the Runner fully breaks the
    // ice — by two objects' abilities, which is 6.5.7b's failing case.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("cleaver"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("break: 1"))
            .stop_at_action(),
    );

    assert_eq!(
        full_break_of(&vm, ice),
        Some(None),
        "6.5.7a/b: the Runner fully broke the ice, but its subroutines were broken \
         using abilities on TWO objects, so neither of them fully broke it: {}",
        t.tail(24)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { .. })),
        "the break really happened — nothing was left to resolve: {}",
        t.tail(24)
    );
}

/// CR 6.5.7c: ice with no subroutines is fully broken by the Runner when step
/// 6.9.3b begins, and "no objects fully break the ice in this case" — there
/// was no breaking for an object to have done.
#[test]
fn ice_with_no_subroutines_is_fully_broken_by_the_runner_and_by_no_object() {
    let mut vm = Vm::empty(6572);
    let troll = tk::install_ice(&mut vm, tk::troll_like("Troll-like"), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::forked_button("Forked-like", ServerId::Remote(1)));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("forked")).stop_at_action(),
    );

    assert_eq!(
        full_break_of(&vm, troll),
        Some(None),
        "6.5.7c: zero subroutines are vacuously all-broken at step 6.9.3b, and no \
         object fully breaks the ice: {}",
        t.tail(24)
    );
}

/// The two readings are different sentences about the same moment. Paper Wall
/// prints the ice-side one — "when the Runner fully breaks **this ice**,
/// trash it" — and it is met by a break some OTHER object performed, which is
/// the whole point of 6.5.7a naming the Runner rather than the breaker.
#[test]
fn the_ice_side_sentence_is_met_by_a_break_another_object_performed() {
    let mut vm = Vm::empty(6573);
    let wall = tk::install_ice(&mut vm, cards::paper_wall(), ServerId::Hq, true);
    let button = tk::install_rig(&mut vm, tk::break_button("Break-button"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("break: 1"))
            .stop_at_action(),
    );

    assert_eq!(
        full_break_of(&vm, wall),
        Some(Some(button)),
        "6.5.7b: the breaker was the button, not the wall: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&wall].zone,
        Zone::Discard(Side::Corp),
        "6.5.7a: Paper Wall's sentence is about the ice that was fully broken, so it \
         is met however the breaking was done, and the wall trashed itself: {}",
        t.tail(24)
    );
}
