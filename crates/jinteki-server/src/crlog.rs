//! The change log, narrated — ONE RENDERING PER VIEWER.
//!
//! `jinteki_cr::change::GameChange` is the kernel's authoritative event
//! stream: every mutation appends one, and each record carries the detail a
//! player wants ("which card did I just access", "what did that subroutine
//! do", "who paid what"). Nothing in the VM ever says any of it out loud —
//! the kernel is sans-IO and stays that way. This module is the whole of the
//! saying: one pure function from a record plus a VIEWER to at most one line
//! of English.
//!
//! **The viewer is not decoration.** A log line is information delivered to a
//! player, and CR §10.2 governs which information a player is entitled to. So
//! every card a line could name goes through [`card_name_for`], which asks
//! `Vm::identity_visible_to` — the same predicate `Vm::view_of` is built from,
//! and the same one that implements 7.1.2's "while the Runner is accessing a
//! card, the Runner is allowed to look at that card". A card the reader may
//! not see is called "a card" and nothing more. That is why the two logs are
//! rendered separately instead of once and copied: an R&D access names the
//! card in the Runner's log and says "a card" in the Corp's, and the same
//! record produces both.
//!
//! Being VAGUE is always safe; being specific is only safe when the kernel
//! says the reader is entitled to it. When in doubt this module is vague.
//!
//! Timeliness matters, and the caller owns it: visibility is evaluated when
//! the line is rendered, not when the record was made, so `CrGame::narrate`
//! runs after every single VM step. For an access that is belt and braces
//! anyway — CR 7.3.1a keeps an accessed card visible to the Runner for the
//! remainder of the breach, which `identity_visible_to` already honours.

use jinteki_cr::change::GameChange;
use jinteki_cr::effects::DamageKind;
use jinteki_cr::object::{CounterKind, ObjectId, ServerId, Side, Zone};
use jinteki_cr::Vm;

use crate::cr::{server_label, side_name};

/// CR 10.2.2b: a card this reader has not been shown is not named to them.
///
/// `Vm::identity_visible_to` is the kernel's own answer — the one `view_of`
/// is derived from — so a log line cannot disagree with the board about what
/// a player may see.
pub fn card_name_for(vm: &Vm, viewer: Side, id: ObjectId) -> String {
    if vm.identity_visible_to(id, viewer) {
        vm.st
            .objects
            .get(&id)
            .map(|o| o.printed.name.to_string())
            .unwrap_or_else(|| "a card".into())
    } else {
        "a card".into()
    }
}

/// CR §4: what a zone is called at the table. Both names of each paired zone
/// are printed on the cards, so both are used.
fn zone_label(z: Zone) -> String {
    match z {
        Zone::Deck(Side::Corp) => "R&D".into(),
        Zone::Deck(Side::Runner) => "the stack".into(),
        Zone::Hand(Side::Corp) => "HQ".into(),
        Zone::Hand(Side::Runner) => "the grip".into(),
        Zone::Discard(Side::Corp) => "Archives".into(),
        Zone::Discard(Side::Runner) => "the heap".into(),
        Zone::ScoreArea(s) => format!("the {}'s score area", side_name(s)),
        Zone::Root(s) => format!("the root of {}", server_label(s)),
        Zone::Ice(s) => format!("the ice protecting {}", server_label(s)),
        Zone::Rig => "the rig".into(),
        Zone::PlayArea(_) => "the play area".into(),
        Zone::Bank => "the bank".into(),
        Zone::SetAside => "the set-aside zone".into(),
        Zone::RemovedFromGame => "outside the game".into(),
        Zone::OutsideGame(_) => "the identity pile".into(),
    }
}

/// CR 4.6.2/4.6.3 + 10.2.3a: WHERE a card is, is open information even where
/// WHAT it is, is not — so an access may always say which server it came out
/// of. The zone is read now, which is why narration is prompt.
fn server_of(vm: &Vm, id: ObjectId) -> Option<ServerId> {
    match vm.st.objects.get(&id)?.zone {
        Zone::Hand(Side::Corp) => Some(ServerId::Hq),
        Zone::Deck(Side::Corp) => Some(ServerId::Rnd),
        Zone::Discard(Side::Corp) => Some(ServerId::Archives),
        Zone::Root(s) | Zone::Ice(s) => Some(s),
        _ => None,
    }
}

/// Where an installed card ended up, as a clause. Empty for the rig, which
/// 4.6.5c gives no location of its own.
fn installed_clause(vm: &Vm, id: ObjectId) -> String {
    match vm.st.objects.get(&id).map(|o| o.zone) {
        Some(Zone::Ice(s)) => format!(" protecting {}", server_label(s)),
        Some(Zone::Root(s)) => format!(" in {}", server_label(s)),
        _ => String::new(),
    }
}

fn damage_word(k: DamageKind) -> &'static str {
    match k {
        DamageKind::Meat => "meat",
        DamageKind::Net => "net",
        DamageKind::Core => "core",
    }
}

fn counter_word(k: CounterKind) -> &'static str {
    match k {
        CounterKind::Credit => "credit",
        CounterKind::Power => "power",
        CounterKind::Virus => "virus",
        CounterKind::Agenda => "agenda",
        CounterKind::Advancement => "advancement",
        CounterKind::BadPublicity => "bad publicity",
    }
}

fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        one.into()
    } else {
        many.into()
    }
}

/// The subroutine's printed label, if the ice still has it — 9.8.3's printed
/// subroutines and any a grant added, in the order `index` counts.
fn sub_label(vm: &Vm, ice: ObjectId, index: usize) -> Option<&'static str> {
    vm.current_subs(ice).get(index).map(|(_, d)| d.label)
}

/// The line's SPEAKER — "Corp: " on its own, or "Corp: Predictive Planogram —"
/// where a card is what did it.
///
/// One shape, everywhere: `<Side>: <Card> — <effect>`. The log used to say
/// only the effect ("Corp: gains 1[c]."), which is the same sentence whether
/// the credit came from a click, from Hedge Fund or from an identity that
/// flipped — a reader could see WHAT the game did and never WHY. The change
/// records already carry `source` (9.1.3: the ability's source object); this
/// reads it back.
///
/// CR 10.2.2b still governs: a source this viewer may not see is not named,
/// and the line falls back to the bare speaker rather than saying "a card did
/// something" — the vaguer line is the one that already existed and it leaks
/// nothing. `card_name_for` is the same entitlement predicate the rest of the
/// module uses.
///
/// The em dash is the shape [`abilityText`](../../ui/app.js) already renders
/// for "this card — this ability", so a log line and a rail chip about the
/// same ability read the same way.
fn did(vm: &Vm, viewer: Side, side: Side, src: Option<ObjectId>) -> String {
    let named = src.filter(|id| vm.identity_visible_to(*id, viewer)).and_then(|id| {
        vm.st.objects.get(&id).map(|o| o.printed.name)
    });
    match named {
        Some(n) => format!("{}: {n} —", side_name(side)),
        None => format!("{}:", side_name(side)),
    }
}

/// ONE record, as ONE viewer is entitled to read it. `None` is "this record
/// is bookkeeping, not news" — the CR records a great deal that no player
/// would say out loud (every cost of zero, every click spent, every internal
/// move behind a trash that is already being reported).
///
/// The actions a player CHOOSES are logged by the adapter as they are taken
/// (`cr::describe_move` / `cr::apply_command`); this is what the game did in
/// answer, which is the half that was missing.
pub fn narrate(vm: &Vm, c: &GameChange, viewer: Side) -> Option<String> {
    let card = |id: ObjectId| card_name_for(vm, viewer, id);
    let from = |id: ObjectId| match server_of(vm, id) {
        Some(s) => format!(" from {}", server_label(s)),
        None => String::new(),
    };
    // "<Side>: <Card> — <what it did>" (see `did`). The speaker alone, for a
    // change no card is responsible for.
    let by = |side: Side, src: Option<ObjectId>| did(vm, viewer, side, src);
    let line = match c {
        GameChange::GameBegan => "The game begins.".to_string(),
        GameChange::TurnBegan { side } => {
            format!("— {}'s turn {} —", side_name(*side), vm.st.turn_seq)
        }

        // ── credits, clicks, costs ─────────────────────────────────────
        GameChange::CreditsGained { side, amount, source } => {
            format!("{} gains {amount}[c].", by(*side, *source))
        }
        GameChange::CreditsLost { side, amount, source } => {
            format!("{} loses {amount}[c].", by(*side, *source))
        }
        GameChange::ClicksGained { side, amount } => {
            format!("{}: gains {amount}[click].", side_name(*side))
        }
        GameChange::ClicksLost { side, amount } => {
            format!("{}: loses {amount}[click].", side_name(*side))
        }
        // CR 1.16.3: paying credits is the ONLY record of the spend — the
        // pool is decremented inside the payment, without a `CreditsLost`.
        // The trashed and forfeited parts of a cost report themselves.
        GameChange::CostPaid { side, credits, source, .. } if *credits > 0 => {
            format!("{} pays {credits}[c].", by(*side, *source))
        }

        // ── what a card's own text did (9.11.4g) ───────────────────────
        // The chosen mode of a modal card. Everything else in this match
        // reports a change to the game state; this reports a DECISION, which
        // is the only part of a modal card the state never shows. The label
        // is the option exactly as it was offered, so the line can be read
        // against the prompt the player answered.
        GameChange::OptionChosen { source, side, label } => {
            format!("{} resolves {}.", by(*side, Some(*source)), label.trim_end_matches('.'))
        }

        // ── cards moving ───────────────────────────────────────────────
        GameChange::CardDrawn { side, obj, source } => {
            format!("{} draws {}.", by(*side, *source), card(*obj))
        }
        GameChange::CardPlayed { obj, side } => {
            format!("{}: plays {}.", side_name(*side), card(*obj))
        }
        GameChange::CardInstalled { obj, side, .. } => format!(
            "{}: installs {}{}.",
            side_name(*side),
            card(*obj),
            installed_clause(vm, *obj)
        ),
        GameChange::CardTrashed { obj, by, was_zone, .. } => format!(
            "{}: trashes {} from {}.",
            side_name(*by),
            card(*obj),
            zone_label(*was_zone)
        ),
        GameChange::CardDiscarded { obj, side, .. } => {
            format!("{}: discards {}.", side_name(*side), card(*obj))
        }
        // Every other move is already reported by the record that caused it
        // (a trash, an install, a play, a draw). These three are not.
        GameChange::CardMoved { obj, from, to } => match (from, to) {
            (Zone::Deck(_), Zone::Hand(_)) => return None,
            (_, Zone::Deck(s)) => format!(
                "{}: adds {} to {}.",
                side_name(*s),
                card(*obj),
                zone_label(*to)
            ),
            (_, Zone::Hand(s)) => format!(
                "{}: adds {} to {}.",
                side_name(*s),
                card(*obj),
                zone_label(*to)
            ),
            (_, Zone::RemovedFromGame) => {
                format!("{} is removed from the game.", card(*obj))
            }
            _ => return None,
        },
        GameChange::CardHosted { obj, host } => {
            format!("{} is hosted on {}.", card(*obj), card(*host))
        }

        // ── faces turned ───────────────────────────────────────────────
        GameChange::CardRezzed { obj, .. } => format!("Corp: rezzes {}.", card(*obj)),
        GameChange::CardDerezzed { obj } => format!("Corp: derezzes {}.", card(*obj)),
        // CR 1.21.3: revealing shows the front face to ALL players, so both
        // logs name it — and 1.21.4's expose is a reveal with a restriction.
        GameChange::CardRevealed { obj, .. } => format!("{} is revealed.", card(*obj)),
        GameChange::CardExposed { obj } => format!("Runner: exposes {}.", card(*obj)),
        // CR 1.21.2: looking shows the front face to ONE player. Only that
        // player's line names it; the other's cannot, and does not.
        GameChange::CardLookedAt { obj, by } => {
            format!("{}: looks at {}.", side_name(*by), card(*obj))
        }
        GameChange::IdentityFlipped { side } => {
            format!("{}: turns their identity over.", side_name(*side))
        }
        // CR 10.2.2a: the set is open, the face is not — the line says
        // exactly what both players are entitled to know.
        GameChange::IdentityFaceSecretlySet { side } => {
            format!("{}: secretly sets their identity.", side_name(*side))
        }

        // ── counters ───────────────────────────────────────────────────
        GameChange::CounterPlaced { obj, kind, amount } => format!(
            "{amount} {} {} placed on {}.",
            counter_word(*kind),
            plural(*amount, "counter is", "counters are"),
            card(*obj)
        ),
        GameChange::CounterRemoved { obj: Some(o), kind, amount } if *amount > 0 => format!(
            "{amount} {} {} removed from {}.",
            counter_word(*kind),
            plural(*amount, "counter is", "counters are"),
            card(*o)
        ),
        GameChange::VirusCountersPurged => "Corp: purges virus counters.".to_string(),

        // ── the Runner's condition ─────────────────────────────────────
        GameChange::DamageSuffered { kind, amount, cards, .. } => {
            let trashed: Vec<String> = cards.iter().map(|c| card(*c)).collect();
            if trashed.is_empty() {
                format!("Runner: suffers {amount} {} damage.", damage_word(*kind))
            } else {
                format!(
                    "Runner: suffers {amount} {} damage, trashing {}.",
                    damage_word(*kind),
                    trashed.join(", ")
                )
            }
        }
        GameChange::DamagePrevented { by, kind, amount } => format!(
            "{} prevents {amount} {} damage.",
            card(*by),
            damage_word(*kind)
        ),
        GameChange::TagsTaken { amount, .. } => {
            format!("Runner: takes {amount} {}.", plural(*amount, "tag", "tags"))
        }
        GameChange::TagRemoved => "Runner: removes 1 tag.".to_string(),
        GameChange::TagsAvoided { amount } => {
            format!("Runner: avoids {amount} {}.", plural(*amount, "tag", "tags"))
        }
        GameChange::BadPublicityTaken { side, amount } => format!(
            "{}: takes {amount} bad publicity.",
            side_name(*side)
        ),

        // ── agendas ────────────────────────────────────────────────────
        GameChange::AgendaScored { obj, points } => format!(
            "Corp: scores {} ({points} agenda {}).",
            card(*obj),
            plural(points.unsigned_abs(), "point", "points")
        ),
        GameChange::AgendaStolen { obj, points } => format!(
            "Runner: steals {} ({points} agenda {}).",
            card(*obj),
            plural(points.unsigned_abs(), "point", "points")
        ),
        GameChange::AgendaForfeited { obj, by } => {
            format!("{}: forfeits {}.", side_name(*by), card(*obj))
        }

        // ── the run ────────────────────────────────────────────────────
        //
        // The headline, and the one record whose entitlement belongs to the
        // EVENT rather than to the moment the line is read.
        //
        // CR 7.1.2: "While the Runner is accessing a card, the Runner is
        // allowed to look at that card, even if it would normally not be
        // visible to them." An access is over in a step or two — 7.3.1a keeps
        // the card visible for the rest of the breach and no longer — while
        // the LINE about it stays in the log for the rest of the game. Asking
        // `identity_visible_to` here would therefore hand the Runner "you
        // accessed a card" thirty seconds after they accessed it, which is
        // the complaint this whole change answers. The Runner saw the card;
        // their own log says which.
        //
        // The Corp's copy is the ordinary question, and 4.2.2 answers it: the
        // Corp reads an access out of HQ, Archives or a remote, because those
        // cards are already theirs to see, and reads "a card" out of R&D,
        // which is hidden from them as much as from the Runner. Which server
        // it came out of is open information either way (4.6.2/10.2.3a).
        GameChange::CardAccessed { obj } => {
            let name = match viewer {
                Side::Runner => vm
                    .st
                    .objects
                    .get(obj)
                    .map(|o| o.printed.name.to_string())
                    .unwrap_or_else(|| "a card".into()),
                Side::Corp => card(*obj),
            };
            format!("Runner: accesses {name}{}.", from(*obj))
        }
        GameChange::RunBegan { server } => {
            format!("Runner: runs {}.", server_label(*server))
        }
        GameChange::RunDeclaredSuccessful { server, .. } => {
            format!("The run on {} is successful.", server_label(*server))
        }
        GameChange::RunDeclaredUnsuccessful { server } => {
            format!("The run on {} is unsuccessful.", server_label(*server))
        }
        GameChange::RunEnded { server, .. } => {
            format!("The run on {} ends.", server_label(*server))
        }
        GameChange::BreachBegan { server } => {
            format!("Runner: breaches {}.", server_label(*server))
        }
        GameChange::ServerApproached { server } => {
            format!("Runner: approaches {}.", server_label(*server))
        }
        GameChange::IceApproached { ice } => {
            format!("Runner: approaches {}.", card(*ice))
        }
        GameChange::EncounterBegan { ice, .. } => {
            format!("Runner: encounters {}.", card(*ice))
        }
        GameChange::IcePassed { ice, .. } => format!("Runner: passes {}.", card(*ice)),
        GameChange::SubroutineBroken { ice, .. } => {
            format!("Runner: breaks a subroutine on {}.", card(*ice))
        }
        GameChange::SubroutineResolved { ice, index } => match sub_label(vm, *ice, *index) {
            Some(l) => format!("{}: [sub] {l}", card(*ice)),
            None => format!("A subroutine on {} resolves.", card(*ice)),
        },

        // ── searching, shuffling, tracing ──────────────────────────────
        GameChange::DeckShuffled { side } => {
            format!("{}: shuffles {}.", side_name(*side), zone_label(Zone::Deck(*side)))
        }
        GameChange::ZoneSearched { by, zone } => {
            format!("{}: searches {}.", side_name(*by), zone_label(*zone))
        }
        GameChange::TraceInitiated { base } => {
            format!("Corp: initiates a trace with a base strength of {base}.")
        }
        GameChange::TraceDetermined { success, trace_strength, link_strength } => format!(
            "Trace {trace_strength} vs link {link_strength} — the trace {}.",
            if *success { "succeeds" } else { "fails" }
        ),
        GameChange::SecretlySpentCreditsRevealed { corp, runner } => format!(
            "Secretly spent credits are revealed: the Corp spent {corp}, the Runner spent {runner}."
        ),

        // Bookkeeping. The CR records these so conditions can be met; no
        // player says them out loud, and a log that did would bury the ones
        // above. The action a player TOOK is logged where it is taken.
        GameChange::CostPaid { .. }
        | GameChange::CounterRemoved { .. }
        | GameChange::ActionTaken { .. }
        | GameChange::ActionCompleted { .. }
        | GameChange::ActionPhaseEnded { .. }
        | GameChange::ClickSpent { .. }
        | GameChange::TurnEnded { .. }
        | GameChange::CardPlayResolved { .. }
        | GameChange::CardUninstalled { .. }
        | GameChange::CardEnteredRoot { .. }
        // The install that created the server is already logged; the server
        // coming into existence is bookkeeping around it and adds no line.
        | GameChange::RemoteServerCreated { .. }
        | GameChange::CardAdvanced { .. }
        | GameChange::AbilityUsed { .. }
        | GameChange::TrashAbilityUsed { .. }
        | GameChange::AccessEnded { .. }
        | GameChange::AllSubsBroken { .. }
        | GameChange::EncounterEnded { .. }
        // The movements it attributes are already logged by their CardMoved
        // lines ("… is removed from the game."); this record exists so a
        // "removed a card from the game WITH IT this turn" sentence can ask
        // whose ability removed.
        | GameChange::CardsRemovedFromGame { .. }
        | GameChange::BreachEnded { .. } => return None,
    };
    Some(line)
}


#[cfg(test)]
mod tests {
    use super::*;
    use jinteki_cr::object::{CardType, PrintedCard};
    use jinteki_cr::plan::{Match, Plan, Reply, Script};
    use jinteki_cr::{cards, GameSetup};

    /// A real game of implemented cards: both players draw real opening
    /// hands out of hidden decks, so the visibility question is live from
    /// the very first record.
    fn setup(corp_deck: Vec<PrintedCard>, seed: u64) -> GameSetup {
        let mut runner_deck = Vec::new();
        for _ in 0..10 {
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

    fn mixed_corp_deck() -> Vec<PrintedCard> {
        let mut d = Vec::new();
        for _ in 0..8 {
            d.push(cards::hedge_fund());
            d.push(cards::ice_wall());
            d.push(cards::hostile_takeover());
            d.push(cards::pad_campaign());
        }
        d
    }

    /// The cards ONE record is about — its own subjects, and the only cards
    /// it has any business naming.
    fn subjects(c: &GameChange) -> Vec<ObjectId> {
        match c {
            GameChange::CardDrawn { obj, .. }
            | GameChange::CardTrashed { obj, .. }
            | GameChange::CardDiscarded { obj, .. }
            | GameChange::CardInstalled { obj, .. }
            | GameChange::CardRevealed { obj, .. }
            | GameChange::CardLookedAt { obj, .. }
            | GameChange::CardExposed { obj }
            | GameChange::CardPlayed { obj, .. }
            | GameChange::CardPlayResolved { obj, .. }
            | GameChange::CardUninstalled { obj, .. }
            | GameChange::CardRezzed { obj, .. }
            | GameChange::CardDerezzed { obj }
            | GameChange::CardMoved { obj, .. }
            | GameChange::CardAdvanced { obj }
            | GameChange::CounterPlaced { obj, .. }
            | GameChange::CardEnteredRoot { obj, .. }
            | GameChange::AgendaScored { obj, .. }
            | GameChange::AgendaForfeited { obj, .. }
            | GameChange::AgendaStolen { obj, .. }
            | GameChange::CardAccessed { obj }
            | GameChange::AccessEnded { obj }
            | GameChange::AbilityUsed { source: obj }
            | GameChange::TrashAbilityUsed { source: obj, .. }
            // 9.11.4g: the chosen option is ABOUT the card that offered it,
            // so the same entitlement governs naming it.
            | GameChange::OptionChosen { source: obj, .. }
            | GameChange::DamagePrevented { by: obj, .. } => vec![*obj],
            GameChange::CardHosted { obj, host } => vec![*obj, *host],
            GameChange::CounterRemoved { obj, .. } => obj.iter().copied().collect(),
            GameChange::DamageSuffered { cards, .. } => cards.clone(),
            GameChange::CostPaid { trashed, .. } => trashed.clone(),
            GameChange::EncounterBegan { ice, .. }
            | GameChange::EncounterEnded { ice, .. }
            | GameChange::IceApproached { ice }
            | GameChange::IcePassed { ice, .. }
            | GameChange::SubroutineResolved { ice, .. }
            | GameChange::SubroutineBroken { ice, .. } => vec![*ice],
            // 6.5.7b's second fully-breaker is a subject of this record too:
            // the occurrence is about the object that broke as much as about
            // the ice it broke.
            GameChange::AllSubsBroken { ice, by } => {
                std::iter::once(*ice).chain(by.iter().copied()).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The invariant, stated over a stretch of the log: a line handed to a
    /// reader names no card that reader is not entitled to see (CR 10.2.2b).
    ///
    /// Two passes, because a printed title is not one card. The first is
    /// exact — every card the record is ABOUT. The second is the catch-all:
    /// a title of which this reader can see NO copy anywhere may not appear
    /// in their log at all, whatever the line thought it was naming.
    fn assert_no_leaks(vm: &Vm, from: usize) {
        for c in &vm.changes.log[from..] {
            for viewer in [Side::Corp, Side::Runner] {
                let Some(line) = narrate(vm, c, viewer) else { continue };
                // CR 7.1.2 is an entitlement of the ACCESS, not of the state
                // this line is being read in: the Runner looked at the card
                // they accessed, and their own log may go on saying so after
                // the access is over. It is the only such record.
                if viewer == Side::Runner && matches!(c, GameChange::CardAccessed { .. }) {
                    continue;
                }
                for id in subjects(c) {
                    if vm.identity_visible_to(id, viewer) {
                        continue;
                    }
                    let title = vm.st.objects[&id].printed.name;
                    assert!(
                        !line.contains(title),
                        "the {viewer:?} log names {title:?}, the subject of a record \
                         they may not see: {line:?} (from {c:?})"
                    );
                }
                for o in vm.st.objects.values() {
                    let title = o.printed.name;
                    let anywhere = vm
                        .st
                        .objects
                        .values()
                        .any(|p| p.printed.name == title && vm.identity_visible_to(p.id, viewer));
                    assert!(
                        anywhere || !line.contains(title),
                        "the {viewer:?} log names {title:?}, of which they can see no copy: \
                         {line:?} (from {c:?})"
                    );
                }
            }
        }
    }

    /// A card the reader may not see is not named to them, and the SAME
    /// record names it to the reader who may. The opening draw is the
    /// cleanest case: 4.3.2 gives a player their own hand and denies them
    /// their opponent's, so one `CardDrawn` has two renderings.
    #[test]
    fn a_drawn_card_is_named_to_its_drawer_and_to_nobody_else() {
        let mut vm = Vm::new_game(setup(mixed_corp_deck(), 20_260_804));
        // Both players spend their turn drawing, so both sides of the rule
        // are exercised by the same log.
        let mut s = Script::new(
            Plan::corp().when(Match::action().times(3), Reply::draw()).stop_at_action(),
            Plan::runner().when(Match::action().times(2), Reply::draw()).stop_at_action(),
        );
        s.run(&mut vm);
        assert_no_leaks(&vm, 0);

        let mut checked = [0usize; 2];
        for c in vm.changes.log.clone() {
            let GameChange::CardDrawn { side, obj, .. } = c else { continue };
            // Only while it is still in hand — a card played since is open
            // information to everyone, and says nothing about the rule.
            if vm.st.objects[&obj].zone != Zone::Hand(side) {
                continue;
            }
            let title = vm.st.objects[&obj].printed.name;
            let mine = narrate(&vm, &c, side).expect("a draw is news");
            let theirs = narrate(&vm, &c, side.other()).expect("a draw is news to both");
            assert!(mine.contains(title), "the drawer reads the title: {mine:?}");
            assert!(
                !theirs.contains(title),
                "4.3.2: the opponent's hand is not theirs to read: {theirs:?}"
            );
            assert!(theirs.contains("a card"), "and is told so plainly: {theirs:?}");
            checked[if side == Side::Corp { 0 } else { 1 }] += 1;
        }
        assert!(checked[0] >= 3, "the Corp's draws were checked ({checked:?})");
        assert!(checked[1] >= 2, "the Runner's draws were checked ({checked:?})");
    }

    /// The complaint, as a test: "I don't see what I'm accessing." An R&D
    /// access is the sharp case — CR 7.1.2 entitles the RUNNER to look at the
    /// card being accessed, and 4.2.2 keeps R&D hidden from the Corp as much
    /// as from the Runner, so one record must produce two different
    /// sentences. The plan halts INSIDE the access (the mid-access window a
    /// trashable asset opens), which is where the adapter renders it.
    #[test]
    fn an_rnd_access_is_named_to_the_runner_and_never_to_the_corp() {
        // A deck of trashable assets, so the access is guaranteed to reach a
        // mid-access window whatever the shuffle gives.
        let corp_deck = std::iter::repeat_with(cards::pad_campaign).take(20).collect();
        let mut vm = Vm::new_game(setup(corp_deck, 4_242));
        let mut s = Script::new(
            Plan::corp().otherwise_click_credit(),
            Plan::runner()
                .runs(ServerId::Rnd)
                .when(Match::mid_access(), Reply::Halt)
                .stop_at_action(),
        );
        s.run(&mut vm);

        let accessed = vm.st.accessed.expect("the plan halted mid-access");
        let title = vm.st.objects[&accessed].printed.name;
        let rec = GameChange::CardAccessed { obj: accessed };
        let runner = narrate(&vm, &rec, Side::Runner).expect("an access is news");
        let corp = narrate(&vm, &rec, Side::Corp).expect("an access is news to the Corp too");

        assert!(
            runner.contains(title),
            "7.1.2: the Runner may look at the card they are accessing: {runner:?}"
        );
        assert!(
            runner.contains("from R&D"),
            "and is told which server it came out of: {runner:?}"
        );
        assert!(
            !vm.identity_visible_to(accessed, Side::Corp),
            "4.2.2: R&D is hidden from the Corp too"
        );
        assert!(!corp.contains(title), "so the Corp's line does not name it: {corp:?}");
        assert!(corp.contains("a card"), "it says so plainly: {corp:?}");
        assert!(corp.contains("from R&D"), "location is open information: {corp:?}");
        assert_no_leaks(&vm, 0);

        // And it still reads that way AFTER the access is over, which is the
        // case a live game actually produces: several records can land
        // between two renderings, and 7.1.2's entitlement is measured in
        // steps while the line stays in the log for the rest of the game.
        let mut s = Script::new(Plan::corp().stop_at_action(), Plan::runner().stop_at_action());
        s.run(&mut vm);
        assert!(vm.st.accessed.is_none(), "the access has ended");
        assert!(
            !vm.identity_visible_to(accessed, Side::Runner),
            "and 7.3.1a's sighting has lapsed with the breach"
        );
        let after = narrate(&vm, &rec, Side::Runner).expect("still news");
        assert!(
            after.contains(title),
            "the Runner saw the card; their log goes on saying which: {after:?}"
        );
        assert_eq!(
            narrate(&vm, &rec, Side::Corp).unwrap(),
            corp,
            "and the Corp's copy is unchanged by any of it"
        );
    }

    /// The blunt instrument: a few turns of a real game with a real run, and
    /// no line in either log ever names a card its reader may not see.
    #[test]
    fn no_line_in_either_log_ever_names_a_card_its_reader_may_not_see() {
        let mut vm = Vm::new_game(setup(mixed_corp_deck(), 777));
        let mut s = Script::new(
            Plan::corp().otherwise_click_credit(),
            Plan::runner()
                .runs(ServerId::Hq)
                .when(Match::action().nth(4), Reply::Halt)
                .otherwise_click_credit(),
        );
        s.run(&mut vm);
        assert!(
            vm.changes.log.len() > 50,
            "a few turns produced a real log ({} records)",
            vm.changes.log.len()
        );
        assert_no_leaks(&vm, 0);
    }

    /// The log says WHAT A CARD DID, in one shape: `<Side>: <Card> — <effect>`.
    ///
    /// A real modal card, played for real. Before this, the whole of
    /// Predictive Planogram's turn read "Corp: plays Predictive Planogram." /
    /// "Corp: gains 3[c]." — the same second line a click for credit writes,
    /// with nothing saying which of the card's two modes the Corp had chosen.
    /// Now the choice is a line of its own and the effect carries the card
    /// that caused it, so the decision is auditable from the log alone.
    #[test]
    fn a_modal_cards_log_names_the_card_and_the_mode_it_resolved() {
        use jinteki_cr::object::Zone;
        use jinteki_cr::plan::Kind;

        let mut vm = Vm::empty(4472);
        let pp = vm.new_object(
            jinteki_cards::find("Predictive Planogram").unwrap().printed,
            Zone::Hand(Side::Corp),
        );
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(pp);
        for _ in 0..8 {
            let c = vm.new_object(cards::hedge_fund(), Zone::Deck(Side::Corp));
            vm.st.deck.get_mut(&Side::Corp).unwrap().push(c);
        }
        for _ in 0..3 {
            let c = vm.new_object(cards::sure_gamble(), Zone::Deck(Side::Runner));
            vm.st.deck.get_mut(&Side::Runner).unwrap().push(c);
        }
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let from = vm.changes.log.len();
        let mut s = Script::new(
            Plan::corp()
                .when(Match::action().once(), Reply::play_card(pp))
                .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("Draw 3 cards."))
                .when(Match::action().once(), Reply::Halt),
            Plan::runner(),
        );
        s.run(&mut vm);

        let corp: Vec<String> = vm.changes.log[from..]
            .iter()
            .filter_map(|c| narrate(&vm, c, Side::Corp))
            .collect();
        let joined = corp.join("\n");
        assert!(
            corp.iter().any(|l| l == "Corp: Predictive Planogram — resolves Draw 3 cards."),
            "the chosen MODE is named, by the card that offered it:\n{joined}"
        );
        assert_eq!(
            corp.iter().filter(|l| l.starts_with("Corp: Predictive Planogram — draws ")).count(),
            3,
            "…and each card it drew is attributed to it:\n{joined}"
        );
        // The rules' own draw keeps its bare form: 5.6.2b's mandatory draw is
        // nobody's card, and a line claiming it was would be a lie.
        assert!(
            corp.iter().any(|l| l.starts_with("Corp: draws ")),
            "the mandatory draw stays unattributed:\n{joined}"
        );
        assert_no_leaks(&vm, from);
    }

    /// The other half of the same rule: a change the RULES caused carries no
    /// card, so the basic action's own credit reads exactly as it always did.
    #[test]
    fn a_basic_action_is_not_attributed_to_any_card() {
        let mut vm = Vm::new_game(setup(mixed_corp_deck(), 4473));
        let from = vm.changes.log.len();
        let mut s = Script::new(
            Plan::corp().when(Match::action().nth(2), Reply::Halt).otherwise_click_credit(),
            Plan::runner().otherwise_click_credit(),
        );
        s.run(&mut vm);
        let lines: Vec<String> = vm.changes.log[from..]
            .iter()
            .filter_map(|c| narrate(&vm, c, Side::Corp))
            .collect();
        assert!(
            lines.iter().any(|l| l == "Corp: gains 1[c]."),
            "a click for credit is the rules', not a card's: {lines:?}"
        );
        assert!(
            !lines.iter().any(|l| l.starts_with("Corp: Test Corp —")),
            "and it is not blamed on the identity that happened to be out: {lines:?}"
        );
    }
}
