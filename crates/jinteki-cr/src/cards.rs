//! The card layer: REAL cards, re-derived from printed text.
//!
//! DP-7c (`docs/vm/CORPUS.md` §3.1). Every function here returns one printed
//! card, built EXCLUSIVELY through the public vocabulary (`PrintedCard` +
//! `AbilityDef` + `Instruction` + `StaticDecl`) — ARCHITECTURE §12 rule 3 —
//! and carries the card's oracle text verbatim in its doc comment, quoted
//! from the reference checkout's own `data/cards.edn`. The kernel never
//! branches on a card name (§12 rule 1): this module is a dictionary, exactly
//! as `testkit` is, and the VM cannot tell where a `PrintedCard` came from.
//!
//! **Partial cards are marked, never faked.** Where a clause of the printed
//! text has no expression in the current vocabulary, the card carries the
//! clauses that do and the doc comment says `UNIMPLEMENTED:` with the missing
//! sentence. `corpus.rs`'s manifest test reads those markers, so the gap list
//! in CORPUS.md §5 cannot drift from the code. A partial card is legitimate
//! only while the missing clause is orthogonal to every test using it.

use crate::ability::{AbilityDef, AbilityFlag, Cost, StaticDecl, TriggerCond};
use crate::instr::{Instruction, Quantity, TargetFilter, TargetSpec};
use crate::object::{CardType, CounterKind, PrintedCard, Side};

// ---------------------------------------------------------------------------
// Corp — operations
// ---------------------------------------------------------------------------

/// Hedge Fund — Operation: Transaction. Cost 5.
/// "Gain 9[credit]."
pub fn hedge_fund() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Hedge Fund", Side::Corp, CardType::Operation);
    c.subtypes = vec!["Transaction"];
    c.cost = Some(5);
    c.abilities = vec![AbilityDef::play(vec![Instruction::GainCredits(Side::Corp, Quantity::c(9))])
        .labeled("hedge fund: gain 9 credits")];
    c
}

/// IPO — Operation: Terminal - Transaction. Cost 8.
/// "After you resolve this operation, end your action phase.
///  Gain 13[credit]."
pub fn ipo() -> PrintedCard {
    let mut c = PrintedCard::vanilla("IPO", Side::Corp, CardType::Operation);
    c.subtypes = vec!["Terminal", "Transaction"];
    c.cost = Some(8);
    c.abilities = vec![AbilityDef::play(vec![
        Instruction::GainCredits(Side::Corp, Quantity::c(13)),
        // 10.10: the terminal clause ends the action phase after the
        // operation resolves (5.6.2b's loop stops taking actions).
        Instruction::EndActionPhase(Side::Corp),
    ])
    .labeled("ipo: gain 13 credits, end your action phase")];
    c
}

/// Beanstalk Royalties — Operation: Transaction. Cost 0.
/// "Gain 3[credit]."
pub fn beanstalk_royalties() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Beanstalk Royalties", Side::Corp, CardType::Operation);
    c.subtypes = vec!["Transaction"];
    c.cost = Some(0);
    c.abilities = vec![AbilityDef::play(vec![Instruction::GainCredits(Side::Corp, Quantity::c(3))])
        .labeled("beanstalk royalties: gain 3 credits")];
    c
}

/// Cyberdex Trial — Operation. Cost 0.
/// "Purge virus counters."
pub fn cyberdex_trial() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Cyberdex Trial", Side::Corp, CardType::Operation);
    c.cost = Some(0);
    c.abilities = vec![AbilityDef::play(vec![Instruction::PurgeVirusCounters])
        .labeled("cyberdex trial: purge virus counters")];
    c
}

// ---------------------------------------------------------------------------
// Corp — ice
// ---------------------------------------------------------------------------

/// Ice Wall — ICE: Barrier. Rez 1, strength 1.
/// "You can advance this ice. It gets +1 strength for each hosted advancement
///  counter.
///  [subroutine] End the run."
pub fn ice_wall() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Ice Wall", Side::Corp, CardType::Ice);
    c.subtypes = vec!["Barrier"];
    c.cost = Some(1);
    c.strength = Some(1);
    c.abilities = vec![
        // 1.18.3's permission and 9.12.1b's strength modification are ONE
        // printed sentence pair and one static ability.
        AbilityDef::static_ability(vec![
            StaticDecl::CanBeAdvancedSelf,
            StaticDecl::SelfStrength(Quantity::base_plus_per_counter(
                1,
                1,
                CounterKind::Advancement,
            )),
        ])
        .labeled("ice wall: can be advanced, +1 strength per advancement counter"),
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run"),
    ];
    c
}

/// Vanilla — ICE: Barrier. Rez 0, strength 0.
/// "[subroutine] End the run."
pub fn vanilla_ice() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Vanilla", Side::Corp, CardType::Ice);
    c.subtypes = vec!["Barrier"];
    c.cost = Some(0);
    c.strength = Some(0);
    c.abilities =
        vec![AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run")];
    c
}

// ---------------------------------------------------------------------------
// Corp — assets, upgrades, agendas
// ---------------------------------------------------------------------------

/// PAD Campaign — Asset: Advertisement. Rez 2, trash 4.
/// "When your turn begins, gain 1[credit]."
pub fn pad_campaign() -> PrintedCard {
    let mut c = PrintedCard::vanilla("PAD Campaign", Side::Corp, CardType::Asset);
    c.subtypes = vec!["Advertisement"];
    c.cost = Some(2);
    c.trash_cost = Some(4);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Corp),
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("pad campaign: gain 1 credit")];
    c
}

/// Lt. Todachine — Asset. Rez 3, trash 2.
/// "Whenever you rez a piece of ice, give the Runner 1 tag."
///
/// UNIMPLEMENTED: none — but the trigger condition the kernel has is
/// "a card was rezzed", so the ice restriction is expressed as the
/// condition's own requirement below rather than as prose.
pub fn lt_todachine() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Lt. Todachine", Side::Corp, CardType::Asset);
    c.cost = Some(3);
    c.trash_cost = Some(2);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CorpRezzesCard { of_types: vec![CardType::Ice] },
        vec![Instruction::GainTags(1)],
        false,
    )
    .labeled("lt. todachine: give the runner 1 tag")];
    c
}

/// Breaker Bay Grid — Upgrade: Region. Rez 0, trash 2.
/// "The rez cost of each card in the root of this server is lowered by 5.
///  Limit 1 <strong>region</strong> per server."
///
/// UNIMPLEMENTED: both sentences. The kernel has no rez-cost modification
/// scoped to a server (`StaticDecl::InstallDiscount` is an INSTALL cost), and
/// no per-server subtype limit (3.6.1's region rule). The card's
/// characteristics — type, subtype, trash cost — are exact, which is what the
/// install tests use it for.
pub fn breaker_bay_grid() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Breaker Bay Grid", Side::Corp, CardType::Upgrade);
    c.subtypes = vec!["Region"];
    c.cost = Some(0);
    c.trash_cost = Some(2);
    c
}

/// Hostile Takeover — Agenda: Expansion - Liability. 2/1.
/// "When you score this agenda, gain 7[credit] and take 1 bad publicity."
pub fn hostile_takeover() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Hostile Takeover", Side::Corp, CardType::Agenda);
    c.subtypes = vec!["Expansion", "Liability"];
    c.cost = None;
    c.advancement_requirement = Some(2);
    c.agenda_points = Some(1);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfScored { requires: Vec::new() },
        vec![
            Instruction::GainCredits(Side::Corp, Quantity::c(7)),
            Instruction::TakeBadPublicity { side: Side::Corp, amount: Quantity::c(1) },
        ],
        false,
    )
    .labeled("hostile takeover: gain 7 credits and take 1 bad publicity")];
    c
}

/// Project Beale — Agenda: Research. 3/2.
/// "When you score this agenda, place 1 agenda counter on it for every 2
///  hosted advancement counters past 3.
///  This agenda is worth 1 more agenda point for each hosted agenda counter."
///
/// The first sentence IS the dividends keyword (10.13), which the kernel
/// carries as `PrintedCard::with_dividends`.
/// UNIMPLEMENTED: the second sentence — the kernel has no modification of an
/// agenda's POINT value (2.5), so a Beale scored with extra counters is worth
/// its printed 2. Orthogonal to the install/advance tests that use it.
pub fn project_beale() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Project Beale", Side::Corp, CardType::Agenda);
    c.subtypes = vec!["Research"];
    c.cost = None;
    c.advancement_requirement = Some(3);
    c.agenda_points = Some(2);
    c.with_dividends(1)
}

// ---------------------------------------------------------------------------
// Runner — events, resources, hardware
// ---------------------------------------------------------------------------

/// Sure Gamble — Event. Cost 5.
/// "Gain 9[credit]."
pub fn sure_gamble() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Sure Gamble", Side::Runner, CardType::Event);
    c.cost = Some(5);
    c.abilities =
        vec![AbilityDef::play(vec![Instruction::GainCredits(Side::Runner, Quantity::c(9))])
            .labeled("sure gamble: gain 9 credits")];
    c
}

/// Fan Site — Resource: Virtual. Install 0.
/// "Whenever the Corp scores an agenda, add Fan Site to your score area as an
///  agenda worth 0 agenda points."
///
/// UNIMPLEMENTED: the ability. The kernel has no instruction that moves a
/// non-agenda card into a score area (4.5/1.17.6), which is a movement of its
/// own. The card's characteristics are exact.
pub fn fan_site() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Fan Site", Side::Runner, CardType::Resource);
    c.subtypes = vec!["Virtual"];
    c.cost = Some(0);
    c
}

/// Bookmark — Hardware. Install 0.
/// "[click]: Host up to 3 cards from your grip facedown on this hardware
///  (you may look at these cards at any time).
///  [click]: Add all hosted cards to your grip.
///  [trash]: Add all hosted cards to your grip."
///
/// UNIMPLEMENTED: the facedown status of the hosted cards and the "you may
/// look at these cards at any time" permission (1.21.2a already gives the
/// controller that, so nothing is lost); the hosting is otherwise exact.
pub fn bookmark() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Bookmark", Side::Runner, CardType::Hardware);
    c.cost = Some(0);
    let hosted_to_grip = || Instruction::AddCardsToHand {
        cards: TargetSpec::Choose {
            count: Quantity::Count(TargetFilter::HostedOnSource),
            criteria: vec![TargetFilter::HostedOnSource],
        },
    };
    c.abilities = vec![
        AbilityDef::paid(
            Cost { clicks: 1, ..Default::default() },
            vec![Instruction::HostCards {
                cards: TargetSpec::Choose {
                    count: Quantity::c(3),
                    criteria: vec![TargetFilter::CardsInHandOf(Side::Runner)],
                },
                host: TargetSpec::SelfSource,
            }],
        )
        .labeled("bookmark: host up to 3 cards from your grip"),
        AbilityDef::paid(Cost { clicks: 1, ..Default::default() }, vec![hosted_to_grip()])
            .labeled("bookmark: add all hosted cards to your grip"),
        AbilityDef::paid(Cost::trash_self(), vec![hosted_to_grip()])
            .labeled("bookmark: [trash] add all hosted cards to your grip"),
    ];
    c
}

/// Misdirection — Program. Install 0, 1[mu], strength 0.
/// "[click], [click], X[credit]: Remove X tags."
pub fn misdirection() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Misdirection", Side::Runner, CardType::Program);
    c.cost = Some(0);
    c.memory_cost = Some(1);
    c.strength = Some(0);
    let mut cost = Cost { clicks: 2, ..Default::default() };
    // 1.16.2c: X is announced when the cost is paid, and the Runner cannot
    // announce more than they can pay or more tags than they have.
    cost.credits = Quantity::AnnouncedX;
    cost.x_restriction = Some(Quantity::RunnerTags);
    c.abilities = vec![AbilityDef::paid(
        cost,
        vec![Instruction::RemoveTags(Quantity::AnnouncedX)],
    )
    .labeled("misdirection: remove X tags")];
    c
}

// ---------------------------------------------------------------------------
// Runner — viruses (the purge corner of the corpus)
// ---------------------------------------------------------------------------

/// Clot — Program: Virus. Install 2, 1[mu].
/// "The Corp cannot score an agenda during the same turn they installed that
///  agenda.
///  When the Corp purges virus counters, trash this program."
///
/// UNIMPLEMENTED: the first sentence. The kernel has no prohibition on
/// SCORING scoped to when the agenda was installed (9.1.9's restrictions
/// cover abilities, not the (S) option), and the score-turn history it would
/// read is not recorded. The purge clause is exact.
pub fn clot() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Clot", Side::Runner, CardType::Program);
    c.subtypes = vec!["Virus"];
    c.cost = Some(2);
    c.memory_cost = Some(1);
    c.strength = None;
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CorpPurgesVirusCounters,
        vec![Instruction::TrashSelf],
        false,
    )
    .labeled("clot: trash this program")];
    c
}

/// Imp — Program: Virus. Install 2, 1[mu].
/// "When you install this program, place 2 virus counters on it.
///  Access, once per turn → <strong>Hosted virus counter:</strong> Trash the
///  card you are accessing."
pub fn imp() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Imp", Side::Runner, CardType::Program);
    c.subtypes = vec!["Virus"];
    c.cost = Some(2);
    c.memory_cost = Some(1);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfInstalled,
            vec![Instruction::PlaceCounters {
                target: TargetSpec::SelfSource,
                kind: CounterKind::Virus,
                amount: Quantity::c(2),
            }],
            false,
        )
        .labeled("imp: place 2 virus counters"),
        AbilityDef::paid(
            Cost::spend_counters(CounterKind::Virus, 1),
            vec![Instruction::TrashCards(TargetSpec::AccessedCard)],
        )
        .with_flag(AbilityFlag::Access)
        .with_flag(AbilityFlag::OncePerTurn)
        .labeled("imp: trash the card you are accessing"),
    ];
    c
}

/// Botulus — Program: Virus - Trojan. Install 2, 1[mu].
/// "Install only on a piece of ice. (If the host ice is uninstalled, this
///  program is trashed.)
///  When you install this program and when your turn begins, place 1 virus
///  counter on this program.
///  <strong>Hosted virus counter:</strong> Break 1 subroutine on host ice."
pub fn botulus() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Botulus", Side::Runner, CardType::Program);
    c.subtypes = vec!["Virus", "Trojan"];
    c.cost = Some(2);
    c.memory_cost = Some(1);
    let place_one = || Instruction::PlaceCounters {
        target: TargetSpec::SelfSource,
        kind: CounterKind::Virus,
        amount: Quantity::c(1),
    };
    c.abilities = vec![
        // 1.13.6c: the install restriction is a static declaration, so the
        // install's 8.5.16b destination declaration offers only ice.
        AbilityDef::static_ability(vec![StaticDecl::InstallOnlyHostedOn(vec![
            TargetFilter::CardTypeIs(CardType::Ice),
        ])])
        .labeled("botulus: install only on a piece of ice"),
        AbilityDef::conditional(TriggerCond::SelfInstalled, vec![place_one()], false)
            .labeled("botulus: place 1 virus counter (installed)"),
        AbilityDef::conditional(TriggerCond::TurnBegins(Side::Runner), vec![place_one()], false)
            .labeled("botulus: place 1 virus counter (turn begins)"),
        AbilityDef::paid(
            Cost::spend_counters(CounterKind::Virus, 1),
            vec![Instruction::BreakSubroutines {
                subs: crate::instr::SubroutineSpec::Chosen { count: Quantity::c(1), up_to: false },
            }],
        )
        .labeled("botulus: break 1 subroutine on host ice"),
    ];
    c
}
