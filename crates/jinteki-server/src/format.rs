//! Tournament formats, as a closed type.
//!
//! # What a format is, and what it is not
//!
//! A format is a TOURNAMENT construct, not a rules one: NSG publishes a list
//! of formats, and for each one a dated series of snapshots naming a card pool
//! and a restriction (ban/points) list. The Comprehensive Rules say almost
//! nothing about them — CR 1.4.2 settles legality before the game begins and
//! then never mentions a format again. So nothing in this module cites a CR
//! rule, because no CR rule decides any of it.
//!
//! # Why this is an enum and not a string
//!
//! This repository has been bitten twice by stringly-typed card data. CR 2.16
//! subtypes were `&'static str`: the card layer spelled them as printed
//! ("Region"), several kernel sites spelled them lowercase, a lowercase
//! literal matched no real card, and two rules were silently dead — with the
//! test fixtures spelling them lowercase too, so the tests agreed with the
//! defect. [`jinteki_cr::Subtype`] is now a closed enum with a guard test
//! (`crates/jinteki-cr/tests/subtypes.rs`); this module is that pattern
//! applied to the next piece of NSG vocabulary the tree needed, before it
//! could become the third instance.
//!
//! The spelling of a format exists in exactly one place — [`Format::as_str`] —
//! and it is NSG's own `id`, the vocabulary `formats/*.json`,
//! `card_pools/*.json` and every decklist's `mwl_code` all speak.
//!
//! # The pool data, and where the computation lives
//!
//! A format's CURRENT card pool is derived, not stored here: NSG's snapshots
//! are dated, the pools are named, and the join runs
//! `formats/<f>.json`.snapshots → `card_pools/<f>.json` → `card_sets.json` →
//! `printings/<set>.json`. That data is vendored verbatim under
//! `crates/jinteki-server/data/nsg-v2/` (the workspace never reads outside
//! itself), and the derivation lives in the guard test
//! `crates/jinteki-server/tests/formats.rs`, which pins this enum against
//! NSG's list, proves the containment chain, and — the point of the exercise
//! — recomputes the format of every deck we carry and refuses a build where a
//! recorded [`Format`] and the deck's actual contents disagree.
//!
//! Deliberately no pool data at runtime: the recorded [`Format`] on a
//! [`crate::cr::DeckSpec`] IS the metadata a UI would surface, so a UI that
//! grows a Standard or Startup shelf reads the field and re-derives nothing.

use std::fmt;

/// A tournament format NSG publishes (`v2/formats/*.json`).
///
/// Ordering is by canonical id, which is the order the format files sort in —
/// stable, and independent of the containment chain, which is a separate fact
/// about the pools ([`Format::CHAIN`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    /// "eternal" — everything ever printed for constructed play.
    Eternal,
    /// "ram" — Random Access Memories, the rotating-cube format.
    Ram,
    /// "snapshot" — the frozen 2018 pool.
    Snapshot,
    /// "standard" — the current championship format.
    Standard,
    /// "startup" — the current small-pool format.
    Startup,
    /// "system_gateway" — the two-product beginner format.
    SystemGateway,
}

impl Format {
    /// NSG's own `id` for this format — the ONLY place a format is spelled.
    pub const fn as_str(self) -> &'static str {
        match self {
            Format::Eternal => "eternal",
            Format::Ram => "ram",
            Format::Snapshot => "snapshot",
            Format::Standard => "standard",
            Format::Startup => "startup",
            Format::SystemGateway => "system_gateway",
        }
    }

    /// NSG's display name, for a UI that shows a format to a player.
    pub const fn display_name(self) -> &'static str {
        match self {
            Format::Eternal => "Eternal",
            Format::Ram => "Random Access Memories",
            Format::Snapshot => "Snapshot",
            Format::Standard => "Standard",
            Format::Startup => "Startup",
            Format::SystemGateway => "System Gateway",
        }
    }

    /// Every format NSG defines, in canonical id order. The guard test pins
    /// this against the vendored `formats/` directory, so "every format"
    /// stays true when NSG publishes the next one.
    pub const ALL: &'static [Format] = &[
        Format::Eternal,
        Format::Ram,
        Format::Snapshot,
        Format::Standard,
        Format::Startup,
        Format::SystemGateway,
    ];

    /// The constructed chain, NARROWEST FIRST — the only formats a deck is
    /// classified into, because they are the only ones whose current pools are
    /// totally ordered by containment.
    ///
    /// This ordering is an ASSERTION ABOUT THE DATA, not a convention: the
    /// guard test recomputes the subset relation over all six current pools
    /// and fails if this is not exactly the chain. `ram` and `snapshot` are
    /// left out because they are not on it — each is a subset of `eternal` and
    /// comparable to nothing else, so "narrower than" is undefined for them
    /// and a deck cannot be classified into them without inventing an order.
    pub const CHAIN: &'static [Format] = &[
        Format::SystemGateway,
        Format::Startup,
        Format::Standard,
        Format::Eternal,
    ];

    /// Parse NSG's id, EXACT CASE. Deliberately case-sensitive for the same
    /// reason [`jinteki_cr::Subtype::from_canonical`] is: this is the boundary
    /// where outside data (NSG JSON, a decklist's `mwl_code`, a stored deck
    /// row) comes in, and "Standard" is not a format id — "standard" is.
    pub fn from_canonical(s: &str) -> Option<Format> {
        Format::ALL.iter().copied().find(|f| f.as_str() == s)
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serialized as NSG's id, so anything that already speaks that vocabulary —
/// the catalog endpoint, a decklist import — is unchanged by the enum.
impl serde::Serialize for Format {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Format {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Format, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        Format::from_canonical(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("not an NSG format id: {s:?}")))
    }
}
