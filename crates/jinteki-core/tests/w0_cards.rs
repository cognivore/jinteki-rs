//! W0 mechanics-pack cards: tags, traces, psi games, expose, on-access
//! (ambush), on-rez loading, generic counters, and the corp tag actions.
//!
//! Each test mirrors the reference test where one exists
//! (`jinteki-reference/test/clj/game/cards/*_test.clj`); cards without a
//! reference test get a behavior test written from card text + defcard.

mod common;
use common::*;
use jinteki_core::state::*;
use jinteki_core::types::*;
use jinteki_core::Command;

/// Answer whatever prompt is open for `side` with its first choice.
fn click_first(st: &mut GameState, side: Side) {
    let label = st
        .current_prompt(side)
        .unwrap_or_else(|| panic!("no prompt for {side:?}"))
        .choices[0]
        .label
        .clone();
    click_prompt(st, side, &label);
}

// ── Snare! (assets_test.clj `snare`) ───────────────────────────────────────

#[test]
fn snare_pays_4_tags_and_damages_on_remote_access() {
    let mut st = new_test_game(101, &["Snare!"], &["Sure Gamble", "Sure Gamble", "Sure Gamble", "Easy Mark", "Easy Mark"]);
    corp_install(&mut st, "Snare!", "New remote");
    let remote = newest_remote(&st);
    take_credits(&mut st, Side::Corp);
    assert_eq!(st.credits(Side::Corp), 7, "corp has 7 before the ambush");

    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    // On-access resolves BEFORE the trash/no-action prompt.
    assert!(st.current_prompt(Side::Corp).is_some(), "corp asked to pay 4");
    let grip_before = st.hand(Side::Runner).len();
    click_prompt(&mut st, Side::Corp, "Yes");
    assert_eq!(st.credits(Side::Corp), 3, "corp had 7 and paid 4 for Snare!");
    assert_eq!(st.tags, 1, "runner has 1 tag");
    assert_eq!(
        st.hand(Side::Runner).len(),
        grip_before - 3,
        "runner took 3 net damage"
    );
    // Now the normal access prompt (Snare!'s printed trash cost is 0).
    click_prompt(&mut st, Side::Runner, "Pay 0 [Credits] to trash");
    assert!(st.discard(Side::Corp).len() == 1, "Snare! trashed");
    assert!(st.run.is_none());
}

#[test]
fn snare_fires_on_hq_access_but_corp_may_decline() {
    let mut st = new_test_game(102, &["Snare!"], &["Sure Gamble", "Sure Gamble", "Sure Gamble", "Easy Mark", "Easy Mark"]);
    // Re-stack after the mandatory draw so HQ is exactly [Snare!].
    stack_hand(&mut st, Side::Corp, &["Snare!"]);
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    // Accessed from hand: "anywhere except in Archives" still fires.
    assert!(st.current_prompt(Side::Corp).is_some(), "ambush fires from HQ");
    click_prompt(&mut st, Side::Corp, "No");
    assert_eq!(st.tags, 0, "declined: no tag");
    assert_eq!(st.hand(Side::Runner).len(), 5, "declined: no damage");
    click_prompt(&mut st, Side::Runner, "Pay 0 [Credits] to trash");
    assert!(st.run.is_none());
}

// ── Ghost Branch (assets_test.clj `ghost-branch`) ──────────────────────────

#[test]
fn ghost_branch_gives_tags_per_advancement() {
    let mut st = new_test_game(103, &["Ghost Branch"], &[]);
    let gb = corp_install(&mut st, "Ghost Branch", "New remote");
    let remote = newest_remote(&st);
    advance_n(&mut st, gb, 2);
    assert_eq!(st.card(gb).advancement, 2);
    take_credits(&mut st, Side::Corp);
    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    click_prompt(&mut st, Side::Corp, "Yes");
    assert_eq!(st.tags, 2, "runner given 2 tags");
    click_prompt(&mut st, Side::Runner, "Pay 0 [Credits] to trash");
    assert!(st.run.is_none());
}

#[test]
fn ghost_branch_with_no_advancements_does_not_fire() {
    let mut st = new_test_game(104, &["Ghost Branch"], &[]);
    corp_install(&mut st, "Ghost Branch", "New remote");
    let remote = newest_remote(&st);
    take_credits(&mut st, Side::Corp);
    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    // No corp prompt: :req (pos? advancements) fails; straight to access.
    assert!(st.current_prompt(Side::Corp).is_none());
    click_prompt(&mut st, Side::Runner, "Pay 0 [Credits] to trash");
    assert_eq!(st.tags, 0);
}

// ── Project Junebug (assets_test.clj `project-junebug`) ────────────────────

#[test]
fn project_junebug_does_double_advancement_net_damage() {
    let mut st = new_test_game(105, &["Project Junebug"], &["Sure Gamble", "Sure Gamble", "Sure Gamble", "Easy Mark", "Easy Mark"]);
    let jb = corp_install(&mut st, "Project Junebug", "New remote");
    let remote = newest_remote(&st);
    advance_n(&mut st, jb, 2);
    take_credits(&mut st, Side::Corp);
    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    let credits = st.credits(Side::Corp);
    click_prompt(&mut st, Side::Corp, "Yes");
    assert_eq!(
        st.credits(Side::Corp),
        credits - 1,
        "corp should pay 1 for Project Junebug ability"
    );
    assert_eq!(
        st.discard(Side::Runner).len(),
        4,
        "Project Junebug should do 4 net damage"
    );
    click_prompt(&mut st, Side::Runner, "Pay 0 [Credits] to trash");
}

// ── Cerebral Overwriter (assets_test.clj `cerebral-overwriter`) ────────────

#[test]
fn cerebral_overwriter_does_core_damage() {
    let mut st = new_test_game(106, &["Cerebral Overwriter"], &["Sure Gamble", "Sure Gamble", "Sure Gamble", "Easy Mark", "Easy Mark"]);
    let co = corp_install(&mut st, "Cerebral Overwriter", "New remote");
    let remote = newest_remote(&st);
    advance_n(&mut st, co, 2);
    assert_eq!(st.card(co).advancement, 2);
    take_credits(&mut st, Side::Corp);
    let server = ServerId::from_key(&remote).unwrap();
    cmd(&mut st, Side::Runner, Command::Run { server });
    click_prompt(&mut st, Side::Corp, "Yes"); // pay 3, do the optional ability
    assert_eq!(st.brain_damage, 2, "runner takes 2 core damage");
    assert_eq!(st.hand(Side::Runner).len(), 3, "2 cards trashed from grip");
    assert_eq!(
        st.max_hand_size(Side::Runner),
        3,
        "core damage permanently lowers max hand size"
    );
    click_prompt(&mut st, Side::Runner, "Pay 0 [Credits] to trash");
}

// ── Psychic Field (assets_test.clj `psychic-field`) ────────────────────────

#[test]
fn psychic_field_psi_game_on_expose() {
    let mut st = new_test_game(
        107,
        &["Psychic Field", "Psychic Field"],
        &["Infiltration", "Sure Gamble", "Sure Gamble"],
    );
    let psyf1 = corp_install(&mut st, "Psychic Field", "New remote");
    corp_install(&mut st, "Psychic Field", "New remote");
    take_credits(&mut st, Side::Corp);
    let corp_credits = st.credits(Side::Corp);

    play(&mut st, Side::Runner, "Infiltration");
    click_prompt(&mut st, Side::Runner, "Expose a card");
    click_card(&mut st, Side::Runner, psyf1);
    assert_eq!(st.hand(Side::Runner).len(), 2);
    // Both psi prompts are open; neither message leaks the other's bid.
    assert!(st.current_prompt(Side::Corp).is_some());
    assert!(st.current_prompt(Side::Runner).is_some());
    click_prompt(&mut st, Side::Corp, "2 [Credits]");
    click_prompt(&mut st, Side::Runner, "0 [Credits]");
    assert_eq!(
        st.discard(Side::Runner).len(),
        3,
        "suffered 2 net damage on expose and psi loss (plus played Infiltration)"
    );
    assert_eq!(st.credits(Side::Corp), corp_credits - 2, "corp paid its bid");
}

#[test]
fn psychic_field_fires_on_installed_access_not_hq() {
    let mut st = new_test_game(108, &["Psychic Field"], &[]);
    // In HQ the ambush must NOT fire (installed-only condition).
    stack_hand(&mut st, Side::Corp, &["Psychic Field"]);
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    assert!(
        st.current_prompt(Side::Corp).map(|p| p.choices.is_empty()).unwrap_or(true),
        "no psi game on HQ access"
    );
    // Straight to the access prompt (trash cost 2).
    click_prompt(&mut st, Side::Runner, "Pay 2 [Credits] to trash");
    assert!(st.run.is_none());
}

// ── Adonis Campaign (campaign helper: load 12, take 3 each turn) ───────────

#[test]
fn adonis_campaign_loads_12_takes_3_and_self_trashes() {
    let mut st = new_test_game(109, &["Adonis Campaign"], &[]);
    let adonis = corp_install(&mut st, "Adonis Campaign", "New remote");
    cmd(&mut st, Side::Corp, Command::Rez { cid: adonis });
    assert_eq!(st.card(adonis).counters.credit, 12, "loads 12 when rezzed");
    assert_eq!(st.credits(Side::Corp), 1, "paid 4 to rez");

    let mut expected = 1;
    for turn in 0..4 {
        take_credits(&mut st, Side::Corp);
        expected += st.clicks(Side::Runner) * 0; // clarity: corp clicked credits
        take_credits(&mut st, Side::Runner);
        // Corp turn began: Adonis takes 3.
        let left = 12 - 3 * (turn + 1);
        if left > 0 {
            assert_eq!(st.card(adonis).counters.credit, left as u32);
        }
    }
    let _ = expected;
    assert!(
        st.discard(Side::Corp).contains(&adonis),
        "Adonis trashes itself when the last credits are taken"
    );
    assert!(st
        .log
        .iter()
        .any(|l| l.text.contains("uses Adonis Campaign to gain 3 [Credits]")));
}

// ── Nisei MK II (agendas_test.clj `nisei-mk-ii`) ───────────────────────────

#[test]
fn nisei_mk_ii_counter_ends_the_run() {
    let mut st = new_test_game(110, &["Nisei MK II", "Vanilla"], &[]);
    corp_install(&mut st, "Vanilla", "hq");
    let nisei = corp_install(&mut st, "Nisei MK II", "New remote");
    advance_n(&mut st, nisei, 1);
    take_credits(&mut st, Side::Corp);
    take_credits(&mut st, Side::Runner);
    advance_n(&mut st, nisei, 3);
    cmd(&mut st, Side::Corp, Command::Score { cid: nisei });
    assert_eq!(
        st.card(nisei).counters.agenda,
        1,
        "scored Nisei has one agenda counter"
    );
    take_credits(&mut st, Side::Corp);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Hq });
    click_prompt(&mut st, Side::Corp, "No action"); // decline to rez Vanilla
    assert_eq!(
        st.run.as_ref().unwrap().phase,
        RunPhase::Movement,
        "in movement phase"
    );
    cmd(&mut st, Side::Corp, Command::Ability { cid: nisei, index: 0 });
    assert!(st.run.is_none(), "run ended by using Nisei counter");
    assert_eq!(
        st.card(nisei).counters.agenda,
        0,
        "scored Nisei has no counters"
    );
}

// ── Data Raven (defcard: encounter tag-or-ETR, power-counter tag, trace) ───

#[test]
fn data_raven_encounter_choice_trace_and_power_counter() {
    let mut st = new_test_game(111, &["Data Raven"], &[]);
    let dr = corp_install(&mut st, "Data Raven", "archives");
    cmd(&mut st, Side::Corp, Command::Rez { cid: dr });
    assert_eq!(st.credits(Side::Corp), 1);
    take_credits(&mut st, Side::Corp);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    // Encounter: the runner must take 1 tag or end the run.
    click_prompt(&mut st, Side::Runner, "Take 1 tag");
    assert_eq!(st.tags, 1, "took the encounter tag");

    // Let the trace subroutine fire: Trace 3 - place 1 power counter.
    cmd(&mut st, Side::Runner, Command::Continue);
    click_prompt(&mut st, Side::Corp, "0"); // corp boost
    click_prompt(&mut st, Side::Runner, "0"); // runner link boost
    assert_eq!(st.card(dr).counters.power, 1, "trace success placed a power counter");

    // Finish the run (archives empty: breach ends it).
    cmd(&mut st, Side::Runner, Command::Continue);
    assert!(st.run.is_none());

    // Corp turn: spend the hosted power counter to give 1 tag.
    take_credits(&mut st, Side::Runner);
    cmd(&mut st, Side::Corp, Command::Ability { cid: dr, index: 0 });
    assert_eq!(st.tags, 2, "power counter gave a second tag");
    assert_eq!(st.card(dr).counters.power, 0);
}

#[test]
fn data_raven_end_the_run_choice_ends_the_run() {
    let mut st = new_test_game(112, &["Data Raven"], &[]);
    let dr = corp_install(&mut st, "Data Raven", "archives");
    cmd(&mut st, Side::Corp, Command::Rez { cid: dr });
    take_credits(&mut st, Side::Corp);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    click_prompt(&mut st, Side::Runner, "End the run");
    assert!(st.run.is_none(), "runner chose to end the run");
    assert_eq!(st.tags, 0);
}

// ── Caduceus (defcard: trace 3 gain 3, trace 2 ETR) ────────────────────────

#[test]
fn caduceus_gains_credits_on_first_trace_second_beaten_by_link_boost() {
    let mut st = new_test_game(113, &["Caduceus"], &[]);
    let cad = corp_install(&mut st, "Caduceus", "archives");
    cmd(&mut st, Side::Corp, Command::Rez { cid: cad });
    assert_eq!(st.credits(Side::Corp), 2);
    take_credits(&mut st, Side::Corp);
    assert_eq!(st.credits(Side::Corp), 4);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    cmd(&mut st, Side::Runner, Command::Continue);
    // Sub 1: Trace 3 - gain 3 credits. Corp doesn't boost, runner doesn't.
    click_prompt(&mut st, Side::Corp, "0");
    click_prompt(&mut st, Side::Runner, "0");
    assert_eq!(st.credits(Side::Corp), 7, "corp gained 3 from the trace");
    // Sub 2: Trace 2 - end the run. Runner boosts link to 3: 2 > 3 fails.
    click_prompt(&mut st, Side::Corp, "0");
    click_prompt(&mut st, Side::Runner, "3");
    assert_eq!(st.credits(Side::Runner), 2, "runner paid 3 to beat the trace");
    assert!(st.run.is_some(), "failed trace does not end the run");
    cmd(&mut st, Side::Runner, Command::JackOut);
}

// ── Hunter (defcard: tag-trace 3) ──────────────────────────────────────────

#[test]
fn hunter_trace_gives_tag_unless_link_wins() {
    let mut st = new_test_game(114, &["Hunter"], &[]);
    let hunter = corp_install(&mut st, "Hunter", "archives");
    cmd(&mut st, Side::Corp, Command::Rez { cid: hunter });
    take_credits(&mut st, Side::Corp);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    cmd(&mut st, Side::Runner, Command::Continue);
    click_prompt(&mut st, Side::Corp, "0");
    click_prompt(&mut st, Side::Runner, "0");
    assert_eq!(st.tags, 1, "trace 3 vs link 0: runner tagged");
    cmd(&mut st, Side::Runner, Command::JackOut);

    // Second run: the runner buys the trace off (3 >= 3 means corp loses —
    // trace succeeds only on strictly greater).
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    cmd(&mut st, Side::Runner, Command::Continue);
    click_prompt(&mut st, Side::Corp, "0");
    click_prompt(&mut st, Side::Runner, "3");
    assert_eq!(st.tags, 1, "equal strengths: no new tag");
    cmd(&mut st, Side::Runner, Command::JackOut);
}

// ── Neural Katana (defcard: 3 net damage) ──────────────────────────────────

#[test]
fn neural_katana_does_3_net_damage() {
    let mut st = new_test_game(115, &["Neural Katana"], &["Sure Gamble", "Sure Gamble", "Sure Gamble", "Easy Mark", "Easy Mark"]);
    let nk = corp_install(&mut st, "Neural Katana", "archives");
    cmd(&mut st, Side::Corp, Command::Rez { cid: nk });
    take_credits(&mut st, Side::Corp);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    let grip = st.hand(Side::Runner).len();
    cmd(&mut st, Side::Runner, Command::Continue);
    assert_eq!(st.hand(Side::Runner).len(), grip - 3, "3 net damage");
    assert!(st.run.is_some(), "Neural Katana does not end the run");
    cmd(&mut st, Side::Runner, Command::JackOut);
}

// ── Snowflake (defcard: psi sub, ETR on differing bids) ────────────────────

#[test]
fn snowflake_psi_ends_run_when_bids_differ() {
    let mut st = new_test_game(116, &["Snowflake"], &[]);
    let sf = corp_install(&mut st, "Snowflake", "archives");
    cmd(&mut st, Side::Corp, Command::Rez { cid: sf });
    take_credits(&mut st, Side::Corp);
    let corp_credits = st.credits(Side::Corp);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    cmd(&mut st, Side::Runner, Command::Continue);
    click_prompt(&mut st, Side::Corp, "1 [Credits]");
    click_prompt(&mut st, Side::Runner, "0 [Credits]");
    assert_eq!(st.credits(Side::Corp), corp_credits - 1, "corp paid its bid");
    assert!(st.run.is_none(), "differing bids end the run");

    // Equal bids: the run survives.
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    cmd(&mut st, Side::Runner, Command::Continue);
    click_prompt(&mut st, Side::Corp, "0 [Credits]");
    click_prompt(&mut st, Side::Runner, "0 [Credits]");
    assert!(st.run.is_some(), "equal bids do not end the run");
    cmd(&mut st, Side::Runner, Command::JackOut);
}

// ── SEA Source (operations_test.clj `sea-source`) ──────────────────────────

#[test]
fn sea_source_traces_for_a_tag_and_gates_on_last_turn_run() {
    let mut st = new_test_game(117, &["SEA Source"], &[]);
    // Play only if the runner made a successful run during their LAST turn.
    let sea = find_in_hand(&st, Side::Corp, "SEA Source");
    assert!(
        try_cmd(&mut st, Side::Corp, Command::Play { cid: sea }).is_err(),
        "no run last turn: SEA Source unplayable"
    );
    take_credits(&mut st, Side::Corp);

    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    assert!(st.run.is_none(), "empty archives: run completed");
    take_credits(&mut st, Side::Runner);

    assert_eq!(st.tags, 0, "runner should start with 0 tags");
    play(&mut st, Side::Corp, "SEA Source");
    click_prompt(&mut st, Side::Corp, "0");
    click_prompt(&mut st, Side::Runner, "0");
    assert_eq!(st.tags, 1, "runner should get 1 tag from losing SEA Source trace");
}

// ── Infiltration (events_test.clj `infiltration-gain-2` / `-expose`) ───────

#[test]
fn infiltration_gain_2() {
    let mut st = new_test_game(118, &[], &["Infiltration"]);
    take_credits(&mut st, Side::Corp);
    let credits = st.credits(Side::Runner);
    play(&mut st, Side::Runner, "Infiltration");
    click_prompt(&mut st, Side::Runner, "Gain 2 [Credits]");
    assert_eq!(st.credits(Side::Runner), credits + 2, "runner gains 2");
}

#[test]
fn infiltration_exposes_a_card() {
    let mut st = new_test_game(119, &["Ice Wall"], &["Infiltration"]);
    let iw = corp_install(&mut st, "Ice Wall", "hq");
    take_credits(&mut st, Side::Corp);
    play(&mut st, Side::Runner, "Infiltration");
    click_prompt(&mut st, Side::Runner, "Expose a card");
    click_card(&mut st, Side::Runner, iw);
    assert!(
        st.log
            .iter()
            .any(|l| l.text.contains("uses Infiltration to expose Ice Wall")),
        "Infiltration properly exposes the ice"
    );
}

// ── Datasucker (programs_test.clj `datasucker`, Wall of Static for Fire
//    Wall — same barrier pattern, in-pool) + purge basic action ─────────────

#[test]
fn datasucker_gains_virus_on_central_runs_and_lowers_ice_strength() {
    let mut st = new_test_game(120, &["Wall of Static"], &["Datasucker"]);
    corp_install(&mut st, "Wall of Static", "New remote");
    let remote = newest_remote(&st);
    take_credits(&mut st, Side::Corp);

    let ds = runner_install(&mut st, "Datasucker");
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    assert_eq!(st.card(ds).counters.virus, 1);
    cmd(&mut st, Side::Runner, Command::Run { server: ServerId::Archives });
    assert_eq!(st.card(ds).counters.virus, 2);
    take_credits(&mut st, Side::Runner);
    take_credits(&mut st, Side::Corp);

    let server = ServerId::from_key(&remote).unwrap();
    let wos = st.server(server).unwrap().ices[0];
    cmd(&mut st, Side::Runner, Command::Run { server });
    click_prompt(&mut st, Side::Corp, "Rez");
    assert_eq!(st.ice_strength(wos), 3);
    cmd(&mut st, Side::Runner, Command::Ability { cid: ds, index: 0 });
    assert_eq!(st.card(ds).counters.virus, 1, "1 counter spent from Datasucker");
    assert_eq!(st.ice_strength(wos), 2, "Wall of Static strength lowered by 1");
    cmd(&mut st, Side::Runner, Command::Continue); // ETR sub fires
    assert!(st.run.is_none());
    assert_eq!(st.ice_strength(wos), 3, "strength restored after the encounter");
    // Remote runs never fed the virus pool.
    assert_eq!(st.card(ds).counters.virus, 1);

    // Corp purges: [click][click][click] clears every virus counter.
    take_credits(&mut st, Side::Runner);
    cmd(&mut st, Side::Corp, Command::Purge);
    assert_eq!(st.card(ds).counters.virus, 0, "purge clears virus counters");
    assert_eq!(st.clicks(Side::Corp), 0, "purge costs three clicks");
}

// ── Corp basic action: trash a resource if the runner is tagged ────────────

#[test]
fn corp_trash_resource_basic_action() {
    let mut st = new_test_game(121, &[], &["Armitage Codebusting"]);
    // Not tagged: the action is illegal.
    assert!(try_cmd(&mut st, Side::Corp, Command::TrashResource).is_err());
    take_credits(&mut st, Side::Corp);
    let arm = runner_install(&mut st, "Armitage Codebusting");
    take_credits(&mut st, Side::Runner);

    st.tags = 1; // test shortcut, like the reference's core/gain :tag
    let credits = st.credits(Side::Corp);
    let clicks = st.clicks(Side::Corp);
    cmd(&mut st, Side::Corp, Command::TrashResource);
    click_card(&mut st, Side::Corp, arm);
    assert!(st.discard(Side::Runner).contains(&arm), "resource trashed");
    assert_eq!(st.credits(Side::Corp), credits - 2, "paid 2 credits");
    assert_eq!(st.clicks(Side::Corp), clicks - 1, "spent a click");
}
