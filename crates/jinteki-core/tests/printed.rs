//! Full card database + coverage: every Netrunner card exists in the system,
//! with unimplemented behaviors isolated and playable as vanilla.

mod common;

use common::*;
use jinteki_core::printed::{self, ImplStatus};
use jinteki_core::state::GameState;
use jinteki_core::types::*;
use jinteki_core::view::{render_state, Viewer};
use jinteki_core::{carddb, Command};

#[test]
fn database_loads_and_covers_the_whole_pool() {
    let all = printed::all_printed();
    assert!(
        all.len() >= 2000,
        "expected the full card pool, got {}",
        all.len()
    );
    // Every hand-implemented behavior row is a real card.
    for def in carddb::CARDS {
        assert!(
            printed::printed(def.title).is_some(),
            "behavior row not in printed data: {}",
            def.title
        );
    }
}

#[test]
fn printed_stats_spot_checks() {
    let hf = printed::printed("Hedge Fund").expect("Hedge Fund exists");
    assert_eq!(hf.cost, Some(5));
    assert_eq!(hf.card_type, "Operation");
    assert_eq!(hf.side, "Corp");

    let iw = printed::printed("Ice Wall").expect("Ice Wall exists");
    assert_eq!(iw.strength, Some(1));
    assert_eq!(iw.cost, Some(1));

    let cor = printed::printed("Corroder").expect("Corroder exists");
    assert_eq!(cor.memoryunits, Some(1));
    assert_eq!(cor.cost, Some(2));

    let oo = printed::printed("Offworld Office").expect("Offworld Office exists");
    assert_eq!(oo.advancement_requirement, Some(4));
    assert_eq!(oo.agenda_points, Some(2));

    // Text now flows from printed data for every title.
    assert!(printed::printed_text("Daily Casts").is_some());
    assert!(carddb::card_text("Daily Casts").contains("credit"));
}

#[test]
fn impl_status_classifies_titles() {
    assert_eq!(printed::impl_status("Hedge Fund"), ImplStatus::Behavior);
    assert_eq!(printed::impl_status("Corroder"), ImplStatus::Behavior);
    // jnet implements these; we have no behavior row yet.
    assert_eq!(printed::impl_status("Daily Casts"), ImplStatus::JnetOnly);
    assert_eq!(printed::impl_status("Palisade"), ImplStatus::JnetOnly);
    // Not implemented anywhere (rules-insert pseudo-card): isolated.
    assert_eq!(printed::impl_status("Charge"), ImplStatus::Unimplemented);
    // Unknown titles are isolated too, not a crash.
    assert_eq!(
        printed::impl_status("This Card Does Not Exist"),
        ImplStatus::Unimplemented
    );
    // Partial-implementation caveats from the reference are tracked.
    let wendigo = printed::coverage_row("Wendigo").expect("Wendigo covered");
    assert!(wendigo.jnet_impl && wendigo.jnet_partial);
}

#[test]
fn unknown_title_is_a_clean_error() {
    assert!(carddb::def_index_or_synth("This Card Does Not Exist").is_err());
    // Rules inserts exist in the data but are not playable cards.
    assert!(carddb::def_index_or_synth("Charge").is_err());
}

#[test]
fn synthesized_vanilla_defs_carry_printed_stats_and_no_behavior() {
    let idx = carddb::def_index_or_synth("Palisade").expect("Palisade synthesizes");
    let def = carddb::def_at(idx);
    assert_eq!(def.title, "Palisade");
    assert_eq!(def.kind, CardType::Ice);
    assert_eq!(def.cost, 3);
    assert_eq!(def.strength, Some(2));
    assert_eq!(def.ice_subtype, Some(IceSubtype::Barrier));
    assert!(def.subroutines.is_empty(), "vanilla ice has zero subroutines");
    assert!(def.triggered.is_empty(), "vanilla defs register no abilities");

    // Interned: same index on the second request.
    assert_eq!(carddb::def_index_or_synth("Palisade").unwrap(), idx);
}

#[test]
fn game_with_jnet_only_titles_spawns_and_flags_them() {
    let corp: Vec<&str> = std::iter::repeat_n("Hedge Fund", 8).collect();
    let mut runner: Vec<&str> = std::iter::repeat_n("Sure Gamble", 6).collect();
    runner.extend(["Daily Casts", "Daily Casts", "Daily Casts"]);
    let mut st = GameState::new_with_decks(
        7,
        carddb::CORP_ID,
        &corp,
        carddb::RUNNER_ID,
        &runner,
    );
    keep_both_and_start(&mut st);
    take_credits(&mut st, Side::Corp);

    stack_hand(&mut st, Side::Runner, &["Daily Casts", "Sure Gamble"]);
    let cid = runner_install(&mut st, "Daily Casts");
    assert!(st.rig.resources.contains(&cid), "Daily Casts installs and sits there");
    assert_eq!(st.credits(Side::Runner), 2, "paid the printed 3-credit cost");

    let json = render_state(&st, Viewer::Side(Side::Runner));
    let rig = json["runner"]["rig"]["resource"].as_array().unwrap();
    let dc = rig
        .iter()
        .find(|c| c["title"] == "Daily Casts")
        .expect("Daily Casts rendered");
    assert_eq!(
        dc["implementation"],
        "rs-unimplemented: engine treats as vanilla"
    );
    assert!(dc["text"].as_str().unwrap().contains("credit"));

    // Behavior-covered cards carry a null implementation flag.
    let hand = json["runner"]["hand"].as_array().unwrap();
    let sg = hand
        .iter()
        .find(|c| c["title"] == "Sure Gamble")
        .expect("Sure Gamble in hand");
    assert!(sg["implementation"].is_null());
}

#[test]
fn vanilla_operation_resolves_with_cost_only_and_log_line() {
    let mut corp: Vec<&str> = std::iter::repeat_n("Hedge Fund", 6).collect();
    corp.extend(["Anonymous Tip", "Anonymous Tip", "Anonymous Tip"]);
    let runner: Vec<&str> = std::iter::repeat_n("Sure Gamble", 8).collect();
    let mut st = GameState::new_with_decks(
        11,
        carddb::CORP_ID,
        &corp,
        carddb::RUNNER_ID,
        &runner,
    );
    keep_both_and_start(&mut st);

    stack_hand(&mut st, Side::Corp, &["Anonymous Tip"]);
    let credits_before = st.credits(Side::Corp);
    let cid = find_in_hand(&st, Side::Corp, "Anonymous Tip");
    cmd(&mut st, Side::Corp, Command::Play { cid });

    assert_eq!(
        st.credits(Side::Corp),
        credits_before,
        "cost 0 paid, no effect resolved"
    );
    assert!(st.discard(Side::Corp).contains(&cid), "operation went to Archives");
    assert!(
        st.log.iter().any(|l| l.text.contains("(no implemented effect)")),
        "vanilla resolution is visible in the log"
    );
}
