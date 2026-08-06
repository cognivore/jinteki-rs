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

/// CR 9.6.5c over CR 1.16.4d: the ordinal counts the times the condition was
/// MET, and 1.16.4d's condition is met by the Nth [click] spent on an action
/// and not by the ones before it.
///
/// The two rules are read from the same records — every [click] spent is one
/// `ClickSpent` — so an ordinal that asks only "did an earlier change of this
/// shape occur?" is spent by click ONE of a three-click action, and click
/// three, the one the sentence is about, is then refused as a repeat. A card
/// printing "the first time you spend 3[click] on the same action each turn"
/// would never fire at all. 9.6.5c is explicit that a requirement listed in a
/// trigger condition is PART of the condition, so a change that did not meet
/// the requirement was never one of "the times".
///
/// Both halves of the ordinal are asserted, and the two `count`s are what
/// separate them:
///
/// * `count = 3` — one basic purge (5.2.6h), three clicks, one action. The
///   condition is met once, at the third click, and the ordinal must permit
///   it. Ordinal or no ordinal, the payout is the same.
/// * `count = 1` — three basic draw actions, one click each. The condition is
///   met three times, and here the ordinal MUST bite: one payout with it and
///   three without. Draws rather than credits, so the actions themselves put
///   nothing in the pool the ability is measured in.
#[test]
fn the_ordinal_over_clicks_spent_on_an_action_is_not_spent_by_the_earlier_clicks() {
    for (count, ordinal, want) in [(3u32, false, 1u32), (3, true, 1), (1, false, 3), (1, true, 1)] {
        let mut vm = Vm::empty(9020);
        let shape = if ordinal {
            tk::jeeves_like_first_each_turn("Jeeves-like", count)
        } else {
            tk::jeeves_like("Jeeves-like", count)
        };
        tk::install_root(&mut vm, shape, ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 12);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let corp = if count == 3 {
            // 5.2.6h: one action, and it costs the Corp's whole turn.
            Plan::corp().when(Match::action().once(), Reply::Take(Pick::Purge))
        } else {
            Plan::corp()
        };
        let t = plan::play(
            &mut vm,
            corp.when(Match::action(), Reply::draw()),
            // Stop once the Corp's whole turn has gone by.
            Plan::runner().when(Match::action().first(), Reply::Halt),
        );

        assert_eq!(
            vm.st.corp.credits, want,
            "9.6.5c/1.16.4d: count={count}, ordinal={ordinal}: {}",
            t.tail(30)
        );
    }
}

/// CR 8.6.6d, as a nested CONDITIONAL ability rather than a step of the play.
///
/// "If an ability that plays an event or operation also contains the nested
/// conditional ability 'After it resolves, remove it from the game.', the
/// event or operation is not trashed. The card remains in the play area until
/// the conditional ability removes it from the game." A conditional ability's
/// condition is met at 8.6.7h and it resolves in the reaction window the
/// following checkpoint opens (10.3.2) — so anything else whose condition (h)
/// also met is in that same window, and finds the played card still in the
/// play area. The kernel used to remove it as the play step's own last act,
/// which is a step too early.
#[test]
fn the_8_6_6d_removal_resolves_in_the_reaction_window_after_the_play() {
    let mut vm = Vm::empty(9019);
    let op = vm.new_object(
        tk::operation_with_after_resolve(
            "Flashback",
            0,
            vec![jinteki_cr::instr::Instruction::GainCredits(
                Side::Corp,
                jinteki_cr::instr::Quantity::c(1),
            )],
            vec![jinteki_cr::instr::Instruction::GainCredits(
                Side::Corp,
                jinteki_cr::instr::Quantity::c(2),
            )],
        ),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
    tk::install_root(&mut vm, tk::play_operation_button_rfg("Player", op), ServerId::Remote(1), true);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("play-op-rfg"))
            // 9.1.2a: the controller orders the instances in the window. Take
            // the card's own rider first — it can only resolve because the
            // card is still in the play area, and therefore still active.
            .when(Match::reaction(), Reply::take("after-resolve rider"))
            .when(Match::reaction(), Reply::take("remove it from the game"))
            .stop_at_action(),
        Plan::runner(),
    );
    let together = t
        .entries
        .iter()
        .any(|e| match &e.spec {
            jinteki_cr::decision::DecisionSpec::ReactionWindow { options, .. } => {
                let labels: Vec<&str> = options
                    .iter()
                    .map(|o| match o {
                        jinteki_cr::decision::WindowOption::TriggerInstance { label, .. } => *label,
                        _ => "",
                    })
                    .collect();
                labels.iter().any(|l| l.contains("after-resolve rider"))
                    && labels.iter().any(|l| l.contains("remove it from the game"))
            }
            _ => false,
        });
    assert!(
        together,
        "8.6.6d/10.3.2: the removal is a pending conditional in the same \
         reaction window as everything else (h) met, not a step of the play"
    );
    assert_eq!(vm.st.corp.credits, 3, "the play ability gained 1 and the rider gained 2");
    assert_eq!(
        vm.st.objects[&op].zone,
        Zone::RemovedFromGame,
        "8.6.6d: not trashed — removed from the game by the nested conditional"
    );
}

/// CR 1.14.4b — an ability that names the player who may use it.
///
/// 1.14.4 makes the controller of an ability the controller of its source "by
/// default", and "a player can only use abilities they control". 1.14.4b is
/// the clause that was missing from the kernel: "Some abilities state that
/// they can only be used by a specific player. The specified player controls
/// each such ability, **even if they do not control its source**."
///
/// This is every Bioroid in the game. The break ability is printed on the
/// Corp's ice; the Runner is the one who may use it, and 1.14.3 spends the
/// RUNNER's click to do it. Before this, the option was filtered by the
/// controller of the OBJECT, so the ability was offered to the Corp — the
/// player who can never use it — and to nobody else.
///
/// Both halves are asserted, because 1.14.4b says both: the named player may,
/// and the source's controller may not.
#[test]
fn an_ability_naming_its_user_goes_to_that_player_and_not_to_the_cards_controller() {
    let mut vm = Vm::empty(4411);
    tk::install_ice(&mut vm, tk::bioroid_ice("Bioroid-like", 0, 1), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().offering("bioroid").once(), Reply::take("bioroid"))
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("End the run"))
            .stop_at_action(),
    );

    // The Runner was offered it — 1.14.4b's named player controls it.
    assert!(
        t.ever_offered("bioroid"),
        "1.14.4b: the Runner controls an ability that says only they can use \
         it, even though the Corp controls the ice: {}",
        t.tail(20)
    );
    // …and the Corp never was, which is the other half of the same sentence.
    let offered_to_corp = t
        .windows(jinteki_cr::plan::Kind::Paid, Side::Corp)
        .iter()
        .any(|e| e.options().iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::TriggerPaid { label, .. } if label.contains("bioroid")
        )));
    assert!(
        !offered_to_corp,
        "1.14.4: a player can only use abilities they control, and 1.14.4b \
         took this one away from the source's controller: {}",
        t.tail(20)
    );
    // 1.14.3/1.14.5: the ability's controller carries it out and pays for it
    // with objects THEY control — the click came off the Runner.
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, jinteki_cr::change::GameChange::RunDeclaredUnsuccessful { .. })),
        "the subroutine was broken, so the run was not ended: {}",
        t.tail(20)
    );
}
