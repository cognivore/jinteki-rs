//! CR 9.12.2 selectors: three numbers the quantity language could not read.
//!
//! Every quantity position in the kernel is ONE selector language (§12 rule
//! 6), and a number a printed sentence names but the language cannot say is a
//! sentence that cannot be written at all. Three of them are added here, and
//! each test drives the selector where a card would use it — because 9.12.2's
//! whole point is that a calculated quantity is READ where it is used and
//! never stamped anywhere:
//!
//! - 4.6.6a's SERVERS matching a description ([`Quantity::CountServers`]),
//! - 10.6.1's BAD PUBLICITY on a player ([`Quantity::BadPublicityOf`]),
//! - 1.11.3's CLICKS a player has ([`Quantity::ClicksOf`]).
//!
//! The cards are built from the public vocabulary (§12 rule 3) and driven with
//! the shared plan driver (§12 rule 5): what each test asserts is the number
//! that reached a credit pool.

use jinteki_cr::instr::{Instruction, Quantity, ServerFilter, ServerLocation, TargetFilter};
use jinteki_cr::object::{CardType, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

// ---------------------------------------------------------------------------
// CR 4.6.6a — a count of servers
// ---------------------------------------------------------------------------

/// One board carrying every case a server description has to tell apart.
///
/// Four remotes and a central, chosen so that no count of CARDS can stand in
/// for the count of servers: the cards installed in the remotes' roots number
/// 4 and the ice protecting them numbers 4, while the servers the printed
/// sentence reaches number 2.
fn servers_board(vm: &mut Vm) {
    // Remote 1: a card in the root, one piece of ice — the printed sentence.
    tk::install_root(vm, tk::vanilla_asset("R1 Asset", 0, 2), ServerId::Remote(1), false);
    tk::install_ice(vm, tk::vanilla_ice("R1 Ice", 1, 1), ServerId::Remote(1), false);
    // Remote 2: 4.6.6e's root at its fullest — an asset AND an upgrade —
    // behind TWO pieces of ice. Still one server.
    tk::install_root(vm, tk::vanilla_asset("R2 Asset", 0, 2), ServerId::Remote(2), false);
    tk::install_root(vm, tk::vanilla_upgrade("R2 Upgrade", 0), ServerId::Remote(2), false);
    tk::install_ice(vm, tk::vanilla_ice("R2 Inner", 1, 1), ServerId::Remote(2), false);
    tk::install_ice(vm, tk::vanilla_ice("R2 Outer", 1, 1), ServerId::Remote(2), false);
    // Remote 3: ice and an empty root. 4.6.8d makes it a server all the same,
    // and the root stipulation is what excludes it.
    tk::install_ice(vm, tk::vanilla_ice("R3 Ice", 1, 1), ServerId::Remote(3), false);
    // Remote 4: a root card and no ice — excluded by the other stipulation.
    tk::install_root(vm, tk::vanilla_asset("R4 Asset", 0, 2), ServerId::Remote(4), false);
    // HQ: a central with both, so the TYPE stipulation has work to do.
    tk::install_root(vm, tk::vanilla_upgrade("HQ Upgrade", 0), ServerId::Hq, false);
    tk::install_ice(vm, tk::vanilla_ice("HQ Ice", 1, 1), ServerId::Hq, false);
}

/// Play a Corp operation whose only instruction gains 1[credit] for each
/// server the description reaches, and answer with the credits it gained.
fn credits_per_server(criteria: Vec<ServerFilter>, seed: u64) -> (u32, String) {
    let mut vm = Vm::empty(seed);
    servers_board(&mut vm);
    let op = vm.new_object(
        tk::operation(
            "Server Counter",
            0,
            vec![Instruction::GainCredits(Side::Corp, Quantity::CountServers(criteria))],
        ),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(op)).stop_at_action(),
        Plan::runner(),
    );
    (vm.st.corp.credits, t.tail(16))
}

fn is_remote() -> ServerFilter {
    ServerFilter::IsCentral(false)
}
fn a_card_in_the_root() -> ServerFilter {
    ServerFilter::HasCardIn { location: ServerLocation::Root, criteria: Vec::new() }
}
fn protected_by_ice() -> ServerFilter {
    ServerFilter::HasCardIn {
        location: ServerLocation::Protecting,
        criteria: vec![TargetFilter::CardTypeIs(CardType::Ice)],
    }
}

/// CR 4.6.6a — "for each remote server that has a card in its root and is
/// protected by ice", counted as SERVERS.
///
/// Both halves of the sentence are asserted on one board: the servers it
/// reaches are counted, and the servers it does not reach are not — Remote 3
/// has ice and no root card, Remote 4 has a root card and no ice, and HQ has
/// both and is the wrong type.
///
/// The second assertion is the one the blocker was written for. No count of
/// cards is this number: 4.6.6e lets a remote root hold an asset AND any
/// number of upgrades, so the cards in the qualifying roots number 3 and the
/// ice protecting them numbers 3, while the servers number 2. A card-count
/// stand-in pays out counters and credits that were never earned.
#[test]
fn a_server_count_reads_the_servers_and_never_the_cards_installed_in_them() {
    let (credits, tail) =
        credits_per_server(vec![is_remote(), a_card_in_the_root(), protected_by_ice()], 9701);
    assert_eq!(
        credits, 2,
        "4.6.6a: Remote 1 and Remote 2 match; Remote 3 has no card in its root, \
         Remote 4 is protected by nothing, and HQ is a central (4.6.6c): {tail}"
    );
    assert_ne!(
        credits, 3,
        "…and it is a count of SERVERS: the cards in those two roots number 3 \
         (4.6.6e's asset and upgrade in Remote 2) and the ice protecting them \
         numbers 3, and neither is the answer: {tail}"
    );
}

/// CR 4.6.6b/c — the same word, different content (§12 rule 2).
///
/// Every stipulation is content on one selector, so dropping one widens the
/// answer by exactly what that stipulation excluded, and the server's TYPE and
/// the LOCATION inside it are read the same way. Each case names the servers
/// it expects, so a wrong answer names which one moved.
#[test]
fn the_server_selector_says_type_location_and_description_with_one_word() {
    for (criteria, expected, why, seed) in [
        (
            vec![is_remote(), protected_by_ice()],
            3u32,
            "4.6.6d: without the root stipulation Remote 3 joins Remote 1 and Remote 2",
            9702u64,
        ),
        (
            vec![is_remote(), a_card_in_the_root()],
            3,
            "4.6.6e: without the ice stipulation Remote 4 joins them instead",
            9703,
        ),
        (
            vec![ServerFilter::IsCentral(true)],
            3,
            "4.6.7a: the Corp has three central servers at all times — R&D and \
             Archives count with nothing installed at either",
            9704,
        ),
        (
            vec![ServerFilter::HasCardIn {
                location: ServerLocation::RootOrProtecting,
                criteria: Vec::new(),
            }],
            5,
            "4.6.6b: the cards in a root and the cards protecting a server are \
             both IN it — HQ and all four remotes, and not R&D or Archives",
            9705,
        ),
        (
            vec![
                is_remote(),
                ServerFilter::HasCardIn {
                    location: ServerLocation::Root,
                    criteria: vec![TargetFilter::CardTypeIs(CardType::Upgrade)],
                },
            ],
            1,
            "the description of the CARD is content too: only Remote 2 has an \
             upgrade in its root, and HQ has one but is not a remote",
            9706,
        ),
    ] {
        let (credits, tail) = credits_per_server(criteria, seed);
        assert_eq!(credits, expected, "{why}: {tail}");
    }
}

// ---------------------------------------------------------------------------
// CR 10.6.1 — a player's bad publicity
// ---------------------------------------------------------------------------

/// CR 10.6.1 — the bad publicity counters on the named player.
///
/// Three readings on one starting board, each its own game: the Corp's two,
/// the Runner's none (the side is content, §12 rule 2), and the Corp's again
/// from an ability that takes one first — which is 9.12.2's re-read in person.
/// A stamped value would pay the Corp 2 for the third card; the number a
/// quantity names is the number at the moment the instruction reading it
/// resolves.
#[test]
fn bad_publicity_is_the_named_players_counters_read_where_the_sentence_asks() {
    for (instrs, expected, why, seed) in [
        (
            vec![Instruction::GainCredits(Side::Corp, Quantity::BadPublicityOf(Side::Corp))],
            2u32,
            "10.6.1: the Corp has 2 bad publicity and the sentence reads them",
            9711u64,
        ),
        (
            vec![Instruction::GainCredits(Side::Corp, Quantity::BadPublicityOf(Side::Runner))],
            0,
            "the SIDE is content: the same word asked about the Runner finds \
             the bad publicity they do not have",
            9712,
        ),
        (
            vec![
                Instruction::TakeBadPublicity { side: Side::Corp, amount: Quantity::c(1) },
                Instruction::GainCredits(Side::Corp, Quantity::BadPublicityOf(Side::Corp)),
            ],
            3,
            "9.12.2: the quantity is RE-READ where it is used — the third bad \
             publicity was taken by the instruction before it and is counted",
            9713,
        ),
    ] {
        let mut vm = Vm::empty(seed);
        let op = vm.new_object(tk::operation("Bad Press", 0, instrs), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.st.corp.bad_publicity = 2;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action().once(), Reply::play_card(op)).stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(vm.st.corp.credits, expected, "{why}: {}", t.tail(16));
    }
}

// ---------------------------------------------------------------------------
// CR 1.11.3 — the clicks a player has
// ---------------------------------------------------------------------------

/// CR 1.11.3 — the number of clicks the named player HAS.
///
/// The Runner plays one event whose only instruction gains 1[credit] for each
/// click they have. Played as the first action of the turn it finds 3 of the
/// 4 the turn allotted (1.11.2b), because the click spent to take the action
/// has already left the pool (1.11.3b) — which is what makes this a reading of
/// the pool and not of the allotment. Played as the third it finds 1. The
/// actions taken first are basic DRAWS (5.2.7d) rather than basic credits, so
/// the only credits the Runner ends with are the ones this quantity paid.
///
/// The other half is the side: the same word asked about the CORP during the
/// Runner's turn finds an empty pool, because 5.6.5 takes a player's unspent
/// clicks away when their turn ends.
#[test]
fn a_click_count_is_the_pool_as_it_stands_and_the_side_is_content() {
    for (of, draws_first, expected, why, seed) in [
        (
            Side::Runner,
            0u32,
            3u32,
            "1.11.2b/1.11.3b: 4 allotted, 1 spent to play this event, 3 left to count",
            9721u64,
        ),
        (
            Side::Runner,
            2,
            1,
            "…and after two basic draw actions the same word finds 1 — the pool \
             as it stands, not the 4 the turn allotted",
            9722,
        ),
        (
            Side::Corp,
            0,
            0,
            "the SIDE is content: the Corp's pool is empty during the Runner's \
             turn (5.6.5 took their unspent clicks when it ended)",
            9723,
        ),
    ] {
        let mut vm = Vm::empty(seed);
        let ev = vm.new_object(
            tk::event(
                "Click Counter",
                0,
                vec![Instruction::GainCredits(Side::Runner, Quantity::ClicksOf(of))],
            ),
            Zone::Hand(Side::Runner),
        );
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(ev);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let mut runner = Plan::runner();
        for _ in 0..draws_first {
            runner = runner.when(Match::action().once(), Reply::draw());
        }
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            runner.when(Match::action().once(), Reply::play_card(ev)).stop_at_action(),
        );
        assert_eq!(vm.st.runner.credits, expected, "{why}: {}", t.tail(16));
    }
}
