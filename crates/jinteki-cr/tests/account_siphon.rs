//! Account Siphon, end to end — the "prove a fucky eternal staple actually
//! works" test. The REAL card from `cards::account_siphon()`, played through
//! the REAL basic play action (5.2.7e), no test backdoors:
//!
//!   "Run HQ. If successful, instead of breaching HQ, you may force the Corp
//!    to lose up to 5[credit], then you gain 2[credit] for each credit lost
//!    and take 2 tags."
//!
//! Everything subtle about the card gets its own assertion: the breach is
//! REPLACED (an agenda in HQ survives untouched), the run is nonetheless
//! SUCCESSFUL (Desperado pays out), "up to 5" is the observed 1.10.3b loss
//! (a 3-credit Corp loses 3 and the Runner gains 6, not 10), the tags are
//! real, and declining the "may" produces an ordinary breach instead.

use jinteki_cr::cards;
use jinteki_cr::change::GameChange;
use jinteki_cr::object::{ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// Corp with `credits` in the pool and an agenda + a filler in HQ; Runner
/// holding the real Account Siphon, with the real Desperado installed.
fn setup(credits: u32, with_desperado: bool) -> (Vm, jinteki_cr::object::ObjectId, jinteki_cr::object::ObjectId) {
    let mut vm = Vm::empty(31337);
    let agenda = vm.new_object(tk::vanilla_agenda("Government Takeover-ish", 6, 3), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    let filler = vm.new_object(tk::corp_filler("Filler"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(filler);
    vm.st.corp.credits = credits;

    let siphon = vm.new_object(cards::account_siphon(), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(siphon);
    if with_desperado {
        tk::install_rig(&mut vm, cards::desperado());
    }
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);
    (vm, agenda, siphon)
}

#[test]
fn siphon_replaces_the_breach_but_the_run_is_still_successful() {
    let (mut vm, agenda, siphon) = setup(8, true);
    let runner_before = vm.st.runner.credits;

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(true))
            .stop_at_action(),
    );

    // The Corp lost exactly 5 — one forced loss, observed via the change log.
    assert_eq!(vm.st.corp.credits, 3, "8 - 5: {}", t.tail(14));
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CreditsLost { side: Side::Corp, amount: 5, .. }
        )),
        "the forced loss is a real CreditsLost event"
    );

    // The Runner banked 2 per credit lost (10) — plus Desperado's 1, which is
    // the observational proof that a replaced breach still leaves the run
    // SUCCESSFUL (6.8.4; the classic Siphon+Desperado interaction).
    assert_eq!(
        vm.st.runner.credits,
        runner_before + 10 + 1,
        "2 for each of 5 lost, plus Desperado: {}",
        t.tail(14)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::RunDeclaredSuccessful { server: ServerId::Hq }
        )),
        "the run was declared successful"
    );

    // Two real tags.
    assert_eq!(vm.st.runner.tags, 2, "took 2 tags");

    // And the breach NEVER happened: no breach began, and the agenda sits in
    // HQ untouched — "instead of breaching HQ" meant exactly that.
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "no breach of HQ occurred: {}",
        t.tail(14)
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::Hand(Side::Corp), "the agenda was never accessed");
    assert_eq!(vm.score(Side::Runner), 0, "nothing stolen");
}

#[test]
fn up_to_five_means_the_observed_loss_not_the_number_printed() {
    // A Corp with 3 credits loses 3 (1.10.3b takes what the pool holds), and
    // the Runner gains 2 FOR EACH CREDIT LOST — 6, not 10.
    let (mut vm, _, siphon) = setup(3, false);
    let runner_before = vm.st.runner.credits;

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(true))
            .stop_at_action(),
    );

    assert_eq!(vm.st.corp.credits, 0, "{}", t.tail(12));
    assert!(vm.changes.log.iter().any(|c| matches!(
        c,
        GameChange::CreditsLost { side: Side::Corp, amount: 3, .. }
    )));
    assert_eq!(vm.st.runner.credits, runner_before + 6, "2 × the 3 actually lost");
    assert_eq!(vm.st.runner.tags, 2, "the tags do not scale with the loss");
}

#[test]
fn declining_the_replacement_breaches_hq_normally() {
    // "you may" — saying no means the ordinary breach happens and the agenda
    // is right there to steal.
    let (mut vm, agenda, siphon) = setup(8, false);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(false))
            .stop_at_action(),
    );

    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "declined replacement: the breach proceeds: {}",
        t.tail(14)
    );
    assert_eq!(vm.st.corp.credits, 8, "nobody was siphoned");
    assert_eq!(vm.st.runner.tags, 0, "no tags either");
    // The 2-card HQ was breached; whether the random access found the agenda
    // is seed-dependent, but SOME access happened.
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { .. })),
        "at least one HQ card was accessed: {}",
        t.tail(14)
    );
    let _ = agenda;
}
