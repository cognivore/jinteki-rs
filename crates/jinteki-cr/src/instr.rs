//! The Instruction vocabulary (§9.3.4, §9.11) for the W1 kernel wave.
//!
//! Card-text instructions (9.11.3: one sentence = one instruction) plus the
//! timing-structure-internal instructions the §11 step tables need
//! (9.11.2: each step in a timing structure forms a single instruction).
//! Every variant resolves to real state mutation — no silent no-ops.

use crate::effects::{DamageKind, EffectClass};
use crate::object::{CardType, ObjectId, ServerId, Side, Zone};

/// The ONE selector language for quantity positions (ARCHITECTURE §12
/// rule 5): a pure data expression evaluated against world state, returning
/// an integer. Calculated quantities (9.12.2) are these expressions; their
/// dependencies are readable from the expression itself, which is what lets
/// the characteristics pipeline (9.12.1) and calculated-quantity timing
/// (9.12.2) re-evaluate them reactively. Never a closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quantity {
    /// A printed constant.
    Const(i64),
    /// "…for each <object matching the criteria>" — the count of matching
    /// objects (9.12.2a). The criteria are the shared filter vocabulary and
    /// combine as a conjunction, exactly as a target announcement's and a
    /// search's do (§12 rule 5), so "every copy of the named card in the
    /// heap" is one selector and not a filter that has to be invented.
    /// 1.15.2c applies to the list as a whole: with no criterion naming a
    /// zone, only installed cards are counted.
    Count(Vec<TargetFilter>),
    /// "…N or more cards **that share a type**" (Slot Machine) — among the
    /// objects matching the criteria, the size of the LARGEST group sharing
    /// one card type (2.15: a card has exactly one type). The threshold the
    /// card compares this against is content on whatever asks (§12 rule 2),
    /// which is why Slot Machine's two subroutines — "2 or more" and "3 or
    /// more" — are the same selector twice and not two selectors.
    LargestGroupSharingCardType(Vec<TargetFilter>),
    /// "…for each <kind> counter hosted on this card" — counts hosted
    /// counters INCLUDING those set aside by a [trash] trigger cost (9.5.5).
    CountersOnSource(crate::object::CounterKind),
    /// Sum of two quantities ("2 plus 1 for each …").
    Plus(Box<Quantity>, Box<Quantity>),
    /// Difference of two quantities ("…past its advancement requirement").
    Minus(Box<Quantity>, Box<Quantity>),
    /// CR 2.4: the advancement requirement of the source agenda, as modified
    /// (a SanSan-class declaration lowers it). CR 1.17.8 / 10.13.2: for an
    /// agenda that has been scored or stolen, this — like
    /// [`Quantity::CountersOnSource`] — reads the last known value from
    /// before the move, since the real one no longer exists.
    RequirementOfSource,
    /// Scale ("N for each …").
    Times(i64, Box<Quantity>),
    /// CR 9.12.2a: "…1 for every N <things>" (Project Beale's "1 agenda
    /// counter for every 2 hosted advancement counters past 3"). The
    /// complement of [`Quantity::Times`] — integer division, so a remainder
    /// buys nothing — and a negative inner quantity yields 0, since there is
    /// no such thing as a negative number of things to count.
    PerEvery(Box<Quantity>, i64),
    /// "…for each credit lost" (Account Siphon class): credits the named
    /// player ACTUALLY lost during the resolution of the ability now
    /// resolving — the observed 1.10.3b loss, not the requested amount.
    CreditsLostThisAbility(Side),
    /// CR 7.3.6: "for each time they accessed a card during the run" — the
    /// number of accesses ACTUALLY PERFORMED during the run in progress (or
    /// the run that just ended, for a "when this run ends" ability). An
    /// access that was replaced by another effect never happened and is not
    /// counted.
    AccessesThisRun,
    /// CR 7.3.6 / 1.12.6: "…if you accessed a card this turn" (Hoshiko
    /// Shiro) — the number of accesses ACTUALLY PERFORMED since the current
    /// turn began, [`Quantity::AccessesThisRun`]'s count over the turn's
    /// history window instead of the run's, read from the change log (10.2.1
    /// open information). The threshold and its direction are content on
    /// whatever asks (§12 rule 2): "you accessed a card this turn" is this
    /// at least 1, and "you did not access any cards this turn" is this at
    /// most 0 — the only way a count says "none".
    AccessesThisTurn,
    /// CR 7.5 / 1.12.6: the number of agendas the Runner STOLE inside a
    /// history window — "if you stole an agenda during that run" (Mad Dash),
    /// "for each agenda you stole this turn".
    ///
    /// 7.5's steal is an occurrence, and `TriggerCond::RunnerStealsAgenda` is
    /// met by it as it happens. This is the other question, the one a "when
    /// this run ends" ability has to put: not "is a steal happening" but "did
    /// one happen inside this span". A condition met during the run is no
    /// answer to it, because by the time the later ability resolves the
    /// occurrence is over.
    ///
    /// The WINDOW is content (§12 rule 2), which is what
    /// [`Quantity::AccessesThisRun`] and [`Quantity::AccessesThisTurn`] are
    /// the older two-variant spelling of: the count is the same count, and a
    /// run and a turn are two spans of the same 1.12.6 history review. Read
    /// from the change log, which 10.2.1 makes open information.
    ///
    /// Only the Runner steals (7.5), so there is no side to carry. The
    /// threshold and its direction are content on whatever asks: "you stole
    /// an agenda" is this at least 1, and "you stole no agendas" is this at
    /// most 0.
    AgendasStolen(HistoryWindow),
    /// CR 1.20.4a: the Runner's UNUSED [mu] — the memory limit (1.20.2, as
    /// modified) minus the total memory cost of installed programs (1.20.3),
    /// which 1.20.4a names as a calculated value directly and rules that it
    /// ignores requirements about how restricted [mu] can be filled. "Your
    /// [mu] is full" (Dewi Subrotoputri, front) is this at most 0; "you have
    /// at least 1 unused [mu]" (the back) is this at least 1 — thresholds and
    /// directions as content (§12 rule 2), like every other quantity.
    UnusedMemory,
    /// CR 1.12.6: "for each piece of ice you passed during this run" — the
    /// number of DISTINCT ice objects the Runner passed, counted by reviewing
    /// the game history. An object that no longer exists in the present game
    /// state still counts, which is the whole of the rule.
    DistinctIcePassedThisRun,
    /// CR 9.8.7: "if you did not break any subroutines during that run" — the
    /// number of subroutines BROKEN since the run in progress began (or the
    /// run that just ended, for a "when this run ends" ability), read from
    /// the change log, which 10.2.1 makes open information. 0 outside a run.
    SubroutinesBrokenThisRun,
    /// CR 9.8.10: "after a subroutine resolved during that run" — the number
    /// of subroutines that RESOLVED during the run, the same history read
    /// from the other side. A subroutine that was broken never resolves, and
    /// a 9.8.9 replacement still resolves from the ice, so both are counted
    /// exactly as the rules record them.
    SubroutinesResolvedThisRun,
    /// CR 9.12.2e: a value defined by X, where the ability defining X lives
    /// on the source (the Surveyor class shares one X between a strength
    /// definition and a trace). While the defining ability is inactive or
    /// lost, X is treated as 0.
    XOfSource(Box<Quantity>),
    /// CR 1.16.2c: the value the payer ANNOUNCED for X, for the cost being
    /// paid right now. 1.16.2d: "if an ability needs to know the value of a
    /// cost in a context where that cost is not being paid, treat any X that
    /// appears in that cost as 0" — which is exactly what this reads outside
    /// a payment.
    AnnouncedX,
    /// CR 10.7: the number of tags the Runner has. A quantity position
    /// (§12 rule 6), used both in effects and as a 1.16.2c restriction on X.
    RunnerTags,
    /// CR 1.10.1: "all credits in their credit pool" (Closed Accounts) — the
    /// named player's credit POOL, which 1.13.3 keeps distinct from any
    /// credits hosted on cards.
    CreditsInPoolOf(Side),
    /// CR 1.11.3: "…if they have **no [click] remaining**" (Vertigo class) —
    /// the number of clicks the named player HAS, which 1.11.3a increases by
    /// a gain and 1.11.3b reduces by a loss or a spend. The side is content
    /// (§12 rule 2), read the way [`Quantity::CreditsInPoolOf`] reads a
    /// credit pool, and the threshold and its direction belong to whatever
    /// asks: "no [click] remaining" is this at most 0 and "at least
    /// 1[click]" is this at least 1.
    ///
    /// 1.11.3b's other half is why this is a count of what a player HAS and
    /// not of anything they did with it: "lose" and "spend" are not
    /// synonymous, so a sentence asking how many clicks were SPENT on
    /// something is a different question and reads the action's own record
    /// (1.16.4d), never this.
    ClicksOf(Side),
    /// CR 10.6.1: "…if the Corp has **at least 1 bad publicity**" (Blackmail
    /// class) — the bad publicity counters on the named player, which 1.9.5d
    /// makes a player-level count exactly as 1.9.5c makes tags one. The side
    /// is content (§12 rule 2), read the way [`Quantity::CreditsInPoolOf`]
    /// reads a pool; the threshold and its direction belong to whatever asks,
    /// so "at least 1 bad publicity" and "no bad publicity" are this selector
    /// twice.
    ///
    /// 10.6.2's bad publicity FUND is a different number and never this one:
    /// the fund holds credits the RUNNER controls, filled at step 6.9.1b and
    /// emptied at 6.9.6b, and 10.6.3c is explicit that bad publicity taken or
    /// removed after 6.9.1b leaves that run's fund alone. A sentence asking
    /// how much bad publicity a player HAS reads the counters.
    BadPublicityOf(Side),
    /// CR 4.6.6a: "…for each **remote server that has a card in its root and
    /// is protected by ice**" (Fully Operational class) — the number of
    /// SERVERS the description reaches, with the stipulations as content in
    /// the server-filter vocabulary ([`ServerFilter`]) exactly as
    /// [`Quantity::Count`] carries the ones it makes about cards.
    ///
    /// A server is not a card and no count of cards stands in for one:
    /// 4.6.6e lets a remote root hold an asset or agenda AND any number of
    /// upgrades, so counting the cards in the qualifying roots over-counts a
    /// server carrying an upgrade, and counting the ice protecting them
    /// over-counts a server behind two. Which servers there are to count is
    /// 4.6.7a and 4.6.8d: the three centrals always exist, and a remote
    /// exists while at least one card is installed in its root or protecting
    /// it.
    CountServers(Vec<ServerFilter>),
    /// CR 1.16.4a: "the rez cost of that ice" (Nasir Meidan) — read off the
    /// ice of the encounter in progress, which is 1.15.4's back-reference
    /// [`TargetSpec::EncounteredIce`] asked for a number instead of a card.
    ///
    /// The rez cost is an inherent property of the CARD, printed on it, and
    /// not a record of what the Corp paid: an ice rezzed for free through an
    /// ability still has one, and the sentence still names it. Outside an
    /// encounter there is no ice to read and the quantity is 0 (9.12.2e).
    RezCostOfEncounteredIce,
    /// CR 1.16.4a + 1.15.4: "…gain credits equal to **its** rez cost" (Blue
    /// Sun) — [`TargetSpec::EarlierTarget`] asked for a number instead of a
    /// card, exactly as [`Quantity::RezCostOfEncounteredIce`] is
    /// [`TargetSpec::EncounteredIce`] asked for one. `nth` is 0-based over
    /// the ability's announcements in order, and the announcement 1.15.2
    /// puts before imminence is what makes the number readable while the
    /// same sentence's other half has yet to move the card. Printed cost,
    /// as ever; no such target reads 0 (9.12.2e).
    RezCostOfEarlierTarget { nth: usize },
    /// CR 1.16.4 + 7.1.2: "that card's **rez or play cost**", said of the
    /// card being ACCESSED (Freedom Khumalo) —
    /// [`TargetSpec::AccessedCard`] asked for a number, exactly as
    /// [`Quantity::RezCostOfEncounteredIce`] is
    /// [`TargetSpec::EncounteredIce`] asked for one. Which cost the sentence
    /// means is settled by the card's type: 1.16.4a gives assets, ice and
    /// upgrades a rez cost and 1.16.4b gives events and operations a play
    /// cost, both the number printed in the top corner — an inherent
    /// property of the CARD, not a record of anything paid. A printed cost
    /// of 0 reads as 0: the cost exists and is zero (1.16.1d pays it by
    /// announcing it), which is not the same as having none.
    ///
    /// An agenda has neither a rez cost nor a play cost (2.5's card type
    /// carries no such number), so it reads 0 here — and every printed
    /// sentence naming this quantity excludes agendas anyway. Outside an
    /// access there is no card to read and the quantity is 0 (9.12.2e).
    RezOrPlayCostOfAccessedCard,
    /// CR 9.9.6: "the number of cards you would draw" (The Class Act) — the
    /// modifiable value the IMMINENT instruction currently expects for
    /// effects of this class, read by an ability resolving in the interrupt
    /// window that imminence opened. The class is content (§12 rule 2), so
    /// one selector says every "…you would <verb>" quantity a card prints,
    /// and it reads the same values [`crate::effects::EffectAtom`] carries
    /// for prevention and modification.
    ///
    /// 9.9.7a/b: the value is read as it now STANDS, so an earlier interrupt
    /// that already decreased or removed it is seen. Outside an imminence
    /// there is no such value at all and this is 0 — the same treatment
    /// 1.16.2d gives an X outside the payment of its cost.
    ImminentValueOf(EffectClass),
    /// CR 1.17.2: "the agenda point value of the forfeited agenda" (Jemison
    /// Astronautics) — the agenda points printed on the cards a description
    /// matches, summed. The description is the shared FILTER vocabulary, the
    /// same one [`Quantity::Count`] counts with (§12 rule 5): a quantity
    /// position is read, never announced, so it describes cards rather than
    /// naming targets. A description matching nothing is 0 (9.12.2e), and so
    /// is a card with no agenda points printed on it.
    ///
    /// The value is the card's PRINTED one, so it is still readable after the
    /// agenda has left the score area — which is the only reason a sentence
    /// about a forfeited agenda can be answered at all (8.2.5 has already
    /// removed it from the game by the time the ability resolves).
    AgendaPointsOf(Vec<TargetFilter>),
    /// CR 4.3: the number of cards in the named player's hand. "Draw until
    /// you have 5 cards in HQ" (NEXT Design) is `Const(5) − CardsInHandOf`,
    /// asked at resolution — the hand as it stands then, not as it stood when
    /// the ability began. A count, not a criterion:
    /// [`TargetFilter::CardsInHandOf`] describes the cards, this reads how
    /// many there are, and 4.3.4 makes that number open information.
    CardsInHandOf(Side),
    /// CR 5.5.1/5.7.3: the named player's maximum hand size, as modified —
    /// read through the same 9.12.1a pipeline the discard step reads
    /// (`Vm::max_hand_size`), so a Safety-First-class "-2" or a Cybernetics
    /// "-1" is already inside the number a sentence compares against.
    MaxHandSizeOf(Side),
}

impl Default for Quantity {
    /// A quantity position with nothing in it is 0 (`Cost::default`).
    fn default() -> Self {
        Quantity::Const(0)
    }
}

impl Quantity {
    /// Shorthand for a printed constant.
    pub fn c(n: i64) -> Quantity {
        Quantity::Const(n)
    }
    /// CR 1.16.2c: does this quantity CONTAIN the variable X? "Some costs
    /// contain the variable X. Before a player pays such a cost, they choose
    /// and announce a positive integer or 0 to be the value for X" — so the
    /// announcement is owed by the cost's SHAPE, not by whether the ability
    /// also states a restriction on the value.
    pub fn mentions_announced_x(&self) -> bool {
        match self {
            Quantity::AnnouncedX => true,
            Quantity::Plus(a, b) | Quantity::Minus(a, b) => {
                a.mentions_announced_x() || b.mentions_announced_x()
            }
            Quantity::Times(_, q) | Quantity::PerEvery(q, _) | Quantity::XOfSource(q) => {
                q.mentions_announced_x()
            }
            _ => false,
        }
    }
    /// "base plus per × (counters of `kind` on this card)" — the common
    /// calculated-quantity shape (9.12.2b, Urtica/Fermenter classes).
    pub fn base_plus_per_counter(base: i64, per: i64, kind: crate::object::CounterKind) -> Quantity {
        Quantity::Plus(
            Box::new(Quantity::Const(base)),
            Box::new(Quantity::Times(per, Box::new(Quantity::CountersOnSource(kind)))),
        )
    }
}

/// CR 1.12.6: the SPAN of game history a count reviews — "during that run"
/// against "this turn".
///
/// Content on the quantity that counts (§12 rule 2), never a quantity per
/// span: a sentence asking how many of something happened differs from its
/// neighbour in the span alone. Both spans are marks into the change log,
/// which 10.2.1 makes open information — `Vm::st.run_log_start` and
/// `Vm::st.turn_log_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryWindow {
    /// "…during that run" — from the run in progress beginning, or from the
    /// run that just ended, for a "when this run ends" ability. Outside a run
    /// the mark is stale and the count is whatever the last run left; a
    /// sentence saying "that run" is only ever asked inside or at the end of
    /// one.
    ThisRun,
    /// "…this turn" — from the current turn beginning (5.1).
    ThisTurn,
}

/// The shared SERVER-filter language: the stipulations a sentence makes about
/// 4.6.6a's servers, standing to [`Quantity::CountServers`] exactly as
/// [`TargetFilter`] stands to [`Quantity::Count`] (§12 rule 5 — one filter
/// vocabulary per kind of thing, with every stipulation as content on it).
/// Each variant is one predicate atom, and several combine as a conjunction
/// wherever a sentence stacks them: "each remote server that has a card in
/// its root **and** is protected by ice" is three atoms and one description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerFilter {
    /// CR 4.6.6c: "…each **central** server" / "…each **remote** server" —
    /// the server's TYPE. 4.6.6c says there are exactly two of them, so one
    /// position with a boolean names both (4.6.7a's three centrals against
    /// 4.6.8a's remotes) and neither is an atom of its own.
    IsCentral(bool),
    /// CR 4.6.6b: "…that **has a card in its root**", "…that is **protected
    /// by ice**", "…**you have a card installed in**" — at least one card the
    /// description reaches is in this server, at the location the sentence
    /// names.
    ///
    /// 4.6.6b puts the cards in the root and the cards protecting the server
    /// both *in* that server, so WHICH of the two a sentence means is content
    /// on this one atom ([`ServerLocation`]) and not an atom apiece. The
    /// cards are described in the shared card-filter vocabulary, read through
    /// the same candidate derivation a count or an announcement reads (§12
    /// rule 5); an empty description is the printed word "a card".
    HasCardIn { location: ServerLocation, criteria: Vec<TargetFilter> },
}

/// CR 4.6.6b: where in a server a card is — "the cards in the root of a
/// server and the cards protecting it are considered to be in that server",
/// which makes three things a sentence can name and not two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerLocation {
    /// CR 4.6.6e: the server's ROOT, which every server has — a central's
    /// holding any number of upgrades, a remote's holding 1 asset or agenda
    /// and any number of upgrades.
    Root,
    /// CR 4.6.6d / 4.6.9a: the positions PROTECTING the server, which only
    /// ice occupies.
    Protecting,
    /// CR 4.6.6b: either of them — the whole server, which is what a sentence
    /// saying "in" a server means.
    RootOrProtecting,
}

/// CR 6.7.4a: the set of servers an effect that initiates a run allowed the
/// Runner to choose from. A selector, not a literal list, because "a remote
/// server" names a set the game state computes (§12 rule 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunServerSet {
    /// The effect named no restriction ("make a run").
    Any,
    /// "Run a remote server."
    AnyRemote,
    /// "Run HQ." — exactly these servers.
    These(Vec<ServerId>),
}

impl RunServerSet {
    /// CR 6.7.4a: "…no longer is a server that could have been chosen for
    /// this run".
    pub fn allows(&self, s: ServerId) -> bool {
        match self {
            RunServerSet::Any => true,
            RunServerSet::AnyRemote => matches!(s, ServerId::Remote(_)),
            RunServerSet::These(v) => v.contains(&s),
        }
    }
}

/// The movement vocabulary, as a list — a **citation anchor** for the rules
/// each movement's own instruction or procedure implements.
///
/// CR 8.2.1: the MOVEMENTS — the ways a card changes zone or location under
/// special rules. Each is an instruction or a procedure below; 8.2.1a's
/// non-movements ("add", "move", "discard", "set aside", "remove from the
/// game") simply put the cards where they are told, which is why they are
/// ordinary moves in the kernel and meet no movement-related condition.
pub const fn movements() -> &'static [&'static str] {
    cite!("sec_movement");
    cite!("rule_non_movements");
    cite!("movement_arrange");
    cite!("movement_draw");
    cite!("movement_install");
    cite!("movement_play");
    cite!("movement_search");
    cite!("movement_score");
    cite!("movement_steal");
    cite!("movement_swap");
    cite!("movement_trash");
    &["arrange", "draw", "forfeit", "install", "play", "search", "score", "steal", "swap", "trash"]
}

/// A single instruction: the atomic unit of ability resolution (9.3.4c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // ---- card-text vocabulary -------------------------------------------
    /// "Gain N credits." — N is a quantity position (9.12.2: "…for each" is
    /// the same instruction with a computed selector).
    GainCredits(Side, Quantity),
    /// "Lose N credits." (1.10.3b: loses as much as possible if short.) N is
    /// a quantity position (§12 rule 6), so "the Runner loses all credits in
    /// their credit pool" (Closed Accounts) is this instruction with a
    /// selector rather than a variant of its own.
    LoseCredits(Side, Quantity),
    /// CR 1.11.3a: "A player gains clicks whenever the number of clicks they
    /// have is increased." The count is a quantity position (§12 rule 6).
    GainClicks(Side, Quantity),
    /// CR 1.11.3b: "If a player loses or spends clicks, the number of clicks
    /// they have is reduced by that amount." This is the LOSE half — Enigma's
    /// "the Runner loses [click]" — and 1.11.3b is explicit that losing and
    /// spending are not synonymous, so a card that counts spent clicks does
    /// not see this. A player with fewer clicks than the amount simply reaches
    /// zero: the number they have cannot go below it.
    LoseClicks(Side, Quantity),
    /// "Draw N cards." CR 8.4.1: drawing moves cards from a deck to a hand,
    /// and 8.4.2 makes that a PROCEDURE — the cards are set aside facedown
    /// first, are considered drawn there, and only reach the hand once every
    /// ability that acts on the draw has resolved. So this instruction
    /// expands into the 8.4.5 step sequence, exactly as an install expands
    /// into 8.5.16.
    ///
    /// The count is a quantity position (§12 rule 6): "draw 1 card" is a
    /// constant, and "draw until you have 5 cards in HQ" (NEXT Design) is
    /// `Const(5) − CardsInHandOf` — a draw-up-to is a draw whose number is
    /// calculated from the hand as it stands, evaluated when the instruction
    /// resolves and floored at zero like every 9.12.2e count of nothing.
    Draw(Side, Quantity),
    /// CR 8.4.5a: "Set aside N cards from the top of the drawing player's
    /// deck. The cards are now considered drawn and can be looked at by their
    /// controller." They are one facedown 4.8.7 group, so 4.8.2a's exception
    /// (abilities referring to drawn cards CAN see them there) has something
    /// to name.
    DrawStepSetAside { side: Side, n: u32, group: u64 },
    /// CR 8.4.5c: "Add the set-aside cards to the player's hand." Whatever is
    /// still in the group goes — 8.4.3a's card that left is not added, and
    /// 8.4.3b's card swapped in is.
    DrawStepAddToHand { side: Side, group: u64 },
    /// "Do N <kind> damage." / "Suffer N <kind> damage."
    /// `responsible` per 10.4.1 (Corp "does", Runner "suffers"). The amount
    /// is a quantity position: "2 net plus 1 per advancement counter" is one
    /// instruction whose selector aggregates into a single instance
    /// (9.12.2b/c, Urtica class).
    Damage { kind: DamageKind, amount: Quantity, responsible: Side },
    /// "Take N tags." / "Give the Runner N tags." (the Runner takes them
    /// either way — 10.5.1 puts the counter on the Runner and no one else.)
    ///
    /// `avoidable` is the printed "**(cannot be avoided)**" (NBN: Controlling
    /// the Message) as content on this one instruction (§12 rule 2): CR
    /// 9.3.3g makes a stipulation that part of an effect cannot be prevented
    /// a RESTRICTION, and 9.4.5 makes the restriction ride the value — which
    /// is exactly [`crate::effects::EffectAtom::unpreventable`], the flag
    /// [`Instruction::DamageUnpreventable`] already sets for the damage half
    /// of the same sentence pattern. 9.9.5 makes "prevent" and "avoid"
    /// synonyms, so one flag says both words.
    GainTags { amount: u32, avoidable: bool },
    /// "Trash <targets>." — one effect acting on the whole set (9.12.2a).
    TrashCards(TargetSpec),
    /// CR 9.10.3: "choose a server", "choose an installed piece of ice",
    /// "choose an ice subtype" — the choice is REMEMBERED by a lingering
    /// effect (`Payload::MaintainedChoice`) that later abilities of the same
    /// source read by `key`. The duration is 9.10.3's, which the card layer
    /// states as one of its three cases: (a) the same duration as the effect
    /// that reads it, (b) `ThisTurn` for a bare "when your turn begins"
    /// choice, (c) `WhileSourceActive` otherwise.
    MaintainChoice { key: &'static str, of: ChoiceSpec, duration: crate::lingering::WantedDuration },
    /// CR 9.12.3a/b: "the Runner MUST trash this card, if able." The
    /// instruction states a requirement, not an effect: it forbids the Runner
    /// from passing the mid-access window (9.2.10) while a permitted means of
    /// trashing the accessed card is available to them. `means` is 9.12.3's
    /// distinction — with no means stipulated the Runner must make any
    /// decision that satisfies the requirement, including using another
    /// card's ability (9.12.3a); with a means stipulated only that means
    /// counts and nothing else can be forced (9.12.3b).
    MustTrashAccessedCard { means: TrashMeans },
    /// "End the run."
    EndTheRun,
    /// CR 6.1.5: "…you may **jack out**." Jacking out is the PROCESS by which
    /// the Runner voluntarily ends a run, and 6.1.5's own words are that it
    /// "follows the usual process for ending the run" — so it is the same
    /// ending, and the atom it makes imminent is the same one, which is what
    /// keeps an ability that acts on a run ending relevant to it (9.9.3a).
    ///
    /// Distinct from [`Instruction::JackOutChoice`], which is 6.1.5b's STEP:
    /// the step is an opportunity the run's structure opens at one place, and
    /// this is the ability's effect, offered wherever the card says (Nero
    /// Severn offers it during an encounter, where the structure opens none).
    /// The step's yes-branch and this instruction end the run by the same
    /// path.
    ///
    /// 6.1.5's remaining sentence — that some abilities function differently
    /// depending on whether the Runner chose to end the run this way — has no
    /// reader in the kernel yet, and the step does not record the choice
    /// either; when one arrives it is the same record for both.
    JackOut,
    /// An optional part its controller may decline (9.6.9c): "you may …".
    DeclineableChoice(Box<Instruction>),
    /// CR 9.11.4f / 1.16.11a: "you may pay [cost] to [effect]" — the pay/
    /// decline choice ends an instruction; the paid-for branch becomes the
    /// next instruction.
    NestedCostThen {
        cost: crate::ability::Cost,
        effect: Box<Instruction>,
        /// Who pays (None = the ability's controller). "…unless the Runner
        /// pays" names the payer explicitly.
        payer: Option<crate::object::Side>,
    },
    /// CR 1.16.11b: "[effect] unless [cost]" — paying suppresses the effect;
    /// declining (or being unable to pay) makes it the next instruction.
    NestedCostUnless {
        cost: crate::ability::Cost,
        effect: Box<Instruction>,
        payer: Option<crate::object::Side>,
    },
    /// "…and access it" (§7.2): the announced cards are accessed, one at a
    /// time, each in its own access timing structure. The cards are a target
    /// POSITION, so "access the card you chose from the top of R&D" (Top Hat
    /// class) and "access this card" are the same instruction.
    AccessCards {
        cards: TargetSpec,
        /// "…you cannot steal or trash it during this access." (Pinhole
        /// Threading class; a printed restriction on the access, 1.2.2.)
        restricted: bool,
    },
    /// "…access 2 additional cards." (Cupellation class; raises 7.3.5's
    /// random-access limit for the breach in progress.)
    AdditionalAccesses(Quantity),
    /// CR 9.6.14d: "Resolve the <class> ability of <a card>." — an effect
    /// that attempts to resolve an ability of a card by naming its class
    /// rather than by the stipulation occurring. For the three conditional
    /// classes of 9.6.14 the ability is marked PENDING as though the
    /// stipulation had occurred, so it resolves through the ordinary
    /// reaction window; any additional requirements of its trigger condition
    /// must still be met by the game state, and an unmet requirement means
    /// the ability cannot even become pending. For
    /// [`crate::ability::AbilityClass::Subroutine`] the named subroutine
    /// resolves directly (9.8.10), since a subroutine is not a conditional
    /// ability and never pends.
    ///
    /// The card is a target POSITION and the class is the content (§12 rule
    /// 2), so a 24/7-News-Cycle-class "resolve the 'when scored' ability of
    /// an agenda in your score area" and a Nanisivik-Grid-class "resolve its
    /// first subroutine" are one instruction.
    ResolveAbilityOf { source: TargetSpec, which: crate::ability::AbilityClass },
    /// CR 8.1.2: "Rez <a card>." — an unrezzed card is turned faceup, which
    /// makes it active (9.1.7). 8.1.2b: card abilities can direct or allow
    /// the Corp to rez cards outside the paid ability windows of 8.1.2a;
    /// 8.1.2d: the rez cost is paid first unless the ability states that it
    /// is ignored (1.16.5c), which is what `ignore_costs` says.
    ///
    /// `reduce` is 1.16.6's cost reduction — "rez 1 **bioroid** card, paying
    /// 4[credit] less" (Haas-Bioroid: Architects of Tomorrow). It is the same
    /// content [`Instruction::InstallCard`] carries on the install cost, said
    /// about the other inherent cost: 1.16.6b floors the payment at zero, so
    /// a reduction larger than the cost is not a gain.
    RezCard { target: TargetSpec, ignore_costs: bool, reduce: Quantity },
    /// CR 5.6.2b: "Your action phase ends." (Oppo Research class.) The action
    /// phase loop takes an action only while the player has unspent [click],
    /// so ending the phase early is losing the ones they have left.
    EndActionPhase(Side),
    /// CR 1.21.2: "Look at <cards>." — the looking player sees their front
    /// faces without showing them to the other player. CR 9.11.4e: where an
    /// older card puts the look in the SAME sentence as what is then done to
    /// those cards, making the cards visible is the end of an instruction, so
    /// the look is its own instruction and a checkpoint occurs before the
    /// next one announces its targets among them.
    LookAtCards { cards: TargetSpec, by: Side },
    /// CR 9.11.4c: "**Choose 2 cards in your heap.**" — a sentence that only
    /// directs a player to choose targets and "does not act on that choice or
    /// describe any other effects". Usually such a sentence merges into the
    /// next one as its 1.15.2 announcement (Tinkering, SSO Industries); it
    /// stands on its own inside the single instruction 9.11.4c forms when the
    /// following sentence's halves act on DIFFERENT subsets of the choice —
    /// one removed by the Corp, "the other card" added to the grip (Steve
    /// Cambridge class) — so no one half can own the announcement. Resolving
    /// it does nothing at all: the announcement was the whole of it, and
    /// 1.15.4 is how the later halves read what it chose.
    ChooseCards { targets: TargetSpec },
    /// CR 1.21.4: "Expose <cards>." — the named cards are revealed, except
    /// that only INSTALLED, UNREZZED cards can be exposed. 1.21.5 keeps
    /// exposing distinct from looking, revealing and accessing, and 9.12.2
    /// does not list it among the aggregated effect classes, so exposing two
    /// cards in one instruction is two occurrences (9.6.4b).
    ExposeCards { cards: TargetSpec },
    /// CR 1.21.3: "Reveal <cards>." — show the cards' front faces to all
    /// players, then return them to their previous state. 1.21.3a: this is NOT
    /// turning them faceup, so a facedown card stays facedown; what changes is
    /// what each player has SEEN (10.2.2b). Unlike exposing (1.21.4) it is not
    /// restricted to installed unrezzed cards — a card in hand, in a deck or
    /// set aside can be revealed.
    RevealCards { cards: TargetSpec },
    /// CR 1.21.3 + 1.15.2: "reveal 1 card from the grip **at random**"
    /// (Hyoubu Institute). A random pick is not an announcement — 1.15.2b
    /// puts the choice to a player, and this sentence takes it away from
    /// both — so the cards cannot be a [`TargetSpec`] and the instruction
    /// says the hand and the number instead. The same shape
    /// [`Instruction::TrashRandomFromHand`] takes for 10.4.3's damage.
    RevealRandomFromHand { side: Side, count: Quantity },
    /// CR 1.10.3a: "Take N[credit] from this card." — hosted credits move from
    /// a card to a credit pool, which is a GAIN (they enter the pool from
    /// another location). With fewer hosted than asked for, the card gives
    /// what it has.
    TakeHostedCredits { from: TargetSpec, amount: Quantity, to: Side },
    /// CR 1.9.2: "Remove N hosted <kind> counters." — counters leave the card
    /// and return to the bank. This is the mandatory-effect counterpart of
    /// `Cost::spend_counters`, which is the paid-ability half: costs are
    /// SPENT (1.16.1), these are simply removed.
    RemoveCounters {
        target: TargetSpec,
        kind: crate::object::CounterKind,
        amount: Quantity,
        /// "remove up to N" — take what is there when there are fewer.
        up_to: bool,
    },
    /// "Derez <targets>." (§8.1.2) — a rezzed card is turned facedown.
    /// CR 1.12.5: turning a card faceup or facedown does not make it a new
    /// object, since it does not change zones.
    Derez { target: TargetSpec },
    /// "Move the (set-aside) hosted counters to <target>" (Reconstruction
    /// Contract class, 9.5.5).
    MoveSetAsideCounters { kind: crate::object::CounterKind, target: TargetSpec },
    /// Combined-sentence instruction: several effects in ONE instruction
    /// (e.g. Snare!'s "Do 3 net damage and give the Runner 1 tag.").
    Combined(Vec<Instruction>),
    /// Interrupt-effect: prevent N damage of a kind (9.9.5).
    PreventDamage { kind: DamageKind, amount: u32 },
    /// Interrupt-effect: prevent ALL damage of a kind (9.9.7b).
    PreventAllDamage { kind: DamageKind },
    /// Interrupt-effect: avoid N tags (9.9.5).
    AvoidTags(u32),
    /// CR 10.5.5: "Remove N tags." — the tags return to the bank (1.9.2). A
    /// quantity position (§12 rule 6): "remove X tags" is this instruction
    /// with the announced X (Misdirection class).
    RemoveTags(Quantity),
    /// Interrupt-effect: increase imminent damage by N (The Cleaners class).
    IncreaseImminentDamage { kind: DamageKind, amount: u32 },
    /// Interrupt-effect: prevent a specific object from being trashed
    /// (Sacrificial Construct class).
    PreventTrashOf(ObjectId),
    /// "Do N <kind> damage. This damage cannot be prevented." (Flare class;
    /// 9.3.3g/9.4.5: the restriction rides the value.)
    DamageUnpreventable { kind: DamageKind, amount: Quantity, responsible: Side },
    /// Interrupt-effect: replace the imminent damage's type (Tori Hanzō
    /// class; 9.9.10: applies immediately when the interrupt resolves).
    ReplaceImminentDamageKind { to: DamageKind },
    /// "Run any server." / "make another run" (Doppelgänger class) — pushes
    /// a nested run timing structure.
    InitiateRun {
        /// CR 6.9.1a: the attacked server the Runner ANNOUNCES. `None` is an
        /// effect that named no server ("Run any server", "Run a remote
        /// server"): the Runner announces one from `allowed` when the run is
        /// initiated, and the announcement rewrites this position exactly as
        /// an install's declared destination rewrites 8.5.16b's.
        server: Option<ServerId>,
        /// CR 6.7.4a: the servers this initiating effect ALLOWED. An "If
        /// successful" ability is tied to them, so a run moved (6.1.2d) to a
        /// server the effect could not have chosen no longer meets the
        /// condition — while a move WITHIN the set keeps it.
        allowed: RunServerSet,
        /// CR 6.7.4: "If successful, …" — a conditional ability whose trigger
        /// condition is "after the run created this way becomes successful".
        /// It cannot be an ordinary delayed conditional, because 9.6.13d
        /// wants the run already in progress; the instruction that creates
        /// the run is what carries it.
        if_successful: Vec<Instruction>,
        /// CR 9.9.1: "If that run **would** be declared successful, …" — the
        /// same clause one instruction earlier. It is an INTERRUPT (the word
        /// "would" makes it one), relevant to the imminence of 6.9.5a's
        /// declaration, and it rides here for exactly the reason
        /// `if_successful` does: it belongs to the run this instruction
        /// creates, and there is no printed conditional ability on any object
        /// for 9.9.4b's scan to find.
        ///
        /// 6.7.4a's server tie is deliberately NOT applied to it. That rule
        /// is stated about "If successful" abilities; this sentence names the
        /// RUN ("that run"), and the run is what carries the clause.
        if_would_be_successful: Vec<Instruction>,
        /// CR 6.9.1 / 5.2.2b: what the sentence states about the run it
        /// initiates, UNCONDITIONALLY — "The Corp cannot rez ice **during
        /// that run**" (Blackmail), "**During that run**, <X>". Resolved as
        /// the run begins, before its first step.
        ///
        /// The third position beside `if_successful` and
        /// `if_would_be_successful`, and it rides here for the same reason
        /// both of those do: the sentence says "that run", and the run this
        /// instruction creates is what identifies it.
        ///
        /// It is a position rather than an instruction written after the run
        /// because 5.2.2b makes the two different things. "If a timing
        /// structure is initiated during the resolution of an action, that
        /// action is not complete until the new timing structure is complete
        /// **and any further effects of the initiated action following the
        /// completion of the new timing structure are resolved**" — so an
        /// instruction printed after the run does not resolve until the run is
        /// over, and a lingering effect it created for "that run" would meet
        /// 9.10.4's "duration based on a timing structure that is not in
        /// progress at the time the lingering effect is created" and expire
        /// immediately. The card would do nothing at all.
        ///
        /// Neither workaround is offered. Reordering the sentence in front of
        /// the run invents an instruction boundary the card does not print
        /// (9.11.3) and creates the effect while there is no run to bind it
        /// to; folding it into `if_successful` gates it on a success the card
        /// never mentions.
        during: Vec<Instruction>,
    },
    /// "Trace [N] — if successful, …; if unsuccessful, …" (10.8). Expanded
    /// by the resolution loop into the 10.8.6 step sequence. The base is a
    /// quantity position: Trace[3] is a constant selector; "Trace[X], X = 2
    /// per ice protecting this server" (Surveyor class) is
    /// `XOfSource(Times(2, Count(IceProtectingSourceServer)))` — evaluated
    /// when the trace initiates (9.12.2e), 0 if the defining ability is
    /// inactive or lost.
    Trace {
        base: Quantity,
        if_successful: Vec<Instruction>,
        if_unsuccessful: Vec<Instruction>,
        /// "When the trace is determined…, if your trace strength is N or
        /// greater, …" (Gemini class, 10.8.5).
        determined_min: Option<(i64, Vec<Instruction>)>,
    },
    /// 10.8.6a: the trace initiates ("when initiated" conditions meet); the
    /// base trace strength is a modifiable value (9.9.6d).
    TraceInitiate { base: i64 },
    /// 10.8.6c: the Corp may spend credits to increase the trace strength.
    TraceCorpSpend,
    /// 10.8.6d: the Runner may spend credits to increase their link strength.
    TraceRunnerSpend,
    /// 10.8.6e: determine success; the associated conditionals pend (10.8.5).
    TraceDetermine {
        if_successful: Vec<Instruction>,
        if_unsuccessful: Vec<Instruction>,
        determined_min: Option<(i64, Vec<Instruction>)>,
    },
    /// "Play a Psi Game." — one instruction: sealed bids, reveal, immediate
    /// spend, branch (10.14.6).
    PsiGame { on_match: Vec<Instruction>, on_differ: Vec<Instruction> },
    /// "<Ice> gains N copies of <sub> …" (Brainstorm class grants to
    /// itself; a Marker-class ability grants to another piece of ice —
    /// category 9.8.3e, external, ordered oldest-first by grant time). The
    /// INSTRUCTION is the position; `to` and `duration` are the content, so
    /// self-grants and external grants are one variant (§12 rule 2).
    GrantSubroutines {
        to: TargetSpec,
        /// WHICH subroutines are granted (§12 rule 2: the instruction is the
        /// position, this is the content).
        grant: SubroutineGrant,
        before: bool,
        /// CR 9.8.2c: "in the order of your choice" — the granting player
        /// declares where the granted subroutines sit relative to every
        /// subroutine the ice has at that time, regardless of categories.
        any_order: bool,
        duration: crate::lingering::WantedDuration,
    },
    /// "The Corp discards N cards from HQ." (Utopia Shard class driver.)
    CorpDiscards { count: u32 },
    /// "The Runner cannot access cards other than this one for the
    /// remainder of the run." (Ash class, 7.4.2.)
    RestrictAccessToSelf,
    /// Create a delayed conditional ability (9.6.13) with the given
    /// duration request; "when this run ends" with no run → never created
    /// (9.6.13d).
    CreateDelayedConditional {
        def: Box<crate::ability::AbilityDef>,
        duration: crate::lingering::WantedDuration,
    },
    /// Create a lingering effect (§9.10) with a stated duration: "prevent
    /// all damage for the remainder of this run", "the next time you would
    /// breach, instead …", "access N additional cards". The INSTRUCTION is
    /// the position; [`LingeringSpec`] is the content, so the whole class is
    /// expressible without a bespoke instruction per card (§12 rule 2). The
    /// requested duration is bound to the structure instance in progress at
    /// resolution (9.10.4).
    CreateLingeringEffect {
        payload: LingeringSpec,
        duration: crate::lingering::WantedDuration,
    },
    /// "Choose <targets>. <They> gain/lose <subtypes> [for a duration]."
    /// (Tinkering class.) CR 9.11.4c: the choosing sentence and the
    /// modifying sentence form ONE instruction — the target is announced as
    /// the instruction becomes imminent and the subtypes change when it
    /// resolves. Both the target and the duration are positions; 2.16.5
    /// counts instances, so an add and a printed subtype coexist.
    ModifySubtypes {
        target: TargetSpec,
        add: Vec<crate::subtype::Subtype>,
        remove: Vec<crate::subtype::Subtype>,
        duration: crate::lingering::WantedDuration,
    },
    /// "The Runner loses N memory units until end of turn." (Bad Times.)
    ReduceRunnerMemoryThisTurn(u32),
    /// CR 9.11.4g / 9.12.3c-d: choose one of several optioned effects; the
    /// choice ends an instruction and must select a fully-resolvable option
    /// if any exists; the chosen effect is then separately interruptible.
    ChooseOne { options: Vec<(&'static str, Vec<Instruction>)> },
    /// "…take another different action, paying [click] less." (MirrorMorph:
    /// Endless Iteration.) The resolving player takes ONE action (5.2.2's
    /// full procedure: initiate, spend the discounted [click] cost, resolve)
    /// from inside this ability's resolution — the only place other than the
    /// action phase's own step (5.2.4) an action is ever offered. The
    /// options are the same set the 9.2.6 action window would offer, priced
    /// with `click_discount` fewer [click] (1.16.2a floors each click cost
    /// at 0; every other cost — install credits, an operation's play cost —
    /// is still required and still paid).
    ///
    /// `different_from_this_turn` is 5.2.5's identity test as a filter:
    /// "another DIFFERENT action" removes every option whose 5.2.5a/b
    /// identity was already taken this turn. The action taken here is an
    /// action like any other — `ActionTaken` and `ActionCompleted` are
    /// recorded for it, so a condition counting the turn's actions sees it.
    ///
    /// 5.2.2a ("once an action is initiated, it must be completed before
    /// the game can advance to the next step or open another action
    /// window") holds because the offer itself sits between two actions:
    /// the condition that resolves this ability is met by an action
    /// COMPLETING, so the whole ability — this instruction and the action
    /// it hands out — runs inside that completion's checkpoint, before the
    /// turn structure advances to its next step.
    OfferAction { different_from_this_turn: bool, click_discount: u32 },
    /// "Break N subroutines on the encountered ice." — the subroutines it
    /// acts on are a target POSITION (1.15.1: subroutines are targets like
    /// objects are), so "break up to 2 subroutines" (Cleaver class),
    /// "break all subroutines" (9.8.6a, no targets) and "break all but 1
    /// subroutine" (9.8.6b, Grappling Hook) are one instruction.
    BreakSubroutines { subs: SubroutineSpec },
    /// "Bypass the ice you are encountering." — ends the encounter (6.5.8).
    BypassEncounteredIce,
    /// CR 6.5.9a: "The Runner encounters <a piece of ice>." — a FORCED
    /// encounter: an Encounter Ice Phase resolved outside the run's normal
    /// progression, without changing the Runner's position, after which
    /// resolution returns to this instruction's ability and proceeds from
    /// there (6.5.9c: this instruction is not finished until the encounter
    /// is complete). The ice is a target POSITION, so a Chrysalis-class
    /// "they encounter it" (itself, uninstalled — its subroutines are active
    /// for exactly that encounter, 9.1.8h) and a Ganked!-class "force the
    /// Runner to encounter a rezzed piece of ice you control" are the same
    /// instruction.
    ForceEncounter { ice: TargetSpec },
    /// "+N strength" / "-N strength" on a card, for a duration. The TARGET
    /// is a position (an icebreaker pumping itself is `SelfSource`; a Devil-
    /// Charm-class ability lowering a piece of ice's strength names the ice)
    /// and so is the DURATION: `None` is "no duration stated", which on an
    /// icebreaker modifying its own strength means "for the remainder of the
    /// current encounter" (3.9.5b / 9.10.4a) and outside an encounter means
    /// "until the next checkpoint" (3.9.5d). A stated duration runs
    /// ALONGSIDE that implicit one, not instead of it (3.9.5c / 3.4.4a).
    /// The AMOUNT is a quantity position too (§12 rule 6): "+X strength"
    /// (Paperclip, Corporate Troubleshooter) and "+X strength, X = the number
    /// of installed icebreakers" (Unity) are this instruction with a
    /// selector. A negative quantity lowers the strength.
    ModifyStrength {
        target: TargetSpec,
        amount: Quantity,
        duration: Option<crate::lingering::WantedDuration>,
    },
    /// "Place N <kind> counters on <target>." The count is a quantity
    /// position (§12 rule 6): the dividends keyword's "N agenda counters for
    /// each hosted advancement counter past its advancement requirement"
    /// (10.13.1) is this instruction with a selector.
    ///
    /// CR 1.18.2: placing an advancement counter directly is NOT advancing —
    /// that is [`Instruction::AdvanceCard`].
    PlaceCounters { target: TargetSpec, kind: crate::object::CounterKind, amount: Quantity },
    /// CR 6.1.2d: "The attacked server becomes <server>." (Sneakdoor Beta
    /// class.) A few abilities change the attacked server DIRECTLY, without
    /// referring to the Runner's position — so the run's current timing step
    /// does not change, and the Runner does not approach or encounter the ice
    /// protecting the new server. (Contrast `MoveRunnerToIce`, 6.2.8a, which
    /// changes the attacked server BY moving the Runner and does change the
    /// timing step.)
    ChangeAttackedServer { server: ServerId },
    /// CR 1.15.1 / 1.18.2: "Move up to N <kind> counters from 1 other card to
    /// the chosen card" (Trick of Light class). The COUNTERS are targets and
    /// so is the destination card (1.12.1 makes a counter an object); the card
    /// the counters come from is NOT a target — it is implied by which
    /// counters were chosen, and "1 other card" is the requirement that they
    /// all share a host. 1.18.2: moving an advancement counter is not
    /// advancing, so no "whenever you advance" condition is met.
    MoveCounters {
        kind: crate::object::CounterKind,
        count: Quantity,
        up_to: bool,
        /// The destination, announced FIRST (the counters are then chosen
        /// from another card — 1.15.4's cross-instruction reference in one
        /// instruction).
        to: TargetSpec,
        /// Which cards the counters may come from.
        from_criteria: Vec<TargetFilter>,
    },
    /// CR 1.18.1: "Advance <a card>." — place an advancement counter from the
    /// bank on it, as an ADVANCE, so that "whenever you advance" conditions
    /// are met (1.18.2 distinguishes this from placing the counter directly).
    AdvanceCard { target: TargetSpec },
    /// CR 10.1.2: "Purge virus counters." — remove ALL virus counters hosted
    /// on cards and return them to the bank. One instruction, one occurrence:
    /// the rule names the whole board at once, so a condition looking for the
    /// purge is met once however many counters came off (and is met even if
    /// none did — the Corp purged).
    PurgeVirusCounters,
    /// "…flip this identity." (rule_identity_double_sided; Nebula class.)
    FlipIdentity(Side),
    /// "…**secretly set** your identity to any copy of <this identity>."
    /// (Méliès U class.) The named side's identity has several printed back
    /// faces — the copies it ships as — and its controller seals a choice
    /// among them: WHICH is hidden information (10.2.2a, the psi-bid grain),
    /// held by the VM outside the open change log; THAT the set happened is
    /// open, recorded as `GameChange::IdentityFaceSecretlySet`. The reveal
    /// is the flip: `FlipIdentity` turns the sealed face up.
    SecretlySetFlipFace(Side),
    /// CR 1.5.4 / 1.5.4b: "switch your identity with another identity …"
    /// (Rebirth class). The identity described by `with` takes the play area
    /// (3.1.1) and the one it replaces goes back to the pile outside the game,
    /// because 1.5.4b says an identity leaving the play area "must be returned
    /// to the pile".
    ///
    /// Not a 8.8 swap: 3.1.1b says identities are not installed, so none of
    /// 8.8.2's destination legality or 8.8.4's uninstalling applies. `with` is
    /// an ordinary target position, so the stipulations a card makes about the
    /// new identity — its faction, a subtype, where it is — are the shared
    /// criteria vocabulary and nothing here has to know about any of them.
    SwitchIdentity { side: Side, with: TargetSpec },
    /// "Shuffle up to 3 cards from Archives into R&D." (Jackson class;
    /// 8.1.4/1.12.3 — entering the deck makes new objects, and the shuffle
    /// follows.) The targets are announced (1.15.2).
    ShuffleCardsIntoDeck { targets: TargetSpec, to: Side },
    /// "Then, they shuffle their stack." (Chronos Protocol: Haas-Bioroid.)
    ///
    /// A shuffle with nothing moving into the deck — the other half of what
    /// [`Instruction::ShuffleCardsIntoDeck`] does, said on its own because
    /// this sentence says only that half. 4.2.3 is why it is an instruction
    /// at all rather than bookkeeping: a deck's order "must be maintained
    /// except when a player is explicitly directed to manipulate the cards in
    /// a deck", and this is that direction. 8.7.3's post-search shuffle is a
    /// different one — that one the search procedure performs, with no
    /// sentence asking for it.
    ///
    /// `side` is whose deck the sentence names ("their stack"). Who does the
    /// physical shuffling is not a game fact, so nothing here records it.
    ShuffleDeck { side: Side },
    /// "Remove 1 card in the heap from the game." (Bloo Moose class; §4.9.)
    /// Distinct from RemoveSelfFromGame: the card is a TARGET.
    RemoveCardsFromGame { targets: TargetSpec },
    /// "Trash this card." (self-referencing; strandable per 9.1.4)
    TrashSelf,
    /// Steal the accessed agenda (7.1.4 via access step 7.2.3).
    StealSelfAgenda,
    /// CR 1.17.3 / 1.16.10c: "score this agenda" — the (S) option's effect,
    /// made an instruction so that an additional cost to score is paid in an
    /// ability frame's 9.5.7b PayCost phase and the checkpoint that follows
    /// resolves BEFORE the agenda moves to the score area.
    ScoreSelfAgenda,
    /// §8.5: install one card. The resolution loop expands this into the
    /// 8.5.16 step sequence (installing is a procedure, NOT a timing
    /// structure — 9.2.2e; its only explicitly-called-for checkpoint is the
    /// cost-paid one at 8.5.16d).
    InstallCard {
        card: TargetSpec,
        dest: InstallDest,
        /// 8.5.15: rez directly after the installation is complete.
        and_rez: bool,
        /// 1.16.5c: "ignoring all costs".
        ignore_costs: bool,
        /// "…**ignoring credit costs**" (Ob Superheavy Logistics) — a
        /// different axis from `ignore_costs`: 1.16.5c's "ignoring all costs"
        /// the kernel reads as the INHERENT costs only (1.16.4 — an
        /// additional rez cost is still paid, the Ob/Archer case written into
        /// `InstallRezPayCost`), while this selects costs by their KIND and
        /// cuts across 1.16.4's inherent/additional split: every CREDIT
        /// component — the inherent install cost (8.5.11), the rez cost
        /// (8.1.2d) of an install-and-rez, the credit part of an additional
        /// cost — becomes 0, and the non-credit components of an additional
        /// cost (an Archer-class forfeit) are STILL paid.
        ignore_credit_costs: bool,
        /// 8.5.13c: a requirement imposed by the installing ability that
        /// must be verified by revealing a hidden card.
        reveal_check: Option<RevealCheck>,
        /// CR 1.16.2f: "…paying a total of N[credit] less" on an install-AND-
        /// rez effect. "Total" means the Corp divides the modifier between
        /// the install cost and the rez cost, declaring the split at the
        /// beginning of step 8.5.16d. A quantity position (§12 rule 6); 0 is
        /// "no total modifier", and the modifier is inert without `and_rez`,
        /// since there is no second cost to divide it with.
        reduce_total: Quantity,
        /// CR 1.16.6: "…paying N[credit] less" — a reduction of the INSTALL
        /// cost alone (Career Fair class). Distinct from `reduce_total`:
        /// nothing is divided, so it needs no second cost and applies to a
        /// plain install. A quantity position (§12 rule 6); 0 is no
        /// reduction. 1.16.2a floors the result at 0.
        reduce_install: Quantity,
        /// CR 4.6.4d / 8.1.4: "install 1 card from your grip **facedown**"
        /// (Apex). 8.5.16a places the card "with the same faceup or facedown
        /// status it will have when the installation is complete", and every
        /// install so far has taken that status from the card's side alone —
        /// 8.5.2's Corp cards facedown, 4.6.4c's Runner cards faceup. This is
        /// the stipulation an ability makes about it instead, so it is
        /// content on the install (§12 rule 2) rather than an instruction of
        /// its own: 8.1.4d's turning-facedown is a different thing done to a
        /// card that is ALREADY installed.
        ///
        /// It follows 8.1.4a that the installed card has no characteristics
        /// at all — no name, no card type, no cost — which is why 8.5.11a
        /// gives "facedown Runner cards" no install cost in the same breath
        /// as agendas and upgrades.
        facedown: bool,
        /// [`Instruction::InstallCards`]' `distinct_servers`, carried onto
        /// the one-card install the expansion cut: the 8.5.16b destination
        /// declaration excludes every server this ability has already
        /// installed a card into or protecting.
        distinct_servers: bool,
    },
    /// 8.5.5: an effect installing more than one card — the cards are chosen
    /// and installed ONE AT A TIME, each as a separate instruction
    /// (9.11.4b). `and_rez_if_able` is the Ad Blitz "if able" stipulation
    /// (8.5.13d): unrezzable cards cannot be chosen.
    ///
    /// `distinct_servers` is "…with no more than a single piece of ice **per
    /// server**" (NEXT Design) — a stipulation about the multi-install as a
    /// whole (§12 rule 2), so it rides here and on each
    /// [`Instruction::InstallCard`] the expansion cuts: a server this
    /// ability has already installed a card into (or protecting) is not
    /// offered as a destination for the next one.
    InstallCards {
        count: u32,
        from_hand_of: Side,
        filter: InstallFilter,
        dest: InstallDest,
        and_rez: bool,
        and_rez_if_able: bool,
        ignore_costs: bool,
        distinct_servers: bool,
    },
    /// "Rez any number of them, lowering the total rez cost among all cards
    /// by 20." (Cyber Bureau; 8.1.2b + 1.16.2f.) "Them" is 1.15.4's
    /// back-reference to the cards THIS ability installed (the same record
    /// Kabonesa Wu's "if that program is still installed" reads), so the
    /// instruction carries no target position: the pick is 8.5.5's
    /// one-at-a-time choice said about rezzes, declinable at every step —
    /// which is the whole of "any number".
    ///
    /// `reduce_total` is 1.16.2f's divide-the-modifier said about MANY
    /// second costs: one pool of credits lowers the rez costs of all the
    /// chosen cards, and the Corp declares, before each rez pays (the
    /// 1.16.2f timing, "before calculating the value"), how many of the
    /// REMAINING credits come off this one — nonnegative, at most what is
    /// left, exactly as that rule's numbers must sum to the modifier. Each
    /// share lowers its own rez cost and 1.16.2a floors it at 0, so a share
    /// larger than the cost buys nothing; a pool left over when the Corp
    /// stops rezzing lowers nothing and lapses. Each rez is otherwise
    /// ordinary — 8.1.2d's payment, its cost-paid checkpoint, and the
    /// post-instruction checkpoint per rez, because the expansion cuts one
    /// [`Instruction::RezCard`] per chosen card (9.11.4b).
    RezInstalledCards { reduce_total: Quantity },
    /// 8.5.16a–c: place into the play area (not installed, not active),
    /// declare the destination, trash like cards.
    InstallStepPlace,
    /// 8.5.16d: pay the install cost. (The post-instruction checkpoint IS
    /// the 10.3.4 cost-paid checkpoint — 8.5.11c.)
    InstallStepPayCost,
    /// 8.5.16e–f: create the server if new, move the card, it becomes
    /// installed (faceup → active); "when installed" conditions are met and
    /// the install effect is complete.
    InstallStepComplete,
    /// 8.5.15 → 8.1.2d: pay the rez cost of the just-installed card. (The
    /// post-instruction checkpoint is the cost-paid checkpoint; per the
    /// 9.6.5b THG example this is the checkpoint that processes the
    /// CardInstalled change, while the card is still facedown.)
    InstallRezPayCost,
    /// Finish rezzing: the card turns faceup and becomes active.
    InstallRezFinish,
    /// §8.6: play one event/operation. Expanded by the resolution loop into
    /// the 8.6.7 step sequence.
    ///
    /// `then_remove_from_game` is CR 8.6.6d — "if an ability that plays an
    /// event or operation ALSO CONTAINS the nested conditional ability
    /// 'After it resolves, remove it from the game.', the event or operation
    /// is not trashed. The card remains in the play area until the
    /// conditional ability removes it from the game." The rule describes the
    /// pair as one construction, and its whole content is a change to what
    /// step 8.6.7g does to the played card, so it rides here as content
    /// rather than as an instruction of its own: written as a second
    /// instruction it could not work at all, because 9.1.4 stops an ability
    /// acting on a source that changed zones and the play moves the card
    /// into the play area.
    PlayCard { card: TargetSpec, ignore_costs: bool, then_remove_from_game: bool },
    /// 8.6.3: an effect playing more than one card — chosen and played one
    /// at a time, each as a separate instruction (9.11.4b).
    PlayCards { count: u32, from_hand_of: Side, ignore_costs: bool },
    /// 8.6.7a: place the card faceup in the play area; not installed, not
    /// yet active.
    PlayStepPlace,
    /// 8.6.7b: pay the play cost. (Post-instruction checkpoint = 10.3.4.)
    PlayStepPayCost,
    /// 8.6.7c–d: the card becomes active; conditions related to playing it
    /// are met. (The post-instruction checkpoint is 8.6.7e.)
    PlayStepActivate,
    /// 8.6.7f: resolve the play abilities of the card (a nested frame).
    PlayStepResolve,
    /// 8.6.7g–i: trash the card if applicable (8.6.6); after-resolve
    /// conditions are met; the play effect is complete.
    PlayStepFinish,
    /// "Remove this card from the game." (Ashen Epilogue class; 8.6.6a — a
    /// played card no longer in the play area is not trashed.)
    RemoveSelfFromGame,
    /// "If <state>, <effect>[. If you do not, <other effect>]." — a
    /// requirement that lives in the INSTRUCTIONS rather than in the trigger
    /// condition (9.6.5d), so it is checked when this instruction resolves
    /// and not when the condition was met.
    ///
    /// The predicate is the SAME `TriggerRequirement` vocabulary 9.6.5c uses
    /// on conditions — one state language for both places (§12 rule 2) —
    /// which is why "if you have at least 2 link" (Underworld Contact), "if
    /// the Runner is tagged" (IP Block) and "if you made a successful run
    /// this turn" (Mutual Favor) are one instruction rather than three.
    /// `otherwise` is the printed "if you do not" branch; empty where the
    /// sentence has none.
    IfMet {
        requires: Vec<crate::ability::TriggerRequirement>,
        then: Vec<Instruction>,
        otherwise: Vec<Instruction>,
    },
    /// CR 8.3.3 / 4.8.2: "set aside the top N cards of <a deck> facedown."
    /// The first half of the 8.3.3 arranging procedure, and the point at which
    /// 8.3.3b's "other effects on cards in a deck" become possible: while the
    /// cards are set aside, `TargetFilter::SetAsideByThisAbility` names them.
    ///
    /// `with_source` is 4.8.7's group stamped with the CARD whose ability set
    /// the cards aside — "set aside the top 6 cards of your stack facedown …
    /// Add 1 card **set aside with this identity** to your grip" (Ayla "Bios"
    /// Rahim). The plain 8.3.3 group dies with the arranging ability's frame;
    /// a group set aside WITH a card outlives every frame, because a later
    /// ability of the same card refers to it by the card and not by the
    /// ability ([`TargetFilter::SetAsideWithSource`]). The group's `by` is
    /// still its controller, so 8.3.3a's "may look at any time" entitlement
    /// falls out of the same visibility rule as every facedown group.
    SetAsideTopOfDeck { deck_of: Side, count: Quantity, with_source: bool },
    /// CR 8.3.3: "…secretly puts them in the order of their choice, and
    /// returns them to the top of that deck." The order is a declaration, not
    /// a target announcement (nothing is chosen to be acted ON), and 8.3.3
    /// makes every returned card a NEW object (1.12.3). 8.3.1a: arranging 1
    /// or fewer cards does nothing.
    ArrangeSetAside { to_top_of: Side },
    /// The Corp rearranges (or looks at and returns) cards in R&D
    /// (Bacterial Programming class). The returned cards are NEW OBJECTS
    /// (1.12.3), so 7.4.7a's "already chosen" bookkeeping forgets them and
    /// the breach continues from the top of R&D.
    CorpRearrangesRnd,
    /// CR 8.2: "Add <cards> to the top / bottom of <a deck>." The cards are a
    /// target POSITION and the end of the deck is the content (§12 rule 2),
    /// so a Seidr-class "add a card from Archives to the top of R&D" and a
    /// Compile-class "add that program to the bottom of your stack" are one
    /// instruction. The deck is the card's OWNER's (4.2.1: a card can only
    /// ever be in its owner's deck).
    MoveToDeck { card: TargetSpec, top: bool },
    /// CR 8.7.1–8.7.3: "Search <zone> for <criteria>." The searching player
    /// looks at every card in the zone (8.7.1; hidden/secret zones are
    /// temporarily visible to them alone, 8.7.1a) and may FIND up to `count`
    /// cards matching the criteria (8.7.2), taking them from the zone and
    /// setting them aside facedown (4.8.4). A searched DECK is reshuffled
    /// immediately when the search completes, before any remaining effect of
    /// this ability resolves (8.7.3); resolution then resumes (8.7.4) with
    /// the found cards addressable as [`TargetSpec::FoundBySearch`].
    ///
    /// The find is NOT a target announcement (2.x `rule_searching_does_not
    /// _target`), so it is asked at RESOLUTION time, never from the
    /// announce-targets phase.
    Search {
        /// The zone searched (§4). Only decks are shuffled afterwards.
        zone: Zone,
        /// CR 8.7.2a: the criteria a found card must match — a conjunction
        /// of the shared filter vocabulary. Empty = "a card" (no criteria).
        criteria: Vec<TargetFilter>,
        /// How many cards may be found — a quantity position (§12 rule 6).
        count: Quantity,
        /// CR 8.7.2e: searching a deck for cards with specified criteria may
        /// fail to find. Where it is false, 8.7.2d forces the player to find
        /// as many matching cards as the zone holds, up to `count`.
        may_fail: bool,
    },
    /// "Add <cards> to your grip/HQ." (8.2 movement to a hand.) The cards
    /// position takes the shared [`TargetSpec`], so "the cards found by this
    /// ability's search" (8.7.4), a fixed card and an announced choice are
    /// all one instruction.
    AddCardsToHand { cards: TargetSpec },
    /// "Then, add all of those cards that are still set aside **to the
    /// heap**." (Skorpios Defense Systems.) The same 8.2 movement
    /// [`Instruction::AddCardsToHand`] makes, said to a pile instead of a
    /// hand: "the heap" is 4.4.1's name for the Runner's discard pile, so the
    /// destination is fixed by the word and only the cards are a position. It
    /// is an ADD and not a trash — the trash occurred when the movement was
    /// replaced (8.2.2 records it there), and this is that movement
    /// completing, so a second `CardTrashed` here would count one occurrence
    /// twice.
    AddCardsToHeap { cards: TargetSpec },
    /// CR 1.17.3e / 1.17.3f / 10.1.3: "Add <cards> to <side>'s score area
    /// [as an agenda worth N agenda points]." An effect that DIRECTLY adds a
    /// card to a score area — the agenda (or the converted card) "is not
    /// considered scored or stolen", so nothing is recorded that a
    /// scored/stolen trigger condition could meet, and the (S) option's
    /// procedure is not involved at all.
    ///
    /// `as_agenda` carries 10.1.3's conversion: `Some(n)` converts a
    /// non-agenda into an agenda worth n agenda points (Fan Site's 0), which
    /// lasts until the card leaves the score area; `None` adds a card that is
    /// already an agenda, keeping its own value (Film Critic class).
    AddToScoreArea { cards: TargetSpec, to: Side, as_agenda: Option<i32> },
    /// "<side> trashes N random cards from their grip/HQ." (Personality
    /// Profiles class; the random selection is the mechanism 10.4.2 damage
    /// uses.) The count is a quantity position (§12 rule 6).
    TrashRandomFromHand { side: Side, count: Quantity },
    /// CR 1.13.1: "Host <cards> on <card>." Creating the host relationship
    /// moves each hosted object to the host's zone (1.13.12) without
    /// installing it (1.13.2a), and uninstalls an installed Corp card that
    /// becomes hosted on a Runner card (1.13.2b). Both positions take the
    /// shared [`TargetSpec`], so "host a card from HQ on this card" (Glenn
    /// Station class), "host 2 cards from your grip on this card" (Madani
    /// class) and "host that card on <another card>" are one instruction.
    /// `faceup` is CR 1.21.1: "Host those cards **faceup** on this resource"
    /// (Asmund Pudlat). A hosted card is not installed (1.13.2a), so nothing
    /// else decides which face is up, and the difference is what each player
    /// is entitled to know (§10.2).
    HostCards { cards: TargetSpec, host: TargetSpec, faceup: bool },
    /// CR 8.8.1: "Swap <a> with <b>." — the two cards exchange locations
    /// simultaneously (8.8.3/8.8.4), keeping whatever is hosted on either of
    /// them hosted on it (8.8.3a/8.8.4c).
    SwapCards { a: TargetSpec, b: TargetSpec },
    /// CR 6.2.2: "Move <ice> to <a position>." An installed piece of ice
    /// leaves its position for a new one, created when the movement happens
    /// and occupied immediately. The destination takes the shared
    /// [`InstallDest`] language — that is where the CR's position vocabulary
    /// (6.2.2a outermost, 6.2.2c directly inward) already lives — and a
    /// destination that names no position protecting a server moves nothing.
    MoveIce { ice: TargetSpec, dest: InstallDest },
    /// CR 6.2.8a: "Move to <a piece of ice>, then approach (or encounter)
    /// it." The Runner's position becomes that ice's position, the server it
    /// protects becomes the attacked server (6.1.2), and the run's current
    /// timing step becomes the Approach Ice Phase or the Encounter Ice Phase.
    /// 6.2.8c: with no position to move to — the Success Phase, the Run Ends
    /// Phase, or no run at all — the Runner does nothing.
    MoveRunnerToIce { ice: TargetSpec, encounter: bool },
    /// CR 6.2.8b: "The Runner moves to **the outermost position** of <a
    /// server>." That server becomes the attacked server; if any ice
    /// protects it, the Runner's position becomes the outermost such
    /// piece's and the current timing step becomes the Approach Ice Phase
    /// (6.9.2) — the rule names a POSITION rather than a piece of ice, so
    /// unlike [`Instruction::MoveRunnerToIce`] it never encounters directly:
    /// the approach is the rule's, and 6.4.4 decides from there. If no ice
    /// protects the server, the Runner ceases to have a position and the
    /// step becomes the Movement Phase (6.9.4). 6.2.8c: during the Success
    /// Phase, the Run Ends Phase, or outside a run, nothing.
    MoveRunnerToOutermost { server: ServerRef },
    /// CR 1.14.5: "<player> does X." — an instruction naming the player who
    /// carries out the effect. By DEFAULT the ability's controller carries
    /// out every effect and makes every choice it requires (1.14.5); where
    /// the text specifies a player ("The Runner trashes 1 program"), that
    /// player carries out that part and makes its choices instead. The
    /// INSTRUCTION is the position and the named side is the content, so the
    /// whole class — Rototurret's "Trash 1 installed program" (unwrapped,
    /// Corp-carried) versus Bulwark's "The Runner trashes 1 installed
    /// program" (wrapped) — is one variant. 1.14.5a then reads the same
    /// attribution: a trigger condition about effects "performed by" a player
    /// is met only when that player carried the effect out.
    PerformedBy { side: Side, instr: Box<Instruction> },
    /// CR 10.12.1: "Sabotage N." — the Corp trashes N cards collectively from
    /// HQ and the top of R&D. The Corp chooses how many come out of HQ
    /// (10.12.2), with the 10.12.3a floor when R&D is short and the 10.12.3b
    /// everything-goes case when both are; all of them are trashed
    /// simultaneously and enter Archives facedown (10.12.2a). The count is a
    /// quantity position (§12 rule 6).
    Sabotage { count: Quantity },
    /// CR 1.9.5: "Remove N <kind> counters from <player>." (Scapegoat class
    /// for bad publicity.) Counters HOSTED on cards are not on a player, so
    /// this instruction can never reach them (1.13.3). The count is a
    /// quantity position (§12 rule 6). Only bad publicity (10.6.1) is wired:
    /// tags have their own removal path (5.2.7g / `TagRemoved`), and the
    /// other counter kinds only ever exist hosted on cards.
    RemoveCountersFromPlayer { side: Side, kind: crate::object::CounterKind, amount: Quantity },

    /// CR 9.9.6c: "the next card you install or play this turn costs N less"
    /// as an INTERRUPT (Patchwork class): a cost that would be paid while
    /// resolving an effect is a value, so the interrupt decreases it exactly
    /// as a damage prevention decreases an imminent damage value. 1.16.2a
    /// applies to the final value at the time the cost is paid, so the value
    /// is floored at 0 there.
    ReduceImminentCost { amount: Quantity },

    /// CR 9.12.2b: "<effects>, for each <quantity>" — the effects TIED to a
    /// calculated quantity. If every one of them is an aggregated class
    /// (9.12.2c) the group is performed ONCE with its values multiplied by
    /// the quantity; if any of them is not, the group is not aggregated at
    /// all and is performed once per unit, as separate occurrences that a
    /// per-occurrence trigger condition (9.6.4b) meets separately.
    ForEach { count: Quantity, effects: Vec<Instruction> },

    /// CR 10.11.2: "identify your mark." If no server is designated, a random
    /// CENTRAL server becomes the mark for the remainder of the turn
    /// (10.11.2a); if one already is, the instruction does nothing (10.11.3).
    IdentifyMark,

    /// CR 10.9.1: "load this card with N credits" — a placement of counters
    /// that also marks the kind as LOADED, which is what an "empty" ability
    /// on the same card is linked to (10.9.2).
    LoadCounters { target: TargetSpec, kind: crate::object::CounterKind, amount: Quantity },

    /// CR 10.6.1: "the Corp takes N bad publicity" — a bad publicity counter
    /// is placed on the player. The count is a quantity position (§12 rule 6).
    /// 10.6.3c: taking it during a run does not change that run's bad
    /// publicity fund, which was filled at step 6.9.1b.
    TakeBadPublicity { side: Side, amount: Quantity },

    // ---- timing-structure-internal vocabulary ---------------------------
    /// `step_corp_turn_allotted_clicks` / `step_runner_turn_allotted_clicks`.
    GainAllottedClicks(Side),
    /// `step_*_recurring_credits_refill`.
    RefillRecurring(Side),
    /// `step_*_formal_begin`: "The <side>'s turn begins."
    TurnFormallyBegins(Side),
    /// `step_corp_turn_mandatory_draw`.
    MandatoryDraw,
    /// `step_*_discard`: discard down to maximum hand size (5.5.4).
    DiscardToHandSize(Side),
    /// `step_*_lose_unspent_clicks`.
    LoseUnspentClicks(Side),
    /// `step_*_formal_end`: "The <side>'s turn ends."
    TurnFormallyEnds(Side),
    /// `step_*_complete`: turn structure complete.
    TurnComplete(Side),
    /// `step_initiation_announce`: the Runner announces the attacked server.
    AnnounceAttackedServer(ServerId),
    /// `step_initiation_bad_publicity`: fill the bad publicity fund.
    FillBadPubFund,
    /// `step_initiation_formal_begin`: the run begins.
    RunFormallyBegins,
    /// `step_runner_position`: position set to the outermost ice, if any.
    SetPositionOutermost,
    /// `step_approach_begins`: the Runner approaches ice.
    ApproachIce,
    /// `step_encounter_begins`: the Runner encounters ice.
    EncounterIce,
    /// `step_resolve_subroutine` (yes-branch): the Corp resolves the next
    /// unbroken subroutine.
    ResolveNextSubroutine,
    /// `step_pass_ice`.
    PassIce,
    /// `step_jack_out_choice`: the Runner may jack out.
    JackOutChoice,
    /// `step_move_position`: move 1 position inward, if possible.
    MovePositionInward,
    /// `step_approach_server`.
    ApproachServer,
    /// `step_run_declared_successful`.
    DeclareRunSuccessful,
    /// `step_breach` / standalone breach: breach a server.
    BreachServer(ServerId),
    /// `step_open_priority_windows_closed` (6.9.6a).
    CloseRunPriorityWindows,
    /// `step_run_ends_bad_publicity`: empty the fund.
    EmptyBadPubFund,
    /// `step_run_declared_unsuccessful` (conditional on 6.8.4).
    DeclareRunUnsuccessfulIfApplicable,
    /// `step_run_complete`.
    RunComplete,
    /// `step_breaching_begins`.
    BreachBegins,
    /// `step_flip_archives`.
    FlipArchivesFaceup,
    /// `step_determine_candidates_limit` (HQ/R&D access counts).
    DetermineAccessLimit,
    /// `step_choose_candidate` yes-branch: Runner chooses a candidate (11.5).
    ChooseCandidate,
    /// `step_access_candidate`: access the chosen card (runs the 7.2 table).
    AccessChosenCandidate,
    /// `step_breach_complete`.
    BreachComplete,
    /// `step_card_accessed` (7.2.1).
    CardBecomesAccessed,
    /// `step_mid_access_ability` (7.2.2): the mid-access ability window.
    MidAccessWindow,
    /// `step_access_agenda` (7.2.3): if it is an agenda, the Runner steals it.
    StealIfAgenda,
    /// `step_access_complete` (7.2.4).
    AccessComplete,
}

impl Instruction {
    /// "Make a run on <server>." — no server restriction (6.7.4a) and no
    /// "If successful" clause (6.7.4).
    pub fn run(server: ServerId) -> Instruction {
        Instruction::InitiateRun {
            server: Some(server),
            allowed: RunServerSet::Any,
            if_successful: Vec::new(),
            if_would_be_successful: Vec::new(),
            during: Vec::new(),
        }
    }

    /// "Run any server." — CR 6.7.4a's unrestricted set, with the attacked
    /// server announced by the Runner at step 6.9.1a.
    pub fn run_any_server(if_successful: Vec<Instruction>) -> Instruction {
        Instruction::InitiateRun {
            server: None,
            allowed: RunServerSet::Any,
            if_successful,
            if_would_be_successful: Vec::new(),
            during: Vec::new(),
        }
    }

    /// CR 1.15.1/1.15.2: **the instruction's target positions, in
    /// announcement order** — every place this instruction's text directs a
    /// player to choose the objects it acts on, as DATA.
    ///
    /// This function is the announcement obligation itself, not a description
    /// of it: [`crate::vm::Vm`] derives both how many announcements an
    /// instruction requires and what each one asks from what is returned
    /// here, so a position declared here is announced with no other code
    /// written anywhere. Five separate instructions shipped without one
    /// (`MoveToDeck`, the counter family, `ModifyStrength`, `RevealCards`,
    /// `IfMet`), each silently resolving a `TargetSpec::Choose` to nothing;
    /// this match is exhaustive **on purpose**, with no wildcard arm, so a
    /// new `Instruction` variant does not compile until its positions — or
    /// their absence — are declared, and `tests/announcements.rs` reads this
    /// function's own arms against the enum's fields so that declaring the
    /// absence of a position a variant really has fails too.
    ///
    /// A position that is not a `TargetSpec::Choose` announces nothing
    /// (1.15.1: the objects are already named), so listing one costs nothing
    /// and omitting one is the defect.
    pub fn target_positions(&self) -> Vec<&TargetSpec> {
        match self {
            Instruction::TrashCards(spec)
            | Instruction::AccessCards { cards: spec, .. }
            | Instruction::ResolveAbilityOf { source: spec, .. }
            | Instruction::RezCard { target: spec, .. }
            | Instruction::LookAtCards { cards: spec, .. }
            | Instruction::ExposeCards { cards: spec }
            | Instruction::RevealCards { cards: spec }
            | Instruction::TakeHostedCredits { from: spec, .. }
            | Instruction::RemoveCounters { target: spec, .. }
            | Instruction::Derez { target: spec }
            | Instruction::MoveSetAsideCounters { target: spec, .. }
            | Instruction::ModifySubtypes { target: spec, .. }
            | Instruction::ForceEncounter { ice: spec }
            | Instruction::ModifyStrength { target: spec, .. }
            | Instruction::PlaceCounters { target: spec, .. }
            | Instruction::MoveCounters { to: spec, .. }
            | Instruction::AdvanceCard { target: spec }
            | Instruction::SwitchIdentity { with: spec, .. }
            | Instruction::ShuffleCardsIntoDeck { targets: spec, .. }
            | Instruction::RemoveCardsFromGame { targets: spec }
            | Instruction::InstallCard { card: spec, .. }
            | Instruction::PlayCard { card: spec, .. }
            | Instruction::MoveToDeck { card: spec, .. }
            | Instruction::AddCardsToHand { cards: spec }
            | Instruction::AddCardsToHeap { cards: spec }
            | Instruction::AddToScoreArea { cards: spec, .. }
            | Instruction::MoveIce { ice: spec, .. }
            | Instruction::MoveRunnerToIce { ice: spec, .. }
            | Instruction::LoadCounters { target: spec, .. }
            // 9.11.4c: the choice IS the position — the sentence has no
            // other content.
            | Instruction::ChooseCards { targets: spec } => vec![spec],
            // 1.13.1: WHICH cards are hosted, then WHICH card hosts them —
            // two positions, announced in printed order.
            Instruction::HostCards { cards, host, .. } => vec![cards, host],
            // 8.8.1: a swap names both cards it exchanges (8.8.2 filters the
            // second by what may occupy the first's location).
            Instruction::SwapCards { a, b } => vec![a, b],
            // 9.10.3: "choose an installed piece of ice" — the object a
            // maintained choice is a choice OF is announced like any other
            // (1.15.1b's other namespaces are named at resolution instead,
            // so they are not target positions at all).
            Instruction::MaintainChoice { of: ChoiceSpec::Object(spec), .. } => vec![spec],
            Instruction::MaintainChoice { .. } => Vec::new(),
            // 9.8.3a: the ice whose subroutines are copied is a target, and
            // so is the ice they are granted TO.
            Instruction::GrantSubroutines { to, grant: SubroutineGrant::CopiedFrom(from), .. } => {
                vec![to, from]
            }
            Instruction::GrantSubroutines { to, .. } => vec![to],
            // Everything else acts on objects it does not choose (or on no
            // object at all). Listed rather than wildcarded: this is the
            // compile-time gate.
            Instruction::GainCredits(..) | Instruction::LoseCredits(..) | Instruction::GainClicks(..)
            | Instruction::LoseClicks(..) | Instruction::Draw(..) | Instruction::DrawStepSetAside { .. }
            | Instruction::DrawStepAddToHand { .. } | Instruction::Damage { .. } | Instruction::GainTags { .. }
            | Instruction::MustTrashAccessedCard { .. } | Instruction::EndTheRun | Instruction::JackOut
            | Instruction::DeclineableChoice(..)
            | Instruction::NestedCostThen { .. } | Instruction::NestedCostUnless { .. } | Instruction::AdditionalAccesses(..)
            | Instruction::EndActionPhase(..) | Instruction::Combined(..) | Instruction::PreventDamage { .. }
            | Instruction::PreventAllDamage { .. } | Instruction::AvoidTags(..) | Instruction::RemoveTags(..)
            | Instruction::IncreaseImminentDamage { .. } | Instruction::PreventTrashOf(..)
            | Instruction::DamageUnpreventable { .. } | Instruction::ReplaceImminentDamageKind { .. }
            | Instruction::InitiateRun { .. } | Instruction::Trace { .. } | Instruction::TraceInitiate { .. }
            | Instruction::TraceCorpSpend | Instruction::TraceRunnerSpend | Instruction::TraceDetermine { .. }
            | Instruction::PsiGame { .. } | Instruction::CorpDiscards { .. } | Instruction::RestrictAccessToSelf
            | Instruction::CreateDelayedConditional { .. } | Instruction::CreateLingeringEffect { .. }
            | Instruction::ReduceRunnerMemoryThisTurn(..) | Instruction::ChooseOne { .. } | Instruction::BreakSubroutines { .. }
            // 5.2.2: the offered action's own announcements (an install's
            // destination, a play's card) are the action procedure's, made
            // as it runs — the offer itself names no object.
            | Instruction::OfferAction { .. }
            | Instruction::BypassEncounteredIce | Instruction::ChangeAttackedServer { .. }
            // 6.2.8b: the server is a ServerRef, never an object — a
            // maintained-choice server was chosen when it was maintained.
            | Instruction::MoveRunnerToOutermost { .. }
            | Instruction::PurgeVirusCounters | Instruction::FlipIdentity(..)
            | Instruction::SecretlySetFlipFace(..) | Instruction::TrashSelf
            | Instruction::StealSelfAgenda | Instruction::ScoreSelfAgenda | Instruction::InstallCards { .. }
            // 1.15.4: "them" names the cards this ability installed, so the
            // pick is a choice among a remembered list, not an announcement.
            | Instruction::RezInstalledCards { .. }
            | Instruction::InstallStepPlace | Instruction::InstallStepPayCost | Instruction::InstallStepComplete
            | Instruction::InstallRezPayCost | Instruction::InstallRezFinish | Instruction::PlayCards { .. }
            | Instruction::PlayStepPlace | Instruction::PlayStepPayCost | Instruction::PlayStepActivate
            | Instruction::PlayStepResolve | Instruction::PlayStepFinish | Instruction::RemoveSelfFromGame
            | Instruction::IfMet { .. } | Instruction::SetAsideTopOfDeck { .. } | Instruction::ArrangeSetAside { .. }
            | Instruction::CorpRearrangesRnd | Instruction::Search { .. } | Instruction::TrashRandomFromHand { .. }
            // 1.15.2b: a card taken at random is not announced by anyone.
            | Instruction::RevealRandomFromHand { .. }
            | Instruction::PerformedBy { .. } | Instruction::Sabotage { .. } | Instruction::RemoveCountersFromPlayer { .. }
            | Instruction::ReduceImminentCost { .. } | Instruction::ForEach { .. } | Instruction::IdentifyMark
            | Instruction::TakeBadPublicity { .. } | Instruction::GainAllottedClicks(..) | Instruction::RefillRecurring(..)
            | Instruction::TurnFormallyBegins(..) | Instruction::MandatoryDraw | Instruction::DiscardToHandSize(..)
            | Instruction::LoseUnspentClicks(..) | Instruction::TurnFormallyEnds(..) | Instruction::TurnComplete(..)
            | Instruction::AnnounceAttackedServer(..) | Instruction::FillBadPubFund | Instruction::RunFormallyBegins
            | Instruction::SetPositionOutermost | Instruction::ApproachIce | Instruction::EncounterIce
            | Instruction::ResolveNextSubroutine | Instruction::PassIce | Instruction::JackOutChoice
            | Instruction::MovePositionInward | Instruction::ApproachServer | Instruction::DeclareRunSuccessful
            | Instruction::BreachServer(..) | Instruction::CloseRunPriorityWindows | Instruction::EmptyBadPubFund
            | Instruction::DeclareRunUnsuccessfulIfApplicable | Instruction::RunComplete | Instruction::BreachBegins
            | Instruction::FlipArchivesFaceup | Instruction::DetermineAccessLimit | Instruction::ChooseCandidate
            | Instruction::AccessChosenCandidate | Instruction::BreachComplete | Instruction::CardBecomesAccessed
            | Instruction::MidAccessWindow | Instruction::StealIfAgenda | Instruction::AccessComplete
            // 4.2.3: the sentence names a DECK, not any card in it.
            | Instruction::ShuffleDeck { .. } => Vec::new(),
        }
    }

    /// CR 9.11.3/9.11.4: **the instructions this instruction contains**, and
    /// whose choices those are (1.15.2 scopes an announcement to "the
    /// instruction" that requires it).
    ///
    /// The same compile-time gate as [`Instruction::target_positions`], for
    /// the other half of the defect class: a contained instruction with a
    /// target position of its own is announced by whoever owns the
    /// announcement, and getting that wrong is how `IfMet` shipped without
    /// announcing its branch's targets. Exhaustive, no wildcard arm.
    pub fn contains(&self) -> Contained<'_> {
        match self {
            // 9.11.4a: several effects in ONE instruction (Snare!'s "do 3 net
            // damage and give the Runner 1 tag"). The halves resolve as part
            // of THIS instruction, so 1.15.2 puts their announcements here —
            // which is what lets a half's back-reference ("gain credits equal
            // to **its** rez cost", Blue Sun) read a card its other half
            // chose: the choice is made before the sentence becomes imminent,
            // not after part of it has resolved. The exception is a §9.2.2e
            // procedure half (9.11.4b splits several plays/installs into
            // instructions of their own), which expands into its own step
            // sequence and announces from inside it, exactly as
            // `DeclineableChoice` defers one below.
            Instruction::Combined(list) => {
                Contained::Inline(list.iter().filter(|i| !i.expands_into_steps()).collect())
            }
            // 1.14.5: the wrapper only names who chooses; the choices are the
            // wrapped instruction's, made here.
            Instruction::PerformedBy { instr, .. } => Contained::Inline(vec![instr]),
            // 9.6.9: the optional component is carried out as part of this
            // instruction unless it is one of the §9.2.2e procedures, which
            // are spliced in to expand and announce for themselves.
            Instruction::DeclineableChoice(inner) => {
                if inner.expands_into_steps() {
                    Contained::Deferred(vec![inner])
                } else {
                    Contained::Inline(vec![inner])
                }
            }
            // 9.6.5d: "if <state>, <do this>; otherwise <do that>" — ONE
            // instruction whose effects are the live branch's, so the branch
            // announces here. Which branch is live is a question about game
            // state, which only the VM can answer: the guards ride along.
            Instruction::IfMet { requires, then, otherwise } => {
                Contained::Branches(vec![(requires.as_slice(), then.as_slice()), (&[], otherwise.as_slice())])
            }
            // 9.11.4f/9.12.2b/9.11.4g: the branch, the repetition and the
            // chosen option all become instructions in their own right when
            // this one resolves, and announce their targets when they get
            // there.
            Instruction::NestedCostThen { effect, .. } | Instruction::NestedCostUnless { effect, .. } => {
                Contained::Deferred(vec![effect])
            }
            Instruction::ForEach { effects, .. } => Contained::Deferred(effects.iter().collect()),
            Instruction::ChooseOne { options } => {
                Contained::Deferred(options.iter().flat_map(|(_, i)| i.iter()).collect())
            }
            // 6.7.4/10.8/10.14: the conditional halves of a run, a trace and
            // a psi game resolve as their own ability chains later.
            // …and so does 6.9.1c's unconditional clause: it resolves as its
            // own pending instance when the run formally begins, so its
            // targets are announced then and not here.
            Instruction::InitiateRun { if_successful, if_would_be_successful, during, .. } => {
                Contained::Deferred(
                    if_successful
                        .iter()
                        .chain(if_would_be_successful.iter())
                        .chain(during.iter())
                        .collect(),
                )
            }
            Instruction::Trace { if_successful, if_unsuccessful, determined_min, .. }
            | Instruction::TraceDetermine { if_successful, if_unsuccessful, determined_min, .. } => {
                // 10.8.5's "if the trace is successful with a strength of N or
                // more, also …" is a further chain of the same kind.
                Contained::Deferred(
                    if_successful
                        .iter()
                        .chain(if_unsuccessful.iter())
                        .chain(determined_min.iter().flat_map(|(_, i)| i.iter()))
                        .collect(),
                )
            }
            Instruction::PsiGame { on_match, on_differ, .. } => {
                Contained::Deferred(on_match.iter().chain(on_differ.iter()).collect())
            }
            Instruction::GainCredits(..) | Instruction::LoseCredits(..) | Instruction::GainClicks(..)
            | Instruction::LoseClicks(..) | Instruction::Draw(..) | Instruction::DrawStepSetAside { .. }
            | Instruction::DrawStepAddToHand { .. } | Instruction::Damage { .. } | Instruction::GainTags { .. }
            | Instruction::TrashCards(..) | Instruction::MaintainChoice { .. } | Instruction::MustTrashAccessedCard { .. }
            | Instruction::EndTheRun | Instruction::JackOut | Instruction::AccessCards { .. }
            | Instruction::AdditionalAccesses(..)
            | Instruction::ResolveAbilityOf { .. } | Instruction::RezCard { .. } | Instruction::EndActionPhase(..)
            | Instruction::LookAtCards { .. } | Instruction::ChooseCards { .. } | Instruction::ExposeCards { .. }
            | Instruction::RevealCards { .. }
            | Instruction::RevealRandomFromHand { .. }
            | Instruction::TakeHostedCredits { .. } | Instruction::RemoveCounters { .. } | Instruction::Derez { .. }
            | Instruction::MoveSetAsideCounters { .. } | Instruction::PreventDamage { .. }
            | Instruction::PreventAllDamage { .. } | Instruction::AvoidTags(..) | Instruction::RemoveTags(..)
            | Instruction::IncreaseImminentDamage { .. } | Instruction::PreventTrashOf(..)
            | Instruction::DamageUnpreventable { .. } | Instruction::ReplaceImminentDamageKind { .. }
            | Instruction::TraceInitiate { .. } | Instruction::TraceCorpSpend | Instruction::TraceRunnerSpend
            | Instruction::GrantSubroutines { .. } | Instruction::CorpDiscards { .. } | Instruction::RestrictAccessToSelf
            | Instruction::CreateDelayedConditional { .. } | Instruction::CreateLingeringEffect { .. }
            | Instruction::ModifySubtypes { .. } | Instruction::ReduceRunnerMemoryThisTurn(..)
            | Instruction::BreakSubroutines { .. } | Instruction::BypassEncounteredIce | Instruction::ForceEncounter { .. }
            | Instruction::ModifyStrength { .. } | Instruction::PlaceCounters { .. } | Instruction::ChangeAttackedServer { .. }
            | Instruction::MoveCounters { .. } | Instruction::AdvanceCard { .. } | Instruction::PurgeVirusCounters
            | Instruction::FlipIdentity(..) | Instruction::SecretlySetFlipFace(..)
            | Instruction::SwitchIdentity { .. } | Instruction::ShuffleCardsIntoDeck { .. }
            | Instruction::RemoveCardsFromGame { .. } | Instruction::TrashSelf | Instruction::StealSelfAgenda
            // 5.2.2: the action this instruction offers is not a contained
            // instruction — it is the ordinary action procedure, dispatched
            // whole when the offer is answered, announcing for itself.
            | Instruction::OfferAction { .. }
            | Instruction::ScoreSelfAgenda | Instruction::InstallCard { .. } | Instruction::InstallCards { .. }
            | Instruction::RezInstalledCards { .. }
            | Instruction::InstallStepPlace | Instruction::InstallStepPayCost | Instruction::InstallStepComplete
            | Instruction::InstallRezPayCost | Instruction::InstallRezFinish | Instruction::PlayCard { .. }
            | Instruction::PlayCards { .. } | Instruction::PlayStepPlace | Instruction::PlayStepPayCost
            | Instruction::PlayStepActivate | Instruction::PlayStepResolve | Instruction::PlayStepFinish
            | Instruction::RemoveSelfFromGame | Instruction::SetAsideTopOfDeck { .. } | Instruction::ArrangeSetAside { .. }
            | Instruction::CorpRearrangesRnd | Instruction::MoveToDeck { .. } | Instruction::Search { .. }
            | Instruction::AddCardsToHand { .. } | Instruction::AddCardsToHeap { .. }
            | Instruction::AddToScoreArea { .. } | Instruction::TrashRandomFromHand { .. }
            | Instruction::HostCards { .. } | Instruction::SwapCards { .. } | Instruction::MoveIce { .. }
            | Instruction::MoveRunnerToIce { .. } | Instruction::MoveRunnerToOutermost { .. }
            | Instruction::Sabotage { .. } | Instruction::RemoveCountersFromPlayer { .. }
            | Instruction::ReduceImminentCost { .. } | Instruction::IdentifyMark | Instruction::LoadCounters { .. }
            | Instruction::TakeBadPublicity { .. } | Instruction::GainAllottedClicks(..) | Instruction::RefillRecurring(..)
            | Instruction::TurnFormallyBegins(..) | Instruction::MandatoryDraw | Instruction::DiscardToHandSize(..)
            | Instruction::LoseUnspentClicks(..) | Instruction::TurnFormallyEnds(..) | Instruction::TurnComplete(..)
            | Instruction::AnnounceAttackedServer(..) | Instruction::FillBadPubFund | Instruction::RunFormallyBegins
            | Instruction::SetPositionOutermost | Instruction::ApproachIce | Instruction::EncounterIce
            | Instruction::ResolveNextSubroutine | Instruction::PassIce | Instruction::JackOutChoice
            | Instruction::MovePositionInward | Instruction::ApproachServer | Instruction::DeclareRunSuccessful
            | Instruction::BreachServer(..) | Instruction::CloseRunPriorityWindows | Instruction::EmptyBadPubFund
            | Instruction::DeclareRunUnsuccessfulIfApplicable | Instruction::RunComplete | Instruction::BreachBegins
            | Instruction::FlipArchivesFaceup | Instruction::DetermineAccessLimit | Instruction::ChooseCandidate
            | Instruction::AccessChosenCandidate | Instruction::BreachComplete | Instruction::CardBecomesAccessed
            | Instruction::MidAccessWindow | Instruction::StealIfAgenda | Instruction::AccessComplete
            | Instruction::ShuffleDeck { .. } => {
                Contained::Nothing
            }
        }
    }

    /// CR 1.15.2: does this instruction choose any of the objects it acts on?
    /// True when one of its declared target positions is an announcement,
    /// which is what makes it an instruction of its own under 9.11.3 rather
    /// than something another instruction can absorb.
    ///
    /// An instruction that CONTAINS one inline chooses too: 1.14.5's "the
    /// Corp trashes 1 card from HQ" declares no position of its own — the
    /// wrapper only names who chooses — and the choice is still made here.
    /// Without looking through it, such a sub-instruction inside a
    /// `Combined` was not spliced out and resolved against no targets at all
    /// (the defect class of W14b's `MoveToDeck`, one level down).
    pub fn chooses_targets(&self) -> bool {
        if self.target_positions().iter().any(|s| s.announcement_slots() > 0) {
            return true;
        }
        match self.contains() {
            Contained::Inline(list) => list.iter().any(|i| i.chooses_targets()),
            Contained::Nothing | Contained::Deferred(_) | Contained::Branches(_) => false,
        }
    }

    /// CR 9.2.2e: the instructions that are PROCEDURES — they expand into a
    /// step sequence when they become imminent, and therefore announce their
    /// own targets from inside that expansion rather than where they are
    /// written.
    pub fn expands_into_steps(&self) -> bool {
        matches!(
            self,
            Instruction::InstallCard { .. }
                | Instruction::InstallCards { .. }
                | Instruction::RezInstalledCards { .. }
                | Instruction::PlayCard { .. }
                | Instruction::PlayCards { .. }
                | Instruction::Trace { .. }
        )
    }
}

/// CR 9.11.3/9.11.4: what an instruction contains, and where the contained
/// instructions' target announcements belong. Returned by
/// [`Instruction::contains`].
#[derive(Debug)]
pub enum Contained<'a> {
    /// Nothing: this instruction's effects are its own.
    Nothing,
    /// Resolved as part of THIS instruction, so their choices are made when
    /// this instruction announces (1.15.2).
    Inline(Vec<&'a Instruction>),
    /// They become instructions in their own right — spliced into the frame,
    /// pushed as a new chain, or created as an ability later — and announce
    /// their own targets when they do.
    Deferred(Vec<&'a Instruction>),
    /// One branch resolves as part of this instruction, chosen by game state
    /// (9.6.5d). Each pair is (the requirements that select it, its
    /// instructions); the first pair whose requirements all hold is live, so
    /// a branch with no requirements is the "otherwise".
    Branches(Vec<(&'a [crate::ability::TriggerRequirement], &'a [Instruction])>),
}

/// What a card's text asks a lingering effect to DO (§9.10 payload classes),
/// as data. Paired with a [`crate::lingering::WantedDuration`] by
/// [`Instruction::CreateLingeringEffect`]; the effect's source is the
/// resolving ability's source object (9.10.1: the effect then exists
/// independently of it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LingeringSpec {
    /// "Prevent all damage." for the duration (The Noble Path class; 6.8.5 —
    /// a run-bound shield expires at step 6.9.6d).
    PreventAllDamage,
    /// CR 9.9.8c: a replacement effect created ahead of time — "instead of
    /// <the effect class>, <transform>" (Security Testing / Account Siphon /
    /// Showing Off / Immolation Script classes).
    Replacement {
        applies_to: crate::effects::EffectClass,
        with: crate::lingering::ReplacementTransform,
        /// CR 6.7.4c: the replacement is one its controller may OPTIONALLY
        /// carry out, so applying it is a Decision — made where the replaced
        /// effect would happen (for a breach, step 6.9.5b).
        optional: bool,
    },
    /// "Access N additional cards from <server>." (The Maker's Eye class;
    /// added to the 7.3.6 access limit at breach step 7.5.3.)
    AdditionalAccess { server: ServerId, extra: u32 },
    /// CR 9.5.3a: "the Runner cannot use <targets>' abilities" for the
    /// duration (Wendigo class).
    CannotUseAbilitiesOf(TargetSpec),
    /// CR 7.4.2b: "the Runner cannot access more than N cards during this
    /// run" (Hudson 1.0 class). The bound is a quantity position (§12 rule 6)
    /// evaluated when the effect is created.
    AccessLimit { limit: Quantity },
    /// CR 1.2.2 + 9.10.1: "<a player> cannot <do these things to> <these
    /// cards> [for a duration]" (Saraswati Mnemonics, A Teia, Luminal
    /// Transubstantiation). WHO, WHICH CARDS and WHICH ACTS are all content
    /// on the one instruction (§12 rule 2) — see
    /// [`crate::lingering::Payload::Prohibited`], which this creates.
    Prohibit {
        scope: ProhibitionSpec,
        by: Option<Side>,
        actions: Vec<crate::lingering::ProhibitedAction>,
    },
    /// CR 1.13.3 waived: "**hosted credits are considered to be in your credit
    /// pool**" for a duration (Stimhack). The cards are DESCRIBED in the
    /// shared filter vocabulary, so "hosted credits" said of this card is
    /// [`TargetFilter::IsSource`] and a sentence about a class of card is the
    /// same effect with different content.
    ///
    /// Distinct from every [`CreditUse`] allowance, and the distinction is the
    /// whole of it. `CreditUse` says what hosted credits may be SPENT on
    /// (1.10.3c). 1.13.3 says something else — hosted counters "do not count
    /// as 'on' a player or as objects a player 'has'" — and this waives that:
    /// the credits are then read by everything that reads the pool, so a
    /// forced 1.10.3b loss takes them and a quantity asking how many credits
    /// the player has counts them. `CreditUse::AnyPayment` reaches every
    /// payment and none of those READS, which is a silent under-reach
    /// wherever the pool is counted rather than spent.
    HostedCreditsAsPool { cards: Vec<TargetFilter> },
}

/// CR 1.2.2: how a printed "cannot" picks the cards it is about, as written
/// on the card rather than as the effect ends up holding them
/// ([`crate::lingering::ProhibitionScope`]).
///
/// The split is NOT cosmetic and the kernel does not let a card author blur
/// it. A NAMING position resolves once, when the effect is created; a
/// DESCRIPTION is never resolved at all, and is re-read wherever the act is
/// offered. Writing a description into the naming position is the one mistake
/// this enum exists to prevent: [`Instruction::CreateLingeringEffect`]
/// announces nothing, deliberately (9.10.1 — the position describes rather
/// than targets), so every describing [`TargetSpec`] resolves through the
/// instruction's ANNOUNCED targets (1.15.2) and finds the empty set. Such a
/// prohibition forbids nothing and says nothing about it, which is a card
/// that compiles, looks implemented and does not exist. `Cards` therefore
/// REFUSES a describing spec outright rather than resolving it to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProhibitionSpec {
    /// "…**that card**" — a position that NAMES its objects: the card an
    /// earlier instruction of the same ability installed (8.5.16f), the
    /// source, the triggering card, an explicit list. One lingering effect is
    /// created per object named, because the prohibition is about each of
    /// them; a position that names none creates none, which is what a
    /// Saraswati whose install found an empty HQ correctly does.
    Cards(TargetSpec),
    /// "…**agendas**", "…**Corp cards**" — a DESCRIPTION in the shared filter
    /// vocabulary. One lingering effect, carrying the criteria, read wherever
    /// the act is offered.
    Matching(Vec<TargetFilter>),
}

impl ProhibitionSpec {
    /// Does this naming position actually DESCRIBE its cards? CR 1.15.2: a
    /// describing spec resolves through the announced targets, and
    /// [`Instruction::CreateLingeringEffect`] announces none — so a
    /// prohibition written this way is silently empty and
    /// [`ProhibitionSpec::Matching`] is the position that was meant.
    ///
    /// This is a property of the card DEFINITION, not of the game state: the
    /// answer is the same on every board, so a card that passes it once
    /// passes it always, and a card that fails it fails the first time it is
    /// driven rather than quietly doing nothing forever.
    pub fn misdescribes(&self) -> Option<&'static str> {
        let ProhibitionSpec::Cards(spec) = self else { return None };
        match spec {
            TargetSpec::Choose { .. } => Some("TargetSpec::Choose"),
            TargetSpec::Each(_) => Some("TargetSpec::Each"),
            TargetSpec::AccessedCardMatching(_) => Some("TargetSpec::AccessedCardMatching"),
            _ => None,
        }
    }
}

/// CR 9.8.2/9.8.3: what an ability grants when it grants subroutines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubroutineGrant {
    /// "…gains \"[subroutine] …\"", N copies of one stated subroutine.
    Stated { count: u32, sub: Box<crate::ability::AbilityDef> },
    /// "…gains the subroutines of that ice" (Loki class): ONE effect grants
    /// several subroutines at once, and 9.8.3a orders them among themselves
    /// in the order they have on the card they were copied from.
    CopiedFrom(TargetSpec),
}

/// CR 9.10.3: what a maintained choice is a choice OF.
///
/// CR 1.15.1b enumerates what an instruction can direct a player to choose or
/// "name": *"a number, a card type, a subtype, a card name, a server, or one
/// of a specified set of effects"*. Every one of those is here, and the split
/// between the variants is exactly whether the namespace is ENUMERABLE. A
/// choice between the servers, the subtypes a card lists, the ten card types
/// (2.15.2) or a stated set of effects is 9.11.4g's option choice — an
/// [`Instruction::ChooseOne`] whose branches each maintain a different value
/// — so those variants name ONE value each. A card NAME and a NUMBER have no
/// such branches to enumerate, and are [`ChoiceSpec::Named`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceSpec {
    /// "…this server." (Security Testing class.) A choice BETWEEN servers is
    /// 9.11.4g's option choice — an `Instruction::ChooseOne` whose branches
    /// each maintain a different one — so this variant names one server.
    Server(ServerId),
    /// "…choose an installed piece of ice." (Femme Fatale class.) The pick is
    /// a 1.15.2 target announcement over the shared criteria vocabulary.
    Object(TargetSpec),
    /// "…this ice subtype." (Pelangi class.) As with `Server`, the choice
    /// between subtypes is an `Instruction::ChooseOne`.
    Subtype(crate::subtype::Subtype),
    /// CR 2.15.2: "name **asset**, **ice**, **operation** or **upgrade**"
    /// (Embezzle), "name a card type" (Azmari EdTech, Falsified Credentials,
    /// Ibrahim Salem). A card has exactly one type and there are ten of them,
    /// so an unrestricted "name a card type" is as enumerable as Embezzle's
    /// four — both are an `Instruction::ChooseOne` whose branches each
    /// maintain one type, and this variant names one.
    CardType(crate::object::CardType),
    /// CR 1.15.1b: "name a card" (Ark Lockdown, Targeted Marketing…), "name a
    /// number" (RNG Key). The namespace is OPEN — every printed title, every
    /// integer (1.1.3) — so there are no branches to write and the value
    /// arrives as an answer to a decision instead. The namespace and the
    /// exclusion are both content on this one variant (§12 rule 2).
    Named { of: NameSpace, excluding: Option<NameExclusion> },
    /// CR 1.15.1b + 4.6.8: "Choose a server other than the attacked server."
    /// (AgInfusion class.) A choice BETWEEN servers is 9.11.4g's option
    /// choice wherever its branches can be written — the three centrals can —
    /// but 4.6.8d's remote servers exist only while cards make them up, so
    /// "a server" said of ALL the servers there are has no set of branches
    /// at card-write time. The namespace closes at RESOLUTION instead: the
    /// servers are enumerated then, the stated exclusion is applied, and the
    /// value arrives as the answer to a decision, exactly as
    /// [`ChoiceSpec::Named`]'s open namespaces do. The exclusion is content
    /// on this one variant (§12 rule 2); `None` is a bare "choose a server".
    AnyServer { excluding: Option<ServerExclusion> },
}

/// CR 6.1.2: a server a resolution-enumerated server choice may NOT name —
/// the "other than …" of the sentence, read from the game when the options
/// are built rather than written as a name (4.6.8's remotes have none to
/// write, and even a central exclusion is a fact about the RUN, not about
/// the card).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerExclusion {
    /// "…other than the attacked server." (6.1.2 — the server announced at
    /// initiation, or as since changed.) Outside a run nothing is excluded.
    AttackedServer,
}

/// A server POSITION in an instruction — which server an effect that acts on
/// one means. A card can name a central outright (4.6.7's three exist before
/// the game does); "**that** server", said of a choice the ability just made,
/// is 9.10.3's back-reference, read the way [`TargetSpec::MaintainedChoice`]
/// reads a remembered object — which is the only way an instruction can ever
/// reach one of 4.6.8's remotes, since none exists to name at card-write
/// time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerRef {
    /// A server the card names outright ("HQ").
    Server(ServerId),
    /// CR 9.10.3: the server this source's maintained choice remembers under
    /// `key` (AgInfusion's "that server"). Names nothing while no server is
    /// being maintained there.
    MaintainedChoice(&'static str),
}

/// CR 1.15.1b: the OPEN namespaces a card can direct a player to name — the
/// two entries of 1.15.1b's list that cannot be written as a set of branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSpace {
    /// "Name a card." CR 2.1.1: the identifier of a card is its name.
    CardName,
    /// "Name a number." CR 1.1.3: all numbers used in the game are integers.
    Number,
}

/// CR 10.1.5 / 2.1.3: a name the naming player may NOT say.
///
/// Reclamation Order's "name a card other than Reclamation Order" is
/// self-referential language: 10.1.5 says a card referencing its own name
/// without the word "copy" is to be read as "this object", so the exclusion
/// is the SOURCE's own name and no card name enters the kernel (§12 rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameExclusion {
    /// "…other than <this card>."
    SourceName,
}

/// CR 1.15.1b: a value a player NAMED, in whichever open namespace was asked
/// for. Not a target (1.15.1b: "only objects and subroutines are announced as
/// targets"), which is why it is chosen at RESOLUTION and not announced.
///
/// A card name is carried as `&'static str`, the same representation
/// [`crate::object::PrintedCard::name`] and [`TargetFilter::HasName`] already
/// use: the kernel never manufactures a title, so every name that reaches it
/// came from a real printed card, and a driver that cannot resolve a player's
/// input to one has not been given a legal answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedValue {
    /// CR 2.1.1: a card's title.
    CardName(&'static str),
    /// CR 1.1.3: an integer.
    Number(i64),
}

/// CR 9.12.3a/b: how a "must trash" requirement may be satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashMeans {
    /// 9.12.3a: no means stipulated — any mid-access ability whose resolution
    /// trashes the accessed card satisfies the requirement, and the Runner is
    /// forced to use one if they can (Mumbad Virtual Tour class).
    AnyAbility,
    /// 9.12.3b: "…if you can pay its trash cost" — only the basic trash
    /// ability (7.1.5) satisfies the requirement, so an ability that would
    /// trash the card by some other means cannot be forced (Neutralize All
    /// Threats class).
    PayingTheTrashCost,
}

/// Targets, either fixed at card-compile time or chosen at announce time
/// (9.3.4b: targets are announced before the instruction becomes imminent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetSpec {
    /// Fixed objects (test-card layer wires these directly).
    Objects(Vec<ObjectId>),
    /// The ability's own source.
    SelfSource,
    /// The source's host (Parasite-style).
    HostOfSource,
    /// The card currently being accessed.
    AccessedCard,
    /// CR 7.1.2 + 1.15.2: "the **non-agenda** card you are accessing"
    /// (Freedom Khumalo) — [`TargetSpec::AccessedCard`] with the sentence's
    /// stipulation carried in the shared filter vocabulary (§12 rule 5).
    /// Nothing is chosen: the access fixed the card, and the criteria only
    /// decide whether the description reaches it. An accessed card the
    /// criteria do not match is described by nothing, so the position
    /// resolves to no objects — and an ability whose instructions refer to
    /// the accessed card this way is not offered in the mid-access window
    /// of an access it does not describe (1.15.3 / 9.1.10's no-potential
    /// reading, enforced where the window gathers its options).
    AccessedCardMatching(Vec<TargetFilter>),
    /// CR 9.10.3: the object remembered by this source's maintained choice
    /// under `key` (Femme Fatale's "that ice"). Resolves to nothing when no
    /// such choice is being maintained.
    MaintainedChoice(&'static str),
    /// The ice currently being encountered (Forked class).
    EncounteredIce,
    /// CR 1.15.4: "…place 1 agenda counter on **it**" / "…you may expose
    /// **that card**" — the card the OCCURRENCE that met this ability's
    /// condition named. Nothing is announced: the condition fixed the card,
    /// exactly as an access fixes [`TargetSpec::AccessedCard`] and an
    /// encounter fixes [`TargetSpec::EncounteredIce`]. A condition that names
    /// no card, or an ability resolving for some other reason, reaches
    /// nothing — the same "acts on nothing" a stranded self-reference gets
    /// under 9.1.4.
    TriggeringCard,
    /// Chosen by the controller at announce time from the shared filter
    /// vocabulary. Several atoms combine as a conjunction, exactly as a
    /// search's 8.7.2a criteria do — "an installed program" is
    /// `[InstalledRunnerCard, CardTypeIs(Program)]`, and a plain
    /// "1 program" is `[CardTypeIs(Program)]` with 1.15.2c supplying the
    /// play-area restriction.
    ///
    /// `count` is a quantity POSITION (§12 rule 6): "trash 1 program" is a
    /// constant, "trash X programs, where X is the number of advancement
    /// counters on this card" (Aggressive Secretary class) is a selector.
    /// CR 1.15.2e caps it at the number of distinct valid targets available.
    Choose { count: Quantity, criteria: Vec<TargetFilter>, up_to: bool },
    /// The top N cards of a deck (Breached Dome-style). `count` is a quantity
    /// POSITION (§12 rule 6): "the top card of the stack" is a constant, and
    /// "the top X cards of your stack, where X is the number of cards you
    /// would draw plus 1" (The Class Act) is a selector.
    TopOfDeck { side: Side, count: Quantity },
    /// "…the top card of your heap." (Wyvern.) The other pile, said the same
    /// way — the cards themselves, in pile order, not a description anyone
    /// picks from — and the count is a quantity position for the same reason.
    ///
    /// CR 4.4.2 is what makes this different from [`TargetSpec::TopOfDeck`]:
    /// discard piles are NOT ordered, so this position describes NOTHING
    /// unless an active [`crate::ability::StaticDecl::DiscardPileIsOrdered`]
    /// says that pile has an order to have a top of. A card that says "the top
    /// card of your heap" without a card saying the heap is ordered is a card
    /// pointing at nothing, which is 9.1.4's stranded reference and not an
    /// error.
    TopOfDiscard { side: Side, count: Quantity },
    /// CR 8.7.4: the cards found by this ability's search, still set aside
    /// facedown (4.8.4). This is how an install/play/add-to-hand instruction
    /// refers to them without a per-card hook.
    FoundBySearch,
    /// CR 1.15.2: ONE instruction that requires SEVERAL announcements —
    /// "Trash 1 installed program and 1 installed resource" (Colossus
    /// class). Each element is announced in turn, as its own Decision, and
    /// the instruction acts on the union when it resolves (9.12.2a: one
    /// effect over a set).
    Each(Vec<TargetSpec>),
    /// CR 1.15.4: a target this ABILITY already announced, addressed by a
    /// later instruction without re-announcing it (Howler class). `nth` is
    /// 0-based over the ability's announcements in order.
    EarlierTarget { nth: usize },
    /// CR 1.15.4 in the PLURAL: "…and add **them** to HQ" (Reclamation
    /// Order). One announcement can choose several cards (1.15.2d), and a
    /// later instruction of the same ability acts on all of them — which
    /// `EarlierTarget` cannot say, since it names one.
    EarlierTargets,
    /// CR 1.15.4: "…the Corp removes 1 of those cards from the game, then you
    /// add **the other card** to your grip" (Steve Cambridge class) — the
    /// objects the current instruction's EARLIER announcements chose, except
    /// every object its LATEST announcement chose. Neither
    /// [`TargetSpec::EarlierTargets`] (all of them, the removed one included)
    /// nor [`TargetSpec::EarlierTarget`] (one, by a position that shifts when
    /// 1.15.2e announces fewer than the sentence asked for) can say it.
    ///
    /// Read as a SET difference over the announcement record, not as a zone
    /// question: 1.15.4 lets the sentence act on the card wherever the other
    /// halves have moved it, so "the one not removed" cannot be "the one
    /// still in the heap". With fewer than two announcements made — the heap
    /// held one card, so the choice and the removal named the same object —
    /// the difference is empty and 1.15.3 resolves the rest without it.
    EarlierTargetsExceptLatest,
    /// CR 8.5.16f + 1.15.4: "…remove **it** from the game", said of the card
    /// this ability's own earlier instruction INSTALLED (Kabonesa Wu). The
    /// pointing twin of [`TargetFilter::InstalledByThisAbility`], and the
    /// counterpart of [`TargetSpec::EarlierTarget`] for a card that was never
    /// announced: nothing is chosen here, so this position requires no
    /// announcement, exactly as [`TargetSpec::TriggeringCard`] requires none.
    InstalledByThisAbility,
    /// CR 4.8.7 + 1.15.4: "…all of those cards **that are still set aside**"
    /// (Skorpios Defense Systems) — every card still in this ability's own
    /// 4.8.7 set-aside group. The pointing twin of
    /// [`TargetFilter::SetAsideByThisAbility`], and a position that requires
    /// no announcement for the reason [`TargetSpec::FoundBySearch`] requires
    /// none: the group was fixed when the cards were set aside, and "still"
    /// is the zone — a card an earlier half moved somewhere else has left the
    /// set-aside zone (4.8) and is no longer one of them.
    StillSetAsideByThisAbility,
}

impl TargetSpec {
    /// CR 1.15.2: how many separate announcements this position requires —
    /// "for each time the instruction requires a player to choose 1 or more
    /// objects". A position that names its objects outright requires none
    /// (1.15.1: they are already the targets), and an `Each` requires one per
    /// announcing element.
    pub fn announcement_slots(&self) -> usize {
        match self {
            TargetSpec::Choose { .. } => 1,
            TargetSpec::Each(specs) => specs.iter().map(|s| s.announcement_slots()).sum(),
            TargetSpec::Objects(_)
            | TargetSpec::SelfSource
            | TargetSpec::HostOfSource
            | TargetSpec::AccessedCard
            | TargetSpec::AccessedCardMatching(_)
            | TargetSpec::MaintainedChoice(_)
            | TargetSpec::EncounteredIce
            | TargetSpec::TriggeringCard
            | TargetSpec::TopOfDeck { .. }
            | TargetSpec::TopOfDiscard { .. }
            | TargetSpec::FoundBySearch
            | TargetSpec::EarlierTarget { .. }
            | TargetSpec::EarlierTargets
            | TargetSpec::EarlierTargetsExceptLatest
            | TargetSpec::InstalledByThisAbility
            | TargetSpec::StillSetAsideByThisAbility => 0,
        }
    }
}

/// Which subroutines of the encountered ice a break ability acts on
/// (§9.8.6). The count is a quantity position (§12 rule 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubroutineSpec {
    /// "Break up to N subroutines." (Cleaver class.) CR 9.8.6: only
    /// UNBROKEN subroutines can be chosen; `up_to` is the "up to" of the
    /// printed text, since 1.15.2e otherwise forces as many as possible.
    Chosen { count: Quantity, up_to: bool },
    /// CR 9.8.6a: "break all subroutines" targets nothing at all, and can
    /// be used while the ice has at least 1 unbroken subroutine.
    All,
    /// CR 9.8.6b: "break all but N subroutines" — the announced targets are
    /// the ones that will NOT be broken, so already-broken subroutines are
    /// valid targets (Grappling Hook).
    AllBut { count: Quantity },
}

/// The shared object-filter language: announce-time target filters, the
/// counting filters `Quantity::Count` evaluates, and the 8.7.2a criteria of
/// a search (§12 rule 5 — one filter vocabulary for choosing, counting and
/// finding). Each variant is one predicate atom; several atoms combine as a
/// conjunction wherever the CR speaks of "the criteria" in the plural.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFilter {
    InstalledCorpCard,
    InstalledRunnerCard,
    InstalledResource,
    /// Ice protecting the server the source is protecting (Surveyor-class
    /// counting; empty when the source is not protecting a server).
    IceProtectingSourceServer,
    /// CR 6.1.2: ice protecting the server currently under attack (Na'Not'K
    /// class). Empty when no run is in progress, which is what makes the
    /// modification it feeds lapse the moment the run ends.
    IceProtectingAttackedServer,
    /// CR 4.6.6b + 6.1.2: "…a card **in the root of or protecting the
    /// attacked server**" (LEO Construction). 4.6.6b puts both the root and
    /// the ice protecting it *in* the server, so this is the whole server
    /// rather than [`TargetFilter::IceProtectingAttackedServer`]'s half of
    /// it. Empty when no run is in progress, which is what keeps an ability
    /// costing one of these cards unusable outside a run.
    InAttackedServer,
    /// CR 6.5.1: "the piece of ice being encountered" — the ice the
    /// encounter in progress is with, and nothing while there is none.
    /// [`TargetSpec::EncounteredIce`] already names it as a target; this is
    /// the same card as a DESCRIPTION, so a static ability's 9.3.7a stated
    /// condition ("during encounters with…", Acme Consulting) can ask a
    /// `BoardHasMatching` about it. The encounter fixes the card by identity,
    /// so there is no selection left for 1.15.2c to restrict — and 6.1.4's
    /// encounter with an uninstalled card is still an encounter, which is why
    /// this reads the encounter record rather than any position.
    IsEncounteredIce,
    /// CR 6.4.1/6.4.2: "the piece of ice the Runner is approaching" — the
    /// ice in the Runner's current position while the run's current timing
    /// step is inside the Approach Ice Phase (6.9.2; 6.4.2 makes the
    /// approach that phase's span at that ice's position). The approach
    /// fixes the card the way an encounter fixes
    /// [`TargetFilter::IsEncounteredIce`], so there is no selection left for
    /// 1.15.2c to restrict: a cost that trashes "the unrezzed piece of ice
    /// the Runner is approaching" (AgInfusion) names exactly it, and nothing
    /// at all in any other phase — a position still occupied after the pass
    /// (6.9.4a-d) is no longer being APPROACHED, and an encounter has a
    /// criterion of its own.
    IsApproachedIce,
    /// CR 6.2.1/6.2.2: "the outermost piece of ice protecting its server" —
    /// the ice in ITS OWN server's outermost occupied position (positions are
    /// innermost first, so it is the last ice in the sequence). Read against
    /// whatever server the candidate protects, so one atom serves "…the
    /// outermost piece of ice protecting **any** server" (Acme Consulting:
    /// conjoined with [`TargetFilter::IsEncounteredIce`]) wherever a
    /// sentence DESCRIBES such ice. (AgInfusion's "the outermost position
    /// of that server" is not this atom at all: 6.2.8b reads it as a
    /// movement to a POSITION — [`Instruction::MoveRunnerToOutermost`] —
    /// and the ice there is met by approaching it.) Hosted ice occupies no
    /// position (6.2.1a) and is never this.
    OutermostIceOfItsServer,
    /// Cards in a player's hand (Ashigaru-class counting).
    CardsInHandOf(Side),
    // ---- card-characteristic atoms (§2), location-agnostic --------------
    /// CR 2.15: "a program", "an asset", "a piece of ice" — the card's type.
    CardTypeIs(CardType),
    /// CR 2.16: "a virus program", "a region" — an effective subtype
    /// (9.12.1b counting applies through the characteristics pipeline).
    HasSubtype(crate::subtype::Subtype),
    /// CR 2.16: "…**virus or weapon** cards" (Asmund Pudlat) — ANY of these
    /// subtypes. The criteria of a description are a conjunction, so a
    /// printed "or" between subtypes cannot be two atoms; the disjunction is
    /// content on one (§12 rule 2). A `&'static [Subtype]` rather than a
    /// `Vec` so the filter vocabulary stays `Copy`, which is what lets a
    /// criterion be read wherever an object is examined.
    HasAnySubtype(&'static [crate::subtype::Subtype]),
    /// CR 2.3: "…with printed install/rez/play cost N or lower".
    PrintedCostAtMost(u32),
    /// CR 8.1.2: "a rezzed piece of ice", "a rezzed card" — an installed
    /// faceup Corp card.
    Rezzed,
    /// CR 8.1.2's other half: "1 unrezzed card" (Leela Patel class) — an
    /// installed FACEDOWN Corp card. 8.1.2 defines the pair together ("a card
    /// that is installed faceup is rezzed… a card that is installed facedown
    /// is unrezzed"), so the word names the play area exactly as its opposite
    /// does, and it reaches every installed Corp card that is not faceup —
    /// including an agenda, which can never be rezzed at all.
    ///
    /// A facedown RUNNER card is not "unrezzed": 8.1.1 makes rezzing
    /// something only the Corp does, so only a Corp card has a rez state to
    /// be in. This mirrors [`TargetFilter::Rezzed`]'s own Corp restriction.
    Unrezzed,
    /// CR 1.15.4 + 2.15: "…another card **of the same type**" (Hayley Kaplan)
    /// — the same type as the card the OCCURRENCE that met this ability's
    /// condition named. A card has exactly one type, so this is an equality
    /// and not a list. With no such card (a condition naming none, or an
    /// ability resolving for another reason) nothing matches, which is the
    /// same "reaches nothing" a stranded self-reference gets under 9.1.4.
    SameCardTypeAsTriggeringCard,
    /// CR 1.15.4 + 2.1.4: "…**another copy of that ice**" (The Foundry) — a
    /// card whose name is the name of the card the OCCURRENCE that met this
    /// ability's condition named. 2.1.4 is what makes "a copy of" a question
    /// about the NAME and nothing else, and 10.1.5 does not apply: the
    /// sentence never writes a name, so there is no self-reference to read.
    ///
    /// With no such card nothing matches, the same way
    /// [`TargetFilter::SameCardTypeAsTriggeringCard`] reaches nothing.
    SameNameAsTriggeringCard,
    /// CR 1.15.4 + 1.16.4/8.7.2a: "…a card with a printed rez cost exactly
    /// 1[credit] **less than the trashed card's** printed rez cost" (Ob
    /// Superheavy Logistics) — a card whose PRINTED rez cost differs by
    /// exactly `delta` from that of the card the OCCURRENCE that met this
    /// ability's condition named. A relational atom beside
    /// [`TargetFilter::SameNameAsTriggeringCard`], comparing a printed number
    /// (2.3) the way that one compares the name: printed characteristics
    /// travel with the card (1.17.8-adjacent), so the trashed card still
    /// answers from the discard pile.
    ///
    /// "Printed REZ cost" is 2.3's cost said of the cards that rez at all
    /// (8.1.2's assets, ice and upgrades): a side must HAVE one to stand in
    /// the relation, so an operation, an agenda or a card with no printed
    /// cost matches nothing — the same "reaches nothing" the other
    /// triggering-card atoms get with no card. 8.7.2a is why the comparison
    /// is a criterion at all: it is what the search may find, asked in the
    /// shared filter vocabulary (§12 rule 5).
    RezCostRelativeToTriggeringCard { delta: i64 },
    /// CR 2.15: "…1 resource **or** piece of hardware" (Barry "Baz" Wong) —
    /// a card has exactly one type, so several `CardTypeIs` criteria together
    /// would mean ALL of them and describe nothing. The type LIST is content
    /// on one criterion, exactly as [`TargetFilter::HasAnySubtype`] already
    /// is for the subtype "or".
    CardTypeIsAny(&'static [CardType]),
    /// "…a **non**-agenda card" / "…a **non**-virus program" — the negation
    /// of another criterion, in the same shared filter vocabulary (§12 rule
    /// 5). One word for every "non-", rather than a filter per thing negated.
    ///
    /// A negation never names a zone: "a non-agenda card" says nothing about
    /// where the card is, so 1.15.2c's play-area default still applies unless
    /// another criterion beside it lifts it.
    Not(&'static TargetFilter),
    /// "…an **icebreaker** or a **run** event" — a DISJUNCTION of criteria,
    /// in the same shared filter vocabulary (§12 rule 5). Several criteria
    /// written beside each other are a conjunction wherever the CR speaks of
    /// "the criteria" in the plural, so a printed "or" between two
    /// descriptions that are each already several words has no other way to
    /// be said: one alternative here is one such description, and the card
    /// matches when it matches every criterion of at least one of them.
    ///
    /// Deliberately NOT [`TargetFilter::CardTypeIsAny`] or
    /// [`TargetFilter::HasAnySubtype`], which are the printed "or" between
    /// single words of one kind. Those stay, because "a resource or piece of
    /// hardware" really is one description word about the type; this is for
    /// the "or" that separates whole descriptions.
    AnyOf(&'static [&'static [TargetFilter]]),
    /// "…a card that already has [a power counter]" — CR 1.9: a card hosting
    /// at least `at_least` counters of a kind. The threshold is content
    /// (§12 rule 2), so the same atom says "a card with a counter on it" and
    /// "a card with 3 or more virus counters".
    HasCounters { kind: crate::object::CounterKind, at_least: u32 },
    /// CR 1.13.2: a card that is FACEDOWN — its back face is the one showing,
    /// wherever it is. Not [`TargetFilter::Unrezzed`], which 8.1.2 restricts
    /// to installed Corp cards: 10.3.1a puts a card the Corp trashes into
    /// Archives facedown and one the RUNNER trashes there faceup, so "a
    /// facedown card in Archives" is this criterion beside
    /// [`TargetFilter::InDiscardOf`], and neither names the play area.
    Facedown,
    /// CR 9.5.5 / 4.8.3: a card SET ASIDE by the trigger cost of the ability
    /// making this selection — the only kind of ability that can see the
    /// set-aside zone at all (Street Peddler class). A zone-naming criterion,
    /// so 1.15.2c's play-area restriction lifts for it.
    SetAsideByThisAbility,
    /// CR 4.8.7 + 4.8.3: "1 card **set aside with this identity**" (Ayla
    /// "Bios" Rahim) — a card whose set-aside group was stamped with the
    /// selecting ability's SOURCE card
    /// ([`Instruction::SetAsideTopOfDeck`]'s `with_source`). The other way
    /// an ability "refers to set-aside objects" under 4.8.3: not the frame's
    /// own set-aside (that record dies with the frame) but the card's, which
    /// is how one ability of a card reaches what another ability of the same
    /// card set aside in a different resolution entirely. A zone-naming
    /// criterion, so 1.15.2c's play-area restriction lifts for it.
    SetAsideWithSource,
    /// CR 2.6: a card with a trash cost printed on it — "…you access a card
    /// **with a trash cost**" (Neutralize All Threats). 7.1.5a's polarity: a
    /// card either has one or does not, and no modifier gives one to a card
    /// that prints none, so the question is asked of the printed box.
    HasTrashCost,
    /// CR 8.4.2a: the cards a player has DRAWN and that are still set aside —
    /// "abilities with a trigger condition that refers to cards being drawn
    /// can see the drawn cards in the set-aside zone. This is an exception to
    /// rule 4.8.3." A zone-naming criterion, so 1.15.2c's play-area
    /// restriction lifts for it.
    DrawnCards,
    /// CR 1.12.3 / 1.21.2: a card THIS ability is currently looking at. An
    /// entry whose object has been re-made — a shuffle or a rearrangement
    /// moves cards to an unknown location, and 1.12.3 makes each a NEW object
    /// — no longer matches, so the ability can no longer act on it. A
    /// zone-naming criterion, so 1.15.2c's play-area restriction lifts.
    LookedAtByThisAbility,
    /// CR 1.21.6 / 1.12.3: a card THIS ability REVEALED — "the rest of **the
    /// revealed cards**" (Inject), "copies of **that agenda**" (Lakshmi
    /// Smartfabrics).
    ///
    /// 1.21.6 is one rule over two verbs, so the reveal keeps its cards on the
    /// resolving ability exactly as 1.21.2's look does, and this criterion
    /// reads that record the way [`TargetFilter::LookedAtByThisAbility`] reads
    /// its own. What keeps them two criteria and not one with a polarity is
    /// 1.21.5: looking, revealing, exposing and accessing "are not the same,
    /// even if they would be performed similarly at times", and the kernel
    /// already says so everywhere else — one instruction and one
    /// [`crate::change::GameChange`] per verb. A sentence saying "the revealed
    /// cards" must not reach a card this ability only looked at.
    ///
    /// Distinct from [`TargetFilter::RevealedThisEncounter`], which is scoped
    /// to the ENCOUNTER (Slot Machine's later subroutines read a reveal made
    /// by an earlier ability) and answers nothing outside one.
    ///
    /// A zone-naming criterion, so 1.15.2c's play-area restriction lifts: the
    /// cards it describes are wherever the reveal left them — 1.21.3a puts a
    /// card back exactly as it was — which for a reveal off the top of a deck
    /// is the deck.
    RevealedByThisAbility,
    /// CR 8.5.16f + 1.15.4: "…**that program**", said of the card THIS
    /// ability's own earlier instruction installed (Kabonesa Wu). The card was
    /// never announced — 8.7.4's find is not 1.15.2's announcement, so the
    /// program a search installed is no target of anything — which is why
    /// [`TargetFilter::IsTriggeringCard`] and the `EarlierTarget` family cannot
    /// say it and this criterion has to.
    ///
    /// Like those, it fixes cards by IDENTITY, so 1.15.2c's play-area default
    /// has nothing left to restrict: the card an ability installed is that card
    /// wherever it now sits, heap included. A sentence that also wants it to
    /// still be in the rig says so with [`TargetFilter::InstalledRunnerCard`]
    /// beside this one, which is what "if that program is **still installed**"
    /// prints.
    ///
    /// Reads the innermost resolving ability's
    /// [`crate::frames::AbilityFrame::installed_cards`], exactly as
    /// [`TargetFilter::LookedAtByThisAbility`] reads its `looked_at`.
    InstalledByThisAbility,
    /// CR 1.21.3 + 6.1.3: a card REVEALED during the encounter in progress —
    /// "the cards you revealed when this encounter began" (Slot Machine). A
    /// reveal puts the card back exactly as it was (1.21.3a), so nothing
    /// about the card records it; the encounter does, and the record dies
    /// with the encounter. A zone-naming criterion, because the cards it
    /// describes are wherever the reveal left them — on top of a deck, in a
    /// hand — and 1.15.2c would otherwise see none of them.
    RevealedThisEncounter,
    /// CR 4.5: "an agenda in the Runner's score area".
    InScoreAreaOf(Side),
    /// CR 4.4: "a card in Archives" / "a card in your heap" — a criterion
    /// that names a HIDDEN-capable zone, which is what makes 4.1.2a's reveal
    /// necessary when the criteria also stipulate a characteristic.
    InDiscardOf(Side),
    /// "…a card in the root of another server" (Pinhole class): installed in
    /// the root of a server OTHER than the attacked one.
    InRootOfServerOtherThanAttacked,
    /// CR 4.6.6: "…a card in a remote server" (Falsified Credentials). 4.6.6b
    /// makes a server the cards installed in its root TOGETHER WITH the ice
    /// protecting it, so both are "in" it; 4.6.8 is what makes "remote" a
    /// distinction the criterion can draw. Names the play area (1.15.2c).
    InRemoteServer,
    /// CR 4.2.2: "1 of the top N cards of R&D" (Top Hat class) — a criterion
    /// that explicitly specifies the zone, which is what lets 1.15.2c's
    /// play-area restriction lift for it.
    TopOfDeckOf { side: Side, n: u32 },
    /// CR 6.2.3: "a piece of ice in the same position" — ice whose server has
    /// the same number of positions inward from it as the reference position
    /// has from its own. Which position is the reference is the content
    /// (§12 rule 2), so the Rook class and the Slipstream class are one atom.
    IceInSamePositionAs(PositionRef),
    /// CR 2.1 / 10.1.5: "a copy of <name>" — a card's NAME, which is not
    /// self-referential language: it describes every card with that name,
    /// including but not limited to the ability's own source.
    HasName(&'static str),
    /// CR 9.10.3 + 2.1.4: "…a copy of **that card**" (Targeted Marketing),
    /// "…all cards with **the chosen name**" (Salem's Hospitality), "…1 card
    /// in it **of the named type**" (Ibrahim Salem), "…if it has **the named
    /// subtype**" (Wari). One atom: the card is compared against the value
    /// the ability's SOURCE is maintaining under this key.
    ///
    /// WHICH characteristic is compared is content on the maintained value
    /// (§12 rule 2), not a variant here — a maintained card name is matched
    /// against the card's name (2.1.4 makes that "copies of"), a card type
    /// against its type (2.15.2), a subtype against its subtypes (2.16, read
    /// through the 9.12.1b pipeline). A maintained SERVER, OBJECT or NUMBER
    /// describes no card, so nothing matches — 1.15.3's "as much as possible"
    /// with nothing possible.
    ///
    /// Matches nothing at all while no choice is being maintained under the
    /// key, which is what makes a Targeted Marketing whose naming was never
    /// resolved simply inert.
    MatchesMaintainedChoice(&'static str),
    /// "the Corp's identity", "a card the Runner controls" — CR 1.14.2's
    /// controller, which is the player responsible for the object and by
    /// 1.14.2c defaults to its owner. The side is content (§12 rule 2), and
    /// the criterion names no zone: an identity is never installed, so every
    /// other side-scoped atom in this vocabulary (`InstalledCorpCard`,
    /// `InstalledRunnerCard`) excludes it by construction.
    ControlledBy(Side),
    /// "this card" as a criterion — the ability's own source, and only it.
    /// Self-referential language (10.1.4), the complement of `OtherThanSource`.
    IsSource,
    /// CR 1.15.4: "**the forfeited agenda**", "**that card**" — the card the
    /// ability's triggering occurrence named, as a DESCRIPTION rather than as
    /// a back-reference. [`TargetSpec::TriggeringCard`] is the same card
    /// pointed at; this is what a quantity's filter list says about it, and
    /// it sits beside [`TargetFilter::SameNameAsTriggeringCard`] and
    /// [`TargetFilter::SameCardTypeAsTriggeringCard`], which say something
    /// weaker about the same occurrence.
    ///
    /// Like [`TargetFilter::IsSource`] it fixes ONE card by identity, so
    /// 1.15.2c's play-area default has nothing left to restrict — the card
    /// this describes is the card it describes wherever it now sits, which
    /// for a forfeited agenda is the removed-from-game zone.
    IsTriggeringCard,
    /// CR 1.15.4 in the PLURAL: "…1 program or piece of hardware from among
    /// **those cards**" (Magdalene Keino-Chemutai) — one of the cards the
    /// occurrence that met this ability's condition named.
    ///
    /// [`TargetFilter::IsTriggeringCard`] is the same description said of one
    /// card, and is what this reduces to after a per-occurrence condition
    /// (9.6.4b), which never names more. The difference is a per-EVENT
    /// condition — every card of one draw (8.4.2), every card of one discard
    /// (5.7.4) — where the sentence speaks of the whole set and the singular
    /// would silently reach only the first of them.
    ///
    /// Like its singular it fixes cards by IDENTITY, so 1.15.2c's play-area
    /// default has nothing left to restrict: the cards an occurrence named are
    /// those cards wherever they now sit, which for a discard is the heap.
    AmongTriggeringCards,
    /// CR 1.15.4: "…the Corp removes **1 of those cards** from the game"
    /// (Steve Cambridge class), where "those cards" are the ones an EARLIER
    /// ANNOUNCEMENT of this ability chose — a later announcement choosing
    /// among the earlier one, which is the same sentence
    /// [`TargetFilter::AmongTriggeringCards`] says about the cards an
    /// occurrence named. The two records are different on purpose: a
    /// condition's occurrence fills `triggering_cards` and 1.15.2's
    /// announcements fill `ability_targets`, and a description has to say
    /// which it is reading.
    ///
    /// Like the triggering-card family it fixes cards by IDENTITY, so
    /// 1.15.2c's play-area default has nothing left to restrict — the cards
    /// this ability chose are those cards wherever they now sit, which for
    /// Steve Cambridge's choice is the heap.
    AmongEarlierTargets,
    /// CR 1.13.2: "cards hosted on this card" — the source's hosted cards,
    /// installed or not (1.13.2a).
    HostedOnSource,
    /// CR 1.12.6: "a card you did not install this turn" (Seamless Launch) /
    /// "an agenda installed during this turn" (Clot's prohibition). A GAME
    /// HISTORY query, not a state one — the change log since the turn began,
    /// which 10.2.1 makes open information to both players. The polarity is
    /// content (§12 rule 2), so one atom says both sentences.
    InstalledThisTurn(bool),
    /// CR 1.12.6 / 1.18.1: "an agenda that you did not **advance** this turn"
    /// (Issuaq Adaptics). The same GAME HISTORY query
    /// [`TargetFilter::InstalledThisTurn`] makes, asked about the other
    /// movement the same sentence names — the change log since the turn began,
    /// which 10.2.1 makes open information to both players. The polarity is
    /// content (§12 rule 2).
    ///
    /// It is the ADVANCE that is asked about and not the counters: 1.18.2's
    /// "place 1 advancement counter" is not advancing, so a card that gained
    /// counters from a Tennin-class ability was never advanced, and a card
    /// advanced this turn whose counters were then removed still was.
    AdvancedThisTurn(bool),
    /// CR 1.18.3: "a card you can advance" (AstroScript Pilot Program, Slot
    /// Machine). The PERMISSION side of advancing — an agenda always, and any
    /// other installed card while an active ability says so — read through
    /// the same derivation the basic advance action uses, so the criterion
    /// and the action can never disagree. Names the play area (1.15.2c).
    CanBeAdvanced,
    /// CR 1.5.4a/b: "another identity" — a card in the additional-identities
    /// pile that player brought along with their deck. A criterion that names
    /// a zone (1.15.2c lifts for it), which is what lets an ability reach
    /// outside the game at all; 1.5.4b is the rule that says an ability
    /// naming an identity other than the current one means exactly these.
    InIdentityPileOf(Side),
    /// CR 1.15.2c's other end: "…**(from any location)**" (Skorpios Defense
    /// Systems), "…**in the heap, stack, grip, or any other location**"
    /// (Chronos Protocol), and the bare "cards" of a sentence that plainly
    /// means every one of them (Whizzard: "use these credits to trash
    /// cards"). It is a zone specification — the widest one — so it lifts the
    /// installed-cards default rather than narrowing anything, and it says
    /// nothing at all about the card, which is why it matches every object
    /// wherever it sits.
    ///
    /// Not the same as writing no criterion. A description with no criterion
    /// naming a zone is 1.15.2c's installed cards, which is what Miss Bones
    /// prints; this is what a card prints when it means the other thing.
    InAnyLocation,
    /// CR 2.13: "…from the same faction" (Rebirth) / "…that does not match
    /// the faction of your identity" (DJ Fenris) — the card's faction (2.13.3
    /// gives every identity one) compared against the faction of the named
    /// player's CURRENT identity (3.1.1: the single one in the play area).
    /// The polarity is content (§12 rule 2), so one atom says both sentences.
    /// A card with no printed faction matches neither way.
    FactionMatchesIdentityOf { side: Side, same: bool },
    /// CR 2.1.5: "…cards **with different names**" (Asmund Pudlat, Harmony
    /// AR Therapy) — "each card chosen or found by the search must have a
    /// different English name from every other card chosen or found".
    ///
    /// This is the one criterion that is NOT a predicate on a single card: it
    /// constrains the SET, so no object satisfies or fails it on its own.
    /// [`TargetFilter::is_set_criterion`] is what keeps it out of per-object
    /// matching, and the two places 2.1.5 names — the announcement of a
    /// choice and the find of a search — carry it as a property of the
    /// DECISION and enforce it on the answer.
    DistinctNames,
    /// "each **other** rezzed piece of ice", "another installed program" —
    /// the word "other" in a description, which excludes the ability's own
    /// source from the set it describes (Mother Goddess and Warden Fatuma
    /// both need it, and a swap that must name "another piece of ice" is the
    /// same atom — see deviation 30).
    OtherThanSource,
}

impl TargetFilter {
    /// CR 4.1.2a: does this criterion stipulate a CHARACTERISTIC of the card
    /// (rather than merely where it is)? A stipulation of this kind has to be
    /// demonstrated when the chosen card is not otherwise visible, which is
    /// what forces the reveal.
    pub fn stipulates_characteristic(self) -> bool {
        matches!(
            self,
            TargetFilter::CardTypeIs(_)
                | TargetFilter::HasSubtype(_)
                | TargetFilter::HasAnySubtype(_)
                | TargetFilter::PrintedCostAtMost(_)
                | TargetFilter::HasName(_)
                // 9.10.3: the maintained value IS a characteristic — a name,
                // a card type or a subtype — so a card chosen for matching it
                // has to be demonstrated to match (Salem's Hospitality trashes
                // from the grip, and the Runner shows what they trashed).
                | TargetFilter::MatchesMaintainedChoice(_)
                // 2.13: the faction is a printed characteristic like any
                // other, so a sentence stipulating one has to be demonstrated.
                | TargetFilter::FactionMatchesIdentityOf { .. }
        ) || match self {
            // A disjunction stipulates a characteristic when any alternative
            // does: showing which alternative was met is showing a
            // characteristic, and there is no alternative that avoids it.
            TargetFilter::AnyOf(alts) => {
                alts.iter().any(|alt| alt.iter().any(|f| f.stipulates_characteristic()))
            }
            _ => false,
        }
    }
}

impl TargetFilter {
    /// CR 2.1.5: is this criterion a property of the chosen SET rather than
    /// of any one card? A set criterion is skipped by per-object matching —
    /// no card satisfies or fails it alone — and is applied where the set is
    /// assembled: the 1.15.2 announcement and the 8.7.2 find.
    pub fn is_set_criterion(self) -> bool {
        matches!(self, TargetFilter::DistinctNames)
    }
}

/// CR 6.2.3: what a "same position" criterion is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionRef {
    /// The position of the ability's source — or of its HOST, when the source
    /// is hosted on a piece of ice (6.2.1a: a hosted card occupies no position
    /// of its own, so a hosted program's "same position" reads its host's).
    Source,
    /// The Runner's current position in the run (6.2.5).
    Runner,
}

impl TargetFilter {
    /// CR 1.15.2c: does this criterion "explicitly specify the zone from
    /// which an object must be selected"? When NO criterion of an
    /// announcement does, only installed cards (and counters in the play
    /// area) are valid targets — "the Runner trashes 1 program" cannot
    /// reach the grip or the stack.
    ///
    /// The installed-ness atoms are listed too: they already restrict to
    /// the play area, so the implicit restriction is a no-op for them.
    pub fn names_zone(self) -> bool {
        matches!(
            self,
            TargetFilter::InstalledCorpCard
                | TargetFilter::InstalledRunnerCard
                | TargetFilter::InstalledResource
                | TargetFilter::Rezzed
                | TargetFilter::Unrezzed
                | TargetFilter::IceProtectingSourceServer
                | TargetFilter::IceProtectingAttackedServer
                // 4.6.6: a server is a place, and only installed cards are in
                // one — root and ice alike.
                | TargetFilter::InAttackedServer
                | TargetFilter::CardsInHandOf(_)
                | TargetFilter::InScoreAreaOf(_)
                | TargetFilter::InDiscardOf(_)
                | TargetFilter::SetAsideByThisAbility
                | TargetFilter::SetAsideWithSource
                | TargetFilter::DrawnCards
                | TargetFilter::LookedAtByThisAbility
                // 1.21.6 + 1.21.3a: the reveal puts each card back exactly
                // where it was, so the cards this criterion describes are
                // wherever that is — the top of a deck, a hand — and 1.15.2c
                // would otherwise see none of them.
                | TargetFilter::RevealedByThisAbility
                // 1.15.4: the description fixes the cards by identity, the
                // same way the triggering-card criteria below do, so there is
                // no selection left for 1.15.2c to restrict.
                | TargetFilter::InstalledByThisAbility
                | TargetFilter::RevealedThisEncounter
                | TargetFilter::TopOfDeckOf { .. }
                // 6.2.1: only ice PROTECTING a server occupies a position, so
                // this criterion already names the play area.
                | TargetFilter::IceInSamePositionAs(_)
                // 6.1.4: the encounter fixes its ice by identity — and an
                // encounter with an UNINSTALLED card is still an encounter,
                // so the description must reach outside the play area.
                | TargetFilter::IsEncounteredIce
                // 6.2.1: only ice PROTECTING a server occupies the position
                // an approach is at, so this criterion already names the
                // play area.
                | TargetFilter::IsApproachedIce
                // 6.2.1: only ice PROTECTING a server occupies a position, so
                // this criterion already names the play area.
                | TargetFilter::OutermostIceOfItsServer
                // 1.18.3: only an INSTALLED card can be advanced, so this
                // criterion already names the play area.
                | TargetFilter::CanBeAdvanced
                // 4.6.6: a server is a place, and only installed cards are in
                // one.
                | TargetFilter::InRemoteServer
                // 1.13.2: a host relationship IS a location — "an agenda
                // hosted on Film Critic" says where the card is as precisely
                // as "a card in HQ" does. Without this the restriction never
                // lifts for a card that is hosted but NOT installed (Film
                // Critic's agenda, Bookmark's facedown cards), and such a
                // card could never be targeted by the very ability that put
                // it there.
                | TargetFilter::HostedOnSource
                // 1.5.4a: the pile is a place, and naming it is the only way
                // an ability can reach a card outside the game.
                | TargetFilter::InIdentityPileOf(_)
                // "from any location" names every zone at once, which is as
                // explicit a specification as naming one of them.
                | TargetFilter::InAnyLocation
                // 1.15.4: the description fixes ONE card by identity, so
                // there is no selection for 1.15.2c to restrict — the card an
                // occurrence named is that card wherever it now is.
                | TargetFilter::IsSource
                | TargetFilter::IsTriggeringCard
                | TargetFilter::AmongTriggeringCards
                // 1.15.4: the same identity-fixing, said of the cards this
                // ability's own earlier announcement chose.
                | TargetFilter::AmongEarlierTargets
        ) || match self {
            // A disjunction specifies the zone only when EVERY alternative
            // does: 1.15.2c lifts for a description that says where to look,
            // and an "or" with one branch saying nothing still leaves the
            // default standing for that branch.
            TargetFilter::AnyOf(alts) => {
                !alts.is_empty() && alts.iter().all(|alt| alt.iter().any(|f| f.names_zone()))
            }
            _ => false,
        }
    }
}

/// CR 8.2.2 / 9.9.8b: where a trashed card goes when a replacement effect has
/// modified the trash movement without replacing it by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashDestination {
    /// CR 4.9: the removed-from-game zone (Skorpios class).
    RemovedFromGame,
    /// CR 8.1.4/8.1.4d: the installed Runner card is turned facedown and
    /// stays where it is — "a Runner card turned facedown is not considered
    /// to be uninstalled and simply remains in the play area" (Harbinger
    /// class).
    FacedownInPlay,
}

/// CR 1.10.3c: "credits hosted on cards may only be spent as the card's
/// ability allows." WHAT the card allows is content (§12 rule 2), not a
/// yes/no: one card lets its credits pay for anything, another names a class
/// of payment and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditUse {
    /// "You can spend these credits on anything." (Fencer Fueno class, and
    /// every recurring-credit card that names no restriction.)
    AnyPayment,
    /// "Use these credits **to trash installed cards**." (Miss Bones.) The
    /// cards are described in the shared filter vocabulary (§12 rule 5), so
    /// 1.15.2c's default applies to the list as a whole: with no criterion
    /// naming a zone, the description reaches installed cards — which is
    /// exactly what "installed cards" says. Whizzard prints the other
    /// reading, "use these credits to trash **cards**", which is
    /// [`TargetFilter::InAnyLocation`] said in the same words.
    TrashingCards(Vec<TargetFilter>),
    /// "Use this credit **to pay for using icebreakers**." (Ele "Smoke"
    /// Scovak.) CR 9.1.6a: a paid ability is used once its trigger cost has
    /// been paid, so a payment made for a card's paid ability IS paying for
    /// using that card — and WHICH cards is the ordinary description
    /// vocabulary (§12 rule 5), read exactly as it is anywhere else.
    UsingAbilitiesOf(Vec<TargetFilter>),
    /// "Use these credits **during trace attempts**." (NBN: Making News.)
    /// CR 10.8.6c/d: the two spend steps of a trace attempt are the payment
    /// this names, and it names no card at all — the restriction is on the
    /// MOMENT, not on what is being paid for.
    TraceAttempts,
    /// "Use this credit **to advance ice**." (Weyland Consortium: Because We
    /// Built It.) CR 1.18.1: advancing is placing an advancement counter by
    /// paying for it, and 5.2.6f's basic action is the payment that does it —
    /// so this names the card the counter is going ON, described with the
    /// ordinary filter vocabulary (§12 rule 5) exactly as the other two
    /// card-naming restrictions are.
    ///
    /// It is not [`CreditUse::UsingAbilitiesOf`] wearing another name: 5.2.6f
    /// is a basic action and has no card whose ability is being used, which is
    /// why 9.1.4 leaves the payment with no cause at all.
    AdvancingCards(Vec<TargetFilter>),
    /// "You can spend hosted credits **to use programs during runs**."
    /// (Trickster Taka.) The payment [`CreditUse::UsingAbilitiesOf`] names —
    /// 9.1.6a's trigger-cost payment for a described card's ability —
    /// restricted further to a MOMENT the way [`CreditUse::TraceAttempts`]
    /// restricts to one: 6.1.1's run in progress. Both halves must hold at
    /// once; neither is the other's paraphrase.
    UsingAbilitiesDuringRuns(Vec<TargetFilter>),
    /// "Use these credits **to rez cards**." (Mumba Temple.) CR 8.1.2d's rez
    /// cost, paid for the card being rezzed — described with the ordinary
    /// filter vocabulary (§12 rule 5), so "to rez cards", "to rez ice" and "to
    /// rez bioroids" are one allowance with different content.
    ///
    /// It is NOT [`CreditUse::UsingAbilitiesOf`] under another name, for the
    /// same reason [`CreditUse::AdvancingCards`] is not: 8.1.2's rez procedure
    /// pays a card's rez cost and uses no ability at all, so writing a rez
    /// permission as the "using" allowance would let the credits pay for paid
    /// abilities they may not pay for and STILL not pay for a rez.
    Rezzing(Vec<TargetFilter>),
    /// "You can spend hosted credits **to play events**." (Mystic Maemi.) The
    /// same position for 8.6.7c's play cost, and missing for the same reason:
    /// the play procedure pays a card's play cost and uses no ability either.
    /// The cards are described the ordinary way, so "to play events", "to play
    /// cards" and "to play run events" are one allowance.
    PlayingCards(Vec<TargetFilter>),
}

/// CR 8.5.16b: the install destination, declared as part of installing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDest {
    /// Corp: the root of an existing server (8.5.2b/c).
    Root(ServerId),
    /// Corp: create a new remote server (8.5.2a).
    NewRemoteRoot,
    /// Corp: ice protecting a NEW remote server (8.5.2a + 8.5.2d) — the
    /// server is created at step 8.5.16e, exactly as for `NewRemoteRoot`.
    NewRemoteProtecting,
    /// Corp: protecting a server, outermost position (8.5.2d).
    Protecting(ServerId),
    /// "directly inward" from the ability's source ice (Brân class). If the
    /// source is not protecting a server, the destination cannot be
    /// identified and no installation takes place (8.5.14).
    InwardFromSource,
    /// CR 6.2.2b: the **innermost** position protecting a server — the new
    /// position is created inward from the innermost already-existing one.
    /// This is the other end of 6.2.2's sequence from `Protecting`, which is
    /// 6.2.2a; the CR states it as its own rule rather than as a variation on
    /// the default, and `InwardFromSource` is the third (6.2.2c). "Unless
    /// otherwise indicated" (8.5.2d) is the sentence a destination naming this
    /// end is the indication for.
    ///
    /// The server is the ATTACKED one — 6.1.2 — because that is what "that
    /// server" means to an ability met by the Runner approaching one (6.9.4g
    /// approaches the attacked server), and because 4.6.8's remote servers are
    /// created during play, so a card written before the game cannot name one.
    /// It is the same server [`TargetFilter::InAttackedServer`] describes, and
    /// it is read from the run the way [`InstallDest::BreachedServerRoot`]
    /// reads the breach.
    ///
    /// Outside a run there is no attacked server, so no destination can be
    /// identified and no installation takes place (8.5.14).
    InnermostProtectingAttackedServer,
    /// Runner: the rig (8.5.4).
    Rig,
    /// Hosted on a specific card (8.5.1a). The card is installed into the
    /// host's zone (1.13.12).
    HostedOn(ObjectId),
    /// The root of the server currently being breached (Ganked/Drafter
    /// class; resolved when the destination is declared).
    BreachedServerRoot,
    /// CR 8.5.16b: the effect states NO destination, so the installing player
    /// chooses and declares one at step 8.5.16b — every location the card
    /// could legally occupy, "including any host relationships". This is the
    /// destination of the basic install action (5.2.6d/5.2.7d), where the
    /// player picks the server; `Vm::install_destinations_for` computes the
    /// list and the answer replaces this variant with the declared one.
    DeclaredByInstaller,
    /// CR 8.5.16b + 1.15.4: "…in the root of or protecting **the same
    /// server**" (Asa Group). The installer still declares which of 4.6.6b's
    /// two halves of the server the card goes to — that is what "the root of
    /// or protecting" leaves open — but the server itself is not theirs to
    /// pick: it is the one the card the OCCURRENCE named is in.
    ///
    /// With no such card, or with one in no server at all, no destination can
    /// be identified and 8.5.14 stops the install.
    DeclaredByInstallerInServerOfTriggeringCard,
    /// CR 8.5.16b + 4.6.8: "…in the root of **a remote server**" (Saraswati
    /// Mnemonics). The installer still declares WHICH remote — 4.6.8's remotes
    /// are created during play, so a card written before the game cannot name
    /// one — but the declaration is narrowed to 4.6.6b's root half and to the
    /// remote servers, with 8.5.2a's brand-new remote among the options.
    ///
    /// A card that could occupy no such root (a piece of ice, or an asset
    /// where every remote root already holds one and no new remote may be
    /// created) leaves no destination to identify, and 8.5.14 stops the
    /// install.
    DeclaredByInstallerInRemoteRoot,
    /// CR 8.5.16b + 4.6.8 + 1.15.4: "…in the root of or protecting **another**
    /// remote server" (A Teia). The comparison
    /// [`InstallDest::DeclaredByInstallerInServerOfTriggeringCard`] makes,
    /// INVERTED — the server the card the occurrence named is in is the one
    /// server this destination will not take — and narrowed to 4.6.8's remotes
    /// the way [`InstallDest::DeclaredByInstallerInRemoteRoot`] is. Both of
    /// 4.6.6b's halves stay open, so the installer declares the server AND
    /// whether the card goes in its root or protects it.
    ///
    /// 8.5.2a's brand-new remote is among the options, and needs no comparison
    /// to qualify: a server that does not exist yet is not the one the
    /// occurrence's card is in. 4.6.8f's limit is what can rule it out, and
    /// `Vm::install_destinations_for` has already applied it.
    ///
    /// "Another" needs a first one to be other THAN. With no card named by the
    /// occurrence, or with one in no server at all, the comparison cannot be
    /// made, no destination can be identified, and 8.5.14 stops the install —
    /// the same answer the un-inverted variant gives.
    DeclaredByInstallerInAnotherRemoteServer,
    /// Runner installs with no stated destination: the rig (8.5.4). Named
    /// for the 1.13.6a choice every install offers — a card whose ability
    /// describes what it can host is an eligible destination, so the
    /// installer is asked to pick one or take the default. That choice is
    /// NOT special to this variant: it is offered for every destination
    /// (see `Vm::eligible_hosts_for`).
    RunnerChoiceHostOrRig,
}

/// Card-class filter for multi-install effects (8.5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFilter {
    Program,
    Ice,
    Any,
}

/// CR 8.5.13c: a stipulation of the installing ability that must be
/// verified by revealing the card when it is not otherwise visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealCheck {
    /// "…with printed rez cost N or lower" (Ob Superheavy class).
    PrintedRezCostAtMost(u32),
}

/// CR 9.5.6a: "A paid ability that contains an instruction that could break 1
/// or more subroutines can only be used during an encounter."
///
/// The restriction is a property of the INSTRUCTIONS, not of the card, so it
/// is derived here and applied wherever a paid ability is offered. A card that
/// additionally *refers* to a stated piece of ice ("break 1 **barrier**
/// subroutine") carries 9.5.6c's separate restriction as a
/// [`crate::ability::TimingRestriction`]; this one holds even for a card that
/// names no ice at all.
pub fn could_break_subroutines(instrs: &[Instruction]) -> bool {
    cite!("rule_paid_ability_breaks_subroutines");
    instrs.iter().any(|i| match i {
        Instruction::BreakSubroutines { .. } => true,
        Instruction::PerformedBy { instr, .. } | Instruction::DeclineableChoice(instr) => {
            could_break_subroutines(std::slice::from_ref(instr))
        }
        Instruction::NestedCostThen { effect, .. } | Instruction::NestedCostUnless { effect, .. } => {
            could_break_subroutines(std::slice::from_ref(effect))
        }
        Instruction::Combined(list) => could_break_subroutines(list),
        Instruction::ChooseOne { options } => {
            options.iter().any(|(_, is)| could_break_subroutines(is))
        }
        _ => false,
    })
}

/// CR 7.1.5b: "The Runner cannot trash or pay the trash cost of a card in the
/// Corp's discard pile, either with the basic trash ability or with other
/// mid-access abilities."
///
/// The "other mid-access abilities" half needs to know which abilities would
/// trash the card being accessed, which — like [`could_break_subroutines`] —
/// is a property of the instructions rather than of the card.
pub fn could_trash_accessed_card(instrs: &[Instruction]) -> bool {
    cite!("rule_trash_in_archives");
    instrs.iter().any(|i| match i {
        Instruction::TrashCards(TargetSpec::AccessedCard)
        | Instruction::TrashCards(TargetSpec::AccessedCardMatching(_)) => true,
        Instruction::MustTrashAccessedCard { .. } => true,
        Instruction::PerformedBy { instr, .. } | Instruction::DeclineableChoice(instr) => {
            could_trash_accessed_card(std::slice::from_ref(instr))
        }
        Instruction::NestedCostThen { effect, .. } | Instruction::NestedCostUnless { effect, .. } => {
            could_trash_accessed_card(std::slice::from_ref(effect))
        }
        Instruction::Combined(list) => could_trash_accessed_card(list),
        Instruction::ChooseOne { options } => {
            options.iter().any(|(_, is)| could_trash_accessed_card(is))
        }
        _ => false,
    })
}
