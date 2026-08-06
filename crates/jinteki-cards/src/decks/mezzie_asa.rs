//! Mezzie's Asa — Asa Group: Security Through Vigilance.
//!
//! Printed text is copied from NSG's official card data. Behaviour is written
//! from that text and from nowhere else (SYS-D-10): the doc comment above each
//! card carries the text for whoever is reading, `.text(…)` carries the same
//! text as data for whatever is checking, and `tests/decks.rs` asserts the two
//! agree. Sentences the vocabulary cannot say yet carry `.unimplemented(…)`
//! rather than an approximation, and the kernel capability each one waits on
//! is on the Blockers list in `docs/vm/MEZZIE-QUEUE.md`.
//!
//! The deck is written in the queue's printed order and fills in as waves
//! land: a card the deck lists and nobody has written yet is simply absent
//! from [`deck`], and a card an earlier deck already carries is reused from
//! there rather than copied.

use crate::edsl::*;

// Two of Mezzie's agendas speak from a zone their card is inactive in — one
// from the Runner's score area (4.5.4), one from the moment it is being
// accessed (9.1.8a) — and the kernel says both things already. What
// `crate::edsl` does not do is NAME them: its re-exports are every kernel type
// a card is built out of except `AbilityDef` and the three words below, so
// neither ability can be spelled with the helpers. `CardBuilder::ability` is
// the documented escape hatch for exactly this case ("reach for it when a card
// wants a combination the shorthands do not name … a static condition"), so
// the two abilities are assembled long-hand here and the shorthands they want
// are recorded in `docs/vm/MEZZIE-QUEUE.md`. Nothing about the SEMANTICS is
// long-hand: these are the same `AbilityDef`s a shorthand would build.
use jinteki_cr::ability::{AbilityDef, AbilityFlag, Condition, StaticCond};

/// The printed phrase "while this card is in a player's score area" — CR
/// 9.3.7a's stated condition, over the zone 4.5 gives each player.
///
/// It is also what keeps the ability ALIVE there. 4.5.4 makes agendas in the
/// Runner's score area inactive "unless stated otherwise", and 9.1.8b's first
/// sentence — abilities stating that they are active in a particular zone are
/// active in that zone — reads this very condition as the statement. An
/// ability that asked the same question any other way (a 9.6.5c requirement
/// about the game state, say) would be inactive in the one zone it is about,
/// and would therefore never be read at all.
fn while_in_the_score_area_of(side: Side, decls: Vec<StaticDecl>) -> AbilityDef {
    let mut def = AbilityDef::static_ability(decls);
    def.condition = Some(Condition::Static(StaticCond::SourceInScoreAreaOf(side)));
    def
}

/// A mandatory [interrupt] (9.3.6d/9.9.1) that is also active while its source
/// is the card being ACCESSED (9.1.8a).
///
/// The access flag is not decoration. An agenda in R&D, in HQ or unrezzed in a
/// remote root is INACTIVE (9.1.8), and the only moment this ability has to
/// act in is the access that is about to steal it — so without 9.1.8a's
/// exception the interrupt is never active when its own condition is met.
fn access_interrupt(cond: TriggerCond, instrs: Vec<Instruction>) -> AbilityDef {
    AbilityDef::conditional(cond, instrs, false)
        .with_flag(AbilityFlag::Interrupt)
        .with_flag(AbilityFlag::Access)
}

// ---------------------------------------------------------------------------
// Agendas
// ---------------------------------------------------------------------------

/// Global Food Initiative — Agenda: Initiative. 5/3.
/// "Global Food Initiative is worth 1 fewer agenda point while in the Runner's
///  score area."
///
/// COMPLETE. One printed sentence, and it is permanently true rather than
/// something that happens — so a static declaration (9.4), never an ability
/// that resolves when the agenda changes hands.
///
/// 2.5's point value is a CHARACTERISTIC, read through the same 9.12.1a
/// pipeline as strength: the value is recomputed wherever it is asked (1.17.1's
/// score, 1.17.2's win condition, the record a score or steal writes), so the
/// card is worth 3 in one score area and 2 in the other without anything ever
/// being stamped on it. An ability that fired on the steal and subtracted a
/// point would be a different card — forfeit it out of the Runner's score area
/// and back into the Corp's and the printed one goes back to 3.
///
/// Where the ability LIVES is the whole difficulty. 4.5.4 makes an agenda in
/// the Runner's score area inactive "unless stated otherwise", so an ability
/// that spoke about that zone in any other words would be switched off in the
/// only zone it is about. 9.1.8b's first sentence is the exception this card is
/// written on, and [`while_in_the_score_area_of`] is that statement — the
/// condition is the zone, so the same call both says WHEN the declaration
/// applies and keeps the ability active where it applies.
pub fn global_food_initiative() -> Card {
    card("Global Food Initiative")
        .corp()
        .agenda(5, 3)
        .faction("Neutral")
        .subtypes(&["Initiative"])
        .text("Global Food Initiative is worth 1 fewer agenda point while in the Runner's score area.")
        .ability(while_in_the_score_area_of(
            Runner,
            vec![StaticDecl::SelfAgendaPointsMod(amount(-1))],
        ))
        .named("1 fewer in the runner's score area")
        .build()
}

/// Luminal Transubstantiation — Agenda: Research. 3/2.
/// "When you score this agenda, gain [click][click][click]. You cannot score
///  agendas for the remainder of the turn.
///  Limit 1 per deck."
///
/// COMPLETE.
///
/// The printed line is two sentences and therefore two instructions (9.11.3)
/// of ONE conditional ability (9.6.1) — the trigger is stated once and governs
/// both.
///
/// "You cannot score agendas for the remainder of the turn" is 9.10.1's
/// lingering effect over a DESCRIPTION. Three things about it decide the
/// shape, and each of them rules out something that was tried:
///
/// * It is a lingering effect and NOT a static declaration, because it has a
///   stated duration and 9.10.1 gives such an effect a life independent of its
///   source. Written as a declaration of this active agenda, forfeiting it out
///   of the score area later the same turn (24/7 News Cycle, Archer) or
///   blanking it (9.1.9) would lift a prohibition the card says nothing about
///   lifting.
/// * Its scope is a DESCRIPTION and not a named card. "Agendas" is every
///   agenda, including one still in R&D when this resolved, so the criteria
///   are re-read wherever the (S) option is offered rather than resolved once.
///   A prohibition written over the naming position instead resolves through
///   the announced targets (1.15.2) that 9.10.1 announces none of, and forbids
///   nothing at all — the kernel now refuses that spelling outright rather
///   than build the prohibition that is not there.
/// * 1.2.2 gives the "cannot" precedence over the permission, so the (S)
///   option is not OFFERED for the rest of the turn — the same treatment
///   A Teia's "you cannot score the second card this turn" already gets, and
///   observably different from an offer that fails.
///
/// "Limit 1 per deck" is a deckbuilding restriction (1.4), not a sentence this
/// card does: it is carried as printed text and denotes into nothing, the same
/// treatment Rebirth's identical line already has.
pub fn luminal_transubstantiation() -> Card {
    card("Luminal Transubstantiation")
        .corp()
        .agenda(3, 2)
        .faction("Haas-Bioroid")
        .subtypes(&["Research"])
        .text("When you score this agenda, gain [click][click][click]. You cannot score agendas for the remainder of the turn.")
        .text("Limit 1 per deck.")
        .when(
            scored(),
            [
                gain_clicks(Corp, 3),
                cannot_act_on_matching(
                    &[of_type(CardType::Agenda)],
                    Some(Corp),
                    &[ProhibitedAction::Score],
                    this_turn(),
                ),
            ],
        )
        .named("three clicks on the score")
        .build()
}

/// Project Vacheron — Agenda: Research. 5/3.
/// "[interrupt] → When this agenda would be added to the Runnerʼs score area
///  from anywhere except Archives, instead it is added to their score area with
///  4 hosted agenda counters.
///  While this agenda is in the Runnerʼs score area with 1 or more hosted
///  agenda counters, it is worth 0 agenda points and gains “When the Runnerʼs
///  turn begins, remove 1 hosted agenda counter.“"
///
/// PARTIAL: the steal arrives with its counters; what the counters DO is
/// marked.
///
/// The first sentence is 9.9.8c's replacement effect, created ahead of the
/// effect it replaces by an interrupt on the imminent steal. 9.9.9c is the part
/// that makes it a replacement rather than a prevention: the agenda IS still
/// added to the Runner's score area — the replacement's result still includes
/// the effect it replaced — and the replacement cannot then apply to its own
/// result. So the Runner steals it, and 1.17.7's "when stolen" abilities are
/// met exactly as they would have been.
///
/// The counters are AGENDA counters (1.9.5), which are not advancement
/// counters and are not touched by 1.17.5's return of the advancement counters
/// to the bank; they are hosted on a card in a score area, which is a place
/// only 4.5 and this card ever look at.
///
/// The ability has to be active in three zones the card is inactive in — R&D,
/// HQ, and a remote root — because the sentence names every one of them by
/// exclusion. 9.1.8a is the rule that reaches them: an ability is active while
/// its source is the card being accessed, which is the only moment a steal is
/// ever imminent. See [`access_interrupt`].
///
/// ANNOTATED SHAPE. "From anywhere except Archives" is written as a 9.6.5c
/// requirement INSIDE the instruction rather than on the trigger condition,
/// because the condition the kernel has for an imminent steal of this agenda
/// takes no requirements. It is load-bearing where it stands, and measured to
/// be: with the requirement removed, a copy stolen out of Archives arrives with
/// the four counters. So the difference the shape makes is only that the
/// interrupt joins a window it then does nothing in — 9.9.8c's replacement is
/// the only thing this ability does, and it is not created either way. The
/// requirement itself is exact: 8.5.13's facedown Archives and the faceup half
/// are one zone (4.4), and the zone the agenda is in when the steal becomes
/// imminent is what the sentence asks about.
///
/// UNIMPLEMENTED: the second sentence, on three counts. It is one static
/// ability whose stated condition is a zone AND a number of hosted counters,
/// and 9.3.7a's condition slot holds one or the other and never both. Its
/// first declaration SETS the point value ("it is worth 0 agenda points"),
/// which is 9.12.1a's second stage, and the declaration that exists modifies
/// the value instead — a subtraction of the printed 3 gives 0 only while
/// nothing else is modifying it. Its second declaration grants the card a
/// stated CONDITIONAL ability, and the only stated ability a declaration can
/// grant is a subroutine. Written with any of the three approximated the card
/// would be worth the wrong number of points in the Runner's score area, which
/// is the one thing this agenda is about. The general capabilities wanted are
/// on MEZZIE-QUEUE.md's Blockers.
pub fn project_vacheron() -> Card {
    card("Project Vacheron")
        .corp()
        .agenda(5, 3)
        .faction("Haas-Bioroid")
        .subtypes(&["Research"])
        .text("[interrupt] → When this agenda would be added to the Runnerʼs score area from anywhere except Archives, instead it is added to their score area with 4 hosted agenda counters.")
        .text("While this agenda is in the Runnerʼs score area with 1 or more hosted agenda counters, it is worth 0 agenda points and gains “When the Runnerʼs turn begins, remove 1 hosted agenda counter.“")
        .ability(access_interrupt(
            TriggerCond::WouldStealSelfAgenda,
            vec![if_met(
                &[source_not_in_archives()],
                [Instruction::CreateLingeringEffect {
                    payload: LingeringSpec::Replacement {
                        applies_to: EffectClass::StealAgenda,
                        with: ReplacementTransform::StealWithHostedCounters {
                            kind: CounterKind::Agenda,
                            amount: 4,
                        },
                        optional: false,
                    },
                    duration: WantedDuration::ThisRun,
                }],
            )],
        ))
        .named("stolen with four agenda counters")
        .unimplemented("While this agenda is in the Runnerʼs score area with 1 or more hosted agenda counters, it is worth 0 agenda points and gains “When the Runnerʼs turn begins, remove 1 hosted agenda counter.“")
        .build()
}

/// Project Vitruvius — Agenda: Research. 3/2.
/// "When you score this agenda, place 1 agenda counter on it for each hosted
///  advancement counter past 3.
///  Hosted agenda counter: Add 1 card from Archives to HQ."
///
/// COMPLETE. Two printed sentences: a conditional ability met by the scoring,
/// and a paid ability whose trigger cost is one of the counters it placed.
///
/// The extra advancement counters do NOT survive the scoring — 1.17.5 returns
/// every advancement counter on a scored agenda to the bank as it becomes
/// uninstalled, before this ability resolves. 1.17.8 is the rule that makes the
/// sentence mean anything at all: an ability meeting its trigger condition from
/// its agenda being scored reads that agenda's LAST KNOWN number of advancement
/// counters, so "for each hosted advancement counter past 3" is answered from
/// the moment scoring began and not from the empty card in the score area.
/// Project Beale prints the same arithmetic at a different rate.
///
/// "Past 3" is the printed 3 and not the advancement requirement, which happens
/// to be the same number here (1.18: they are different questions, and a
/// declaration modifying the requirement would not move this). A card scored on
/// exactly its requirement places nothing, and 9.12.2's calculated quantity
/// floors at zero rather than going negative.
///
/// The paid ability is 1.16.1's trigger cost — everything before the colon —
/// spending one of the hosted agenda counters, which is what AstroScript Pilot
/// Program already does with the counter it places. 1.15.2c is why "from
/// Archives" is written: a description reaches installed cards unless it names
/// another zone, and 4.4.4's inactive cards there are no obstacle to being
/// added to a hand.
pub fn project_vitruvius() -> Card {
    card("Project Vitruvius")
        .corp()
        .agenda(3, 2)
        .faction("Haas-Bioroid")
        .subtypes(&["Research"])
        .text("When you score this agenda, place 1 agenda counter on it for each hosted advancement counter past 3.")
        .text("Hosted agenda counter: Add 1 card from Archives to HQ.")
        .when(
            scored(),
            [place_on_q(
                this_card(),
                CounterKind::Agenda,
                Quantity::Minus(
                    Box::new(per_hosted_counter(CounterKind::Advancement)),
                    Box::new(amount(3)),
                ),
            )],
        )
        .named("a counter for every advancement past three")
        .paid(
            hosted_counters(CounterKind::Agenda, 1),
            [add_to_hand(choose(1, &[in_archives()]))],
        )
        .named("archives to hq")
        .build()
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// Estelle Moon — Asset: Executive. Rez 2, trash 3. ◆
/// "Whenever you install a card in the root of a remote server, place 1 power
///  counter on this asset.
///  [trash]: For each power counter on this asset, gain
///  2[credit] and draw 1 card."
///
/// COMPLETE. Two printed sentences: a conditional ability that counts, and a
/// paid ability that spends what was counted.
///
/// The count is the card. "For each power counter on this asset" is 9.12.2's
/// calculated quantity, read when the ability RESOLVES and not when it was
/// used — and 9.12.2c is what settles the shape the UFAQ was asked about
/// (three counters: six credits and THREE cards, not six credits and one):
/// a calculated quantity aggregates into a single effect, so this is one gain
/// of 2×N and one draw of N, in one instruction, because 9.11.3 makes "gain
/// 2[credit] and draw 1 card" one sentence and `combined(…)` is how a
/// sentence with two effects is written.
///
/// 9.5.5 is what keeps the number from being zero. The [trash] trigger cost
/// uninstalls the source before the effects resolve, and the counters would
/// go to the bank with it — so the rule sets them aside as the cost is paid
/// and they are "still considered to be hosted" for this ability's own
/// effects. Nothing on the card says so; the rule says so for every card
/// shaped like this.
///
/// ANNOTATED SHAPE. "In the root of a remote server" is written as the
/// install condition's remote-server location narrowed by the three card
/// types a root can hold. See [`installs_a_card_in_the_root_of_a_remote_server`]:
/// 4.6.6e and 4.6.9d make that the same set of installs and not an
/// approximation of it — ice is never in a root and an agenda, asset or
/// upgrade never protects a server — but the kernel's location word is still
/// the wider "in the root of or protecting", and the narrower one it wants is
/// on MEZZIE-QUEUE.md's Blockers.
pub fn estelle_moon() -> Card {
    card("Estelle Moon")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .subtypes(&["Executive"])
        .cost(2)
        .trash_cost(3)
        .unique()
        .text("Whenever you install a card in the root of a remote server, place 1 power counter on this asset.")
        .text("[trash]: For each power counter on this asset, gain 2[credit] and draw 1 card.")
        .when(installs_a_card_in_the_root_of_a_remote_server(Corp), [place(CounterKind::Power, 1)])
        .named("a counter for every remote install")
        .paid(
            trash_this_card(),
            [combined([
                gain_q(Corp, times(2, per_hosted_counter(CounterKind::Power))),
                draw_q(Corp, per_hosted_counter(CounterKind::Power)),
            ])],
        )
        .named("cash the counters in")
        .build()
}

/// Jeeves Model Bioroids — Asset: Alliance. Rez 2, trash 5. ◆
/// "This card costs 0 influence if you have 6 or more
///  non-alliance [haas-bioroid] cards in your deck.
///  The first time you spend 3[click] on the same action each turn, gain
///  [click]."
///
/// UNIMPLEMENTED: the second sentence, which is the whole of what this card
/// does at the table.
///
/// The alliance line is not a sentence this card does. Like "Limit 1 per
/// deck" it is a deckbuilding restriction on influence (1.4.5) — it changes
/// what may go in a deck, and nothing about it is ever asked during a game.
/// It is carried as printed text and denotes into nothing, which is the same
/// treatment Salem's Hospitality already has.
///
/// "The first time you spend 3[click] on the same action each turn" counts
/// CLICKS, and the trigger vocabulary counts ACTIONS. The nearest words are
/// The Collective's "the same action three times in a row" and MirrorMorph's
/// "three different actions", and neither is this sentence: 5.2.6h's basic
/// purge action is ONE action costing three clicks and meets Jeeves, and a
/// double operation followed by an ordinary one is TWO actions costing three
/// clicks between them and meets Jeeves — both of which the official rulings
/// list, and neither of which is any number of repeated actions. Written with
/// the words that exist the card would silently fire less often than it
/// should, so it is marked instead. The general capability wanted is on
/// MEZZIE-QUEUE.md's Blockers.
pub fn jeeves_model_bioroids() -> Card {
    card("Jeeves Model Bioroids")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .subtypes(&["Alliance"])
        .cost(2)
        .trash_cost(5)
        .unique()
        .text("This card costs 0 influence if you have 6 or more non-alliance [haas-bioroid] cards in your deck.")
        .text("The first time you spend 3[click] on the same action each turn, gain [click].")
        .unimplemented("The first time you spend 3[click] on the same action each turn, gain [click].")
        .build()
}

/// Lakshmi Smartfabrics — Asset. Rez 1, trash 3.
/// "Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics.
///  X hosted power counters: Reveal an agenda worth X points
///  from HQ. The Runner cannot steal copies of that agenda for the remainder
///  of this turn."
///
/// PARTIAL: the counting works; the ability that spends the counters does
/// not.
///
/// The first sentence says nothing whatever about the card rezzed, so it is
/// met by every rez the Corp makes — including this card's own. The UFAQ was
/// asked exactly that ("does Lakshmi get a power counter when it is rezzed?")
/// and the answer is yes: 8.1.3 turns the card faceup and active as part of
/// the rez, so the ability is there in time to be met by the occurrence that
/// activated it.
///
/// The paid ability is marked, and what it waits on is now the REVEAL at both
/// ends of it rather than the prohibition. The prohibition itself is written:
/// the CR 1.2.2 wave made stealing (7.5) an act a "cannot" names and gave the
/// lingering prohibition a description and a duration, so "…for the remainder
/// of this turn" is sayable.
///
/// What is not sayable is which agenda may be revealed and which cards the
/// sentence then means. "An agenda worth X points" is a description
/// stipulating a characteristic the filter vocabulary does not read — the
/// card's agenda points (2.4.2), compared against the X announced for the
/// ability's own trigger cost (1.16.2c). And "copies of that agenda" is a
/// description of cards sharing a characteristic with the card THIS ABILITY
/// REVEALED, which 1.21.3's reveal records nowhere: unlike 1.21.2's look, a
/// reveal keeps nothing on the resolving ability for a later instruction to
/// refer back to. Both are on MEZZIE-QUEUE.md's Blockers as general
/// capabilities, and both belong to the same reveal.
pub fn lakshmi_smartfabrics() -> Card {
    card("Lakshmi Smartfabrics")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .cost(1)
        .trash_cost(3)
        .text("Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics.")
        .text("X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn.")
        .when(corp_rezzes_a_card(), [place(CounterKind::Power, 1)])
        .named("a counter for every rez")
        .unimplemented("X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn.")
        .build()
}

/// Marilyn Campaign — Asset: Advertisement. Rez 2, trash 3.
/// "When you rez this asset, load 8[credit] onto it. When it is empty, trash
///  it.
///  When your turn begins, take 2[credit] from this asset.
///  [interrupt] → When this asset would be trashed, you may shuffle it into
///  R&D instead of adding it to Archives. (It is still considered
///  trashed.)"
///
/// PARTIAL: the campaign pays out and empties itself; the escape into R&D is
/// marked.
///
/// The first printed line is two sentences and two abilities, which is what
/// Daily Casts already is on the Runner's side of the table — LOADING (1.9.4)
/// is what links the "when it is empty" ability to this card, so an asset
/// that had credits placed on it some other way would never trash itself.
/// The one word that differs from Daily Casts is the trigger: "when you rez
/// this asset" rather than "when you install this resource", because a Corp
/// card installed facedown is inactive (9.1.8) and has no ability to meet
/// anything with until it is rezzed.
///
/// "Take 2[credit] from this asset" is 1.10.3a and not a gain of 2 from the
/// bank: the credits move from the card into the pool, which is why the card
/// runs out. An asset holding only 1 gives the 1 it has.
///
/// UNIMPLEMENTED: the interrupt. It is a 9.9.8a replacement of where a trash
/// puts the card, and the kernel replaces a trash destination in exactly one
/// shape — a static declaration, mandatory, naming the removed-from-game zone
/// or a facedown card in play. Marilyn needs the destination to be a deck and
/// needs the replacement to be one the Corp MAY decline, and neither is
/// content on the word that exists. Writing it with what is there would make
/// every trash of this card a shuffle whether the Corp wanted it or not; 8.2.2
/// is the part both readings must keep, and the parenthetical restates it —
/// the card is still trashed, only where it lands changes. The general
/// capability wanted is on MEZZIE-QUEUE.md's Blockers.
pub fn marilyn_campaign() -> Card {
    card("Marilyn Campaign")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .subtypes(&["Advertisement"])
        .cost(2)
        .trash_cost(3)
        .text("When you rez this asset, load 8[credit] onto it. When it is empty, trash it.")
        .text("When your turn begins, take 2[credit] from this asset.")
        .text("[interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. (It is still considered trashed.)")
        .when(self_rezzed(), [load(CounterKind::Credit, 8)])
        .named("load eight on the rez")
        .when(empty_of(CounterKind::Credit), [trash_self()])
        .named("empty, so gone")
        .when(turn_begins(Corp), [take_hosted_credits(this_card(), 2, Corp)])
        .named("two a turn")
        .unimplemented("[interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. (It is still considered trashed.)")
        .build()
}

/// MCA Austerity Policy — Asset. Rez 1, trash 3. ◆
/// "Once per turn → [click]: Place 1 power counter on this
///  asset. When the Runner's next turn begins, they lose [click].
///  [click], [trash], 3 hosted power counters: Gain
///  [click][click][click][click]."
///
/// COMPLETE. Two paid abilities; the first carries 9.3.6g's once-per-turn
/// flag and the second, as the UFAQ says in so many words, does not — so the
/// Corp may cash the card in on the same turn the third counter lands.
///
/// The first ability is TWO instructions, because 9.11.3 makes each sentence
/// one: the counter is placed, and then a delayed conditional ability
/// (9.6.13) is created that waits for the Runner's next turn to begin. It has
/// no stated duration, so 9.6.13c has it exist until it first resolves — a
/// second use on a later Corp turn arms a second one rather than re-arming
/// this. The click the Runner then loses is 1.11.3b's LOSS and not a spend:
/// the two are not synonymous for meeting conditions, a Runner with none left
/// simply stays at zero, and this is why the sentence needs no way for one
/// player's ability to reach into the other's pool — nothing here is
/// controlled by the Runner or paid by them. 1.14.4 leaves both abilities with
/// the Corp throughout, which is the default and not a departure from it.
///
/// The second ability's cost is three costs paid as one (1.16.10b), and 9.5.5
/// is what makes it payable at all: the [trash] uninstalls the source, so the
/// three hosted counters are set aside as the whole cost is paid rather than
/// returning to the bank ahead of the counters half.
pub fn mca_austerity_policy() -> Card {
    card("MCA Austerity Policy")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .cost(1)
        .trash_cost(3)
        .unique()
        .text("Once per turn → [click]: Place 1 power counter on this asset. When the Runner's next turn begins, they lose [click].")
        .text("[click], [trash], 3 hosted power counters: Gain [click][click][click][click].")
        .paid_once_per_turn(
            clicks(1),
            [
                place(CounterKind::Power, 1),
                when_the_next_turn_begins_of(
                    Runner,
                    "mca austerity policy: the runner loses a click",
                    [lose_clicks(Runner, 1)],
                ),
            ],
        )
        .named("a counter, and a click off the runner")
        .paid(
            clicks(1).plus_cost(trash_this_card()).plus_cost(hosted_counters(CounterKind::Power, 3)),
            [gain_clicks(Corp, 4)],
        )
        .named("cash in for four clicks")
        .build()
}

/// Mumba Temple — Asset: Alliance - Facility. Rez 1, trash 3.
/// "This card costs 0 influence if you have 15 or fewer ice in your deck.
///  2[recurring-credit]
///  Use these credits to rez cards."
///
/// PARTIAL: the credits arrive; nothing can spend them yet.
///
/// The alliance line is a 1.4.5 deckbuilding restriction on influence and not
/// a sentence this card does — the same treatment Jeeves Model Bioroids's
/// first line already has: carried as printed text, denoting into nothing,
/// because nothing about it is ever asked during a game.
///
/// "2[recurring-credit]" is a printed fact rather than an ability (1.10.5),
/// and 1.10.5b is what makes it observable: the credits are first placed as
/// soon as the card becomes active — for a Corp asset, the moment it is
/// rezzed — and 1.10.5d refills rather than accumulates them when the Corp's
/// turn begins, so the card never holds more than the 2 it prints.
///
/// UNIMPLEMENTED: "Use these credits to rez cards." 1.10.3c is the whole of
/// what hosted credits are — they may be spent only as the hosting card's
/// ability allows — and the vocabulary names five allowances: any payment, a
/// payment to trash described cards, a payment for USING described cards
/// (9.1.6a's paid-ability trigger cost), a trace attempt's two spend steps,
/// and a payment to advance described cards. Rezzing is none of them and is
/// not a spelling of any of them: 8.1.2's rez procedure pays a card's rez
/// cost and uses no ability at all, so writing this as the "using" allowance
/// would let the credits pay for paid abilities they may not pay for and
/// still not pay for a rez. With the restriction unsayable the placed credits
/// are reachable by no payment, which is the honest reading of a card whose
/// only permission is the marked sentence. The general capability wanted is
/// on MEZZIE-QUEUE.md's Blockers.
pub fn mumba_temple() -> Card {
    card("Mumba Temple")
        .corp()
        .asset()
        .faction("Neutral")
        .subtypes(&["Alliance", "Facility"])
        .cost(1)
        .trash_cost(3)
        .recurring_credits(2)
        .text("This card costs 0 influence if you have 15 or fewer ice in your deck.")
        .text("2[recurring-credit]")
        .text("Use these credits to rez cards.")
        .unimplemented("Use these credits to rez cards.")
        .build()
}

/// Spin Doctor — Asset: Character. Rez 0, trash 2. ◆
/// "When you rez this asset, draw 2 cards.
///  Remove this asset from the game: Shuffle up to 2 cards from Archives into
///  R&D."
///
/// COMPLETE. Two printed sentences and two abilities, and they are two rather
/// than one because the second prints a colon: everything before it is a cost
/// (1.16.1) and what follows is a paid ability, while the first is 9.6.1's
/// conditional met by an occurrence.
///
/// The conditional's trigger is the REZ and not the install, for the reason
/// Marilyn Campaign's is: a Corp card installed facedown is inactive (9.1.8)
/// and has no ability to meet anything with until 8.1.3 turns it faceup, at
/// which point it is there in time to be met by the occurrence that activated
/// it.
///
/// The paid ability's cost removes its own source from the game (4.9), and
/// 1.16.1 is what makes that the interesting half: a trigger cost is paid
/// BEFORE the ability's effects resolve, so this card is already in the
/// removed-from-game zone — and therefore inactive — while the shuffle it
/// paid for happens. 9.1.8g is why the ability resolves anyway: an ability
/// whose trigger condition was met, or whose cost has been paid, resolves
/// even though its source has left the zone it was in. Nothing of the
/// resolution reads the card, so nothing about it is diminished by the card
/// being gone.
///
/// 9.6.10 is the other end of that rule and the reason the two halves are
/// worth keeping apart: a PENDING instance of an ability that becomes
/// inactive before it resolves never resolves. A card that derezzes or
/// trashes this asset while its "when you rez" instance is still pending —
/// the Councilman class — takes that draw away entirely, which is not what
/// paying a cost does and not a case 9.1.8g reaches.
///
/// "Up to 2 cards from Archives" announces its targets (1.15.2) with 1.15.2e's
/// floor of zero, and 4.2.3 shuffles the deck they go into.
pub fn spin_doctor() -> Card {
    card("Spin Doctor")
        .corp()
        .asset()
        .faction("NBN")
        .subtypes(&["Character"])
        .cost(0)
        .trash_cost(2)
        .unique()
        .text("When you rez this asset, draw 2 cards.")
        .text("Remove this asset from the game: Shuffle up to 2 cards from Archives into R&D.")
        .when(self_rezzed(), [draw(Corp, 2)])
        .named("draw two on the rez")
        .paid(remove_self_cost(), [shuffle_from_discard_into_deck(Corp, 2)])
        .named("shuffle archives back into r&d")
        .build()
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Enhanced Login Protocol — Operation: Current. Cost 2.
/// "This operation is not trashed until another current is played or an agenda
///  is stolen.
///  As an additional cost to take the basic action to run a server for the
///  first time each turn, the Runner must spend [click]."
///
/// PARTIAL: the current stays; the toll it charges cannot be stated.
///
/// The first sentence is 8.6.6c read exactly: instead of trashing the card at
/// step 8.6.7g of playing it, a lingering effect is created whose duration
/// expires as one of the printed occurrences happens, and the card stays in
/// the play area — and therefore active, and therefore still speaking —
/// until then. 3.5.1b prints the two occurrences for a current OPERATION:
/// another current operation or event being played by either player, and the
/// Runner stealing an agenda. That is the same declaration Targeted Marketing
/// already carries, and it is what makes the second sentence a static ability
/// of a card in play rather than something the operation does as it resolves.
///
/// UNIMPLEMENTED: the second sentence. It is 1.16.10's additional cost on
/// 5.2.7a's basic run action, paid at 6.9.1a before the run formally begins
/// (which is exactly the CR's own worked example against Heinlein Grid: the
/// click is spent to INITIATE the run and is not spent during it). The
/// declaration for that cost exists and names which servers it reaches — but
/// it says nothing about WHICH TAKINGS of the action it attaches to, and this
/// card charges only the first each turn. Written with the word that exists
/// the Runner would pay a click for every run action of every turn, which is
/// a strictly worse card than the printed one and a 1.16.1b gate on actions
/// the Runner is entitled to take for free. A conditional ability met by the
/// first run each turn is not a substitute either: an additional cost gates
/// the action (1.16.1b — an unpayable cost means the action cannot be taken
/// at all), and a conditional resolves after it. So it is marked. The general
/// capability wanted is on MEZZIE-QUEUE.md's Blockers.
pub fn enhanced_login_protocol() -> Card {
    card("Enhanced Login Protocol")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .subtypes(&["Current"])
        .cost(2)
        .text("This operation is not trashed until another current is played or an agenda is stolen.")
        .text("As an additional cost to take the basic action to run a server for the first time each turn, the Runner must spend [click].")
        .declares([not_trashed_until_an_agenda_is_stolen()])
        .named("the current stays in the play area")
        .unimplemented("As an additional cost to take the basic action to run a server for the first time each turn, the Runner must spend [click].")
        .build()
}

/// Flood the Market — Operation: Double. Cost 3.
/// "As an additional cost to play this operation, spend [click].
///  Choose 1 installed card you can advance. Place 1 advancement counter on
///  that card for each remote server that has a card in its root and is
///  protected by ice."
///
/// COMPLETE.
///
/// The first sentence is 5.6.2a's *double* said in the card's own words, and
/// it is a printed FACT about this card rather than a declaration — 1.16.10's
/// additional cost to play THIS operation, paid at step 8.6.7b along with the
/// 3[credit], which is where BOOM! already carries the same line.
///
/// The rest is ONE instruction, not two, because 9.11.4c says so: the first
/// half only chooses a target and the second acts on it. So the choice is
/// announced (1.15.2) as that one instruction becomes imminent, and the
/// counters land on the announced card when it resolves.
///
/// "For each remote server that has a card in its root and is protected by
/// ice" is 9.12.2's calculated quantity over 4.6.6a's SERVERS, written with
/// the server-filter words — the type (4.6.6c), a card in the root (4.6.6e),
/// ice protecting it (4.6.6d) — and it is a count of SERVERS on purpose. No
/// count of cards stands in for it: 4.6.6e lets a remote root hold an asset or
/// agenda AND any number of upgrades, so counting what is in the qualifying
/// roots over-counts a server carrying an upgrade, and counting the ice in
/// front of them over-counts a server behind two. Read at resolution like
/// every calculated quantity (9.12.2), so a remote that ceased to exist at a
/// checkpoint (4.6.8e) is no longer among them.
///
/// 1.18.2: this PLACES advancement counters and does not advance, so nothing
/// meets an "advances a card" condition — but the card announced still has to
/// be one the Corp can advance (1.18.3), which is what the printed
/// description says and what an agenda satisfies by default.
pub fn flood_the_market() -> Card {
    card("Flood the Market")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&["Double"])
        .cost(3)
        .text("As an additional cost to play this operation, spend [click].")
        .text("Choose 1 installed card you can advance. Place 1 advancement counter on that card for each remote server that has a card in its root and is protected by ice.")
        .additional_play_cost(clicks(1))
        .play([place_on_q(
            choose(1, &[advanceable()]),
            CounterKind::Advancement,
            per_server_matching(&[
                remote_server(),
                with_a_card_in_its_root(&[]),
                protected_by(&[of_type(CardType::Ice)]),
            ]),
        )])
        .named("an advancement counter per qualifying remote server")
        .build()
}

/// Friends in High Places — Operation: Terminal. Cost 2.
/// "After you resolve this operation, end your action phase.
///  Install up to 2 cards from Archives (paying all install costs)."
///
/// COMPLETE. Two printed sentences: 5.6.2b's *terminal*, written as the
/// conditional it is — met at step 8.6.7h, AFTER the play abilities have
/// resolved, which is why the installs still happen — and the installs
/// themselves.
///
/// "Install up to 2 cards" is written as two instructions and not one, which
/// is 9.11.4b in so many words: a sentence directing a player to install more
/// than one card handles each as a separate instruction, and the CR's own
/// example rewrites Shipment from MirrorMorph's single sentence as one "you
/// may install a card" per card. Each of the two therefore announces at most
/// one card (1.15.2e's "up to" is what makes the floor zero, so a Corp with
/// nothing worth installing declines both), and a card taken by the first is
/// no longer in Archives for the second to describe.
///
/// Archives is where they come from, and 1.15.2c is why the description says
/// so: a description reaches installed cards unless it names another zone.
/// The cards there are INACTIVE (4.4.4) and that is no obstacle — installing
/// is a movement into the play area, and 8.5.13 is what the facedown half of
/// Archives makes interesting, since a card whose provenance was hidden is
/// still installed the same way.
///
/// The parenthetical is the card refusing 1.16.5c: nothing is ignored, so
/// 8.5.11a's 1[credit] for each piece of ice already protecting the
/// destination server is paid, and 8.5.16b is a real choice for a Corp card —
/// unlike the Runner's rig, a Corp destination is a server the installer
/// declares, with 8.5.2a's brand-new remote among the options.
pub fn friends_in_high_places() -> Card {
    card("Friends in High Places")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .subtypes(&["Terminal"])
        .cost(2)
        .text("After you resolve this operation, end your action phase.")
        .text("Install up to 2 cards from Archives (paying all install costs).")
        .when(after_this_resolves(), [end_action_phase(Corp)])
        .named("terminal: the action phase ends")
        .play([
            install(choose_up_to(1, &[in_archives()]), InstallDest::DeclaredByInstaller),
            install(choose_up_to(1, &[in_archives()]), InstallDest::DeclaredByInstaller),
        ])
        .named("two installs out of archives")
        .build()
}

/// Fully Operational — Operation. Cost 1.
/// "Gain 2[credit] or draw 2 cards. Repeat this process for each remote server
///  that has a card in its root and is protected by ice."
///
/// COMPLETE.
///
/// "Gain 2[credit] or draw 2 cards" is 9.11.4g's optioned effect: the choice
/// itself ends an instruction and the option chosen begins the next, so a
/// checkpoint falls between choosing and doing. 1.14.4 leaves the choice with
/// the Corp, the controller of the ability's source, because the sentence
/// names nobody else.
///
/// "Repeat this process for each remote server…" is the second sentence and
/// therefore the second instruction (9.11.3), which is exactly what makes the
/// printed arithmetic 1 + N and never N: the process happens once, and then
/// once more for each such server. The count is the same quantity over
/// 4.6.6a's SERVERS that Flood the Market takes, said in the same words, and
/// it is a count of servers because no count of cards is one (4.6.6e's remote
/// root holds an asset or agenda and any number of upgrades; a server can be
/// behind more than one piece of ice).
///
/// Each repetition is a FRESH choice, and 9.12.2b is why: effects tied to a
/// calculated quantity are aggregated only if every one of them is one of
/// 9.12.2c's classes, and an optioned effect is not — so the group is
/// performed once per unit, as separate occurrences, and the Corp may take
/// the credits once and the cards the next time.
pub fn fully_operational() -> Card {
    card("Fully Operational")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .cost(1)
        .text("Gain 2[credit] or draw 2 cards. Repeat this process for each remote server that has a card in its root and is protected by ice.")
        .play([
            choose_one([
                ("gain 2[credit]", vec![gain(Corp, 2)]),
                ("draw 2 cards", vec![draw(Corp, 2)]),
            ]),
            for_each(
                per_server_matching(&[
                    remote_server(),
                    with_a_card_in_its_root(&[]),
                    protected_by(&[of_type(CardType::Ice)]),
                ]),
                [choose_one([
                    ("gain 2[credit]", vec![gain(Corp, 2)]),
                    ("draw 2 cards", vec![draw(Corp, 2)]),
                ])],
            ),
        ])
        .named("gain two or draw two, once and again per qualifying remote server")
        .build()
}

// ---------------------------------------------------------------------------
// Upgrades
// ---------------------------------------------------------------------------

/// Ash 2X3ZB9CY — Upgrade: Bioroid. Rez 2, trash 3. ◆
/// "Whenever there is a successful run on this server, Trace[4]. If successful,
///  the Runner cannot access any cards other than Ash 2X3ZB9CY for the
///  remainder of this run."
///
/// COMPLETE. One printed sentence and one instruction: 10.8's trace, whose
/// "if successful" branch is part of the same instruction rather than a second
/// one — 10.8.6 resolves the outcome as the trace completes.
///
/// WHEN it happens is the card. 6.9.5a declares the run successful and then
/// opens a reaction window, and the Runner's breach is step 6.9.5b — so this
/// ability resolves BEFORE the Runner accesses anything, which is the only
/// order in which "cannot access any cards other than this one" can mean
/// anything. (It is also why the Runner is still choosing whether to trash Ash
/// with the access it does get.) 4.6.6e is what "on this server" reads: the
/// upgrade sits in the root, and the run's attacked server is compared against
/// the server its source is in, so a rezzed copy in a remote says nothing
/// about a run on HQ.
///
/// The restriction is 7.4.2 in the CR's own words: a card the Runner is
/// prohibited from accessing "ceases to be a candidate", and cannot become one
/// again while the prohibition lasts. So the Runner does not access fewer
/// cards than they were entitled to — there is simply one candidate left, and
/// it is the upgrade that did this to them. It is a lingering effect for the
/// remainder of the RUN (9.10.4 binds it to the run in progress), so a second
/// run on the same server that turn is unaffected.
pub fn ash_2x3zb9cy() -> Card {
    card("Ash 2X3ZB9CY")
        .corp()
        .upgrade()
        .faction("Haas-Bioroid")
        .subtypes(&["Bioroid"])
        .cost(2)
        .trash_cost(3)
        .unique()
        .text("Whenever there is a successful run on this server, Trace[4]. If successful, the Runner cannot access any cards other than Ash 2X3ZB9CY for the remainder of this run.")
        .when(
            TriggerCond::SuccessfulRunOnServer,
            [trace(4, [Instruction::RestrictAccessToSelf])],
        )
        .named("trace on a successful run here")
        .build()
}

/// Manegarm Skunkworks — Upgrade. Rez 2, trash 3. ◆
/// "Whenever the Runner approaches this server, end the run unless they either
///  spend [click][click] or pay 5[credit]."
///
/// UNIMPLEMENTED: the card's only sentence, on two counts that are each enough
/// on their own.
///
/// "Approaches THIS server" is 6.9.4g's step with the server as a stipulation,
/// and the approach condition takes none. The kernel's other two run
/// conditions about a server carry it — a successful run "on this server"
/// (Ash, above) and a run on this server ending (the AMAZE class) both compare
/// the attacked server against the server the source is in — but the approach
/// was written for the Formicary class, whose sentence names A server and means
/// every one of them. Written with the word that exists, a rezzed copy in a
/// remote would end runs on HQ and R&D, which is not a smaller card than the
/// printed one; it is a different and much larger one.
///
/// "Unless they either spend [click][click] or pay 5[credit]" is 1.16.11b's
/// nested cost with TWO costs, and the nested cost holds one. The two are not
/// interchangeable and neither is a subset of the other: 1.11 clicks and 1.10
/// credits are different resources, and a Runner with 5[credit] and no clicks
/// escapes by one door while a Runner with two clicks and no credits escapes by
/// the other. Writing it as one cost drops whichever door was not written and
/// ends runs the Runner had paid to continue; writing it as two nested costs
/// one inside the other invents an instruction boundary the sentence does not
/// have (9.11.3), and with it a checkpoint and an interrupt window between the
/// two halves of a single choice. 9.12.3c is the shape the sentence actually
/// has — a choice among options, restricted to the ones that can be fully
/// resolved — so a Runner who can afford neither faces no choice at all and
/// the run ends. The general capabilities wanted are on MEZZIE-QUEUE.md's
/// Blockers.
pub fn manegarm_skunkworks() -> Card {
    card("Manegarm Skunkworks")
        .corp()
        .upgrade()
        .faction("Haas-Bioroid")
        .cost(2)
        .trash_cost(3)
        .unique()
        .text("Whenever the Runner approaches this server, end the run unless they either spend [click][click] or pay 5[credit].")
        .unimplemented("Whenever the Runner approaches this server, end the run unless they either spend [click][click] or pay 5[credit].")
        .build()
}

// ---------------------------------------------------------------------------
// Ice
// ---------------------------------------------------------------------------

/// Tatu-Bola — ICE: Barrier. Rez 2, strength 1.
/// "When the Runner passes this ice, you may swap it with a piece of ice from
///  HQ. If you do, gain 4[credit]. (The new ice is installed unrezzed. You
///  do not pay an install cost.)
///  [subroutine] End the run."
///
/// COMPLETE. Two printed sentences, two instructions (9.11.3) — the swap, and
/// the gain that reads back whether it happened.
///
/// The printed "you may" governs the whole conditional ability (9.6.9): a
/// Corp who declines swaps nothing and gains nothing. "If you do" is the
/// other half of that, and it is a real question rather than a restatement:
/// with no ice in HQ the ability is still offered, the swap announces
/// nothing, and the gain must not happen. 1.15.4 is what lets the second
/// sentence ask — it names the card the first sentence chose — and 8.8.2 is
/// why an announced partner always means a completed swap: the candidate list
/// is already filtered to cards that may legally occupy each other's
/// locations, so there is no announcement that fails to exchange.
///
/// The parenthetical is 1.4 reminder text for 8.8.4a/b, which the swap
/// already is: exactly one of the two was installed, so the other takes its
/// position without the 8.5.16 install procedure — no cost paid — and enters
/// the play area in the state a Corp card normally enters it, which is
/// unrezzed.
///
/// ANNOTATED SHAPE. The swap is written as ONE instruction with two halves —
/// the 1.15.2 announcement of the piece of ice, then the 8.8.1 exchange of
/// that card with this one — rather than as a swap whose two sides are two
/// descriptions. That is the same single instruction either way (no extra
/// checkpoint, no extra reaction or interrupt window, one announcement), but
/// the shorter spelling cannot be used yet: the vocabulary's swap draws BOTH
/// its sides from one description, because 8.8.2's "may occupy the other's
/// location" filter is applied within that description's own candidates. A
/// swap with one side fixed by the sentence ("swap **it** with …") therefore
/// finds no partner and silently does nothing, which is worse than saying it
/// long-hand. The general capability wanted is in MEZZIE-QUEUE.md's Blockers;
/// when it lands this becomes one call.
pub fn tatu_bola() -> Card {
    card("Tatu-Bola")
        .corp()
        .ice(1)
        .faction("Jinteki")
        .subtypes(&["Barrier"])
        .cost(2)
        .text("When the Runner passes this ice, you may swap it with a piece of ice from HQ. If you do, gain 4[credit]. (The new ice is installed unrezzed. You do not pay an install cost.)")
        .text("[subroutine] End the run.")
        .may_when(
            passed(),
            [
                combined([
                    choose_cards(1, &[in_hand_of(Corp), of_type(CardType::Ice)]),
                    swap(this_card(), earlier_choice(0)),
                ]),
                if_met(
                    &[earlier_choice_matches(0, &[of_type(CardType::Ice)])],
                    [gain(Corp, 4)],
                ),
            ],
        )
        .named("trade places with hq")
        .subroutine([end_the_run()])
        .build()
}

/// Vanilla — ICE: Barrier. Rez 0, strength 0.
/// "[subroutine] End the run."
///
/// COMPLETE.
pub fn vanilla() -> Card {
    card("Vanilla")
        .corp()
        .ice(0)
        .faction("Neutral")
        .subtypes(&["Barrier"])
        .cost(0)
        .text("[subroutine] End the run.")
        .subroutine([end_the_run()])
        .build()
}

/// Fairchild 3.0 — ICE: Code Gate - Bioroid - AP. Rez 6, strength 5.
/// "Lose [click][click][click]: Break up to 3 subroutines on
///  this ice. Only the Runner can use this ability.
///  [subroutine] The Runner must pay 3[credit] or trash 1 of their installed
///  cards.
///  [subroutine] The Runner must pay 3[credit] or trash 1 of their installed
///  cards.
///  [subroutine] Do 1 core damage or end the run."
///
/// UNIMPLEMENTED: the bioroid ability. Everything in it but the last sentence
/// has words — 5.2.1a's "Lose [click]" trigger cost, and 9.8.6's break of up
/// to 3 of this ice's unbroken subroutines — but "Only the Runner can use
/// this ability" is 1.14.4's *"by default"* clause, and the kernel has no
/// default to depart from: an ability's controller is the controller of its
/// source, full stop, and paid abilities are offered to that player alone.
/// Written without it the ability would be the CORP's, which is worse than
/// leaving it unsaid — the Corp would be able to break its own ice and the
/// Runner never could. So it is marked rather than approximated.
///
/// The three subroutines are complete. Each of the first two is 9.12.3c's
/// mandatory choice, and 9.12.3c is the whole of what makes them bite: the
/// Runner "must" choose an option **that can be fully resolved**, so a Runner
/// with 2[credit] cannot elect to pay 3 and a Runner with nothing installed
/// cannot elect to trash — and a Runner who can do neither faces an ability
/// that does nothing at all. 1.14.5 puts the choice with the player the
/// sentence names, which is why it is the Runner's and not the Corp's; the
/// last subroutine names nobody, so 1.14.4 leaves that one with the Corp.
pub fn fairchild_3_0() -> Card {
    card("Fairchild 3.0")
        .corp()
        .ice(5)
        .faction("Haas-Bioroid")
        .subtypes(&["Code Gate", "Bioroid", "AP"])
        .cost(6)
        .text("Lose [click][click][click]: Break up to 3 subroutines on this ice. Only the Runner can use this ability.")
        .text("[subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards.")
        .text("[subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards.")
        .text("[subroutine] Do 1 core damage or end the run.")
        .unimplemented("Lose [click][click][click]: Break up to 3 subroutines on this ice. Only the Runner can use this ability.")
        // The card prints this subroutine twice, so it is written twice.
        .subroutine([performed_by(
            Runner,
            choose_one([
                ("pay 3[credit]", vec![lose(Runner, 3)]),
                (
                    "trash 1 of their installed cards",
                    vec![trash(choose(1, &[installed_runner_card()]))],
                ),
            ]),
        )])
        .named("pay or trash")
        .subroutine([performed_by(
            Runner,
            choose_one([
                ("pay 3[credit]", vec![lose(Runner, 3)]),
                (
                    "trash 1 of their installed cards",
                    vec![trash(choose(1, &[installed_runner_card()]))],
                ),
            ]),
        )])
        .named("pay or trash again")
        .subroutine([choose_one([
            ("do 1 core damage", vec![core_damage(Corp, 1)]),
            ("end the run", vec![end_the_run()]),
        ])])
        .named("core damage or end the run")
        .build()
}

/// Vertigo — ICE: Code Gate. Rez 1, strength 1.
/// "When the Runner passes this ice, if they have no [click] remaining, they
///  cannot steal or trash Corp cards for the remainder of this run.
///  [subroutine] The Runner loses [click]."
///
/// UNIMPLEMENTED: the first sentence — and, as of this merge, NOT because
/// either half is unsayable any more. Both words landed, from two kernel
/// waves that ran in parallel and each knew only its own half:
///
///   * "if they have no [click] remaining" is a 9.6.5c requirement about a
///     NUMBER (1.11.3), and `at_most(clicks_of(Runner), 0)` now states it in
///     the direction the card prints it;
///   * "they cannot steal or trash Corp cards for the remainder of this run"
///     is 1.2.2's cannot over an act, a duration and a description, all of
///     which `ProhibitionScope::Matching` now carries.
///
/// The sentence is one instruction (9.11.3), so it stays marked until it is
/// written WHOLE — which is now a card-writing job and no longer a kernel
/// one.
///
/// The other half is no longer a blocker. "They cannot steal or trash Corp
/// cards for the remainder of this run" is 9.10.1's prohibition with the run
/// as its duration and a description for its scope, and the CR 1.2.2 wave
/// added all three of the pieces it wanted: stealing (7.5) and trashing
/// (7.1.5 / 1.19.4) are acts a "cannot" names, "Corp cards" is a description
/// read where the act is offered, and "they" is the player the prohibition
/// names — which matters here, because trashing is the one act BOTH players
/// perform and the Corp must go on trashing its own cards.
///
/// The sentence still carries `.unimplemented(…)` whole, because 9.11.3 makes
/// it ONE instruction: a requirement that cannot be stated is not a smaller
/// version of the sentence, it is a prohibition that would apply when the
/// printed one does not.
///
/// The subroutine is complete: 1.11.3b's loss, which is not a spend, and
/// which leaves a Runner with no clicks at zero rather than failing.
pub fn vertigo() -> Card {
    card("Vertigo")
        .corp()
        .ice(1)
        .faction("Haas-Bioroid")
        .subtypes(&["Code Gate"])
        .cost(1)
        .text("When the Runner passes this ice, if they have no [click] remaining, they cannot steal or trash Corp cards for the remainder of this run.")
        .text("[subroutine] The Runner loses [click].")
        .unimplemented("When the Runner passes this ice, if they have no [click] remaining, they cannot steal or trash Corp cards for the remainder of this run.")
        .subroutine([lose_clicks(Runner, 1)])
        .named("the runner loses a click")
        .build()
}

/// Drafter — ICE: Sentry. Rez 3, strength 3.
/// "[subroutine] You may add 1 card from Archives to HQ.
///  [subroutine] You may install 1 card from Archives or HQ, ignoring all
///  costs."
///
/// COMPLETE. One sentence each (9.11.3), and each sentence's "you may" is
/// 9.6.9d's optional part inside the instruction rather than an optional
/// ability: a subroutine resolves whether or not the Corp takes what it
/// offers.
///
/// "1 card from Archives" and "1 card from Archives or HQ" both name their
/// zones, which is exactly what 1.15.2c asks for before a description may
/// reach outside the play area — and "Archives **or** HQ" is one description
/// with two alternatives, not two descriptions, so it is one announcement and
/// the Corp picks from both piles at once.
///
/// "Ignoring all costs" is 1.16.5c: every element of the install cost goes,
/// 8.5.11a's 1[credit] per piece of ice already protecting the destination
/// included. The sentence states no destination, so the Corp declares one at
/// step 8.5.16b.
pub fn drafter() -> Card {
    card("Drafter")
        .corp()
        .ice(3)
        .faction("Haas-Bioroid")
        .subtypes(&["Sentry"])
        .cost(3)
        .text("[subroutine] You may add 1 card from Archives to HQ.")
        .text("[subroutine] You may install 1 card from Archives or HQ, ignoring all costs.")
        .subroutine([may(add_to_hand(choose(1, &[in_archives()])))])
        .named("archives to hq")
        .subroutine([may(install_ignoring_all_costs(
            choose(1, &[any_of(&[&[in_archives()], &[in_hand_of(Corp)]])]),
            InstallDest::DeclaredByInstaller,
        ))])
        .named("install for free")
        .build()
}

/// Tour Guide — ICE: Sentry. Rez 2, strength 0.
/// "This ice gains "[subroutine] End the run." for each rezzed asset."
///
/// COMPLETE. Permanently true rather than something that happens, so a static
/// declaration — and the count is the point of the card. CR 9.12.2b's
/// calculated quantity sits in a STATIC ability, which means it is never
/// evaluated once and remembered: 9.12.1d–e recompute an object's effective
/// characteristics from its printed ones every time they are read, so the
/// number of subroutines this ice has is a question asked afresh at every
/// checkpoint and every time the encounter looks for the next unbroken
/// subroutine. Rez an asset mid-encounter and the Runner faces one more;
/// trash one and the list shrinks. A lingering effect (9.10) would have been
/// the wrong shape — it would have been created once, with the count it had
/// then, and 9.10.1 would have kept it alive at that value.
///
/// 9.8.3d places them: a static ability ON THE ICE ITSELF that states no
/// order puts its subroutines after the printed ones (there are none), in the
/// order gained, and takes the LAST one back first when the count falls. That
/// category needs no 9.8.2c "order of your choice" declaration, and the card
/// prints no words asking for one.
///
/// 9.8.4b is what makes a subroutine gained during an encounter matter: it
/// arrives unbroken, so an asset rezzed after the Runner has broken everything
/// still costs them.
pub fn tour_guide() -> Card {
    card("Tour Guide")
        .corp()
        .ice(0)
        .faction("Weyland Consortium")
        .subtypes(&["Sentry"])
        .cost(2)
        .text("This ice gains \"[subroutine] End the run.\" for each rezzed asset.")
        .declares([gains_subroutines(
            per_card_matching(&[of_type(CardType::Asset), rezzed()]),
            [end_the_run()],
        )])
        .named("one end-the-run per rezzed asset")
        .build()
}

/// The deck so far, in the order `docs/vm/MEZZIE-QUEUE.md` lists it.
///
/// The identity and Rashida Jaheem are REUSED — they came out of the identity
/// queue and out of the Gauntlet deck respectively, and a card is written
/// once. The rest of the queue's 24 distinct cards arrive as waves land; a
/// card nobody has written yet is absent from this list rather than present
/// as a stub, so the list and the tick-boxes always say the same thing.
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::corp_haas_bioroid::asa_group(),
        global_food_initiative(),
        luminal_transubstantiation(),
        project_vacheron(),
        project_vitruvius(),
        estelle_moon(),
        jeeves_model_bioroids(),
        lakshmi_smartfabrics(),
        marilyn_campaign(),
        mca_austerity_policy(),
        mumba_temple(),
        super::gauntlet::rashida_jaheem(),
        spin_doctor(),
        enhanced_login_protocol(),
        flood_the_market(),
        friends_in_high_places(),
        fully_operational(),
        ash_2x3zb9cy(),
        manegarm_skunkworks(),
        tatu_bola(),
        vanilla(),
        fairchild_3_0(),
        vertigo(),
        drafter(),
        tour_guide(),
    ]
}

/// CR 1.5.4a: the pile is the RUNNER's — "a player may bring any number of
/// additional **Runner** identity cards along with their deck" — so a Corp
/// deck brings none.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
