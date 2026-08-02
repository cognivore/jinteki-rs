//! Command processing: turns, economy, installs, plays, scores, prompts.
//! Run-specific logic lives in `runs.rs`.

use crate::runs;
use crate::state::*;
use crate::types::*;

/// Entry point: apply one player command. After any successful mutation the
/// engine auto-advances (checkpoint) until the next human decision point.
pub fn process_command(st: &mut GameState, side: Side, cmd: Command) -> Result<(), EngineError> {
    if st.game_over() && !matches!(cmd, Command::Concede) {
        return Err(EngineError::GameOver);
    }
    match cmd {
        Command::Concede => {
            let name = st.username(side).to_string();
            st.system_log(format!("{name} concedes."));
            st.declare_winner(side.opponent(), "Concede");
            return Ok(());
        }
        Command::Choice { ref uuid } => {
            resolve_choice(st, side, uuid)?;
            checkpoint(st);
            return Ok(());
        }
        Command::Select { cid } => {
            resolve_select(st, side, cid)?;
            checkpoint(st);
            return Ok(());
        }
        _ => {}
    }
    // Any open prompt for this side blocks everything except answering it.
    if st.current_prompt(side).is_some() {
        return Err(EngineError::PromptOpen);
    }

    match cmd {
        Command::Keep | Command::Mulligan => return Err(EngineError::NoPrompt),
        Command::StartTurn => start_turn(st, side)?,
        Command::EndTurn => end_turn(st, side)?,
        Command::Credit => click_for_credit(st, side)?,
        Command::Draw => click_for_draw(st, side)?,
        Command::Play { cid } => play_card(st, side, cid)?,
        Command::InstallCorp { cid, ref server } => install_corp(st, side, cid, server)?,
        Command::InstallRunner { cid } => install_runner(st, side, cid)?,
        Command::Advance { cid } => advance_card(st, side, cid)?,
        Command::Score { cid } => score_agenda(st, side, cid)?,
        Command::Rez { cid } => rez_card(st, side, cid, false)?,
        Command::Run { server } => runs::click_run(st, side, server)?,
        Command::Ability { cid, index } => use_ability(st, side, cid, index)?,
        Command::Continue => runs::continue_run(st, side)?,
        Command::JackOut => runs::jack_out(st, side)?,
        Command::RemoveTag => remove_tag(st, side)?,
        Command::TrashAccessed { .. } => return Err(EngineError::InvalidCommand("trash".into())),
        Command::Choice { .. } | Command::Select { .. } | Command::Concede => unreachable!(),
    }
    checkpoint(st);
    Ok(())
}

/// Auto-advance until the next decision point: run transitions that need no
/// input, breach continuation, win checks.
pub fn checkpoint(st: &mut GameState) {
    st.check_agenda_win();
    for _ in 0..64 {
        if st.game_over() || st.any_prompt_open() {
            return;
        }
        if !runs::advance_auto(st) {
            return;
        }
        st.check_agenda_win();
    }
}

// ── click gating ───────────────────────────────────────────────────────────

fn require_action_window(st: &GameState, side: Side) -> Result<(), EngineError> {
    if st.turn_state != TurnState::Acting || st.active != side {
        return Err(EngineError::NotYourTurn);
    }
    if st.run.is_some() || st.breach.is_some() {
        return Err(EngineError::InvalidCommand("run in progress".into()));
    }
    if st.any_prompt_open() {
        return Err(EngineError::PromptOpen);
    }
    Ok(())
}

fn pay_click_action(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    require_action_window(st, side)?;
    if st.clicks(side) < 1 {
        return Err(EngineError::NoClicks);
    }
    st.spend_click(side, 1);
    Ok(())
}

// ── turn machinery ─────────────────────────────────────────────────────────

fn start_turn(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    if st.turn_state != TurnState::AwaitingStart || st.active != side {
        return Err(EngineError::NotYourTurn);
    }
    if side == Side::Corp {
        st.turn += 1;
    }
    let turn = st.turn;
    let name = st.username(side).to_string();
    st.system_log(format!("{name} started [their] turn {turn}."));
    st.clicks[if side == Side::Corp { 0 } else { 1 }] =
        if side == Side::Corp { 3 } else { 4 };
    st.hq_success_this_turn = false;
    st.turn_state = TurnState::Acting;

    if side == Side::Corp {
        // Start-of-turn drips (PAD Campaign).
        let drippers: Vec<Cid> = st
            .servers
            .iter()
            .flat_map(|(_, s)| s.content.iter().copied())
            .filter(|&cid| {
                let c = st.card(cid);
                c.rezzed && c.def().drip_corp_turn > 0
            })
            .collect();
        for cid in drippers {
            let n = st.card(cid).def().drip_corp_turn as i64;
            let title = st.card(cid).title();
            st.gain_credits(Side::Corp, n);
            st.side_log(Side::Corp, format!("uses {title} to gain {n} [Credits]"));
        }
        // Mandatory draw; corp loses if it cannot draw.
        if st.deck(Side::Corp).is_empty() {
            st.declare_winner(Side::Runner, "Decked");
            return Ok(());
        }
        st.draw_n(Side::Corp, 1);
        st.side_log(Side::Corp, "makes [their] mandatory start of turn draw".into());
    }
    Ok(())
}

fn end_turn(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    require_action_window(st, side)?;
    let max = st.max_hand_size(side);
    if (st.hand(side).len() as i32) > max {
        prompt_discard_down(st, side);
        return Ok(());
    }
    finish_end_turn(st, side);
    Ok(())
}

fn prompt_discard_down(st: &mut GameState, side: Side) {
    let max = st.max_hand_size(side);
    let over = st.hand(side).len() as i32 - max;
    st.prompt_select(
        side,
        format!("Discard down to maximum hand size ({over} more)"),
        SelectKind::OwnHandCard(side),
        PromptContext::DiscardDown,
    );
}

pub(crate) fn finish_end_turn(st: &mut GameState, side: Side) {
    let name = st.username(side).to_string();
    st.system_log(format!("{name} is ending [their] turn."));
    st.clicks[if side == Side::Corp { 0 } else { 1 }] = 0;
    st.active = side.opponent();
    st.turn_state = TurnState::AwaitingStart;
}

// ── basic actions ──────────────────────────────────────────────────────────

fn click_for_credit(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    pay_click_action(st, side)?;
    st.gain_credits(side, 1);
    st.side_log(side, "spends [Click] to gain 1 [Credits]".into());
    Ok(())
}

fn click_for_draw(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    require_action_window(st, side)?;
    if st.clicks(side) < 1 {
        return Err(EngineError::NoClicks);
    }
    if st.deck(side).is_empty() {
        return Err(EngineError::InvalidCommand("deck is empty".into()));
    }
    st.spend_click(side, 1);
    st.draw_n(side, 1);
    st.side_log(side, "spends [Click] to draw 1 card".into());
    Ok(())
}

fn remove_tag(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    if side != Side::Runner {
        return Err(EngineError::InvalidCommand("corp cannot remove tags".into()));
    }
    require_action_window(st, side)?;
    if st.tags == 0 {
        return Err(EngineError::InvalidCommand("no tags".into()));
    }
    if st.clicks(side) < 1 || st.spendable(side) < 2 {
        return Err(EngineError::CantAfford);
    }
    st.spend_click(side, 1);
    st.pay_credits(side, 2);
    st.tags -= 1;
    st.side_log(side, "spends [Click] and 2 [Credits] to remove 1 tag".into());
    Ok(())
}

// ── playing operations and events ──────────────────────────────────────────

fn play_card(st: &mut GameState, side: Side, cid: Cid) -> Result<(), EngineError> {
    require_action_window(st, side)?;
    if !st.hand(side).contains(&cid) {
        return Err(EngineError::InvalidCard);
    }
    let def = st.card(cid).def();
    let is_instant = matches!(def.kind, CardType::Operation | CardType::Event);
    if !is_instant || def.side != side {
        return Err(EngineError::InvalidCard);
    }
    if st.clicks(side) < 1 || st.spendable(side) < def.cost as i64 {
        return Err(EngineError::CantAfford);
    }
    st.spend_click(side, 1);
    st.pay_credits(side, def.cost as i64);
    let title = def.title;
    let cost = def.cost;
    st.side_log(side, format!("spends [Click] and {cost} [Credits] to play {title}"));

    // Weyland BABW: gain 1 when the corp plays a transaction.
    if side == Side::Corp
        && def.is_transaction()
        && st.card(st.identity(Side::Corp)).def().identity_ability
            == Some(IdentityAbility::GainOnTransaction)
    {
        st.gain_credits(Side::Corp, 1);
        let id_title = st.card(st.identity(Side::Corp)).title();
        st.side_log(side, format!("uses {id_title} to gain 1 [Credits]"));
    }

    // The card goes to the discard pile; effects resolve after.
    st.trash(cid, true);

    match def.on_play {
        Some(OnPlay::GainCredits(n)) => {
            st.gain_credits(side, n as i64);
            st.side_log(side, format!("gains {n} [Credits]"));
        }
        Some(OnPlay::Draw(n)) => {
            let drawn = st.draw_n(side, n as usize);
            st.side_log(side, format!("draws {drawn} cards"));
        }
        Some(OnPlay::RunEvent { target, access_bonus, success_credits }) => match target {
            Some(server) => runs::initiate_run(st, server, access_bonus, success_credits, Some(cid)),
            None => {
                let mut labels: Vec<String> =
                    vec!["HQ".into(), "R&D".into(), "Archives".into()];
                for (id, _) in &st.servers {
                    if let ServerId::Remote(_) = id {
                        labels.push(id.display());
                    }
                }
                let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                st.prompt_buttons(
                    side,
                    "Choose a server".into(),
                    &refs,
                    PromptContext::ChooseRunServer { success_credits, access_bonus },
                );
            }
        },
        None => {}
    }
    Ok(())
}

// ── installing ─────────────────────────────────────────────────────────────

fn install_corp(st: &mut GameState, side: Side, cid: Cid, server: &str) -> Result<(), EngineError> {
    if side != Side::Corp {
        return Err(EngineError::InvalidCommand("runner cannot corp-install".into()));
    }
    require_action_window(st, side)?;
    if !st.hand(side).contains(&cid) {
        return Err(EngineError::InvalidCard);
    }
    let def = st.card(cid).def();
    let installable = matches!(
        def.kind,
        CardType::Agenda | CardType::Asset | CardType::Ice | CardType::Upgrade
    );
    if !installable || def.side != Side::Corp {
        return Err(EngineError::InvalidCard);
    }
    if st.clicks(side) < 1 {
        return Err(EngineError::NoClicks);
    }

    // Resolve target server; "New remote" creates one.
    let server_id = if server == "New remote" {
        if def.kind == CardType::Ice {
            // Ice on a brand-new remote is legal.
        }
        let id = ServerId::Remote(st.next_remote);
        st.next_remote += 1;
        st.servers.push((id, Server::default()));
        id
    } else {
        ServerId::from_key(server).ok_or(EngineError::InvalidCommand("bad server".into()))?
    };
    if matches!(def.kind, CardType::Agenda | CardType::Asset) && server_id.is_central() {
        return Err(EngineError::InvalidCommand("agendas/assets go to remotes".into()));
    }
    if st.server(server_id).is_none() {
        return Err(EngineError::InvalidCommand("no such server".into()));
    }

    if def.kind == CardType::Ice {
        // Install cost: 1 credit per ice already protecting the server.
        let ice_count = st.server(server_id).unwrap().ices.len() as i64;
        if st.spendable(side) < ice_count {
            return Err(EngineError::CantAfford);
        }
        st.spend_click(side, 1);
        st.pay_credits(side, ice_count);
        st.card_mut(cid).zone = Zone::InServer { server: server_id, ice: true };
        let hand_i = st.hand[0].iter().position(|&c| c == cid).unwrap();
        st.hand[0].remove(hand_i);
        st.server_mut(server_id).unwrap().ices.push(cid);
        let disp = server_id.display();
        st.side_log(side, format!("installs ice protecting {disp}"));
    } else {
        // One agenda/asset per remote: installing over trashes the old card.
        let existing: Vec<Cid> = st.server(server_id).unwrap().content.clone();
        st.spend_click(side, 1);
        for old in existing {
            let title = if st.card(old).rezzed || st.card(old).faceup {
                st.card(old).title().to_string()
            } else {
                "a card".to_string()
            };
            st.trash(old, st.card(old).rezzed);
            st.side_log(side, format!("trashes {title} to make room"));
        }
        st.card_mut(cid).zone = Zone::InServer { server: server_id, ice: false };
        let hand_i = st.hand[0].iter().position(|&c| c == cid).unwrap();
        st.hand[0].remove(hand_i);
        st.server_mut(server_id).unwrap().content.push(cid);
        let disp = server_id.display();
        st.side_log(side, format!("installs a card in {disp}"));
    }
    Ok(())
}

fn install_runner(st: &mut GameState, side: Side, cid: Cid) -> Result<(), EngineError> {
    if side != Side::Runner {
        return Err(EngineError::InvalidCommand("corp cannot runner-install".into()));
    }
    require_action_window(st, side)?;
    if !st.hand(side).contains(&cid) {
        return Err(EngineError::InvalidCard);
    }
    let def = st.card(cid).def();
    let installable = matches!(
        def.kind,
        CardType::Program | CardType::Hardware | CardType::Resource
    );
    if !installable || def.side != Side::Runner {
        return Err(EngineError::InvalidCard);
    }
    if st.clicks(side) < 1 || st.spendable(side) < def.cost as i64 {
        return Err(EngineError::CantAfford);
    }
    if def.kind == CardType::Program && st.mu_used() + def.mu_cost as i32 > st.mu_limit() {
        return Err(EngineError::InvalidCommand("not enough MU".into()));
    }
    st.spend_click(side, 1);
    st.pay_credits(side, def.cost as i64);
    let hand_i = st.hand[1].iter().position(|&c| c == cid).unwrap();
    st.hand[1].remove(hand_i);
    let c = st.card_mut(cid);
    c.zone = Zone::Rig;
    c.faceup = true;
    c.rezzed = true; // runner installs are active
    c.credits = def.start_credits; // Armitage's 12
    match def.kind {
        CardType::Program => st.rig.programs.push(cid),
        CardType::Hardware => st.rig.hardware.push(cid),
        CardType::Resource => st.rig.resources.push(cid),
        _ => unreachable!(),
    }
    let title = def.title;
    let cost = def.cost;
    st.side_log(side, format!("spends [Click] and {cost} [Credits] to install {title}"));
    Ok(())
}

// ── advancing and scoring ──────────────────────────────────────────────────

fn advance_card(st: &mut GameState, side: Side, cid: Cid) -> Result<(), EngineError> {
    if side != Side::Corp {
        return Err(EngineError::InvalidCommand("only the corp advances".into()));
    }
    require_action_window(st, side)?;
    let advanceable = {
        let c = st.card(cid);
        let installed = matches!(c.zone, Zone::InServer { .. });
        installed && (c.is_agenda() || c.def().advanceable)
    };
    if !advanceable {
        return Err(EngineError::InvalidCard);
    }
    if st.clicks(side) < 1 || st.spendable(side) < 1 {
        return Err(EngineError::CantAfford);
    }
    st.spend_click(side, 1);
    st.pay_credits(side, 1);
    st.card_mut(cid).advancement += 1;
    st.side_log(side, "spends [Click] and 1 [Credits] to advance a card".into());
    Ok(())
}

fn score_agenda(st: &mut GameState, side: Side, cid: Cid) -> Result<(), EngineError> {
    if side != Side::Corp {
        return Err(EngineError::InvalidCommand("only the corp scores".into()));
    }
    require_action_window(st, side)?;
    let (ok, req) = {
        let c = st.card(cid);
        let installed = matches!(c.zone, Zone::InServer { ice: false, .. });
        let req = c.def().advancement_requirement.unwrap_or(u32::MAX);
        (installed && c.is_agenda() && c.advancement >= req, req)
    };
    let _ = req;
    if !ok {
        return Err(EngineError::InvalidCard);
    }
    let title = st.card(cid).title().to_string();
    let points = st.card(cid).def().agenda_points.unwrap_or(0);
    st.to_score_area(cid, Side::Corp);
    st.side_log(side, format!("scores {title} and gains {points} agenda points"));
    resolve_on_score(st, cid);
    st.check_agenda_win();
    Ok(())
}

fn resolve_on_score(st: &mut GameState, cid: Cid) {
    match st.card(cid).def().on_score {
        Some(OnScore::GainCredits(n)) => {
            st.gain_credits(Side::Corp, n as i64);
            st.side_log(Side::Corp, format!("gains {n} [Credits]"));
        }
        Some(OnScore::GainCreditsAndBadPub(n)) => {
            st.gain_credits(Side::Corp, n as i64);
            st.bad_pub += 1;
            st.side_log(
                Side::Corp,
                format!("gains {n} [Credits] and takes 1 bad publicity"),
            );
        }
        Some(OnScore::OptionalRezIceFree) => {
            let any_target = st
                .all_installed_ice()
                .iter()
                .any(|&i| !st.card(i).rezzed);
            if any_target {
                st.prompt_select(
                    Side::Corp,
                    "Choose a piece of ice to rez, ignoring all costs".into(),
                    SelectKind::UnrezzedInstalledIce,
                    PromptContext::PriorityReqRez,
                );
            }
        }
        Some(OnScore::DrawUpTo(_)) => {
            st.prompt_buttons(
                Side::Corp,
                "Draw 2 cards?".into(),
                &["Yes", "No"],
                PromptContext::HubDraw,
            );
        }
        None => {}
    }
}

// ── rezzing ────────────────────────────────────────────────────────────────

pub(crate) fn rez_card(
    st: &mut GameState,
    side: Side,
    cid: Cid,
    free: bool,
) -> Result<(), EngineError> {
    if side != Side::Corp {
        return Err(EngineError::InvalidCommand("only the corp rezzes".into()));
    }
    let c = st.card(cid);
    let installed = matches!(c.zone, Zone::InServer { .. });
    if !installed || c.rezzed || c.is_agenda() {
        return Err(EngineError::InvalidCard);
    }
    let cost = c.def().cost as i64;
    if !free {
        if st.spendable(side) < cost {
            return Err(EngineError::CantAfford);
        }
        st.pay_credits(side, cost);
    }
    let def = st.card(cid).def();
    let start_credits = def.start_credits;
    let title = def.title;
    let c = st.card_mut(cid);
    c.rezzed = true;
    c.faceup = true;
    c.credits = start_credits; // Regolith's 15
    if free {
        st.side_log(side, format!("rezzes {title}, ignoring all costs"));
    } else {
        st.side_log(side, format!("rezzes {title} paying {cost} [Credits]"));
    }
    Ok(())
}

// ── paid abilities on installed cards ──────────────────────────────────────

fn use_ability(st: &mut GameState, side: Side, cid: Cid, index: usize) -> Result<(), EngineError> {
    let def = st.card(cid).def();
    // Breaker abilities are only usable during an encounter.
    if let Some(bd) = def.breaker {
        if side != Side::Runner || !st.rig.programs.contains(&cid) {
            return Err(EngineError::InvalidCard);
        }
        return runs::breaker_ability(st, cid, bd, index);
    }
    // Click abilities (Armitage, Regolith).
    if let Some(ClickAbility::TakeCredits(per)) = def.click_ability {
        if index != 0 || def.side != side {
            return Err(EngineError::InvalidCommand("no such ability".into()));
        }
        require_action_window(st, side)?;
        let active = match side {
            Side::Runner => st.rig.resources.contains(&cid) || st.rig.programs.contains(&cid),
            Side::Corp => {
                matches!(st.card(cid).zone, Zone::InServer { ice: false, .. })
                    && st.card(cid).rezzed
            }
        };
        if !active {
            return Err(EngineError::InvalidCard);
        }
        if st.clicks(side) < 1 {
            return Err(EngineError::NoClicks);
        }
        st.spend_click(side, 1);
        let take = per.min(st.card(cid).credits);
        st.card_mut(cid).credits -= take;
        st.gain_credits(side, take as i64);
        let title = def.title;
        st.side_log(side, format!("spends [Click] to use {title} to gain {take} [Credits]"));
        if st.card(cid).credits == 0 {
            st.trash(cid, true);
            st.side_log(side, format!("trashes {title}"));
        }
        return Ok(());
    }
    Err(EngineError::InvalidCommand("no such ability".into()))
}

// ── prompt resolution ──────────────────────────────────────────────────────

fn resolve_choice(st: &mut GameState, side: Side, uuid: &str) -> Result<(), EngineError> {
    let prompt = st.current_prompt(side).ok_or(EngineError::NoPrompt)?;
    let choice = prompt
        .choices
        .iter()
        .find(|c| c.uuid == uuid)
        .ok_or(EngineError::BadChoice)?;
    let label = choice.label.clone();
    let context = prompt.context.clone();
    st.pop_prompt(side);

    match context {
        PromptContext::Mulligan => {
            let i = if side == Side::Corp { 0 } else { 1 };
            if label == "Mulligan" && !st.mulliganed[i] {
                st.shuffle_hand_into_deck(side);
                st.draw_n(side, 5);
                st.mulliganed[i] = true;
                st.side_log(side, "takes a mulligan".into());
            } else {
                st.side_log(side, "keeps [their] hand".into());
            }
            st.keep[i] = Some(true);
            if st.keep == [Some(true), Some(true)] {
                st.turn_state = TurnState::AwaitingStart;
                st.active = Side::Corp;
            }
        }
        PromptContext::RezApproached { ice } => {
            if label == "Rez" {
                rez_card(st, Side::Corp, ice, false)?;
                runs::begin_encounter(st, ice);
            } else {
                runs::pass_ice(st);
            }
        }
        PromptContext::AccessSteal { cid } => {
            let title = st.card(cid).title().to_string();
            let points = st.card(cid).def().agenda_points.unwrap_or(0);
            st.to_score_area(cid, Side::Runner);
            st.side_log(
                Side::Runner,
                format!("steals {title} and gains {points} agenda points"),
            );
            st.check_agenda_win();
            runs::breach_continue(st);
        }
        PromptContext::AccessTrashOrNo { cid, trash_cost } => {
            if label.starts_with("Pay") {
                if !st.pay_credits(Side::Runner, trash_cost as i64) {
                    return Err(EngineError::CantAfford);
                }
                let title = st.card(cid).title().to_string();
                st.trash(cid, true);
                st.side_log(
                    Side::Runner,
                    format!("pays {trash_cost} [Credits] to trash {title}"),
                );
            }
            runs::breach_continue(st);
        }
        PromptContext::AccessNoAction { cid: _ } => {
            runs::breach_continue(st);
        }
        PromptContext::ChooseRunServer { success_credits, access_bonus } => {
            let server =
                ServerId::from_key(&label).ok_or(EngineError::BadChoice)?;
            if st.server(server).is_none() {
                return Err(EngineError::BadChoice);
            }
            runs::initiate_run(st, server, access_bonus, success_credits, None);
        }
        PromptContext::HubDraw => {
            if label == "Yes" {
                let n = st.draw_n(Side::Corp, 2);
                st.side_log(Side::Corp, format!("draws {n} cards"));
            }
        }
        PromptContext::BreakChooseSub { breaker, ice } => {
            runs::resolve_break_choice(st, breaker, ice, &label)?;
        }
        PromptContext::PriorityReqRez
        | PromptContext::RototurretTrash { .. }
        | PromptContext::DiscardDown => {
            return Err(EngineError::BadChoice); // these are select prompts
        }
    }
    Ok(())
}

fn resolve_select(st: &mut GameState, side: Side, target: Cid) -> Result<(), EngineError> {
    let prompt = st.current_prompt(side).ok_or(EngineError::NoPrompt)?;
    let select = prompt.select.ok_or(EngineError::BadChoice)?;
    let context = prompt.context.clone();

    let valid = match select {
        SelectKind::UnrezzedInstalledIce => {
            st.all_installed_ice().contains(&target) && !st.card(target).rezzed
        }
        SelectKind::InstalledRunnerProgram => st.rig.programs.contains(&target),
        SelectKind::OwnHandCard(s) => st.hand(s).contains(&target),
    };
    if !valid {
        return Err(EngineError::BadChoice);
    }
    st.pop_prompt(side);

    match context {
        PromptContext::PriorityReqRez => {
            rez_card(st, Side::Corp, target, true)?;
        }
        PromptContext::RototurretTrash { ice, resume_index } => {
            let title = st.card(target).title().to_string();
            st.trash(target, true);
            st.side_log(Side::Corp, format!("trashes {title}"));
            runs::fire_subs_from(st, ice, resume_index);
        }
        PromptContext::DiscardDown => {
            let title = st.card(target).title().to_string();
            let seen = side == Side::Runner;
            st.trash(target, seen);
            if side == Side::Runner {
                st.side_log(side, format!("discards {title}"));
            } else {
                st.side_log(side, "discards a card".into());
            }
            let max = st.max_hand_size(side);
            if (st.hand(side).len() as i32) > max {
                prompt_discard_down(st, side);
            } else {
                finish_end_turn(st, side);
            }
        }
        _ => return Err(EngineError::BadChoice),
    }
    Ok(())
}

// ── damage ─────────────────────────────────────────────────────────────────

pub(crate) fn net_damage(st: &mut GameState, n: u32) {
    let hand = st.hand(Side::Runner).clone();
    if (hand.len() as u32) < n {
        st.side_log(Side::Runner, format!("suffers {n} net damage"));
        st.declare_winner(Side::Corp, "Flatline");
        return;
    }
    let victims = st.pick_random(hand, n as usize);
    let mut names = Vec::new();
    for cid in victims {
        names.push(st.card(cid).title().to_string());
        st.trash(cid, true);
    }
    let list = names.join(", ");
    st.side_log(
        Side::Runner,
        format!("suffers {n} net damage, trashing {list}"),
    );
}
