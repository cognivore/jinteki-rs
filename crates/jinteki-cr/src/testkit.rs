//! Test support: minimal card definitions and script drivers for the CR
//! example suite and the playable-slice test. The real card layer arrives in
//! a later wave; these builders exist so kernel tests can express the CR's
//! worked examples with faithful minimal cards.

use crate::ability::{
    AbilityDef, AbilityFlag, Condition, Cost, StaticCond, StaticDecl, TimingRestriction,
    TriggerCond,
};
use crate::decision::{ActionOption, DecisionAnswer, DecisionSpec, WindowOption, Yield};
use crate::effects::DamageKind;
use crate::instr::{Instruction, TargetSpec};
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
        vec![Instruction::Damage { kind: DamageKind::Net, amount: 1, responsible: Side::Corp }],
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
        vec![Instruction::GainCredits(Side::Corp, 1)],
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
            Instruction::GainCredits(Side::Runner, 3),
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
            Instruction::Damage { kind: DamageKind::Net, amount: 3, responsible: Side::Corp },
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
        vec![Instruction::Damage { kind: DamageKind::Meat, amount: 1, responsible: Side::Corp }],
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
        vec![Instruction::Damage { kind: DamageKind::Meat, amount: n, responsible: Side::Corp }],
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
            Instruction::Damage { kind: DamageKind::Net, amount: 1, responsible: Side::Corp },
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
        vec![Instruction::Damage { kind: DamageKind::Net, amount: n, responsible: Side::Corp }],
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
        instructions: vec![Instruction::GainCredits(Side::Corp, 1)],
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
        vec![Instruction::GainCredits(Side::Corp, 2)],
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
        vec![Instruction::GainCredits(Side::Runner, 1)],
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
        vec![Instruction::GainCredits(Side::Runner, 1)],
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
        vec![Instruction::GainCreditsPerCounter { kind: CounterKind::Virus, per: 2 }],
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
    c.abilities = vec![AbilityDef::paid(Cost::free(), vec![Instruction::GainCredits(Side::Corp, 1)])
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
        AbilityDef::subroutine(vec![Instruction::GainCredits(Side::Runner, 5)])
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
        vec![Instruction::GainCredits(Side::Runner, 1)],
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
            Instruction::GainCredits(Side::Runner, 2),
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
        instructions: vec![Instruction::GainCredits(Side::Runner, 1)],
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
        instructions: vec![Instruction::GainCredits(Side::Runner, 1)],
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
            amount: 2,
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
            base: 3,
            if_successful: vec![Instruction::Damage {
                kind: DamageKind::Net,
                amount: 1,
                responsible: Side::Corp,
            }],
            if_unsuccessful: vec![],
            determined_min: Some((
                5,
                vec![Instruction::Damage {
                    kind: DamageKind::Net,
                    amount: 1,
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
            on_match: vec![Instruction::GainCredits(Side::Corp, 1)],
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

/// A neutral default answer: pass/decline where legal, first mandatory
/// obligation otherwise.
pub fn default_answer(spec: &DecisionSpec) -> DecisionAnswer {
    match spec {
        DecisionSpec::Mulligan => DecisionAnswer::KeepHand,
        DecisionSpec::TakeAction { options } => DecisionAnswer::Action(
            options.first().cloned().unwrap_or(ActionOption::BasicCredit),
        ),
        DecisionSpec::PaidWindow { .. } => DecisionAnswer::Pass,
        DecisionSpec::ReactionWindow { options, can_pass } => {
            if *can_pass {
                DecisionAnswer::Pass
            } else {
                let mandatory = options
                    .iter()
                    .find(|o| matches!(o, WindowOption::TriggerInstance { mandatory: true, .. }))
                    .or(options.first())
                    .cloned()
                    .expect("mandatory option");
                DecisionAnswer::Take(mandatory)
            }
        }
        DecisionSpec::InterruptWindow { options, can_pass } => {
            if *can_pass {
                DecisionAnswer::Pass
            } else {
                DecisionAnswer::Take(options.first().cloned().unwrap())
            }
        }
        DecisionSpec::MidAccessWindow { .. } => DecisionAnswer::Pass,
        DecisionSpec::ChooseTargets { candidates, count, .. } => {
            DecisionAnswer::Targets(candidates.iter().take(*count as usize).copied().collect())
        }
        DecisionSpec::ChooseOption { .. } => DecisionAnswer::Option(0),
        DecisionSpec::NestedCost { .. } => DecisionAnswer::PayNestedCost(false),
        DecisionSpec::OptionalEffect { .. } => DecisionAnswer::ResolveOptional(false),
        DecisionSpec::ChooseCandidate { candidates } => {
            DecisionAnswer::Candidate(candidates[0])
        }
        DecisionSpec::JackOut => DecisionAnswer::JackOut(false),
        DecisionSpec::DiscardCards { count, hand } => {
            DecisionAnswer::Discard(hand.iter().take(*count as usize).copied().collect())
        }
        DecisionSpec::MinimalSet { .. } => DecisionAnswer::ChooseSet(0),
        DecisionSpec::TraceSpend { .. } => DecisionAnswer::SpendCredits(0),
        DecisionSpec::PsiBid { .. } => DecisionAnswer::Bid(0),
    }
}

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
