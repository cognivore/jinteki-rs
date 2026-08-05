//! The game timer, end to end: per-side chess clocks and the "rope" fuse
//! system (`jinteki_server::timing`), driven against the real CR session
//! machinery with millisecond-scale configs.
//!
//! Every test builds a headless human-vs-bot session (the same
//! `cr::create_session_with` path the ws door uses, minus the socket),
//! answers prompts through `cr::command_headless` (the same `apply_command`
//! the socket feeds), and advances time by sleeping and calling
//! `cr::timing_tick` — which is exactly what the server's ticker task does,
//! so nothing here exercises a test-only code path.

use jinteki_cr::object::{CardType, PrintedCard, Side};
use jinteki_cr::{cards, GameSetup};
use jinteki_server::cr;
use jinteki_server::timing::{RopeParams, TimingParams};
use serde_json::{json, Value};
use std::time::Duration;

/// Two small decks of cards whose behavior the VM implements (the same shape
/// `cr_mode.rs` uses): the timer is about any game at all, not the eternal
/// decks.
fn small_setup(seed: u64) -> GameSetup {
    let mut corp_deck = Vec::new();
    for _ in 0..6 {
        corp_deck.push(cards::hedge_fund());
        corp_deck.push(cards::ice_wall());
        corp_deck.push(cards::hostile_takeover());
        corp_deck.push(cards::pad_campaign());
    }
    let mut runner_deck = Vec::new();
    for _ in 0..8 {
        runner_deck.push(cards::sure_gamble());
        runner_deck.push(cards::easy_mark());
        runner_deck.push(cards::diesel());
    }
    GameSetup {
        corp_deck,
        runner_deck,
        corp_identity: Some(PrintedCard::vanilla("Test Corp", Side::Corp, CardType::Identity)),
        runner_identity: Some(PrintedCard::vanilla(
            "Test Runner",
            Side::Runner,
            CardType::Identity,
        )),
        additional_identities: Default::default(),
        extra_cards: Default::default(),
        seed,
        shuffle: true,
    }
}

fn rope_ms(action: u64, decision: u64, timeout_fuse: u64) -> TimingParams {
    TimingParams {
        main_clock: None,
        rope: Some(RopeParams {
            action: Duration::from_millis(action),
            decision: Duration::from_millis(decision),
            timeout_fuse: Duration::from_millis(timeout_fuse),
        }),
    }
}

/// A registered human(Runner)-vs-bot session with the given clocks, driven to
/// its first human decision (the mulligan).
async fn timed_game(seed: u64, params: TimingParams) -> cr::Seat {
    let token = cr::create_session_with(small_setup(seed), Side::Runner, 0, params).await;
    let seat = cr::lookup(&token).await.expect("the session is registered");
    {
        let mut g = seat.game.lock().await;
        cr::drive_headless(&mut g).await;
    }
    seat
}

fn state(g: &cr::CrGame) -> Value {
    cr::state_json(g, Side::Runner)
}

fn log_of(st: &Value) -> Vec<String> {
    st["log"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|l| l["text"].as_str().map(|s| s.to_string()))
        .collect()
}

fn log_contains(st: &Value, needle: &str) -> bool {
    log_of(st).iter().any(|l| l.contains(needle))
}

/// Answer the mulligan by keeping (the first choice on the prompt).
async fn keep_hand(g: &mut cr::CrGame) {
    let st = state(g);
    let uuid = st["runner"]["prompt-state"]["choices"][0]["uuid"]
        .as_str()
        .expect("the mulligan prompt is up")
        .to_string();
    cr::command_headless(g, Side::Runner, &json!({"command":"choice","args":{"choice":{"uuid": uuid}}}))
        .await
        .expect("keep resolves");
}

/// Spend the Runner's whole turn on basic credits, inside the fuse, and run
/// on until their NEXT action window (the bot plays the Corp turn between).
async fn clean_credit_turn(g: &mut cr::CrGame) {
    for _ in 0..4 {
        cr::command_headless(g, Side::Runner, &json!({"command":"credit","args":{}}))
            .await
            .expect("the credit action is legal");
    }
}

fn timing_of(st: &Value) -> &Value {
    &st["timing"]
}

// ───────────────────────────────────────────────────────────────────────────
// 1. The main clock
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_flag_is_a_loss_on_time() {
    let seat = timed_game(
        20_260_805,
        TimingParams { main_clock: Some(Duration::from_millis(60)), rope: None },
    )
    .await;
    let mut g = seat.game.lock().await;
    // The Runner owes the mulligan: their clock is running, the Corp's is not.
    let t = state(&g);
    assert_eq!(timing_of(&t)["main"]["running"], json!("runner"));
    assert!(timing_of(&t)["rope"].is_null() || timing_of(&t).get("rope").is_none());
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(cr::timing_tick(&mut g).await, "the flag is an event");
    let t = state(&g);
    assert_eq!(t["winner"], json!("corp"), "the flagged side loses immediately");
    assert_eq!(t["reason"], json!("out of time"));
    assert!(log_contains(&t, "loses on time"), "a clear game-end message: {:#?}", log_of(&t));
    assert!(log_contains(&t, "Game over — corp wins (out of time)"));
    // Over means over: nothing further ticks.
    assert!(!cr::timing_tick(&mut g).await);
}

// ───────────────────────────────────────────────────────────────────────────
// 2. The rope: first pops
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_first_pop_on_the_action_window_plays_the_turn_as_credits() {
    let seat = timed_game(20_260_806, rope_ms(120, 800, 300)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    // The Runner's first action window (turn 2), on the short action fuse.
    let t = state(&g);
    assert!(t["runner"]["prompt-state"].is_null(), "an action window is the board");
    assert_eq!(timing_of(&t)["rope"]["kind"], json!("action"));
    let before_cred = t["runner"]["credit"].as_u64().unwrap();
    let before_turn = t["turn"].as_u64().unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(cr::timing_tick(&mut g).await, "the pop is an event");
    let t = state(&g);
    assert!(
        log_contains(&t, "the rope burned out — the rest of the turn is spent on credits"),
        "{:#?}",
        log_of(&t)
    );
    // All four clicks landed as credits and the turn ended normally: the bot
    // played the Corp turn and the machine is back at the Runner's NEXT window.
    assert_eq!(
        t["runner"]["credit"].as_u64().unwrap(),
        before_cred + 4,
        "four clicks, four credits"
    );
    assert!(t["turn"].as_u64().unwrap() >= before_turn + 2, "the turn passed and came back");
    assert_eq!(t["runner"]["click"], json!(4), "a fresh window, a fresh allotment");
    assert!(t["winner"].is_null(), "a first pop is not a loss");
    assert_eq!(
        timing_of(&t)["rope"]["kind"],
        json!("action"),
        "the next window is on the rope again"
    );
}

#[tokio::test]
async fn a_first_pop_on_a_decision_prompt_gives_the_neutral_answer() {
    // The mulligan is a non-action prompt: its pop must auto-pass with the
    // plan driver's neutral default (KeepHand), not play a turn of credits.
    let seat = timed_game(20_260_807, rope_ms(800, 120, 300)).await;
    let mut g = seat.game.lock().await;
    let t = state(&g);
    assert_eq!(timing_of(&t)["rope"]["kind"], json!("decision"), "a non-action prompt");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(cr::timing_tick(&mut g).await);
    let t = state(&g);
    assert!(log_contains(&t, "the rope burned out — passes"), "{:#?}", log_of(&t));
    assert!(t["winner"].is_null(), "a first pop is not a loss");
    // The neutral answer kept the hand: five cards, and the game moved on to
    // the Runner's action window (the Corp bot's turn played out between).
    assert_eq!(t["runner"]["hand-count"], json!(5), "KeepHand is the neutral mulligan");
    assert!(t["runner"]["prompt-state"].is_null(), "the machine reached the action window");
    assert_eq!(t["active-player"], json!("runner"));
}

// ───────────────────────────────────────────────────────────────────────────
// 3. The rope: the second consecutive pop
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_second_consecutive_pop_loses_the_game() {
    let seat = timed_game(20_260_808, rope_ms(120, 120, 300)).await;
    let mut g = seat.game.lock().await;
    // Pop 1: the mulligan (auto-kept). Nothing was answered by the player.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(cr::timing_tick(&mut g).await);
    assert!(state(&g)["winner"].is_null());
    // Pop 2: the action window that followed, still with no answer between.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(cr::timing_tick(&mut g).await);
    let t = state(&g);
    assert_eq!(t["winner"], json!("corp"), "two consecutive pops are a loss");
    assert_eq!(t["reason"], json!("roped out"));
    assert!(log_contains(&t, "it's game"), "{:#?}", log_of(&t));
}

#[tokio::test]
async fn an_answer_between_pops_breaks_the_chain() {
    let seat = timed_game(20_260_809, rope_ms(120, 120, 300)).await;
    let mut g = seat.game.lock().await;
    // Pop 1: the mulligan.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(cr::timing_tick(&mut g).await);
    // The player answers the action window themselves (one credit)…
    cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
        .await
        .expect("the credit action is legal");
    // …so the next pop is a FIRST pop again: auto-credits, not a loss.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(cr::timing_tick(&mut g).await);
    let t = state(&g);
    assert!(t["winner"].is_null(), "the chain was broken by the answered prompt");
    assert!(log_contains(&t, "the rest of the turn is spent on credits"));
}

// ───────────────────────────────────────────────────────────────────────────
// 4. Timeout tokens: banking, firing, and the streak
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn three_clean_turns_bank_a_timeout() {
    let seat = timed_game(20_260_810, rope_ms(2000, 2000, 500)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(0));
    // Three clean Runner turns (every prompt answered inside the fuse).
    for turn in 0..3 {
        clean_credit_turn(&mut g).await;
        let t = state(&g);
        let want = if turn == 2 { 1 } else { 0 };
        assert_eq!(
            timing_of(&t)["timeouts"],
            json!(want),
            "after {} clean turn(s)",
            turn + 1
        );
    }
}

#[tokio::test]
async fn a_banked_timeout_fires_restarts_the_fuse_and_is_consumed() {
    let seat = timed_game(20_260_811, rope_ms(400, 2000, 900)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    for _ in 0..3 {
        clean_credit_turn(&mut g).await;
    }
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(1), "one ⌛ banked");
    // Let the action-window fuse burn out: the token fires instead of a pop.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(cr::timing_tick(&mut g).await, "the fire is an event");
    let t = state(&g);
    assert!(
        log_contains(&t, "Test Runner used a timeout"),
        "announced by IDENTITY name: {:#?}",
        log_of(&t)
    );
    assert_eq!(timing_of(&t)["timeouts"], json!(0), "the token is consumed");
    let rope = &timing_of(&t)["rope"];
    assert_eq!(rope["kind"], json!("timeout"), "the fuse restarted as a timeout fuse");
    assert_eq!(rope["total_ms"], json!(900), "…at timeout_fuse length");
    assert!(rope["remaining_ms"].as_u64().unwrap() > 0, "…and it is burning");
    assert!(t["winner"].is_null());
    assert!(!log_contains(&t, "the rope burned out"), "a timeout firing is not a pop");
    // The prompt is still on the table and still answerable.
    cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
        .await
        .expect("the window survived the fire");
}

#[tokio::test]
async fn a_timeout_fire_resets_the_clean_turn_streak() {
    let seat = timed_game(20_260_812, rope_ms(400, 2000, 1500)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    // Bank one (three clean turns), then two MORE clean turns: streak 2.
    for _ in 0..5 {
        clean_credit_turn(&mut g).await;
    }
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(1));
    // The sixth turn's window burns out: the ⌛ fires. The documented reading:
    // the rope ran out, so the streak RESETS (this turn is also not clean),
    // though none of a pop's consequences apply.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(cr::timing_tick(&mut g).await);
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(0), "fired and consumed");
    // Finish the fired turn inside the restarted fuse, then two clean turns:
    // if the streak had survived (2 + these) a token would appear — it must not.
    clean_credit_turn(&mut g).await;
    clean_credit_turn(&mut g).await;
    clean_credit_turn(&mut g).await;
    assert_eq!(
        timing_of(&state(&g))["timeouts"],
        json!(0),
        "two clean turns after the fire do not bank: the streak restarted at zero"
    );
    // The third clean turn after the fire banks again.
    clean_credit_turn(&mut g).await;
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(1), "three from scratch bank");
}
