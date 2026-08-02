//! Per-card behavior tests mirroring the reference test corpus assertions.

mod common;
use common::*;
use jinteki_core::state::*;
use jinteki_core::types::*;
use jinteki_core::Command;

#[test]
fn hedge_fund_gains_9_and_babw_adds_1() {
    let mut st = new_test_game(1, &["Hedge Fund"], &[]);
    assert_eq!(st.credits(Side::Corp), 5);
    play(&mut st, Side::Corp, "Hedge Fund");
    // 5 - 5 + 9, +1 from Building a Better World (Hedge Fund is a Transaction).
    assert_eq!(st.credits(Side::Corp), 10);
    assert_eq!(st.clicks(Side::Corp), 2);
}

#[test]
fn beanstalk_royalties_gains_3_plus_babw() {
    let mut st = new_test_game(2, &["Beanstalk Royalties"], &[]);
    play(&mut st, Side::Corp, "Beanstalk Royalties");
    assert_eq!(st.credits(Side::Corp), 9); // 5 + 3 + 1
}

#[test]
fn sure_gamble_gains_9() {
    let mut st = new_test_game(3, &[], &["Sure Gamble"]);
    take_credits(&mut st, Side::Corp);
    let before = st.credits(Side::Runner);
    play(&mut st, Side::Runner, "Sure Gamble");
    assert_eq!(st.credits(Side::Runner), before - 5 + 9);
}

#[test]
fn easy_mark_gains_3() {
    let mut st = new_test_game(4, &[], &["Easy Mark"]);
    take_credits(&mut st, Side::Corp);
    let before = st.credits(Side::Runner);
    play(&mut st, Side::Runner, "Easy Mark");
    assert_eq!(st.credits(Side::Runner), before + 3);
}

#[test]
fn diesel_draws_3() {
    let mut st = new_test_game(5, &[], &["Diesel"]);
    take_credits(&mut st, Side::Corp);
    let before = st.hand(Side::Runner).len();
    play(&mut st, Side::Runner, "Diesel");
    assert_eq!(st.hand(Side::Runner).len(), before - 1 + 3);
}

#[test]
fn ice_wall_strength_and_etr() {
    let mut st = new_test_game(6, &["Ice Wall", "Offworld Office"], &[]);
    let iw = corp_install(&mut st, "Ice Wall", "New remote");
    let remote = newest_remote(&st);
    let _ag = corp_install(&mut st, "Offworld Office", &remote);
    cmd(&mut st, Side::Corp, Command::Rez { cid: iw });
    assert_eq!(st.ice_strength(iw), 1);
    advance_n(&mut st, iw, 1);
    assert_eq!(st.ice_strength(iw), 2);
    take_credits(&mut st, Side::Corp);

    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    // Ice already rezzed: encounter begins automatically.
    assert_eq!(st.run.as_ref().unwrap().phase, RunPhase::EncounterIce);
    cmd(&mut st, Side::Runner, Command::Continue); // let subs fire
    assert!(st.run.is_none(), "End the run subroutine should end the run");
}

#[test]
fn rez_window_prompt_on_approach() {
    let mut st = new_test_game(7, &["Ice Wall"], &[]);
    corp_install(&mut st, "Ice Wall", "hq");
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    // Corp gets the rez prompt.
    assert!(st.current_prompt(Side::Corp).is_some());
    click_prompt(&mut st, Side::Corp, "No action");
    // Unrezzed ice is passed; runner is in movement.
    assert_eq!(st.run.as_ref().unwrap().phase, RunPhase::Movement);
    cmd(&mut st, Side::Runner, Command::JackOut);
    assert!(st.run.is_none());
}

#[test]
fn pad_campaign_drips_at_turn_start() {
    let mut st = new_test_game(8, &["PAD Campaign"], &[]);
    let pad = corp_install(&mut st, "PAD Campaign", "New remote");
    cmd(&mut st, Side::Corp, Command::Rez { cid: pad });
    let before = st.credits(Side::Corp);
    take_credits(&mut st, Side::Corp); // corp's 2 remaining clicks, runner turn starts
    take_credits(&mut st, Side::Runner); // corp turn starts: drip fires
    // 2 clicked credits (1 click went to the install) + 1 drip.
    assert_eq!(st.credits(Side::Corp), before + 2 + 1);
}

#[test]
fn armitage_codebusting_takes_2_and_self_trashes() {
    let mut st = new_test_game(9, &[], &["Armitage Codebusting"]);
    take_credits(&mut st, Side::Corp);
    let arm = runner_install(&mut st, "Armitage Codebusting");
    assert_eq!(st.card(arm).counters.credit, 12);
    let mut taken = 0;
    for _ in 0..6 {
        while st.clicks(Side::Runner) > 0 && st.card(arm).counters.credit > 0 {
            cmd(&mut st, Side::Runner, Command::Ability { cid: arm, index: 0 });
            taken += 2;
        }
        if st.card(arm).counters.credit == 0 {
            break;
        }
        cmd(&mut st, Side::Runner, Command::EndTurn);
        cmd(&mut st, Side::Corp, Command::StartTurn);
        take_credits(&mut st, Side::Corp);
    }
    assert_eq!(taken, 12);
    assert!(st.discard(Side::Runner).iter().any(|&c| c == arm), "Armitage should self-trash");
}

#[test]
fn regolith_takes_3_per_click() {
    let mut st = new_test_game(10, &["Regolith Mining License"], &[]);
    let reg = corp_install(&mut st, "Regolith Mining License", "New remote");
    cmd(&mut st, Side::Corp, Command::Rez { cid: reg });
    assert_eq!(st.card(reg).counters.credit, 15);
    let before = st.credits(Side::Corp);
    cmd(&mut st, Side::Corp, Command::Ability { cid: reg, index: 0 });
    assert_eq!(st.credits(Side::Corp), before + 3);
    assert_eq!(st.card(reg).counters.credit, 12);
}

#[test]
fn offworld_office_scores_for_7() {
    let mut st = new_test_game(11, &["Offworld Office"], &[]);
    let ag = corp_install(&mut st, "Offworld Office", "New remote");
    // 4 advancements over two turns.
    advance_n(&mut st, ag, 2);
    take_credits(&mut st, Side::Corp);
    take_credits(&mut st, Side::Runner);
    advance_n(&mut st, ag, 2);
    let before = st.credits(Side::Corp);
    cmd(&mut st, Side::Corp, Command::Score { cid: ag });
    assert_eq!(st.agenda_points(Side::Corp), 2);
    assert_eq!(st.credits(Side::Corp), before + 7);
}

#[test]
fn hostile_takeover_gives_bad_pub_and_run_credits() {
    let mut st = new_test_game(12, &["Hostile Takeover"], &[]);
    let ag = corp_install(&mut st, "Hostile Takeover", "New remote");
    advance_n(&mut st, ag, 2);
    cmd(&mut st, Side::Corp, Command::Score { cid: ag });
    assert_eq!(st.bad_pub, 1);
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    // Bad-pub credit granted for the run (archives empty: run resolves through).
    assert!(st.log.iter().any(|l| l.text.contains("bad publicity credits")));
}

#[test]
fn priority_requisition_rezzes_ice_free() {
    let mut st = new_test_game(13, &["Priority Requisition", "Rototurret"], &[]);
    let roto = corp_install(&mut st, "Rototurret", "hq");
    let ag = corp_install(&mut st, "Priority Requisition", "New remote");
    advance_n(&mut st, ag, 1);
    take_credits(&mut st, Side::Corp);
    take_credits(&mut st, Side::Runner);
    advance_n(&mut st, ag, 2);
    take_credits(&mut st, Side::Corp);
    take_credits(&mut st, Side::Runner);
    advance_n(&mut st, ag, 2);
    let credits_before = st.credits(Side::Corp);
    cmd(&mut st, Side::Corp, Command::Score { cid: ag });
    // Select prompt: rez Rototurret ignoring its 4-credit cost.
    click_card(&mut st, Side::Corp, roto);
    assert!(st.card(roto).rezzed);
    assert_eq!(st.credits(Side::Corp), credits_before);
}

#[test]
fn superconducting_hub_hand_size_and_draw() {
    let mut st = new_test_game(14, &["Superconducting Hub"], &[]);
    let ag = corp_install(&mut st, "Superconducting Hub", "New remote");
    advance_n(&mut st, ag, 2);
    take_credits(&mut st, Side::Corp);
    take_credits(&mut st, Side::Runner);
    advance_n(&mut st, ag, 1);
    let hand_before = st.hand(Side::Corp).len();
    cmd(&mut st, Side::Corp, Command::Score { cid: ag });
    click_prompt(&mut st, Side::Corp, "Yes");
    assert_eq!(st.hand(Side::Corp).len(), hand_before + 2);
    assert_eq!(st.max_hand_size(Side::Corp), 7);
}

#[test]
fn enigma_takes_click_and_ends_run() {
    let mut st = new_test_game(15, &["Enigma"], &[]);
    let enigma = corp_install(&mut st, "Enigma", "hq");
    cmd(&mut st, Side::Corp, Command::Rez { cid: enigma });
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    let clicks_before = st.clicks(Side::Runner);
    cmd(&mut st, Side::Runner, Command::Continue);
    assert_eq!(st.clicks(Side::Runner), clicks_before - 1);
    assert!(st.run.is_none());
}

#[test]
fn tithe_nets_and_gains() {
    let mut st = new_test_game(16, &["Tithe"], &["Sure Gamble", "Diesel"]);
    let tithe = corp_install(&mut st, "Tithe", "hq");
    cmd(&mut st, Side::Corp, Command::Rez { cid: tithe });
    take_credits(&mut st, Side::Corp);
    let corp_credits = st.credits(Side::Corp);
    let grip = st.hand(Side::Runner).len();
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    cmd(&mut st, Side::Runner, Command::Continue); // both subs fire
    assert_eq!(st.hand(Side::Runner).len(), grip - 1, "1 net damage");
    assert_eq!(st.credits(Side::Corp), corp_credits + 1);
    // Tithe does not end the run: movement continues.
    assert!(st.run.is_some());
    cmd(&mut st, Side::Runner, Command::JackOut);
}

#[test]
fn rototurret_trashes_program_then_etr() {
    let mut st = new_test_game(17, &["Rototurret"], &["Mimic"]);
    let roto = corp_install(&mut st, "Rototurret", "hq");
    take_credits(&mut st, Side::Corp);
    let mimic = runner_install(&mut st, "Mimic");
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    click_prompt(&mut st, Side::Corp, "Rez");
    cmd(&mut st, Side::Runner, Command::Continue);
    // Corp must choose a program to trash.
    click_card(&mut st, Side::Corp, mimic);
    assert!(st.discard(Side::Runner).contains(&mimic));
    assert!(st.run.is_none(), "second sub ends the run");
}

#[test]
fn corroder_breaks_ice_wall() {
    let mut st = new_test_game(18, &["Ice Wall", "Hostile Takeover"], &["Corroder"]);
    let iw = corp_install(&mut st, "Ice Wall", "New remote");
    let remote = newest_remote(&st);
    let ag = corp_install(&mut st, "Hostile Takeover", &remote);
    let _ = ag;
    take_credits(&mut st, Side::Corp);
    let corroder = runner_install(&mut st, "Corroder");
    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    click_prompt(&mut st, Side::Corp, "Rez");
    assert!(st.card(iw).rezzed);
    // Break the ETR sub.
    cmd(&mut st, Side::Runner, Command::Ability { cid: corroder, index: 0 });
    click_prompt(&mut st, Side::Runner, "End the run");
    cmd(&mut st, Side::Runner, Command::Continue); // nothing fires
    assert!(st.run.is_some());
    cmd(&mut st, Side::Runner, Command::Continue); // movement -> approach server -> access
    // Access the agenda: steal it.
    click_prompt(&mut st, Side::Runner, "Steal");
    assert_eq!(st.agenda_points(Side::Runner), 1);
    assert!(st.run.is_none());
}

#[test]
fn gordian_pump_lasts_the_run() {
    let mut st = new_test_game(19, &["Enigma", "Enigma"], &["Gordian Blade", "Sure Gamble"]);
    st.credits[0] = 10; // test cheat, like the reference's /credit command
    let e1 = corp_install(&mut st, "Enigma", "rd");
    let e2 = corp_install(&mut st, "Enigma", "rd");
    cmd(&mut st, Side::Corp, Command::Rez { cid: e1 });
    cmd(&mut st, Side::Corp, Command::Rez { cid: e2 });
    take_credits(&mut st, Side::Corp);
    play(&mut st, Side::Runner, "Sure Gamble");
    let gb = runner_install(&mut st, "Gordian Blade");
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Rd });
    // Encounter outermost Enigma (str 2): break both subs after no pump needed
    // (Gordian base str 2 >= 2), but pump anyway to prove persistence.
    cmd(&mut st, Side::Runner, Command::Ability { cid: gb, index: 1 });
    assert_eq!(st.breaker_strength(gb), 3);
    cmd(&mut st, Side::Runner, Command::Ability { cid: gb, index: 0 });
    click_prompt(&mut st, Side::Runner, "The Runner loses [Click]");
    cmd(&mut st, Side::Runner, Command::Ability { cid: gb, index: 0 });
    click_prompt(&mut st, Side::Runner, "End the run");
    cmd(&mut st, Side::Runner, Command::Continue);
    cmd(&mut st, Side::Runner, Command::Continue); // movement -> encounter inner Enigma
    assert_eq!(st.run.as_ref().unwrap().phase, RunPhase::EncounterIce);
    // Pump persisted into the second encounter (end-of-run duration).
    assert_eq!(st.breaker_strength(gb), 3);
    cmd(&mut st, Side::Runner, Command::Continue); // inner Enigma fires: ETR
    assert!(st.run.is_none());
    // After the run ends, the pump expires.
    assert_eq!(st.breaker_strength(gb), 2);
}

#[test]
fn dirty_laundry_pays_on_success() {
    let mut st = new_test_game(20, &[], &["Dirty Laundry"]);
    take_credits(&mut st, Side::Corp);
    let before = st.credits(Side::Runner);
    play(&mut st, Side::Runner, "Dirty Laundry");
    click_prompt(&mut st, Side::Runner, "Archives");
    // No ice, archives empty: run resolves to success immediately.
    assert!(st.run.is_none());
    assert_eq!(st.credits(Side::Runner), before - 2 + 5);
}

#[test]
fn legwork_accesses_3_from_hq() {
    let mut st = new_test_game(
        21,
        &["Hostile Takeover", "Hostile Takeover", "Hostile Takeover"],
        &["Legwork"],
    );
    // Re-stack after the mandatory draw so HQ is exactly the three agendas.
    stack_hand(&mut st, Side::Corp, &["Hostile Takeover", "Hostile Takeover", "Hostile Takeover"]);
    take_credits(&mut st, Side::Corp);
    play(&mut st, Side::Runner, "Legwork");
    for _ in 0..3 {
        click_prompt(&mut st, Side::Runner, "Steal");
    }
    assert_eq!(st.agenda_points(Side::Runner), 3);
    assert!(st.run.is_none());
}

#[test]
fn makers_eye_accesses_3_from_rd() {
    let mut st = new_test_game(22, &[], &["The Maker's Eye"]);
    take_credits(&mut st, Side::Corp);
    play(&mut st, Side::Runner, "The Maker's Eye");
    let mut accesses = 0;
    while st.run.is_some() {
        let p = st.current_prompt(Side::Runner).expect("access prompt");
        let label = p.choices[0].label.clone();
        click_prompt(&mut st, Side::Runner, &label);
        accesses += 1;
        assert!(accesses <= 3, "should access exactly 3 cards");
    }
    assert_eq!(accesses, 3);
}

#[test]
fn akamatsu_raises_mu() {
    let mut st = new_test_game(23, &[], &["Akamatsu Mem Chip"]);
    take_credits(&mut st, Side::Corp);
    assert_eq!(st.mu_limit(), 4);
    runner_install(&mut st, "Akamatsu Mem Chip");
    assert_eq!(st.mu_limit(), 5);
}

#[test]
fn gabriel_gains_2_on_first_hq_run() {
    let mut st = GameState::new_with_decks(
        24,
        jinteki_core::carddb::CORP_ID,
        &jinteki_core::carddb::corp_deck(),
        "Gabriel Santiago: Consummate Professional",
        &jinteki_core::carddb::runner_deck(),
    );
    click_prompt(&mut st, Side::Corp, "Keep");
    click_prompt(&mut st, Side::Runner, "Keep");
    cmd(&mut st, Side::Corp, Command::StartTurn);
    take_credits(&mut st, Side::Corp);
    let before = st.credits(Side::Runner);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    // Unprotected HQ: success, gain 2, then access one random card.
    let p = st.current_prompt(Side::Runner).expect("access prompt");
    let label = p.choices[0].label.clone();
    click_prompt(&mut st, Side::Runner, &label);
    assert!(st.credits(Side::Runner) >= before + 2 - 5, "gained 2 minus any trash cost");
    assert!(st.log.iter().any(|l| l.text.contains("gain 2 [Credits]")));
}

#[test]
fn steal_from_archives() {
    let mut st = new_test_game(25, &["Hostile Takeover"], &[]);
    let ag = find_in_hand(&st, Side::Corp, "Hostile Takeover");
    st.trash(ag, false); // discarded facedown
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    click_prompt(&mut st, Side::Runner, "Steal");
    assert_eq!(st.agenda_points(Side::Runner), 1);
}

#[test]
fn corp_decked_loses() {
    let mut st = new_test_game(26, &[], &[]);
    st.deck[0].clear();
    take_credits(&mut st, Side::Corp);
    take_credits(&mut st, Side::Runner); // corp start-turn draw fails
    assert_eq!(st.winner, Some(Side::Runner));
    assert_eq!(st.reason.as_deref(), Some("Decked"));
}

#[test]
fn flatline_on_net_damage_with_empty_grip() {
    let mut st = new_test_game(27, &["Tithe"], &[]);
    let tithe = corp_install(&mut st, "Tithe", "hq");
    cmd(&mut st, Side::Corp, Command::Rez { cid: tithe });
    take_credits(&mut st, Side::Corp);
    // Empty the grip.
    let hand = st.hand(Side::Runner).clone();
    for c in hand {
        st.trash(c, true);
    }
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    cmd(&mut st, Side::Runner, Command::Continue);
    assert_eq!(st.winner, Some(Side::Corp));
    assert_eq!(st.reason.as_deref(), Some("Flatline"));
}

#[test]
fn mulligan_redraws_5() {
    let mut st = GameState::new(28);
    click_prompt(&mut st, Side::Corp, "Keep");
    let before: Vec<Cid> = st.hand(Side::Runner).clone();
    click_prompt(&mut st, Side::Runner, "Mulligan");
    assert_eq!(st.hand(Side::Runner).len(), 5);
    // New hand should (overwhelmingly) differ; assert the deck got shuffled in.
    let after: Vec<Cid> = st.hand(Side::Runner).clone();
    assert_ne!(before, after);
}

#[test]
fn trash_accessed_pad_campaign() {
    let mut st = new_test_game(29, &["PAD Campaign"], &["Sure Gamble"]);
    corp_install(&mut st, "PAD Campaign", "New remote");
    let remote = newest_remote(&st);
    take_credits(&mut st, Side::Corp);
    play(&mut st, Side::Runner, "Sure Gamble");
    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    let before = st.credits(Side::Runner);
    click_prompt(&mut st, Side::Runner, "Pay 4 [Credits] to trash");
    assert_eq!(st.credits(Side::Runner), before - 4);
    assert_eq!(st.discard(Side::Corp).len(), 1);
    assert!(st.run.is_none());
}

#[test]
fn win_at_7_points() {
    let mut st = new_test_game(30, &["Offworld Office", "Offworld Office", "Priority Requisition"], &[]);
    // Cheat the agendas straight into the score area to test the win check.
    for title in ["Offworld Office", "Offworld Office", "Priority Requisition"] {
        let cid = find_in_hand(&st, Side::Corp, title);
        st.to_score_area(cid, Side::Corp);
    }
    st.check_agenda_win();
    assert_eq!(st.winner, Some(Side::Corp));
}
