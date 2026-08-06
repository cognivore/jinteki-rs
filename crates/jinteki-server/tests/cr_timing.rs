//! The game timer, end to end: per-side chess clocks and the "rope"
//! reservoir (`jinteki_server::timing`), driven against the real CR session
//! machinery with millisecond-scale configs.
//!
//! Every test builds a headless human-vs-bot session (the same
//! `cr::create_session_with` path the ws door uses, minus the socket),
//! answers prompts through `cr::command_headless` (the same `apply_command`
//! the socket feeds), and advances time by sleeping and calling
//! `cr::timing_tick` — which is exactly what the server's ticker task does,
//! so nothing here exercises a test-only code path.
//!
//! The rope is a BANK, not a per-prompt fuse: it drains only while the game
//! waits on that player, every completed action pays it back, and it is only
//! when the bank is empty that a rope appears and burns.

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

/// A millisecond-scale reservoir: `calm` is the bank's cap, `opening` what
/// both sides start holding, `inc` what one completed action pays back, and
/// `rope` how long the rope burns once the bank is spent.
fn reservoir(calm: u64, opening: u64, inc: u64, rope: u64) -> TimingParams {
    TimingParams {
        main_clock: None,
        rope: Some(RopeParams {
            calm: Duration::from_millis(calm),
            opening_calm: Duration::from_millis(opening),
            action_increment: Duration::from_millis(inc),
            rope: Duration::from_millis(rope),
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

/// Spend the Runner's whole turn on basic credits, inside the bank, and run
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

/// The reservoir as the Runner's screen would read it.
fn rope_of(st: &Value) -> &Value {
    &st["timing"]["rope"]
}

fn bank_ms(st: &Value) -> u64 {
    rope_of(st)["bank_ms"].as_u64().expect("a reservoir is on the wire")
}

fn rope_visible(st: &Value) -> bool {
    rope_of(st)["visible"].as_bool().expect("a reservoir is on the wire")
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
// 2. The reservoir: a player who is PLAYING never meets the rope
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_opening_bank_is_the_opening_calm() {
    let seat = timed_game(20_260_820, reservoir(400, 2000, 200, 300)).await;
    let g = seat.game.lock().await;
    let t = state(&g);
    // The keep/mulligan window opens on the LONG bank, not the steady one.
    assert_eq!(rope_of(&t)["side"], json!("runner"));
    assert!(!rope_visible(&t), "no rope on a fresh game's screen");
    assert!(bank_ms(&t) > 1500, "the opening bank is the opening calm, not the cap: {}", bank_ms(&t));
    assert_eq!(rope_of(&t)["rope_total_ms"], json!(300));
}

#[tokio::test]
async fn a_fast_player_never_sees_a_rope() {
    // A short bank (120ms) and a short rope: only a player who STOPS could
    // reach it. This one keeps acting.
    let seat = timed_game(20_260_821, reservoir(120, 200, 40, 60)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    for turn in 0..3 {
        for click in 0..4 {
            cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
                .await
                .expect("the credit action is legal");
            // Nothing burns, ever: the tick has no rope to find.
            assert!(!cr::timing_tick(&mut g).await, "turn {turn}, click {click}");
            let t = state(&g);
            assert!(!rope_visible(&t), "turn {turn}, click {click}: a rope appeared");
            assert!(bank_ms(&t) > 0, "turn {turn}, click {click}: the bank emptied");
        }
    }
    assert!(state(&g)["winner"].is_null());
}

#[tokio::test]
async fn the_bank_caps_at_the_steady_state() {
    // Opening 2000, cap 120: the first completed action of the game collapses
    // the long opening bank into the steady-state regime (timing.rs documents
    // this cap as the coordinator's inference, retunable in one place).
    let seat = timed_game(20_260_822, reservoir(120, 2000, 40, 60)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    assert!(bank_ms(&state(&g)) > 1500, "the mulligan is not an action: the bank is untouched");
    cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
        .await
        .expect("the credit action is legal");
    let after = bank_ms(&state(&g));
    assert!(after <= 120, "one action caps the bank at the steady state, got {after}");
    assert!(after > 0);
}

// ───────────────────────────────────────────────────────────────────────────
// 3. The rope: it appears when the bank is spent, and an action lifts you off
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_empty_bank_shows_the_rope_and_an_action_lifts_the_player_off_it() {
    // A long rope, so the burn is observable without burning out.
    let seat = timed_game(20_260_823, reservoir(200, 100, 80, 2000)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    assert!(!rope_visible(&state(&g)), "the action window opens calm");
    // Stop playing: the bank drains and the rope appears.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!cr::timing_tick(&mut g).await, "burning is not yet burnt");
    let t = state(&g);
    assert!(rope_visible(&t), "the bank is spent, so the rope is on screen");
    assert_eq!(bank_ms(&t), 0);
    let left = rope_of(&t)["rope_ms_left"].as_u64().unwrap();
    assert!(left > 0 && left < 2000, "the rope is part-burnt: {left}");
    // Acting mid-rope lifts them off it.
    cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
        .await
        .expect("the window is still answerable while the rope burns");
    let t = state(&g);
    assert!(!rope_visible(&t), "the action bought calm time back");
    assert!(bank_ms(&t) > 0);
    assert_eq!(rope_of(&t)["rope_ms_left"], json!(2000), "and a whole rope waits below");
    assert!(state(&g)["winner"].is_null());
}

// ───────────────────────────────────────────────────────────────────────────
// 4. The rope: first burn-outs
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_first_burn_out_on_the_action_window_plays_the_turn_as_credits() {
    let seat = timed_game(20_260_806, reservoir(120, 120, 40, 80)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    // The Runner's first action window (turn 2), on a full opening bank.
    let t = state(&g);
    assert!(t["runner"]["prompt-state"].is_null(), "an action window is the board");
    assert!(!rope_visible(&t));
    let before_cred = t["runner"]["credit"].as_u64().unwrap();
    let before_turn = t["turn"].as_u64().unwrap();
    // Bank (120) then rope (80), and then it is out.
    tokio::time::sleep(Duration::from_millis(260)).await;
    assert!(cr::timing_tick(&mut g).await, "the burn-out is an event");
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
    assert!(t["winner"].is_null(), "a first burn-out is not a loss");
    // The house's credits did NOT buy the bank back: they are still on a rope,
    // a whole one, and playing is the only way off it.
    assert!(rope_visible(&t), "still roped, because they still have not played");
    assert_eq!(bank_ms(&t), 0);
}

#[tokio::test]
async fn a_first_burn_out_on_a_decision_prompt_gives_the_neutral_answer() {
    // The mulligan is a non-action prompt: its burn-out must auto-pass with
    // the plan driver's neutral default (KeepHand), not play a turn of credits.
    let seat = timed_game(20_260_807, reservoir(400, 80, 40, 80)).await;
    let mut g = seat.game.lock().await;
    let t = state(&g);
    assert!(!t["runner"]["prompt-state"].is_null(), "a non-action prompt");
    tokio::time::sleep(Duration::from_millis(220)).await;
    assert!(cr::timing_tick(&mut g).await);
    let t = state(&g);
    assert!(log_contains(&t, "the rope burned out — passes"), "{:#?}", log_of(&t));
    assert!(t["winner"].is_null(), "a first burn-out is not a loss");
    // The neutral answer kept the hand: five cards, and the game moved on to
    // the Runner's action window (the Corp bot's turn played out between).
    assert_eq!(t["runner"]["hand-count"], json!(5), "KeepHand is the neutral mulligan");
    assert!(t["runner"]["prompt-state"].is_null(), "the machine reached the action window");
    assert_eq!(t["active-player"], json!("runner"));
}

// ───────────────────────────────────────────────────────────────────────────
// 5. The rope: the second consecutive burn-out
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_second_consecutive_burn_out_loses_the_game() {
    let seat = timed_game(20_260_808, reservoir(400, 80, 40, 80)).await;
    let mut g = seat.game.lock().await;
    // Burn-out 1: the mulligan (auto-kept). Nothing was answered by the player.
    tokio::time::sleep(Duration::from_millis(220)).await;
    assert!(cr::timing_tick(&mut g).await);
    assert!(state(&g)["winner"].is_null());
    assert!(rope_visible(&state(&g)), "the burn-out leaves them on a relit rope");
    // Burn-out 2: the action window that followed, still with no answer between.
    tokio::time::sleep(Duration::from_millis(140)).await;
    assert!(cr::timing_tick(&mut g).await);
    let t = state(&g);
    assert_eq!(t["winner"], json!("corp"), "two consecutive burn-outs are a loss");
    assert_eq!(t["reason"], json!("roped out"));
    assert!(log_contains(&t, "it's game"), "{:#?}", log_of(&t));
}

#[tokio::test]
async fn an_answer_between_burn_outs_breaks_the_chain() {
    let seat = timed_game(20_260_809, reservoir(400, 80, 60, 80)).await;
    let mut g = seat.game.lock().await;
    // Burn-out 1: the mulligan.
    tokio::time::sleep(Duration::from_millis(220)).await;
    assert!(cr::timing_tick(&mut g).await);
    // The player answers the action window themselves (one credit), which is
    // both what breaks the chain and what buys their calm time back…
    cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
        .await
        .expect("the credit action is legal");
    assert!(!rope_visible(&state(&g)), "playing lifted them off the rope");
    // …so the next burn-out is a FIRST burn-out again: auto-credits, not a loss.
    tokio::time::sleep(Duration::from_millis(220)).await;
    assert!(cr::timing_tick(&mut g).await);
    let t = state(&g);
    assert!(t["winner"].is_null(), "the chain was broken by the answered prompt");
    assert!(log_contains(&t, "the rest of the turn is spent on credits"));
}

// ───────────────────────────────────────────────────────────────────────────
// 6. Timeout tokens: banking, firing, and the streak
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn three_clean_turns_bank_a_timeout() {
    let seat = timed_game(20_260_810, reservoir(2000, 2000, 500, 500)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(0));
    // Three clean Runner turns (every prompt answered out of the bank).
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
async fn a_banked_timeout_fires_restarts_the_rope_and_is_consumed() {
    let seat = timed_game(20_260_811, reservoir(120, 2000, 40, 200)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    for _ in 0..3 {
        clean_credit_turn(&mut g).await;
    }
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(1), "one ⌛ banked");
    // Let the bank and then the rope run out: the token fires instead of a
    // burn-out.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(cr::timing_tick(&mut g).await, "the fire is an event");
    let t = state(&g);
    assert!(
        log_contains(&t, "Test Runner used a timeout"),
        "announced by IDENTITY name: {:#?}",
        log_of(&t)
    );
    assert_eq!(timing_of(&t)["timeouts"], json!(0), "the token is consumed");
    let rope = rope_of(&t);
    assert!(rope_visible(&t), "the bank is still empty, so the rope is still on screen");
    assert_eq!(rope["rope_total_ms"], json!(200), "…at the rope's own length");
    assert!(rope["rope_ms_left"].as_u64().unwrap() > 0, "…restarted, and burning");
    assert!(t["winner"].is_null());
    assert!(!log_contains(&t, "the rope burned out"), "a timeout firing is not a burn-out");
    // The prompt is still on the table and still answerable.
    cr::command_headless(&mut g, Side::Runner, &json!({"command":"credit","args":{}}))
        .await
        .expect("the window survived the fire");
}

#[tokio::test]
async fn a_timeout_fire_resets_the_clean_turn_streak() {
    let seat = timed_game(20_260_812, reservoir(120, 2000, 40, 400)).await;
    let mut g = seat.game.lock().await;
    keep_hand(&mut g).await;
    // Bank one (three clean turns), then two MORE clean turns: streak 2.
    for _ in 0..5 {
        clean_credit_turn(&mut g).await;
    }
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(1));
    // The sixth turn's window burns out: the ⌛ fires. The documented reading:
    // the rope ran out, so the streak RESETS (this turn is also not clean),
    // though none of a burn-out's consequences apply.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(cr::timing_tick(&mut g).await);
    assert_eq!(timing_of(&state(&g))["timeouts"], json!(0), "fired and consumed");
    // Finish the fired turn inside the restarted rope, then two clean turns:
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

// ───────────────────────────────────────────────────────────────────────────
// 7. The decision's identity survives every re-send of it
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_timing_resync_does_not_change_the_decision_stamp() {
    // The client throws away half-finished intents (an armed board target
    // waiting for its confirming second tap) when the QUESTION changes, and
    // `decision-seq` is how it tells that from the once-a-second timing
    // re-push of the same question. If a re-serialization could bump it, the
    // second tap of every two-tap target would be eaten.
    let seat = timed_game(20_260_824, reservoir(400, 2000, 200, 400)).await;
    let mut g = seat.game.lock().await;
    let first = state(&g)["decision-seq"].clone();
    assert!(first.is_u64(), "a decision is on the table");
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!cr::timing_tick(&mut g).await);
        assert_eq!(state(&g)["decision-seq"], first, "re-sending is not re-asking");
    }
    // A genuinely new decision is a new stamp.
    keep_hand(&mut g).await;
    let second = state(&g)["decision-seq"].clone();
    assert!(second.is_u64());
    assert_ne!(second, first, "the next question has its own identity");
}
