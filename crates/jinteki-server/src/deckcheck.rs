//! Pure deck-construction validation (ACCOUNTS-AND-DECKS.md §6.2).
//!
//! Data-in / data-out, no I/O, no clock: this module is written to move into
//! `jinteki-core` unchanged once the printed-card struct there grows the
//! deck-construction fields. Until then it reads the server-side card index
//! (`carddata.rs`) which is itself pure over embedded JSON.
//!
//! Rule sources, cited per check:
//!   - docs/rules/CR-v26.03.md §1.4 (Deck Construction) — rule ids in
//!     comments below;
//!   - the reference validator `jinteki-reference/src/cljc/jinteki/
//!     validator.cljc` at the pin, whose outcomes v1 must reproduce
//!     (DESIGN.md SYS-K-3); line anchors noted where the CR is silent.
//!
//! Deferred to v2 (validator.cljc anchors): alliance discounts (:13-42), The
//! Professor (:88-92,111-113), singleton identities Nova/Ampère (:68-78,
//! 146-167), Custom Biotics (:180-181), full MWL/points machinery (:253+).

use crate::carddata::{self, Card};
use jinteki_core::printed;
use serde::Serialize;

/// One deck line: canonical title + quantity.
#[derive(Debug, Clone)]
pub struct DeckLine {
    pub title: String,
    pub qty: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Counts {
    pub cards: u32,
    pub influence_used: i64,
    /// None = unlimited (identity has no printed influence limit).
    pub influence_limit: Option<i64>,
    pub agenda_points: i64,
    pub min_deck_size: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayableSummary {
    pub behavior: u32,
    pub jnet_only: u32,
    pub unimplemented: u32,
}

/// Per-card verdict row: everything a client needs to render a deck line.
#[derive(Debug, Clone, Serialize)]
pub struct CardVerdict {
    pub title: String,
    pub code: String,
    pub qty: u32,
    /// "behavior" | "jnet_only" | "unimplemented"
    pub impl_status: String,
    pub influence_spent: i64,
    pub banned: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Verdict {
    pub legal: bool,
    pub problems: Vec<Problem>,
    /// Non-blocking (standard bans surface here in v1 — casual server).
    pub warnings: Vec<Problem>,
    pub counts: Counts,
    pub playable: PlayableSummary,
    pub cards: Vec<CardVerdict>,
}

pub fn impl_status_str(title: &str) -> &'static str {
    match printed::impl_status(title) {
        printed::ImplStatus::Behavior => "behavior",
        printed::ImplStatus::JnetOnly => "jnet_only",
        printed::ImplStatus::Unimplemented => "unimplemented",
    }
}

fn playable_add(p: &mut PlayableSummary, title: &str, qty: u32) {
    match printed::impl_status(title) {
        printed::ImplStatus::Behavior => p.behavior += qty,
        printed::ImplStatus::JnetOnly => p.jnet_only += qty,
        printed::ImplStatus::Unimplemented => p.unimplemented += qty,
    }
}

/// Validate a deck against the construction rules. `identity_title` is the
/// canonical title of the claimed identity; `lines` the deck body (no
/// identity line). Unknown titles yield problems rather than panics.
pub fn check(identity_title: &str, lines: &[DeckLine]) -> Verdict {
    let mut problems: Vec<Problem> = Vec::new();
    let mut warnings: Vec<Problem> = Vec::new();
    let mut cards_out: Vec<CardVerdict> = Vec::new();
    let mut playable = PlayableSummary::default();

    let push = |list: &mut Vec<Problem>, code: &str, message: String| {
        list.push(Problem { code: code.into(), message });
    };

    // CR 1.4.1: the identity determines faction, minimum deck size and
    // influence limit of the deck.
    let identity: Option<&Card> = carddata::by_title(identity_title);
    match identity {
        None => push(
            &mut problems,
            "identity-unknown",
            format!("unknown identity \"{identity_title}\""),
        ),
        Some(id) if !id.is_identity() => push(
            &mut problems,
            "identity-not-identity",
            format!("\"{}\" is not an identity card", id.title),
        ),
        Some(_) => {}
    }
    let id_side = identity.filter(|c| c.is_identity()).map(|c| c.side.as_str());
    let id_faction = identity
        .filter(|c| c.is_identity())
        .and_then(|c| c.faction.as_deref());
    // CR 1.4.3: minimum deck size from the identity. CR 1.4.5: influence
    // limit from the identity (validator.cljc:62-66 — nil limit = unlimited).
    let min_deck_size = identity.and_then(|c| c.min_deck_size).unwrap_or(45);
    let influence_limit = identity.and_then(|c| c.influence_limit);
    if let Some(id) = identity {
        playable_add(&mut playable, &id.title, 1);
        if id.standard_banned {
            push(
                &mut warnings,
                "banned",
                format!("{} is banned in standard", id.title),
            );
        }
    }

    let mut n_cards: u32 = 0;
    let mut influence_used: i64 = 0;
    let mut agenda_points: i64 = 0;

    for line in lines {
        let Some(card) = carddata::by_title(&line.title) else {
            push(
                &mut problems,
                "unknown-card",
                format!("unknown card \"{}\"", line.title),
            );
            continue;
        };
        n_cards += line.qty;
        playable_add(&mut playable, &card.title, line.qty);

        // CR 1.4.4: no identity cards inside the deck.
        if card.is_identity() {
            push(
                &mut problems,
                "identity-in-deck",
                format!("{} is an identity and cannot be a deck card", card.title),
            );
        }
        // CR 1.4.4: no cards from the wrong side.
        if let Some(side) = id_side {
            if card.side != side {
                push(
                    &mut problems,
                    "wrong-side",
                    format!("{} is a {} card in a {} deck", card.title, card.side, side),
                );
            }
        }
        // Agendas must be neutral or identity-faction (validator.cljc:174-178
        // `allowed?`; the CR leaves this to the card-pool structure — agendas
        // carry no influence cost, so CR 1.4.4's "out-of-faction cards that
        // lack influence costs" excludes them too).
        if card.is_agenda() {
            agenda_points += card.agenda_points.unwrap_or(0) * i64::from(line.qty);
            if let (Some(idf), Some(cf)) = (id_faction, card.faction.as_deref()) {
                if cf != "Neutral" && cf != idf {
                    push(
                        &mut problems,
                        "agenda-faction",
                        format!(
                            "{} is a {} agenda; only {} or Neutral agendas are allowed",
                            card.title, cf, idf
                        ),
                    );
                }
            }
        }
        // CR 1.4.7: at most 3 copies by name, unless card text stipulates
        // another limit (deck_limit from the card data; validator.cljc:81-86).
        let copy_limit = card.deck_limit.unwrap_or(3);
        if i64::from(line.qty) > copy_limit {
            push(
                &mut problems,
                "copy-limit",
                format!(
                    "{} × {} exceeds the limit of {} copies",
                    line.qty, card.title, copy_limit
                ),
            );
        }
        // CR 1.4.5 + 1.4.5a: out-of-faction influence, counted by copy.
        let spent = match (id_faction, card.faction.as_deref()) {
            (Some(idf), Some(cf)) if cf != idf => {
                card.influence_cost.unwrap_or(0) * i64::from(line.qty)
            }
            _ => 0,
        };
        influence_used += spent;

        if card.standard_banned {
            push(
                &mut warnings,
                "banned",
                format!("{} is banned in standard", card.title),
            );
        }
        if card.rotated {
            push(
                &mut warnings,
                "rotated",
                format!("{} has rotated out of standard", card.title),
            );
        }

        cards_out.push(CardVerdict {
            title: card.title.clone(),
            code: card.code.clone(),
            qty: line.qty,
            impl_status: impl_status_str(&card.title).into(),
            influence_spent: spent,
            banned: card.standard_banned,
        });
    }

    // CR 1.4.3: at least the identity's minimum deck size.
    if i64::from(n_cards) < min_deck_size {
        push(
            &mut problems,
            "deck-size",
            format!("{n_cards} cards; the identity requires at least {min_deck_size}"),
        );
    }
    // CR 1.4.5: total out-of-faction influence within the identity's limit.
    if let Some(limit) = influence_limit {
        if influence_used > limit {
            push(
                &mut problems,
                "influence",
                format!("{influence_used} influence used; the identity allows {limit}"),
            );
        }
    }
    // CR 1.4.6a-d: the Corp agenda-point band. The table (40-44 → 18/19,
    // 45-49 → 20/21, 50-54 → 22/23, then +2 per full 5 cards over 54) is
    // exactly min = 2 + 2·⌊n/5⌋ with n clamped up to the minimum deck size,
    // legal iff min ≤ points ≤ min+1 (validator.cljc:51-55,199-201).
    if id_side == Some("Corp") {
        let n = i64::from(n_cards).max(min_deck_size);
        let min_pts = 2 + 2 * (n / 5);
        if agenda_points < min_pts || agenda_points > min_pts + 1 {
            push(
                &mut problems,
                "agenda-points",
                format!(
                    "{agenda_points} agenda points; a {n}-card deck needs {min_pts}\u{2013}{}",
                    min_pts + 1
                ),
            );
        }
    }

    Verdict {
        legal: problems.is_empty(),
        problems,
        warnings,
        counts: Counts {
            cards: n_cards,
            influence_used,
            influence_limit,
            agenda_points,
            min_deck_size,
        },
        playable,
        cards: cards_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[(&str, u32)]) -> Vec<DeckLine> {
        v.iter()
            .map(|(t, q)| DeckLine { title: (*t).into(), qty: *q })
            .collect()
    }

    /// A legal 45-card Weyland deck: 21 agenda points (45-49 band wants
    /// 20-21, CR 1.4.6b), in-faction + neutral fillers, zero influence.
    fn legal_weyland() -> Vec<DeckLine> {
        lines(&[
            ("Project Atlas", 3),       // Weyland agenda, 2 pt each
            ("Oaktown Renovation", 3),  // Weyland agenda, 2 pt each
            ("SDS Drone Deployment", 2), // Weyland agenda, 3 pt each
            ("Hostile Takeover", 3),    // Weyland agenda, 1 pt each
            ("Hedge Fund", 3),
            ("IPO", 3),
            ("Beanstalk Royalties", 3),
            ("Ice Wall", 3),
            ("Wall of Static", 3),
            ("Enigma", 3),
            ("Hunter", 3),
            ("PAD Campaign", 3),
            ("Launch Campaign", 3),
            ("Hortum", 3),
            ("Priority Construction", 3),
            ("Colossus", 1),
        ])
    }

    #[test]
    fn legal_corp_deck_passes() {
        let v = check("Weyland Consortium: Building a Better World", &legal_weyland());
        assert_eq!(v.counts.cards, 45);
        assert_eq!(v.counts.agenda_points, 21);
        assert_eq!(v.counts.influence_used, 0);
        assert!(v.legal, "problems: {:?}", v.problems);
    }

    #[test]
    fn agenda_band_tracks_cr_146_table() {
        // CR 1.4.6a-d: drop one Hostile Takeover → 44 cards, 20 AP. A
        // 44-card deck must hold 18-19 (CR 1.4.6a), so 20 is now illegal —
        // the band moved with the count exactly as the table says.
        let mut deck = legal_weyland();
        deck.iter_mut().find(|l| l.title == "Hostile Takeover").unwrap().qty = 2;
        let v = check("Weyland Consortium: Building a Better World", &deck);
        assert_eq!(v.counts.cards, 44);
        assert_eq!(v.counts.agenda_points, 20);
        // 44 cards but min_deck_size 45: count clamps UP to 45 (validator
        // .cljc:51-55), so the band stays 20-21 and only deck-size fails.
        assert!(v.problems.iter().any(|p| p.code == "deck-size"));
        assert!(
            !v.problems.iter().any(|p| p.code == "agenda-points"),
            "clamped band keeps 20 AP legal: {:?}",
            v.problems
        );
        // Pure band math against the CR table values.
        for (n, want_min) in [(40i64, 18), (44, 18), (45, 20), (49, 20), (50, 22), (54, 22), (55, 24), (66, 28)] {
            assert_eq!(2 + 2 * (n / 5), want_min, "CR 1.4.6 minimum for {n} cards");
        }
    }

    #[test]
    fn wrong_side_and_identity_in_deck_flagged() {
        let v = check(
            "Weyland Consortium: Building a Better World",
            &lines(&[("Sure Gamble", 3), ("Hostile Takeover", 3)]),
        );
        assert!(v.problems.iter().any(|p| p.code == "wrong-side"));
        let v2 = check(
            "Weyland Consortium: Building a Better World",
            &lines(&[("Weyland Consortium: Building a Better World", 1)]),
        );
        assert!(v2.problems.iter().any(|p| p.code == "identity-in-deck"));
    }

    #[test]
    fn copy_limit_and_alt_limits() {
        let v = check(
            "Weyland Consortium: Building a Better World",
            &lines(&[("Hedge Fund", 4)]),
        );
        assert!(v.problems.iter().any(|p| p.code == "copy-limit"));
        // 15 Minutes stipulates deck_limit 1 (CR 1.4.7 alternative limits).
        let v2 = check(
            "NBN: Making News",
            &lines(&[("15 Minutes", 2)]),
        );
        assert!(v2.problems.iter().any(|p| p.code == "copy-limit"));
    }

    #[test]
    fn influence_counted_by_copy_and_capped() {
        // Diesel is Shaper, 2 influence. In an Anarch deck 3 copies = 6.
        let v = check("Noise: Hacker Extraordinaire", &lines(&[("Diesel", 3)]));
        assert_eq!(v.counts.influence_used, 6); // CR 1.4.5a example, doubled
        let row = v.cards.iter().find(|c| c.title == "Diesel").unwrap();
        assert_eq!(row.influence_spent, 6);
    }

    #[test]
    fn off_faction_agenda_flagged() {
        // Nisei MK II is a Jinteki agenda: illegal in a Weyland deck even
        // with influence to spare (validator.cljc:174-178).
        let v = check(
            "Weyland Consortium: Building a Better World",
            &lines(&[("Nisei MK II", 1)]),
        );
        assert!(v.problems.iter().any(|p| p.code == "agenda-faction"));
    }

    #[test]
    fn unknown_titles_reported_not_panicked() {
        let v = check("Weyland Consortium: Building a Better World", &lines(&[("Not A Card", 1)]));
        assert!(v.problems.iter().any(|p| p.code == "unknown-card"));
        let v2 = check("Not An Identity", &[]);
        assert!(v2.problems.iter().any(|p| p.code == "identity-unknown"));
    }

    #[test]
    fn runner_min_deck_size_enforced_and_no_agenda_band() {
        // Hoshiko: 45 minimum. 3 cards is far short.
        let v = check(
            "Hoshiko Shiro: Untold Protagonist",
            &lines(&[("Sure Gamble", 3)]),
        );
        assert!(v.problems.iter().any(|p| p.code == "deck-size"));
        assert!(!v.problems.iter().any(|p| p.code == "agenda-points"));
    }

    #[test]
    fn per_card_impl_status_present() {
        let v = check(
            "Weyland Consortium: Building a Better World",
            &lines(&[("Hedge Fund", 3)]),
        );
        let row = v.cards.iter().find(|c| c.title == "Hedge Fund").unwrap();
        assert_eq!(row.impl_status, "behavior");
        assert!(v.playable.behavior >= 3);
    }
}
