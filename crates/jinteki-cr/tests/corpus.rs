//! DP-7c: the jinteki-reference corpus, ported.
//!
//! One Rust test per reference `deftest`, **named after it**, so the two
//! suites can be diffed by name (`docs/vm/CORPUS.md` §3.2). The reference
//! drives its VM procedurally through ~25 helpers; we drive ours with plans
//! (ARCHITECTURE §12 rule 5), so each port is a translation of procedure into
//! policy, not a transliteration. `PORTED` at the bottom is the manifest
//! ratchet: every entry must name a `deftest` that exists in the pinned
//! checkout and a test that exists in this file.
//!
//! What a port asserts is the CR-observable OUTCOME the reference test
//! asserts — never the reference's internal state shapes or prompt text
//! (CORPUS.md §6).

use jinteki_cr::cards;
use jinteki_cr::change::GameChange;
use jinteki_cr::instr::InstallDest;
use jinteki_cr::object::{CardType, CounterKind, ObjectId, PrintedCard, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Match, Pick, Plan, Reply};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

// ---------------------------------------------------------------------------
// Setup — the `new-game` helper, as data
// ---------------------------------------------------------------------------

/// The reference's `(new-game {:corp {:hand [...] :deck [...] :credits n}})`,
/// as a builder over the public vocabulary. Cards come from `cards::` (real
/// printed text); filler decks come from `testkit`, because the reference's
/// own filler ("Hedge Fund" ×10 in a deck nobody draws) is scenery.
struct Game {
    vm: Vm,
    named: Vec<(&'static str, ObjectId)>,
}

impl Game {
    fn new(seed: u64) -> Game {
        let mut vm = Vm::empty(seed);
        tk::fill_deck(&mut vm, Side::Corp, 10);
        tk::fill_deck(&mut vm, Side::Runner, 10);
        vm.st.corp.credits = 5;
        vm.st.runner.credits = 5;
        Game { vm, named: Vec::new() }
    }

    /// Put these cards in a player's hand, in order.
    fn hand(mut self, side: Side, cards: Vec<PrintedCard>) -> Game {
        for c in cards {
            let name = c.name;
            let id = self.vm.new_object(c, Zone::Hand(side));
            self.vm.st.hand.get_mut(&side).unwrap().push(id);
            self.named.push((name, id));
        }
        self
    }

    fn credits(mut self, side: Side, n: u32) -> Game {
        self.vm.st.player_mut(side).credits = n;
        self
    }

    /// Start the named player's turn (the reference's `:start-as`).
    fn start(mut self, side: Side) -> Game {
        self.vm.start_turn(side);
        self
    }

    /// The object installed/held under this printed name.
    fn id(&self, name: &str) -> ObjectId {
        self.named
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, id)| *id)
            .unwrap_or_else(|| panic!("no card named {name} in this game"))
    }

    fn zone_of(&self, name: &str) -> Zone {
        self.vm.st.objects[&self.id(name)].zone
    }

    fn counters(&self, name: &str, kind: CounterKind) -> u32 {
        *self.vm.st.objects[&self.id(name)].counters.get(&kind).unwrap_or(&0)
    }
}

/// `(get-content state :remoteN 0)` / `(get-ice state :remoteN 0)`: is the
/// card installed in SOME remote server's root (or protecting one)?
fn in_a_remote_root(z: Zone) -> bool {
    matches!(z, Zone::Root(ServerId::Remote(_)))
}

fn protecting_a_remote(z: Zone) -> bool {
    matches!(z, Zone::Ice(ServerId::Remote(_)))
}

// ---------------------------------------------------------------------------
// test/clj/game/cards/basic_test.clj — the basic actions (5.2.6 / 5.2.7)
// ---------------------------------------------------------------------------

/// corp-basic-actions-gain-1-credit: `(click-credit state :corp)` gains 1.
#[test]
fn corp_basic_actions_gain_1_credit() {
    let mut g = Game::new(11).start(Side::Corp);
    let before = g.vm.st.corp.credits;
    plan::play(
        &mut g.vm,
        Plan::corp().when(Match::action().once(), Reply::credit()).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(g.vm.st.corp.credits, before + 1, "5.2.6b: gain 1 credit");
}

/// corp-basic-actions-draw-card: `(click-draw state :corp)` draws 1.
#[test]
fn corp_basic_actions_draw_card() {
    let mut g = Game::new(11).start(Side::Corp);
    // The Corp's mandatory draw (5.6.1) has already happened by the first
    // action window, so the delta is taken there — the reference's
    // `changed?` around `click-draw`.
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::draw())
            .stop_at_action(),
        Plan::runner(),
    );
    script.run(&mut g.vm);
    let before = g.vm.st.hand[&Side::Corp].len();
    script.run(&mut g.vm);
    assert_eq!(g.vm.st.hand[&Side::Corp].len(), before + 1, "5.2.6c: draw 1 card");
}

/// corp-basic-actions-install-agenda: Project Beale into a new remote.
#[test]
fn corp_basic_actions_install_agenda() {
    let g = install_one(cards::project_beale(), Side::Corp, InstallDest::NewRemoteRoot);
    assert!(
        in_a_remote_root(g.zone_of("Project Beale")),
        "5.2.6d: the agenda is installed in the root of the new remote"
    );
}

/// corp-basic-actions-install-asset: PAD Campaign into a new remote.
#[test]
fn corp_basic_actions_install_asset() {
    let g = install_one(cards::pad_campaign(), Side::Corp, InstallDest::NewRemoteRoot);
    assert!(in_a_remote_root(g.zone_of("PAD Campaign")), "5.2.6d: the asset is installed");
}

/// corp-basic-actions-install-upgrade: Breaker Bay Grid into a new remote.
#[test]
fn corp_basic_actions_install_upgrade() {
    let g = install_one(cards::breaker_bay_grid(), Side::Corp, InstallDest::NewRemoteRoot);
    assert!(in_a_remote_root(g.zone_of("Breaker Bay Grid")), "5.2.6d: the upgrade is installed");
}

/// corp-basic-actions-install-ice: Ice Wall protecting a new remote.
#[test]
fn corp_basic_actions_install_ice() {
    let g = install_one(cards::ice_wall(), Side::Corp, InstallDest::NewRemoteProtecting);
    assert!(protecting_a_remote(g.zone_of("Ice Wall")), "5.2.6d/8.5.2d: the ice protects it");
}

/// corp-basic-actions-play-operation: Hedge Fund nets +4.
#[test]
fn corp_basic_actions_play_operation() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::hedge_fund()]).start(Side::Corp);
    let hedge = g.id("Hedge Fund");
    let before = g.vm.st.corp.credits;
    plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(hedge))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.corp.credits, before + 4, "Hedge Fund: pay 5, gain 9");
    assert_eq!(g.zone_of("Hedge Fund"), Zone::Discard(Side::Corp), "8.6.7g: trashed on resolution");
}

/// corp-basic-actions-advance-installed-ice: Ice Wall protecting HQ takes an
/// advancement counter from the basic advance action — 1.18.3's permission is
/// what makes a non-agenda advanceable, and 9.1.8f keeps it active unrezzed.
#[test]
fn corp_basic_actions_advance_installed_ice() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::ice_wall()]).start(Side::Corp);
    let wall = g.id("Ice Wall");
    plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(wall)))
            .when(Match::destination(), Reply::Destination(InstallDest::Protecting(ServerId::Hq)))
            .when(Match::action().once(), Reply::Take(Pick::Advance(wall)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.zone_of("Ice Wall"), Zone::Ice(ServerId::Hq), "installed protecting HQ");
    assert_eq!(g.counters("Ice Wall", CounterKind::Advancement), 1, "5.2.6f: 1 advancement");
}

/// corp-basic-actions-advance-agenda: an agenda can always be advanced
/// (1.18.3), with no permission on the card.
#[test]
fn corp_basic_actions_advance_agenda() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::project_beale()]).start(Side::Corp);
    let beale = g.id("Project Beale");
    plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(beale)))
            .when(Match::destination(), Reply::Destination(InstallDest::NewRemoteRoot))
            .when(Match::action().once(), Reply::Take(Pick::Advance(beale)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.counters("Project Beale", CounterKind::Advancement), 1, "5.2.6f: 1 advancement");
}

/// corp-basic-actions-trash-resource-if-runner-is-tagged (5.2.6g / 10.5.3).
///
/// The reference reaches into state with `gain-tags`; we make the tag the way
/// the game does — Lt. Todachine's "whenever you rez a piece of ice, give the
/// Runner 1 tag", with the ice rezzed as the Runner approaches it (9.2.7e),
/// which is the only way ice gets rezzed at all (CORPUS.md §2).
#[test]
fn corp_basic_actions_trash_resource_if_runner_is_tagged() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::fan_site()])
        .credits(Side::Corp, 10)
        .start(Side::Runner);
    let fan = g.id("Fan Site");
    tk::install_root(&mut g.vm, cards::lt_todachine(), ServerId::Remote(1), true);
    tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);

    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            .when(Match::targets().once(), Reply::Targets(vec![fan]))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(fan)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .otherwise_click_credit()
            .when(Match::paid(), Reply::Pass)
            .when(Match::discard(), Reply::Default)
            .when(Match::reaction(), Reply::Default),
    );
    assert_eq!(
        g.vm.st.runner.tags, 1,
        "Lt. Todachine gave the Runner a tag when the approached ice was rezzed: {}",
        t.tail(12)
    );
    assert_eq!(
        g.zone_of("Fan Site"),
        Zone::Discard(Side::Runner),
        "5.2.6g: the tagged Runner's resource is trashed: {}",
        t.tail(12)
    );
}

/// corp-basic-actions-purge (5.2.6h / 10.1.2): every virus counter on the
/// board goes back to the bank, Clot's conditional trashes it, and Botulus
/// replaces its counter when the Runner's turn begins.
#[test]
fn corp_basic_actions_purge() {
    let mut g = Game::new(11).credits(Side::Runner, 10).start(Side::Corp);
    let wall = tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);
    let clot = tk::install_rig(&mut g.vm, cards::clot());
    let imp = tk::install_rig(&mut g.vm, cards::imp());
    let botulus = g.vm.new_object(cards::botulus(), Zone::Rig);
    tk::host_on(&mut g.vm, botulus, wall);
    // The two viruses' install conditionals are not replayed for a board
    // seeded directly, so the counters they arrive with are setup — the same
    // starting position the reference test reaches by installing them.
    g.vm.st.objects.get_mut(&imp).unwrap().counters.insert(CounterKind::Virus, 2);
    g.vm.st.objects.get_mut(&botulus).unwrap().counters.insert(CounterKind::Virus, 1);

    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::Purge))
            // The purge spends all three clicks, so the next Corp decision is
            // whatever follows the action phase: halt there and look.
            .when(Match::any().once(), Reply::Halt)
            .when(Match::paid(), Reply::Pass)
            .when(Match::discard(), Reply::Default)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Halt)
            .otherwise_click_credit()
            .when(Match::paid(), Reply::Pass)
            .when(Match::discard(), Reply::Default)
            .when(Match::reaction(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.objects[&clot].zone,
        Zone::Discard(Side::Runner),
        "Clot: trashed when the Corp purges virus counters"
    );
    assert_eq!(
        g.vm.st.objects[&imp].counters.get(&CounterKind::Virus).copied().unwrap_or(0),
        0,
        "10.1.2: every virus counter returns to the bank"
    );
    // The hosted counter came off too. By the time the Corp's action phase
    // has ended the Runner's turn has begun and Botulus has replaced it, so
    // the removal is asserted where the kernel records it.
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CounterRemoved { obj: Some(o), kind: CounterKind::Virus, amount: 1 }
                if *o == botulus
        )),
        "10.1.2: hosted virus counters return to the bank too"
    );
    assert_eq!(
        g.vm.st.objects[&botulus].counters.get(&CounterKind::Virus).copied().unwrap_or(0),
        1,
        "Botulus: place 1 virus counter when your turn begins"
    );
}

/// runner-basic-actions-gain-1-credit.
#[test]
fn runner_basic_actions_gain_1_credit() {
    let mut g = Game::new(11).start(Side::Runner);
    let before = g.vm.st.runner.credits;
    plan::play(
        &mut g.vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::credit()).stop_at_action(),
    );
    assert_eq!(g.vm.st.runner.credits, before + 1, "5.2.7b: gain 1 credit");
}

/// runner-basic-actions-draw-card.
#[test]
fn runner_basic_actions_draw_card() {
    let mut g = Game::new(11).start(Side::Runner);
    let before = g.vm.st.hand[&Side::Runner].len();
    plan::play(
        &mut g.vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::draw()).stop_at_action(),
    );
    assert_eq!(g.vm.st.hand[&Side::Runner].len(), before + 1, "5.2.7c: draw 1 card");
}

/// runner-basic-actions-install-program: Misdirection into the rig.
#[test]
fn runner_basic_actions_install_program() {
    let g = install_one(cards::misdirection(), Side::Runner, InstallDest::Rig);
    assert_eq!(g.zone_of("Misdirection"), Zone::Rig, "5.2.7d/8.5.4: installed in the rig");
}

/// runner-basic-actions-install-resource: Fan Site into the rig.
#[test]
fn runner_basic_actions_install_resource() {
    let g = install_one(cards::fan_site(), Side::Runner, InstallDest::Rig);
    assert_eq!(g.zone_of("Fan Site"), Zone::Rig, "5.2.7d: installed in the rig");
}

/// runner-basic-actions-install-hardware: Bookmark into the rig.
#[test]
fn runner_basic_actions_install_hardware() {
    let g = install_one(cards::bookmark(), Side::Runner, InstallDest::Rig);
    assert_eq!(g.zone_of("Bookmark"), Zone::Rig, "5.2.7d: installed in the rig");
}

/// runner-basic-actions-play-operation: Sure Gamble nets +4. (The reference
/// calls it "play operation"; the Runner's action is 5.2.7e, "play 1 event".)
#[test]
fn runner_basic_actions_play_operation() {
    let mut g = Game::new(11).hand(Side::Runner, vec![cards::sure_gamble()]).start(Side::Runner);
    let gamble = g.id("Sure Gamble");
    let before = g.vm.st.runner.credits;
    plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(gamble))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.runner.credits, before + 4, "Sure Gamble: pay 5, gain 9");
}

/// runner-basic-actions-run-hq: the basic run action initiates a run (5.2.7f).
#[test]
fn runner_basic_actions_run_hq() {
    let mut g = Game::new(11).start(Side::Runner);
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::paid().once(), Reply::Halt),
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunBegan { server } if *server == ServerId::Hq)),
        "5.2.7f: a run on HQ was initiated: {}",
        t.tail(6)
    );
}

/// runner-basic-actions-remove-tag (5.2.7g). The tag comes from Lt. Todachine
/// as the Corp rezzes the approached ice, not from reaching into state.
#[test]
fn runner_basic_actions_remove_tag() {
    let mut g = Game::new(11).credits(Side::Corp, 10).credits(Side::Runner, 5).start(Side::Runner);
    tk::install_root(&mut g.vm, cards::lt_todachine(), ServerId::Remote(1), true);
    tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::Take(Pick::RemoveTag))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(g.vm.st.runner.tags, 1, "the rez of the approached ice tagged the Runner");
    script.run(&mut g.vm);
    assert_eq!(g.vm.st.runner.tags, 0, "5.2.7g: the tag was removed");
}

// ---------------------------------------------------------------------------
// test/clj/game/core/runs_test.clj — run timing (§6)
// ---------------------------------------------------------------------------

/// run-timing-with-no-ice: a run on an empty, unprotected server reaches
/// Success and completes without the Runner deciding anything more.
#[test]
fn run_timing_with_no_ice() {
    let mut g = Game::new(11).start(Side::Runner);
    plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Archives)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert!(g.vm.current_run.is_none(), "6.9.6d: the run is over");
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "6.9.5: with nothing to pass, the run reaches the Success Phase"
    );
}

/// run-timing-with-an-ice: the Corp rezzes the approached ice (9.2.7e), the
/// encounter begins, and Ice Wall's subroutine ends the run — which does NOT
/// leave any "ended" status behind (6.8.2/6.9.6d).
#[test]
fn run_timing_with_an_ice() {
    let mut g = Game::new(11).credits(Side::Corp, 10).start(Side::Runner);
    tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Remote(1), false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Remote(1))))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert!(g.vm.current_run.is_none(), "Ice Wall's subroutine ended the run: {}", t.tail(10));
    assert!(
        !g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run ended before the Success Phase"
    );
}

// ---------------------------------------------------------------------------
// test/clj/game/core/rules_test.clj
// ---------------------------------------------------------------------------

/// no-scoring-after-terminal: IPO ends the Corp's action phase, so the (S)
/// option that WAS on offer while the agenda stood fully advanced is never
/// offered again this turn.
#[test]
fn no_scoring_after_terminal() {
    let mut g =
        Game::new(11).hand(Side::Corp, vec![cards::ipo()]).credits(Side::Corp, 15).start(Side::Corp);
    let ipo = g.id("IPO");
    let ht = tk::install_root(&mut g.vm, cards::hostile_takeover(), ServerId::Remote(1), false);
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::Advance(ht)))
            .when(Match::action().once(), Reply::Take(Pick::Advance(ht)))
            .when(Match::action().once(), Reply::play_card(ipo))
            .when(Match::paid(), Reply::Pass)
            // Whatever the Corp is asked next, stop and look.
            .when(Match::any().once(), Reply::Halt)
            .when(Match::discard(), Reply::Default)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Halt)
            .otherwise_click_credit()
            .when(Match::paid(), Reply::Pass)
            .when(Match::discard(), Reply::Default)
            .when(Match::reaction(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.objects[&ht].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        2,
        "advanced twice with the basic action"
    );
    let offers_score = |e: &plan::Entry| match &e.spec {
        jinteki_cr::decision::DecisionSpec::PaidWindow { options, .. } => options.iter().any(
            |o| matches!(o, jinteki_cr::decision::WindowOption::Score { card } if *card == ht),
        ),
        _ => false,
    };
    let played_ipo = script
        .transcript()
        .entries
        .iter()
        .position(|e| {
            matches!(&e.answer, Some(jinteki_cr::decision::DecisionAnswer::Action(a))
                if matches!(a, jinteki_cr::decision::ActionOption::BasicPlayOperation { card } if *card == ipo))
        })
        .expect("IPO was played");
    assert!(
        script.transcript().entries[..played_ipo].iter().any(offers_score),
        "1.17.3: the fully advanced agenda WAS scorable before the terminal operation: {}",
        script.transcript().tail(12)
    );
    assert!(
        !script.transcript().entries[played_ipo + 1..].iter().any(offers_score),
        "5.6.2b: the terminal operation ended the action phase, so scoring was \
         never offered again this turn: {}",
        script.transcript().tail(12)
    );
    assert_eq!(
        g.vm.st.objects[&ht].zone,
        Zone::Root(ServerId::Remote(1)),
        "Hostile Takeover is still installed, not scored"
    );
}

/// purge-corp: "Purge virus counters" from a card ability reaches virus
/// counters on CORP cards too (10.1.2 names the board, not a side).
#[test]
fn purge_corp() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::cyberdex_trial()]).start(Side::Corp);
    let trial = g.id("Cyberdex Trial");
    let wall = tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);
    // The reference seeds the counters with a debug command; there is no card
    // that puts virus counters on ice, so they are setup here too.
    g.vm.st.objects.get_mut(&wall).unwrap().counters.insert(CounterKind::Virus, 2);
    plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(trial))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(
        g.vm.st.objects[&wall].counters.get(&CounterKind::Virus).copied().unwrap_or(0),
        0,
        "10.1.2: purging removed the counters on the Corp's own card"
    );
}

// ---------------------------------------------------------------------------
// Shared shape: install one card with the basic install action
// ---------------------------------------------------------------------------

/// The `(play-from-hand state side "Card" "server")` shape: take the basic
/// install action with the card and declare `dest` at step 8.5.16b.
fn install_one(card: PrintedCard, side: Side, dest: InstallDest) -> Game {
    let installing_corp = side == Side::Corp;
    let mut g = Game::new(11).hand(side, vec![card]).credits(side, 10).start(side);
    let id = g.named.last().expect("the card was put in hand").1;
    let acting = Plan::for_side(side)
        .when(Match::action().once(), Reply::Take(Pick::InstallCard(id)))
        .when(Match::destination(), Reply::Destination(dest))
        .stop_at_action()
        .when(Match::paid(), Reply::Pass);
    let idle = Plan::for_side(side.other()).when(Match::paid(), Reply::Pass);
    let (corp, runner) = if installing_corp { (acting, idle) } else { (idle, acting) };
    plan::play(&mut g.vm, corp, runner);
    let _ = installing_corp;
    g
}

// ---------------------------------------------------------------------------
// The manifest ratchet
// ---------------------------------------------------------------------------

/// Reference `deftest`s ported here, by their reference names. The test below
/// checks each one against the pinned checkout AND against this file, so a
/// port cannot be silently dropped or misnamed.
const PORTED: &[&str] = &[
    // test/clj/game/cards/basic_test.clj — all 19.
    "corp-basic-actions-gain-1-credit",
    "corp-basic-actions-draw-card",
    "corp-basic-actions-install-agenda",
    "corp-basic-actions-install-asset",
    "corp-basic-actions-install-upgrade",
    "corp-basic-actions-install-ice",
    "corp-basic-actions-play-operation",
    "corp-basic-actions-advance-installed-ice",
    "corp-basic-actions-advance-agenda",
    "corp-basic-actions-trash-resource-if-runner-is-tagged",
    "corp-basic-actions-purge",
    "runner-basic-actions-gain-1-credit",
    "runner-basic-actions-draw-card",
    "runner-basic-actions-install-program",
    "runner-basic-actions-install-resource",
    "runner-basic-actions-install-hardware",
    "runner-basic-actions-play-operation",
    "runner-basic-actions-run-hq",
    "runner-basic-actions-remove-tag",
    // test/clj/game/core/runs_test.clj
    "run-timing-with-no-ice",
    "run-timing-with-an-ice",
    // test/clj/game/core/rules_test.clj
    "no-scoring-after-terminal",
    "purge-corp",
];

/// Every ported name exists as a `#[test] fn` in this file, spelled the
/// reference's way with `-` → `_`. (The reference checkout itself is not read
/// here — it is not part of the build's source tree; `docs/vm/tools/
/// corpus_survey.py` checks the names against it.)
#[test]
fn corpus_manifest_is_honest() {
    let src = include_str!("corpus.rs");
    let mut missing = Vec::new();
    for name in PORTED {
        let rust = format!("fn {}()", name.replace('-', "_"));
        if !src.contains(&rust) {
            missing.push(*name);
        }
    }
    assert!(missing.is_empty(), "ported names with no test: {missing:?}");
    let tests = src.matches("\n#[test]\nfn ").count();
    assert_eq!(
        tests,
        PORTED.len() + 2,
        "every test in this file is either a port in PORTED or one of the two \
         ratchets — a test that is neither means the manifest drifted"
    );
}

/// The DP-7c odometer. Ratchets like `dp7a_complete`: it may only go up.
#[test]
fn dp7c_odometer() {
    const CORPUS_TOTAL: usize = 3717;
    assert!(
        PORTED.len() >= 23,
        "DP-7c ported {} of {CORPUS_TOTAL}; the ratchet floor is 23",
        PORTED.len()
    );
    // Cards carrying an UNIMPLEMENTED clause are the gap list; the count is
    // asserted so CORPUS.md §5 cannot drift silently from the code.
    let card_src = jinteki_cr::EMBEDDED_SOURCES
        .iter()
        .find(|(n, _)| *n == "cards.rs")
        .map(|(_, s)| *s)
        .expect("cards.rs is embedded");
    let partial = card_src.matches("/// UNIMPLEMENTED:").count();
    assert_eq!(partial, 6, "partial cards (CORPUS.md §5): {partial}");
    let _ = CardType::Agenda;
}
