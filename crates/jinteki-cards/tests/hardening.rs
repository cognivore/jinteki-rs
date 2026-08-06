//! The hardening wave: the branches the first pass under-tested.
//!
//! The audit of 2026-08-05 found four systematically untested branch families
//! across the implemented identities — decline paths, second-trigger-same-turn,
//! cross-turn reset, and cannot-pay boundaries — and both of its real bugs
//! (Null: Whistleblower's unspent once-per-turn flag; Ryō Ōno's ordinal) lived
//! exactly there. Each test below drives ONE such branch with the shared plan
//! driver and asserts an observable outcome: credits, tags, counters, zones,
//! offer counts, or a change-log position.
//!
//! The two flag disciplines under test, throughout:
//! - CR 9.6.5c: a printed "the first time each turn" is about the OCCURRENCE,
//!   so declining the offer does not un-happen the occurrence — the ordinal is
//!   spent and no second offer comes this turn.
//! - CR 9.1.6b / 9.3.6g: a printed "Once per turn →" flag is spent by USING
//!   the ability, and a declined optional ability was never used — so the
//!   offer comes back on the next occurrence of the same turn.

use jinteki_cr::Subtype;

use jinteki_cr::change::GameChange;
use jinteki_cr::object::{CardType, CounterKind, PrintedCard, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Kind, Match, Pick, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// The card as the deck module writes it — complete, or the test cannot claim
/// the printed sentence is expressed.
fn card(name: &str) -> PrintedCard {
    let c = jinteki_cards::find(name)
        .unwrap_or_else(|| panic!("no card named {name} in any deck"));
    assert!(
        c.is_complete(),
        "{name} still carries an `.unimplemented(…)` marker — it cannot be asserted as playable"
    );
    c.printed
}

// ---------------------------------------------------------------------------
// A. Decline, then trigger again the same turn
// ---------------------------------------------------------------------------

/// Zahya (CR 9.1.6b): declining the once-per-turn offer is not a use, so the
/// second HQ run of the same turn is offered again — and taking it pays.
#[test]
fn zahya_declined_offer_returns_on_the_second_run_of_the_turn() {
    let mut vm = Vm::empty(8001);
    tk::install_identity(&mut vm, card("Zahya Sadeghi: Versatile Smuggler"), Side::Runner);
    tk::fill_hand(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            // No rule for the first offer: the plan's fallback declines it.
            .when(
                Match::reaction().offering("versatile smuggler").nth(2),
                Reply::take("versatile smuggler"),
            )
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("versatile smuggler"),
        2,
        "9.1.6b: the declined offer was never a use, so the second run end offers again: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.runner.credits, 1,
        "taken on the second run — one access, one credit — and nothing off the declined first: {}",
        t.tail(40)
    );
}

/// 419 (CR 9.6.5c): the Runner declines the first-install offer, so the Corp
/// is never asked to pay — and the ordinal is spent, so a second install the
/// same turn offers nothing.
#[test]
fn four_one_nine_declined_still_spends_the_turns_first_install() {
    let mut vm = Vm::empty(8002);
    tk::install_identity(&mut vm, card("419: Amoral Scammer"), Side::Runner);
    let first = vm.new_object(tk::vanilla_asset("First Install", 0, 2), Zone::Hand(Side::Corp));
    let second = vm.new_object(tk::vanilla_asset("Second Install", 0, 2), Zone::Hand(Side::Corp));
    for id in [first, second] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(first)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(second)))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        t.offers("419"),
        1,
        "9.6.5c: offered on the first install only — the decline spent the ordinal: {}",
        t.tail(40)
    );
    assert!(
        t.of_kind(Kind::NestedCost).is_empty(),
        "the Runner declined, so the Corp was never asked for its 1[credit]: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardExposed { .. })),
        "and nothing was exposed: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.corp.credits, 5, "the Corp kept its credits: {}", t.tail(40));
}

/// Hayley Kaplan (CR 9.6.5c): a declined first-install offer does not come
/// back on the second install of the same turn.
#[test]
fn hayley_kaplan_declined_offer_does_not_return_this_turn() {
    let mut vm = Vm::empty(8003);
    tk::install_identity(&mut vm, card("Hayley Kaplan: Universal Scholar"), Side::Runner);
    let mut mk = |name: &'static str, cost: u32| {
        let mut c = tk::vanilla_runner_card(name, CardType::Program);
        c.cost = Some(cost);
        c.memory_cost = Some(1);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let first = mk("First Program", 0);
    let second = mk("Second Program", 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(first)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(second)))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("universal scholar"),
        1,
        "9.6.5c: the declined offer was the turn's one — the second install meets nothing: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&second].zone,
        Zone::Rig,
        "the second install is the basic action's own, not the ability's: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.runner.credits, 3, "5 − 0 − 2, both at full price: {}", t.tail(40));
}

/// Barry "Baz" Wong prints "whenever", NOT an ordinal (CR 9.6.4b): a declined
/// rez does not exhaust anything, and the next rez of the same turn offers
/// again.
#[test]
fn barry_baz_wong_whenever_offers_again_after_a_decline() {
    let mut vm = Vm::empty(8004);
    tk::install_identity(&mut vm, card("Barry \"Baz\" Wong: Tri-Maf Veteran"), Side::Runner);
    tk::install_ice(&mut vm, tk::vanilla_ice("Inner Wall", 0, 1), ServerId::Archives, false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Outer Wall", 0, 1), ServerId::Archives, false);
    let resource = {
        let mut c = tk::vanilla_runner_card("Some Resource", CardType::Resource);
        c.cost = Some(0);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            // Decline the first rez's offer, take the second's.
            .when(Match::reaction().offering("tri-maf").nth(2), Reply::take("tri-maf"))
            .when(Match::targets().once(), Reply::Targets(vec![resource]))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("tri-maf"),
        2,
        "no printed ordinal and no flag: every rez is an occurrence: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&resource].zone,
        Zone::Rig,
        "the second offer was good — the resource was installed off it: {}",
        t.tail(40)
    );
}

/// Silhouette (CR 9.6.5c): declining the first-HQ-run offer spends the
/// ordinal; the second HQ run of the turn is not offered.
#[test]
fn silhouette_declined_expose_is_not_offered_again_this_turn() {
    let mut vm = Vm::empty(8005);
    tk::install_identity(&mut vm, card("Silhouette: Stealth Operative"), Side::Runner);
    tk::install_root(&mut vm, tk::vanilla_asset("Hidden", 0, 2), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("silhouette"),
        1,
        "9.6.5c: the declined first HQ run was still the first time: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardExposed { .. })),
        "and nothing was ever exposed: {}",
        t.tail(40)
    );
}

/// Khan (CR 9.6.5c): the declined first pass spends the ordinal; passing ice
/// again the same turn offers nothing and the icebreaker stays in the grip.
#[test]
fn khan_declined_install_is_not_offered_on_the_second_pass() {
    let mut vm = Vm::empty(8006);
    tk::install_identity(&mut vm, card("Khan: Savvy Skiptracer"), Side::Runner);
    tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 1), ServerId::Archives, true);
    let breaker = {
        let mut c = tk::vanilla_runner_card("Some Breaker", CardType::Program);
        c.cost = Some(1);
        c.memory_cost = Some(1);
        c.subtypes = vec![Subtype::Icebreaker];
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("skiptracer"),
        1,
        "9.6.5c: offered on the first pass only, declined or not: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&breaker].zone,
        Zone::Hand(Side::Runner),
        "nothing was installed: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.runner.credits, 5, "and nothing was paid: {}", t.tail(40));
}

/// Laramy Fisk (CR 9.6.5c): a decline on the first central run leaves no
/// second offer for a run on a DIFFERENT central the same turn.
#[test]
fn laramy_fisk_declined_offer_is_gone_for_the_other_central_too() {
    let mut vm = Vm::empty(8007);
    tk::install_identity(&mut vm, card("Laramy Fisk: Savvy Investor"), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    assert!(vm.st.hand[&Side::Corp].is_empty(), "HQ starts empty");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("laramy fisk"),
        1,
        "9.6.5c: the ordinal counts occurrences across all central servers: {}",
        t.tail(40)
    );
    assert!(vm.st.hand[&Side::Corp].is_empty(), "the Corp never drew: {}", t.tail(40));
}

/// Nero Severn (CR 9.1.6b): a declined once-per-turn jack-out is not a use —
/// the second sentry encounter of the turn is offered again, and taking it
/// ends the run before the breach.
#[test]
fn nero_severn_declined_jack_out_returns_on_the_next_sentry() {
    let mut vm = Vm::empty(8008);
    tk::install_identity(&mut vm, card("Nero Severn: Information Broker"), Side::Runner);
    tk::install_ice(
        &mut vm,
        tk::subtyped_ice("Some Sentry", vec![Subtype::Sentry], 0, 1),
        ServerId::Hq,
        true,
    );
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            // Decline the first encounter's offer; take the second's.
            .when(
                Match::reaction().offering("information broker").nth(2),
                Reply::take("information broker"),
            )
            .when(Match::optional(), Reply::Optional(true))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("information broker"),
        2,
        "9.1.6b: the declined offer was never a use, so the flag is unspent: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::BreachBegan { .. }))
            .count(),
        1,
        "the first run breached; the second jacked out before its breach: {}",
        t.tail(40)
    );
}

/// Mercury (CR 9.1.6b): declining the additional access on the first breach
/// leaves the flag unspent — the other named server's breach the same turn is
/// offered, and taking it accesses one more card.
#[test]
fn mercury_declined_offer_returns_on_the_other_central() {
    let mut vm = Vm::empty(8009);
    tk::install_identity(&mut vm, card("Mercury: Chrome Libertador"), Side::Runner);
    let hq = tk::fill_hand(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(
                Match::reaction().offering("chrome libertador").nth(2),
                Reply::take("chrome libertador"),
            )
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("chrome libertador"),
        2,
        "9.1.6b: declined on the R&D breach, offered again on the HQ breach: {}",
        t.tail(40)
    );
    let hq_accessed = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardAccessed { obj } if hq.contains(obj)))
        .count();
    assert_eq!(
        hq_accessed, 2,
        "…and the taken offer really accessed one additional HQ card: {}",
        t.tail(40)
    );
}

/// MuslihaT (CR 9.1.6b crossed with 5.6.2): the declined reveal moves nothing
/// this turn, and the ability arms again when the next turn begins.
#[test]
fn muslihat_declined_reveal_leaves_the_card_and_returns_next_turn() {
    let mut vm = Vm::empty(8010);
    tk::install_identity(&mut vm, card("MuslihaT: Multifarious Marketeer"), Side::Runner);
    let mut top = tk::program_cost("Top Card", 0);
    top.subtypes = vec![Subtype::Icebreaker];
    let top = vm.new_object(top, Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(top);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    vm.start_turn(Side::Runner);

    // Turn 1: decline, and drain the clicks on basic credits.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::optional().once(), Reply::Optional(false))
            .otherwise_click_credit(),
    );
    assert_eq!(
        vm.st.objects[&top].zone,
        Zone::Deck(Side::Runner),
        "declined: the card stays on top of the stack: {}",
        t.tail(30)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { obj, .. } if *obj == top)),
        "and it was never revealed: {}",
        t.tail(30)
    );

    // The Corp's turn passes; the Runner's next turn begins and looks again.
    let t2 = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner().when(Match::optional().once(), Reply::Optional(true)).stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&top].zone,
        Zone::Hand(Side::Runner),
        "the next turn's look found the same card and this time it was taken: {}",
        t2.tail(30)
    );
}

/// Pravdivost (CR 9.6.5c): the Corp declines on the first successful run;
/// the second successful run of the turn is not offered.
#[test]
fn pravdivost_declined_advancement_is_spent_for_the_turn() {
    let mut vm = Vm::empty(8011);
    tk::install_identity(&mut vm, card("Pravdivost Consulting: Political Solutions"), Side::Corp);
    let agenda =
        tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 5, 3), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("pravdivost"),
        1,
        "9.6.5c: offered once, and the decline spent the turn's first time: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&agenda].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        0,
        "no counter was ever placed: {}",
        t.tail(30)
    );
}

/// Editorial Division (CR 9.6.5c): the Corp declines the first bad-publicity
/// search; taking bad publicity again the same turn offers nothing.
#[test]
fn editorial_division_declined_search_is_spent_for_the_turn() {
    let mut vm = Vm::empty(8012);
    tk::install_identity(&mut vm, card("Editorial Division: Ad Nihilum"), Side::Corp);
    tk::install_root(&mut vm, tk::take_bad_pub_button("Bad Press", 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::paid().offering_pick(Pick::Labeled("take bad publicity")).times(2),
                Reply::take("take bad publicity"),
            )
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.bad_publicity, 2, "bad publicity arrived twice: {}", t.tail(40));
    assert_eq!(
        t.offers("ad nihilum"),
        1,
        "9.6.5c: the declined first taking was still the first time each turn: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.hand[&Side::Corp].len(),
        1,
        "HQ holds 5.5.2's mandatory draw and nothing from any search: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { .. })),
        "no search find was ever revealed: {}",
        t.tail(40)
    );
}

/// Azmari EdTech (CR 9.6.9): declining to name a type leaves no "last named"
/// type, so the Runner's event the next turn pays the Corp nothing.
#[test]
fn azmari_edtech_declining_to_name_disarms_the_payout() {
    let mut vm = Vm::empty(8013);
    tk::install_identity(&mut vm, card("Azmari EdTech: Shaping the Future"), Side::Corp);
    let gamble = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(gamble);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        // No naming rule: the turn-end offer is declined by the fallback.
        Plan::corp().otherwise_click_credit(),
        Plan::runner().when(Match::action().once(), Reply::play_card(gamble)).stop_at_action(),
    );
    assert_eq!(t.offers("zmari"), 1, "the naming was really offered, once: {}", t.tail(40));
    assert_eq!(vm.st.objects[&gamble].zone, Zone::Discard(Side::Runner), "{}", t.tail(40));
    assert_eq!(
        vm.st.corp.credits, 3,
        "three basic credits and nothing else — no type was named, so no payout: {}",
        t.tail(40)
    );
}

/// Gagarin (CR 1.16.10): the access cost is declinable even while affordable —
/// declining forgoes the access and keeps the credit.
#[test]
fn gagarin_declining_the_access_cost_while_able_skips_the_access() {
    let mut vm = Vm::empty(8014);
    tk::install_identity(&mut vm, card("Gagarin Deep Space: Expanding the Horizon"), Side::Corp);
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 99), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::of(Kind::NestedCost), Reply::PayCost(false))
            .stop_at_action(),
    );
    assert_eq!(t.of_kind(Kind::NestedCost).len(), 1, "the cost was really put: {}", t.tail(40));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == asset)),
        "declined, so the card was not accessed: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.runner.credits, 3, "and the credit was kept: {}", t.tail(40));
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run itself was still successful: {}",
        t.tail(40)
    );
}

// ---------------------------------------------------------------------------
// D. Cannot-pay and boundary cases
// ---------------------------------------------------------------------------

/// Thule Subsea (CR 1.16.1b): a Runner with 1[credit] cannot pay the 2 — no
/// choice is put, and the core damage is automatic.
#[test]
fn thule_subsea_poor_runner_is_never_asked_and_takes_the_damage() {
    let mut vm = Vm::empty(8040);
    tk::install_identity(&mut vm, card("Thule Subsea: Safety Below"), Side::Corp);
    tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert!(
        t.of_kind(Kind::NestedCost).is_empty(),
        "1.16.1b: an unpayable cost is never offered: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "so the core damage happened, unasked: {}",
        t.tail(30)
    );
    assert_eq!(vm.max_hand_size(Side::Runner), 4, "and it was CORE damage: {}", t.tail(30));
    assert_eq!(vm.st.runner.credits, 1, "the credit was never taken: {}", t.tail(30));
}

/// Thule Subsea (CR 1.16.1b, the click half): stealing on the turn's last
/// click leaves no [click] to spend — rich in credits or not, the damage is
/// automatic.
#[test]
fn thule_subsea_no_click_left_means_automatic_damage() {
    let mut vm = Vm::empty(8041);
    tk::install_identity(&mut vm, card("Thule Subsea: Safety Below"), Side::Corp);
    tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            // Three basic credits first: the run is the fourth, last click.
            .when(Match::action().times(3), Reply::credit())
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert!(
        t.of_kind(Kind::NestedCost).is_empty(),
        "no [click] remains, so the cost is unpayable and never put: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "the damage is automatic: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.runner.credits, 8, "5 + 3, untouched by the cost: {}", t.tail(30));
}

/// PT Untaian (CR 1.16.1b): with 0[credit] at the end of the discard phase
/// the may-pay is unpayable — no offer, no counter.
#[test]
fn pt_untaian_cannot_afford_the_credit_and_is_not_asked() {
    let mut vm = Vm::empty(8042);
    tk::install_identity(&mut vm, card("PT Untaian: Life's Building Blocks"), Side::Corp);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Some Agenda", 5, 3),
        ServerId::Remote(1),
        false,
    );
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        // 5.2.6h's purge takes all three clicks and pays nothing — the only
        // way through the turn that leaves the pool at zero.
        Plan::corp().when(Match::action(), Reply::Take(Pick::Purge)),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn finished: {}", t.tail(40));
    assert_eq!(vm.st.corp.credits, 0, "still broke at the discard phase: {}", t.tail(40));
    assert!(
        t.of_kind(Kind::NestedCost).is_empty(),
        "1.16.1b: the 1[credit] was unpayable, so the choice was never put: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&agenda].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        0,
        "and no counter arrived: {}",
        t.tail(40)
    );
}

/// Vic at 0 tags (CR 1.15.3): the cost is payable, so the ability is offered
/// — and the effect does as much as possible: the draw happens, the removal
/// removes nothing.
#[test]
fn virtual_intelligence_at_zero_tags_still_draws() {
    let mut vm = Vm::empty(8043);
    tk::install_identity(
        &mut vm,
        card("Virtual Intelligence, P.I.: \"You Can Call Me Vic\""),
        Side::Runner,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 4;
    vm.st.runner.tags = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::take("vic")).stop_at_action(),
    );
    assert_eq!(
        t.times_taken("vic"),
        1,
        "1.16.1b withholds only for the COST, and [click] + 1[credit] is payable: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 1, "the draw half happened: {}", t.tail(30));
    assert_eq!(vm.st.runner.tags, 0, "1.15.3: no tag to remove, none removed: {}", t.tail(30));
    assert_eq!(vm.st.runner.credits, 3, "and the cost was really paid: {}", t.tail(30));
    assert_eq!(vm.st.runner.clicks, 3, "both halves of it: {}", t.tail(30));
}

/// MaxX with one card in the stack (CR 1.15.3): the trash takes what exists,
/// and the draw from an empty stack draws nothing — the Runner does not lose.
#[test]
fn maxx_with_one_stack_card_mills_it_and_draws_nothing() {
    let mut vm = Vm::empty(8044);
    tk::install_identity(&mut vm, card("MaxX: Maximum Punk Rock"), Side::Runner);
    let stack = tk::fill_deck(&mut vm, Side::Runner, 1);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    assert_eq!(
        vm.st.objects[&stack[0]].zone,
        Zone::Discard(Side::Runner),
        "the one card was trashed: {}",
        t.tail(24)
    );
    assert!(vm.st.deck[&Side::Runner].is_empty(), "the stack is empty: {}", t.tail(24));
    assert!(
        vm.st.hand[&Side::Runner].is_empty(),
        "1.15.3: the draw found nothing and drew nothing: {}",
        t.tail(24)
    );
    assert!(vm.game_over.is_none(), "and the Runner does not lose for it: {}", t.tail(24));
}

/// Alice Merchant against an empty HQ (CR 1.15.3): there is no card for the
/// Corp to trash, so nothing happens and the run still succeeds.
#[test]
fn alice_merchant_with_an_empty_hq_trashes_nothing() {
    let mut vm = Vm::empty(8045);
    tk::install_identity(&mut vm, card("Alice Merchant: Clan Agitator"), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    assert!(vm.st.hand[&Side::Corp].is_empty(), "HQ starts empty");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run succeeded: {}",
        t.tail(30)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardTrashed { .. })),
        "1.15.3: no card existed to trash, so none was: {}",
        t.tail(30)
    );
    assert!(
        t.of_kind(Kind::Targets).is_empty(),
        "and the Corp was never asked to pick from nothing: {}",
        t.tail(30)
    );
}

/// SSO Industries with no faceup installed agendas (CR 1.15.3): X is 0, so
/// choosing an ice places zero tokens.
#[test]
fn sso_industries_with_no_faceup_agendas_places_zero_tokens() {
    let mut vm = Vm::empty(8046);
    tk::install_identity(&mut vm, card("SSO Industries: Fueling Innovation"), Side::Corp);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 1), ServerId::Hq, false);
    // An agenda installed FACEDOWN: outside the description, so it counts
    // for nothing.
    tk::install_root(&mut vm, tk::vanilla_agenda("Hidden Agenda", 3, 2), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("fueling innovation"), Reply::take("fueling innovation"))
            .when(Match::targets().once(), Reply::target(ice))
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn ended: {}", t.tail(30));
    assert_eq!(
        vm.st.objects[&ice].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        0,
        "1 token per faceup installed agenda point is 0 tokens: {}",
        t.tail(30)
    );
}

/// Fringe Applications with no ice installed (CR 1.15.3): the mandatory
/// placement has no candidate and does nothing — the turn goes on.
#[test]
fn fringe_applications_with_no_ice_does_nothing() {
    let mut vm = Vm::empty(8047);
    tk::install_identity(&mut vm, card("Fringe Applications: Tomorrow, Today"), Side::Corp);
    let mut weyland = |name: &'static str, server| {
        let mut c = tk::vanilla_asset(name, 0, 2);
        c.faction = Some("Weyland Consortium");
        tk::install_root(&mut vm, c, server, true)
    };
    weyland("Weyland One", ServerId::Remote(1));
    weyland("Weyland Two", ServerId::Remote(2));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    assert!(
        !vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CounterPlaced { kind: CounterKind::Advancement, .. }
        )),
        "no ice, no token — 1.15.3, not an error: {}",
        t.tail(30)
    );
    assert!(
        !t.of_kind(Kind::Action).is_empty(),
        "and the Runner's turn reached its action window: {}",
        t.tail(30)
    );
}

/// Leela Patel with zero unrezzed candidates (CR 1.15.3): a score with
/// nothing unrezzed on the board bounces nothing.
#[test]
fn leela_patel_with_no_unrezzed_cards_bounces_nothing() {
    let mut vm = Vm::empty(8048);
    tk::install_identity(&mut vm, card("Leela Patel: Trained Pragmatist"), Side::Runner);
    let agenda =
        tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
    let asset =
        tk::install_root(&mut vm, tk::vanilla_asset("Rezzed Asset", 0, 2), ServerId::Remote(2), true);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Corp), "{}", t.tail(30));
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Root(ServerId::Remote(2)),
        "the rezzed asset was never a candidate and never moved: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.hand[&Side::Corp].len(),
        3,
        "HQ holds its 2 cards and 5.5.2's mandatory draw — nothing was added to it: {}",
        t.tail(30)
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert!(
        announcements.iter().all(|a| a.candidates().is_empty()),
        "1.15.3: the announcement finds zero candidates and the ability moves nothing: {}",
        t.tail(30)
    );
}

/// Tennin Institute on the game's first Corp turn (CR 9.6.5c): "the Runner
/// did not make a successful run during their last turn" is vacuously true —
/// there was no last turn — so the ability fires.
#[test]
fn tennin_institute_fires_on_the_games_first_turn() {
    let mut vm = Vm::empty(8049);
    tk::install_identity(&mut vm, card("Tennin Institute: The Secrets Within"), Side::Corp);
    let target =
        tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("tennin institute"), Reply::take("tennin institute"))
            .when(Match::targets().once(), Reply::Targets(vec![target]))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&target].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        1,
        "a turn that never happened held no successful run — the condition is met: {}",
        t.tail(30)
    );
}

// ---------------------------------------------------------------------------
// E. The three ordinal record-facts of commit 7361978, pinned by POSITION
// ---------------------------------------------------------------------------

/// Ryō Ōno (CR 9.6.5c on RunDeclaredSuccessful.subroutine_resolved): a plain
/// success does not spend the turn's one time — the gain's log position
/// follows the SECOND, qualifying success.
#[test]
fn ryo_ono_gain_follows_the_second_qualifying_success() {
    let mut vm = Vm::empty(8050);
    tk::install_identity(
        &mut vm,
        card("Ryō \"Phoenix\" Ōno: Out of the Ashes"),
        Side::Runner,
    );
    tk::install_ice(&mut vm, tk::three_sub_ice("Some Ice"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);
    let hq_before = vm.st.hand[&Side::Corp].len();

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            // Run 1: R&D, no ice, no subroutine — a plain success.
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            // Run 2: HQ, whose ice resolves subroutines.
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    let successes: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(successes.len(), 2, "both runs were declared successful: {}", t.tail(50));
    assert_eq!(vm.st.runner.credits, 1, "one payout: {}", t.tail(50));
    assert_eq!(
        hq_before - vm.st.hand[&Side::Corp].len(),
        1,
        "and one Corp pitch: {}",
        t.tail(50)
    );
    let gained_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CreditsGained { side: Side::Runner, .. }))
        .expect("the identity paid out");
    assert!(
        gained_at > successes[1],
        "W23y: the gain follows the SECOND declaration — the subroutine-less first \
         was never one of the times: {}",
        t.tail(50)
    );
}

/// Builder of Nations (CR 9.6.5c on EncounterEnded.ice_advanced): counters
/// placed after an encounter ended do not rewrite it — the damage's log
/// position follows the second run's advanced encounter.
#[test]
fn builder_of_nations_damage_follows_the_second_runs_encounter() {
    let mut vm = Vm::empty(8051);
    tk::install_identity(&mut vm, card("Weyland Consortium: Builder of Nations"), Side::Corp);
    let plain =
        tk::install_ice(&mut vm, tk::vanilla_ice("Plain Ice", 0, 1), ServerId::Archives, true);
    let advanced_ice =
        tk::install_ice(&mut vm, tk::vanilla_ice("Advanced Ice", 0, 1), ServerId::Rnd, true);
    tk::place_counters(&mut vm, advanced_ice, CounterKind::Advancement, 1);
    // The moment run 1 ends, its plain ice is advanced by a card ability —
    // the board then claims that encounter was "with an advanced ice".
    tk::install_root(
        &mut vm,
        tk::run_ends_advancer("Groundskeeper", ServerId::Archives, plain),
        ServerId::Remote(1),
        true,
    );
    let grip = tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&plain].counter(CounterKind::Advancement),
        1,
        "the groundskeeper advanced the first run's ice after its encounter ended: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        grip.len() - 1,
        "one meat damage, once: {}",
        t.tail(50)
    );
    let runs: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::RunBegan { .. }))
        .map(|(i, _)| i)
        .collect();
    let damage_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("the damage happened");
    assert!(
        damage_at > runs[1],
        "W23y: the damage follows the R&D run — the rewritten Archives encounter \
         was never one of the times: {}",
        t.tail(50)
    );
}

/// Architects of Tomorrow (CR 9.6.5c): the declined first pass of a rezzed
/// bioroid still spends the ordinal — the second rezzed bioroid passed the
/// same turn offers nothing.
#[test]
fn architects_of_tomorrow_declined_pass_spends_the_ordinal() {
    let mut vm = Vm::empty(8052);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Architects of Tomorrow"), Side::Corp);
    tk::install_ice(
        &mut vm,
        tk::subtyped_ice("First Bioroid", vec![Subtype::Bioroid], 0, 1),
        ServerId::Hq,
        true,
    );
    tk::install_ice(
        &mut vm,
        tk::subtyped_ice("Second Bioroid", vec![Subtype::Bioroid], 0, 1),
        ServerId::Rnd,
        true,
    );
    let asset = {
        let mut c = tk::vanilla_asset("Bioroid Asset", 5, 2);
        c.subtypes = vec![Subtype::Bioroid];
        tk::install_root(&mut vm, c, ServerId::Remote(1), false)
    };
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 4;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        // No reaction rule: the fallback declines the offer.
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("architects"),
        1,
        "9.6.5c: the declined first pass was still the first time each turn: {}",
        t.tail(50)
    );
    assert!(!vm.st.objects[&asset].faceup, "and nothing was ever rezzed: {}", t.tail(50));
}

/// Architects of Tomorrow (the IcePassed.rezzed record-fact of 7361978): a
/// derez AFTER the first pass does not erase that pass from the record — the
/// second rezzed bioroid passed the same turn still offers nothing.
#[test]
fn architects_of_tomorrow_derez_does_not_rewrite_the_spent_pass() {
    let mut vm = Vm::empty(8053);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Architects of Tomorrow"), Side::Corp);
    let first = tk::install_ice(
        &mut vm,
        tk::subtyped_ice("First Bioroid", vec![Subtype::Bioroid], 0, 1),
        ServerId::Hq,
        true,
    );
    tk::install_ice(
        &mut vm,
        tk::subtyped_ice("Second Bioroid", vec![Subtype::Bioroid], 0, 1),
        ServerId::Rnd,
        true,
    );
    let asset = {
        let mut c = tk::vanilla_asset("Bioroid Asset", 5, 2);
        c.subtypes = vec![Subtype::Bioroid];
        tk::install_root(&mut vm, c, ServerId::Remote(1), false)
    };
    // A Divert-Power-shaped button that derezzes the first bioroid.
    tk::install_root(&mut vm, tk::derez_button("Divert", first), ServerId::Remote(2), true);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 4;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        // The second approach window is run 2's, after the first pass and
        // before the second: derez the already-passed bioroid there. The
        // pass offers are declined by the fallback throughout.
        Plan::corp().when(
            Match::paid().approaching_ice().offering("divert").nth(2),
            Reply::take("divert"),
        ),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert!(
        !vm.st.objects[&first].faceup,
        "the first bioroid was derezzed between the two passes: {}",
        t.tail(50)
    );
    assert_eq!(
        t.offers("architects"),
        1,
        "W23y: IcePassed.rezzed rides the record — derezzing the board does not \
         un-spend the ordinal, so the second pass offers nothing: {}",
        t.tail(50)
    );
    assert!(!vm.st.objects[&asset].faceup, "and nothing was rezzed off it: {}", t.tail(50));
}

// ---------------------------------------------------------------------------
// F. Null: Whistleblower — the audit's open finding 2
// ---------------------------------------------------------------------------

/// Null (CR 9.3.6g via 9.1.6b): paying the optional cost IS using the
/// ability, so the second encounter of the same turn must offer nothing.
#[test]
fn null_whistleblower_paying_spends_the_flag_for_the_turn() {
    let mut vm = Vm::empty(8015);
    tk::install_identity(&mut vm, card("Null: Whistleblower"), Side::Runner);
    tk::install_ice(&mut vm, tk::vanilla_ice("First Ice", 0, 4), ServerId::Hq, true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Second Ice", 0, 4), ServerId::Rnd, true);
    let hand = tk::fill_hand(&mut vm, Side::Runner, 2);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .when(Match::payment_cards().once(), Reply::Targets(vec![hand[0]]))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&hand[0]].zone,
        Zone::Discard(Side::Runner),
        "the first encounter's cost was paid: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::NestedCost).len(),
        1,
        "9.1.6b: paying was the use, so the second encounter offers nothing: {}",
        t.tail(40)
    );
}

/// Null's decline branch (CR 9.1.6b): a declined optional cost is not a use,
/// so the second encounter of the same turn is offered — and pays.
#[test]
fn null_whistleblower_declining_leaves_the_flag_for_the_next_encounter() {
    let mut vm = Vm::empty(8016);
    tk::install_identity(&mut vm, card("Null: Whistleblower"), Side::Runner);
    tk::install_ice(&mut vm, tk::vanilla_ice("First Ice", 0, 4), ServerId::Hq, true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Second Ice", 0, 4), ServerId::Rnd, true);
    let hand = tk::fill_hand(&mut vm, Side::Runner, 2);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            // Decline on the first encounter, pay on the second.
            .when(Match::nested_cost().nth(2), Reply::PayCost(true))
            .when(Match::payment_cards().once(), Reply::Targets(vec![hand[0]]))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    assert_eq!(
        t.of_kind(Kind::NestedCost).len(),
        2,
        "9.1.6b: the declined cost was never a use, so the second encounter asks again: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&hand[0]].zone,
        Zone::Discard(Side::Runner),
        "and the second offer was good — the grip card paid it: {}",
        t.tail(40)
    );
}

// ---------------------------------------------------------------------------
// B. Second trigger the same turn: the ordinal is spent — and only by a
//    matching occurrence
// ---------------------------------------------------------------------------

/// Ken Tenma (CR 9.6.5c): a plain event is not a run event, so it does not
/// spend the ordinal — the run event played after it still pays.
#[test]
fn ken_tenma_plain_event_does_not_spend_the_ordinal() {
    let mut vm = Vm::empty(8020);
    tk::install_identity(&mut vm, card("Ken \"Express\" Tenma: Disappeared Clone"), Side::Runner);
    let plain = vm.new_object(tk::event("Plain Event", 0, vec![]), Zone::Hand(Side::Runner));
    let getaway = vm.new_object(card("Clean Getaway"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([plain, getaway]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(plain))
            .when(Match::action().once(), Reply::play_card(getaway))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            .stop_at_action(),
    );
    // 3 − 0 (plain) − 3 (Clean Getaway) + 6 (its gain) + 1 (Ken).
    assert_eq!(
        vm.st.runner.credits, 7,
        "the plain event left the ordinal unspent, so the run event still paid: {}",
        t.tail(40)
    );
}

/// Gabriel Santiago (CR 9.6.5c): the change-log position pins WHICH run paid —
/// the gain follows the first HQ success, and no gain follows the second.
#[test]
fn gabriel_santiago_gain_follows_the_first_success_not_the_second() {
    let mut vm = Vm::empty(8021);
    tk::install_identity(
        &mut vm,
        card("Gabriel Santiago: Consummate Professional"),
        Side::Runner,
    );
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 2, "one payout for two HQ runs: {}", t.tail(40));
    let successes: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(successes.len(), 2, "both runs succeeded: {}", t.tail(40));
    let gains: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::CreditsGained { side: Side::Runner, .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(gains.len(), 1, "and exactly one gain was logged: {}", t.tail(40));
    assert!(
        gains[0] > successes[0] && gains[0] < successes[1],
        "the gain sits after the FIRST success and before the second: {}",
        t.tail(40)
    );
}

/// Los (CR 9.6.5c): rezzing an ASSET is not rezzing ice, so it leaves the
/// ordinal unspent — the first ice rez still pays, the second does not.
#[test]
fn los_asset_rez_does_not_spend_the_ordinal() {
    let mut vm = Vm::empty(8022);
    tk::install_identity(&mut vm, card("Los: Data Hijacker"), Side::Runner);
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 1, 2), ServerId::Remote(1), false);
    let inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner Wall", 1, 1), ServerId::Archives, false);
    let outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer Wall", 1, 1), ServerId::Archives, false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::Rez(asset)))
            .when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert!(
        vm.st.objects[&asset].faceup && vm.st.objects[&inner].faceup && vm.st.objects[&outer].faceup,
        "all three rezzes happened: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.runner.credits, 2,
        "the asset spent nothing; the FIRST ice rez paid and the second did not: {}",
        t.tail(40)
    );
}

/// Liza (CR 9.6.5c): a successful run on a REMOTE is not one of the times, so
/// the central run after it still draws and tags — and the second central
/// does not.
#[test]
fn liza_remote_success_does_not_spend_the_ordinal() {
    let mut vm = Vm::empty(8023);
    tk::install_identity(
        &mut vm,
        card("Liza Talking Thunder: Prominent Legislator"),
        Side::Runner,
    );
    tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 5), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip starts empty");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
            .count(),
        3,
        "all three runs succeeded: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "two cards from the R&D run — the remote spent nothing, Archives got nothing: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.runner.tags, 1, "and exactly one tag: {}", t.tail(40));
}

/// Az McCaffrey (CR 9.6.5c as a cost declaration): the second qualifying
/// install of the turn pays full price.
#[test]
fn az_mccaffrey_second_qualifying_install_pays_in_full() {
    let mut vm = Vm::empty(8024);
    tk::install_identity(&mut vm, card("Az McCaffrey: Mechanical Prodigy"), Side::Runner);
    let mut mk = |name: &'static str| {
        let mut c = tk::vanilla_runner_card(name, CardType::Hardware);
        c.cost = Some(3);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let first = mk("First Hardware");
    let second = mk("Second Hardware");
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(first)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(second)))
            .stop_at_action(),
    );
    for c in [first, second] {
        assert_eq!(vm.st.objects[&c].zone, Zone::Rig, "both installed: {}", t.tail(40));
    }
    assert_eq!(
        vm.st.runner.credits, 0,
        "2 + 3: the discount reached the first install only: {}",
        t.tail(40)
    );
}

/// Kate "Mac" McCaffrey (CR 9.6.5c as a cost declaration): the second program
/// of the turn pays full price.
#[test]
fn kate_mac_mccaffrey_second_program_pays_in_full() {
    let mut vm = Vm::empty(8025);
    tk::install_identity(&mut vm, card("Kate \"Mac\" McCaffrey: Digital Tinker"), Side::Runner);
    let mut mk = |name: &'static str| {
        let mut c = tk::vanilla_runner_card(name, CardType::Program);
        c.cost = Some(3);
        c.memory_cost = Some(1);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let first = mk("First Program");
    let second = mk("Second Program");
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(first)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(second)))
            .stop_at_action(),
    );
    for c in [first, second] {
        assert_eq!(vm.st.objects[&c].zone, Zone::Rig, "both installed: {}", t.tail(40));
    }
    assert_eq!(
        vm.st.runner.credits, 0,
        "2 + 3: the reduction is the first program's only: {}",
        t.tail(40)
    );
}

/// Seidr Laboratories (CR 9.6.5c): two click losses in one run offer once —
/// the second loss of the same turn is past the ordinal.
#[test]
fn seidr_laboratories_second_click_loss_of_the_turn_offers_nothing() {
    let mut vm = Vm::empty(8026);
    tk::install_identity(&mut vm, card("Seidr Laboratories: Destiny Defined"), Side::Corp);
    tk::install_ice(&mut vm, tk::etr_ice("Inner Wall", 0, 1), ServerId::Archives, true);
    tk::install_ice(&mut vm, tk::etr_ice("Outer Wall", 0, 1), ServerId::Archives, true);
    tk::install_rig(&mut vm, tk::lose_click_break_program("Eli-like"));
    let buried = vm.new_object(tk::corp_filler("Buried"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("seidr"), Reply::take("seidr"))
            .when(Match::targets().once(), Reply::Targets(vec![buried])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::paid().times(2), Reply::take("lose-click"))
            .stop_at_action(),
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::ClicksLost { side: Side::Runner, .. } ))
            .count(),
        2,
        "both encounters took a click: {}",
        t.tail(50)
    );
    assert_eq!(
        t.offers("seidr"),
        1,
        "9.6.5c: offered on the first loss only: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.deck[&Side::Corp].first().copied(),
        Some(buried),
        "and the one offer was good — the Archives card tops R&D: {}",
        t.tail(50)
    );
}

/// LEO Construction (CR 9.3.6g): the flag refuses a second use the same turn —
/// the second run's paid windows never offer the ability again.
#[test]
fn leo_construction_flag_refuses_a_second_use_the_same_turn() {
    let mut vm = Vm::empty(8027);
    tk::install_identity(&mut vm, card("LEO Construction: Labor Solutions"), Side::Corp);
    let bioroid = |vm: &mut Vm, name: &'static str, server| {
        let mut c = tk::vanilla_asset(name, 0, 2);
        c.subtypes = vec![Subtype::Bioroid];
        tk::install_root(vm, c, server, true)
    };
    let first = bioroid(&mut vm, "First Bioroid", ServerId::Remote(1));
    let second = bioroid(&mut vm, "Second Bioroid", ServerId::Remote(2));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        // No `.once()`: the Corp takes the ability EVERY time it is offered,
        // so a second end-the-run would mean the flag was never spent.
        Plan::corp().when(Match::paid().offering("labor solutions"), Reply::take("labor solutions")),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("labor solutions"),
        1,
        "9.3.6g: the flag was spent by the first use and the second run offers nothing: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&first].zone,
        Zone::Discard(Side::Corp),
        "the first run's bioroid paid the cost: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&second].zone,
        Zone::Root(ServerId::Remote(2)),
        "the second run's bioroid was never touched: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
            .count(),
        1,
        "so the second run succeeded where the first was ended: {}",
        t.tail(50)
    );
}

/// Mti Mwekundu (CR 9.3.6g): the flag refuses a second use the same turn —
/// the second approach of the turn offers nothing and the second ice stays
/// in HQ.
#[test]
fn mti_mwekundu_flag_refuses_a_second_use_the_same_turn() {
    let mut vm = Vm::empty(8028);
    tk::install_identity(&mut vm, card("Mti Mwekundu: Life Improved"), Side::Corp);
    let ice1 = vm.new_object(tk::vanilla_ice("First Ice", 3, 1), Zone::Hand(Side::Corp));
    let ice2 = vm.new_object(tk::vanilla_ice("Second Ice", 3, 1), Zone::Hand(Side::Corp));
    for id in [ice1, ice2] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("life improved"), Reply::take("life improved"))
            .when(Match::targets().once(), Reply::Targets(vec![ice1])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("life improved"),
        1,
        "9.3.6g: one use a turn — the R&D approach offers nothing: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.position_of_ice(ice1).map(|(s, _)| s),
        Some(ServerId::Hq),
        "the first approach's ice went in: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&ice2].zone,
        Zone::Hand(Side::Corp),
        "and the second stayed in HQ: {}",
        t.tail(50)
    );
}

/// The Outfit prints NO ordinal (CR 9.6.4b): the second taking of bad
/// publicity the same turn pays again.
#[test]
fn the_outfit_pays_for_every_taking_with_no_ordinal_to_spend() {
    let mut vm = Vm::empty(8029);
    tk::install_identity(&mut vm, card("The Outfit: Family Owned and Operated"), Side::Corp);
    tk::install_root(&mut vm, tk::take_bad_pub_button("Scandal", 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().times(2), Reply::take("take bad publicity"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.bad_publicity, 2, "two takings, one point each: {}", t.tail(40));
    assert_eq!(
        vm.st.corp.credits, 6,
        "3 + 3: 'whenever' has no ordinal, so the second taking pays again: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CreditsGained { side: Side::Corp, amount: 3, .. }))
            .count(),
        2,
        "as two separate gains of 3: {}",
        t.tail(40)
    );
}

// ---------------------------------------------------------------------------
// C. Cross-turn reset: both flag classes arm again next turn
// ---------------------------------------------------------------------------

/// Gabriel Santiago (CR 9.6.5c): "each turn" — the ordinal resets, and the
/// first HQ run of the NEXT turn pays again.
#[test]
fn gabriel_santiago_ordinal_resets_next_turn() {
    let mut vm = Vm::empty(8030);
    tk::install_identity(
        &mut vm,
        card("Gabriel Santiago: Consummate Professional"),
        Side::Runner,
    );
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .otherwise_click_credit(),
    );
    assert_eq!(vm.st.runner.credits, 5, "turn 1: 2 gained + 3 basic credits: {}", t.tail(30));

    let t2 = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    assert_eq!(
        vm.st.runner.credits, 7,
        "turn 2's first HQ run pays 2 again — the ordinal reset with the turn: {}",
        t2.tail(30)
    );
}

/// Zahya (CR 9.3.6g): the once-per-turn flag resets — used on turn 1, the
/// ability is offered and pays again on turn 2.
#[test]
fn zahya_flag_resets_next_turn() {
    let mut vm = Vm::empty(8031);
    tk::install_identity(&mut vm, card("Zahya Sadeghi: Versatile Smuggler"), Side::Runner);
    tk::fill_hand(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::reaction().offering("versatile smuggler"), Reply::take("versatile smuggler"))
            .when(Match::optional(), Reply::Optional(true))
            .otherwise_click_credit(),
    );
    assert_eq!(t.offers("versatile smuggler"), 1, "used on turn 1: {}", t.tail(30));
    assert_eq!(vm.st.runner.credits, 4, "1 access + 3 basic credits: {}", t.tail(30));

    let t2 = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::reaction().offering("versatile smuggler"), Reply::take("versatile smuggler"))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(
        t2.offers("versatile smuggler"),
        1,
        "9.3.6g's flag died with the turn — turn 2's run end offers again: {}",
        t2.tail(30)
    );
    assert_eq!(vm.st.runner.credits, 5, "and pays again: {}", t2.tail(30));
}

/// Vic (CR 9.3.6g): the paid ability's flag resets — usable once on each of
/// two consecutive Runner turns.
#[test]
fn virtual_intelligence_flag_resets_next_turn() {
    let mut vm = Vm::empty(8032);
    tk::install_identity(
        &mut vm,
        card("Virtual Intelligence, P.I.: \"You Can Call Me Vic\""),
        Side::Runner,
    );
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.runner.credits = 4;
    vm.st.runner.tags = 4;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::take("vic"))
            .otherwise_click_credit(),
    );
    assert_eq!(vm.st.runner.tags, 3, "turn 1 removed a tag: {}", t.tail(30));
    assert_eq!(vm.st.hand[&Side::Runner].len(), 1, "and drew a card: {}", t.tail(30));

    let t2 = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::take("vic"))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.runner.tags, 2,
        "turn 2 offered the ability again and it worked again: {}",
        t2.tail(30)
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "second draw: {}", t2.tail(30));
}

/// LEO Construction (CR 9.3.6g, the Corp's flag across the Runner's turns):
/// spent during one Runner turn, armed again for the next.
#[test]
fn leo_construction_flag_resets_for_the_next_runner_turn() {
    let mut vm = Vm::empty(8033);
    tk::install_identity(&mut vm, card("LEO Construction: Labor Solutions"), Side::Corp);
    let bioroid = |vm: &mut Vm, name: &'static str, server| {
        let mut c = tk::vanilla_asset(name, 0, 2);
        c.subtypes = vec![Subtype::Bioroid];
        tk::install_root(vm, c, server, true)
    };
    let first = bioroid(&mut vm, "First Bioroid", ServerId::Remote(1));
    let second = bioroid(&mut vm, "Second Bioroid", ServerId::Remote(2));
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().offering("labor solutions"), Reply::take("labor solutions"))
            .when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .otherwise_click_credit(),
    );
    assert_eq!(vm.st.objects[&first].zone, Zone::Discard(Side::Corp), "{}", t.tail(30));

    let t2 = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().offering("labor solutions"), Reply::take("labor solutions"))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .stop_at_action(),
    );
    assert_eq!(
        t2.offers("labor solutions"),
        1,
        "the flag re-armed for the next Runner turn: {}",
        t2.tail(30)
    );
    assert_eq!(
        vm.st.objects[&second].zone,
        Zone::Discard(Side::Corp),
        "and the second use went through — trash, and the run ends: {}",
        t2.tail(30)
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunEnded { .. }))
            .count(),
        2,
        "both runs were ended by the ability: {}",
        t2.tail(30)
    );
}

/// Engineering the Future (CR 9.6.5c, the Corp's own ordinal): the first
/// install of each Corp turn pays — on both of two consecutive turns.
#[test]
fn engineering_the_future_ordinal_resets_next_corp_turn() {
    let mut vm = Vm::empty(8034);
    tk::install_identity(
        &mut vm,
        card("Haas-Bioroid: Engineering the Future"),
        Side::Corp,
    );
    let a1 = vm.new_object(tk::vanilla_asset("First Asset", 0, 2), Zone::Hand(Side::Corp));
    let a2 = vm.new_object(tk::vanilla_asset("Second Asset", 0, 2), Zone::Hand(Side::Corp));
    for id in [a1, a2] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(a1)))
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.corp.credits, 3, "turn 1: 1 gained + 2 basic credits: {}", t.tail(30));

    let t2 = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(a2)))
            .stop_at_action(),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(
        vm.st.corp.credits, 4,
        "turn 2's first install pays 1 again — 'each turn' reset the ordinal: {}",
        t2.tail(30)
    );
}

/// NBN: Controlling the Message (CR 9.6.5c): "each turn" spans every turn —
/// a trash on each of two Runner turns traces twice.
#[test]
fn controlling_the_message_re_arms_for_the_next_turn() {
    let mut vm = Vm::empty(8035);
    tk::install_identity(&mut vm, card("NBN: Controlling the Message"), Side::Corp);
    let one =
        tk::install_root(&mut vm, tk::vanilla_asset("First Asset", 0, 1), ServerId::Remote(1), true);
    let two =
        tk::install_root(&mut vm, tk::vanilla_asset("Second Asset", 0, 1), ServerId::Remote(2), true);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let corp_rules = || {
        Plan::corp()
            .when(
                Match::reaction().offering("controlling the message"),
                Reply::take("controlling the message"),
            )
            .when(Match::trace_spend(), Reply::Spend(0))
    };
    let t = plan::play(
        &mut vm,
        corp_rules().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::Take(Pick::BasicTrash))
            .when(Match::trace_spend(), Reply::Spend(0))
            .otherwise_click_credit(),
    );
    assert_eq!(vm.st.objects[&one].zone, Zone::Discard(Side::Corp), "{}", t.tail(40));
    assert_eq!(vm.st.runner.tags, 1, "turn 1's trace landed its tag: {}", t.tail(40));

    let t2 = plan::play(
        &mut vm,
        corp_rules().otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .when(Match::mid_access().once(), Reply::Take(Pick::BasicTrash))
            .when(Match::trace_spend(), Reply::Spend(0))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&two].zone, Zone::Discard(Side::Corp), "{}", t2.tail(40));
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::TraceInitiated { .. }))
            .count(),
        2,
        "one trace per turn — the ordinal reset between them: {}",
        t2.tail(40)
    );
    assert_eq!(vm.st.runner.tags, 2, "and both tags stuck: {}", t2.tail(40));
}

// ---------------------------------------------------------------------------
// F. Where a choice may be written (CR 9.11.4g), over the whole corpus
// ---------------------------------------------------------------------------

/// CR 9.11.4g: a choice ENDS the instruction it is written in, and "the chosen
/// effect is resolved as the next instruction". An instruction that resolves
/// INSIDE another one's imminence has no next instruction, so a choice
/// written there can never be put to anyone — it resolves to nothing, in
/// silence.
///
/// Predictive Planogram shipped exactly that way. Its "Resolve 1 of the
/// following" sat in an `if_met_else` branch, `IfMet` resolved its branch
/// inline, and the card produced neither the credits nor the cards in a real
/// game — no prompt, no error, nothing. One card was the symptom; the class
/// is "a choice written under any container that resolves what it holds
/// inline", and that is what this walks.
///
/// It is a STRUCTURAL walk rather than a fixture per card because the
/// property is structural: it covers every card a fixture could reach, and
/// also every card a fixture could not (a subroutine on ice nobody has
/// written a run for, a branch that needs three prior turns to reach). The
/// kernel carries the same rule as a `debug_assert!` in
/// `Vm::apply_imminent`'s `ChooseOne` arm, so a card slipping past this walk
/// would still fail loudly the first time a test resolved it — this is the
/// layer that does not need the resolution to happen at all.
#[test]
fn no_card_writes_a_choice_where_the_kernel_could_never_put_it() {
    use jinteki_cr::instr::{Contained, Instruction};

    fn short(i: &Instruction) -> String {
        let d = format!("{i:?}");
        match d.find([' ', '(', '{']) {
            Some(n) => d[..n].to_string(),
            None => d,
        }
    }

    /// Every place a choice instruction sits, paired with whether the kernel
    /// will give it an instruction of its own when it gets there.
    fn walk(instr: &Instruction, own_instruction: bool, path: &str, bad: &mut Vec<String>) {
        let here = format!("{path} > {}", short(instr));
        if instr.resolves_as_its_own_instruction() && !own_instruction {
            bad.push(here.clone());
        }
        match instr.contains() {
            Contained::Nothing => {}
            // These become instructions in their own right — spliced into the
            // frame, pushed as a new chain, or created as an ability later.
            Contained::Deferred(list) => {
                for i in list {
                    walk(i, true, &here, bad);
                }
            }
            // 9.6.5d: the live branch is spliced whole when any step of it
            // needs its own instruction (`Vm::branch_becomes_instructions`),
            // so a branch is a legal home for a choice. WHICH branch is live
            // is a game-state question; structurally, every branch is one.
            Contained::Branches(branches) => {
                for (_, effects) in branches {
                    for i in effects {
                        walk(i, true, &here, bad);
                    }
                }
            }
            // 9.11.4a: resolved as part of THIS instruction. `PerformedBy` is
            // peeled everywhere (1.14.5 names a player, not a position) and
            // `DeclineableChoice` splices what must be its own instruction,
            // so both hand their content the enclosing position; anything
            // else resolves what it holds inside an imminence.
            Contained::Inline(list) => {
                let passes_position = matches!(
                    instr,
                    Instruction::PerformedBy { .. } | Instruction::DeclineableChoice(_)
                );
                for i in list {
                    walk(i, own_instruction && passes_position, &here, bad);
                }
            }
        }
    }

    fn has_choice(i: &Instruction) -> bool {
        if matches!(i, Instruction::ChooseOne { .. }) {
            return true;
        }
        match i.contains() {
            Contained::Nothing => false,
            Contained::Inline(l) | Contained::Deferred(l) => l.iter().any(|x| has_choice(x)),
            Contained::Branches(b) => b.iter().any(|(_, e)| e.iter().any(has_choice)),
        }
    }

    let mut bad: Vec<String> = Vec::new();
    let mut choices = 0usize;
    for c in jinteki_cards::all_cards() {
        for (n, ab) in c.printed.abilities.iter().enumerate() {
            for (k, instr) in ab.instructions.iter().enumerate() {
                choices += usize::from(has_choice(instr));
                walk(instr, true, &format!("{} ability #{n} #{k}", c.printed.name), &mut bad);
            }
        }
    }

    assert!(
        choices >= 15,
        "the corpus still holds the choices this walk is about — {choices} found, so \
         the walk has not been quietly emptied by a refactor"
    );
    assert!(
        bad.is_empty(),
        "9.11.4g: these choices are written where the kernel would resolve them inside \
         another instruction's imminence, so they could never be put to a player \
         (the Predictive Planogram defect):\n  {}",
        bad.join("\n  ")
    );
}

