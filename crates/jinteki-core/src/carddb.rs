//! The card database: printed data + declarative behavior for the playable pool.
//!
//! Every definition mirrors the reference implementation in
//! `jinteki-reference/src/clj/game/cards/*.clj` (see DESIGN.md Appendix B pin).
//! This table is the denotation target of the future designer DSL: a card is a
//! row of data (triggers × effect sequences), and the engine gives the data
//! meaning through `ir::fire_event`.

use crate::types::*;

use Amount::{Fixed, PerAdvancement, RunnerHandSize};
use CardType::*;
use CounterKind::{Agenda as AgendaCounter, Credit as CreditCounter, Power, Virus};
use Effect as E;
use IceSubtype::*;
use ServerFilter::Central;
use Side::*;
use SubEffect::{CorpGainCredits, NetDamage, RunnerLosesClick, TrashProgram};
use Trigger as T;

const ETR: SubEffect = SubEffect::EndTheRun;
const fn when(trigger: Trigger, effects: &'static [Effect]) -> TriggeredAbility {
    TriggeredAbility::when(trigger, effects)
}

pub const CARDS: &[CardDef] = &[
    // ── Identities ─────────────────────────────────────────────────────────
    CardDef {
        subtypes: &["Megacorp"],
        // "Whenever you play a transaction operation, gain 1 credit."
        triggered: &[when(
            T::PlayOperationWithSubtype("Transaction"),
            &[E::GainCredits(Corp, 1)],
        )],
        ..CardDef::blank("Weyland Consortium: Building a Better World", Corp, Identity)
    },
    CardDef {
        subtypes: &["Natural"],
        ..CardDef::blank("The Catalyst: Convention Breaker", Runner, Identity)
    },
    CardDef {
        subtypes: &["Cyborg"],
        // "The first time each turn you make a successful run on HQ, gain 2."
        triggered: &[TriggeredAbility {
            trigger: T::SuccessfulRun(ServerFilter::Hq),
            condition: Condition::Always,
            once_per_turn: true,
            effects: &[E::GainCredits(Runner, 2)],
        }],
        ..CardDef::blank("Gabriel Santiago: Consummate Professional", Runner, Identity)
    },
    // ── Corp: operations ───────────────────────────────────────────────────
    CardDef {
        cost: 5,
        subtypes: &["Transaction"],
        triggered: &[when(T::OnPlaySelf, &[E::GainCredits(Corp, 9)])],
        ..CardDef::blank("Hedge Fund", Corp, Operation)
    },
    CardDef {
        cost: 0,
        subtypes: &["Transaction"],
        triggered: &[when(T::OnPlaySelf, &[E::GainCredits(Corp, 3)])],
        ..CardDef::blank("Beanstalk Royalties", Corp, Operation)
    },
    CardDef {
        cost: 2,
        subtypes: &["Gray Ops"],
        // "Play only if the Runner made a successful run during their last
        // turn. Trace 3 - give the Runner 1 tag."
        play_condition: Some(Condition::RunnerSuccessfulRunLastTurn),
        triggered: &[when(
            T::OnPlaySelf,
            &[E::Trace {
                base: 3,
                on_success: &[E::GainTags(Fixed(1))],
                on_fail: &[],
            }],
        )],
        ..CardDef::blank("SEA Source", Corp, Operation)
    },
    // ── Corp: assets ───────────────────────────────────────────────────────
    CardDef {
        cost: 2,
        trash_cost: Some(4),
        subtypes: &["Advertisement"],
        triggered: &[when(T::TurnBegins(Corp), &[E::GainCredits(Corp, 1)])],
        ..CardDef::blank("PAD Campaign", Corp, Asset)
    },
    CardDef {
        cost: 2,
        trash_cost: Some(3),
        triggered: &[when(T::OnRezSelf, &[E::PlaceCounters(CreditCounter, 15)])],
        click_ability: Some(ClickAbility::TakeCredits(3)),
        ..CardDef::blank("Regolith Mining License", Corp, Asset)
    },
    CardDef {
        cost: 4,
        trash_cost: Some(3),
        subtypes: &["Advertisement"],
        // "Load 12 credits when rezzed; take 3 when your turn begins; trash
        // when empty" (the reference's `campaign 12 3`).
        triggered: &[
            when(T::OnRezSelf, &[E::PlaceCounters(CreditCounter, 12)]),
            when(T::TurnBegins(Corp), &[E::TakeCreditsFromSelf(3)]),
        ],
        ..CardDef::blank("Adonis Campaign", Corp, Asset)
    },
    CardDef {
        cost: 0,
        trash_cost: Some(0),
        subtypes: &["Ambush"],
        // "When the Runner accesses this asset anywhere except in Archives,
        // you may pay 4 credits: give the Runner 1 tag and do 3 net damage."
        triggered: &[when(
            T::OnAccessSelf { installed_only: false },
            &[E::Optional {
                prompt: "Pay 4 [Credits] to use Snare! ability?",
                cost: 4,
                yes: &[E::GainTags(Fixed(1)), E::Damage(DamageKind::Net, Fixed(3))],
                no: &[],
            }],
        )],
        ..CardDef::blank("Snare!", Corp, Asset)
    },
    CardDef {
        cost: 0,
        trash_cost: Some(0),
        subtypes: &["Ambush", "Facility"],
        advanceable: true,
        // advance-ambush 0: tags equal to hosted advancement counters.
        triggered: &[TriggeredAbility {
            trigger: T::OnAccessSelf { installed_only: true },
            condition: Condition::AdvancementPositive,
            once_per_turn: false,
            effects: &[E::Optional {
                prompt: "Use Ghost Branch to give the Runner tags?",
                cost: 0,
                yes: &[E::GainTags(PerAdvancement(1))],
                no: &[],
            }],
        }],
        ..CardDef::blank("Ghost Branch", Corp, Asset)
    },
    CardDef {
        cost: 0,
        trash_cost: Some(0),
        subtypes: &["Ambush", "Research"],
        advanceable: true,
        // advance-ambush 1: 2 net damage per advancement counter.
        triggered: &[TriggeredAbility {
            trigger: T::OnAccessSelf { installed_only: true },
            condition: Condition::AdvancementPositive,
            once_per_turn: false,
            effects: &[E::Optional {
                prompt: "Pay 1 [Credits] to use Project Junebug ability?",
                cost: 1,
                yes: &[E::Damage(DamageKind::Net, PerAdvancement(2))],
                no: &[],
            }],
        }],
        ..CardDef::blank("Project Junebug", Corp, Asset)
    },
    CardDef {
        cost: 0,
        trash_cost: Some(0),
        subtypes: &["Ambush"],
        advanceable: true,
        // advance-ambush 3: 1 core damage per advancement counter.
        triggered: &[TriggeredAbility {
            trigger: T::OnAccessSelf { installed_only: true },
            condition: Condition::AdvancementPositive,
            once_per_turn: false,
            effects: &[E::Optional {
                prompt: "Pay 3 [Credits] to use Cerebral Overwriter ability?",
                cost: 3,
                yes: &[E::Damage(DamageKind::Brain, PerAdvancement(1))],
                no: &[],
            }],
        }],
        ..CardDef::blank("Cerebral Overwriter", Corp, Asset)
    },
    CardDef {
        cost: 0,
        trash_cost: Some(2),
        subtypes: &["Ambush", "Psi"],
        // Psi game on access-while-installed or expose: if bids differ,
        // net damage equal to the number of cards in the grip.
        triggered: &[
            TriggeredAbility {
                trigger: T::OnAccessSelf { installed_only: true },
                condition: Condition::Always,
                once_per_turn: false,
                effects: PSYCHIC_FIELD_PSI,
            },
            when(T::OnExposeSelf, PSYCHIC_FIELD_PSI),
        ],
        ..CardDef::blank("Psychic Field", Corp, Asset)
    },
    // ── Corp: agendas ──────────────────────────────────────────────────────
    CardDef {
        advancement_requirement: Some(4),
        agenda_points: Some(2),
        subtypes: &["Expansion"],
        triggered: &[when(T::OnScoreSelf, &[E::GainCredits(Corp, 7)])],
        ..CardDef::blank("Offworld Office", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(2),
        agenda_points: Some(1),
        subtypes: &["Expansion"],
        triggered: &[when(
            T::OnScoreSelf,
            &[E::GainCredits(Corp, 7), E::GainBadPub(1)],
        )],
        ..CardDef::blank("Hostile Takeover", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(5),
        agenda_points: Some(3),
        subtypes: &["Security"],
        triggered: &[when(T::OnScoreSelf, &[E::RezIceIgnoringCosts])],
        ..CardDef::blank("Priority Requisition", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(3),
        agenda_points: Some(1),
        subtypes: &["Expansion"],
        triggered: &[when(
            T::OnScoreSelf,
            &[E::Optional {
                prompt: "Draw 2 cards?",
                cost: 0,
                yes: &[E::Draw(Corp, 2)],
                no: &[],
            }],
        )],
        statics: &[StaticMod::MaxHandSize(2)],
        ..CardDef::blank("Superconducting Hub", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(4),
        agenda_points: Some(2),
        subtypes: &["Initiative"],
        // "When you score, place 1 agenda counter. Hosted agenda counter:
        // end the run."
        triggered: &[when(T::OnScoreSelf, &[E::PlaceCounters(AgendaCounter, 1)])],
        counter_abilities: &[CounterAbility {
            label: "end the run",
            cost: (AgendaCounter, 1),
            timing: AbilityTiming::DuringRun,
            effects: &[E::EndTheRun],
        }],
        ..CardDef::blank("Nisei MK II", Corp, Agenda)
    },
    // ── Corp: ice ──────────────────────────────────────────────────────────
    CardDef {
        cost: 1,
        strength: Some(1),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier"],
        advanceable: true,
        subroutines: &[ETR],
        ..CardDef::blank("Ice Wall", Corp, Ice)
    },
    CardDef {
        cost: 0,
        strength: Some(0),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier"],
        subroutines: &[ETR],
        ..CardDef::blank("Vanilla", Corp, Ice)
    },
    CardDef {
        cost: 3,
        strength: Some(3),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier"],
        subroutines: &[ETR],
        ..CardDef::blank("Wall of Static", Corp, Ice)
    },
    CardDef {
        cost: 3,
        strength: Some(2),
        ice_subtype: Some(CodeGate),
        subtypes: &["Code Gate"],
        subroutines: &[RunnerLosesClick, ETR],
        ..CardDef::blank("Enigma", Corp, Ice)
    },
    CardDef {
        cost: 1,
        strength: Some(1),
        ice_subtype: Some(Sentry),
        subtypes: &["Sentry", "AP"],
        subroutines: &[NetDamage(1), CorpGainCredits(1)],
        ..CardDef::blank("Tithe", Corp, Ice)
    },
    CardDef {
        cost: 4,
        strength: Some(0),
        ice_subtype: Some(Sentry),
        subtypes: &["Sentry", "Destroyer"],
        subroutines: &[TrashProgram, ETR],
        ..CardDef::blank("Rototurret", Corp, Ice)
    },
    CardDef {
        cost: 4,
        strength: Some(3),
        ice_subtype: Some(Sentry),
        subtypes: &["Sentry", "AP"],
        subroutines: &[NetDamage(3)],
        ..CardDef::blank("Neural Katana", Corp, Ice)
    },
    CardDef {
        cost: 3,
        strength: Some(3),
        ice_subtype: Some(Sentry),
        subtypes: &["Sentry", "Tracer"],
        subroutines: &[
            SubEffect::Ability {
                label: "Trace 3 - Gain 3 [Credits]",
                effects: &[E::Trace {
                    base: 3,
                    on_success: &[E::GainCredits(Corp, 3)],
                    on_fail: &[],
                }],
            },
            SubEffect::Ability {
                label: "Trace 2 - End the run",
                effects: &[E::Trace {
                    base: 2,
                    on_success: &[E::EndTheRun],
                    on_fail: &[],
                }],
            },
        ],
        ..CardDef::blank("Caduceus", Corp, Ice)
    },
    CardDef {
        cost: 1,
        strength: Some(4),
        ice_subtype: Some(Sentry),
        subtypes: &["Sentry", "Tracer", "Observer"],
        subroutines: &[SubEffect::Ability {
            label: "Trace 3 - Give the Runner 1 tag",
            effects: &[E::Trace {
                base: 3,
                on_success: &[E::GainTags(Fixed(1))],
                on_fail: &[],
            }],
        }],
        ..CardDef::blank("Hunter", Corp, Ice)
    },
    CardDef {
        cost: 4,
        strength: Some(4),
        ice_subtype: Some(Sentry),
        subtypes: &["Sentry", "Tracer", "Observer"],
        // "When the Runner encounters this ice, they must take 1 tag or end
        // the run. Hosted power counter: give the Runner 1 tag.
        // [sub] Trace 3 - place 1 power counter."
        triggered: &[when(
            T::OnEncounterSelf,
            &[E::Choose {
                who: Runner,
                options: &[
                    ChoiceOption {
                        label: "Take 1 tag",
                        effects: &[E::GainTags(Fixed(1))],
                    },
                    ChoiceOption {
                        label: "End the run",
                        effects: &[E::EndTheRun],
                    },
                ],
            }],
        )],
        counter_abilities: &[CounterAbility {
            label: "give the Runner 1 tag",
            cost: (Power, 1),
            timing: AbilityTiming::Anytime,
            effects: &[E::GainTags(Fixed(1))],
        }],
        subroutines: &[SubEffect::Ability {
            label: "Trace 3 - Place 1 power counter",
            effects: &[E::Trace {
                base: 3,
                on_success: &[E::PlaceCounters(Power, 1)],
                on_fail: &[],
            }],
        }],
        ..CardDef::blank("Data Raven", Corp, Ice)
    },
    CardDef {
        cost: 1,
        strength: Some(3),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier", "Psi"],
        // "[sub] Psi game: end the run if the bids differ."
        subroutines: &[SubEffect::Ability {
            label: "Psi Game - End the run",
            effects: &[E::Psi {
                on_equal: &[],
                on_differ: &[E::EndTheRun],
            }],
        }],
        ..CardDef::blank("Snowflake", Corp, Ice)
    },
    // ── Runner: events ─────────────────────────────────────────────────────
    CardDef {
        cost: 5,
        triggered: &[when(T::OnPlaySelf, &[E::GainCredits(Runner, 9)])],
        ..CardDef::blank("Sure Gamble", Runner, Event)
    },
    CardDef {
        cost: 0,
        subtypes: &["Job"],
        triggered: &[when(T::OnPlaySelf, &[E::GainCredits(Runner, 3)])],
        ..CardDef::blank("Easy Mark", Runner, Event)
    },
    CardDef {
        cost: 2,
        subtypes: &["Run"],
        run_event: Some(RunEventDef { target: None }),
        // "When that run ends, if it was successful, gain 5 credits."
        triggered: &[TriggeredAbility {
            trigger: T::RunEnds,
            condition: Condition::RunSuccessful,
            once_per_turn: false,
            effects: &[E::GainCredits(Runner, 5)],
        }],
        ..CardDef::blank("Dirty Laundry", Runner, Event)
    },
    CardDef {
        cost: 0,
        triggered: &[when(T::OnPlaySelf, &[E::Draw(Runner, 3)])],
        ..CardDef::blank("Diesel", Runner, Event)
    },
    CardDef {
        cost: 2,
        subtypes: &["Run", "Sabotage"],
        run_event: Some(RunEventDef { target: Some(ServerId::Hq) }),
        triggered: &[when(T::BreachServer(ServerFilter::Hq), &[E::AccessBonus(2)])],
        ..CardDef::blank("Legwork", Runner, Event)
    },
    CardDef {
        cost: 2,
        subtypes: &["Run", "Sabotage"],
        run_event: Some(RunEventDef { target: Some(ServerId::Rd) }),
        triggered: &[when(T::BreachServer(ServerFilter::Rd), &[E::AccessBonus(2)])],
        ..CardDef::blank("The Maker's Eye", Runner, Event)
    },
    CardDef {
        cost: 0,
        // "Gain 2 credits or expose 1 card."
        triggered: &[when(
            T::OnPlaySelf,
            &[E::Choose {
                who: Runner,
                options: &[
                    ChoiceOption {
                        label: "Gain 2 [Credits]",
                        effects: &[E::GainCredits(Runner, 2)],
                    },
                    ChoiceOption {
                        label: "Expose a card",
                        effects: &[E::ExposeSelect],
                    },
                ],
            }],
        )],
        ..CardDef::blank("Infiltration", Runner, Event)
    },
    // ── Runner: resources / hardware ───────────────────────────────────────
    CardDef {
        cost: 1,
        subtypes: &["Job"],
        triggered: &[when(T::OnInstallSelf, &[E::PlaceCounters(CreditCounter, 12)])],
        click_ability: Some(ClickAbility::TakeCredits(2)),
        ..CardDef::blank("Armitage Codebusting", Runner, Resource)
    },
    CardDef {
        cost: 1,
        subtypes: &["Chip"],
        statics: &[StaticMod::MemoryUnits(1)],
        ..CardDef::blank("Akamatsu Mem Chip", Runner, Hardware)
    },
    // ── Runner: programs ───────────────────────────────────────────────────
    CardDef {
        cost: 1,
        mu_cost: 1,
        strength: Some(0),
        subtypes: &["Virus"],
        // "Successful run on a central: place 1 virus counter. Hosted virus
        // counter: encountered ice gets -1 strength for the encounter."
        triggered: &[when(
            T::SuccessfulRun(Central),
            &[E::PlaceCounters(Virus, 1)],
        )],
        counter_abilities: &[CounterAbility {
            label: "give -1 strength to the encountered ice",
            cost: (Virus, 1),
            timing: AbilityTiming::DuringEncounter,
            effects: &[E::ModIceStrengthThisEncounter(-1)],
        }],
        ..CardDef::blank("Datasucker", Runner, Program)
    },
    CardDef {
        cost: 2,
        mu_cost: 1,
        strength: Some(2),
        subtypes: &["Icebreaker", "Fracter"],
        breaker: Some(BreakerDef {
            breaks: Barrier,
            break_cost: 1,
            pump: Some((1, 1)),
            pump_for_run: false,
            base_strength: 2,
        }),
        ..CardDef::blank("Corroder", Runner, Program)
    },
    CardDef {
        cost: 4,
        mu_cost: 1,
        strength: Some(2),
        subtypes: &["Icebreaker", "Decoder"],
        breaker: Some(BreakerDef {
            breaks: CodeGate,
            break_cost: 1,
            pump: Some((1, 1)),
            pump_for_run: true,
            base_strength: 2,
        }),
        ..CardDef::blank("Gordian Blade", Runner, Program)
    },
    CardDef {
        cost: 3,
        mu_cost: 1,
        strength: Some(3),
        subtypes: &["Icebreaker", "Killer"],
        breaker: Some(BreakerDef {
            breaks: Sentry,
            break_cost: 1,
            pump: None,
            pump_for_run: false,
            base_strength: 3,
        }),
        ..CardDef::blank("Mimic", Runner, Program)
    },
];

/// Psychic Field's shared psi ability (access-while-installed and expose).
const PSYCHIC_FIELD_PSI: &[Effect] = &[E::Psi {
    on_equal: &[],
    on_differ: &[E::Damage(DamageKind::Net, RunnerHandSize)],
}];

/// Printed rules text for the zoom view, backed by the full card database
/// (`printed::printed_text`); the hand-written strings below remain only as a
/// fallback for pool cards should the printed data lack text.
pub fn card_text(title: &str) -> &'static str {
    if let Some(t) = crate::printed::printed_text(title) {
        return t;
    }
    legacy_card_text(title)
}

/// Hand-written text for the original 28-card pool (fallback only).
fn legacy_card_text(title: &str) -> &'static str {
    match title {
        "Weyland Consortium: Building a Better World" => "Whenever you play a transaction operation, gain 1[c].",
        "The Catalyst: Convention Breaker" => "Teaching identity. No special ability.",
        "Gabriel Santiago: Consummate Professional" => "The first time each turn you make a successful run on HQ, gain 2[c].",
        "Hedge Fund" => "Gain 9[c].",
        "Beanstalk Royalties" => "Gain 3[c].",
        "PAD Campaign" => "When your turn begins, gain 1[c].",
        "Regolith Mining License" => "When rezzed, load 15[c]. When empty, trash it. [click]: Take 3[c] from this asset.",
        "Offworld Office" => "When you score this agenda, gain 7[c].",
        "Hostile Takeover" => "When you score this agenda, gain 7[c] and take 1 bad publicity.",
        "Priority Requisition" => "When you score this agenda, you may rez 1 piece of ice, ignoring all costs.",
        "Superconducting Hub" => "You get +2 maximum hand size. When you score this agenda, you may draw 2 cards.",
        "Ice Wall" => "You can advance this ice. It gets +1 strength for each hosted advancement counter.\n[sub] End the run.",
        "Vanilla" => "[sub] End the run.",
        "Wall of Static" => "[sub] End the run.",
        "Enigma" => "[sub] The Runner loses [click], if able.\n[sub] End the run.",
        "Tithe" => "[sub] Do 1 net damage.\n[sub] Gain 1[c].",
        "Rototurret" => "[sub] Trash 1 installed program.\n[sub] End the run.",
        "Sure Gamble" => "Gain 9[c].",
        "Easy Mark" => "Gain 3[c].",
        "Dirty Laundry" => "Run any server. When that run ends, if it was successful, gain 5[c].",
        "Diesel" => "Draw 3 cards.",
        "Legwork" => "Run HQ. If successful, access 2 additional cards when you breach HQ.",
        "The Maker's Eye" => "Run R&D. If successful, access 2 additional cards when you breach R&D.",
        "Armitage Codebusting" => "When installed, load 12[c]. When empty, trash it. [click]: Take 2[c] from this resource.",
        "Akamatsu Mem Chip" => "+1 memory unit.",
        "Corroder" => "Interface — 1[c]: Break 1 barrier subroutine. 1[c]: +1 strength.",
        "Gordian Blade" => "Interface — 1[c]: Break 1 code gate subroutine. 1[c]: +1 strength for the remainder of this run.",
        "Mimic" => "Interface — 1[c]: Break 1 sentry subroutine.",
        _ => "",
    }
}

pub fn def_index(title: &str) -> Option<usize> {
    CARDS.iter().position(|c| c.title == title)
}

pub fn def_by_title(title: &str) -> Option<&'static CardDef> {
    CARDS.iter().find(|c| c.title == title)
}

// ── vanilla runtime definitions for cards without a behavior row ───────────
//
// Mirrors the reference implementation's stance: a title without a defcard
// body still exists in the game with its printed stats and an empty behavior
// map. Here, any title known to `printed::printed` but absent from `CARDS`
// gets a synthesized `CardDef` with correct printed stats and NO behavior
// hooks (no triggered abilities, zero subroutines, nothing to host or drip).
// Definitions are interned once per title (`Box::leak`) so the existing
// `&'static CardDef` API stays stable; the leak is bounded by the card pool.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct SynthRegistry {
    by_title: HashMap<&'static str, usize>,
    defs: Vec<&'static CardDef>,
}

static SYNTH: OnceLock<Mutex<SynthRegistry>> = OnceLock::new();

fn synth_registry() -> &'static Mutex<SynthRegistry> {
    SYNTH.get_or_init(Default::default)
}

/// The definition behind a `CardInstance::def` index: the hand-written
/// behavior table first, then the synthesized-vanilla registry.
pub fn def_at(index: usize) -> &'static CardDef {
    if index < CARDS.len() {
        &CARDS[index]
    } else {
        let reg = synth_registry().lock().unwrap();
        reg.defs[index - CARDS.len()]
    }
}

/// Index for a title, synthesizing a vanilla definition from printed data
/// when the behavior table has no row. Errors (instead of panicking) for
/// titles unknown even to the printed database, and for non-playable
/// entries (e.g. "Rules Insert" pseudo-cards).
pub fn def_index_or_synth(title: &str) -> Result<usize, String> {
    if let Some(i) = def_index(title) {
        return Ok(i);
    }
    let mut reg = synth_registry().lock().unwrap();
    if let Some(&i) = reg.by_title.get(title) {
        return Ok(CARDS.len() + i);
    }
    let p = crate::printed::printed(title)
        .ok_or_else(|| format!("unknown card title: {title}"))?;
    let def = synth_vanilla(p)?;
    let leaked: &'static CardDef = Box::leak(Box::new(def));
    let i = reg.defs.len();
    reg.defs.push(leaked);
    reg.by_title.insert(p.title.as_str(), i);
    Ok(CARDS.len() + i)
}

/// A behavior-free `CardDef` carrying the printed stats: operations/events
/// resolve with no effect, ice has zero subroutines, installables sit there.
fn synth_vanilla(p: &'static crate::printed::PrintedCard) -> Result<CardDef, String> {
    let title = p.title.as_str();
    let side = match p.side.as_str() {
        "Corp" => Corp,
        "Runner" => Runner,
        other => return Err(format!("{title}: unplayable side {other:?}")),
    };
    let kind = match p.card_type.as_str() {
        "Identity" => Identity,
        "Agenda" => Agenda,
        "Asset" => Asset,
        "Upgrade" => Upgrade,
        "ICE" => Ice,
        "Operation" => Operation,
        "Event" => Event,
        "Program" => Program,
        "Hardware" => Hardware,
        "Resource" => Resource,
        other => return Err(format!("{title}: unplayable card type {other:?}")),
    };
    let subtypes: &'static [&'static str] =
        Box::leak(p.subtypes.iter().map(|s| s.as_str()).collect::<Vec<_>>().into_boxed_slice());
    let ice_subtype = if kind == Ice {
        subtypes.iter().find_map(|s| match *s {
            "Barrier" => Some(Barrier),
            "Code Gate" => Some(CodeGate),
            "Sentry" => Some(Sentry),
            _ => None,
        })
    } else {
        None
    };
    Ok(CardDef {
        cost: p.cost.unwrap_or(0).max(0) as u32,
        subtypes,
        ice_subtype,
        strength: p.strength.map(|s| s as i32),
        trash_cost: p.trash_cost.map(|t| t.max(0) as u32),
        mu_cost: p.memoryunits.unwrap_or(0).max(0) as u32,
        advancement_requirement: p.advancement_requirement.map(|a| a.max(0) as u32),
        agenda_points: p.agenda_points.map(|a| a.max(0) as u32),
        ..CardDef::blank(title, side, kind)
    })
}

/// Default playtest decklists (also the self-play fuzz decks, so they carry
/// at least one of every mechanic).
pub fn corp_deck() -> Vec<&'static str> {
    let mut d = Vec::new();
    let mut add = |t: &'static str, n: usize| {
        for _ in 0..n {
            d.push(t)
        }
    };
    add("Hedge Fund", 3);
    add("Beanstalk Royalties", 2);
    add("SEA Source", 2);
    add("PAD Campaign", 2);
    add("Regolith Mining License", 1);
    add("Adonis Campaign", 2);
    add("Snare!", 2);
    add("Ghost Branch", 1);
    add("Project Junebug", 1);
    add("Cerebral Overwriter", 1);
    add("Psychic Field", 2);
    add("Offworld Office", 3);
    add("Hostile Takeover", 3);
    add("Priority Requisition", 1);
    add("Superconducting Hub", 1);
    add("Nisei MK II", 2);
    add("Ice Wall", 2);
    add("Vanilla", 1);
    add("Wall of Static", 1);
    add("Enigma", 2);
    add("Tithe", 1);
    add("Rototurret", 1);
    add("Neural Katana", 1);
    add("Caduceus", 1);
    add("Hunter", 1);
    add("Data Raven", 2);
    add("Snowflake", 1);
    d
}

pub fn runner_deck() -> Vec<&'static str> {
    let mut d = Vec::new();
    let mut add = |t: &'static str, n: usize| {
        for _ in 0..n {
            d.push(t)
        }
    };
    add("Sure Gamble", 3);
    add("Easy Mark", 2);
    add("Dirty Laundry", 3);
    add("Diesel", 3);
    add("Legwork", 2);
    add("The Maker's Eye", 2);
    add("Infiltration", 2);
    add("Armitage Codebusting", 2);
    add("Akamatsu Mem Chip", 2);
    add("Datasucker", 2);
    add("Corroder", 3);
    add("Gordian Blade", 2);
    add("Mimic", 2);
    d
}

pub const CORP_ID: &str = "Weyland Consortium: Building a Better World";
pub const RUNNER_ID: &str = "The Catalyst: Convention Breaker";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decks_only_reference_known_cards() {
        for t in corp_deck().iter().chain(runner_deck().iter()) {
            assert!(def_by_title(t).is_some(), "unknown card in deck: {t}");
        }
        assert!(def_by_title(CORP_ID).is_some());
        assert!(def_by_title(RUNNER_ID).is_some());
    }

    #[test]
    fn agenda_math_is_playable() {
        let pts: u32 = corp_deck()
            .iter()
            .filter_map(|t| def_by_title(t).unwrap().agenda_points)
            .sum();
        assert!(pts >= 14, "not enough agenda points to reliably win: {pts}");
    }

    #[test]
    fn behavior_stats_match_printed_data() {
        // Every behavior row must carry the printed cost/strength/trash
        // numbers (fidelity bar: printed stats come from cardboard).
        for def in CARDS {
            let Some(p) = crate::printed::printed(def.title) else {
                panic!("behavior row not in printed db: {}", def.title);
            };
            if let Some(cost) = p.cost {
                assert_eq!(def.cost as i64, cost.max(0), "{}: cost", def.title);
            }
            assert_eq!(
                def.strength,
                p.strength.map(|s| s as i32),
                "{}: strength",
                def.title
            );
            assert_eq!(
                def.trash_cost,
                p.trash_cost.map(|t| t.max(0) as u32),
                "{}: trash cost",
                def.title
            );
            assert_eq!(
                def.advancement_requirement,
                p.advancement_requirement.map(|a| a.max(0) as u32),
                "{}: advancement requirement",
                def.title
            );
            assert_eq!(
                def.agenda_points,
                p.agenda_points.map(|a| a.max(0) as u32),
                "{}: agenda points",
                def.title
            );
            assert_eq!(
                def.mu_cost as i64,
                p.memoryunits.unwrap_or(0).max(0),
                "{}: memory units",
                def.title
            );
        }
    }
}
