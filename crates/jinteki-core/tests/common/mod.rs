//! Test harness: a lite port of the reference test-framework vocabulary
//! (new-game with stacked hands, click-prompt by label, take-credits).

use jinteki_core::state::*;
use jinteki_core::types::*;
use jinteki_core::{process_command, Command};

pub fn cmd(st: &mut GameState, side: Side, c: Command) {
    let dbg = format!("{c:?}");
    process_command(st, side, c).unwrap_or_else(|e| panic!("command failed: {e} ({dbg})"));
}

pub fn try_cmd(st: &mut GameState, side: Side, c: Command) -> Result<(), EngineError> {
    process_command(st, side, c)
}

/// Answer the current prompt for `side` by button label.
pub fn click_prompt(st: &mut GameState, side: Side, label: &str) {
    let p = st
        .current_prompt(side)
        .unwrap_or_else(|| panic!("no prompt open for {side:?}"));
    let uuid = p
        .choices
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| {
            let have: Vec<_> = p.choices.iter().map(|c| c.label.clone()).collect();
            panic!("no choice {label:?}; have {have:?}")
        })
        .uuid
        .clone();
    cmd(st, side, Command::Choice { uuid });
}

/// Answer the current select-prompt for `side` by clicking a card.
pub fn click_card(st: &mut GameState, side: Side, cid: Cid) {
    cmd(st, side, Command::Select { cid });
}

/// Both players keep their opening hands; corp starts turn 1.
pub fn keep_both_and_start(st: &mut GameState) {
    click_prompt(st, Side::Corp, "Keep");
    click_prompt(st, Side::Runner, "Keep");
    cmd(st, Side::Corp, Command::StartTurn);
}

/// Move the named cards from deck into hand (reference "starting-hand" trick):
/// existing hand goes back into the deck first, so tests fully control hands.
pub fn stack_hand(st: &mut GameState, side: Side, titles: &[&str]) {
    st.shuffle_hand_into_deck(side);
    let i = if side == Side::Corp { 0 } else { 1 };
    for t in titles {
        let pos = st.deck[i]
            .iter()
            .position(|&c| st.card(c).title() == *t)
            .unwrap_or_else(|| panic!("{t} not in {side:?} deck"));
        let cid = st.deck[i].remove(pos);
        st.card_mut(cid).zone = Zone::Hand;
        st.hand[i].push(cid);
    }
}

pub fn find_in_hand(st: &GameState, side: Side, title: &str) -> Cid {
    *st.hand(side)
        .iter()
        .find(|&&c| st.card(c).title() == title)
        .unwrap_or_else(|| panic!("{title} not in hand"))
}

/// Spend all remaining clicks on credits, end turn (auto-discarding down to
/// hand size if needed), opponent starts theirs.
pub fn take_credits(st: &mut GameState, side: Side) {
    while st.clicks(side) > 0 {
        cmd(st, side, Command::Credit);
    }
    cmd(st, side, Command::EndTurn);
    while st
        .current_prompt(side)
        .map(|p| p.select.is_some())
        .unwrap_or(false)
    {
        let cid = st.hand(side)[0];
        click_card(st, side, cid);
    }
    cmd(st, side.opponent(), Command::StartTurn);
}

/// Standard test game: default decks, hands stacked as given, corp turn started.
pub fn new_test_game(seed: u64, corp_hand: &[&str], runner_hand: &[&str]) -> GameState {
    let mut st = GameState::new(seed);
    click_prompt(&mut st, Side::Corp, "Keep");
    click_prompt(&mut st, Side::Runner, "Keep");
    stack_hand(&mut st, Side::Corp, corp_hand);
    stack_hand(&mut st, Side::Runner, runner_hand);
    cmd(&mut st, Side::Corp, Command::StartTurn);
    st
}

/// Install a corp card from hand and return its cid.
pub fn corp_install(st: &mut GameState, title: &str, server: &str) -> Cid {
    let cid = find_in_hand(st, Side::Corp, title);
    cmd(st, Side::Corp, Command::InstallCorp { cid, server: server.into() });
    cid
}

pub fn runner_install(st: &mut GameState, title: &str) -> Cid {
    let cid = find_in_hand(st, Side::Runner, title);
    cmd(st, Side::Runner, Command::InstallRunner { cid });
    cid
}

pub fn play(st: &mut GameState, side: Side, title: &str) -> Cid {
    let cid = find_in_hand(st, side, title);
    cmd(st, side, Command::Play { cid });
    cid
}

/// Newest remote's id key (e.g. "remote1").
pub fn newest_remote(st: &GameState) -> String {
    st.servers
        .iter()
        .rev()
        .find(|(id, _)| matches!(id, ServerId::Remote(_)))
        .map(|(id, _)| id.key())
        .expect("no remote")
}

pub fn advance_n(st: &mut GameState, cid: Cid, n: u32) {
    for _ in 0..n {
        cmd(st, Side::Corp, Command::Advance { cid });
    }
}
