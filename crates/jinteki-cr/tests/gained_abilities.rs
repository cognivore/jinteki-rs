//! CR 9.1.9 in both directions: an object can LOSE abilities and GAIN them.
//!
//! The kernel could only do the first. `Effective::ability_present` was a
//! presence mask over `printed.abilities` and `AbilityRef` indexed that same
//! list, so "this card gains the text of that card" had nowhere to put the
//! text. 9.1.9b says the abilities an object actually has come out of
//! 9.12.1d/e's procedure — so they are a LIST the pipeline computes, and
//! every place that reads an object's abilities has to read that list.
//!
//! These tests take one ability of each kind through the gain: a static
//! declaration, a paid ability offered in a paid window, and a conditional
//! ability that pends at a checkpoint.

use jinteki_cr::object::{ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// A gained STATIC declaration applies — once. The guest is hosted without
/// being installed (1.13.2a), so it is inactive and its own copy of the
/// declaration does nothing; the host is active and has the ability now.
#[test]
fn a_gained_static_ability_applies_from_the_gaining_card() {
    let mut vm = Vm::empty(9101);
    let dj = tk::install_rig(&mut vm, tk::gains_text_of_hosted("Gainer"));
    let base = vm.memory_limit();
    let guest = vm.new_object(tk::three_ability_guest("Guest"), Zone::OutsideGame(Side::Runner));
    assert_eq!(vm.memory_limit(), base, "the guest is outside the game and inactive");

    tk::host_on_uninstalled(&mut vm, guest, dj);
    assert_eq!(
        vm.memory_limit(),
        base + 1,
        "9.1.9b: the host has the ability; 4.6.5h keeps the guest's own copy inactive"
    );

    vm.move_card(guest, Zone::OutsideGame(Side::Runner));
    assert_eq!(vm.memory_limit(), base, "nothing hosted, nothing gained");
}

/// CR 9.1.9a with 9.12.1d's ordering: a card that LOST its abilities has none
/// to lend. The removal and the copy are two characteristic effects and the
/// copy depends on the removal, so the pipeline applies them in that order —
/// which is 9.12.1d in person.
#[test]
fn a_guest_whose_abilities_were_removed_lends_nothing() {
    let mut vm = Vm::empty(9102);
    let dj = tk::install_rig(&mut vm, tk::gains_text_of_hosted("Gainer"));
    let base = vm.memory_limit();
    let guest = vm.new_object(tk::three_ability_guest("Guest"), Zone::OutsideGame(Side::Runner));
    tk::host_on_uninstalled(&mut vm, guest, dj);
    assert_eq!(vm.memory_limit(), base + 1);

    // A Direct-Access-class declaration: "identity cards do not have
    // abilities" (9.1.9a). The guest is an identity card.
    let blanker = tk::install_rig(&mut vm, tk::identity_blanker("Blanker"));
    assert_eq!(
        vm.memory_limit(),
        base,
        "9.1.9a/9.12.1d: the ability was removed before it could be copied"
    );

    // And it comes back when the removal does not apply any more (9.10:
    // characteristics are recomputed, never cached).
    vm.move_card(blanker, Zone::Discard(Side::Runner));
    assert_eq!(vm.memory_limit(), base + 1);
}

/// A gained PAID ability is offered where paid abilities are offered, and
/// using it resolves the gaining card's ability (9.1.1b: the object that has
/// the ability is its source).
#[test]
fn a_gained_paid_ability_is_offered_and_resolves() {
    let mut vm = Vm::empty(9103);
    let dj = tk::install_rig(&mut vm, tk::gains_text_of_hosted("Gainer"));
    let guest = vm.new_object(tk::three_ability_guest("Guest"), Zone::OutsideGame(Side::Runner));
    tk::host_on_uninstalled(&mut vm, guest, dj);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("guest-paid")).stop_at_action(),
    );
    assert_eq!(t.times_taken("guest-paid"), 1, "the gained paid ability was offered: {}", t.tail(8));
    assert_eq!(vm.st.runner.credits, 2, "…and resolved");
}

/// A gained CONDITIONAL ability meets its condition and pends at the
/// checkpoint like any other — the checkpoint's scan reads the abilities an
/// object HAS, not the ones printed on it.
#[test]
fn a_gained_conditional_ability_pends_and_resolves() {
    let mut vm = Vm::empty(9104);
    let dj = tk::install_rig(&mut vm, tk::gains_text_of_hosted("Gainer"));
    let guest = vm.new_object(tk::three_ability_guest("Guest"), Zone::OutsideGame(Side::Runner));
    tk::host_on_uninstalled(&mut vm, guest, dj);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::reaction(), Reply::take("guest-conditional"))
            .stop_at_action(),
    );
    assert_eq!(
        t.times_taken("guest-conditional"),
        1,
        "the gained conditional pended when the run ended: {}",
        t.tail(10)
    );
    assert_eq!(vm.st.runner.credits, 1);
}
