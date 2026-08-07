//! Conditional influence: the printed waiver that makes a card cost ZERO
//! influence in a deck that satisfies the card's own condition.
//!
//! A card's influence cost is a FUNCTION OF THE DECK, not a constant. Twelve
//! cards in NSG's data print a deckbuilding sentence of one shape —
//!
//! > This card costs 0 influence if you have *&lt;quantity&gt;* in your deck.
//!
//! — and a validator that charges `influence_cost` unconditionally is wrong
//! about every deck that contains one. Mezzie's Asa (12 ice, Mumba Temple ×3)
//! read as 21 influence against a limit of 15 for exactly this reason; the
//! three Temples are free and the deck spends 15.
//!
//! ## The sentence is parsed, not tabulated
//!
//! [`parse_waiver`] reads the printed text. It does NOT key off card titles.
//! A title-keyed table is a table that silently omits the next Alliance card
//! NSG prints — the card would be charged full price and nothing in the build
//! would say so. Reading the sentence means a new card of an existing shape
//! works the day its text lands, and a new card of a NEW shape is a parse
//! ERROR that `tests/influence.rs` turns into a failing build.
//!
//! The parse is TOTAL in that sense: every outcome is `Ok(Some(waiver))`,
//! `Ok(None)` (the card prints no waiver sentence at all), or `Err`. There is
//! no path on which an unreadable waiver quietly becomes "charge full price".
//!
//! ## What the counts mean
//!
//! "…cards in your deck" counts COPIES, not distinct titles, and the deck is
//! the deck: CR 1.4.3a keeps the identity outside it, so the identity never
//! counts toward any of these. A card counts toward its own condition when it
//! satisfies the description (Museum of History is one of the 50 cards) and
//! not otherwise (Mumba Temple is not ice; Mumbad Virtual Tour is an upgrade,
//! not one of the 7 assets).
//!
//! ## Not a CR rule
//!
//! Deck construction is CR 1.4 and influence is CR 1.4.5, but the CR says
//! nothing whatever about these waivers — influence is a tournament construct
//! and the waiver is printed card text, read the way printed text is read.
//! Nothing here carries a `cite!`. The one citation in this neighbourhood,
//! CR 1.4.5 for who is out of faction, stays where it already is in
//! `eternal.rs` and is unchanged by any of this.
//!
//! The outcomes match the reference validator
//! (`jinteki-reference/src/cljc/jinteki/validator.cljc:13-42`,
//! `alliance-is-free?`), which hardcodes the twelve titles; this module
//! derives the same predicates from the text instead.

use crate::carddata::{self, Card};
use jinteki_cr::Subtype;
use std::collections::HashMap;
use std::fmt;

/// The printed words that mark a card as carrying a waiver at all. Every one
/// of the twelve prints them; nothing else in NSG's data does.
const MARKER: &str = "costs 0 influence";

// ───────────────────────────────────────────────────────────────────────────
// Faction
// ───────────────────────────────────────────────────────────────────────────

/// A faction, in the two spellings this module has to read: the card
/// database's name (`Card::faction`) and the bracketed icon code printed
/// inside card text (`[haas-bioroid]`).
///
/// Typed rather than a `String` so that a waiver naming a faction cannot be
/// built out of a spelling no card carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Faction {
    Adam,
    Anarch,
    Apex,
    Criminal,
    HaasBioroid,
    Jinteki,
    Nbn,
    Neutral,
    Shaper,
    SunnyLebeau,
    WeylandConsortium,
}

impl Faction {
    /// Every faction the card database names. `tests/influence.rs` pins this
    /// against the factions actually present in the data.
    pub const ALL: &'static [Faction] = &[
        Faction::Adam,
        Faction::Anarch,
        Faction::Apex,
        Faction::Criminal,
        Faction::HaasBioroid,
        Faction::Jinteki,
        Faction::Nbn,
        Faction::Neutral,
        Faction::Shaper,
        Faction::SunnyLebeau,
        Faction::WeylandConsortium,
    ];

    /// The card database's spelling, as `Card::faction` carries it.
    pub const fn name(self) -> &'static str {
        match self {
            Faction::Adam => "Adam",
            Faction::Anarch => "Anarch",
            Faction::Apex => "Apex",
            Faction::Criminal => "Criminal",
            Faction::HaasBioroid => "Haas-Bioroid",
            Faction::Jinteki => "Jinteki",
            Faction::Nbn => "NBN",
            Faction::Neutral => "Neutral",
            Faction::Shaper => "Shaper",
            Faction::SunnyLebeau => "Sunny Lebeau",
            Faction::WeylandConsortium => "Weyland Consortium",
        }
    }

    /// The icon code printed between brackets in card text. It is [`name`]
    /// lowercased with spaces hyphenated, and the test
    /// `icon_codes_are_the_slugified_names` keeps the two from drifting.
    ///
    /// [`name`]: Faction::name
    pub const fn icon_code(self) -> &'static str {
        match self {
            Faction::Adam => "adam",
            Faction::Anarch => "anarch",
            Faction::Apex => "apex",
            Faction::Criminal => "criminal",
            Faction::HaasBioroid => "haas-bioroid",
            Faction::Jinteki => "jinteki",
            Faction::Nbn => "nbn",
            Faction::Neutral => "neutral",
            Faction::Shaper => "shaper",
            Faction::SunnyLebeau => "sunny-lebeau",
            Faction::WeylandConsortium => "weyland-consortium",
        }
    }

    /// From the card database's spelling.
    pub fn from_name(s: &str) -> Option<Faction> {
        Faction::ALL.iter().copied().find(|f| f.name() == s)
    }

    /// From a printed icon code, unbracketed.
    pub fn from_icon_code(s: &str) -> Option<Faction> {
        Faction::ALL.iter().copied().find(|f| f.icon_code() == s)
    }
}

impl fmt::Display for Faction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The waiver
// ───────────────────────────────────────────────────────────────────────────

/// What a card's printed waiver asks of the deck it is in.
///
/// One variant per printed shape, and each variant names its own POLARITY.
/// Mumba Temple's condition is "15 **or fewer** ice" while every other card's
/// is "N **or more**"; a shared `{ bound, n }` field would let a parse bug
/// swap them and make the Temple free in exactly the decks where it should
/// cost. [`IceAtMost`] cannot be confused with [`AssetsAtLeast`].
///
/// [`IceAtMost`]: Waiver::IceAtMost
/// [`AssetsAtLeast`]: Waiver::AssetsAtLeast
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Waiver {
    /// "…if you have `n` or more non-alliance \[`faction`\] cards in your
    /// deck." — the eight Alliance cards. Copies of that faction, excluding
    /// every card with the Alliance subtype (CR 2.16).
    NonAllianceFactionAtLeast { faction: Faction, n: u32 },
    /// "…if you have `0` or fewer ice in your deck." — Mumba Temple, and the
    /// only card whose condition is an upper bound.
    IceAtMost(u32),
    /// "…if you have `0` or more assets in your deck." — Mumbad Virtual Tour.
    AssetsAtLeast(u32),
    /// "…if you have `0` or more cards in your deck." — Museum of History.
    CardsAtLeast(u32),
    /// "…if you have `n` `title`s in your deck." — PAD Factory. Printed as a
    /// bare number ("3 PAD Campaigns"); read as "at least", which is the same
    /// predicate here because CR 1.4.7 caps PAD Campaign at 3 copies, and is
    /// the reading that survives a card whose text stipulates a higher limit.
    NamedCopiesAtLeast { title: String, n: u32 },
}

/// A printed waiver sentence this module could not read.
///
/// Reported, never swallowed: the validator turns it into a deck problem and
/// `tests/influence.rs` turns it into a failing build. The alternative — a
/// parser that shrugs and charges full price — is the defect this whole
/// module exists to remove, one card later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverParseError {
    /// The sentence as printed, markup stripped.
    pub sentence: String,
    /// Which step of the grammar failed.
    pub reason: &'static str,
}

impl fmt::Display for WaiverParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} in \"{}\"", self.reason, self.sentence)
    }
}

impl std::error::Error for WaiverParseError {}

// ───────────────────────────────────────────────────────────────────────────
// Parsing
// ───────────────────────────────────────────────────────────────────────────

/// Printed text with NSG's markup removed and its whitespace normalized.
///
/// The data wraps words in `<strong>…</strong>` (`non-<strong>alliance
/// </strong>`) and Product Recall separates the icon with a non-breaking
/// space. Both are typography, not grammar, and neither survives to the
/// parser.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

/// The sentence around `at`, for an error message: back to the previous full
/// stop, forward to the next one.
fn sentence_around(plain: &str, at: usize) -> String {
    let start = plain[..at].rfind(". ").map_or(0, |i| i + 2);
    let end = plain[at..].find('.').map_or(plain.len(), |i| at + i + 1);
    plain[start..end].trim().to_string()
}

/// A word title-cased for lookup against a canonical spelling: "alliance" →
/// "Alliance". The printed text lowercases the subtype the sentence excludes;
/// [`Subtype::from_canonical`] is deliberately exact-case, so the parse
/// title-cases and lets the closed enum reject anything that is not a real
/// subtype.
fn title_cased(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Parse a card's printed text into its waiver, if it prints one.
///
/// * `Ok(None)` — no waiver sentence in the text (the overwhelming majority).
/// * `Ok(Some(w))` — the sentence, read.
/// * `Err(e)` — the text says "costs 0 influence" in a shape this grammar
///   does not cover. A new predicate to implement, not a card to ignore.
pub fn parse_waiver(text: &str) -> Result<Option<Waiver>, WaiverParseError> {
    let plain = plain(text);
    let Some(at) = plain.find(MARKER) else { return Ok(None) };
    let sentence = sentence_around(&plain, at);
    let err = |reason: &'static str| WaiverParseError { sentence: sentence.clone(), reason };

    // "This <noun> costs 0 influence if you have <quantity> in your deck."
    let after = &plain[at + MARKER.len()..];
    let rest = after
        .strip_prefix(" if you have ")
        .ok_or_else(|| err("the waiver is not conditioned on \"if you have …\""))?;
    let end = rest
        .find(" in your deck")
        .ok_or_else(|| err("the condition does not end \"… in your deck\""))?;
    parse_quantity(&rest[..end]).map(Some).map_err(err)
}

/// The condition between "if you have " and " in your deck".
fn parse_quantity(quant: &str) -> Result<Waiver, &'static str> {
    let (count, subject) =
        quant.split_once(' ').ok_or("the condition is not \"<number> <subject>\"")?;
    let n: u32 = count.parse().map_err(|_| "the condition does not open with a number")?;

    // "N or more …" / "N or fewer …" — a bare "N <thing>s" is the copy count.
    let Some((bound, subject)) = subject
        .strip_prefix("or more ")
        .map(|s| ("or more", s))
        .or_else(|| subject.strip_prefix("or fewer ").map(|s| ("or fewer", s)))
    else {
        return parse_named_copies(subject, n);
    };

    match (bound, subject) {
        ("or fewer", "ice") => Ok(Waiver::IceAtMost(n)),
        ("or more", "assets") => Ok(Waiver::AssetsAtLeast(n)),
        ("or more", "cards") => Ok(Waiver::CardsAtLeast(n)),
        ("or more", s) if s.starts_with("non-") => parse_non_subtype_faction(s, n),
        // Every combination NSG has never printed: an upper bound on assets,
        // a lower bound on ice, a subject nothing counts. Reported, so that
        // the card whose text invents one is looked at by a human.
        _ => Err("the condition counts something with a bound this grammar does not print"),
    }
}

/// "non-alliance \[haas-bioroid\] cards".
fn parse_non_subtype_faction(subject: &str, n: u32) -> Result<Waiver, &'static str> {
    let rest = subject.strip_prefix("non-").ok_or("not a \"non-…\" condition")?;
    let (word, rest) = rest.split_once(' ').ok_or("\"non-\" names nothing")?;
    // The excluded class must be a real CR 2.16 subtype, and the only one
    // ever printed here is Alliance. Anything else is a shape to implement,
    // not a sentence to guess at.
    match Subtype::from_canonical(&title_cased(word)) {
        Some(Subtype::Alliance) => {}
        Some(_) => return Err("the condition excludes a subtype other than Alliance"),
        None => return Err("the condition excludes something that is not a subtype"),
    }
    let code = rest
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix("] cards"))
        .ok_or("the condition does not read \"[<faction>] cards\"")?;
    let faction = Faction::from_icon_code(code).ok_or("the condition names no known faction")?;
    Ok(Waiver::NonAllianceFactionAtLeast { faction, n })
}

/// "3 PAD Campaigns" — a bare count of a named card.
///
/// The subject is the card's title pluralized, so the parse strips the plural
/// and REQUIRES the result to be a real card title. A title that resolves to
/// nothing would make the count meaningless, and is an error rather than a
/// condition that can never be met.
fn parse_named_copies(subject: &str, n: u32) -> Result<Waiver, &'static str> {
    let candidates = [subject.strip_suffix('s').unwrap_or(subject), subject];
    for title in candidates {
        if let Some(card) = carddata::by_title(title) {
            return Ok(Waiver::NamedCopiesAtLeast { title: card.title.clone(), n });
        }
    }
    Err("the condition counts copies of a card no title in the database matches")
}

/// The waiver a card prints, if any.
pub fn waiver_of(card: &Card) -> Result<Option<Waiver>, WaiverParseError> {
    match card.text.as_deref() {
        Some(t) => parse_waiver(t),
        None => Ok(None),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Evaluating a waiver against a deck
// ───────────────────────────────────────────────────────────────────────────

/// The deck a waiver is evaluated against, tallied once.
///
/// Everything here is counted BY COPY. CR 1.4.3a puts the identity (and any
/// extra cards it brings) outside the deck, so neither is tallied — build
/// this from the deck's card lines alone.
#[derive(Debug, Clone, Default)]
pub struct DeckCounts {
    cards: u32,
    ice: u32,
    assets: u32,
    /// Faction → copies of that faction WITHOUT the Alliance subtype.
    non_alliance_by_faction: HashMap<Faction, u32>,
    /// Title → copies.
    copies_by_title: HashMap<String, u32>,
}

impl DeckCounts {
    /// Tally a deck given as `(card, copies)` lines. Identity cards are
    /// skipped: a deck that somehow contains one has a placement problem
    /// (CR 1.4.4) and the identity is still not part of the deck.
    pub fn tally<'a, I>(lines: I) -> DeckCounts
    where
        I: IntoIterator<Item = (&'a Card, u32)>,
    {
        let mut c = DeckCounts::default();
        for (card, qty) in lines {
            if qty == 0 || card.is_identity() {
                continue;
            }
            c.cards += qty;
            if card.is_ice() {
                c.ice += qty;
            }
            if card.is_asset() {
                c.assets += qty;
            }
            if let Some(f) = card.faction.as_deref().and_then(Faction::from_name) {
                if !card.has_subtype(Subtype::Alliance) {
                    *c.non_alliance_by_faction.entry(f).or_default() += qty;
                }
            }
            *c.copies_by_title.entry(card.title.clone()).or_default() += qty;
        }
        c
    }

    /// Does this deck satisfy the waiver? Polarity lives in the variant, so
    /// each arm reads as its own sentence does.
    pub fn satisfies(&self, w: &Waiver) -> bool {
        match w {
            Waiver::NonAllianceFactionAtLeast { faction, n } => {
                self.non_alliance_by_faction.get(faction).copied().unwrap_or(0) >= *n
            }
            Waiver::IceAtMost(n) => self.ice <= *n,
            Waiver::AssetsAtLeast(n) => self.assets >= *n,
            Waiver::CardsAtLeast(n) => self.cards >= *n,
            Waiver::NamedCopiesAtLeast { title, n } => {
                self.copies_by_title.get(title).copied().unwrap_or(0) >= *n
            }
        }
    }
}

/// Is this card's influence waived in this deck?
///
/// The waiver makes the cost ZERO. It is not a reduction and does not stack
/// with anything: a waived card contributes nothing at all, whatever its
/// printed pips (`validator.cljc:113-121`).
pub fn is_waived(card: &Card, counts: &DeckCounts) -> Result<bool, WaiverParseError> {
    Ok(waiver_of(card)?.is_some_and(|w| counts.satisfies(&w)))
}

/// The influence `qty` copies of `card` cost in this deck, given the card is
/// out of faction and prints `pips`.
///
/// `Err` carries the unreadable sentence AND the full price, because a caller
/// that must still produce a number should produce the conservative one — but
/// only alongside the error it is required to report.
pub fn line_cost(
    card: &Card,
    pips: i64,
    qty: u32,
    counts: &DeckCounts,
) -> Result<i64, (WaiverParseError, i64)> {
    let full = pips * i64::from(qty);
    match is_waived(card, counts) {
        Ok(true) => Ok(0),
        Ok(false) => Ok(full),
        Err(e) => Err((e, full)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(title: &str) -> &'static Card {
        carddata::by_title(title).unwrap_or_else(|| panic!("{title} is in the card database"))
    }

    fn waiver(title: &str) -> Waiver {
        waiver_of(card(title))
            .unwrap_or_else(|e| panic!("{title} prints a readable waiver, got: {e}"))
            .unwrap_or_else(|| panic!("{title} prints a waiver at all"))
    }

    #[test]
    fn icon_codes_are_the_slugified_names() {
        for f in Faction::ALL {
            let slug = f.name().to_lowercase().replace(' ', "-");
            assert_eq!(
                f.icon_code(),
                slug,
                "{f:?}'s icon code must be its database name slugified; the two \
                 spellings drifted",
            );
        }
    }

    #[test]
    fn markup_and_nonbreaking_spaces_do_not_reach_the_parser() {
        assert_eq!(
            plain("non-<strong>alliance</strong>\u{a0}[haas-bioroid]"),
            "non-alliance [haas-bioroid]",
            "NSG's <strong> wrapper and Product Recall's non-breaking space are \
             typography, and must be gone before the grammar sees the sentence",
        );
    }

    #[test]
    fn each_printed_shape_parses_to_its_own_predicate() {
        assert_eq!(
            waiver("Mumba Temple"),
            Waiver::IceAtMost(15),
            "Mumba Temple's condition is an UPPER bound on ice",
        );
        assert_eq!(
            waiver("Mumbad Virtual Tour"),
            Waiver::AssetsAtLeast(7),
            "Mumbad Virtual Tour wants 7 or more assets",
        );
        assert_eq!(
            waiver("Museum of History"),
            Waiver::CardsAtLeast(50),
            "Museum of History wants a 50-card deck",
        );
        assert_eq!(
            waiver("PAD Factory"),
            Waiver::NamedCopiesAtLeast { title: "PAD Campaign".to_string(), n: 3 },
            "PAD Factory counts copies of a named card, depluralized to its title",
        );
        assert_eq!(
            waiver("Jeeves Model Bioroids"),
            Waiver::NonAllianceFactionAtLeast { faction: Faction::HaasBioroid, n: 6 },
            "an Alliance card wants 6 non-Alliance cards of its own faction",
        );
        assert_eq!(
            waiver("Product Recall"),
            Waiver::NonAllianceFactionAtLeast { faction: Faction::HaasBioroid, n: 6 },
            "Product Recall parses despite the non-breaking space before its icon",
        );
    }

    #[test]
    fn a_card_with_no_waiver_sentence_parses_to_none() {
        assert_eq!(
            waiver_of(card("Hedge Fund")),
            Ok(None),
            "Hedge Fund prints no waiver, which is not the same as an unreadable one",
        );
        assert_eq!(
            waiver_of(card("Asa Group: Security Through Vigilance")),
            Ok(None),
            "nor does an identity",
        );
    }

    #[test]
    fn an_unreadable_waiver_sentence_is_an_error_not_full_price() {
        for (text, why) in [
            ("This card costs 0 influence.", "no condition at all"),
            (
                "This card costs 0 influence if you have 6 or more non-alliance \
                 [mumbad] cards in your deck.",
                "a faction that does not exist",
            ),
            (
                "This card costs 0 influence if you have 6 or more non-region \
                 [nbn] cards in your deck.",
                "a subtype other than Alliance",
            ),
            (
                "This card costs 0 influence if you have 6 or more non-frobnicate \
                 [nbn] cards in your deck.",
                "a word that is not a subtype at all",
            ),
            (
                "This card costs 0 influence if you have 15 or fewer assets in \
                 your deck.",
                "a bound/subject pair nothing prints",
            ),
            (
                "This card costs 0 influence if you have 3 Blorpo Campaigns in \
                 your deck.",
                "copies of a card that does not exist",
            ),
            ("This card costs 0 influence if you have lots of ice in your deck.", "no number"),
        ] {
            assert!(
                parse_waiver(text).is_err(),
                "a waiver sentence naming {why} must be reported as an error, never \
                 silently charged at full price: {text:?}",
            );
        }
    }

    #[test]
    fn polarity_is_not_reversible() {
        // 15 ice: satisfied. 16 ice: not. This is the case a swapped
        // comparison would invert, making the Temple free exactly where it
        // ought to cost.
        let mut c = DeckCounts::default();
        c.ice = 15;
        assert!(c.satisfies(&Waiver::IceAtMost(15)), "15 ice is \"15 or fewer\"");
        c.ice = 16;
        assert!(!c.satisfies(&Waiver::IceAtMost(15)), "16 ice is not \"15 or fewer\"");
        c.assets = 7;
        assert!(c.satisfies(&Waiver::AssetsAtLeast(7)), "7 assets is \"7 or more\"");
        c.assets = 6;
        assert!(!c.satisfies(&Waiver::AssetsAtLeast(7)), "6 assets is not \"7 or more\"");
    }

    #[test]
    fn the_alliance_cards_do_not_count_toward_their_own_condition() {
        // Six Jeeves and nothing else: six Haas-Bioroid cards in the deck,
        // none of them non-Alliance, so the condition is not met by the
        // Alliance cards themselves.
        let counts = DeckCounts::tally([(card("Jeeves Model Bioroids"), 6)]);
        assert!(
            !counts.satisfies(&waiver("Jeeves Model Bioroids")),
            "\"non-alliance\" excludes the Alliance cards, so a pile of Jeeves can \
             never pay for itself",
        );
        let counts = DeckCounts::tally([
            (card("Jeeves Model Bioroids"), 3),
            (card("Estelle Moon"), 3),
            (card("Drafter"), 3),
        ]);
        assert!(
            counts.satisfies(&waiver("Jeeves Model Bioroids")),
            "six non-Alliance Haas-Bioroid cards is the threshold, met exactly",
        );
    }

    #[test]
    fn the_identity_is_not_in_the_deck() {
        let counts =
            DeckCounts::tally([(card("Asa Group: Security Through Vigilance"), 1), (card("Vanilla"), 1)]);
        assert!(
            !counts.satisfies(&Waiver::CardsAtLeast(2)),
            "CR 1.4.3a: the identity is not part of the deck and cannot be one of \
             the cards a waiver counts",
        );
    }

    #[test]
    fn a_waived_card_costs_zero_not_less() {
        let counts = DeckCounts::tally([(card("Mumba Temple"), 3), (card("Vanilla"), 3)]);
        assert_eq!(
            line_cost(card("Mumba Temple"), 2, 3, &counts),
            Ok(0),
            "three Temples at 2 pips in a 3-ice deck are free outright, not 6 minus \
             something",
        );
    }
}
