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

use crate::ability::{AbilityDef, AbilityFlag, Cost, StaticDecl, TriggerCond, TriggerRequirement};
use crate::effects::DamageKind;
use crate::instr::{Instruction, Quantity, TargetFilter, TargetSpec};
use crate::object::{CardType, CounterKind, PrintedCard, ServerId, Side};

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

/// Extract — Operation: Transaction. Cost 3. COMPLETE.
/// "Gain 6[credit]. You may trash 1 of your installed cards to gain
///  3[credit]."
///
/// The second sentence is 1.16.11a's "you may pay [cost] to [effect]", where
/// the cost is trashing a card the Corp chooses — so with nothing installed
/// there is no choice to make (1.16.1b) and the Corp is never asked.
pub fn extract() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Extract", Side::Corp, CardType::Operation);
    c.subtypes = vec!["Transaction"];
    c.cost = Some(3);
    c.abilities = vec![AbilityDef::play(vec![
        Instruction::GainCredits(Side::Corp, Quantity::c(6)),
        Instruction::NestedCostThen {
            cost: Cost {
                trash_matching: Some((1, vec![TargetFilter::InstalledCorpCard])),
                ..Default::default()
            },
            effect: Box::new(Instruction::GainCredits(Side::Corp, Quantity::c(3))),
            payer: Some(Side::Corp),
        },
    ])
    .labeled("extract: gain 6, then may trash to gain 3")];
    c
}

/// Neural EMP — Operation: Gray Ops. Cost 2. COMPLETE.
/// "Play only if the Runner made a run during their last turn.
///  Do 1 net damage."
///
/// 9.1.8c: the first sentence is a static ability that modifies WHEN the card
/// can be played, so it is active while the card sits in HQ — which is the
/// only place it could ever matter.
pub fn neural_emp() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Neural EMP", Side::Corp, CardType::Operation);
    c.subtypes = vec!["Gray Ops"];
    c.cost = Some(2);
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::PlayOnlyIf(vec![
            TriggerRequirement::RunnerMadeRunLastTurn { successful_only: false },
        ])])
        .labeled("neural emp: play only if the Runner made a run during their last turn"),
        AbilityDef::play(vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(1),
            responsible: Side::Corp,
        }])
        .labeled("neural emp: do 1 net damage"),
    ];
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

/// Enigma — ICE: Code Gate. Rez 3, strength 2. COMPLETE.
/// "[subroutine] The Runner loses [click].
///  [subroutine] End the run."
///
/// 1.11.3b: losing clicks is not spending them, and a Runner with none left
/// simply stays at zero — the subroutine still resolves.
pub fn enigma() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Enigma", Side::Corp, CardType::Ice);
    c.subtypes = vec!["Code Gate"];
    c.cost = Some(3);
    c.strength = Some(2);
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::LoseClicks(Side::Runner, Quantity::c(1))])
            .labeled("[sub] The Runner loses [click]"),
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run"),
    ];
    c
}

/// Tithe — ICE: Sentry - AP. Rez 1, strength 1. COMPLETE.
/// "[subroutine] Do 1 net damage.
///  [subroutine] Gain 1[credit]."
pub fn tithe() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Tithe", Side::Corp, CardType::Ice);
    c.subtypes = vec!["Sentry", "AP"];
    c.cost = Some(1);
    c.strength = Some(1);
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(1),
            responsible: Side::Corp,
        }])
        .labeled("[sub] Do 1 net damage"),
        AbilityDef::subroutine(vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))])
            .labeled("[sub] Gain 1 credit"),
    ];
    c
}

/// Pup — ICE: Sentry - AP. Rez 1, strength 0. COMPLETE.
/// "[subroutine] Do 1 net damage unless the Runner pays 1[credit].
///  [subroutine] Do 1 net damage unless the Runner pays 1[credit]."
///
/// 1.16.11b: paying suppresses the effect; declining (or being unable to pay)
/// makes it the next instruction.
pub fn pup() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Pup", Side::Corp, CardType::Ice);
    c.subtypes = vec!["Sentry", "AP"];
    c.cost = Some(1);
    c.strength = Some(0);
    let sub = || {
        AbilityDef::subroutine(vec![Instruction::NestedCostUnless {
            cost: Cost::credits(1),
            effect: Box::new(Instruction::Damage {
                kind: DamageKind::Net,
                amount: Quantity::c(1),
                responsible: Side::Corp,
            }),
            payer: Some(Side::Runner),
        }])
        .labeled("[sub] Do 1 net damage unless the Runner pays 1")
    };
    c.abilities = vec![sub(), sub()];
    c
}

/// Paper Wall — ICE: Barrier. Rez 0, strength 1. COMPLETE.
/// "When the Runner fully breaks this ice, trash it.
///  [subroutine] End the run."
pub fn paper_wall() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Paper Wall", Side::Corp, CardType::Ice);
    c.subtypes = vec!["Barrier"];
    c.cost = Some(0);
    c.strength = Some(1);
    c.abilities = vec![
        AbilityDef::conditional(TriggerCond::SelfFullyBroken, vec![Instruction::TrashSelf], false)
            .labeled("paper wall: trash it"),
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

/// Hostile Infrastructure — Asset. Rez 5, trash 5. COMPLETE.
/// "Whenever the Runner trashes a Corp card (including Hostile
///  Infrastructure), do 1 net damage."
pub fn hostile_infrastructure() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Hostile Infrastructure", Side::Corp, CardType::Asset);
    c.cost = Some(5);
    c.trash_cost = Some(5);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerTrashesCorpCard,
        vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(1),
            responsible: Side::Corp,
        }],
        false,
    )
    .labeled("hostile infrastructure: do 1 net damage")];
    c
}

/// Rashida Jaheem — Asset: Character. Rez 0, trash 1. COMPLETE.
/// "When your turn begins, you may trash Rashida Jaheem to gain 3[credit] and
///  draw 3 cards."
///
/// 9.6.9: "you may" makes the conditional ability optional; the trash is
/// 1.16.11a's cost, paid before the two effects — and the draw is 8.4.2's
/// procedure, so a deck that runs out during it loses the game at the next
/// checkpoint (10.3.1b) rather than silently drawing fewer.
pub fn rashida_jaheem() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Rashida Jaheem", Side::Corp, CardType::Asset);
    c.subtypes = vec!["Character"];
    c.cost = Some(0);
    c.trash_cost = Some(1);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Corp),
        vec![Instruction::NestedCostThen {
            cost: Cost::trash_self(),
            effect: Box::new(Instruction::Combined(vec![
                Instruction::GainCredits(Side::Corp, Quantity::c(3)),
                Instruction::Draw(Side::Corp, 3),
            ])),
            payer: Some(Side::Corp),
        }],
        true,
    )
    .labeled("rashida jaheem: trash to gain 3 and draw 3")];
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

/// Government Takeover — Agenda: Expansion. 9/6. Unique.
/// "[click]: Gain 3[credit].
///  Limit 1 Government Takeover per deck."
///
/// The paid ability is a [click]-cost ability on a card in the score area,
/// which 9.1.8a keeps active — that is the whole point of the card.
/// UNIMPLEMENTED: the per-deck limit. It is a DECKBUILDING restriction
/// (§1.4), not an ability of a card in play; the kernel's `deck` module
/// validates influence and size and nothing names a per-title limit.
pub fn government_takeover() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Government Takeover", Side::Corp, CardType::Agenda);
    c.subtypes = vec!["Expansion"];
    c.cost = None;
    c.unique = true;
    c.advancement_requirement = Some(9);
    c.agenda_points = Some(6);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Default::default() },
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(3))],
    )
    .labeled("government takeover: gain 3 credits")];
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

/// Easy Mark — Event: Job. Cost 0. COMPLETE.
/// "Gain 3[credit]."
pub fn easy_mark() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Easy Mark", Side::Runner, CardType::Event);
    c.subtypes = vec!["Job"];
    c.cost = Some(0);
    c.abilities =
        vec![AbilityDef::play(vec![Instruction::GainCredits(Side::Runner, Quantity::c(3))])
            .labeled("easy mark: gain 3 credits")];
    c
}

/// Diesel — Event. Cost 0. COMPLETE.
/// "Draw 3 cards."
///
/// 8.4.2's procedure, not a one-shot: `Instruction::Draw` expands into
/// 8.4.5's steps.
pub fn diesel() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Diesel", Side::Runner, CardType::Event);
    c.cost = Some(0);
    c.abilities = vec![AbilityDef::play(vec![Instruction::Draw(Side::Runner, 3)])
        .labeled("diesel: draw 3 cards")];
    c
}

/// Dirty Laundry — Event: Run. Cost 2. COMPLETE.
/// "Run any server. When that run ends, if it was successful, gain
///  5[credit]."
///
/// "Run any server" is 6.7.4a's unrestricted set, with the attacked server
/// announced by the Runner at step 6.9.1a. The second sentence is a delayed
/// conditional (9.6.13) whose window is that run — so it is armed from inside
/// 6.7.4's "if successful" clause, which is both where the "if it was
/// successful" test is settled and the only place a `ThisRun` duration can
/// bind (the run does not exist yet while the play's own instructions are
/// being read, and everything after `InitiateRun` resolves only once the run
/// is over — 9.2.4d's LIFO nesting).
pub fn dirty_laundry() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Dirty Laundry", Side::Runner, CardType::Event);
    c.subtypes = vec!["Run"];
    c.cost = Some(2);
    c.abilities = vec![AbilityDef::play(vec![Instruction::run_any_server(vec![
        Instruction::CreateDelayedConditional {
            def: Box::new(
                AbilityDef::conditional(
                    TriggerCond::RunEnds { successful_only: true },
                    vec![Instruction::GainCredits(Side::Runner, Quantity::c(5))],
                    false,
                )
                .labeled("dirty laundry: gain 5 credits"),
            ),
            duration: crate::lingering::WantedDuration::ThisRun,
        },
    ])])
    .labeled("dirty laundry: run any server")];
    c
}

/// Infiltration — Event. Cost 0. COMPLETE.
/// "Gain 2[credit] or expose 1 card."
///
/// 9.11.4g: an "or" is one instruction with two optioned effects, and the
/// player carrying it out chooses which to resolve.
pub fn infiltration() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Infiltration", Side::Runner, CardType::Event);
    c.cost = Some(0);
    c.abilities = vec![AbilityDef::play(vec![Instruction::ChooseOne {
        options: vec![
            (
                "gain 2 credits",
                vec![Instruction::GainCredits(Side::Runner, Quantity::c(2))],
            ),
            (
                "expose 1 card",
                vec![Instruction::ExposeCards {
                    cards: TargetSpec::Choose {
                        count: Quantity::c(1),
                        criteria: vec![TargetFilter::InstalledCorpCard],
                    },
                }],
            ),
        ],
    }])
    .labeled("infiltration: gain 2 credits or expose 1 card")];
    c
}

/// Daily Casts — Resource. Install 3. COMPLETE.
/// "When you install this resource, load 8[credit] onto it. When it is empty,
///  trash it.
///  When your turn begins, take 2[credit] from this resource."
///
/// 1.10.3a: credits taken from a card ENTER the pool, so this is a gain;
/// 1.13.3 keeps the hosted ones out of every "credits you have" count until
/// then. "When it is empty" is a condition on the source's counters, not a
/// step of the take.
pub fn daily_casts() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Daily Casts", Side::Runner, CardType::Resource);
    c.cost = Some(3);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfInstalled,
            // 10.9.1: "LOAD 8[credit] onto it" — loading is placing that
            // remembers the kind, which is what "when it is empty" (10.9.2)
            // is linked to.
            vec![Instruction::LoadCounters {
                target: TargetSpec::SelfSource,
                kind: CounterKind::Credit,
                amount: Quantity::c(8),
            }],
            false,
        )
        .labeled("daily casts: load 8 credits"),
        AbilityDef::conditional(
            TriggerCond::SelfEmpty { kind: CounterKind::Credit },
            vec![Instruction::TrashSelf],
            false,
        )
        .labeled("daily casts: trash it when empty"),
        AbilityDef::conditional(
            TriggerCond::TurnBegins(Side::Runner),
            vec![Instruction::TakeHostedCredits {
                from: TargetSpec::SelfSource,
                amount: Quantity::c(2),
                to: Side::Runner,
            }],
            false,
        )
        .labeled("daily casts: take 2 credits"),
    ];
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

/// Magnum Opus — Program. Install 5, 2[mu], strength 0. COMPLETE.
/// "[click]: Gain 2[credit]."
///
/// 1.11.3c: a paid ability beginning with [click] IS an action (5.2.1), so it
/// is offered in the action window and nowhere else.
pub fn magnum_opus() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Magnum Opus", Side::Runner, CardType::Program);
    c.cost = Some(5);
    c.memory_cost = Some(2);
    c.strength = Some(0);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Default::default() },
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(2))],
    )
    .labeled("magnum opus: gain 2 credits")];
    c
}

/// Rezeki — Program. Install 2, 1[mu], strength 0. COMPLETE.
/// "When your turn begins, gain 1[credit]."
pub fn rezeki() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Rezeki", Side::Runner, CardType::Program);
    c.cost = Some(2);
    c.memory_cost = Some(1);
    c.strength = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Runner),
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        false,
    )
    .labeled("rezeki: gain 1 credit")];
    c
}

/// Mimic — Program: Icebreaker - Killer. Install 3, 1[mu], strength 3.
/// COMPLETE.
/// "Interface → <strong>1[credit]:</strong> Break 1 <strong>sentry</strong>
///  subroutine."
///
/// The icebreaker class with no pump: 9.3.6c's [interface] strength gate and
/// 9.5.6c's subtype restriction, and nothing else.
pub fn mimic() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Mimic", Side::Runner, CardType::Program);
    c.subtypes = vec!["Icebreaker", "Killer"];
    c.cost = Some(3);
    c.memory_cost = Some(1);
    c.strength = Some(3);
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![Instruction::BreakSubroutines {
            subs: crate::instr::SubroutineSpec::Chosen { count: Quantity::c(1), up_to: false },
        }],
    )
    .with_flag(AbilityFlag::Interface)
    .with_timing(crate::ability::TimingRestriction::EncounterOnly {
        required_subtype: Some("Sentry"),
    })
    .labeled("mimic: break 1 sentry subroutine")];
    c
}

/// Corroder — Program: Icebreaker - Fracter. Install 2, 1[mu], strength 2.
/// "Interface → <strong>1[credit]:</strong> Break 1 <strong>barrier</strong>
///  subroutine.
///  <strong>1[credit]:</strong> +1 strength."
///
/// The two abilities are the whole icebreaker class: 9.3.6c's [interface]
/// flag (usable only while the breaker's strength is at least the encountered
/// ice's), 9.5.6c's restriction to an encounter with ice of a stated subtype,
/// and a strength modification whose duration is the implicit one 3.9.5b/d
/// gives a pump — the current encounter.
pub fn corroder() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Corroder", Side::Runner, CardType::Program);
    c.subtypes = vec!["Icebreaker", "Fracter"];
    c.cost = Some(2);
    c.memory_cost = Some(1);
    c.strength = Some(2);
    c.abilities = vec![
        AbilityDef::paid(
            Cost::credits(1),
            vec![Instruction::BreakSubroutines {
                subs: crate::instr::SubroutineSpec::Chosen {
                    count: Quantity::c(1),
                    up_to: false,
                },
            }],
        )
        .with_flag(AbilityFlag::Interface)
        .with_timing(crate::ability::TimingRestriction::EncounterOnly {
            required_subtype: Some("Barrier"),
        })
        .labeled("corroder: break 1 barrier subroutine"),
        AbilityDef::paid(
            Cost::credits(1),
            vec![Instruction::ModifyStrength {
                target: TargetSpec::SelfSource,
                amount: 1,
                duration: None,
            }],
        )
        .labeled("corroder: +1 strength"),
    ];
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

/// Cache — Program: Virus. Install 1, 1[mu], strength 0. COMPLETE.
/// "Place 3 virus counters on Cache when it is installed.
///  <strong>Hosted virus counter:</strong> Gain 1[credit]."
pub fn cache() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Cache", Side::Runner, CardType::Program);
    c.subtypes = vec!["Virus"];
    c.cost = Some(1);
    c.memory_cost = Some(1);
    c.strength = Some(0);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfInstalled,
            vec![Instruction::PlaceCounters {
                target: TargetSpec::SelfSource,
                kind: CounterKind::Virus,
                amount: Quantity::c(3),
            }],
            false,
        )
        .labeled("cache: place 3 virus counters"),
        AbilityDef::paid(
            Cost::spend_counters(CounterKind::Virus, 1),
            vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        )
        .labeled("cache: gain 1 credit"),
    ];
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

// ---------------------------------------------------------------------------
// Runner — Criminal staples (the Andromeda deck's spine)
// ---------------------------------------------------------------------------

/// Account Siphon — Event: Run - Sabotage. Cost 0. COMPLETE.
/// "Run HQ. If successful, instead of breaching HQ, you may force the Corp to
///  lose up to 5[credit], then you gain 2[credit] for each credit lost and
///  take 2 tags."
///
/// The whole card is CR machinery that already exists: an initiated run whose
/// effect carries the "if successful" ability (6.7.4), an OPTIONAL breach
/// replacement decided at step 6.9.5b (6.7.4c/9.9.2), a forced loss that
/// takes only what the pool holds (1.10.3b — the "up to"), a gain calculated
/// from the credits ACTUALLY lost, and tags that go through the ordinary
/// imminence pipeline, which is exactly what makes them avoidable.
pub fn account_siphon() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Account Siphon", Side::Runner, CardType::Event);
    c.subtypes = vec!["Run", "Sabotage"];
    c.cost = Some(0);
    c.abilities = vec![AbilityDef::play(vec![Instruction::InitiateRun {
        server: Some(ServerId::Hq),
        allowed: crate::instr::RunServerSet::These(vec![ServerId::Hq]),
        if_successful: vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::Replacement {
                applies_to: crate::effects::EffectClass::Breach,
                with: crate::lingering::ReplacementTransform::SuppressAndResolve(vec![
                    Instruction::LoseCredits(Side::Corp, 5),
                    Instruction::GainCredits(
                        Side::Runner,
                        Quantity::Times(2, Box::new(Quantity::CreditsLostThisAbility(Side::Corp))),
                    ),
                    Instruction::GainTags(2),
                ]),
                optional: true,
            },
            duration: crate::lingering::WantedDuration::ThisRun,
        }],
    }])
    .labeled("account siphon: run hq")];
    c
}

/// Desperado — Hardware: Console. Cost 3. Unique. COMPLETE.
/// "+1[mu]
///  Gain 1[credit] whenever you make a successful run.
///  Limit 1 console per player."
///
/// (The console limit is checkpoint step 10.3.1d, driven by `console: true`.)
pub fn desperado() -> PrintedCard {
    let mut c = PrintedCard::vanilla("Desperado", Side::Runner, CardType::Hardware);
    c.subtypes = vec!["Console"];
    c.cost = Some(3);
    c.unique = true;
    c.console = true;
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::MemoryLimitMod(1)])
            .labeled("desperado: +1 memory"),
        AbilityDef::conditional(
            TriggerCond::MakesSuccessfulRun,
            vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
            false,
        )
        .labeled("desperado: gain 1 credit (successful run)"),
    ];
    c
}
