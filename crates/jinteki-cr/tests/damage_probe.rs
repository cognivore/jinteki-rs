//! A directly-asked-for probe: 4 net damage into a 2-card grip.
//!
//! Not a CR worked example — this is the situation a reader asked to see
//! proved, and it sits on top of a defect W14 fixed: damage trashed cards with
//! a bare move, so the trash was not a trash *event* and nothing could observe
//! or replace it.
//!
//! What the CR says (10.4.2a, 10.4.3, 1.7.2b): the damage trashes cards at
//! random from the grip, all at once as ONE occurrence; a grip of 2 cannot pay
//! 4, so the Runner is flatlined and the game ends there — including before
//! any ability that just triggered gets to resolve (10.3.1c precedes the
//! window that would resolve it).

use jinteki_cr::change::GameChange;
use jinteki_cr::decision::GameResult;
use jinteki_cr::effects::DamageKind;
use jinteki_cr::object::{ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// Keep R&D non-empty so a later mandatory draw cannot end the game for an
/// unrelated reason (the Corp loses on drawing from an empty R&D).
fn stock_rnd(vm: &mut Vm, n: usize) {
    for _ in 0..n {
        let c = vm.new_object(tk::corp_filler("Filler"), Zone::Deck(Side::Corp));
        vm.st.deck.get_mut(&Side::Corp).unwrap().push(c);
    }
}

#[test]
fn four_net_damage_into_a_two_card_grip_flatlines() {
    let mut vm = Vm::empty(4242);
    stock_rnd(&mut vm, 5);
    tk::install_root(&mut vm, tk::net_damage_button("NetDamage-4", 4), ServerId::Remote(1), true);

    // Exactly two cards in the grip; one of them watches for its own
    // damage-trash ("When this card is trashed by damage, gain 3[c]").
    let watcher = vm.new_object(tk::ive_had_worse_like("IHW-like", &[DamageKind::Meat, DamageKind::Net, DamageKind::Core]), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(watcher);
    let other = tk::fill_hand(&mut vm, Side::Runner, 1);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("do net damage")).stop_at_action(),
        Plan::runner().stop_at_action(),
    );

    // 1. ONE damage occurrence of 4, taking the whole grip at once (10.4.3).
    let dmg: Vec<&GameChange> = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .collect();
    assert_eq!(dmg.len(), 1, "one occurrence, not four: {}", t.tail(10));
    match dmg[0] {
        GameChange::DamageSuffered { amount, cards, .. } => {
            assert_eq!(*amount, 4, "the full 4 damage is suffered");
            assert_eq!(cards.len(), 2, "a 2-card grip yields exactly 2 trashes");
        }
        _ => unreachable!(),
    }

    // 2. The cards LEFT BY BEING TRASHED (10.4.2a) — the assertion the pre-W14
    //    kernel failed, because damage moved them without trashing them.
    let trashes: Vec<&GameChange> = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardTrashed { was_zone: Zone::Hand(Side::Runner), .. }))
        .collect();
    assert_eq!(trashes.len(), 2, "both grip cards were trashed: {}", t.tail(10));
    for g in std::iter::once(&watcher).chain(other.iter()) {
        assert!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::CardTrashed { obj, .. } if obj == g)),
            "grip card {g:?} produced a CardTrashed change"
        );
    }

    // 3. The Runner is flatlined and the game is over (1.7.2b).
    assert_eq!(t.result, Some(GameResult::Flatline), "2 cards cannot pay 4 damage");
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip is empty afterwards");

    // 4. …and the game ending WINS the race against the ability that just
    //    triggered: checkpoint step 10.3.1c ends the game before step 10.3.2
    //    opens the window that would resolve the "when trashed by damage"
    //    conditional. So the Runner never banks the 3 credits.
    assert_eq!(
        vm.st.runner.credits, 0,
        "flatline precedes the triggered ability's resolution: {}",
        t.tail(12)
    );
}

/// The same 4 net damage into a grip of exactly 4: every card is trashed, but
/// the damage did not exceed the grip, so there is no flatline — and now the
/// "when this card is trashed by damage" ability DOES resolve, which is the
/// ability-level proof that a damage trash is a real, observable trash.
#[test]
fn four_net_damage_into_a_four_card_grip_empties_it_without_a_flatline() {
    let mut vm = Vm::empty(4243);
    stock_rnd(&mut vm, 5);
    tk::install_root(&mut vm, tk::net_damage_button("NetDamage-4", 4), ServerId::Remote(1), true);
    let watcher = vm.new_object(tk::ive_had_worse_like("IHW-like", &[DamageKind::Meat, DamageKind::Net, DamageKind::Core]), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(watcher);
    tk::fill_hand(&mut vm, Side::Runner, 3);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("do net damage")).stop_at_action(),
        Plan::runner().stop_at_action(),
    );

    assert_eq!(t.result, None, "damage equal to the grip is survivable");
    assert!(vm.st.hand[&Side::Runner].is_empty(), "all four cards were trashed");
    assert_eq!(
        vm.st.runner.credits, 3,
        "the trashed card's 'when trashed by damage' ability resolved: {}",
        t.tail(12)
    );
}
