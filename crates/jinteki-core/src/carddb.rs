//! The card database: printed data + declarative behavior for the playable pool.
//!
//! Every definition mirrors the reference implementation in
//! `jinteki-reference/src/clj/game/cards/*.clj` (see DESIGN.md Appendix B pin).
//! This table is the denotation target of the future designer DSL: a card is a
//! row of data, and the engine gives the data meaning.

use crate::types::*;

use CardType::*;
use IceSubtype::*;
use Side::*;
use SubEffect::*;

pub const CARDS: &[CardDef] = &[
    // ── Identities ─────────────────────────────────────────────────────────
    CardDef {
        subtypes: &["Megacorp"],
        identity_ability: Some(IdentityAbility::GainOnTransaction),
        ..CardDef::blank("Weyland Consortium: Building a Better World", Corp, Identity)
    },
    CardDef {
        subtypes: &["Natural"],
        ..CardDef::blank("The Catalyst: Convention Breaker", Runner, Identity)
    },
    CardDef {
        subtypes: &["Cyborg"],
        identity_ability: Some(IdentityAbility::GabrielHq),
        ..CardDef::blank("Gabriel Santiago: Consummate Professional", Runner, Identity)
    },
    // ── Corp: operations ───────────────────────────────────────────────────
    CardDef {
        cost: 5,
        subtypes: &["Transaction"],
        on_play: Some(OnPlay::GainCredits(9)),
        ..CardDef::blank("Hedge Fund", Corp, Operation)
    },
    CardDef {
        cost: 0,
        subtypes: &["Transaction"],
        on_play: Some(OnPlay::GainCredits(3)),
        ..CardDef::blank("Beanstalk Royalties", Corp, Operation)
    },
    // ── Corp: assets ───────────────────────────────────────────────────────
    CardDef {
        cost: 2,
        trash_cost: Some(4),
        subtypes: &["Advertisement"],
        drip_corp_turn: 1,
        ..CardDef::blank("PAD Campaign", Corp, Asset)
    },
    CardDef {
        cost: 2,
        trash_cost: Some(3),
        start_credits: 15,
        click_ability: Some(ClickAbility::TakeCredits(3)),
        ..CardDef::blank("Regolith Mining License", Corp, Asset)
    },
    // ── Corp: agendas ──────────────────────────────────────────────────────
    CardDef {
        advancement_requirement: Some(4),
        agenda_points: Some(2),
        subtypes: &["Expansion"],
        on_score: Some(OnScore::GainCredits(7)),
        ..CardDef::blank("Offworld Office", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(2),
        agenda_points: Some(1),
        subtypes: &["Expansion"],
        on_score: Some(OnScore::GainCreditsAndBadPub(7)),
        ..CardDef::blank("Hostile Takeover", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(5),
        agenda_points: Some(3),
        subtypes: &["Security"],
        on_score: Some(OnScore::OptionalRezIceFree),
        ..CardDef::blank("Priority Requisition", Corp, Agenda)
    },
    CardDef {
        advancement_requirement: Some(3),
        agenda_points: Some(1),
        subtypes: &["Expansion"],
        on_score: Some(OnScore::DrawUpTo(2)),
        statics: &[StaticMod::MaxHandSize(2)],
        ..CardDef::blank("Superconducting Hub", Corp, Agenda)
    },
    // ── Corp: ice ──────────────────────────────────────────────────────────
    CardDef {
        cost: 1,
        strength: Some(1),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier"],
        advanceable: true,
        subroutines: &[EndTheRun],
        ..CardDef::blank("Ice Wall", Corp, Ice)
    },
    CardDef {
        cost: 0,
        strength: Some(0),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier"],
        subroutines: &[EndTheRun],
        ..CardDef::blank("Vanilla", Corp, Ice)
    },
    CardDef {
        cost: 3,
        strength: Some(3),
        ice_subtype: Some(Barrier),
        subtypes: &["Barrier"],
        subroutines: &[EndTheRun],
        ..CardDef::blank("Wall of Static", Corp, Ice)
    },
    CardDef {
        cost: 3,
        strength: Some(2),
        ice_subtype: Some(CodeGate),
        subtypes: &["Code Gate"],
        subroutines: &[RunnerLosesClick, EndTheRun],
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
        subroutines: &[TrashProgram, EndTheRun],
        ..CardDef::blank("Rototurret", Corp, Ice)
    },
    // ── Runner: events ─────────────────────────────────────────────────────
    CardDef {
        cost: 5,
        on_play: Some(OnPlay::GainCredits(9)),
        ..CardDef::blank("Sure Gamble", Runner, Event)
    },
    CardDef {
        cost: 0,
        subtypes: &["Job"],
        on_play: Some(OnPlay::GainCredits(3)),
        ..CardDef::blank("Easy Mark", Runner, Event)
    },
    CardDef {
        cost: 2,
        subtypes: &["Run"],
        on_play: Some(OnPlay::RunEvent { target: None, access_bonus: 0, success_credits: 5 }),
        ..CardDef::blank("Dirty Laundry", Runner, Event)
    },
    CardDef {
        cost: 0,
        on_play: Some(OnPlay::Draw(3)),
        ..CardDef::blank("Diesel", Runner, Event)
    },
    CardDef {
        cost: 2,
        subtypes: &["Run", "Sabotage"],
        on_play: Some(OnPlay::RunEvent {
            target: Some(ServerId::Hq),
            access_bonus: 2,
            success_credits: 0,
        }),
        ..CardDef::blank("Legwork", Runner, Event)
    },
    CardDef {
        cost: 2,
        subtypes: &["Run", "Sabotage"],
        on_play: Some(OnPlay::RunEvent {
            target: Some(ServerId::Rd),
            access_bonus: 2,
            success_credits: 0,
        }),
        ..CardDef::blank("The Maker's Eye", Runner, Event)
    },
    // ── Runner: resources / hardware ───────────────────────────────────────
    CardDef {
        cost: 1,
        subtypes: &["Job"],
        start_credits: 12,
        click_ability: Some(ClickAbility::TakeCredits(2)),
        ..CardDef::blank("Armitage Codebusting", Runner, Resource)
    },
    CardDef {
        cost: 1,
        subtypes: &["Chip"],
        statics: &[StaticMod::MemoryUnits(1)],
        ..CardDef::blank("Akamatsu Mem Chip", Runner, Hardware)
    },
    // ── Runner: icebreakers ────────────────────────────────────────────────
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

/// Printed rules text for the zoom view. Kept out of `CardDef` so the
/// behavioral table stays purely mechanical.
pub fn card_text(title: &str) -> &'static str {
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

/// Default playtest decklists.
pub fn corp_deck() -> Vec<&'static str> {
    let mut d = Vec::new();
    let mut add = |t: &'static str, n: usize| {
        for _ in 0..n {
            d.push(t)
        }
    };
    add("Hedge Fund", 3);
    add("Beanstalk Royalties", 3);
    add("PAD Campaign", 3);
    add("Regolith Mining License", 2);
    add("Offworld Office", 3);
    add("Hostile Takeover", 3);
    add("Priority Requisition", 2);
    add("Superconducting Hub", 2);
    add("Ice Wall", 3);
    add("Vanilla", 2);
    add("Wall of Static", 3);
    add("Enigma", 3);
    add("Tithe", 2);
    add("Rototurret", 2);
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
    add("Easy Mark", 3);
    add("Dirty Laundry", 3);
    add("Diesel", 3);
    add("Legwork", 2);
    add("The Maker's Eye", 2);
    add("Armitage Codebusting", 3);
    add("Akamatsu Mem Chip", 2);
    add("Corroder", 3);
    add("Gordian Blade", 3);
    add("Mimic", 3);
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
}
