//! Deck construction (§1.4): the pure arithmetic of a legal deck.
//!
//! Nothing here touches the VM — deck legality is a property of a list of
//! cards, checked before the game begins (1.4.2). The two functions are the
//! ones the CR states with worked examples.

/// CR 1.4.5/1.4.5a: the total influence cost of the out-of-faction cards in
/// a deck, counted BY COPY and not by name — three copies of a 2-influence
/// card cost 6, not 2.
/// A deck is given here as one entry per COPY — `(faction, influence cost)` —
/// because 1.4.5a is precisely the rule that the list is not deduplicated by
/// name. A card is out of faction when its faction differs from the
/// identity's (neutral cards included, 1.4.5); `influence` is `None` for a
/// card with no influence cost, which 1.4.4 forbids out of faction.
pub fn total_influence(deck: &[(Option<&str>, Option<u32>)], faction: &str) -> u32 {
    cite!("rule_influence_limit");
    cite!("rule_influence_by_copy");
    deck.iter()
        .filter(|(f, _)| *f != Some(faction))
        .map(|(_, inf)| inf.unwrap_or(0))
        .sum()
}

/// CR 1.4.6: the agenda points a Corp deck of this size must contain, as the
/// inclusive range (low, high).
///
/// 1.4.6a 40–44 → 18/19, 1.4.6b 45–49 → 20/21, 1.4.6c 50–54 → 22/23, and
/// 1.4.6d more than 54 → 22/23 plus 2 more for every FULL 5 cards over 50
/// (so a 66-card deck adds 3 × 2 = 6, giving 28 or 29).
pub fn agenda_points_required(deck_size: u32) -> (u32, u32) {
    cite!("rule_agenda_points");
    match deck_size {
        0..=44 => {
            cite!("rule_40_44");
            (18, 19)
        }
        45..=49 => {
            cite!("rule_45_49");
            (20, 21)
        }
        50..=54 => {
            cite!("rule_50_54");
            (22, 23)
        }
        n => {
            cite!("rule_54+");
            let extra = 2 * ((n - 50) / 5);
            (22 + extra, 23 + extra)
        }
    }
}
