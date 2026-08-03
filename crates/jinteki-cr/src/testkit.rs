//! Test support: minimal card definitions and script drivers for the CR
//! example suite and the playable-slice test. The real card layer arrives in
//! a later wave; these builders exist so kernel tests can express the CR's
//! worked examples with faithful minimal cards.

use crate::ability::{
    AbilityDef, AbilityFlag, Condition, Cost, StaticCond, StaticDecl, TimingRestriction,
    TriggerCond,
};
use crate::decision::{DecisionAnswer, DecisionSpec, WindowOption, Yield};
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
    vm.st.ice.entry(server).or_default().push(id);
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

/// Host `guest` on `host` (both must exist).
pub fn host_on(vm: &mut Vm, guest: ObjectId, host: ObjectId) {
    vm.st.objects.get_mut(&guest).unwrap().host = Some(host);
    vm.st.objects.get_mut(&host).unwrap().hosted.push(guest);
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
        TriggerCond::RunnerTrashesAtLeastOneCorpCard,
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
                count: 1,
                filter: crate::instr::TargetFilter::InstalledResource,
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
        TriggerCond::SelfAccessed,
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
        TriggerCond::SelfAccessed,
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
        vec![Instruction::PumpStrengthSelf { amount: 2 }],
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
        vec![Instruction::PlaceCounters {
            target: TargetSpec::Objects(vec![target]),
            kind: crate::object::CounterKind::Advancement,
            amount: 1,
        }],
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
                count: 1,
                filter: crate::instr::TargetFilter::InstalledCorpCard,
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
        .with_timing(TimingRestriction::EncounterOnly)
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
        vec![Instruction::DeclineableChoice(Box::new(Instruction::InitiateRun(server)))],
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
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::BreakSubroutines { count: 1 }])
        .with_timing(TimingRestriction::EncounterOnly)
        .labeled("break: 1 subroutine")];
    c
}

/// Brainstorm shape: "When the Runner encounters this ice, it gains 2
/// '[sub] Do 1 core damage.' subroutines." (category 9.8.3e.)
pub fn brainstorm_like(name: &'static str) -> PrintedCard {
    let mut c = vanilla_ice(name, 9, 4);
    c.abilities = vec![AbilityDef::conditional(
        TriggerCond::SelfEncountered,
        vec![Instruction::GrantSubroutinesToSelf {
            count: 2,
            sub: Box::new(
                AbilityDef::subroutine(vec![Instruction::Damage {
                    kind: DamageKind::Core,
                    amount: Quantity::c(1),
                    responsible: Side::Corp,
                }])
                .labeled("[sub] 1 core"),
            ),
            before: false,
        }],
        false,
    )
    .labeled("brainstorm: gain core subs")];
    c
}

/// Inject a breach-replacement lingering effect (Security Testing / Account
/// Siphon / Showing Off class), turn-bound.
pub fn inject_breach_replacement(
    vm: &mut Vm,
    source: ObjectId,
    transform: crate::lingering::ReplacementTransform,
) {
    let id = vm.next_lingering_id();
    vm.lingering.push(crate::lingering::LingeringEffect {
        id,
        source,
        payload: crate::lingering::Payload::ReplacementEffect {
            applies_to: crate::effects::EffectClass::Breach,
            replace_with: transform,
        },
        duration: crate::lingering::Duration::Turn(vm.st.turn_seq),
        applied_to: Vec::new(),
    });
}

/// Grant an external subroutine to a piece of ice ahead of time (Marker
/// class): creates the lingering effect directly.
pub fn grant_external_sub(
    vm: &mut Vm,
    ice: ObjectId,
    sub: AbilityDef,
    before: bool,
    run_bound: bool,
) {
    let id = vm.next_lingering_id();
    let duration = if run_bound {
        crate::lingering::Duration::Run(vm.current_run.map(|(r, _, _)| r).unwrap_or(0))
    } else {
        crate::lingering::Duration::Turn(vm.st.turn_seq)
    };
    vm.lingering.push(crate::lingering::LingeringEffect {
        id,
        source: ice,
        payload: crate::lingering::Payload::GrantedSubroutine { to: ice, sub, before, seq: id },
        duration,
        applied_to: Vec::new(),
    });
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
        TriggerCond::SelfAccessed,
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
                    count: 1,
                    filter: crate::instr::TargetFilter::InstalledRunnerCard,
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
    c.abilities = vec![AbilityDef::static_ability(vec![StaticDecl::HostsPrograms {
        capacity: 1,
        install_discount: 1,
    }])
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
        TriggerCond::SelfAccessed,
        vec![Instruction::InstallCard {
            card: TargetSpec::Objects(vec![installee]),
            dest: crate::instr::InstallDest::BreachedServerRoot,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
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
            TriggerCond::SelfAccessedIfRunnerTagged,
            vec![Instruction::GainCredits(Side::Corp, Quantity::c(1))],
            false,
        )
        .labeled("qpm: if tagged when accessed"),
        AbilityDef::conditional(
            TriggerCond::SelfAccessed,
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
            Instruction::InitiateRun(server),
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

/// Inject a Chum-class delayed conditional: "when this encounter ends, do 3
/// net damage." (one-shot).
pub fn inject_chum_delayed(vm: &mut Vm, source: ObjectId) {
    let id = vm.next_lingering_id();
    vm.lingering.push(crate::lingering::LingeringEffect {
        id,
        source,
        payload: crate::lingering::Payload::DelayedConditional {
            def: AbilityDef::conditional(
                TriggerCond::EncounterEnds,
                vec![Instruction::Damage {
                    kind: DamageKind::Net,
                    amount: Quantity::c(3),
                    responsible: Side::Corp,
                }],
                false,
            )
            .labeled("chum-delayed: 3 net when encounter ends"),
        },
        duration: crate::lingering::Duration::UntilResolved,
        applied_to: Vec::new(),
    });
}

/// Inject a Noble-Path-class prevent-all-damage shield bound to the current
/// run (6.8.5: expires at step 6.9.6d).
pub fn inject_run_damage_shield(vm: &mut Vm, source: ObjectId) {
    let run_id = vm.current_run.map(|(r, _, _)| r).expect("a run in progress");
    let id = vm.next_lingering_id();
    vm.lingering.push(crate::lingering::LingeringEffect {
        id,
        source,
        payload: crate::lingering::Payload::DamagePreventionAll,
        duration: crate::lingering::Duration::Run(run_id),
        applied_to: Vec::new(),
    });
}

// ---------------------------------------------------------------------------
// W3e shapes: candidates (7.4.3, 7.4.7a)
// ---------------------------------------------------------------------------

/// Inject an Immolation-Script-class access replacement: "instead of
/// accessing the chosen card, trash <victim>" (turn-bound).
pub fn inject_access_replacement(vm: &mut Vm, source: ObjectId, victim: ObjectId) {
    let id = vm.next_lingering_id();
    vm.lingering.push(crate::lingering::LingeringEffect {
        id,
        source,
        payload: crate::lingering::Payload::ReplacementEffect {
            applies_to: crate::effects::EffectClass::AccessCard,
            replace_with: crate::lingering::ReplacementTransform::SuppressAccessAndTrashOther(
                victim,
            ),
        },
        duration: crate::lingering::Duration::Turn(vm.st.turn_seq),
        applied_to: Vec::new(),
    });
}

/// Inject a Maker's-Eye-class additional-access effect (turn-bound).
pub fn inject_additional_access(vm: &mut Vm, server: ServerId, extra: u32) {
    let id = vm.next_lingering_id();
    vm.lingering.push(crate::lingering::LingeringEffect {
        id,
        source: ObjectId(0),
        payload: crate::lingering::Payload::AdditionalAccess { server, extra },
        duration: crate::lingering::Duration::Turn(vm.st.turn_seq),
        applied_to: Vec::new(),
    });
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
        vec![Instruction::MoveToTopOfRnd { card: TargetSpec::Objects(vec![card]) }],
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
// Script drivers
// ---------------------------------------------------------------------------

/// Step until the next Decision (panics on Progressed-forever or GameOver).
pub fn until_decision(vm: &mut Vm) -> (Side, DecisionSpec) {
    loop {
        match vm.step() {
            Yield::Decision(s, d) => return (s, d),
            Yield::Progressed => continue,
            Yield::GameOver(r) => panic!("unexpected game over: {r:?}"),
        }
    }
}

/// Step until GameOver (auto-passing every window, keeping mandatory
/// obligations: triggers the first mandatory option when passing is illegal).
pub fn drain_to_game_over(vm: &mut Vm, max_decisions: usize) -> crate::decision::GameResult {
    for _ in 0..max_decisions {
        match vm.step() {
            Yield::GameOver(r) => return r,
            Yield::Progressed => continue,
            Yield::Decision(_, spec) => {
                vm.answer(default_answer(&spec));
            }
        }
    }
    panic!("no game over within {max_decisions} decisions");
}

/// The neutral default answer now lives with the plan language (§12 rule 5)
/// as the meaning of `Reply::Default`; re-exported while the migration to
/// plans completes.
pub use crate::plan::default_answer;

/// Find a window option whose label contains `needle`.
pub fn option_labeled(options: &[WindowOption], needle: &str) -> Option<WindowOption> {
    options
        .iter()
        .find(|o| match o {
            WindowOption::TriggerInstance { label, .. } | WindowOption::TriggerPaid { label, .. } => {
                label.contains(needle)
            }
            _ => false,
        })
        .cloned()
}

/// Drive until a decision for `side` whose options include a label matching
/// `needle`; answer by taking it. Auto-passes everything else.
/// Returns when the option has been taken.
pub fn take_labeled(vm: &mut Vm, side: Side, needle: &str, budget: usize) {
    for _ in 0..budget {
        let (s, spec) = until_decision(vm);
        let options = match &spec {
            DecisionSpec::PaidWindow { options, .. }
            | DecisionSpec::ReactionWindow { options, .. }
            | DecisionSpec::InterruptWindow { options, .. }
            | DecisionSpec::MidAccessWindow { options } => options.clone(),
            _ => Vec::new(),
        };
        if s == side {
            if let Some(opt) = option_labeled(&options, needle) {
                vm.answer(DecisionAnswer::Take(opt));
                return;
            }
        }
        vm.answer(default_answer(&spec));
    }
    panic!("option labeled {needle:?} never offered to {side:?}");
}
