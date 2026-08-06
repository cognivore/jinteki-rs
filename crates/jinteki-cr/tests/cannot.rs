//! CR 1.2.2 — "If a rule or ability directs something to happen, but another
//! effect states that it cannot happen, the 'cannot' ability takes precedence."
//!
//! The kernel's prohibition used to know two acts (scoring, rezzing) and one
//! way of picking its cards (one object, named when the effect was created).
//! Neither limit is in the rules. 1.2.2 is about ANY act, and 9.10.1's
//! lingering effect is created once, expires on its own stated duration and
//! exists independently of its source — over cards a sentence may equally well
//! DESCRIBE ("agendas", "Corp cards") as name ("that card").
//!
//! What makes the description a different thing and not a spelling is 1.15.2.
//! A describing position resolves through the instruction's ANNOUNCED targets,
//! and `Instruction::CreateLingeringEffect` announces none — deliberately,
//! because 9.10.1's position describes rather than targets. So a prohibition
//! written over the naming position with a description in it reaches the empty
//! set: it forbids nothing, and says nothing about forbidding nothing. The
//! last test here is that the kernel now refuses that spelling outright.
//!
//! Every test drives the prohibition on a board and asserts the act it names
//! did not happen, against a control on the identical board without it.

use jinteki_cr::change::GameChange;
use jinteki_cr::decision::WindowOption;
use jinteki_cr::instr::TargetFilter;
use jinteki_cr::lingering::{ProhibitedAction, WantedDuration};
use jinteki_cr::object::{CardType, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Pick, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

fn offered(t: &plan::Transcript) -> Vec<WindowOption> {
    t.entries.iter().flat_map(|e| e.options().iter().cloned()).collect()
}

/// CR 7.5 / 7.2.3: stealing is an act a "cannot" can forbid.
///
/// Stealing is not an option the Runner takes — 7.2.3 makes it happen during
/// the access and 1.17.3 makes it mandatory when it costs nothing — so 1.2.2's
/// precedence shows up as the steal simply not happening while the access
/// carries on. The agenda is accessed and stays exactly where it was, which is
/// the shape Haarpsichord Studios' limit already has and a Pinhole-class
/// access restriction has too.
///
/// The scope is a DESCRIPTION ("agendas"), so the run is made on an agenda the
/// prohibition never saw: it was installed before the effect existed and is
/// reached anyway, because the criteria are re-read at step 7.2.3 rather than
/// resolved once.
#[test]
fn a_cannot_over_a_description_stops_the_steal_and_leaves_the_access_alone() {
    for prohibited in [false, true] {
        let mut vm = Vm::empty(9401);
        let button = tk::install_root(
            &mut vm,
            tk::prohibiting_asset(
                "Vertigo Class",
                Some(Side::Runner),
                vec![ProhibitedAction::Steal],
                vec![TargetFilter::CardTypeIs(CardType::Agenda)],
                WantedDuration::ThisTurn,
            ),
            ServerId::Remote(1),
            true,
        );
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Loose Agenda", 3, 2),
            ServerId::Remote(2),
            false,
        );
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let corp = if prohibited {
            Plan::corp().when(Match::paid().once(), Reply::take("cannot:"))
        } else {
            Plan::corp()
        };
        let t = plan::play(
            &mut vm,
            corp,
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
                .stop_at_action(),
        );

        assert!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == agenda)),
            "7.2.3: the agenda was accessed either way — the prohibition is on the steal, \
             not on the access (prohibited={prohibited}): {}",
            t.tail(40)
        );
        let want = if prohibited {
            Zone::Root(ServerId::Remote(2))
        } else {
            Zone::ScoreArea(Side::Runner)
        };
        assert_eq!(
            vm.st.objects[&agenda].zone, want,
            "1.2.2/7.2.3: the agenda is accessed either way and stolen only when nothing \
             forbids it (prohibited={prohibited}): {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&button].zone,
            Zone::Root(ServerId::Remote(1)),
            "the source is untouched by any of it (prohibited={prohibited})"
        );
    }
}

/// CR 7.1.5 / 1.19.4: trashing is an act a "cannot" can forbid, and 1.2.2
/// removes the OPTION rather than making it fail.
///
/// The basic trash ability is 7.1.4's one mid-access opportunity, and the
/// Runner can afford it here — 3[credit] against a trash cost of 2. Prohibited,
/// it is not put to the Runner at all. An unofferable act and an act that
/// fizzles are observably different to a player, and 1.2.2 says the former.
#[test]
fn a_cannot_on_trashing_withholds_the_basic_trash_option_rather_than_failing_it() {
    for prohibited in [false, true] {
        let mut vm = Vm::empty(9402);
        tk::install_root(
            &mut vm,
            tk::prohibiting_asset(
                "Vertigo Class",
                Some(Side::Runner),
                vec![ProhibitedAction::Trash],
                vec![TargetFilter::InstalledCorpCard],
                WantedDuration::ThisTurn,
            ),
            ServerId::Remote(1),
            true,
        );
        let target =
            tk::install_root(&mut vm, tk::vanilla_asset("Trashable", 0, 2), ServerId::Remote(2), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 3;
        vm.start_turn(Side::Runner);

        let corp = if prohibited {
            Plan::corp().when(Match::paid().once(), Reply::take("cannot:"))
        } else {
            Plan::corp()
        };
        let t = plan::play(
            &mut vm,
            corp,
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
                .when(Match::mid_access().once(), Reply::Take(Pick::BasicTrash))
                .stop_at_action(),
        );

        let was_offered = offered(&t)
            .iter()
            .any(|o| matches!(o, WindowOption::BasicTrash { card, .. } if *card == target));
        assert_eq!(
            was_offered, !prohibited,
            "1.2.2/7.1.5: the basic trash ability is WITHHELD, not failed, and the Runner \
             could always pay for it (prohibited={prohibited}): {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&target].zone == Zone::Discard(Side::Corp),
            !prohibited,
            "…so the card is trashed only when nothing forbids it (prohibited={prohibited}): {}",
            t.tail(40)
        );
    }
}

/// CR 1.2.2: the "cannot" reaches a trash an ABILITY directs, not only the
/// basic trash ability's option — and it reaches it for the named player and
/// nobody else.
///
/// Both players hold a button that trashes the same Corp card, and the printed
/// sentence names the Runner. 9.9.2 leaves nothing expected of a trash the
/// prohibition covers, so the Runner's button does nothing at all; the Corp's
/// trashes the very same card a moment later. That is the whole reason WHO is
/// content on the effect: trashing is the one act both players perform, and a
/// sentence forbidding the Runner must not disarm the Corp.
#[test]
fn a_cannot_naming_a_player_stops_that_players_directed_trash_and_no_ones_else() {
    let mut vm = Vm::empty(9403);
    tk::install_root(
        &mut vm,
        tk::prohibiting_asset(
            "Vertigo Class",
            Some(Side::Runner),
            vec![ProhibitedAction::Trash],
            vec![TargetFilter::InstalledCorpCard],
            WantedDuration::ThisTurn,
        ),
        ServerId::Remote(1),
        true,
    );
    let victim =
        tk::install_root(&mut vm, tk::vanilla_asset("Victim", 0, 2), ServerId::Remote(2), true);
    tk::install_rig(&mut vm, tk::trash_set_button("Runner's Button", vec![victim]));
    tk::install_root(
        &mut vm,
        tk::trash_set_button_of(Side::Corp, "Corp's Button", vec![victim]),
        ServerId::Remote(3),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // The Runner passes until the Corp's window has created the prohibition,
    // so the button is used against a "cannot" that is already standing.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("cannot:"))
            .when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::paid().times(2), Reply::Pass)
            .when(Match::paid().once(), Reply::take("trash the set"))
            .when(Match::paid(), Reply::Pass)
            .stop_at_action(),
    );
    assert!(
        t.entries.iter().any(|e| e.side == Side::Runner
            && e.answer.is_some()
            && format!("{:?}", e.answer).contains("trash the set")),
        "the Runner did use the button: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&victim].zone,
        Zone::Root(ServerId::Remote(2)),
        "1.2.2: the Runner's ability directed the trash and the 'cannot' beat it: {}",
        t.tail(40)
    );

    let t2 = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("trash the set"))
            .when(Match::paid(), Reply::Pass),
        Plan::runner().when(Match::paid(), Reply::Pass).otherwise_click_credit(),
    );
    assert_eq!(
        vm.st.objects[&victim].zone,
        Zone::Discard(Side::Corp),
        "…and the sentence named the Runner, so the Corp trashes its own card as ever: {}",
        t2.tail(40)
    );
}

/// CR 9.10.1 + 1.15.2: a prohibition written over the position that NAMES its
/// cards, with a description in it, reaches nothing — and the kernel refuses
/// it rather than build it.
///
/// This is the failure this whole shape exists to prevent, and it is not
/// hypothetical: a card was once built this way, passed its build, looked
/// implemented and forbade nothing, silently. A prohibition that reaches no
/// object because it was written in the wrong position is a card-definition
/// bug, and it is a bug about a rule that 1.2.2 makes absolute — so it stops
/// the first time the card is driven instead of never.
///
/// Nothing about the refusal depends on the board: whether a spec describes or
/// names is a property of the definition, so a card that passes it passes it
/// everywhere.
#[test]
#[should_panic(expected = "would forbid nothing at all")]
fn a_prohibition_written_over_a_description_in_the_naming_position_is_refused() {
    let mut vm = Vm::empty(9404);
    tk::install_root(
        &mut vm,
        tk::misdescribing_prohibiting_asset("Wrongly Written"),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(&mut vm, tk::vanilla_agenda("Loose Agenda", 3, 2), ServerId::Remote(2), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("cannot:")).stop_at_action(),
        Plan::runner(),
    );
}
