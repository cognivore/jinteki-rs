//! CR 9.1.6 — what it means to USE an ability, and what that spends.
//!
//! 9.3.6g's once-per-turn flag says an ability "can only be used once per
//! turn" and points at 9.1.6 for what using is. 9.1.6's second sentence is
//! the one the kernel used to ignore: *"Players do not 'use' abilities that
//! are entirely mandatory."* A mandatory ability is therefore never used, so
//! nothing expends its flag — and the printed sentence a designer means when
//! a mandatory ability happens once a turn is not the flag at all but
//! 9.6.5c's stipulation about the OCCURRENCE.
//!
//! The distinction was invisible while every card relying on it was unique.
//! It is not invisible on a non-unique card: 1.12.2's Vaporframe Fabricator
//! example makes the flag per OBJECT, so a second copy — or the same copy
//! reinstalled — carries a fresh one, while the occurrence a sentence counts
//! belongs to the turn.

use jinteki_cr::object::{ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Pick, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// CR 9.1.6: an entirely mandatory ability is never used, so 9.3.6g's flag
/// on one is never expended — it resolves as often as its condition is met.
/// Two copies of a NON-UNIQUE card, two runs: four credits, not two, and not
/// one.
#[test]
fn a_mandatory_once_per_turn_ability_is_never_used_so_its_flag_never_expends() {
    let mut vm = Vm::empty(9016);
    let a = tk::install_rig(&mut vm, tk::mandatory_once_per_turn_gainer("Flagged"));
    let b = tk::install_rig(&mut vm, tk::mandatory_once_per_turn_gainer("Flagged"));
    assert!(!vm.st.objects[&a].printed.unique, "the card is not unique");
    assert_ne!(a, b, "two objects of the same printed card");
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().times(2), Reply::run(ServerId::Archives))
            .when(Match::reaction(), Reply::take("opt-flag"))
            .stop_at_action(),
    );
    assert_eq!(
        t.times_taken("opt-flag"),
        4,
        "9.1.6: the ability is entirely mandatory, so it is never 'used' and \
         9.3.6g has nothing to expend — both copies resolve on both runs"
    );
    assert_eq!(vm.st.runner.credits, 4);
}

/// CR 9.6.5c: the sentence a card actually prints — "the first time each turn
/// <the condition>" — is a stipulation about the OCCURRENCE. Both copies
/// resolve on the first run of the turn, because that IS the first time; on
/// the second run neither does.
#[test]
fn the_first_time_each_turn_is_counted_per_occurrence_not_per_object() {
    let mut vm = Vm::empty(9017);
    tk::install_rig(&mut vm, tk::first_time_each_turn_gainer("Ordinal"));
    tk::install_rig(&mut vm, tk::first_time_each_turn_gainer("Ordinal"));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().times(2), Reply::run(ServerId::Archives))
            .when(Match::reaction(), Reply::take("first-each-turn"))
            .stop_at_action(),
    );
    assert_eq!(
        t.times_taken("first-each-turn"),
        2,
        "the first run of the turn is the first time for BOTH copies"
    );
    assert_eq!(t.offers("first-each-turn"), 2, "…and the second run is not offered at all");
    assert_eq!(vm.st.runner.credits, 2);
}

/// The case the flag gets wrong on a non-unique card: a copy installed AFTER
/// the first occurrence of the turn does not get a fresh "first time".
/// 1.12.2 would hand a new object a fresh 9.3.6g flag; 9.6.5c's stipulation
/// is about the turn's history (10.2.1), which the new object does not reset.
#[test]
fn a_copy_installed_after_the_first_occurrence_gets_no_fresh_first_time() {
    let mut vm = Vm::empty(9018);
    tk::install_rig(&mut vm, tk::first_time_each_turn_gainer("Ordinal"));
    let late = vm.new_object(tk::first_time_each_turn_gainer("Ordinal"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(late);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            // Run once — the first time this turn, so the installed copy
            // resolves — then install the second copy (5.2.7d) and run again.
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .when(Match::action().first(), Reply::Take(Pick::InstallCard(late)))
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .when(Match::reaction(), Reply::take("first-each-turn"))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&late].zone, Zone::Rig, "the second copy was installed mid-turn");
    assert_eq!(
        t.times_taken("first-each-turn"),
        1,
        "9.6.5c: the first run of the turn already happened, and a new object \
         does not make it happen again"
    );
    assert_eq!(vm.st.runner.credits, 1, "one occurrence, one credit");
}
