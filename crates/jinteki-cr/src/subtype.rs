//! CR 2.16: subtypes, as a closed type.
//!
//! # Why this is an enum and not a string
//!
//! Subtypes used to be `&'static str` on [`crate::object::PrintedCard`],
//! spelled by whoever wrote the site. The card layer spells them as they are
//! printed — "Region", "Icebreaker", "Code Gate" — and several kernel sites
//! spelled them lowercase. A lowercase literal matches no real card, so the
//! rule reading it silently never fired, and the kernel tests passed because
//! the testkit fixtures spelled them lowercase too: the fixture agreed with
//! the defect. Two rules were dead this way (CR 3.6.5's one-region-per-root
//! limit and CR 3.9.5b's implicit encounter duration on an icebreaker's
//! self-pump), and nothing in the build could have said so.
//!
//! A closed enum makes the whole class impossible: there is no way to write a
//! subtype that is not one of NSG's, and no way to misspell one, because the
//! spelling exists in exactly one place — [`Subtype::as_str`]. A mismatch is
//! now a compile error rather than a silent no-op.
//!
//! The variants are generated from `data/card_subtypes.json`, vendored from
//! NSG's `netrunner-cards-json` (`v2/card_subtypes.json`) so that nothing
//! here reads outside the workspace. `subtype_guard.rs` pins this list
//! against that file and against every subtype-shaped literal in the tree.

use std::fmt;

/// CR 2.16: "A subtype is a property of a card that has no inherent effect
/// but can be referred to by other card abilities and rules."
///
/// Ordering is by canonical name, which is what [`crate::object::Effective`]
/// stores subtypes in a `BTreeSet` by — so iteration order is stable and
/// alphabetical, and does not depend on the order effects applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// "Academic"
    Academic,
    /// "AI"
    Ai,
    /// "AP"
    Ap,
    /// "Advertisement"
    Advertisement,
    /// "Alliance"
    Alliance,
    /// "Ambush"
    Ambush,
    /// "Barrier"
    Barrier,
    /// "Beanstalk"
    Beanstalk,
    /// "Bioroid"
    Bioroid,
    /// "Black Ops"
    BlackOps,
    /// "Bomb"
    Bomb,
    /// "Caïssa"
    Caissa,
    /// "Cast"
    Cast,
    /// "Character"
    Character,
    /// "Chip"
    Chip,
    /// "Clan"
    Clan,
    /// "Clone"
    Clone,
    /// "Cloud"
    Cloud,
    /// "Code Gate"
    CodeGate,
    /// "Companion"
    Companion,
    /// "Condition"
    Condition,
    /// "Connection"
    Connection,
    /// "Console"
    Console,
    /// "Consumer-grade"
    ConsumerGrade,
    /// "Corp"
    Corp,
    /// "Corporation"
    Corporation,
    /// "Current"
    Current,
    /// "Cybernetic"
    Cybernetic,
    /// "Cyborg"
    Cyborg,
    /// "Daemon"
    Daemon,
    /// "Decoder"
    Decoder,
    /// "Deep Net"
    DeepNet,
    /// "Deflector"
    Deflector,
    /// "Department"
    Department,
    /// "Destroyer"
    Destroyer,
    /// "Deva"
    Deva,
    /// "Digital"
    Digital,
    /// "Directive"
    Directive,
    /// "Division"
    Division,
    /// "Double"
    Double,
    /// "Enforcer"
    Enforcer,
    /// "Executive"
    Executive,
    /// "Expansion"
    Expansion,
    /// "Expendable"
    Expendable,
    /// "Facility"
    Facility,
    /// "Fracter"
    Fracter,
    /// "G-mod"
    GMod,
    /// "Gear"
    Gear,
    /// "Genetics"
    Genetics,
    /// "Government"
    Government,
    /// "Grail"
    Grail,
    /// "Gray Ops"
    GrayOps,
    /// "Harmonic"
    Harmonic,
    /// "Hostile"
    Hostile,
    /// "Icebreaker"
    Icebreaker,
    /// "Industrial"
    Industrial,
    /// "Initiative"
    Initiative,
    /// "Job"
    Job,
    /// "Killer"
    Killer,
    /// "Liability"
    Liability,
    /// "Link"
    Link,
    /// "Location"
    Location,
    /// "Lockdown"
    Lockdown,
    /// "Mandate"
    Mandate,
    /// "Megacorp"
    Megacorp,
    /// "Mod"
    Mod,
    /// "Morph"
    Morph,
    /// "Mythic"
    Mythic,
    /// "NEXT"
    Next,
    /// "Natural"
    Natural,
    /// "Observer"
    Observer,
    /// "Off-site"
    OffSite,
    /// "Orgcrime"
    Orgcrime,
    /// "Police Department"
    PoliceDepartment,
    /// "Political"
    Political,
    /// "Priority"
    Priority,
    /// "Psi"
    Psi,
    /// "Public"
    Public,
    /// "Region"
    Region,
    /// "Remote"
    Remote,
    /// "Reprisal"
    Reprisal,
    /// "Research"
    Research,
    /// "Ritzy"
    Ritzy,
    /// "Run"
    Run,
    /// "Sabotage"
    Sabotage,
    /// "Security"
    Security,
    /// "Security Protocol"
    SecurityProtocol,
    /// "Seedy"
    Seedy,
    /// "Sensie"
    Sensie,
    /// "Sentry"
    Sentry,
    /// "Source"
    Source,
    /// "Stealth"
    Stealth,
    /// "Subsidiary"
    Subsidiary,
    /// "Sysop"
    Sysop,
    /// "Terminal"
    Terminal,
    /// "Tracer"
    Tracer,
    /// "Transaction"
    Transaction,
    /// "Trap"
    Trap,
    /// "Triple"
    Triple,
    /// "Trojan"
    Trojan,
    /// "Unorthodox"
    Unorthodox,
    /// "Unsubstantiated"
    Unsubstantiated,
    /// "Vehicle"
    Vehicle,
    /// "Virtual"
    Virtual,
    /// "Virus"
    Virus,
    /// "Weapon"
    Weapon,
}

impl Subtype {
    /// The canonical printed spelling — the ONLY place a subtype is spelled.
    pub const fn as_str(self) -> &'static str {
        match self {
            Subtype::Academic => "Academic",
            Subtype::Ai => "AI",
            Subtype::Ap => "AP",
            Subtype::Advertisement => "Advertisement",
            Subtype::Alliance => "Alliance",
            Subtype::Ambush => "Ambush",
            Subtype::Barrier => "Barrier",
            Subtype::Beanstalk => "Beanstalk",
            Subtype::Bioroid => "Bioroid",
            Subtype::BlackOps => "Black Ops",
            Subtype::Bomb => "Bomb",
            Subtype::Caissa => "Caïssa",
            Subtype::Cast => "Cast",
            Subtype::Character => "Character",
            Subtype::Chip => "Chip",
            Subtype::Clan => "Clan",
            Subtype::Clone => "Clone",
            Subtype::Cloud => "Cloud",
            Subtype::CodeGate => "Code Gate",
            Subtype::Companion => "Companion",
            Subtype::Condition => "Condition",
            Subtype::Connection => "Connection",
            Subtype::Console => "Console",
            Subtype::ConsumerGrade => "Consumer-grade",
            Subtype::Corp => "Corp",
            Subtype::Corporation => "Corporation",
            Subtype::Current => "Current",
            Subtype::Cybernetic => "Cybernetic",
            Subtype::Cyborg => "Cyborg",
            Subtype::Daemon => "Daemon",
            Subtype::Decoder => "Decoder",
            Subtype::DeepNet => "Deep Net",
            Subtype::Deflector => "Deflector",
            Subtype::Department => "Department",
            Subtype::Destroyer => "Destroyer",
            Subtype::Deva => "Deva",
            Subtype::Digital => "Digital",
            Subtype::Directive => "Directive",
            Subtype::Division => "Division",
            Subtype::Double => "Double",
            Subtype::Enforcer => "Enforcer",
            Subtype::Executive => "Executive",
            Subtype::Expansion => "Expansion",
            Subtype::Expendable => "Expendable",
            Subtype::Facility => "Facility",
            Subtype::Fracter => "Fracter",
            Subtype::GMod => "G-mod",
            Subtype::Gear => "Gear",
            Subtype::Genetics => "Genetics",
            Subtype::Government => "Government",
            Subtype::Grail => "Grail",
            Subtype::GrayOps => "Gray Ops",
            Subtype::Harmonic => "Harmonic",
            Subtype::Hostile => "Hostile",
            Subtype::Icebreaker => "Icebreaker",
            Subtype::Industrial => "Industrial",
            Subtype::Initiative => "Initiative",
            Subtype::Job => "Job",
            Subtype::Killer => "Killer",
            Subtype::Liability => "Liability",
            Subtype::Link => "Link",
            Subtype::Location => "Location",
            Subtype::Lockdown => "Lockdown",
            Subtype::Mandate => "Mandate",
            Subtype::Megacorp => "Megacorp",
            Subtype::Mod => "Mod",
            Subtype::Morph => "Morph",
            Subtype::Mythic => "Mythic",
            Subtype::Next => "NEXT",
            Subtype::Natural => "Natural",
            Subtype::Observer => "Observer",
            Subtype::OffSite => "Off-site",
            Subtype::Orgcrime => "Orgcrime",
            Subtype::PoliceDepartment => "Police Department",
            Subtype::Political => "Political",
            Subtype::Priority => "Priority",
            Subtype::Psi => "Psi",
            Subtype::Public => "Public",
            Subtype::Region => "Region",
            Subtype::Remote => "Remote",
            Subtype::Reprisal => "Reprisal",
            Subtype::Research => "Research",
            Subtype::Ritzy => "Ritzy",
            Subtype::Run => "Run",
            Subtype::Sabotage => "Sabotage",
            Subtype::Security => "Security",
            Subtype::SecurityProtocol => "Security Protocol",
            Subtype::Seedy => "Seedy",
            Subtype::Sensie => "Sensie",
            Subtype::Sentry => "Sentry",
            Subtype::Source => "Source",
            Subtype::Stealth => "Stealth",
            Subtype::Subsidiary => "Subsidiary",
            Subtype::Sysop => "Sysop",
            Subtype::Terminal => "Terminal",
            Subtype::Tracer => "Tracer",
            Subtype::Transaction => "Transaction",
            Subtype::Trap => "Trap",
            Subtype::Triple => "Triple",
            Subtype::Trojan => "Trojan",
            Subtype::Unorthodox => "Unorthodox",
            Subtype::Unsubstantiated => "Unsubstantiated",
            Subtype::Vehicle => "Vehicle",
            Subtype::Virtual => "Virtual",
            Subtype::Virus => "Virus",
            Subtype::Weapon => "Weapon",
        }
    }

    /// Every subtype NSG defines, in canonical order. The guard test pins
    /// this against the vendored list, so "every subtype" stays true.
    pub const ALL: &'static [Subtype] = &[
        Subtype::Academic,
        Subtype::Ai,
        Subtype::Ap,
        Subtype::Advertisement,
        Subtype::Alliance,
        Subtype::Ambush,
        Subtype::Barrier,
        Subtype::Beanstalk,
        Subtype::Bioroid,
        Subtype::BlackOps,
        Subtype::Bomb,
        Subtype::Caissa,
        Subtype::Cast,
        Subtype::Character,
        Subtype::Chip,
        Subtype::Clan,
        Subtype::Clone,
        Subtype::Cloud,
        Subtype::CodeGate,
        Subtype::Companion,
        Subtype::Condition,
        Subtype::Connection,
        Subtype::Console,
        Subtype::ConsumerGrade,
        Subtype::Corp,
        Subtype::Corporation,
        Subtype::Current,
        Subtype::Cybernetic,
        Subtype::Cyborg,
        Subtype::Daemon,
        Subtype::Decoder,
        Subtype::DeepNet,
        Subtype::Deflector,
        Subtype::Department,
        Subtype::Destroyer,
        Subtype::Deva,
        Subtype::Digital,
        Subtype::Directive,
        Subtype::Division,
        Subtype::Double,
        Subtype::Enforcer,
        Subtype::Executive,
        Subtype::Expansion,
        Subtype::Expendable,
        Subtype::Facility,
        Subtype::Fracter,
        Subtype::GMod,
        Subtype::Gear,
        Subtype::Genetics,
        Subtype::Government,
        Subtype::Grail,
        Subtype::GrayOps,
        Subtype::Harmonic,
        Subtype::Hostile,
        Subtype::Icebreaker,
        Subtype::Industrial,
        Subtype::Initiative,
        Subtype::Job,
        Subtype::Killer,
        Subtype::Liability,
        Subtype::Link,
        Subtype::Location,
        Subtype::Lockdown,
        Subtype::Mandate,
        Subtype::Megacorp,
        Subtype::Mod,
        Subtype::Morph,
        Subtype::Mythic,
        Subtype::Next,
        Subtype::Natural,
        Subtype::Observer,
        Subtype::OffSite,
        Subtype::Orgcrime,
        Subtype::PoliceDepartment,
        Subtype::Political,
        Subtype::Priority,
        Subtype::Psi,
        Subtype::Public,
        Subtype::Region,
        Subtype::Remote,
        Subtype::Reprisal,
        Subtype::Research,
        Subtype::Ritzy,
        Subtype::Run,
        Subtype::Sabotage,
        Subtype::Security,
        Subtype::SecurityProtocol,
        Subtype::Seedy,
        Subtype::Sensie,
        Subtype::Sentry,
        Subtype::Source,
        Subtype::Stealth,
        Subtype::Subsidiary,
        Subtype::Sysop,
        Subtype::Terminal,
        Subtype::Tracer,
        Subtype::Transaction,
        Subtype::Trap,
        Subtype::Triple,
        Subtype::Trojan,
        Subtype::Unorthodox,
        Subtype::Unsubstantiated,
        Subtype::Vehicle,
        Subtype::Virtual,
        Subtype::Virus,
        Subtype::Weapon,
    ];

    /// Parse a canonical spelling, EXACT CASE. Deliberately case-sensitive:
    /// this is the boundary where outside data (NSG JSON, a saved game) comes
    /// in, and "icebreaker" is not a subtype — "Icebreaker" is. Accepting the
    /// wrong case here would reintroduce exactly the drift the enum removes.
    pub fn from_canonical(s: &str) -> Option<Subtype> {
        Subtype::ALL.iter().copied().find(|t| t.as_str() == s)
    }
}

impl fmt::Display for Subtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serialized as the canonical printed string, so the wire format the UI
/// already consumes is unchanged by the enum.
impl serde::Serialize for Subtype {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Subtype {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Subtype, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        Subtype::from_canonical(&s).ok_or_else(|| {
            serde::de::Error::custom(format!("not a CR 2.16 subtype: {s:?}"))
        })
    }
}

