//! Test support: minimal card definitions for the CR example suite and the
//! playable-slice test. The real card layer arrives in a later wave; these
//! builders exist so kernel tests can express the CR's worked examples with
//! faithful minimal cards, built EXCLUSIVELY through the public vocabulary
//! (`PrintedCard` + `AbilityDef` + `Instruction`) — no privileged kernel
//! hooks, no state manufacture (ARCHITECTURE §12 rules 3 and 5). Driving
//! those cards is `plan`'s job, not this module's.

use crate::ability::{
    AbilityClass, AbilityDef, AbilityFlag, Condition, Cost, StaticCond, StaticDecl,
    TimingRestriction, TriggerCond, TriggerRequirement,
};
use crate::effects::DamageKind;
use crate::instr::{Instruction, Quantity, TargetSpec};
use crate::object::{CardType, CounterKind, ObjectId, PrintedCard, ServerId, Side, Zone};
use crate::vm::Vm;

// ---------------------------------------------------------------------------
// Card builders
// ---------------------------------------------------------------------------

pub fn vanilla_agenda(name: &'static str, req: u32, points: i32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Agenda);
    c.advancement_requirement = Some(req);
    c.agenda_points = Some(points);
    c
}

pub fn vanilla_asset(name: &'static str, rez: u32, trash: u32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.cost = Some(rez);
    c.trash_cost = Some(trash);
    c
}

/// A piece of ice with one "End the run." subroutine (Ice Wall shape).
pub fn etr_ice(name: &'static str, rez: u32, strength: i32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Ice);
    c.cost = Some(rez);
    c.strength = Some(strength);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run")];
    c
}

pub fn vanilla_ice(name: &'static str, rez: u32, strength: i32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Ice);
    c.cost = Some(rez);
    c.strength = Some(strength);
    c
}

pub fn vanilla_runner_card(name: &'static str, ty: CardType) -> PrintedCard {
    PrintedCard::vanilla(name, Side::Runner, ty)
}

pub fn corp_filler(name: &'static str) -> PrintedCard {
    PrintedCard::vanilla(name, Side::Corp, CardType::Operation)
}

pub fn runner_filler(name: &'static str) -> PrintedCard {
    PrintedCard::vanilla(name, Side::Runner, CardType::Event)
}

// ---------------------------------------------------------------------------
// State builders
// ---------------------------------------------------------------------------

/// Install a Corp card in a server root, optionally rezzed.
pub fn install_root(vm: &mut Vm, card: PrintedCard, server: ServerId, rezzed: bool) -> ObjectId {
    let id = vm.new_object(card, Zone::Root(server));
    vm.st.root.entry(server).or_default().push(id);
    if rezzed {
        vm.st.active_seq += 1;
        let seq = vm.st.active_seq;
        let o = vm.st.objects.get_mut(&id).unwrap();
        o.faceup = true;
        o.active_since = seq;
    }
    id
}

/// Install ice protecting a server (appended OUTERMOST).
pub fn install_ice(vm: &mut Vm, card: PrintedCard, server: ServerId, rezzed: bool) -> ObjectId {
    let id = vm.new_object(card, Zone::Ice(server));
    vm.place_ice_outermost(id, server);
    if rezzed {
        vm.st.active_seq += 1;
        let seq = vm.st.active_seq;
        let o = vm.st.objects.get_mut(&id).unwrap();
        o.faceup = true;
        o.active_since = seq;
    }
    id
}

/// Install a Runner card in the rig.
pub fn install_rig(vm: &mut Vm, card: PrintedCard) -> ObjectId {
    let id = vm.new_object(card, Zone::Rig);
    vm.st.active_seq += 1;
    let seq = vm.st.active_seq;
    let o = vm.st.objects.get_mut(&id).unwrap();
    o.faceup = true;
    o.active_since = seq;
    id
}

/// Host `guest` on `host` (both must exist). CR 1.13.12: the hosted object is
/// in the same zone as its host, so setting the relationship as SETUP puts it
/// there — a program hosted on a piece of ice is protecting that server's
/// position with it.
pub fn host_on(vm: &mut Vm, guest: ObjectId, host: ObjectId) {
    let zone = vm.st.objects[&host].zone;
    vm.st.objects.get_mut(&guest).unwrap().host = Some(host);
    vm.st.objects.get_mut(&guest).unwrap().zone = zone;
    vm.st.objects.get_mut(&host).unwrap().hosted.push(guest);
}

/// Place `n` counters of `kind` on a card as SETUP (CR 1.13.1: they are
/// hosted on it). Board state, like credits or cards in hand — the ability
/// that would have put them there is not what the examples using this are
/// about.
pub fn place_counters(vm: &mut Vm, card: ObjectId, kind: CounterKind, n: u32) {
    *vm.st.objects.get_mut(&card).unwrap().counters.entry(kind).or_insert(0) += n;
}

/// Put N filler cards in a hand.
pub fn fill_hand(vm: &mut Vm, side: Side, n: usize) -> Vec<ObjectId> {
    (0..n)
        .map(|i| {
            let name: &'static str = Box::leak(format!("filler-{i}").into_boxed_str());
            let card = match side {
                Side::Corp => corp_filler(name),
                Side::Runner => runner_filler(name),
            };
            let id = vm.new_object(card, Zone::Hand(side));
            vm.st.hand.get_mut(&side).unwrap().push(id);
            id
        })
        .collect()
}

/// Put N filler cards on top of a deck.
pub fn fill_deck(vm: &mut Vm, side: Side, n: usize) -> Vec<ObjectId> {
    (0..n)
        .map(|i| {
            let name: &'static str = Box::leak(format!("deck-{i}").into_boxed_str());
            let card = match side {
                Side::Corp => corp_filler(name),
                Side::Runner => runner_filler(name),
            };
            let id = vm.new_object(card, Zone::Deck(side));
            vm.st.deck.get_mut(&side).unwrap().push(id);
            id
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Named test-card shapes used by the CR example tests
// ---------------------------------------------------------------------------

/// Hostile-Infrastructure shape: "Whenever the Runner trashes a Corp card,
/// do 1 net damage." (mandatory, per-occurrence — 9.12.2a).
pub fn hostile_infra_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 5);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerTrashesCorpCard,
        vec![Instruction::Damage { kind: DamageKind::Net, amount: Quantity::c(1), responsible: Side::Corp }],
        false,
    )
    .labeled("hostile-infra: 1 net per trash")];
    c
}

/// Warroid-Tracker shape: "Whenever the Runner trashes at least 1 Corp
/// card…" (per-event — 9.12.2a). Effect kept observable-but-simple.
pub fn warroid_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 5);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerTrashesAtLeastOneCorpCard { in_this_server: false },
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("warroid: once per trash-event")];
    c
}

/// Aesop's shape: "When your turn begins, you may trash 1 of your installed
/// resources to gain 3 credits." (optional).
pub fn aesops_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Runner),
        vec![
            Instruction::TrashCards(TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::InstalledResource],
            }),
            Instruction::GainCredits(Side::Runner, Quantity::c(3)),
        ],
        true,
    )
    .labeled("aesops: trash a resource, gain 3")];
    c
}

/// Drug-Dealer shape: "When your turn begins, lose 1 credit." (mandatory).
pub fn drug_dealer_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Runner),
        vec![Instruction::LoseCredits(Side::Runner, 1)],
        false,
    )
    .labeled("drug-dealer: lose 1 credit")];
    c
}

/// Snare! shape (simplified): "When you access this card, do 3 net damage
/// and give the Runner 1 tag." (mandatory; active while inactive, 9.1.8a).
pub fn snare_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::Combined(vec![
            Instruction::Damage { kind: DamageKind::Net, amount: Quantity::c(3), responsible: Side::Corp },
            Instruction::GainTags(1),
        ])],
        false,
    )
    .labeled("snare: 3 net + 1 tag on access")];
    c
}

/// Decoy shape: "[trash]: Avoid receiving 1 tag." (paid interrupt).
pub fn decoy_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::trash_self(), vec![Instruction::AvoidTags(1)])
        .with_flag(AbilityFlag::Interrupt)
        .labeled("decoy: avoid 1 tag")];
    c
}

/// Geist shape: "Whenever you use a [trash] ability, draw 1 card."
pub fn geist_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::UsesTrashAbility(Side::Runner),
        vec![Instruction::Draw(Side::Runner, 1)],
        false,
    )
    .labeled("geist: draw on trash-ability use")];
    c
}

/// Biometric-Spoofing shape: "[trash]: Prevent up to 2 damage." (one kind).
pub fn biometric_like(name: &'static str, kind: DamageKind) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::PreventDamage { kind, amount: 2 }],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("biometric: prevent 2 damage")];
    c
}

/// Chrome-Parlor shape: "Prevent all <kind> damage." (free paid interrupt).
pub fn prevent_all_like(name: &'static str, kind: DamageKind) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PreventAllDamage { kind }],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("chrome-parlor: prevent all")];
    c
}

/// The-Cleaners shape as the CR 9.9.7a example uses it: a triggerable
/// interrupt that adds 1 to imminent meat damage.
pub fn cleaners_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Agenda);
    c.agenda_points = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::IncreaseImminentDamage { kind: DamageKind::Meat, amount: 1 }],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("cleaners: +1 meat")];
    c
}

/// Mr-Stone shape: "Whenever the Runner takes a tag, do 1 meat damage."
pub fn mr_stone_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerTakesTag,
        vec![Instruction::Damage { kind: DamageKind::Meat, amount: Quantity::c(1), responsible: Side::Corp }],
        false,
    )
    .labeled("mr-stone: 1 meat per tag")];
    c
}

/// A runner card with a free paid ability that takes a tag (test driver).
pub fn take_tag_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::GainTags(1)])
        .labeled("take 1 tag")];
    c
}

/// A corp card with a free paid ability doing N meat damage (test driver).
pub fn meat_damage_button(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::Damage {
            kind: DamageKind::Meat,
            amount: Quantity::c(n as i64),
            responsible: Side::Corp,
        }],
    )
    .labeled("do meat damage")];
    c
}

/// Breached-Dome shape: "When you access this card, do 1 net damage and
/// trash the top card of the stack." — ONE instruction, two effects.
pub fn breached_dome_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::Combined(vec![
            Instruction::Damage { kind: DamageKind::Net, amount: Quantity::c(1), responsible: Side::Corp },
            Instruction::TrashCards(TargetSpec::TopOfDeck(Side::Runner, 1)),
        ])],
        false,
    )
    .labeled("dome: 1 net + trash top of stack")];
    c
}

/// A corp card with a free paid ability doing N net damage (test driver).
pub fn net_damage_button(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(n as i64),
            responsible: Side::Corp,
        }],
    )
    .labeled("do net damage")];
    c
}

/// A runner card with a free paid ability trashing fixed targets (Singularity
/// stand-in driver: one instruction, simultaneous set trash — 9.12.2a).
pub fn trash_set_button(name: &'static str, targets: Vec<ObjectId>) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::TrashCards(TargetSpec::Objects(targets))],
    )
    .labeled("trash the set")];
    c
}

/// Tori-Hanzō shape: "The first time each run you would do net damage, …" —
/// conditional interrupt, ordinal-gated (9.9.5a). Effect: +0 observable
/// marker (gain 0 credits) — the test asserts availability, not the effect.
pub fn tori_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Conditional,
        flags: vec![AbilityFlag::Interrupt],
        condition: Some(Condition::Trigger(TriggerCond::WouldDamage {
            kind: Some(DamageKind::Net),
            first_each_run: true,
        })),
        cost: None,
        instructions: vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        statics: Vec::new(),
        optional: true,
        timing: None,
        label: "tori: first net damage each run",
    }];
    c
}

/// Feedback-Filter shape: "[trash]: prevent all net damage."
pub fn feedback_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::PreventAllDamage { kind: DamageKind::Net }],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("feedback: prevent all net")];
    c
}

/// Jesminder shape: "The first time each turn you would take a tag during a
/// run, avoid it." — modelled as an optional conditional interrupt whose
/// relevance requires a run in progress (10.3.6 example).
pub fn jesminder_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Conditional,
        flags: vec![AbilityFlag::Interrupt],
        condition: Some(Condition::Trigger(TriggerCond::WouldTakeTags { during_run: true })),
        cost: None,
        instructions: vec![Instruction::AvoidTags(1)],
        statics: Vec::new(),
        optional: false, // the printed ability is mandatory
        timing: None,
        label: "jesminder: avoid a tag during a run",
    }];
    c
}

/// AMAZE shape: "Whenever a run on this server ends, give the Runner 2
/// tags." (mandatory; upgrade in the root).
pub fn amaze_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunOnThisServerEnds,
        vec![Instruction::GainTags(2)],
        false,
    )
    .labeled("amaze: 2 tags when run on server ends")];
    c
}

/// Crisium shape: "Runs on this server cannot be declared successful."
pub fn crisium_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(5);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::RunsNotDeclaredSuccessful])
        .labeled("crisium: runs not declared successful")];
    c
}

/// Tollbooth shape (assertion-shaped): mandatory "when encountered" body
/// whose observable effect is ending the run.
pub fn tollbooth_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 8, 5);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::EndTheRun],
        false,
    )
    .labeled("tollbooth: pay-or-ETR on encounter")];
    c
}

/// Femme shape: "Whenever you encounter a piece of ice, you may pay 1
/// credit to bypass it." (optional conditional with a nested cost, 9.11.4f).
pub fn femme_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::EncounterBegins,
        vec![Instruction::NestedCostThen {
            cost: Cost::credits(1),
            effect: Box::new(Instruction::BypassEncounteredIce),
            payer: None,
        }],
        true,
    )
    .labeled("femme: pay 1 to bypass")];
    c
}

/// Security-Nexus shape for 6.8.2b: "When you encounter a piece of ice, you
/// may end the run." (optional; ends the run from inside the window).
pub fn nexus_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::EncounterBegins,
        vec![Instruction::DeclineableChoice(Box::new(Instruction::EndTheRun))],
        true,
    )
    .labeled("nexus: may end the run on encounter")];
    c
}

/// An icebreaker with "[1 credit]: +2 strength" (pump; encounter duration
/// 9.10.4a).
pub fn pump_breaker(name: &'static str, base_strength: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(base_strength);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![Instruction::ModifyStrength {
            target: TargetSpec::SelfSource,
            amount: 2,
            duration: None,
        }],
    )
    .labeled("pump: +2 strength")];
    c
}

/// Parasite shape: static condition "while the host ice has 0 or less
/// strength, trash it." (mandatory, 9.6.7).
pub fn parasite_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Conditional,
        flags: Vec::new(),
        condition: Some(Condition::Static(StaticCond::HostStrengthAtMost(0))),
        cost: None,
        instructions: vec![Instruction::TrashCards(TargetSpec::HostOfSource)],
        statics: Vec::new(),
        optional: false,
        timing: None,
        label: "parasite: trash 0-strength host",
    }];
    c
}

/// Architect shape: "This ice cannot be trashed." (static restriction).
pub fn architect_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 4, 0);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::CannotBeTrashed])
        .labeled("architect: cannot be trashed")];
    c
}

/// Sacrificial-Construct shape: "[trash]: prevent an installed card from
/// being trashed." — kernel form targets a fixed protected object.
pub fn sac_con_like(name: &'static str, protects: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::PreventTrashOf(protects)],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("sac-con: prevent trash")];
    c
}

/// Built-to-Last shape: "Whenever you advance a card that had no advancement
/// counters, gain 2 credits." (9.6.6a "had").
pub fn built_to_last_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::AdvancesCard { had_no_advancement: true },
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(2))],
        false,
    )
    .labeled("btl: gain 2 on fresh advance")];
    c
}

/// A corp card with "[click]: place 1 advancement counter on <target>" —
/// advance-button driver for the Built to Last test.
pub fn advance_button_card(name: &'static str, target: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Default::default() },
        vec![Instruction::AdvanceCard { target: TargetSpec::Objects(vec![target]) }],
    )
    .labeled("advance target")];
    c
}

/// Freedom-Khumalo shape: an access-flagged paid ability with a zero cost
/// that trashes the accessed card (1.16.1d: zero costs are really paid).
pub fn khumalo_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::TrashCards(TargetSpec::AccessedCard)],
    )
    .with_flag(AbilityFlag::Access)
    .labeled("khumalo: trash accessed for 0")];
    c
}

/// Clone-Chip shape: "[trash]: gain 1 credit." (the effect is irrelevant;
/// the [trash] trigger cost is the point — 1.16.1a).
pub fn clone_chip_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
    )
    .labeled("clone-chip: [trash] for value")];
    c
}

/// LLDS-Energy-Regulator shape: a paid interrupt that could prevent a
/// hardware trash — never offered against trigger-cost payment (1.16.1a).
pub fn llds_like(name: &'static str, protects: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PreventTrashOf(protects)],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("llds: prevent hardware trash")];
    c
}

/// Zer0 shape: "once per turn — suffer 1 net damage as part of the cost:
/// gain 1 credit." (1.16.1c: the restriction forbids even attempting the
/// cost again.)
pub fn zer0_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::paid(
        Cost::net_damage(1),
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
    )
    .with_flag(AbilityFlag::OncePerTurn)
    .labeled("zer0: damage-cost value")];
    c
}

/// Funhouse shape: "When the Runner encounters this ice, end the run unless
/// they take 1 tag." (1.16.1b / 1.16.11b).
pub fn funhouse_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 5, 4);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::NestedCostUnless {
            cost: Cost::tags(1),
            effect: Box::new(Instruction::EndTheRun),
            payer: Some(Side::Runner),
        }],
        false,
    )
    .labeled("funhouse: ETR unless 1 tag")];
    c
}

/// A subroutine reading "End the run unless the Runner pays 1[credit]."
/// (1.16.11b).
pub fn etr_unless_pay_ice(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::NestedCostUnless {
        cost: Cost::credits(1),
        effect: Box::new(Instruction::EndTheRun),
        payer: Some(Side::Runner),
    }])
    .labeled("[sub] ETR unless 1c")];
    c
}

/// Fermenter shape: "[click], [trash]: gain 2[credit] for each hosted virus
/// counter." (9.5.5: set-aside counters still count.)
pub fn fermenter_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::GainCredits(
            Side::Runner,
            Quantity::Times(2, Box::new(Quantity::CountersOnSource(CounterKind::Virus))),
        )],
    )
    .labeled("fermenter: cash out virus counters")];
    c
}

/// Reconstruction-Contract shape: "[trash]: move the hosted advancement
/// counters to another installed card." (9.5.5 example 3.)
pub fn reconstruction_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::MoveSetAsideCounters {
            kind: CounterKind::Advancement,
            target: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::InstalledCorpCard],
            },
        }],
    )
    .labeled("reconstruction: move counters")];
    c
}

/// Arruaceiras-Crew shape: a paid ability usable only during an encounter
/// (9.5.6c).
pub fn arruaceiras_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::GainTags(1)])
        .with_timing(TimingRestriction::EncounterOnly { required_subtype: None })
        .labeled("arruaceiras: take 1 tag (encounter only)")];
    c
}

/// Project-Wotan shape: a Corp paid ability usable only while the Runner is
/// approaching a rezzed *bioroid* piece of ice (9.5.6b).
pub fn wotan_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Agenda);
    c.agenda_points = Some(1);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))])
        .with_timing(TimingRestriction::ApproachOnly {
            required_subtype: Some("bioroid"),
            rezzed: true,
        })
        .labeled("wotan: approach-only ability")];
    c
}

/// Little-Engine shape: "[sub] End the run. [sub] The Runner gains 5[c]."
pub fn little_engine_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 6, 4);
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run"),
        AbilityDef::subroutine(vec![Instruction::GainCredits(Side::Runner, Quantity::c(5))])
            .labeled("[sub] Runner gains 5"),
    ];
    c
}

/// Obokata shape: an agenda with "as an additional cost to steal, suffer 4
/// net damage" (1.16.10a).
pub fn obokata_like(name: &'static str, points: i32) -> PrintedCard {
    let mut c = vanilla_agenda(name, 4, points);
    c.additional_steal_cost = Some(Cost::net_damage(4));
    c
}

/// Ben-Musashi shape: a rezzed upgrade adding +2 net damage to steal costs.
pub fn musashi_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(2);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::AdditionalStealCost(
        Cost::net_damage(2),
    )])
    .labeled("musashi: +2 net to steal")];
    c
}

/// Predictive-Algorithm shape: +2[credit] additional cost to steal.
pub fn predictive_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Operation);
    // Modeled as an active play-area static for the test's purposes.
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::AdditionalStealCost(
        Cost::credits(2),
    )])
    .labeled("predictive: +2c to steal")];
    c
}

/// Order-of-Sol-adjacent observability card: "Whenever you suffer damage,
/// gain 1 credit." (mandatory; used to observe 1.16.10b trigger timing.)
pub fn sol_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerSuffersDamage,
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        false,
    )
    .labeled("sol: gain 1 on damage")];
    c
}

/// Process-Automation shape: one instruction, "Gain 2[c] and draw 1 card."
pub fn process_automation_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::Combined(vec![
            Instruction::GainCredits(Side::Runner, Quantity::c(2)),
            Instruction::Draw(Side::Runner, 1),
        ])],
    )
    .labeled("process-automation: gain 2 draw 1")];
    c
}

/// Lockdown shape (as a static for the 9.9.2 example): "The Runner cannot
/// draw cards."
pub fn lockdown_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::CannotDraw(Side::Runner)])
        .labeled("lockdown: runner cannot draw")];
    c
}

/// The-Class-Act shape: "The first time each turn you would draw any number
/// of cards…" — a conditional interrupt relevant to imminent draws (9.9.3d).
pub fn class_act_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Conditional,
        flags: vec![AbilityFlag::Interrupt],
        condition: Some(Condition::Trigger(TriggerCond::WouldDraw { first_each_turn: true })),
        cost: None,
        instructions: vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        statics: Vec::new(),
        optional: true,
        timing: None,
        label: "class-act: on first would-draw",
    }];
    c
}

/// Harbinger shape: a conditional interrupt with the condition "this card
/// would be trashed" (9.9.4c relevance re-evaluation).
pub fn harbinger_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(0);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Conditional,
        flags: vec![AbilityFlag::Interrupt],
        condition: Some(Condition::Trigger(TriggerCond::SelfWouldBeTrashed)),
        cost: None,
        instructions: vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        statics: Vec::new(),
        optional: true,
        timing: None,
        label: "harbinger: when this would be trashed",
    }];
    c
}

/// A corp button trashing a fixed set (corp-side driver).
pub fn corp_trash_button(name: &'static str, targets: Vec<ObjectId>) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::TrashCards(TargetSpec::Objects(targets))],
    )
    .labeled("corp-trash: trash the set")];
    c
}

/// Flare shape: "Do 2 meat damage that cannot be prevented." (9.9.7e.)
pub fn flare_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::DamageUnpreventable {
            kind: DamageKind::Meat,
            amount: Quantity::c(2),
            responsible: Side::Corp,
        }],
    )
    .labeled("flare: 2 unpreventable meat")];
    c
}

/// The-Cleaners shape as printed: a STATIC "+1 to meat damage done by the
/// Corp" (9.4.5/9.9.7e).
pub fn cleaners_static_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Agenda);
    c.agenda_points = Some(1);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::DamageBonus {
        kind: DamageKind::Meat,
        responsible: Side::Corp,
        amount: 1,
    }])
    .labeled("cleaners-static: +1 meat")];
    c
}

/// Tori-Hanzō's replacement form: "pay 2[c]: replace the imminent net
/// damage with core damage" (9.9.10).
pub fn tori_replace_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(2),
        vec![Instruction::ReplaceImminentDamageKind { to: DamageKind::Core }],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("tori-replace: net becomes core")];
    c
}

/// AMAZE with the persistent flag armed (9.12.5).
pub fn amaze_persistent_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunOnThisServerEnds,
        vec![Instruction::GainTags(2)],
        false,
    )
    .with_flag(AbilityFlag::Persistent)
    .labeled("amaze: 2 tags when run on server ends")];
    c
}

/// Doppelgänger shape: "When a run ends, you may make another run on
/// <server>."
pub fn doppel_like(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunEnds { successful_only: false },
        vec![Instruction::DeclineableChoice(Box::new(Instruction::run(server)))],
        true,
    )
    .labeled("doppel: run again")];
    c
}

/// Gemini shape (10.8.5): "Trace 3 — if successful, do 1 net damage. When
/// the trace is determined, if your trace strength is 5 or greater, do 1
/// net damage."
pub fn gemini_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::Trace {
            base: Quantity::c(3),
            if_successful: vec![Instruction::Damage {
                kind: DamageKind::Net,
                amount: Quantity::c(1),
                responsible: Side::Corp,
            }],
            if_unsuccessful: vec![],
            determined_min: Some((
                5,
                vec![Instruction::Damage {
                    kind: DamageKind::Net,
                    amount: Quantity::c(1),
                    responsible: Side::Corp,
                }],
            )),
        }],
    )
    .labeled("gemini: trace 3")];
    c
}

/// Adrian-Seis-adjacent psi button: "Play a Psi Game. If the bids differ,
/// the Runner gains 1 credit tag-marker." (outcome observability)
pub fn psi_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PsiGame {
            on_match: vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
            on_differ: vec![Instruction::GainTags(1)],
        }],
    )
    .labeled("psi: play a psi game")];
    c
}

/// Fencer-Fueno shape: hosted credits are spendable by the Runner.
pub fn fencer_like(name: &'static str, credits: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.hosted_credits_spendable = true;
    let _ = credits; // loaded by the test after install
    c
}

/// RSVP shape: "The Runner cannot spend credits." (static for the test.)
pub fn rsvp_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::CannotSpendCredits(
        Side::Runner,
    )])
    .labeled("rsvp: runner cannot spend")];
    c
}

/// Ashigaru shape: "This ice gains '[sub] End the run.' for each card in
/// HQ." (category 9.8.3d).
pub fn ashigaru_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 9, 4);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::GainSubroutines {
        sub: Box::new(AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] ETR")),
        count: Quantity::Count(crate::instr::TargetFilter::CardsInHandOf(Side::Corp)),
    }])
    .labeled("ashigaru: sub per HQ card")];
    c
}

/// Panic-Button shape: a Corp paid ability drawing 1 (usable mid-encounter).
pub fn panic_button_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::Draw(Side::Corp, 1)])
        .labeled("panic-button: corp draws 1")];
    c
}

/// Utopia-Shard shape: force the Corp to discard 2 from HQ.
pub fn utopia_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::CorpDiscards { count: 2 }])
        .labeled("utopia: corp discards 2")];
    c
}

/// A generic "break 1 subroutine" button (encounter-only, 9.5.6a).
pub fn break_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::BreakSubroutines {
            subs: crate::instr::SubroutineSpec::Chosen { count: Quantity::c(1), up_to: false },
        }])
        .with_timing(TimingRestriction::EncounterOnly { required_subtype: None })
        .labeled("break: 1 subroutine")];
    c
}

/// Brainstorm shape: "When the Runner encounters this ice, it gains 2
/// '[sub] Do 1 core damage.' subroutines." (category 9.8.3e.)
pub fn brainstorm_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 9, 4);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::GrantSubroutines {
            to: TargetSpec::SelfSource,
            duration: crate::lingering::WantedDuration::ThisEncounter,
            grant: crate::instr::SubroutineGrant::Stated {
                count: 2,
                sub: Box::new(
                    AbilityDef::subroutine(vec![Instruction::Damage {
                        kind: DamageKind::Core,
                        amount: Quantity::c(1),
                        responsible: Side::Corp,
                    }])
                    .labeled("[sub] 1 core"),
                ),
            },
            before: false,
            any_order: false,
        }],
        false,
    )
    .labeled("brainstorm: gain core subs")];
    c
}

/// Security-Testing / Account-Siphon / Showing-Off shape (9.9.8c): a card
/// whose paid ability creates a turn-bound replacement effect on the Breach
/// effect class — "the next time you would breach a server this turn,
/// instead …". The transform is the parameter, so the whole class is one
/// shape.
pub fn breach_replacement_card(
    name: &'static str,
    label: &'static str,
    transform: crate::lingering::ReplacementTransform,
) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::Replacement {
                optional: false,
                applies_to: crate::effects::EffectClass::Breach,
                with: transform,
            },
            duration: crate::lingering::WantedDuration::ThisTurn,
        }],
    )
    .labeled(label)];
    c
}

/// Marker shape (9.8.3e): a card whose paid ability grants a subroutine to
/// ANOTHER piece of ice for a stated duration — an external grant, which
/// sorts after the ice's printed subroutines and orders within its category
/// by grant time (oldest first).
pub fn subroutine_granter(
    name: &'static str,
    ice: ObjectId,
    sub: AbilityDef,
    before: bool,
    duration: crate::lingering::WantedDuration,
) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::GrantSubroutines {
            to: TargetSpec::Objects(vec![ice]),
            grant: crate::instr::SubroutineGrant::Stated { count: 1, sub: Box::new(sub) },
            before,
            any_order: false,
            duration,
        }],
    )
    .labeled("marker: grant a subroutine")];
    c
}

/// Ash shape (7.4.2): "Whenever the Runner breaches this server, trace 4 —
/// if successful, the Runner cannot access any card other than this one for
/// the remainder of the run."
pub fn ash_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(2);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::ThisServerBreached,
        vec![Instruction::Trace {
            base: Quantity::c(4),
            if_successful: vec![Instruction::RestrictAccessToSelf],
            if_unsuccessful: vec![],
            determined_min: None,
        }],
        false,
    )
    .labeled("ash: lock access on trace")];
    c
}

/// Zahya shape (9.3.6g): "once per turn — when a run ends, you may gain 1
/// credit." Declining does not use it.
pub fn zahya_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunEnds { successful_only: false },
        vec![Instruction::DeclineableChoice(Box::new(Instruction::GainCredits(Side::Runner, Quantity::c(1))))],
        true,
    )
    .with_flag(AbilityFlag::OncePerTurn)
    .labeled("zahya: may gain 1 when run ends")];
    c
}

/// Tithonium shape: ice that prohibits hosting (10.3.1e).
pub fn tithonium_like(name: &'static str, rez: u32) -> PrintedCard {
    let mut c = vanilla_ice(name, rez, 5);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::CannotHost])
        .labeled("tithonium: cannot host")];
    c
}

/// Chisel shape: a hosted program (the hosting side of the 10.3.1e test).
pub fn chisel_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c
}

/// Bad-Times shape: "The Runner loses 2[mu] until end of turn."
pub fn bad_times_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::ReduceRunnerMemoryThisTurn(2)],
    )
    .labeled("bad-times: -2 mu this turn")];
    c
}

/// A program with a given memory cost (10.3.1e minimal-set fodder).
pub fn program_mu(name: &'static str, mu: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(mu);
    c
}

/// In-the-Groove shape (9.6.13b): create a delayed conditional with an
/// explicit "this turn" duration — "whenever the Runner takes a tag, gain
/// 1 credit."
pub fn groove_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateDelayedConditional {
            def: Box::new(AbilityDef::conditional(
                TriggerCond::RunnerTakesTag,
                vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
                false,
            )
            .labeled("groove-delayed: gain 1 per tag")),
            duration: crate::lingering::WantedDuration::ThisTurn,
        }],
    )
    .labeled("groove: install the delayed trigger")];
    c
}

/// Joshua-B shape (9.6.13c): create a delayed conditional with NO stated
/// duration — "when this turn ends, gain 1 credit" — one-shot.
pub fn joshua_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateDelayedConditional {
            def: Box::new(AbilityDef::conditional(
                TriggerCond::TurnEnds(Side::Runner),
                vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
                false,
            )
            .labeled("joshua-delayed: gain 1 at turn end")),
            duration: crate::lingering::WantedDuration::UntilResolved,
        }],
    )
    .labeled("joshua: install the turn-end trigger")];
    c
}

/// Mayfly shape (9.6.13d): try to create a "when this run ends, trash this"
/// delayed conditional.
pub fn mayfly_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateDelayedConditional {
            def: Box::new(AbilityDef::conditional(
                TriggerCond::RunEnds { successful_only: false },
                vec![Instruction::TrashSelf],
                false,
            )
            .labeled("mayfly-delayed: trash at run end")),
            duration: crate::lingering::WantedDuration::UntilResolved,
        }],
    )
    .labeled("mayfly: arm the run-end trash")];
    c
}

/// Ritual shape (9.12.2b): "Draw 3 cards." — one instance of drawing 3.
pub fn ritual_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::Draw(Side::Runner, 3)])
        .labeled("ritual: draw 3")];
    c
}

/// Urtica-Cipher shape (9.12.2b): "When accessed, do 2 net damage plus 1
/// net damage for each hosted advancement counter." — ONE aggregated
/// damage instance.
pub fn urtica_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::base_plus_per_counter(2, 1, CounterKind::Advancement),
            responsible: Side::Corp,
        }],
        false,
    )
    .labeled("urtica: 2 net + 1 per advancement")];
    c
}

/// Fairchild-2.0-style subroutine ice (9.12.3c): each sub reads "The Runner
/// must pay 2[c] or trash 1 of their installed cards."
pub fn fairchild_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 5, 4);
    let sub = AbilityDef::subroutine(vec![Instruction::ChooseOne {
        options: vec![
            ("pay 2", vec![Instruction::LoseCredits(Side::Runner, 2)]),
            (
                "trash installed",
                vec![Instruction::TrashCards(TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![crate::instr::TargetFilter::InstalledRunnerCard],
                })],
            ),
        ],
    }])
    .labeled("[sub] pay 2 or trash");
    c.abilities = vec![sub.clone(), sub];
    c
}

/// Data-Raven shape (9.12.3d): "When encountered, the Runner must take 1
/// tag or end the run."
pub fn data_raven_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 4, 4);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::ChooseOne {
            options: vec![
                ("take 1 tag", vec![Instruction::GainTags(1)]),
                ("end the run", vec![Instruction::EndTheRun]),
            ],
        }],
        false,
    )
    .labeled("data-raven: tag or ETR")];
    c
}

// ---------------------------------------------------------------------------
// §8.5 install shapes (W3)
// ---------------------------------------------------------------------------

/// A program with a printed install cost (install-example fodder).
pub fn program_cost(name: &'static str, cost: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.cost = Some(cost);
    c
}

/// A vanilla Corp upgrade.
pub fn vanilla_upgrade(name: &'static str, rez: u32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.cost = Some(rez);
    c
}

/// A region upgrade (8.5.6a must-trash class).
pub fn region_upgrade(name: &'static str, rez: u32) -> PrintedCard {
    let mut c = vanilla_upgrade(name, rez);
    c.subtypes = vec!["region"];
    c
}

/// Dhegdheer shape: a program that can host 1 program, lowering its install
/// cost by 1 (8.5.1a eligible destination; the 8.5.5 example).
pub fn dhegdheer_like(name: &'static str, cost: u32) -> PrintedCard {
    let mut c = program_cost(name, cost);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::CanHost {
            criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Program)],
            capacity: Some(Quantity::c(1)),
        },
        StaticDecl::HostedInstallDiscount(Quantity::c(1)),
    ])
    .labeled("dhegdheer: hosts 1 program at -1c")];
    c
}

/// Mass-Install shape: install up to `count` programs from the grip, one at
/// a time (8.5.5), choosing hosts freely (8.5.16b).
pub fn mass_install_button(name: &'static str, count: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCards {
            count,
            from_hand_of: Side::Runner,
            filter: crate::instr::InstallFilter::Program,
            dest: crate::instr::InstallDest::RunnerChoiceHostOrRig,
            and_rez: false,
            and_rez_if_able: false,
            ignore_costs: false,
        }],
    )
    .labeled("mass-install: up to N programs")];
    c
}

/// A Corp button installing one fixed card to a fixed destination.
pub fn corp_install_button(
    name: &'static str,
    card: ObjectId,
    dest: crate::instr::InstallDest,
) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![card]),
            dest,
            and_rez: false,
            ignore_costs: false,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
    )
    .labeled("corp-install: fixed card")];
    c
}

/// A Corp button installing AND rezzing one fixed card (8.5.15), with an
/// optional 8.5.13c reveal requirement and 1.16.5c cost ignorance.
pub fn corp_install_rez_button(
    name: &'static str,
    card: ObjectId,
    dest: crate::instr::InstallDest,
    ignore_costs: bool,
    reveal_check: Option<crate::instr::RevealCheck>,
) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![card]),
            dest,
            and_rez: true,
            ignore_costs,
            reveal_check,
            reduce_total: Quantity::c(0),
        }],
    )
    .labeled("corp-install-rez: fixed card")];
    c
}

/// Brân shape: "[sub] Install a piece of ice from HQ directly inward from
/// this ice." (8.5.13a: no reveal; 8.5.14: invalid from Archives.)
pub fn bran_like(name: &'static str, installee: ObjectId) -> PrintedCard {
    let mut c = vanilla_ice(name, 6, 4);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::InstallCard {
        card: TargetSpec::Objects(vec![installee]),
        dest: crate::instr::InstallDest::InwardFromSource,
        and_rez: false,
        ignore_costs: true,
        reveal_check: None,
        reduce_total: Quantity::c(0),
    }])
    .labeled("[sub] install ice directly inward")];
    c
}

/// Ad-Blitz shape: "install and rez up to N pieces of ice, if able" —
/// unrezzable cards cannot be chosen (8.5.13d).
pub fn ad_blitz_button(name: &'static str, count: u32, server: ServerId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCards {
            count,
            from_hand_of: Side::Corp,
            filter: crate::instr::InstallFilter::Any,
            dest: crate::instr::InstallDest::Protecting(server),
            and_rez: true,
            and_rez_if_able: true,
            ignore_costs: true,
        }],
    )
    .labeled("ad-blitz: install and rez if able")];
    c
}

/// Nico-Campaign shape: an asset with "When your turn begins, gain 1[c]."
pub fn nico_like(name: &'static str, rez: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, rez, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Corp),
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("nico: gain 1 when turn begins")];
    c
}

/// Reaper-Function/Ob composite shape for the 9.6.5b example: "When your
/// turn begins, you may trash this card to install and rez <card>."
pub fn reaper_like(name: &'static str, installee: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Corp),
        vec![
            Instruction::TrashSelf,
            Instruction::InstallCard {
                card: TargetSpec::Objects(vec![installee]),
                dest: crate::instr::InstallDest::NewRemoteRoot,
                and_rez: true,
                ignore_costs: false,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
        ],
        true,
    )
    .labeled("reaper: trash to install and rez")];
    c
}

/// Tranquility-Home-Grid shape: an upgrade with "Whenever the Corp installs
/// a card in the root of this server, gain 1[c]." (9.6.5b activity gate.)
pub fn thg_like(name: &'static str, rez: u32) -> PrintedCard {
    let mut c = vanilla_upgrade(name, rez);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CardInstalledInSourceServer,
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("thg: gain 1 per install here")];
    c
}

/// Architect-Deployment-Test shape: "install and rez a card, ignoring all
/// costs." (1.16.5c/1.16.3a: the cost steps still happen, with checkpoints.)
pub fn adt_button(name: &'static str, installee: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::NewRemoteRoot,
            and_rez: true,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
    )
    .labeled("adt: install and rez ignoring costs")];
    c
}

/// Ganked/Drafter composite shape: a facedown root card with "When you
/// access this card, you may install a card from HQ in the root of the
/// server being breached." (drives 10.3.1j).
pub fn ganked_like(name: &'static str, installee: ObjectId) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::BreachedServerRoot,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
        true,
    )
    .labeled("ganked: install into breached server")];
    c
}

// ---------------------------------------------------------------------------
// §8.6 play shapes (W3b)
// ---------------------------------------------------------------------------

/// An operation with a play cost and given play-ability instructions.
pub fn operation(name: &'static str, cost: u32, instrs: Vec<Instruction>) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Operation);
    c.cost = Some(cost);
    if !instrs.is_empty() {
        c.abilities = vec![AbilityDef {
            kind: crate::ability::AbilityKind::Play,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional: false,
            timing: None,
            label: "play ability",
        }];
    }
    c
}

/// An event with a play cost and given play-ability instructions.
pub fn event(name: &'static str, cost: u32, instrs: Vec<Instruction>) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Runner, CardType::Event);
    c.cost = Some(cost);
    if !instrs.is_empty() {
        c.abilities = vec![AbilityDef {
            kind: crate::ability::AbilityKind::Play,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional: false,
            timing: None,
            label: "play ability",
        }];
    }
    c
}

/// Subcontract shape: play up to `count` operations from HQ, one at a time
/// (8.6.3).
pub fn subcontract_button(name: &'static str, count: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PlayCards {
            count,
            from_hand_of: Side::Corp,
            ignore_costs: false,
        }],
    )
    .labeled("subcontract: play operations")];
    c
}

/// A Runner button playing one fixed event.
pub fn play_event_button(name: &'static str, card: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PlayCard { card: TargetSpec::Objects(vec![card]), ignore_costs: false }],
    )
    .labeled("play-event: fixed card")];
    c
}

/// The 5.2.7a "[click]: Play an event" basic action, as a card ability: the
/// kernel has no basic install/play actions yet, so tests that need an event
/// played from an ACTION window (rather than a paid one) use this.
pub fn play_event_action(name: &'static str, card: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Cost::default() },
        vec![Instruction::PlayCard { card: TargetSpec::Objects(vec![card]), ignore_costs: false }],
    )
    .labeled("play-event-action: fixed card")];
    c
}

/// A Corp button playing one fixed operation.
pub fn play_operation_button(name: &'static str, card: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PlayCard { card: TargetSpec::Objects(vec![card]), ignore_costs: false }],
    )
    .labeled("play-op: fixed card")];
    c
}

/// Targeted-Marketing shape: an operation that is not trashed until the
/// Runner steals an agenda (8.6.6c).
pub fn targeted_marketing_like(name: &'static str) -> PrintedCard {
    let mut c = operation(name, 0, vec![]);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::PlayedNotTrashedUntilAgendaSteal,
    ])
    .labeled("tm: current-style trash shield")];
    c
}

/// Quantum-Predictive-Model shape (9.6.5c): TWO access conditionals on one
/// card — the QPM marker requires the Runner to be tagged AT ACCESS TIME
/// (part of the trigger condition), while the Casting-Call rider gives tags
/// on access.
pub fn qpm_with_casting_call(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(0);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfAccessed { requires: vec![TriggerRequirement::RunnerTagged] },
            vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
            false,
        )
        .labeled("qpm: if tagged when accessed"),
        AbilityDef::conditional(
            TriggerCond::SelfAccessed { requires: Vec::new() },
            vec![Instruction::GainTags(2)],
            false,
        )
        .labeled("casting-call: 2 tags on access"),
    ];
    c
}

/// Dyson-Mem-Chip shape: +1 link (static).
pub fn dyson_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::LinkBonus(1)])
        .labeled("dyson: +1 link")];
    c
}

/// The-Supplier shape (9.6.5d): "When your turn begins, you may install a
/// hosted card." Kernel form installs a fixed card, ignoring costs.
pub fn supplier_like(name: &'static str, installee: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Runner),
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::Rig,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
        true,
    )
    .labeled("supplier: install hosted card")];
    c
}

/// Underworld-Contact shape (9.6.5d): "When your turn begins, if you have at
/// least 2 link, gain 1[c]." — the link requirement is in the INSTRUCTIONS.
pub fn underworld_contact_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::TurnBegins(Side::Runner),
        vec![Instruction::IfRunnerLinkAtLeast {
            n: 2,
            then: Box::new(Instruction::GainCredits(Side::Runner, Quantity::c(1))),
        }],
        false,
    )
    .labeled("uc: gain 1 at 2+ link")];
    c
}

// ---------------------------------------------------------------------------
// W3d shapes: vacuous truth (9.12.2d) and run-ends conditions (6.8.5)
// ---------------------------------------------------------------------------

/// Troll shape for 9.12.2d: ice with ZERO subroutines and a "when
/// encountered, you may end the run" ability the Corp can decline.
pub fn troll_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 2, 4);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::DeclineableChoice(Box::new(Instruction::EndTheRun))],
        true,
    )
    .labeled("troll: may end the run on encounter")];
    c
}

/// Forked shape: run a server; if all subroutines on an encountered piece
/// of ice are broken during the encounter (vacuously for zero-sub ice —
/// 9.12.2d), trash that ice.
pub fn forked_button(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Event);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::CreateDelayedConditional {
                def: Box::new(
                    AbilityDef::conditional(
                        TriggerCond::AllSubsBrokenOnEncounteredIce,
                        vec![Instruction::TrashCards(TargetSpec::EncounteredIce)],
                        false,
                    )
                    .labeled("forked-delayed: trash fully-broken ice"),
                ),
                duration: crate::lingering::WantedDuration::UntilResolved,
            },
            Instruction::run(server),
        ],
    )
    .labeled("forked: run and trash fully-broken ice")];
    c
}

/// Dedicated-Response-Team shape (6.8.5 example 2): "Whenever a run ends,
/// do 2 meat damage." (mandatory).
pub fn drt_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunEnds { successful_only: false },
        vec![Instruction::Damage { kind: DamageKind::Meat, amount: Quantity::c(2), responsible: Side::Corp }],
        false,
    )
    .labeled("drt: 2 meat when the run ends")];
    c
}

/// Chum shape (9.6.13): a card whose paid ability arms the delayed
/// conditional "when this encounter ends, do 3 net damage" — a lingering
/// effect maintaining a conditional ability, one-shot (9.6.13c).
pub fn chum_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateDelayedConditional {
            def: Box::new(
                AbilityDef::conditional(
                    TriggerCond::EncounterEnds,
                    vec![Instruction::Damage {
                        kind: DamageKind::Net,
                        amount: Quantity::c(3),
                        responsible: Side::Corp,
                    }],
                    false,
                )
                .labeled("chum-delayed: 3 net when encounter ends"),
            ),
            duration: crate::lingering::WantedDuration::UntilResolved,
        }],
    )
    .labeled("chum: arm the encounter-end damage")];
    c
}

/// The-Noble-Path shape (6.8.5): a card whose paid ability creates the
/// lingering effect "prevent all damage for the remainder of this run".
/// Run-bound, so it expires at step 6.9.6d — which is exactly when
/// run-ends damage resolves (9.10.4 binds the duration to the run instance
/// in progress; used outside a run it expires at the next checkpoint).
pub fn noble_path_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::PreventAllDamage,
            duration: crate::lingering::WantedDuration::ThisRun,
        }],
    )
    .labeled("noble-path: prevent all damage this run")];
    c
}

// ---------------------------------------------------------------------------
// W3e shapes: candidates (7.4.3, 7.4.7a)
// ---------------------------------------------------------------------------

/// Immolation-Script shape (7.4.3): a card whose paid ability creates the
/// turn-bound replacement "instead of accessing the chosen card, trash
/// <victim>". The chosen candidate stays chosen whether or not it was
/// actually accessed.
pub fn access_replacement_card(name: &'static str, victim: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::Replacement {
                optional: false,
                applies_to: crate::effects::EffectClass::AccessCard,
                with: crate::lingering::ReplacementTransform::SuppressAccessAndTrashOther(victim),
            },
            duration: crate::lingering::WantedDuration::ThisTurn,
        }],
    )
    .labeled("immolation: trash instead of accessing")];
    c
}

/// The-Maker's-Eye shape (7.3.6): a card whose paid ability creates the
/// turn-bound "access N additional cards from <server>" lingering effect.
pub fn additional_access_card(name: &'static str, server: ServerId, extra: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::AdditionalAccess { server, extra },
            duration: crate::lingering::WantedDuration::ThisTurn,
        }],
    )
    .labeled("makers-eye: access more cards")];
    c
}

/// Gagarin shape: an additional 1[c] cost to access cards in remote roots.
pub fn gagarin_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::AdditionalAccessCost(
        Cost::credits(1),
    )])
    .labeled("gagarin: 1c to access remote cards")];
    c
}

/// Bacterial-Programming shape: an agenda; when stolen, the Corp rearranges
/// R&D (returned cards are new objects — 7.4.7a example 1).
pub fn bacterial_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_agenda(name, 3, 1);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerStealsAgenda,
        vec![Instruction::CorpRearrangesRnd],
        false,
    )
    .labeled("bacterial: rearrange R&D on steal")];
    c
}

/// Seidr-Laboratories shape: when the Runner steals an agenda, add a fixed
/// card from Archives to the top of R&D (7.4.7a example 2).
pub fn seidr_like(name: &'static str, card: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerStealsAgenda,
        vec![Instruction::MoveToDeck { card: TargetSpec::Objects(vec![card]), top: true }],
        false,
    )
    .labeled("seidr: add a card to the top of R&D")];
    c
}

/// Strongbox shape: [click] as an additional cost to steal.
pub fn strongbox_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(3);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::AdditionalStealCost(
        Cost { clicks: 1, ..Default::default() },
    )])
    .labeled("strongbox: click to steal")];
    c
}

// ---------------------------------------------------------------------------
// W3f shapes: 9.9.4c/d chains (No One Home) and 9.12.2e X-values (Surveyor)
// ---------------------------------------------------------------------------

/// A Corp button giving the Runner N tags at once.
pub fn corp_tags_button(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::GainTags(n)])
        .labeled("give tags")];
    c
}

/// Thunder-Art-Gallery shape: "Whenever you avoid a tag, you may install a
/// card." (kernel: installs a fixed card, ignoring costs).
pub fn gallery_like(name: &'static str, installee: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerAvoidsTag,
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::Rig,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
        true,
    )
    .labeled("gallery: install on tag avoidance")];
    c
}

/// No-One-Home shape: a CONDITIONAL interrupt "when you would take tags,
/// avoid 1" — it can only act if it was pending when the interrupt window
/// opened (9.9.4b/c).
pub fn noh_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Conditional,
        flags: vec![AbilityFlag::Interrupt],
        condition: Some(Condition::Trigger(TriggerCond::WouldTakeTags { during_run: false })),
        cost: None,
        instructions: vec![Instruction::AvoidTags(1)],
        statics: Vec::new(),
        optional: true,
        timing: None,
        label: "no-one-home: avoid 1 tag",
    }];
    c
}

/// Surveyor shape: ice whose strength is X (= 2 × ice protecting this
/// server), with a subroutine tracing X (9.12.2e).
pub fn surveyor_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Ice);
    c.cost = Some(4);
    c.strength = Some(0);
    let x = Quantity::Times(
        2,
        Box::new(Quantity::Count(crate::instr::TargetFilter::IceProtectingSourceServer)),
    );
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::SelfStrength(x.clone())])
            .labeled("surveyor: strength X"),
        AbilityDef::subroutine(vec![Instruction::Trace {
            base: Quantity::XOfSource(Box::new(x)),
            if_successful: vec![Instruction::GainTags(1)],
            if_unsuccessful: vec![],
            determined_min: None,
        }])
        .labeled("[sub] trace X"),
    ];
    c
}

// ---------------------------------------------------------------------------
// §9.10.3 — maintained choices
// ---------------------------------------------------------------------------

/// Pelangi shape (9.10.3a): a paid ability that chooses an ice subtype and
/// gives it to the encountered ice for the remainder of the encounter. The
/// choice BETWEEN subtypes is 9.11.4g's option choice, so each branch both
/// maintains its own choice and applies the matching modification.
pub fn pelangi_like(name: &'static str, subtypes: &[&'static str]) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    let options: Vec<(&'static str, Vec<Instruction>)> = subtypes
        .iter()
        .map(|t| {
            (
                *t,
                vec![
                    Instruction::MaintainChoice {
                        key: "pelangi-subtype",
                        of: crate::instr::ChoiceSpec::Subtype(t),
                        duration: crate::lingering::WantedDuration::ThisEncounter,
                    },
                    Instruction::ModifySubtypes {
                        target: TargetSpec::EncounteredIce,
                        add: vec![*t],
                        remove: vec![],
                        duration: crate::lingering::WantedDuration::ThisEncounter,
                    },
                ],
            )
        })
        .collect();
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::ChooseOne { options }],
    )
    .labeled("pelangi: give the encountered ice a chosen subtype")];
    c
}

/// Security Testing shape (9.10.3b): "When your turn begins, you may choose a
/// server. The first time each turn you make a successful run on that server,
/// instead of breaching it, gain 2[credit]." The first ability's ONLY effect is
/// making the choice, so 9.10.3b gives the choice a turn duration; the second
/// ability reads it back by key.
///
/// Simplification: the second ability gains credits rather than replacing the
/// breach — the replacement half is `sec_test_like`, and this shape is about
/// which server the trigger condition looks for.
pub fn security_testing_choice_like(name: &'static str, servers: &[ServerId]) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    let options: Vec<(&'static str, Vec<Instruction>)> = servers
        .iter()
        .map(|s| {
            let label: &'static str = Box::leak(format!("choose {s:?}").into_boxed_str());
            (
                label,
                vec![Instruction::MaintainChoice {
                    key: "sectest-server",
                    of: crate::instr::ChoiceSpec::Server(*s),
                    duration: crate::lingering::WantedDuration::ThisTurn,
                }],
            )
        })
        .collect();
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::TurnBegins(Side::Runner),
            vec![Instruction::ChooseOne { options }],
            true,
        )
        .labeled("sectest: you may choose a server"),
        AbilityDef::conditional(
            TriggerCond::SuccessfulRunOnChosenServer { key: "sectest-server" },
            vec![Instruction::GainCredits(Side::Runner, Quantity::c(2))],
            false,
        )
        .labeled("sectest: 2 credits on a successful run on the chosen server"),
    ];
    c
}

/// Femme Fatale shape (9.10.3c / 1.12.4): "When you install this program,
/// choose an installed piece of ice. You can bypass that ice." The choice is
/// maintained for as long as the source is active, and the bypass ability
/// reads it back through `TargetSpec::MaintainedChoice`.
///
/// Simplification: the conditional that makes the choice is triggered here by
/// the turn beginning rather than by installing, because the examples set the
/// board up rather than playing the install out; the DURATION under test
/// (9.10.3c, `WhileSourceActive`) is stated on the choice itself and is
/// unaffected.
pub fn femme_choice_like(name: &'static str) -> PrintedCard {
    femme_choice_over(
        name,
        vec![
            crate::instr::TargetFilter::CardTypeIs(CardType::Ice),
            crate::instr::TargetFilter::Rezzed,
        ],
    )
}

/// As [`femme_choice_like`], but the chosen ice need NOT be rezzed. CR
/// 10.2.3b's example chooses "a piece of ice protecting HQ", which is normally
/// an unrezzed card: its own identity stays hidden information (10.2.2a) while
/// the CHOICE is open information that "cannot be hidden from an opponent".
pub fn femme_choice_any_ice_like(name: &'static str) -> PrintedCard {
    femme_choice_over(name, vec![crate::instr::TargetFilter::CardTypeIs(CardType::Ice)])
}

fn femme_choice_over(
    name: &'static str,
    criteria: Vec<crate::instr::TargetFilter>,
) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::TurnBegins(Side::Runner),
            vec![Instruction::MaintainChoice {
                key: "femme-ice",
                of: crate::instr::ChoiceSpec::Object(TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria,
                }),
                duration: crate::lingering::WantedDuration::WhileSourceActive,
            }],
            false,
        )
        .labeled("femme: choose an installed piece of ice"),
        AbilityDef::paid(
            Cost::credits(1),
            vec![Instruction::TrashCards(TargetSpec::MaintainedChoice("femme-ice"))],
        )
        .labeled("femme: act on the remembered ice"),
    ];
    c
}

/// Poêtrï-Luxury-Brands shape (7.3.1a): a Corp card with "Whenever the Runner
/// accesses a card, install a card from R&D in a new remote server."
///
/// Simplification: the printed card installs from HQ and the install is
/// optional; here it installs from the top `n` of R&D so the example's
/// contrast — a card the Runner has accessed versus one they have not — can
/// be arranged without depending on which random HQ card the breach presents.
/// The rule under test (7.3.1a's visibility) is indifferent to the zone.
pub fn poetri_like(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerAccessesCard,
        vec![Instruction::InstallCard {
            card: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::TopOfDeckOf { side: Side::Corp, n }],
            },
            dest: crate::instr::InstallDest::NewRemoteRoot,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
        false,
    )
    .labeled("poetri: install a card from R&D")];
    c
}

// ---------------------------------------------------------------------------
// §9.12.3 — "must"
// ---------------------------------------------------------------------------

/// Mumbad Virtual Tour shape (9.12.3a): an asset whose "when accessed" ability
/// says the Runner must trash it if able, WITHOUT stipulating how.
pub fn must_trash_accessed_like(name: &'static str, trash_cost: u32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(trash_cost);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::MustTrashAccessedCard {
            means: crate::instr::TrashMeans::AnyAbility,
        }],
        false,
    )
    .labeled("mvt: the runner must trash this card if able")];
    c
}

/// Neutralize All Threats shape (9.12.3b): a Runner card whose ability says the
/// Runner must trash the card they access if they can PAY ITS TRASH COST — a
/// stipulated means, so no other ability can be forced.
///
/// Simplification: the printed card's "the first time each turn" restriction is
/// left off; the examples using this shape access exactly one card.
pub fn must_trash_by_paying_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerAccessesCard,
        vec![Instruction::MustTrashAccessedCard {
            means: crate::instr::TrashMeans::PayingTheTrashCost,
        }],
        false,
    )
    .labeled("nat: must trash if the trash cost can be paid")];
    c
}

/// Imp shape (9.3.6b / 1.9.2): an access-flagged paid ability costing a hosted
/// virus counter that trashes the accessed card at no further cost.
pub fn imp_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.subtypes = vec!["virus"];
    c.abilities = vec![AbilityDef::paid(
        Cost::spend_counters(CounterKind::Virus, 1),
        vec![Instruction::TrashCards(TargetSpec::AccessedCard)],
    )
    .with_flag(AbilityFlag::Access)
    .labeled("imp: spend a virus counter to trash the accessed card")];
    c
}

/// Scrubber shape (1.10.3c): a Runner card hosting credits its controller may
/// spend. Simplification: the printed card restricts them to trashing Corp
/// cards; the kernel's `hosted_credits_spendable` carries no restriction, and
/// the examples using this shape spend them on exactly that.
pub fn scrubber_like(name: &'static str, recurring: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.recurring_credits = Some(recurring);
    c.hosted_credits_spendable = true;
    c
}

/// Wendigo shape (9.5.3a): ice with a subroutine prohibiting the Runner from
/// using a named installed card's abilities for the remainder of the run.
pub fn use_prohibition_ice(name: &'static str, target: ObjectId) -> PrintedCard {
    let mut c = vanilla_ice(name, 3, 3);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::CreateLingeringEffect {
        payload: crate::instr::LingeringSpec::CannotUseAbilitiesOf(TargetSpec::Objects(vec![
            target,
        ])),
        duration: crate::lingering::WantedDuration::ThisRun,
    }])
    .labeled("[sub] the runner cannot use that card this run")];
    c
}

/// Warroid Tracker shape (4.6.6i): an asset whose ability meets its condition
/// when the Runner trashes at least 1 card in THIS SERVER — including the
/// asset itself — and whose effect reads "this server" a second time, by
/// gaining 1 credit for each piece of ice protecting it.
///
/// Simplification: the printed card traces and trashes installed cards; the
/// examples using this shape are about which server "this server" names, so
/// the effect is the smallest observable reading of it.
pub fn warroid_this_server_like(name: &'static str, trash_cost: u32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(trash_cost);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerTrashesAtLeastOneCorpCard { in_this_server: true },
        vec![Instruction::GainCredits(
            Side::Corp,
            Quantity::Count(crate::instr::TargetFilter::IceProtectingSourceServer),
        )],
        false,
    )
    .labeled("warroid: credits per ice protecting this server")];
    c
}

/// Border Control shape (4.6.6i): a piece of ice with a [trash] ability whose
/// effect gains 1 credit for each piece of ice protecting "this server".
///
/// Simplification: the CR's example delivers this effect through ZATO City
/// Grid's granted "trash that ice and resolve 1 of its subroutines", which the
/// kernel has no instruction for yet. Border Control's own "[trash]: End the
/// run" is the same branch of 4.6.6i — an ability *initiated by a cost*
/// involving its source moving out of the server it was protecting — so the
/// shape carries the counting effect on that ability instead.
pub fn border_control_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 2, 3);
    let count_this_server = Instruction::GainCredits(
        Side::Corp,
        Quantity::Count(crate::instr::TargetFilter::IceProtectingSourceServer),
    );
    c.abilities = vec![
        AbilityDef::subroutine(vec![count_this_server.clone()])
            .labeled("[sub] credits per ice protecting this server"),
        AbilityDef::paid(Cost::trash_self(), vec![count_this_server])
            .labeled("border control: trash for credits per ice protecting this server"),
    ];
    c
}

/// Earth Station shape (4.6.8f): a card declaring a limit on the number of
/// remote servers that may exist.
pub fn remote_limit_like(name: &'static str, limit: u32) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities =
        vec![AbilityDef::static_ability(vec![StaticDecl::RemoteServerLimit(limit)])
            .labeled("earth station: limit remote servers")];
    c
}

/// Hush shape (9.12.1d/e): a program installed on a piece of ice whose static
/// ability removes all of its HOST's abilities.
pub fn hush_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Runner, CardType::Program);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::RemoveAbilitiesOf(
        crate::ability::HostRelation::Host,
    )])
    .labeled("hush: host loses all abilities")];
    c
}

/// Magnet shape (9.12.1e): ice whose static ability removes all abilities from
/// the cards HOSTED on it — the opposite direction of the Hush relation, which
/// is what makes the two form a dependency loop.
pub fn magnet_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 3, 3);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::RemoveAbilitiesOf(
        crate::ability::HostRelation::Hosted,
    )])
    .labeled("magnet: hosted cards lose all abilities")];
    c
}

/// Mother Goddess shape (9.12.1d): ice with a static ability granting itself
/// the subtypes of every OTHER rezzed piece of ice.
pub fn mother_goddess_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 2, 4);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::GainSubtypesOf {
        criteria: vec![
            crate::instr::TargetFilter::Rezzed,
            crate::instr::TargetFilter::CardTypeIs(CardType::Ice),
            crate::instr::TargetFilter::OtherThanSource,
        ],
    }])
    .labeled("mother goddess: gains the subtypes of other rezzed ice")];
    c
}

/// Warden Fatuma shape (9.8.3a): ice whose static ability gives every OTHER
/// rezzed piece of ice with a named subtype an extra subroutine, before that
/// ice's other subroutines. The granting ability is not on the ice that gains
/// the subroutine, so the grant is external (category a), not self-static.
///
/// Simplification: the granted subroutine is the fixed `sub` passed in — the
/// examples using this shape do not vary it.
pub fn warden_fatuma_like(
    name: &'static str,
    subtype: &'static str,
    sub: AbilityDef,
) -> PrintedCard {
    let mut c = vanilla_ice(name, 6, 8);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::GrantSubroutinesTo {
        criteria: vec![
            crate::instr::TargetFilter::Rezzed,
            crate::instr::TargetFilter::CardTypeIs(CardType::Ice),
            crate::instr::TargetFilter::HasSubtype(subtype),
            crate::instr::TargetFilter::OtherThanSource,
        ],
        sub: Box::new(sub),
        before: true,
    }])
    .labeled("warden fatuma: other bioroid ice gain a subroutine first")];
    c
}

// ---------------------------------------------------------------------------
// §8.7 — searching, finding, shuffling
// ---------------------------------------------------------------------------

/// A program with the "virus" subtype (Datasucker / Imp shape) — search
/// criteria fodder for 8.7.2a.
pub fn virus_program(name: &'static str, cost: u32) -> PrintedCard {
    let mut c = program_cost(name, cost);
    c.subtypes = vec!["virus"];
    c
}

/// Artist-Colony shape (8.7.2b example 1): "Search your stack for a card and
/// install it." Written as the 9.11.4d split — the search ends one
/// instruction, the install is the next — so the found card is addressed by
/// [`TargetSpec::FoundBySearch`].
///
/// SIMPLIFICATION: the printed trigger cost ([click], forfeit an agenda) is
/// elided to a free cost; forfeiting is not in the kernel vocabulary and the
/// cost is orthogonal to what 8.7.2b decides.
pub fn artist_colony_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::Search {
                zone: Zone::Deck(Side::Runner),
                // "for a card": no criteria at all — 8.7.2b alone decides.
                criteria: Vec::new(),
                count: Quantity::c(1),
                may_fail: true,
            },
            Instruction::InstallCard {
                card: TargetSpec::FoundBySearch,
                dest: crate::instr::InstallDest::Rig,
                and_rez: false,
                ignore_costs: false,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
        ],
    )
    .labeled("artist-colony: search and install")];
    c
}

/// Self-modifying-Code shape (8.7.2b example 2): "[trash], 2[c]: Search your
/// stack for a program and install it. Shuffle your stack." The shuffle is
/// the search's own (8.7.3), not a separate instruction.
pub fn smc_like(name: &'static str) -> PrintedCard {
    let mut c = program_cost(name, 0);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self().plus(&Cost::credits(2)),
        vec![
            Instruction::Search {
                zone: Zone::Deck(Side::Runner),
                criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Program)],
                count: Quantity::c(1),
                may_fail: true,
            },
            Instruction::InstallCard {
                card: TargetSpec::FoundBySearch,
                dest: crate::instr::InstallDest::Rig,
                and_rez: false,
                ignore_costs: false,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
        ],
    )
    .labeled("smc: search a program and install it")];
    c
}

/// Patchwork shape (1.16.6 / 8.7.2b example 2): "You may trash 1 card from
/// your grip to lower the install cost of a card you are installing by 2."
///
/// SIMPLIFICATION: the printed once-per-turn limit is elided; no example
/// installs twice through it.
pub fn patchwork_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::InstallDiscount {
        cost: Cost::trash_from_hand(1),
        amount: 2,
    }])
    .labeled("patchwork: -2 install cost for a grip card")];
    c
}

/// Tucana shape (8.7.2b example 3): "Search R&D for a piece of ice, install
/// and rez it protecting <server>." A card that can be installed but not
/// rezzed is still a valid find; 8.5.13d then makes the Corp reveal it.
pub fn tucana_like(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::Search {
                zone: Zone::Deck(Side::Corp),
                criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Ice)],
                count: Quantity::c(1),
                may_fail: true,
            },
            Instruction::InstallCard {
                card: TargetSpec::FoundBySearch,
                dest: crate::instr::InstallDest::Protecting(server),
                and_rez: true,
                ignore_costs: false,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
        ],
    )
    .labeled("tucana: search ice, install and rez")];
    c
}

/// Tech-Startup shape (8.7.3 example): "[trash]: Search R&D for an asset and
/// install it." R&D is reshuffled the moment the search completes, before
/// the install resolves.
pub fn tech_startup_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::Search {
                zone: Zone::Deck(Side::Corp),
                criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Asset)],
                count: Quantity::c(1),
                may_fail: true,
            },
            Instruction::InstallCard {
                card: TargetSpec::FoundBySearch,
                dest: crate::instr::InstallDest::NewRemoteRoot,
                and_rez: false,
                ignore_costs: false,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
        ],
    )
    .labeled("tech-startup: search an asset and install it")];
    c
}

/// Near-Earth-Hub shape (the identity of the 8.7.3 example): "Whenever you
/// install a card, draw 1 card."
///
/// SIMPLIFICATION: the printed "the first time each turn" limit is elided;
/// the example installs once.
pub fn near_earth_hub_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CardInstalledBy(Side::Corp),
        vec![Instruction::Draw(Side::Corp, 1)],
        false,
    )
    .labeled("neh: draw 1 when you install")];
    c
}

/// Djinn shape (9.11.4d example): "Search your stack for a virus program and
/// add it to your grip. Shuffle your stack." — one printed sentence, split at
/// the search into two instructions per 9.11.4d.
///
/// SIMPLIFICATION: the printed [click] component of the trigger cost is
/// elided so the ability is offered in paid windows; the cost is orthogonal.
pub fn djinn_like(name: &'static str) -> PrintedCard {
    let mut c = program_cost(name, 0);
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![
            Instruction::Search {
                zone: Zone::Deck(Side::Runner),
                criteria: vec![
                    crate::instr::TargetFilter::CardTypeIs(CardType::Program),
                    crate::instr::TargetFilter::HasSubtype("virus"),
                ],
                count: Quantity::c(1),
                may_fail: true,
            },
            Instruction::AddCardsToHand { cards: TargetSpec::FoundBySearch },
        ],
    )
    .labeled("djinn: search a virus program")];
    c
}

/// Personality-Profiles shape (8.7.5 / 9.11.4d example): an agenda whose
/// ability reads "Whenever the Runner searches their stack, they trash 1
/// random card from their grip." The point is WHEN it pends — after the
/// search completes and the stack is shuffled, before the found card is
/// acted on.
pub fn personality_profiles_like(name: &'static str, points: i32) -> PrintedCard {
    let mut c = vanilla_agenda(name, 3, points);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerSearchesDeck(Side::Runner),
        vec![Instruction::TrashRandomFromHand {
            side: Side::Runner,
            count: Quantity::c(1),
        }],
        false,
    )
    .labeled("personality-profiles: random grip trash on search")];
    c
}

/// Put an identity into a player's play area, faceup and active (1.6.2).
// ---------------------------------------------------------------------------
// §1.13 — host, hosted, and hosting
// ---------------------------------------------------------------------------

/// The 1.13.6a class: a card whose ONLY hosting text is "this card can host
/// <criteria>, up to <capacity>" — which is exactly what makes it an
/// eligible installation destination. Off-Campus Apartment (any number of
/// *connections*) and Glenn Station minus its paid ability are both this.
pub fn can_host_card(
    name: &'static str,
    side: Side,
    ty: CardType,
    criteria: Vec<crate::instr::TargetFilter>,
    capacity: Option<u32>,
    label: &'static str,
) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, side, ty);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::CanHost {
        criteria,
        capacity: capacity.map(|n| Quantity::c(n as i64)),
    }])
    .labeled(label)];
    c
}

/// Off-Campus-Apartment shape (1.13.6a): "This card can host any number of
/// *connections*." No ability that hosts onto itself. (Its real "when you
/// host a card, draw" ability is orthogonal to every example here and is
/// elided.)
pub fn off_campus_like(name: &'static str) -> PrintedCard {
    can_host_card(
        name,
        Side::Runner,
        CardType::Resource,
        vec![crate::instr::TargetFilter::HasSubtype("connection")],
        None,
        "off-campus: hosts any number of connections",
    )
}

/// Glenn-Station shape (1.13.6b): "This card can host a single card." plus a
/// paid ability that hosts a card from HQ onto itself — so it hosts ONLY
/// through that ability.
pub fn glenn_station_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::CanHost {
            criteria: Vec::new(),
            capacity: Some(Quantity::c(1)),
        }])
        .labeled("glenn-station: hosts a single card"),
        AbilityDef::paid(
            Cost { clicks: 1, ..Cost::default() },
            vec![Instruction::HostCards {
                cards: TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![crate::instr::TargetFilter::CardsInHandOf(Side::Corp)],
                },
                host: TargetSpec::SelfSource,
            }],
        )
        .labeled("glenn-station: host a card from HQ"),
    ];
    c
}

/// Egret shape (1.13.6c): "Install only on a rezzed piece of ice." The
/// restriction is the whole card here; Egret's own subtype grant is elided.
pub fn egret_like(name: &'static str) -> PrintedCard {
    let mut c = program_cost(name, 1);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::InstallOnlyHostedOn(vec![
        crate::instr::TargetFilter::CardTypeIs(CardType::Ice),
        crate::instr::TargetFilter::Rezzed,
    ])])
    .labeled("egret: install only on a rezzed piece of ice")];
    c
}

/// Leprechaun shape (1.13.9): "This card can host up to 2 programs." — no
/// install discount of its own, which is the point of the transitivity
/// example.
pub fn leprechaun_like(name: &'static str, cost: u32) -> PrintedCard {
    let mut c = can_host_card(
        name,
        Side::Runner,
        CardType::Program,
        vec![crate::instr::TargetFilter::CardTypeIs(CardType::Program)],
        Some(2),
        "leprechaun: hosts 2 programs",
    );
    c.cost = Some(cost);
    c
}

/// Madani shape (1.13.12): "[click]: Host up to N programs from your grip on
/// this card." Hosting without installing (1.13.2a), so the hosted cards
/// move to this card's zone but do not become installed or active.
pub fn madani_like(name: &'static str, count: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Cost::default() },
        vec![Instruction::HostCards {
            cards: TargetSpec::Choose {
                count: Quantity::c(count as i64),
                criteria: vec![
                    crate::instr::TargetFilter::CardsInHandOf(Side::Runner),
                    crate::instr::TargetFilter::CardTypeIs(CardType::Program),
                ],
            },
            host: TargetSpec::SelfSource,
        }],
    )
    .labeled("madani: host programs from the grip")];
    c
}

/// Detente shape (1.13.13 example 1): an installed Runner program that hosts
/// Corp cards. Which Corp cards it can take, and how they get there, is not
/// what the example turns on — the shape hosts an installed Corp card, which
/// also exercises 1.13.2b (the Corp card becomes uninstalled).
pub fn detente_like(name: &'static str) -> PrintedCard {
    let mut c = program_cost(name, 0);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Cost::default() },
        vec![Instruction::HostCards {
            cards: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::InstalledCorpCard],
            },
            host: TargetSpec::SelfSource,
        }],
    )
    .labeled("detente: host an installed Corp card")];
    c
}

/// Rejig shape: "Add 1 installed program to your grip." (Its second
/// sentence — installing a program at a discount — is elided.)
pub fn rejig_like(name: &'static str) -> PrintedCard {
    event(
        name,
        0,
        vec![Instruction::AddCardsToHand {
            cards: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![
                    crate::instr::TargetFilter::InstalledRunnerCard,
                    crate::instr::TargetFilter::CardTypeIs(CardType::Program),
                ],
            },
        }],
    )
}

/// IP-Enforcement shape: "Install an agenda from the Runner's score area."
pub fn ip_enforcement_like(name: &'static str) -> PrintedCard {
    operation(
        name,
        0,
        vec![Instruction::InstallCard {
            card: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![
                    crate::instr::TargetFilter::CardTypeIs(CardType::Agenda),
                    crate::instr::TargetFilter::InScoreAreaOf(Side::Runner),
                ],
            },
            dest: crate::instr::InstallDest::NewRemoteRoot,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
    )
}

/// Exchange-of-Information shape (8.8.4c): "Swap 1 agenda in the Runner's
/// score area with 1 agenda in your score area." SIMPLIFICATION: both sides
/// of the swap are fixed at card-build time — the example turns on what
/// happens to the counters hosted on the swapped agendas, not on which
/// agendas are chosen.
pub fn exchange_of_information_like(
    name: &'static str,
    theirs: ObjectId,
    ours: ObjectId,
) -> PrintedCard {
    operation(
        name,
        0,
        vec![Instruction::SwapCards {
            a: TargetSpec::Objects(vec![theirs]),
            b: TargetSpec::Objects(vec![ours]),
        }],
    )
}

/// Whitespace shape (1.13.3 example 1): "[sub] The Runner loses 3[credit]."
pub fn whitespace_like(name: &'static str, amount: u32) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 0);
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::LoseCredits(Side::Runner, amount)])
            .labeled("[sub] the Runner loses credits"),
    ];
    c
}

/// Scapegoat shape (1.13.3 example 3): "Remove 2 bad publicity."
pub fn remove_bad_pub_operation(name: &'static str, n: i64) -> PrintedCard {
    operation(
        name,
        0,
        vec![Instruction::RemoveCountersFromPlayer {
            side: Side::Corp,
            kind: CounterKind::BadPublicity,
            amount: Quantity::c(n),
        }],
    )
}

/// Cyberfeeder/Fencer-Fueno shape (1.10.3c): a card whose ability lets its
/// controller spend the credits hosted on it. SIMPLIFICATION: the real
/// Cyberfeeder restricts what those credits may pay for; here they are
/// spendable for any cost, which is what 9.1.6c's example needs.
pub fn hosted_credit_source(name: &'static str, ty: CardType) -> PrintedCard {
    let mut c = vanilla_runner_card(name, ty);
    c.hosted_credits_spendable = true;
    c
}

/// Mimic shape (9.1.6c): a program with a paid ability whose trigger cost is
/// 1[credit]. Mimic's break ability needs an encounter to be usable; the
/// example is about who counts as USED when the cost is paid, so the shape
/// uses the strength pump every icebreaker of its class carries.
pub fn credit_cost_program(name: &'static str) -> PrintedCard {
    let mut c = program_cost(name, 0);
    c.strength = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![Instruction::ModifyStrength {
            target: TargetSpec::SelfSource,
            amount: 1,
            duration: None,
        }],
    )
    .labeled("mimic: 1c pump")];
    c
}

/// A Runner button that installs one card from the grip, one at a time
/// (8.5.5), with the destination left to the 8.5.16b declaration — which is
/// where the 1.13.6a host choice is offered.
pub fn runner_install_button(name: &'static str, count: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCards {
            count,
            from_hand_of: Side::Runner,
            filter: crate::instr::InstallFilter::Any,
            dest: crate::instr::InstallDest::RunnerChoiceHostOrRig,
            and_rez: false,
            and_rez_if_able: false,
            ignore_costs: false,
        }],
    )
    .labeled("install-button: install from the grip")];
    c
}

/// A Corp button that installs one card from HQ into the root of a server,
/// one at a time (8.5.5).
pub fn corp_install_from_hq_button(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCards {
            count: 1,
            from_hand_of: Side::Corp,
            filter: crate::instr::InstallFilter::Any,
            dest: crate::instr::InstallDest::Root(server),
            and_rez: false,
            and_rez_if_able: false,
            ignore_costs: false,
        }],
    )
    .labeled("corp-install-button: install from HQ")];
    c
}

pub fn install_identity(vm: &mut Vm, card: PrintedCard, side: Side) -> ObjectId {
    let id = vm.new_object(card, Zone::PlayArea(side));
    vm.st.active_seq += 1;
    let seq = vm.st.active_seq;
    let o = vm.st.objects.get_mut(&id).unwrap();
    o.faceup = true;
    o.active_since = seq;
    id
}

/// Put a card into a player's score area (4.5) — how the CR's Personality
/// Profiles example has it on the board.
pub fn put_in_score_area(vm: &mut Vm, card: PrintedCard, side: Side) -> ObjectId {
    let id = vm.new_object(card, Zone::ScoreArea(side));
    vm.st.score_area.get_mut(&side).unwrap().push(id);
    vm.st.active_seq += 1;
    let seq = vm.st.active_seq;
    let o = vm.st.objects.get_mut(&id).unwrap();
    o.faceup = true;
    o.active_since = seq;
    id
}

// ---------------------------------------------------------------------------
// Script drivers: RETIRED.
//
// The hand-rolled `until_decision` / `drain_to_game_over` / `take_labeled` /
// `option_labeled` scaffold is gone (ARCHITECTURE §12 rule 5, FT-0). Tests
// declare setup, one `plan::Plan` per player, and assertions; the ONE shared
// driver is `plan::Script`, and the neutral policy that was `default_answer`
// now lives in `plan` as the meaning of `plan::Reply::Default`. Nothing in
// this module drives the VM any more: testkit builds cards, plan drives them.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// W6a shapes: credits, recurring credits, threat, memory, damage attribution
// ---------------------------------------------------------------------------

/// Fall-Guy shape (1.19.4): "[trash]: Gain 2[credit]." The [trash] symbol IS
/// the trigger cost — the whole point of the example.
pub fn fall_guy_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(2))],
    )
    .labeled("fall-guy: [trash] gain 2")];
    c
}

/// T400-Memory-Diamond shape (1.20.2): a static "+1[mu]".
pub fn mem_chip_like(name: &'static str, plus: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::MemoryLimitMod(plus)]).labeled("+mu"),
    ];
    c
}

/// Spinal-Modem shape (1.10.5): "N[recurring]" — credits are placed when the
/// card becomes active and topped back up to N as the turn begins, and the
/// card's own text is what lets its controller spend them (1.10.3c).
pub fn recurring_card(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.recurring_credits = Some(n);
    c.hosted_credits_spendable = true;
    c
}

/// A Corp asset carrying a "threat N"-flagged free paid ability (9.3.6f).
pub fn threat_button(name: &'static str, n: u8, label: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
    )
    .with_flag(AbilityFlag::Threat(n))
    .labeled(label)];
    c
}

/// Argus-Security shape (10.4.1): "Whenever the Runner steals an agenda, they
/// take 1 tag or suffer 2 meat damage." The suffered branch names the RUNNER
/// as responsible, which is what The Cleaners' "damage done by the Corp"
/// bonus does not reach.
pub fn argus_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunnerStealsAgenda,
        // 1.14.5: the text names the Runner as taking the tag or suffering
        // the damage, so the RUNNER chooses, not the ability's controller.
        vec![Instruction::PerformedBy {
            side: Side::Runner,
            instr: Box::new(Instruction::ChooseOne {
                options: vec![
                    ("take 1 tag", vec![Instruction::GainTags(1)]),
                    (
                        "suffer 2 meat damage",
                        vec![Instruction::Damage {
                            kind: DamageKind::Meat,
                            amount: Quantity::c(2),
                            responsible: Side::Runner,
                        }],
                    ),
                ],
            }),
        }],
        false,
    )
    .labeled("argus: tag or suffer 2 meat")];
    c
}

/// Tollbooth shape (1.10.3c example 3): "When the Runner encounters this ice,
/// they must pay N[credit] or the run ends."
pub fn toll_ice(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_ice(name, 8, 5);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::NestedCostUnless {
            cost: Cost::credits(n),
            effect: Box::new(Instruction::EndTheRun),
            payer: Some(Side::Runner),
        }],
        false,
    )
    .labeled("tollbooth: pay or the run ends")];
    c
}

/// Rototurret/Bulwark shape (1.14.5): a piece of ice whose subroutine reads
/// "Trash 1 program." — or, when `by` is set, "The <player> trashes 1
/// program.", which is the only difference between the two cards as far as
/// the choice is concerned.
///
/// The criteria say nothing about a zone on purpose: 1.15.2c is what
/// restricts the announcement to installed programs, which is exactly the
/// `targets_must_be_in_play_area` example's claim.
pub fn trash_program_sub_ice(name: &'static str, by: Option<Side>) -> PrintedCard {
    let mut c = vanilla_ice(name, 4, 4);
    let trash = Instruction::TrashCards(TargetSpec::Choose {
        count: Quantity::c(1),
        criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Program)],
    });
    let instr = match by {
        None => trash,
        Some(side) => Instruction::PerformedBy { side, instr: Box::new(trash) },
    };
    c.abilities = vec![AbilityDef::subroutine(vec![instr]).labeled("[sub] trash 1 program")];
    c
}

/// Alice-Merchant shape (1.14.5a): a RUNNER card whose ability states that
/// "the Corp must trash 1 card from HQ" — the Corp carries the trashing out,
/// so conditions about the Runner trashing Corp cards are not met.
pub fn alice_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::PerformedBy {
            side: Side::Corp,
            instr: Box::new(Instruction::TrashCards(TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::CardsInHandOf(Side::Corp)],
            })),
        }],
    )
    .labeled("alice: the Corp trashes 1 card from HQ")];
    c
}

// ---------------------------------------------------------------------------
// W6b shapes: strengths and durations (§3.9.5, §9.10)
// ---------------------------------------------------------------------------

/// Corroder shape (3.9.5b): an icebreaker whose paid ability states NO
/// duration — "1[credit]: +1 strength."
pub fn implicit_pump_breaker(name: &'static str, base: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(base);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![Instruction::ModifyStrength {
            target: TargetSpec::SelfSource,
            amount: 1,
            duration: None,
        }],
    )
    .labeled("corroder: +1 strength")];
    c
}

/// Gordian-Blade shape (3.9.5c): an icebreaker whose paid ability STATES a
/// duration — "1[credit]: +1 strength for the remainder of this run."
pub fn run_pump_breaker(name: &'static str, base: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(base);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![Instruction::ModifyStrength {
            target: TargetSpec::SelfSource,
            amount: 1,
            duration: Some(crate::lingering::WantedDuration::ThisRun),
        }],
    )
    .labeled("gordian: +1 strength for the run")];
    c
}

/// Na'Not'K shape (9.10.5): an icebreaker whose STATIC ability sets its
/// strength to the number of ice protecting the attacked server — so the
/// static contribution lapses the moment the run ends — plus a paid pump
/// with no stated duration.
pub fn attacked_server_breaker(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(0);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::SelfStrength(Quantity::Count(
            crate::instr::TargetFilter::IceProtectingAttackedServer,
        ))])
        .labeled("nanotk-static"),
        AbilityDef::paid(
            Cost::credits(1),
            vec![Instruction::ModifyStrength {
                target: TargetSpec::SelfSource,
                amount: 2,
                duration: None,
            }],
        )
        .labeled("nanotk: +2 strength"),
    ];
    c
}

/// Puffer shape (9.4.4): an icebreaker whose static ability computes its
/// strength from its hosted virus counters — no duration, no lingering
/// effect — alongside a paid pump that DOES create one.
pub fn counter_strength_breaker(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(0);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::SelfStrength(
            Quantity::base_plus_per_counter(1, 1, CounterKind::Virus),
        )])
        .labeled("puffer-static"),
        AbilityDef::paid(
            Cost::credits(1),
            vec![Instruction::ModifyStrength {
                target: TargetSpec::SelfSource,
                amount: 1,
                duration: None,
            }],
        )
        .labeled("puffer: +1 strength"),
    ];
    c
}

/// Gebrselassie shape (9.10.5 / 9.9.9a): a program hosted on an icebreaker
/// whose static ability replaces the durations of lingering effects
/// modifying its HOST's strength with "for the remainder of the turn".
pub fn duration_extender(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(0);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::ExtendStrengthDurations {
        target_host: true,
        until: crate::lingering::WantedDuration::ThisTurn,
    }])
    .labeled("gebrselassie: host strength effects last the turn")];
    c
}

/// Esâ-Afontov / Chastushka shape (10.12.1): a Runner ability that
/// sabotages N.
pub fn sabotage_button(name: &'static str, n: i64) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![
        AbilityDef::paid(Cost::free(), vec![Instruction::Sabotage { count: Quantity::c(n) }])
            .labeled("sabotage"),
    ];
    c
}

/// Abagnale shape (9.5.6c / 9.3.6c): an icebreaker with a [trash] ability
/// usable during an encounter with a CODE GATE whatever its strength, and an
/// [interface] break ability usable only when the breaker's strength reaches
/// the encountered ice's.
pub fn abagnale_like(name: &'static str, strength: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(strength);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker"];
    c.abilities = vec![
        AbilityDef::paid(Cost::trash_self(), vec![Instruction::BypassEncounteredIce])
            .with_timing(TimingRestriction::EncounterOnly { required_subtype: Some("code gate") })
            .labeled("abagnale: [trash] bypass this code gate"),
        AbilityDef::paid(Cost::credits(1), vec![Instruction::BreakSubroutines {
            subs: crate::instr::SubroutineSpec::Chosen { count: Quantity::c(1), up_to: false },
        }])
            .with_flag(AbilityFlag::Interface)
            .with_timing(TimingRestriction::EncounterOnly { required_subtype: Some("code gate") })
            .labeled("abagnale: interface break 1"),
    ];
    c
}

/// A piece of ice with a subtype and one ETR subroutine.
pub fn subtyped_etr_ice(
    name: &'static str,
    subtype: &'static str,
    cost: u32,
    strength: i32,
) -> PrintedCard {
    let mut c = etr_ice(name, cost, strength);
    c.subtypes = vec![subtype];
    c
}

// ---------------------------------------------------------------------------
// W7a shapes: target announcements (§1.15.2)
// ---------------------------------------------------------------------------

/// Aggressive-Secretary shape (1.15.2e): "When you access this card, you may
/// pay 2[c]. If you do, trash X programs, where X is the number of
/// advancement counters on this card."
///
/// The count is a quantity selector, not a number, and the criteria name no
/// zone — 1.15.2c restricts the announcement to installed programs and
/// 1.15.2e caps it at however many there are.
pub fn aggressive_secretary_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Asset);
    c.trash_cost = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::NestedCostThen {
            cost: Cost::credits(2),
            effect: Box::new(Instruction::TrashCards(TargetSpec::Choose {
                count: Quantity::CountersOnSource(CounterKind::Advancement),
                criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Program)],
            })),
            payer: Some(Side::Corp),
        }],
        true,
    )
    .labeled("secretary: pay 2 to trash X programs")];
    c
}

// ---------------------------------------------------------------------------
// W7b shapes: several announcements per instruction, subroutine targets
// ---------------------------------------------------------------------------

/// Colossus shape (1.15.1): a piece of ice with the subroutine "Trash 1
/// installed program and 1 installed resource." — ONE instruction requiring
/// TWO announcements (1.15.2), acting on both targets at once.
pub fn colossus_like(name: &'static str) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_ice(name, 6, 6);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::TrashCards(TargetSpec::Each(
        vec![
            TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::InstalledRunnerCard, F::CardTypeIs(CardType::Program)],
            },
            TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::InstalledResource],
            },
        ],
    ))])
    .labeled("[sub] trash 1 program and 1 resource")];
    c
}

/// Cleaver shape (1.15.1): "2[c]: Break up to 2 barrier subroutines." The
/// break ability's targets are the 1 or 2 subroutines it will break (9.8.6:
/// only unbroken ones can be chosen). The [interface] strength gate and the
/// barrier stipulation are the card's; the strength-pumping half is elided.
pub fn cleaver_like(name: &'static str, strength: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.strength = Some(strength);
    c.memory_cost = Some(1);
    c.subtypes = vec!["icebreaker", "fracter"];
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(2),
        vec![Instruction::BreakSubroutines {
            subs: crate::instr::SubroutineSpec::Chosen { count: Quantity::c(2), up_to: true },
        }],
    )
    .with_flag(AbilityFlag::Interface)
    .with_timing(TimingRestriction::EncounterOnly { required_subtype: Some("barrier") })
    .labeled("cleaver: break up to 2 barrier subroutines")];
    c
}

/// Grappling-Hook shape (9.8.6b): "[trash]: Break all but 1 subroutine on
/// the encountered ice." The announced target is the subroutine that will
/// NOT be broken, so an already-broken subroutine is a legal choice.
pub fn grappling_hook_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![Instruction::BreakSubroutines {
            subs: crate::instr::SubroutineSpec::AllBut { count: Quantity::c(1) },
        }],
    )
    .with_timing(TimingRestriction::EncounterOnly { required_subtype: None })
    .labeled("hook: break all but 1 subroutine")];
    c
}

/// Heimdall-1.0 shape (9.8.6b): a barrier with "[sub] Do 1 core damage."
/// and two "[sub] End the run." subroutines.
pub fn heimdall_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 8, 6);
    c.subtypes = vec!["barrier"];
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::Damage {
            kind: DamageKind::Core,
            amount: Quantity::c(1),
            responsible: Side::Corp,
        }])
        .labeled("[sub] do 1 core damage"),
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] end the run (a)"),
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] end the run (b)"),
    ];
    c
}

/// Howler shape (1.15.4): a piece of ice whose subroutine installs a piece
/// of ice from HQ and then creates a delayed conditional ability that acts
/// on THAT card — "subsequent instructions of the same ability can continue
/// to act on that target without needing to select it again".
///
/// Simplification (§12 rule 3): the real card's delayed ability also returns
/// Howler to HQ and its install may come from Archives; neither is what the
/// example is about.
pub fn howler_like(name: &'static str, protecting: ServerId) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_ice(name, 0, 3);
    c.abilities = vec![AbilityDef::subroutine(vec![
        Instruction::InstallCard {
            card: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::CardsInHandOf(Side::Corp), F::CardTypeIs(CardType::Ice)],
            },
            dest: crate::instr::InstallDest::Protecting(protecting),
            and_rez: true,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        },
        Instruction::CreateDelayedConditional {
            def: Box::new(
                AbilityDef::conditional(
                    TriggerCond::EncounterEnds,
                    vec![Instruction::TrashCards(TargetSpec::EarlierTarget { nth: 0 })],
                    false,
                )
                .labeled("howler-delayed: trash the installed ice"),
            ),
            duration: crate::lingering::WantedDuration::UntilResolved,
        },
    ])
    .labeled("[sub] install an ice from HQ, trash it when the encounter ends")];
    c
}

/// Top-Hat shape (1.15.1): "You may choose 1 of the top 5 cards of R&D and
/// access it." The target is the card in R&D the Runner chooses — a target
/// in a zone the instruction names, so 1.15.2c's play-area restriction does
/// not apply to it.
///
/// Simplification (§12 rule 3): the real card is a breach REPLACEMENT
/// ("instead of accessing cards…"). The replacement wrapper is exercised by
/// the §9.9.11 tests and is orthogonal to what this example claims, so the
/// choose-and-access half is a paid ability here.
pub fn top_hat_like(name: &'static str, top: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::AccessCards {
            cards: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::TopOfDeckOf {
                    side: Side::Corp,
                    n: top,
                }],
            },
        }],
    )
    .labeled("top-hat: access 1 of the top 5 cards of R&D")];
    c
}

// ---------------------------------------------------------------------------
// W7d shapes: subtypes as modifiable characteristics (§2.16.5, §9.11.4c)
// ---------------------------------------------------------------------------

/// Tinkering shape (9.11.4c / 2.16.5): "Choose a piece of ice. That ice gains
/// sentry, code gate, and barrier until the end of the turn." Two printed
/// sentences, ONE instruction: the first only directs the player to select a
/// target.
pub fn tinkering_like(name: &'static str) -> PrintedCard {
    event(
        name,
        0,
        vec![Instruction::ModifySubtypes {
            target: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Ice)],
            },
            add: vec!["sentry", "code gate", "barrier"],
            remove: Vec::new(),
            duration: crate::lingering::WantedDuration::ThisTurn,
        }],
    )
}

/// Lycan/Morph shape (2.16.5): a piece of ice that prints a subtype and whose
/// own static ability removes ONE instance of it. Subtype presence is a
/// COUNT, so a card that also gains an instance from elsewhere keeps it.
///
/// Simplification (§12 rule 3): the real morph ice swaps subtypes on being
/// advanced; the counting is the whole point here.
pub fn morph_ice(name: &'static str, prints: &'static str, loses: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 3, 3);
    c.subtypes = vec![prints];
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Static,
        flags: Vec::new(),
        condition: None,
        cost: None,
        instructions: Vec::new(),
        statics: vec![StaticDecl::SubtypeModSelf { add: Vec::new(), remove: vec![loses] }],
        optional: false,
        timing: None,
        label: "morph: lose 1 instance of a subtype",
    }];
    c
}

// ---------------------------------------------------------------------------
// W7e shapes
// ---------------------------------------------------------------------------

/// Lycian-Multi-Munition shape (6.1.4c): a Corp card whose free paid ability
/// gains 1 credit and ends the run — the "no further effect" half of 6.1.4c
/// when it is used with no run and no encounter in progress.
pub fn gain_and_etr_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::Combined(vec![
            Instruction::GainCredits(Side::Corp, Quantity::c(1)),
            Instruction::EndTheRun,
        ])],
    )
    .labeled("munition: gain 1 and end the run")];
    c
}

/// Eli-1.0 shape (5.2.1a): a "Lose [click]: Break 1 subroutine on the
/// encountered ice." ability. Spending a click is its cost, but the ability
/// is NOT an action — it is used during a paid ability window.
///
/// Simplification (§12 rule 3): the printed card is a piece of ice whose
/// ability says "Only the Runner can use this ability"; the kernel offers a
/// card's paid abilities to its controller and has no who-may-use modifier,
/// so the shape is a Runner card. The example's claim — where a
/// Lose-[click] ability is offered — is unaffected.
pub fn lose_click_break_program(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::lose_clicks(1),
        vec![Instruction::BreakSubroutines {
            subs: crate::instr::SubroutineSpec::Chosen { count: Quantity::c(1), up_to: false },
        }],
    )
    .with_timing(TimingRestriction::EncounterOnly { required_subtype: None })
    .labeled("lose-click: lose [click] to break 1 subroutine")];
    c
}

/// Professional-Contacts shape (5.2.1a): a card with a "[click]: Gain
/// 1[credit] and draw 1 card." ability — a [click] cost makes it an action.
pub fn click_action_card(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost { clicks: 1, ..Cost::default() },
        vec![Instruction::Combined(vec![
            Instruction::GainCredits(Side::Runner, Quantity::c(1)),
            Instruction::Draw(Side::Runner, 1),
        ])],
    )
    .labeled("procon: [click] gain 1 and draw 1")];
    c
}

/// Stimhack shape (5.2.2b): a Runner event whose play ability runs a server
/// and then does 1 core damage — the action is not complete until the run
/// ends, the damage is suffered and the event is trashed.
pub fn stimhack_like(name: &'static str, server: ServerId) -> PrintedCard {
    event(
        name,
        0,
        vec![
            Instruction::run(server),
            Instruction::Damage {
                kind: DamageKind::Core,
                amount: Quantity::c(1),
                responsible: Side::Runner,
            },
        ],
    )
}

// ---------------------------------------------------------------------------
// W7f shapes: object identity across moves (§1.12)
// ---------------------------------------------------------------------------

/// Vaporframe-Fabricator shape (1.12.2 / 1.12.5): a Corp asset with a
/// once-per-turn free paid ability. Whether that ability is available again
/// is exactly the question of whether the card is still the same OBJECT.
pub fn once_per_turn_asset(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
    )
    .with_flag(AbilityFlag::OncePerTurn)
    .labeled("vaporframe: once per turn, gain 1")];
    c
}

/// Divert-Power shape (1.12.5): a Corp button that derezzes a card.
pub fn derez_button(name: &'static str, target: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::Derez { target: TargetSpec::Objects(vec![target]) }],
    )
    .labeled("divert: derez a card")];
    c
}

/// Priority-Construction shape (1.12.2a): an operation that installs a piece
/// of ice from HQ and then places advancement counters on THAT card — its
/// second instruction finds the new object without re-announcing it (1.15.4).
pub fn priority_construction_like(name: &'static str, protecting: ServerId) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    operation(
        name,
        0,
        vec![
            Instruction::InstallCard {
                card: TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![F::CardsInHandOf(Side::Corp), F::CardTypeIs(CardType::Ice)],
                },
                dest: crate::instr::InstallDest::Protecting(protecting),
                and_rez: false,
                ignore_costs: true,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
            Instruction::PlaceCounters {
                target: TargetSpec::EarlierTarget { nth: 0 },
                kind: CounterKind::Advancement,
                amount: Quantity::c(3),
            },
        ],
    )
}

/// Project Yagi-Uda shape (6.2.7e): a Corp paid ability that swaps two pieces
/// of ice. SIMPLIFICATION: both sides of the swap are fixed at card-build
/// time — the §6.2 examples turn on what a swap does to POSITIONS and to the
/// Runner's progress through them, never on which ice is chosen. (The real
/// card's choice would be two `TargetSpec::Choose` announcements, which
/// `Instruction::SwapCards` already accepts.)
pub fn ice_swap_button(name: &'static str, a: ObjectId, b: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::SwapCards {
            a: TargetSpec::Objects(vec![a]),
            b: TargetSpec::Objects(vec![b]),
        }],
    )
    .labeled("yagi: swap 2 pieces of ice")];
    c
}

/// Drafter shape (6.2.6a): "[sub] Install 1 piece of ice from HQ protecting
/// this server, ignoring all costs. Trash 2 pieces of ice protecting the
/// attacked server." The install takes the outermost position (6.2.2a) and
/// the trashes vacate two positions, which is the whole content of the
/// example — the new ice sits outward from where the Runner is standing.
pub fn drafter_like(name: &'static str, installee: ObjectId, server: ServerId) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_ice(name, 0, 4);
    c.abilities = vec![AbilityDef::subroutine(vec![
        Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::Protecting(server),
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        },
        Instruction::TrashCards(TargetSpec::Choose {
            count: Quantity::c(2),
            criteria: vec![F::IceProtectingAttackedServer],
        }),
    ])
    .labeled("[sub] install ice outermost, trash 2 ice")];
    c
}

/// Rook shape (6.2.3): "[click]: Host this card on a piece of ice in the same
/// position." SIMPLIFICATION: the printed card says "ANOTHER piece of ice",
/// and nothing in the filter vocabulary excludes the current host, so the
/// host itself is among the offered targets. That is orthogonal to the
/// example, which is about WHICH SERVERS' ice qualify — a server protected by
/// only 1 piece of ice has nothing in the 2nd position.
pub fn rook_like(name: &'static str) -> PrintedCard {
    use crate::instr::{PositionRef, TargetFilter as F};
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::HostCards {
            cards: TargetSpec::SelfSource,
            host: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::IceInSamePositionAs(PositionRef::Source)],
            },
        }],
    )
    .labeled("rook: move to ice in the same position")];
    c
}

/// Slipstream shape (6.2.3 / 6.2.8a): "You may move to the same position
/// protecting another server, then approach that ice." SIMPLIFICATION: a paid
/// ability used in the Movement Phase paid window rather than a "when you
/// pass a piece of ice" conditional — the example is about which positions
/// are reachable, not about when the offer arrives — and, as with the Rook
/// shape, "another server" is not expressible as a criterion, so the ice the
/// Runner is already standing at is offered too.
pub fn slipstream_like(name: &'static str) -> PrintedCard {
    use crate::instr::{PositionRef, TargetFilter as F};
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::MoveRunnerToIce {
            ice: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::IceInSamePositionAs(PositionRef::Runner)],
            },
            encounter: false,
        }],
    )
    .labeled("slipstream: move to the same position")];
    c
}

/// Bullfrog shape (6.2.7d): "[sub] Move this ice to the outermost position
/// protecting another server." SIMPLIFICATION: the destination server is
/// fixed at card-build time, because the decision vocabulary addresses
/// objects and subroutines, not servers.
pub fn bullfrog_like(name: &'static str, to: ServerId) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::MoveIce {
        ice: TargetSpec::SelfSource,
        dest: crate::instr::InstallDest::Protecting(to),
    }])
    .labeled("[sub] move this ice to another server")];
    c
}

/// Thimblerig shape (8.8.3a): a piece of ice with a paid ability "Swap this
/// ice with another piece of ice." SIMPLIFICATION: as with the Rook shape,
/// "another" is not expressible as a criterion, so the source is among its
/// own offered targets — swapping a card with itself is a no-op (8.8.1: the
/// two locations are the same).
pub fn thimblerig_like(name: &'static str) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::SwapCards {
            a: TargetSpec::SelfSource,
            b: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::InstalledCorpCard, F::CardTypeIs(CardType::Ice)],
            },
        }],
    )
    .labeled("thimblerig: swap this ice with another")];
    c
}

/// Metamorph shape (8.8.2): "[sub] Swap 2 installed Corp cards." The two
/// announcements are filtered by 8.8.2 — each card must be allowed to occupy
/// the other's location — which is the whole content of the example.
pub fn metamorph_like(name: &'static str) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::SwapCards {
        a: TargetSpec::Choose {
            count: Quantity::c(1),
            criteria: vec![F::InstalledCorpCard],
        },
        b: TargetSpec::Choose {
            count: Quantity::c(1),
            criteria: vec![F::InstalledCorpCard],
        },
    }])
    .labeled("[sub] swap 2 installed cards")];
    c
}

/// Tatu-Bola shape (8.8.4b): "When the Runner passes this ice, you may swap
/// it with a piece of ice from HQ. Gain 4[credit]." SIMPLIFICATION: the ice in
/// HQ is fixed at card-build time, and the swap is not optional — the example
/// is about what a swap between an installed card and an uninstalled one
/// does, and about the next instruction still resolving afterwards.
pub fn tatu_bola_like(name: &'static str, from_hq: ObjectId) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfPassed,
        vec![
            Instruction::SwapCards {
                a: TargetSpec::SelfSource,
                b: TargetSpec::Objects(vec![from_hq]),
            },
            Instruction::GainCredits(Side::Corp, Quantity::c(4)),
        ],
        false,
    )
    .labeled("tatu: swap with HQ ice, then gain 4")];
    c
}

/// A Teia shape (8.8.4b): "Whenever you install a card, you may install 1 card
/// from HQ in the root of another remote server, ignoring all costs."
/// SIMPLIFICATION: the trigger is every Corp install rather than only those in
/// or protecting a remote server, and the destination server is fixed —
/// the example turns on the swapped-in card meeting an INSTALL trigger
/// condition at the next checkpoint (8.8.4b), not on the identity's own
/// wording.
pub fn a_teia_like(name: &'static str, installee: ObjectId, into: ServerId) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CardInstalledBy(Side::Corp),
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::Root(into),
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
        true,
    )
    .labeled("a-teia: install a card in another remote")];
    c
}

/// Tucana shape with the 1.16.2f "total" modifier: "Install and rez 1 piece
/// of ice from HQ protecting <server>, paying a total of N[credit] less."
/// SIMPLIFICATION: the ice is fixed at card-build time (the real card
/// searches R&D — `tucana_like` above covers the search half; this shape is
/// about how the "total" modifier is divided).
pub fn total_discount_install_rez(
    name: &'static str,
    ice: ObjectId,
    server: ServerId,
    total: i64,
) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![ice]),
            dest: crate::instr::InstallDest::Protecting(server),
            and_rez: true,
            ignore_costs: false,
            reveal_check: None,
            reduce_total: Quantity::c(total),
        }],
    )
    .labeled("tucana-total: install and rez for a total less")];
    c
}

/// Cayambe Grid shape (1.16.2b): "When the Runner approaches this server,
/// end the run unless they pay 2[credit] for each piece of ice protecting the
/// attacked server." SIMPLIFICATION: the printed card counts only ADVANCED
/// ice; the example's point is that a "for each" calculation in a cost is one
/// aggregated payment, which the advancement qualifier is orthogonal to.
pub fn cayambe_like(name: &'static str) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::EncounterBegins,
        vec![Instruction::NestedCostUnless {
            cost: Cost::credits_q(Quantity::Times(
                2,
                Box::new(Quantity::Count(F::IceProtectingAttackedServer)),
            )),
            effect: Box::new(Instruction::EndTheRun),
            payer: Some(Side::Runner),
        }],
        false,
    )
    .labeled("cayambe: pay 2 per ice or end the run")];
    c
}

/// GameNET shape (1.16.2b): "Whenever the Runner spends credits, gain
/// 1[credit]." One instance per PAYMENT, which is what the Cayambe example
/// measures.
pub fn gamenet_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerPaysCredits(Side::Runner),
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("gamenet: the Runner spent credits")];
    c
}

/// Guru Davinder shape (1.16.1b): a MANDATORY conditional interrupt —
/// "The first time you would suffer net damage, prevent it; trash this card
/// unless you pay 4[credit]." Because it is mandatory and it prevents, a
/// "suffer 4 net damage" COST cannot be paid at all (1.16.1b), which is what
/// the Obokata example turns on. SIMPLIFICATION: net damage only, and the
/// "first time each turn" limit is not modeled — the example is about the
/// cost being unpayable while the ability is active.
pub fn guru_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::WouldDamage { kind: Some(DamageKind::Net), first_each_run: false },
        vec![
            Instruction::PreventAllDamage { kind: DamageKind::Net },
            Instruction::NestedCostUnless {
                cost: Cost::credits(4),
                effect: Box::new(Instruction::TrashSelf),
                payer: Some(Side::Runner),
            },
        ],
        false,
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("guru: prevent the net damage")];
    c
}

// ---------------------------------------------------------------------------
// W9a shapes: encounters as a timing structure (§6.5.9, §6.1.4b, §6.5.8)
// ---------------------------------------------------------------------------

/// Chrysalis / Archangel shape (6.5.9a + 9.1.8h): a piece of ice reading
/// "When the Runner accesses this card, they encounter it." The encounter
/// happens wherever the card is — accessed from HQ or R&D it is not
/// installed, and 9.1.8h is what keeps the subroutine below active for
/// exactly that encounter.
///
/// SIMPLIFICATION (§12 rule 3): Archangel's printed ability is optional and
/// costs the Corp 3[credit]; the forced encounter here is mandatory and free,
/// because every example using this shape is about the encounter, not about
/// who pays for it. Chrysalis's own text is exactly this shape.
pub fn accessed_encounter_ice(name: &'static str, strength: i32, net: i64) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, strength);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfAccessed { requires: Vec::new() },
            vec![Instruction::ForceEncounter { ice: TargetSpec::SelfSource }],
            false,
        )
        .labeled("chrysalis: encounter this card on access"),
        AbilityDef::subroutine(vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(net),
            responsible: Side::Corp,
        }])
        .labeled("[sub] net damage"),
    ];
    c
}

/// Ganked! shape (6.5.9a): "When the Runner accesses this card, force them to
/// encounter a rezzed piece of ice you control."
///
/// SIMPLIFICATION (§12 rule 3): the printed card is optional ("you may") and
/// trashes itself; both are orthogonal to the forced encounter the examples
/// using this shape are about.
pub fn ganked_encounter_like(name: &'static str) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_asset(name, 0, 0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAccessed { requires: Vec::new() },
        vec![Instruction::ForceEncounter {
            ice: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::Rezzed, F::CardTypeIs(CardType::Ice)],
            },
        }],
        false,
    )
    .labeled("ganked: encounter a rezzed piece of ice")];
    c
}

/// Loot Box shape (6.1.4b): a piece of ice whose FIRST subroutine ends the
/// run and whose second one does something observable. Encountered outside a
/// run, the first subroutine ends the encounter instead, and the second never
/// resolves.
///
/// SIMPLIFICATION (§12 rule 3): the printed card's first subroutine is "end
/// the run unless the Runner pays 2[credit]" and its second reveals and adds
/// a card from the stack; the declinable cost and the reveal are orthogonal
/// to what happens to the second subroutine.
pub fn loot_box_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 4);
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run"),
        AbilityDef::subroutine(vec![Instruction::GainCredits(Side::Corp, Quantity::c(3))])
            .labeled("[sub] The Corp gains 3"),
    ];
    c
}

/// Shiro shape (6.5.9a): a piece of ice whose first subroutine causes a card
/// to be accessed and whose second one does something observable, so the
/// example's "return to resolving subroutines on Shiro" is checkable.
///
/// SIMPLIFICATION (§12 rule 3): the printed card accesses the BOTTOM card of
/// R&D after looking at the top one; which card is accessed is orthogonal.
pub fn shiro_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 4);
    c.abilities = vec![
        AbilityDef::subroutine(vec![Instruction::AccessCards {
            cards: TargetSpec::TopOfDeck(Side::Corp, 1),
        }])
        .labeled("[sub] The Runner accesses the top card of R&D"),
        AbilityDef::subroutine(vec![Instruction::GainCredits(Side::Corp, Quantity::c(2))])
            .labeled("[sub] The Corp gains 2"),
    ];
    c
}

/// The Twins shape (6.5.9b): "When the Runner passes this ice, they encounter
/// it again." — a forced encounter opened from the Movement Phase.
///
/// SIMPLIFICATION (§12 rule 3): the printed card is a Corp asset that trashes
/// a copy of the passed ice from HQ; where the ability LIVES is orthogonal to
/// 6.5.9b's claim about ending the run during the extra encounter.
pub fn twins_ice(name: &'static str, strength: i32) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, strength);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfPassed,
            vec![Instruction::ForceEncounter { ice: TargetSpec::SelfSource }],
            false,
        )
        .labeled("twins: encounter this ice again"),
        AbilityDef::subroutine(vec![Instruction::EndTheRun]).labeled("[sub] End the run"),
    ];
    c
}

/// Cell Portal shape (6.2.5d): "The Runner moves to this piece of ice. Derez
/// this ice." During the Success Phase the Runner has no position and cannot
/// move to one, so only the derez happens.
///
/// SIMPLIFICATION (§12 rule 3): the printed card sends the Runner to the
/// OUTERMOST position; "no position can be entered" is what the example turns
/// on, and it is the same refusal for either destination.
pub fn cell_portal_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 4);
    c.abilities = vec![AbilityDef::subroutine(vec![
        Instruction::MoveRunnerToIce { ice: TargetSpec::SelfSource, encounter: false },
        Instruction::Derez { target: TargetSpec::SelfSource },
    ])
    .labeled("[sub] Move to this ice; derez it")];
    c
}

/// Gang Sign / Détente shape: a paid ability accessing 1 card from HQ with no
/// run in progress (§7.2 as an instruction — the access structure without a
/// breach around it).
pub fn hq_access_button(name: &'static str) -> PrintedCard {
    use crate::instr::TargetFilter as F;
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::AccessCards {
            cards: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![F::CardsInHandOf(Side::Corp)],
            },
        }],
    )
    .labeled("access-hq: access 1 card from HQ")];
    c
}

/// Devil Charm shape (3.4.4a): "Lower the strength of the encountered ice by
/// 3 for the remainder of the run." Used during an encounter with no run in
/// progress, the stated duration names a structure that is not in progress
/// (9.10.4) and the modification lasts for the remainder of the encounter
/// instead (3.4.4a, via the implicit encounter duration of 3.9.5c).
///
/// SIMPLIFICATION (§12 rule 3): the printed card trashes itself as the cost;
/// the cost is orthogonal to the duration this shape exists to measure.
pub fn devil_charm_like(name: &'static str, amount: i32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::ModifyStrength {
            target: TargetSpec::EncounteredIce,
            amount: -amount,
            duration: Some(crate::lingering::WantedDuration::ThisRun),
        }],
    )
    .with_timing(TimingRestriction::EncounterOnly { required_subtype: None })
    .labeled("devil-charm: lower the encountered ice's strength for the run")];
    c
}

// ---------------------------------------------------------------------------
// W9b shapes: advancement (§1.18) and the dividends keyword (§10.13)
// ---------------------------------------------------------------------------

/// Oaktown Renovation shape (1.18.2): "Whenever you advance a card, gain
/// 2[credit]." The point of the shape is the DISCRIMINATION — advancing meets
/// the condition, placing an advancement counter directly does not.
///
/// SIMPLIFICATION (§12 rule 3): the printed card is an agenda that is always
/// installed FACEUP, which is what makes its own ability active while it sits
/// in a server; the kernel has no always-faceup install, and an inactive
/// card's ability would not fire for either reason — so the ability lives on
/// a rezzed card here and watches every advance, which is exactly the
/// discrimination 1.18.2 states.
pub fn advance_watcher(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::AdvancesCard { had_no_advancement: false },
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(2))],
        false,
    )
    .labeled("oaktown: gain 2 on advance")];
    c
}

/// Mushin No Shin shape (1.18.2): "Install 1 card from HQ and place 3
/// advancement counters on it." The counters are PLACED, not advanced.
///
/// SIMPLIFICATION (§12 rule 3): the installed card is fixed at card-build
/// time (the real card chooses from HQ) — which card is installed is
/// orthogonal to whether placing counters is advancing.
pub fn mushin_like(name: &'static str, card: ObjectId, server: ServerId) -> PrintedCard {
    operation(
        name,
        0,
        vec![
            Instruction::InstallCard {
                card: TargetSpec::Objects(vec![card]),
                dest: crate::instr::InstallDest::Root(server),
                and_rez: false,
                ignore_costs: true,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
            Instruction::PlaceCounters {
                target: TargetSpec::Objects(vec![card]),
                kind: CounterKind::Advancement,
                amount: Quantity::c(3),
            },
        ],
    )
}

/// An agenda carrying the **dividends** keyword (10.13.1), expanded into the
/// conditional ability the keyword denotes.
pub fn dividends_agenda(name: &'static str, req: u32, points: i32, n: i64) -> PrintedCard {
    vanilla_agenda(name, req, points).with_dividends(n)
}

/// SanSan City Grid shape (1.17.3a): "The Corp can score agendas in this
/// server with 1 fewer advancement counter." An upgrade whose declaration
/// lowers the advancement requirement of the agendas in its server.
pub fn sansan_like(name: &'static str, fewer: i32) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::ScoreRequirementModInSourceServer(-fewer)])
    .labeled("sansan: score for 1 fewer advancement")];
    c
}

// ---------------------------------------------------------------------------
// W9c shapes: the attacked server (§6.1.2d, §6.3.2a)
// ---------------------------------------------------------------------------

/// Sneakdoor Beta shape (6.1.2d): a paid ability that changes the attacked
/// server directly, without referring to the Runner's position.
///
/// SIMPLIFICATION (§12 rule 3): the printed card's ability is an "if
/// successful" one on a run it initiates itself; the initiation and the
/// success condition are orthogonal to 6.1.2d's claim about the timing step.
pub fn sneakdoor_like(name: &'static str, to: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::ChangeAttackedServer { server: to }],
    )
    .labeled("sneakdoor: the attacked server becomes HQ")];
    c
}

/// Off the Grid shape (6.3.2a): "The Runner cannot initiate a run on this
/// server." A declaration about the ANNOUNCEMENT of the attacked server.
pub fn off_the_grid_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::CannotInitiateRunOnSourceServer,
    ])
    .labeled("off-the-grid: cannot initiate a run here")];
    c
}

// ---------------------------------------------------------------------------
// W11a shapes: resolving an ability by class (§9.6.14d, §9.8), rezzing by
// ability (8.1.2b), forfeit as a cost (8.2.5)
// ---------------------------------------------------------------------------

/// AstroScript / Market Research shape (9.6.14c): an agenda whose "when
/// scored" ability places 1 agenda counter on it, optionally with 9.6.5c's
/// additional requirement that the Runner be tagged.
pub fn when_scored_agenda(
    name: &'static str,
    req: u32,
    points: i32,
    requires_tagged: bool,
) -> PrintedCard {
    let mut c = vanilla_agenda(name, req, points);
    let requires = if requires_tagged {
        vec![TriggerRequirement::RunnerTagged]
    } else {
        Vec::new()
    };
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfScored { requires },
        vec![Instruction::PlaceCounters {
            target: TargetSpec::SelfSource,
            kind: CounterKind::Agenda,
            amount: Quantity::c(1),
        }],
        false,
    )
    .labeled("when scored: place an agenda counter")];
    c
}

/// 24/7 News Cycle shape (9.6.14d): an operation whose additional play cost
/// is forfeiting an agenda and whose play ability resolves the "when scored"
/// ability of an agenda in the Corp's score area — chosen as a 1.15.2 target
/// announcement, since the criteria name the zone (1.15.2c).
///
/// SIMPLIFICATION (§12 rule 3): the printed card's "Play only if you scored
/// an agenda this turn" restriction (9.11.4a) is omitted — it gates when the
/// operation may be played and is orthogonal to what 9.6.14d decides.
pub fn news_cycle_like(name: &'static str) -> PrintedCard {
    let mut c = operation(
        name,
        0,
        vec![Instruction::ResolveAbilityOf {
            source: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![
                    crate::instr::TargetFilter::InScoreAreaOf(Side::Corp),
                    crate::instr::TargetFilter::CardTypeIs(CardType::Agenda),
                ],
            },
            which: AbilityClass::WhenScored,
        }],
    );
    c.additional_play_cost = Some(Cost::forfeit_agenda(1));
    c
}

/// Nanisivik Grid shape (4.6.6i example 3): "[trash]: Turn 1 facedown piece
/// of ice faceup and resolve its first subroutine." Rezzing it ignoring costs
/// IS turning it faceup (8.1.2), which is what makes the ice active so its
/// subroutine can resolve at all (9.1.7).
///
/// SIMPLIFICATION (§12 rule 3): the printed card scopes the choice to ice
/// protecting its own server; the shape lets the Corp announce any piece of
/// ice, which is what makes "this server" in the resolved subroutine
/// DISCRIMINATING — the grid can sit in a different server from the ice.
pub fn nanisivik_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![AbilityDef::paid(
        Cost::trash_self(),
        vec![
            Instruction::RezCard {
                target: TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Ice)],
                },
                ignore_costs: true,
            },
            Instruction::ResolveAbilityOf {
                source: TargetSpec::EarlierTarget { nth: 0 },
                which: AbilityClass::Subroutine(0),
            },
        ],
    )
    .labeled("nanisivik: turn ice faceup and resolve its first subroutine")];
    c
}

// ---------------------------------------------------------------------------
// W11b shapes: §9.1 — is-resolving scope, resolution independence
// ---------------------------------------------------------------------------

/// Attini shape (9.1.2b): a piece of ice declaring that the Runner cannot
/// spend credits during the resolution of its own abilities, plus a
/// net-damage subroutine. The scope is `StaticCond::SourceAbilityResolving`,
/// which is exactly 9.1.2b's "from when its first instruction becomes
/// imminent until its last instruction has finished resolving".
///
/// SIMPLIFICATION (§12 rule 3): the printed card gates the declaration on the
/// size of the Runner's grip; the grip size is orthogonal to what 9.1.2b
/// decides, so the shape declares it unconditionally.
pub fn attini_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 3);
    c.abilities = vec![
        AbilityDef {
            condition: Some(Condition::Static(StaticCond::SourceAbilityResolving)),
            ..AbilityDef::static_ability(vec![StaticDecl::CannotSpendCredits(Side::Runner)])
        }
        .labeled("attini: no spending while its abilities resolve"),
        AbilityDef::subroutine(vec![Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(1),
            responsible: Side::Corp,
        }])
        .labeled("[sub] do 1 net damage"),
    ];
    c
}

/// Caldera shape (9.1.2b): a Runner card with a credit-costed interrupt
/// ability that prevents 1 net damage.
pub fn caldera_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(1),
        vec![Instruction::PreventDamage { kind: DamageKind::Net, amount: 1 }],
    )
    .with_flag(AbilityFlag::Interrupt)
    .labeled("caldera: prevent 1 net damage")];
    c
}

/// Direct Access shape (9.1.2b example 2): an event that runs a server and
/// declares that identity cards do not have abilities. The declaration is
/// active for as long as the card is — which, for an event, is throughout
/// step 8.6.7f (9.1.2b), the very window the run-end conditions pend in.
pub fn direct_access_like(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = event(name, 0, vec![Instruction::run(server)]);
    c.abilities.push(
        AbilityDef::static_ability(vec![StaticDecl::RemoveAbilitiesOfMatching {
            criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Identity)],
        }])
        .labeled("direct access: identities have no abilities"),
    );
    c
}

/// Zahya Sadeghi shape (9.1.2b example 2): a Runner identity whose ability
/// meets its condition when a run ends.
pub fn run_end_identity(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Runner, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunEnds { successful_only: false },
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        false,
    )
    .labeled("identity: gain 1 when a run ends")];
    c
}

/// Compile shape (9.1.4): a Runner card that arms a "when this run ends, add
/// that program to the bottom of your stack" delayed conditional, used from
/// inside the run (9.6.13d: with no run in progress the delayed conditional
/// is never created at all).
///
/// SIMPLIFICATION (§12 rule 3): the printed card makes the run itself and
/// installs the program from the stack during it; the run comes from the
/// basic run action here and the program is pre-installed and named, because
/// what 9.1.4 decides is what happens to the OTHER ability once this one has
/// moved the program.
pub fn compile_like(name: &'static str, program: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateDelayedConditional {
            def: Box::new(
                AbilityDef::conditional(
                    TriggerCond::RunEnds { successful_only: false },
                    vec![Instruction::MoveToDeck {
                        card: TargetSpec::Objects(vec![program]),
                        top: false,
                    }],
                    false,
                )
                .labeled("compile-delayed: the program goes to the bottom of the stack"),
            ),
            duration: crate::lingering::WantedDuration::UntilResolved,
        }],
    )
    .labeled("compile: arm the run-end move")];
    c
}

// ---------------------------------------------------------------------------
// W11c shapes: exposing (1.21.4), trashes that did not happen (8.2.2a),
// ending the run from a paid window (6.8.2a)
// ---------------------------------------------------------------------------

/// Satellite Uplink shape (9.6.4b): "Expose up to 2 cards." — ONE instruction
/// exposing several cards, which is what makes the occurrence count the point.
pub fn satellite_uplink_like(name: &'static str, count: i64) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::ExposeCards {
            cards: TargetSpec::Choose {
                count: Quantity::c(count),
                criteria: vec![crate::instr::TargetFilter::InstalledCorpCard],
            },
        }],
    )
    .labeled("satellite-uplink: expose cards")];
    c
}

/// Blackguard shape (9.6.4b): "Whenever a card is exposed, …" — one instance
/// per exposed card.
pub fn blackguard_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CardExposed,
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        false,
    )
    .labeled("blackguard: react to an exposed card")];
    c
}

/// District 99 / Wasteland shape (8.2.2a): "Whenever an installed program or
/// piece of hardware is trashed, place 1 power counter on this card." A trash
/// that was PREVENTED never happened, so the counter is not placed.
pub fn trash_counter_like(name: &'static str, of: Side) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::InstalledCardTrashed {
            side: of,
            of_types: vec![CardType::Program, CardType::Hardware],
        },
        vec![Instruction::PlaceCounters {
            target: TargetSpec::SelfSource,
            kind: CounterKind::Power,
            amount: Quantity::c(1),
        }],
        false,
    )
    .labeled("district99: count trashed installed cards")];
    c
}

/// Rototurret shape: "[subroutine] Trash 1 installed program."
pub fn rototurret_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 3);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::TrashCards(
        TargetSpec::Choose {
            count: Quantity::c(1),
            criteria: vec![
                crate::instr::TargetFilter::InstalledRunnerCard,
                crate::instr::TargetFilter::CardTypeIs(CardType::Program),
            ],
        },
    )])
    .labeled("[sub] trash 1 installed program")];
    c
}

/// Nisei MK II shape (6.8.2a): a scored agenda whose paid ability spends an
/// agenda counter to end the run.
pub fn nisei_like(name: &'static str, req: u32, points: i32) -> PrintedCard {
    let mut c = vanilla_agenda(name, req, points);
    c.abilities = vec![AbilityDef::paid(
        Cost::spend_counters(CounterKind::Agenda, 1),
        vec![Instruction::EndTheRun],
    )
    .labeled("nisei: spend an agenda counter to end the run")];
    c
}

/// Self-modifying Code shape (6.8.2a): a Runner paid ability with a credit
/// cost, used to show which windows the Runner still has to spend in.
pub fn smc_credit_button(name: &'static str, cost: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(1);
    c.abilities = vec![AbilityDef::paid(
        Cost::credits(cost),
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(0))],
    )
    .labeled("smc: a credit-costed paid ability")];
    c
}

/// Guru Davinder shape (9.9.7f): a Runner resource with an interrupt that
/// prevents all damage of a kind, and a NON-interrupt conditional ability
/// whose trigger condition is "whenever this card prevents 1 or more damage"
/// — which the Runner answers by paying 4[credit] or trashing the resource.
pub fn guru_davinder_like(name: &'static str, kind: DamageKind) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![
        AbilityDef::paid(Cost::free(), vec![Instruction::PreventAllDamage { kind }])
            .with_flag(AbilityFlag::Interrupt)
            .labeled("guru: prevent all damage"),
        AbilityDef::conditional(
            TriggerCond::SourcePreventedDamage,
            vec![Instruction::NestedCostUnless {
                cost: Cost::credits(4),
                effect: Box::new(Instruction::TrashSelf),
                payer: Some(Side::Runner),
            }],
            false,
        )
        .labeled("guru: pay 4 or trash"),
    ];
    c
}

/// Architect shape (9.11.4e): ONE printed sentence that looks at the top N
/// cards of R&D and installs one of them. Transcribed as the 9.11.4e split —
/// making the cards visible ends the first instruction; the install is the
/// second, and it is optional, so the Corp may decline to choose a target.
///
/// SIMPLIFICATION (§12 rule 3): the install destination is fixed at card-build
/// time; where the card goes is orthogonal to what 9.11.4e decides.
pub fn architect_look_install(name: &'static str, n: u32, dest: ServerId) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 5);
    c.abilities = vec![AbilityDef::subroutine(vec![
        Instruction::LookAtCards {
            cards: TargetSpec::TopOfDeck(Side::Corp, n),
            by: Side::Corp,
        },
        Instruction::DeclineableChoice(Box::new(Instruction::InstallCard {
            card: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::TopOfDeckOf { side: Side::Corp, n }],
            },
            dest: crate::instr::InstallDest::Root(dest),
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        })),
    ])
    .labeled("[sub] look at the top of R&D and install one of those cards")];
    c
}

/// Oppo Research shape (9.7.1): an operation carrying FOUR abilities of three
/// types — a static ability that is nothing but a restriction (no
/// declarations), a conditional ability triggered by finishing resolving the
/// operation (8.6.7h), and TWO play abilities, which resolve in sequence while
/// the operation is being played.
///
/// SIMPLIFICATION (§12 rule 3): "your action phase ends" is transcribed as the
/// loss of the remaining clicks, which is what ends an action phase (5.6);
/// and the two play abilities' effects are stand-ins, since 9.7.1 decides
/// which ABILITY TYPE each sentence is, not what the sentences do.
pub fn oppo_research_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Operation);
    c.cost = Some(0);
    let play_ability = |instrs: Vec<Instruction>, label: &'static str| AbilityDef {
        kind: crate::ability::AbilityKind::Play,
        flags: Vec::new(),
        condition: None,
        cost: None,
        instructions: instrs,
        statics: Vec::new(),
        optional: false,
        timing: None,
        label,
    };
    c.abilities = vec![
        // 9.3.4/9.11.4a: a restriction sentence is not an instruction, and a
        // static ability may carry no declarations at all.
        AbilityDef::static_ability(Vec::new()).labeled("oppo: play only if …"),
        AbilityDef::conditional(
            TriggerCond::SelfPlayResolved,
            vec![Instruction::EndActionPhase(Side::Corp)],
            false,
        )
        .labeled("oppo: after you resolve this, your action phase ends"),
        play_ability(
            vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
            "oppo: first play ability",
        ),
        play_ability(vec![Instruction::GainTags(1)], "oppo: second play ability"),
    ];
    c
}

/// Clone Suffrage Movement shape (4.1.2a): "You may add 1 operation from
/// Archives to HQ." The criteria stipulate a CHARACTERISTIC (card type) of a
/// card in a zone where it may be facedown, which is what forces the reveal.
pub fn clone_suffrage_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::AddCardsToHand {
            cards: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![
                    crate::instr::TargetFilter::InDiscardOf(Side::Corp),
                    crate::instr::TargetFilter::CardTypeIs(CardType::Operation),
                ],
            },
        }],
    )
    .labeled("clone-suffrage: add an operation from Archives to HQ")];
    c
}

// ---------------------------------------------------------------------------
// W12a shapes: §7.3 breaching and §7.4 candidates
// ---------------------------------------------------------------------------

/// Flagship shape (7.4.2a/b): an upgrade whose STATIC ability prohibits the
/// Runner from accessing anything but itself, and only once the Runner has
/// actually accessed a card during the run (7.4.2b's condition). Declaring it
/// as a static is the point: the prohibition applies exactly while the ability
/// is active, so uninstalling or trashing the source lifts it mid-breach and
/// 7.4.2a's re-evaluation is observable.
pub fn flagship_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Upgrade);
    c.trash_cost = Some(2);
    let mut a = AbilityDef::static_ability(vec![StaticDecl::RestrictCandidatesToSelf])
        .labeled("flagship: no other accesses once you have accessed a card");
    a.condition = Some(Condition::Static(StaticCond::RunnerHasAccessedCardThisRun));
    c.abilities = vec![a];
    c
}

/// Docklands Pass shape (7.3.5b): "the first time you breach HQ each turn,
/// access 1 additional card".
///
/// SIMPLIFICATION (§12 rule 3): the "first time each turn" flag and the
/// automatic application at the beginning of the breach are elided — the
/// additional access is armed from a paid window like every other
/// Maker's-Eye-class effect in the suite, which is the same lingering effect
/// read at step 7.5.3.
pub fn docklands_pass_like(name: &'static str) -> PrintedCard {
    additional_access_card(name, ServerId::Hq, 1)
}

/// Cupellation shape (7.4.2a): "[interface] → host the accessed card on
/// Cupellation." Hosting UNINSTALLS the card (1.13.2a), and a persistent
/// ability is not what an upgrade's static prohibition is, so the prohibition
/// goes with it.
///
/// SIMPLIFICATION (§12 rule 3): the printed card can only host a card
/// accessed in the ROOT of a central server; this one hosts whatever is being
/// accessed. Which card is hosted is the caller's choice of window, and what
/// 7.4.2a turns on is that hosting uninstalls the prohibiting card.
pub fn cupellation_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::CanHost {
            criteria: Vec::new(),
            capacity: None,
        }])
        .labeled("cupellation: can host cards"),
        AbilityDef::paid(
            Cost::free(),
            vec![Instruction::HostCards {
                cards: TargetSpec::AccessedCard,
                host: TargetSpec::SelfSource,
            }],
        )
        .with_flag(AbilityFlag::Access)
        .labeled("cupellation: host the accessed card"),
    ];
    c
}

/// Otoroshi shape (7.4.2b): a Corp card whose ability makes the Runner access
/// a named card — an access that happens during the run but outside any
/// breach, which is exactly the "accessed a card before the breach began"
/// case the rule calls out.
pub fn otoroshi_like(name: &'static str, card: ObjectId) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::AccessCards { cards: TargetSpec::Objects(vec![card]) }],
    )
    .labeled("otoroshi: the Runner accesses that card")];
    c
}

/// Zahya Sadeghi shape (7.3.6): "when the run ends, gain 1[credit] for each
/// time you accessed a card during that run." The quantity is the selector,
/// so an access that was replaced never enters it.
pub fn zahya_counts_accesses(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Runner, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunEnds { successful_only: false },
        vec![Instruction::GainCredits(Side::Runner, Quantity::AccessesThisRun)],
        false,
    )
    .labeled("zahya: 1 credit per access this run")];
    c
}

/// Hades Shard shape (7.3.6 / 7.3.8): a Runner card whose paid ability
/// breaches a server directly (7.3.1's "card abilities can also directly
/// instruct the Runner to breach a server").
pub fn breach_button(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities =
        vec![AbilityDef::paid(Cost::free(), vec![Instruction::BreachServer(server)])
            .labeled("breach")];
    c
}

/// Archives Interface shape (7.3.6): "instead of accessing the chosen
/// candidate, remove it from the game."
pub fn archives_interface_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::Replacement {
                optional: false,
                applies_to: crate::effects::EffectClass::AccessCard,
                with: crate::lingering::ReplacementTransform::SuppressAccessAndRemoveChosen,
            },
            duration: crate::lingering::WantedDuration::ThisTurn,
        }],
    )
    .labeled("archives-interface: remove instead of accessing")];
    c
}

/// Hudson 1.0 shape (7.4.2b): a piece of ice whose subroutine creates the
/// run-bound "the Runner cannot access more than 1 card during this run"
/// prohibition.
pub fn hudson_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 3);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::CreateLingeringEffect {
        payload: crate::instr::LingeringSpec::AccessLimit { limit: Quantity::c(1) },
        duration: crate::lingering::WantedDuration::ThisRun,
    }])
    .labeled("hudson: no more than 1 access this run")];
    c
}

/// Clone Retirement shape (7.3.8): "when the Runner steals this agenda, the
/// Corp takes 1 bad publicity."
pub fn clone_retirement_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_agenda(name, 3, 1);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfStolen,
        vec![Instruction::TakeBadPublicity { side: Side::Corp, amount: Quantity::c(1) }],
        false,
    )
    .labeled("clone-retirement: the Corp takes 1 bad publicity")];
    c
}

/// Raymond Flint shape (7.3.8): "whenever the Corp takes bad publicity, you
/// may breach HQ."
pub fn raymond_flint_like(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerTakesBadPublicity(Side::Corp),
        vec![Instruction::DeclineableChoice(Box::new(Instruction::BreachServer(server)))],
        true,
    )
    .labeled("raymond: you may breach")];
    c
}

// ---------------------------------------------------------------------------
// W12b shapes: §4.8.3 set-aside passthrough, §9.1.8g, §10.9, §10.11
// ---------------------------------------------------------------------------

/// Test Run shape (4.8.3): "Search your stack or heap for a program and
/// install it." The search sets the program aside before installing it
/// (8.7.4), which is exactly what 4.8.3 says other abilities cannot see.
pub fn test_run_like(name: &'static str, zone: Zone) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::Search {
                zone,
                criteria: vec![crate::instr::TargetFilter::CardTypeIs(CardType::Program)],
                count: Quantity::c(1),
                may_fail: true,
            },
            Instruction::InstallCard {
                card: TargetSpec::FoundBySearch,
                dest: crate::instr::InstallDest::Rig,
                and_rez: false,
                ignore_costs: true,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            },
        ],
    )
    .labeled("test-run: search and install a program")];
    c
}

/// Exile shape (4.8.3): "Whenever you install a program from your heap, draw
/// 1 card." A condition that stipulates the zone the installed card came
/// from — the one kind of ability 4.8.3's passthrough is written for.
pub fn exile_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Runner, CardType::Identity);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::CardInstalledFrom { side: Side::Runner, from: Zone::Discard(Side::Runner) },
        vec![Instruction::Draw(Side::Runner, 1)],
        false,
    )
    .labeled("exile: draw when you install a program from your heap")];
    c
}

/// Test Run's second half (9.1.8g): "When your turn ends, add the installed
/// card to the top of your stack." An installed card, so the move makes it
/// inactive.
pub fn returns_program_at_turn_end(name: &'static str, program: ObjectId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::CreateDelayedConditional {
            def: Box::new(
                AbilityDef::conditional(
                    TriggerCond::TurnEnds(Side::Runner),
                    vec![Instruction::MoveToDeck {
                        card: TargetSpec::Objects(vec![program]),
                        top: true,
                    }],
                    false,
                )
                .labeled("test-run: add it to the top of your stack"),
            ),
            duration: crate::lingering::WantedDuration::UntilResolved,
        }],
    )
    .labeled("test-run: arm the return")];
    c
}

/// Nanuq shape (9.1.8g): "When Nanuq is added to your stack, remove it from
/// the game." The condition is met by the very move that makes the card
/// inactive, so the ability must stay active in the stack until it resolves.
pub fn nanuq_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfAddedToDeck,
        vec![Instruction::RemoveSelfFromGame],
        false,
    )
    .labeled("nanuq: remove it from the game")];
    c
}

/// Crowdfunding shape (10.9.2): "When you install this card, load it with 3
/// credits. Take 1 hosted credit: gain 1[credit]. When Crowdfunding is empty,
/// add it to your grip."
///
/// SIMPLIFICATION (§12 rule 3): the printed card takes its credit at the
/// beginning of the Runner's turn; here that is a free paid ability, because
/// what 10.9.2 is about is the LINK between the loading ability and the empty
/// ability, not when the counters come off.
pub fn crowdfunding_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfInstalled,
            vec![Instruction::LoadCounters {
                target: TargetSpec::SelfSource,
                kind: CounterKind::Credit,
                amount: Quantity::c(3),
            }],
            false,
        )
        .labeled("crowdfunding: load it with 3 credits"),
        AbilityDef::paid(
            Cost::spend_counters(CounterKind::Credit, 1),
            vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        )
        .labeled("crowdfunding: take 1 credit"),
        AbilityDef::conditional(
            TriggerCond::SelfEmpty { kind: CounterKind::Credit },
            vec![Instruction::AddCardsToHand { cards: TargetSpec::SelfSource }],
            false,
        )
        .labeled("crowdfunding: add it to your grip"),
    ];
    c
}

/// A Runner card whose paid ability simply makes a run on a server.
pub fn run_button(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::run(server)])
        .labeled("run-button: make a run")];
    c
}

/// Carpe Diem shape (10.11.2): "Identify your mark."
pub fn identify_mark_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::IdentifyMark])
        .labeled("carpe-diem: identify your mark")];
    c
}

/// Virtuoso shape (10.11.5): "The first time each turn you make a successful
/// run on your mark, access 1 additional card when you breach that server."
///
/// SIMPLIFICATION (§12 rule 3): "that server" is passed in rather than read
/// back from the mark, because the additional-access lingering effect names
/// one server and the caller already knows which server the mark is.
pub fn virtuoso_like(name: &'static str, server: ServerId) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SuccessfulRunOnMark { first_each_turn: true },
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::AdditionalAccess { server, extra: 1 },
            duration: crate::lingering::WantedDuration::ThisRun,
        }],
        false,
    )
    .labeled("virtuoso: access 1 additional card")];
    c
}

// ---------------------------------------------------------------------------
// W12c shapes: §9.12.2b aggregation, §9.9.6c cost interrupts
// ---------------------------------------------------------------------------

/// realloc() shape (9.12.2b): "For each rezzed card, gain 1[credit] and derez
/// a card." Two effects tied to ONE calculated quantity, and one of them —
/// derezzing — is not on 9.12.2c's list, so neither aggregates.
pub fn realloc_like(name: &'static str, count: Quantity) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Operation);
    c.cost = Some(0);
    c.abilities = vec![AbilityDef {
        kind: crate::ability::AbilityKind::Play,
        flags: Vec::new(),
        condition: None,
        cost: None,
        instructions: vec![Instruction::ForEach {
            count,
            effects: vec![
                Instruction::GainCredits(Side::Corp, Quantity::c(1)),
                Instruction::Derez {
                    target: TargetSpec::Choose {
                        count: Quantity::c(1),
                        criteria: vec![crate::instr::TargetFilter::Rezzed],
                    },
                },
            ],
        }],
        statics: Vec::new(),
        optional: false,
        timing: None,
        label: "realloc: gain and derez for each",
    }];
    c
}

/// NASX shape (9.12.2b): "Whenever you gain credits, place 1 power counter on
/// this card." A per-occurrence condition (9.6.4b) — which is how the example
/// counts the instances.
pub fn nasx_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerGainsCredits(Side::Corp),
        vec![Instruction::PlaceCounters {
            target: TargetSpec::SelfSource,
            kind: CounterKind::Power,
            amount: Quantity::c(1),
        }],
        false,
    )
    .labeled("nasx: place a power counter")];
    c
}

/// Patchwork shape (9.9.6c): a conditional INTERRUPT that decreases the cost
/// value of the imminent instruction. What the example is about is relevance
/// — the interrupt applies to any instruction where a card will be played or
/// installed and the corresponding cost paid, and to nothing else.
pub fn patchwork_interrupt(name: &'static str, less: i64) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    let mut a = AbilityDef::conditional(
        TriggerCond::WouldPayCost,
        vec![Instruction::ReduceImminentCost { amount: Quantity::c(less) }],
        true,
    )
    .labeled("patchwork: that cost is lower");
    a.flags.push(AbilityFlag::Interrupt);
    c.abilities = vec![a];
    c
}

/// Street Peddler shape (9.5.5): "[trash]: Install 1 of the hosted cards,
/// ignoring install costs." The trigger cost uninstalls the source, so its
/// hosted cards are set aside as the cost is paid (9.5.5) and this ability —
/// and only this ability — can still address them.
pub fn street_peddler_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::CanHost {
            criteria: Vec::new(),
            capacity: None,
        }])
        .labeled("peddler: can host cards"),
        AbilityDef::paid(
            Cost::trash_self(),
            vec![Instruction::InstallCard {
                card: TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![crate::instr::TargetFilter::SetAsideByThisAbility],
                },
                dest: crate::instr::InstallDest::Rig,
                and_rez: false,
                ignore_costs: true,
                reveal_check: None,
                reduce_total: Quantity::c(0),
            }],
        )
        .labeled("peddler: install one of the hosted cards"),
    ];
    c
}

// ---------------------------------------------------------------------------
// W12d shapes: §9.8 subroutine origins, order declarations, replacements
// ---------------------------------------------------------------------------

/// Loki shape (9.8.3a): "When the Runner encounters this ice, choose another
/// rezzed piece of ice. This ice gains the subroutines of that ice before its
/// other subroutines." ONE effect granting SEVERAL subroutines, which 9.8.3a
/// orders among themselves in the order they had on the card they came from.
pub fn loki_like(name: &'static str, printed_sub: AbilityDef) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 5);
    c.abilities = vec![
        AbilityDef::conditional(
            TriggerCond::SelfEncountered,
            vec![Instruction::GrantSubroutines {
                to: TargetSpec::SelfSource,
                grant: crate::instr::SubroutineGrant::CopiedFrom(TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![
                        crate::instr::TargetFilter::Rezzed,
                        crate::instr::TargetFilter::CardTypeIs(CardType::Ice),
                        crate::instr::TargetFilter::OtherThanSource,
                    ],
                }),
                before: true,
                any_order: false,
                duration: crate::lingering::WantedDuration::ThisEncounter,
            }],
            false,
        )
        .labeled("loki: gain the subroutines of that ice"),
        printed_sub,
    ];
    c
}

/// Merlin shape (9.8.2c): "Reveal this card from HQ to give a piece of ice
/// 1 subroutine, in the order of your choice." The duration is a parameter
/// because 9.8.2c is about the DECLARATION, not about how long the granted
/// subroutine lasts.
pub fn any_order_granter(
    name: &'static str,
    ice: ObjectId,
    sub: AbilityDef,
    duration: crate::lingering::WantedDuration,
) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::GrantSubroutines {
            to: TargetSpec::Objects(vec![ice]),
            grant: crate::instr::SubroutineGrant::Stated { count: 1, sub: Box::new(sub) },
            before: false,
            any_order: true,
            duration,
        }],
    )
    .labeled("merlin: add a subroutine in any order")];
    c
}

/// Chronos Protocol shape (10.4.3a / 9.12.1c): an identity that (i) declares
/// the Corp selects 1 of the cards trashed by damage and (ii) has the Corp
/// LOOK at the Runner's grip whenever the Runner suffers damage. The two are
/// separate abilities on purpose: 9.12.1c says the effect that granted the
/// choice "otherwise resolves as normal" even when the other player's
/// declaration wins the choice.
///
/// SIMPLIFICATION (§12 rule 3): the printed "first time each turn" limit is
/// elided; no example here damages twice in a turn.
pub fn chronos_protocol_like(name: &'static str) -> PrintedCard {
    let mut c = PrintedCard::vanilla(name, Side::Corp, CardType::Identity);
    c.abilities = vec![
        AbilityDef::static_ability(vec![StaticDecl::SelectsDamageTrashes {
            by: Side::Corp,
            count: Quantity::c(1),
        }])
        .labeled("chronos: the Corp chooses the first card trashed"),
        AbilityDef::conditional(
            TriggerCond::RunnerSuffersDamage,
            vec![Instruction::LookAtCards {
                cards: TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![crate::instr::TargetFilter::CardsInHandOf(Side::Runner)],
                },
                by: Side::Corp,
            }],
            false,
        )
        .labeled("chronos: look at the grip"),
    ];
    c
}

/// Titanium Ribs shape (9.12.1c): "You choose the cards you trash to damage."
pub fn titanium_ribs_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Hardware);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::SelectsDamageTrashes {
        by: Side::Runner,
        count: Quantity::c(9),
    }])
    .labeled("ribs: you choose the cards you trash")];
    c
}


/// Bravado shape (1.12.6): "Make a run. When that run ends, gain 1[credit]
/// for each piece of ice you passed during it." The count is a game-HISTORY
/// query, so an ice that no longer exists still counts.
///
/// SIMPLIFICATION (§12 rule 3): the printed card makes the run itself and
/// arms the counting as a delayed conditional; here the run comes from the
/// basic run action and the counting is a plain conditional on the same
/// card, because 9.6.13d would refuse to create a delayed conditional armed
/// by the very ability that initiates the run (no run is in progress yet).
pub fn bravado_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::RunEnds { successful_only: false },
        vec![Instruction::GainCredits(Side::Runner, Quantity::DistinctIcePassedThisRun)],
        false,
    )
    .labeled("bravado: 1 credit per ice passed")];
    c
}

/// Precognition shape (1.12.3): "Rearrange the top cards of R&D." Cards moved
/// to an unknown location within their zone become NEW objects.
pub fn precognition_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities =
        vec![AbilityDef::paid(Cost::free(), vec![Instruction::CorpRearrangesRnd])
            .labeled("precognition: rearrange the top of R&D")];
    c
}

// ---------------------------------------------------------------------------
// W13a shapes: cost payment as a procedure (§1.16)
// ---------------------------------------------------------------------------

/// Biawak shape (1.16.2e): a piece of ice with a large rez cost and an
/// ALTERNATE way to pay part of it — "You can forfeit 1 agenda as you rez
/// this ice to pay for N[credit] of its rez cost." The declaration is a
/// static ability of the ice, and 9.1.8d keeps it active while the ice is
/// still unrezzed, which is the only moment it matters.
pub fn alternate_payment_ice(name: &'static str, rez: u32, covers: u32) -> PrintedCard {
    let mut c = vanilla_ice(name, rez, 4);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::AlternatePaymentForSelf {
            label: "forfeit 1 agenda toward the rez cost",
            covers,
            instead: Cost::forfeit_agenda(1),
        },
    ])
    .labeled("alternate payment: forfeit an agenda")];
    c
}

/// Mahkota Langit Grid shape (1.10.3c): an upgrade holding credits its own
/// ability lets the Corp spend. Credits hosted on a card are one of the
/// "allowed locations" a payer divides a payment among.
pub fn hosted_credit_upgrade(name: &'static str) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.hosted_credits_spendable = true;
    c
}

/// Psychographics shape (1.16.2c): an operation whose play cost is X, with
/// the ability "X must be equal to or less than the number of tags the
/// Runner has". The restriction is a quantity position (§12 rule 6), so the
/// legal announcements are exactly `0..=that`.
pub fn cost_x_operation(name: &'static str, instrs: Vec<Instruction>) -> PrintedCard {
    let mut c = operation(name, 0, instrs);
    c.cost = None;
    c.cost_x = Some(Quantity::RunnerTags);
    c
}

/// Azef Protocol shape (1.16.1c / 1.16.10c): an agenda with an ADDITIONAL
/// COST TO SCORE of "trash 1 of your other installed cards". Scoring
/// normally costs nothing, so 1.16.10c's checkpoint after paying is the only
/// checkpoint between the decision to score and the agenda moving.
pub fn additional_score_cost_agenda(name: &'static str, req: u32, points: i32) -> PrintedCard {
    let mut c = vanilla_agenda(name, req, points);
    c.additional_score_cost = Some(Cost::trash_matching(
        1,
        vec![crate::instr::TargetFilter::OtherThanSource],
    ));
    c
}

/// Ob Superheavy Logistics shape (1.16.10c): a Corp card whose conditional
/// ability meets its condition when the Corp trashes an installed card of
/// their own. Placing a counter on itself is the observable resolution.
pub fn trash_reaction_asset(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::InstalledCardTrashed { side: Side::Corp, of_types: Vec::new() },
        vec![Instruction::PlaceCounters {
            target: TargetSpec::SelfSource,
            kind: CounterKind::Power,
            amount: Quantity::c(1),
        }],
        false,
    )
    .labeled("ob: when you trash an installed card")];
    c
}

// ---------------------------------------------------------------------------
// W13b shapes: "If successful" (§6.7.4)
// ---------------------------------------------------------------------------

/// Because I Can shape (6.7.4a): "Run a remote server. If successful, <gain
/// credits>." The instruction carries the SET of servers the effect allowed,
/// so a run moved to a central drops the clause and a run moved to another
/// remote keeps it.
pub fn because_i_can_like(name: &'static str, server: ServerId, gain: i64) -> PrintedCard {
    event(
        name,
        0,
        vec![Instruction::InitiateRun {
            server: Some(server),
            allowed: crate::instr::RunServerSet::AnyRemote,
            if_successful: vec![Instruction::GainCredits(Side::Runner, Quantity::c(gain))],
        }],
    )
}

/// Account Siphon shape (6.7.4c): "Run HQ. If successful, you may instead of
/// breaching HQ, force the Corp to lose credits." The "instead of breaching"
/// part is an OPTIONAL replacement effect, so the Runner's decision is made
/// where the breach would begin (step 6.9.5b) — after everything the 6.9.5a
/// reaction window held has resolved.
pub fn account_siphon_like(name: &'static str, gain: u32) -> PrintedCard {
    event(
        name,
        0,
        vec![Instruction::InitiateRun {
            server: Some(ServerId::Hq),
            allowed: crate::instr::RunServerSet::These(vec![ServerId::Hq]),
            if_successful: vec![Instruction::CreateLingeringEffect {
                payload: crate::instr::LingeringSpec::Replacement {
                    applies_to: crate::effects::EffectClass::Breach,
                    with: crate::lingering::ReplacementTransform::SuppressAndGainCredits(gain),
                    optional: true,
                },
                duration: crate::lingering::WantedDuration::ThisRun,
            }],
        }],
    )
}

/// Ash 2X3ZB9CY shape as the 6.7.4c example uses it: an upgrade whose trace
/// resolves in the reaction window at step 6.9.5a, i.e. BEFORE the Runner has
/// to decide whether to breach.
pub fn successful_run_trace_upgrade(name: &'static str, base: i64) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SuccessfulRunOnServer,
        vec![Instruction::Trace {
            base: Quantity::c(base),
            if_successful: vec![Instruction::RestrictAccessToSelf],
            if_unsuccessful: vec![],
            determined_min: None,
        }],
        false,
    )
    .labeled("ash: trace when the run is successful")];
    c
}

// ---------------------------------------------------------------------------
// W13c shapes: additional costs on the basic run action (§6.3.4, §9.12.3e)
// ---------------------------------------------------------------------------

/// Enhanced Login Protocol / Service Outage shape (6.3.4 / 1.16.10): "the
/// Runner must pay [cost] as an additional cost to make a run."
pub fn run_surcharge_asset(name: &'static str, extra: Cost) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::AdditionalRunActionCost(
        extra,
    )])
    .labeled("surcharge: additional cost to make a run")];
    c
}

/// Heinlein Grid shape (6.3.4): "whenever the Runner spends [click] during a
/// run, they lose all of their credits." The additional [click] charged to
/// MAKE a run is spent before the run formally begins, so it never meets this.
pub fn heinlein_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_upgrade(name, 0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerSpendsClick { side: Side::Runner, during_run: true },
        vec![Instruction::LoseCredits(Side::Runner, 99)],
        false,
    )
    .labeled("heinlein: lose all credits")];
    c
}

/// Always Be Running shape (9.12.3a/e): "You must make a run with your first
/// [click] each turn."
pub fn always_be_running_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::MustRunWithFirstClick(
        Side::Runner,
    )])
    .labeled("abr: you must run with your first click")];
    c
}

/// CR 5.2.1a: a nested cost containing a [click] symbol without denoting an
/// action — "End the run unless the Runner spends [click]". This is how a
/// [click] actually gets spent DURING a run, since an action (5.2.1) cannot
/// be taken inside one.
pub fn etr_unless_click_ice(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::NestedCostUnless {
        cost: Cost { clicks: 1, ..Cost::free() },
        effect: Box::new(Instruction::EndTheRun),
        payer: Some(Side::Runner),
    }])
    .labeled("[sub] ETR unless the runner spends a click")];
    c
}

/// Project Vacheron shape (9.9.9c): an agenda whose interrupt ability creates
/// a replacement effect overriding "add this agenda to your score area" with
/// "add it to your score area with N hosted agenda counters". The Runner still
/// steals it — the replacement's result still includes the effect it replaced
/// — and the replacement cannot apply again to its own result.
pub fn vacheron_like(name: &'static str, points: i32, counters: u32) -> PrintedCard {
    let mut c = vanilla_agenda(name, 3, points);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::WouldStealSelfAgenda,
        vec![Instruction::CreateLingeringEffect {
            payload: crate::instr::LingeringSpec::Replacement {
                applies_to: crate::effects::EffectClass::StealAgenda,
                with: crate::lingering::ReplacementTransform::StealWithHostedCounters {
                    kind: CounterKind::Agenda,
                    amount: counters,
                },
                optional: false,
            },
            duration: crate::lingering::WantedDuration::ThisRun,
        }],
        false,
    )
    .with_flag(AbilityFlag::Interrupt)
    .with_flag(AbilityFlag::Access)
    .labeled("vacheron: stolen with hosted agenda counters")];
    c
}

// ---------------------------------------------------------------------------
// W13e shapes: action identity (§5.2.5) and clicks spent to take an action
// ---------------------------------------------------------------------------

/// MirrorMorph shape (5.2.5b): "the first time each turn you take N DIFFERENT
/// actions, gain 1[credit]." Two plays of two different operations are still
/// the SAME action — the basic "Play 1 operation from HQ" — so they do not
/// make the identity's condition true.
pub fn mirrormorph_like(name: &'static str, count: usize) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::DifferentActionsThisTurn { side: Side::Corp, count },
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("mirrormorph: different actions")];
    c
}

/// Jeeves Model Bioroids shape (1.16.4d): "the first time each turn you spend
/// N [click] on the same action, gain 1[credit]." The clicks counted include
/// the ones an additional cost takes, several steps into the action.
pub fn jeeves_like(name: &'static str, count: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::ClicksSpentOnAction { side: Side::Corp, count },
        vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
        false,
    )
    .labeled("jeeves: clicks on one action")];
    c
}

/// Blue Level Clearance shape (1.16.4d): an operation with an ADDITIONAL play
/// cost of 1 [click].
pub fn additional_click_operation(name: &'static str, cost: u32) -> PrintedCard {
    let mut c = operation(name, cost, vec![Instruction::GainCredits(Side::Corp, Quantity::c(2))]);
    c.additional_play_cost = Some(Cost { clicks: 1, ..Cost::free() });
    c
}

/// Accelerated Beta Test shape (1.12.3): "Look at the top N cards of R&D …
/// trash the cards you are looking at." Between the two instructions there is
/// a checkpoint, so a chain reaction can shuffle R&D out from under it — and
/// 1.12.3 makes those cards NEW objects, which this ability can no longer act
/// on.
pub fn abt_like(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::LookAtCards {
                cards: TargetSpec::Choose {
                    count: Quantity::c(n as i64),
                    criteria: vec![crate::instr::TargetFilter::TopOfDeckOf { side: Side::Corp, n }],
                },
                by: Side::Corp,
            },
            Instruction::GainCredits(Side::Corp, Quantity::c(1)),
            Instruction::TrashCards(TargetSpec::Choose {
                count: Quantity::c(n as i64),
                criteria: vec![crate::instr::TargetFilter::LookedAtByThisAbility],
            }),
        ],
    )
    .labeled("abt: look at the top of R&D, then trash them")];
    c
}

/// The Foundry shape (1.12.3): a chain-reaction ability that shuffles R&D —
/// the cards another ability is looking at go to an unknown location.
pub fn shuffle_on_credit_asset(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerGainsCredits(Side::Corp),
        vec![Instruction::CorpRearrangesRnd],
        false,
    )
    .labeled("foundry: shuffle R&D")];
    c
}

// ---------------------------------------------------------------------------
// §8.4 — the drawing procedure, and the drawn set as a facedown group
// ---------------------------------------------------------------------------

/// A Corp card whose paid ability is nothing but "Draw N cards" — the
/// simplest way to put the 8.4.5 draw procedure under a plan's control.
pub fn draw_button(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::Draw(Side::Corp, n)])
        .labeled("draw-button: draw cards")];
    c
}

/// Daily-Business-Show shape (4.8.7 / 8.4.3a): "Whenever you draw 1 or more
/// cards, add 1 of the drawn cards to the bottom of R&D." The ability resolves
/// at the 8.4.5b checkpoint, while the drawn cards are still set aside
/// facedown, which is what lets it choose among them (8.4.2a) and what makes
/// which card went where hidden information (10.2.2a).
///
/// Simplification: the printed card also draws the additional card itself; the
/// shape leaves the size of the draw to whatever drew, since the rules under
/// test are about the drawn SET.
pub fn daily_business_show_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerDrawsCards(Side::Corp),
        vec![Instruction::MoveToDeck {
            card: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::DrawnCards],
            },
            top: false,
        }],
        false,
    )
    .labeled("dbs: put a drawn card on the bottom of R&D")];
    c
}

/// Raman-Rai shape (8.4.3b): "Whenever you draw cards, you may swap a card you
/// just drew with a card in Archives." The card swapped INTO the set-aside
/// zone is now considered drawn and is added to HQ with the rest.
///
/// Simplification: the printed card's [click] cost and its reveal are left
/// off; the rule under test is what the swap does to the drawn set.
pub fn raman_rai_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PlayerDrawsCards(Side::Corp),
        vec![Instruction::SwapCards {
            a: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::DrawnCards],
            },
            b: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::InDiscardOf(Side::Corp)],
            },
        }],
        false,
    )
    .labeled("raman-rai: swap a drawn card with one in Archives")];
    c
}

/// A Corp card that draws when a breach ends — the timing CR 4.8.7's example
/// needs, where the previously-accessed card is drawn once the breach in which
/// it was accessed is over, so 7.3.1a's visibility has already lapsed.
pub fn draw_on_breach_end(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::BreachEnds,
        vec![Instruction::Draw(Side::Corp, n)],
        false,
    )
    .labeled("breach-draw: draw when a breach ends")];
    c
}

// ---------------------------------------------------------------------------
// §8.3 — arranging cards
// ---------------------------------------------------------------------------

/// Indexing shape (8.3.3): "Look at the top N cards of R&D. Rearrange them."
/// The Runner arranges an OPPONENT'S deck, so 8.3.3a keeps the Corp from
/// seeing the set-aside cards, and 8.3.3's "secretly puts them in the order of
/// their choice" leaves the Runner — and only the Runner — knowing what is
/// where.
pub fn indexing_like(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::SetAsideTopOfDeck { deck_of: Side::Corp, count: Quantity::c(n as i64) },
            Instruction::ArrangeSetAside { to_top_of: Side::Corp },
        ],
    )
    .labeled("indexing: rearrange the top of R&D")];
    c
}

/// Cultivate shape (8.3.3b): "Look at the top N cards of R&D. Trash 1 card,
/// add 1 card to HQ, and arrange the rest in any order." The other effects are
/// performed while the cards are set aside, and the Corp "does not declare
/// which cards are acted on".
pub fn cultivate_like(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    let set_aside = |k: u32| TargetSpec::Choose {
        count: Quantity::c(k as i64),
        criteria: vec![crate::instr::TargetFilter::SetAsideByThisAbility],
    };
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![
            Instruction::SetAsideTopOfDeck { deck_of: Side::Corp, count: Quantity::c(n as i64) },
            Instruction::TrashCards(set_aside(1)),
            Instruction::AddCardsToHand { cards: set_aside(1) },
            Instruction::ArrangeSetAside { to_top_of: Side::Corp },
        ],
    )
    .labeled("cultivate: trash 1, add 1 to HQ, arrange the rest")];
    c
}

/// A Corp card whose paid ability installs a chosen card from HQ in a new
/// remote server — the "install a card in an empty remote" half of CR
/// 10.2.2b's bluffing example.
pub fn install_from_hq_button(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::InstallCard {
            card: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::CardsInHandOf(Side::Corp)],
            },
            dest: crate::instr::InstallDest::NewRemoteRoot,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
            reduce_total: Quantity::c(0),
        }],
    )
    .labeled("install-hq: install a card from HQ in a new remote")];
    c
}

/// Formicary shape (6.8.2c): "Whenever the Runner approaches a server, you may
/// rez this ice, if it is unrezzed, and move it to the outermost position
/// protecting that server. The Runner is now approaching this ice."
///
/// The destination server is a parameter here because `InstallDest` names a
/// server and the examples run one; the printed card names the server just
/// approached.
pub fn formicary_like(name: &'static str, to: ServerId) -> PrintedCard {
    let mut c = etr_ice(name, 0, 1);
    c.abilities.push(
        AbilityDef::conditional(
            TriggerCond::ServerApproached,
            vec![
                Instruction::RezCard { target: TargetSpec::SelfSource, ignore_costs: false },
                Instruction::MoveIce {
                    ice: TargetSpec::SelfSource,
                    dest: crate::instr::InstallDest::Protecting(to),
                },
                Instruction::MoveRunnerToIce {
                    ice: TargetSpec::SelfSource,
                    encounter: true,
                },
            ],
            true,
        )
        .labeled("formicary: rez and move to the approached server"),
    );
    c
}

/// A Runner card with "Whenever the Runner approaches a server, end the run."
/// — the effect CR 6.8.2c's example needs to end a run from inside the
/// reaction window that follows step 6.9.4g.
pub fn end_run_on_server_approach(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Resource);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::ServerApproached,
        vec![Instruction::EndTheRun],
        false,
    )
    .labeled("approach-etr: end the run when a server is approached")];
    c
}

// ---------------------------------------------------------------------------
// §8.2.2 / §9.1.8b — replacing a movement's destination
// ---------------------------------------------------------------------------

/// Harbinger shape (8.2.2): "If this card would be trashed while it is
/// installed, instead turn it facedown." A static ability stipulating a
/// replacement (9.9.8b) that modifies the trash movement WITHOUT replacing it
/// by name — the card is still trashed, so a Wasteland-class condition about
/// trashing an installed card is still met.
pub fn harbinger_facedown_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(0);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::ReplaceTrashDestination {
            criteria: vec![
                crate::instr::TargetFilter::IsSource,
                crate::instr::TargetFilter::InstalledRunnerCard,
            ],
            to: crate::instr::TrashDestination::FacedownInPlay,
        },
    ])
    .labeled("harbinger: turn facedown instead of going to the heap")];
    c
}

/// Skorpios-Defense-Systems shape (9.1.8b): a Corp static ability replacing
/// where a card trashed from the Runner's grip goes — "remove it from the game
/// instead of adding it to the heap".
///
/// Simplification: the printed card's once-per-turn limit and its wider scope
/// are left off; the rule under test is what the replaced destination does to
/// 9.1.8b's zone stipulation.
pub fn skorpios_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::ReplaceTrashDestination {
            criteria: vec![crate::instr::TargetFilter::CardsInHandOf(Side::Runner)],
            to: crate::instr::TrashDestination::RemovedFromGame,
        },
    ])
    .labeled("skorpios: remove trashed grip cards from the game")];
    c
}

/// I've-Had-Worse shape (9.1.8b): a Runner event with "When this card is
/// trashed by damage, …". The condition can only ever be met by the card
/// moving from the grip to the heap, so 9.1.8b keeps the ability active in the
/// heap — and nowhere else.
///
/// Simplification: the printed card draws 3; gaining credits is the same
/// occurrence and is what the example's assertion is about.
pub fn ive_had_worse_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Event);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfTrashedByDamage,
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(3))],
        false,
    )
    .labeled("ihw: when trashed by damage")];
    c
}

// ---------------------------------------------------------------------------
// §9.8.9 / §6.1.3e-f — replaced subroutines, and passing "after" a phase
// ---------------------------------------------------------------------------

/// Bloop shape: a piece of ice with three subroutines, each gaining the Corp
/// a credit — distinguishable from the 9.8.9 replacement, which does damage.
pub fn three_sub_ice(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = (0..3)
        .map(|i| {
            let label: &'static str =
                Box::leak(format!("[sub] bloop {i}: gain 1").into_boxed_str());
            AbilityDef::subroutine(vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))])
                .labeled(label)
        })
        .collect();
    c
}

/// Tsakhia "Bankhar" Gantulga shape (9.8.9): a Runner card whose static
/// ability replaces every imminent subroutine with "[subroutine] Do 1 net
/// damage." The replaced subroutine still resolves FROM the ice.
pub fn bankhar_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(0);
    c.abilities = vec![AbilityDef::static_ability(vec![
        StaticDecl::ReplaceSubroutineResolution {
            instead: vec![Instruction::Damage {
                kind: DamageKind::Net,
                amount: Quantity::c(1),
                responsible: Side::Corp,
            }],
        },
    ])
    .labeled("bankhar: resolve net damage instead of any subroutine")];
    c
}

/// Persephone shape (9.8.9): "Whenever you pass a piece of ice, if any of its
/// subroutines resolved during that encounter, …". Gains credits here; the
/// printed card trashes cards, which is the same occurrence.
pub fn persephone_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PassedIceWithResolvedSubroutines,
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(2))],
        false,
    )
    .labeled("persephone: subroutines resolved from the ice just passed")];
    c
}

/// Inversificator shape (6.1.3f): "Whenever you pass a piece of ice you fully
/// broke during that encounter, …". The scope is the encounter the pass
/// directly follows (6.1.3e).
pub fn inversificator_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_runner_card(name, CardType::Program);
    c.memory_cost = Some(0);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::PassedIceAfterFullyBreaking,
        vec![Instruction::GainCredits(Side::Runner, Quantity::c(1))],
        false,
    )
    .labeled("inversificator: passed an ice fully broken that encounter")];
    c
}

/// Mirāju shape (6.1.3e): a piece of ice with "When the encounter with this
/// ice ends, you may move the Runner to the outermost position protecting this
/// server." Moving the Runner to another position changes the run's timing
/// point (6.1.3d/6.2.8a), so the Movement Phase's pass step never happens —
/// "because the ice is not passed", conditions about passing it are not met.
pub fn miraju_like(name: &'static str) -> PrintedCard {
    // The printed card's subroutine is elided: an "end the run" would stop the
    // run before the second pass this example is about, and nothing 6.1.3e/f
    // measures depends on which subroutine the ice has.
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::GainCredits(
        Side::Corp,
        Quantity::c(1),
    )])
    .labeled("[sub] miraju: gain 1")];
    c.abilities.push(
        AbilityDef::conditional(
            TriggerCond::EncounterEnds,
            vec![Instruction::MoveRunnerToIce {
                ice: TargetSpec::Choose {
                    count: Quantity::c(1),
                    criteria: vec![
                        crate::instr::TargetFilter::IceProtectingSourceServer,
                        crate::instr::TargetFilter::OtherThanSource,
                    ],
                },
                encounter: false,
            }],
            true,
        )
        .labeled("miraju: move the Runner to another position on this server"),
    );
    c
}

/// Wormhole shape (10.1.6a): a piece of ice with "[subroutine] Resolve a
/// subroutine on another rezzed piece of ice." Two of them create a MANDATORY
/// infinite loop — each one's subroutine resolves the other's, and no player
/// has a choice anywhere in it.
pub fn wormhole_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 0, 1);
    c.abilities = vec![AbilityDef::subroutine(vec![Instruction::ResolveAbilityOf {
        source: TargetSpec::Choose {
            count: Quantity::c(1),
            criteria: vec![
                crate::instr::TargetFilter::CardTypeIs(CardType::Ice),
                crate::instr::TargetFilter::Rezzed,
                crate::instr::TargetFilter::OtherThanSource,
            ],
        },
        which: crate::ability::AbilityClass::Subroutine(0),
    }])
    .labeled("[sub] wormhole: resolve a subroutine on another rezzed ice")];
    c
}

/// Trick-of-Light shape (1.15.1): "Choose 1 installed card you can advance.
/// Move up to 2 advancement counters from 1 other card to the chosen card."
/// The targets are the COUNTERS and the destination card; the card the
/// counters come from is not a target.
pub fn trick_of_light_like(name: &'static str, n: u32) -> PrintedCard {
    let mut c = vanilla_asset(name, 0, 3);
    c.abilities = vec![AbilityDef::paid(
        Cost::free(),
        vec![Instruction::MoveCounters {
            kind: CounterKind::Advancement,
            count: Quantity::c(n as i64),
            up_to: true,
            to: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![crate::instr::TargetFilter::InstalledCorpCard],
            },
            from_criteria: vec![crate::instr::TargetFilter::InstalledCorpCard],
        }],
    )
    .labeled("trick-of-light: move advancement counters")];
    c
}
