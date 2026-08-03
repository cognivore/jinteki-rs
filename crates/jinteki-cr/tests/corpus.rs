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

/// run-timing-with-ice-and-a-breaker: the Runner breaks the only subroutine
/// with Corroder, passes the ice, and the run reaches the Success Phase and
/// the breach that follows it.
#[test]
fn run_timing_with_ice_and_a_breaker() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::corroder()])
        .credits(Side::Runner, 10)
        .credits(Side::Corp, 10)
        .start(Side::Runner);
    let cor = g.id("Corroder");
    tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Rnd, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(cor)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Rnd)))
            .when(Match::paid().once(), Reply::take("break 1 barrier"))
            .when(Match::sub_targets(), Reply::SubroutineNamed("End the run"))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert_eq!(
        g.vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        0,
        "the only subroutine was broken: {}",
        t.tail(14)
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "6.9.5: the run passed the ice and reached the Success Phase: {}",
        t.tail(14)
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { .. })),
        "the breach of R&D accessed a card: {}",
        t.tail(14)
    );
    assert!(g.vm.current_run.is_none(), "the run is over");
}

/// replace-access-you-may-only: Account Siphon's replacement is a "you may",
/// so step 6.9.5b puts the choice to the Runner — taking it means no breach
/// happens at all, declining it means the ordinary breach does.
#[test]
fn replace_access_you_may_only() {
    // …and choosing the replacement effect.
    let mut g = siphon_game();
    let siphon = g.id("Account Siphon");
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(true))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert!(
        !g.vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "9.9.2: the replacement was applied, so HQ was never breached: {}",
        t.tail(14)
    );

    // …and choosing to access cards.
    let mut g = siphon_game();
    let siphon = g.id("Account Siphon");
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(false))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "6.7.4c: declining the optional replacement leaves the breach: {}",
        t.tail(14)
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { .. })),
        "the ordinary access happened: {}",
        t.tail(14)
    );
}

/// account-siphon-use-ability: the reference's exact numbers — a Corp with 8
/// credits ends on 3, the Runner on 15, with 2 tags.
#[test]
fn account_siphon_use_ability() {
    let mut g = siphon_game();
    let siphon = g.id("Account Siphon");
    assert_eq!(g.vm.st.corp.credits, 8, "the Corp has 8 credits");
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(true))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.runner.tags, 2, "the Runner took 2 tags: {}", t.tail(14));
    assert_eq!(g.vm.st.runner.credits, 15, "the Runner gained 10 credits: {}", t.tail(14));
    assert_eq!(g.vm.st.corp.credits, 3, "the Corp lost 5 credits");
}

/// account-siphon-access: declining the replacement costs the Corp nothing
/// and the Runner gains nothing.
#[test]
fn account_siphon_access() {
    let mut g = siphon_game();
    let siphon = g.id("Account Siphon");
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(siphon))
            .when(Match::optional().once(), Reply::Optional(false))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert_eq!(g.vm.st.runner.tags, 0, "no tags: {}", t.tail(14));
    assert_eq!(g.vm.st.runner.credits, 5, "the Runner gained nothing");
    assert_eq!(g.vm.st.corp.credits, 8, "the Corp lost nothing");
}

/// The `(new-game {:runner {:deck [(qty "Account Siphon" 3)]}})` position the
/// three Siphon tests share: a Corp on 8 credits with cards in HQ, a Runner
/// on 5 holding the real card.
fn siphon_game() -> Game {
    Game::new(11)
        .hand(Side::Runner, vec![cards::account_siphon()])
        .hand(Side::Corp, vec![cards::hedge_fund()])
        .credits(Side::Corp, 8)
        .credits(Side::Runner, 5)
        .start(Side::Runner)
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
// test/clj/game/cards/programs_test.clj — the icebreaker class
// ---------------------------------------------------------------------------

/// corroder: pump once, then break Ice Wall's subroutine — the [interface]
/// gate (9.3.6c) and the barrier restriction (9.5.6c) both hold while it
/// happens.
#[test]
fn corroder() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::corroder()])
        .credits(Side::Runner, 15)
        .credits(Side::Corp, 10)
        .start(Side::Runner);
    let cor = g.id("Corroder");
    let wall = tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(cor)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::paid().once(), Reply::take("+1 strength"))
            .when(Match::paid().once(), Reply::take("break 1 barrier"))
            .when(Match::sub_targets(), Reply::SubroutineNamed("End the run"))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert_eq!(
        t.times_taken("break 1 barrier"),
        1,
        "the [interface] break ability was usable and used: {}",
        t.tail(12)
    );
    assert_eq!(
        g.vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        0,
        "9.8.6/9.8.8: the broken subroutine did not resolve — the run reached HQ"
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "Ice Wall's only subroutine was broken, so the run was not ended: {}",
        t.tail(12)
    );
    let _ = wall;
}

/// mimic: an icebreaker with no pump at all breaks both of Pup's
/// subroutines, so neither net damage happens and the Runner spends 2.
#[test]
fn mimic() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::mimic()])
        .credits(Side::Runner, 5)
        .credits(Side::Corp, 10)
        .start(Side::Runner);
    let mim = g.id("Mimic");
    tk::fill_hand(&mut g.vm, Side::Runner, 4);
    let hand_before = g.vm.st.hand[&Side::Runner].len();
    tk::install_ice(&mut g.vm, cards::pup(), ServerId::Hq, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(mim)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::paid().times(2), Reply::take("break 1 sentry"))
            .when(Match::sub_targets(), Reply::Default)
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert_eq!(t.times_taken("break 1 sentry"), 2, "both subroutines broken: {}", t.tail(16));
    assert_eq!(
        g.vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        0,
        "9.8.6: a broken subroutine does not resolve: {}",
        t.tail(16)
    );
    // 5 - 3 (install) - 1 - 1 (the two breaks).
    assert_eq!(g.vm.st.runner.credits, 0, "the Runner spent 2 credits breaking");
    assert_eq!(
        g.vm.st.hand[&Side::Runner].len(),
        hand_before - 1,
        "no net damage — only Mimic left the grip"
    );
}

/// magnum-opus: the [click] paid ability is an ACTION (1.11.3c/5.2.1), so it
/// is offered in the action window and gains 2.
#[test]
fn magnum_opus() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::magnum_opus()])
        .credits(Side::Runner, 5)
        .start(Side::Runner);
    let mo = g.id("Magnum Opus");
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(mo)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::take("magnum opus"))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.zone_of("Magnum Opus"), Zone::Rig, "installed");
    // 5 - 5 (install) + 2.
    assert_eq!(g.vm.st.runner.credits, 2, "gain 2 credits: {}", t.tail(10));
    assert_eq!(g.vm.memory_limit(), 4, "1.20.2: Magnum Opus does not change the limit");
}

/// rezeki: 1 credit when the Runner's turn begins — the turn AFTER it was
/// installed, since its own turn had already begun.
#[test]
fn rezeki() {
    let mut g = Game::new(11).credits(Side::Runner, 5).start(Side::Corp);
    tk::install_rig(&mut g.vm, cards::rezeki());
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Halt)
            .otherwise_click_credit()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Halt)
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
    );
    script.run(&mut g.vm);
    let before = g.vm.st.runner.credits;
    script.run(&mut g.vm);
    assert_eq!(g.vm.st.runner.credits, before + 1, "Rezeki: gain 1 when your turn begins");
}

/// imp-vs-cards-in-archives: 7.1.5b — "the Runner cannot trash or pay the
/// trash cost of a card in the Corp's discard pile, either with the basic
/// trash ability or with other mid-access abilities". Accessing an agenda in
/// Archives, Imp's "trash the card you are accessing" is not on offer, and
/// stealing is all that is left.
#[test]
fn imp_vs_cards_in_archives() {
    let mut g = Game::new(11).credits(Side::Runner, 10).start(Side::Runner);
    let imp = tk::install_rig(&mut g.vm, cards::imp());
    tk::place_counters(&mut g.vm, imp, CounterKind::Virus, 2);
    // The reference puts the agenda in Archives with `core/move`; a card in
    // the Corp's discard pile is a starting position, not an action.
    let ht = g.vm.new_object(cards::hostile_takeover(), Zone::Discard(Side::Corp));
    g.vm.st.discard.get_mut(&Side::Corp).unwrap().push(ht);
    let mut script = plan::Script::new(
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Archives)))
            .when(Match::mid_access().once(), Reply::Halt)
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    script.run(&mut g.vm);
    let t = script.transcript();
    assert!(
        !t.ever_offered("imp:"),
        "7.1.5b: no mid-access ability may trash a card already in Archives: {}",
        t.tail(12)
    );
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.objects[&ht].zone,
        Zone::ScoreArea(Side::Runner),
        "the agenda in Archives was stolen"
    );
}

/// imp-can-t-be-used-when-empty-5190: with no hosted virus counters Imp's
/// mid-access ability is not an option at all (1.16.1b), and Cache's counters
/// are no help — a cost spends counters hosted on the ability's own source.
#[test]
fn imp_can_t_be_used_when_empty_5190() {
    let mut g = Game::new(11)
        .hand(Side::Corp, vec![cards::hostile_takeover()])
        .credits(Side::Runner, 10)
        .start(Side::Runner);
    let imp = tk::install_rig(&mut g.vm, cards::imp());
    let cache = tk::install_rig(&mut g.vm, cards::cache());
    // The two install conditionals are not replayed for a board seeded
    // directly, so the counters each program carries are setup: Imp empty
    // (the reference zeroes it with `core/update!`), Cache with its 3.
    g.vm.st.objects.get_mut(&cache).unwrap().counters.insert(CounterKind::Virus, 3);
    let mut script = plan::Script::new(
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::mid_access().once(), Reply::Halt)
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
    );
    script.run(&mut g.vm);
    let t = script.transcript();
    assert!(
        !t.ever_offered("imp:"),
        "an unpayable cost is not an option (1.16.1b): {}",
        t.tail(12)
    );
    assert_eq!(
        g.vm.st.objects[&imp].counters.get(&CounterKind::Virus).copied().unwrap_or(0),
        0,
        "Imp really is empty"
    );
}

// ---------------------------------------------------------------------------
// test/clj/game/cards/{operations,agendas,assets,ice,events}_test.clj
// ---------------------------------------------------------------------------

/// hedge-fund: 5 credits in, 9 out.
#[test]
fn hedge_fund() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::hedge_fund()]).start(Side::Corp);
    let card = g.id("Hedge Fund");
    assert_eq!(g.vm.st.corp.credits, 5, "the Corp starts with 5 credits");
    play_it(&mut g, Side::Corp, card);
    assert_eq!(g.vm.st.corp.credits, 9, "Hedge Fund: pay 5, gain 9");
}

/// beanstalk-royalties: gain 3.
#[test]
fn beanstalk_royalties() {
    let mut g =
        Game::new(11).hand(Side::Corp, vec![cards::beanstalk_royalties()]).start(Side::Corp);
    let card = g.id("Beanstalk Royalties");
    let before = g.vm.st.corp.credits;
    play_it(&mut g, Side::Corp, card);
    assert_eq!(g.vm.st.corp.credits, before + 3, "Beanstalk Royalties: gain 3");
}

/// ipo: gain 13, and the terminal clause leaves no clicks.
#[test]
fn ipo() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::ipo()]).credits(Side::Corp, 8).start(Side::Corp);
    let card = g.id("IPO");
    play_it(&mut g, Side::Corp, card);
    assert_eq!(g.vm.st.corp.credits, 13, "IPO: pay 8, gain 13");
    // "Terminal ends turns": the Corp's remaining clicks went, and the action
    // phase ended with the play as its only action. (Asserted on the change
    // log because the driver runs on into the Runner's turn, where the Corp
    // has clicks again.)
    let phase_end = g
        .vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::ActionPhaseEnded { side: Side::Corp }))
        .expect("the Corp's action phase ended");
    assert_eq!(
        g.vm.changes.log[..phase_end]
            .iter()
            .filter(|c| matches!(c, GameChange::ActionTaken { side: Side::Corp, .. }))
            .count(),
        1,
        "the terminal operation was the only action of the phase"
    );
    assert!(
        g.vm.changes.log[..phase_end]
            .iter()
            .any(|c| matches!(c, GameChange::ClicksLost { side: Side::Corp, amount: 2 })),
        "the Corp's remaining 2 clicks went with the action phase"
    );
}

/// sure-gamble: the Runner's 5-for-9.
#[test]
fn sure_gamble() {
    let mut g = Game::new(11).hand(Side::Runner, vec![cards::sure_gamble()]).start(Side::Runner);
    let card = g.id("Sure Gamble");
    let before = g.vm.st.runner.credits;
    play_it(&mut g, Side::Runner, card);
    assert_eq!(g.vm.st.runner.credits, before + 4, "Sure Gamble: pay 5, gain 9");
}

/// hostile-takeover: scoring it gains 7 and takes 1 bad publicity.
#[test]
fn hostile_takeover() {
    let mut g = Game::new(11).credits(Side::Corp, 5).start(Side::Corp);
    let ht = tk::install_root(&mut g.vm, cards::hostile_takeover(), ServerId::Remote(1), false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::Advance(ht)))
            .when(Match::action().once(), Reply::Take(Pick::Advance(ht)))
            .when(Match::paid().once(), Reply::Take(Pick::Score(ht)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(
        g.vm.st.objects[&ht].zone,
        Zone::ScoreArea(Side::Corp),
        "the agenda was scored: {}",
        t.tail(10)
    );
    // 5 - 2 (the two advance actions' credits) + 7.
    assert_eq!(g.vm.st.corp.credits, 10, "gain 7 credits");
    assert_eq!(g.vm.st.corp.bad_publicity, 1, "take 1 bad publicity");
}

/// pad-campaign: 1 credit as the Corp's turn begins.
#[test]
fn pad_campaign() {
    let mut g = Game::new(11).credits(Side::Corp, 5).start(Side::Runner);
    tk::install_root(&mut g.vm, cards::pad_campaign(), ServerId::Remote(1), true);
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Halt)
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
        Plan::runner()
            .otherwise_click_credit()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(g.vm.st.corp.credits, 6, "PAD Campaign: gain 1 credit when your turn begins");
}

/// ice-wall: an advancement counter is +1 strength, and the subroutine ends
/// the run.
#[test]
fn ice_wall() {
    let mut g = Game::new(11).credits(Side::Corp, 10).start(Side::Corp);
    let wall = tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Remote(1), true);
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::Advance(wall)))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Remote(1))))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.effective_strength(wall),
        Some(2),
        "9.12.1b: +1 strength for the hosted advancement counter"
    );
    script.run(&mut g.vm);
    assert!(g.vm.current_run.is_none(), "the subroutine ended the run");
    assert!(
        !g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run never reached the Success Phase"
    );
}

/// easy-mark: gain 3.
#[test]
fn easy_mark() {
    let mut g = Game::new(11).hand(Side::Runner, vec![cards::easy_mark()]).start(Side::Runner);
    let card = g.id("Easy Mark");
    let before = g.vm.st.runner.credits;
    play_it(&mut g, Side::Runner, card);
    assert_eq!(g.vm.st.runner.credits, before + 3, "Easy Mark: gain 3 credits");
}

/// diesel: play it (−1 card) and draw 3.
#[test]
fn diesel() {
    let mut g = Game::new(11).hand(Side::Runner, vec![cards::diesel()]).start(Side::Runner);
    tk::fill_hand(&mut g.vm, Side::Runner, 2);
    let card = g.id("Diesel");
    let before = g.vm.st.hand[&Side::Runner].len();
    play_it(&mut g, Side::Runner, card);
    assert_eq!(
        g.vm.st.hand[&Side::Runner].len(),
        before - 1 + 3,
        "8.4.5c: Diesel leaves the grip and 3 drawn cards arrive"
    );
}

/// enigma: the first subroutine takes a click off the Runner (1.11.3b), the
/// second ends the run.
#[test]
fn enigma() {
    let mut g = Game::new(11).credits(Side::Corp, 10).start(Side::Runner);
    tk::install_ice(&mut g.vm, cards::enigma(), ServerId::Hq, false);
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    script.run(&mut g.vm);
    // 4 allotted − 1 spent on the run action − 1 LOST to the subroutine.
    assert_eq!(
        g.vm.st.runner.clicks,
        2,
        "1.11.3b: the Runner lost 1 click: {}",
        script.transcript().tail(12)
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::ClicksLost { side: Side::Runner, amount: 1 }
        )),
        "the loss is a real ClicksLost event"
    );
    assert!(g.vm.current_run.is_none(), "the second subroutine ended the run");
}

/// tithe: 1 net damage from the first subroutine, 1 credit from the second.
#[test]
fn tithe() {
    let mut g = Game::new(11).credits(Side::Corp, 5).start(Side::Runner);
    tk::fill_hand(&mut g.vm, Side::Runner, 3);
    tk::install_ice(&mut g.vm, cards::tithe(), ServerId::Hq, false);
    let hand_before = g.vm.st.hand[&Side::Runner].len();
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    assert_eq!(
        g.vm.st.hand[&Side::Runner].len(),
        hand_before - 1,
        "1 net damage trashed 1 card from the grip: {}",
        t.tail(14)
    );
    assert_eq!(g.vm.st.discard[&Side::Runner].len(), 1, "the damaged card is in the heap");
    // 5 − 1 (the rez) + 1 (the subroutine).
    assert_eq!(g.vm.st.corp.credits, 5, "rezzed for 1, gained 1: {}", t.tail(14));
}

/// government-takeover: a scored agenda is still active (9.1.8a), so its
/// [click] ability is an action the Corp can take from the score area.
#[test]
fn government_takeover() {
    let mut g = Game::new(11).credits(Side::Corp, 5).start(Side::Corp);
    let gt = tk::install_root(&mut g.vm, cards::government_takeover(), ServerId::Remote(1), false);
    // The reference reaches the 9 advancements with `core/gain :click 8
    // :credit 8` and nine advance actions; nine counters on the installed
    // card is the same starting position, expressed as setup.
    tk::place_counters(&mut g.vm, gt, CounterKind::Advancement, 9);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::Score(gt)))
            .when(Match::action().once(), Reply::take("government takeover"))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(
        g.vm.st.objects[&gt].zone,
        Zone::ScoreArea(Side::Corp),
        "the agenda was scored: {}",
        t.tail(12)
    );
    assert_eq!(g.vm.st.corp.credits, 8, "the scored agenda's ability gained 3: {}", t.tail(12));
}

/// lt-todachine: rezzing a piece of ice gives the Runner a tag. The ice can
/// only be rezzed where the CR lets it be — as the Runner approaches it
/// (9.2.7e) — so the port runs at it.
#[test]
fn lt_todachine() {
    let mut g = Game::new(11).credits(Side::Corp, 10).start(Side::Runner);
    tk::install_root(&mut g.vm, cards::lt_todachine(), ServerId::Remote(1), true);
    tk::install_ice(&mut g.vm, cards::vanilla_ice(), ServerId::Hq, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    assert_eq!(g.vm.st.runner.tags, 1, "the Runner has 1 tag: {}", t.tail(12));
}

/// desperado: +1[mu] and 1 credit for a successful run.
#[test]
fn desperado() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::desperado()])
        .credits(Side::Runner, 5)
        .start(Side::Runner);
    let desp = g.id("Desperado");
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(desp)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Archives)))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert_eq!(g.vm.memory_limit(), 5, "1.20.2: +1[mu]");
    // 5 − 3 (install) + 1 (the successful run).
    assert_eq!(g.vm.st.runner.credits, 3, "1 credit for the successful run: {}", t.tail(12));
}

/// botulus: the hosted virus counter breaks a subroutine on the host ice.
#[test]
fn botulus() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::botulus()])
        .credits(Side::Runner, 15)
        .credits(Side::Corp, 10)
        .start(Side::Runner);
    let bot = g.id("Botulus");
    let wall = tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(bot)))
            .when(Match::destination(), Reply::Destination(InstallDest::HostedOn(wall)))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::paid().once(), Reply::take("botulus: break"))
            .when(Match::sub_targets(), Reply::Default)
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    assert_eq!(
        g.vm.st.objects[&bot].host,
        Some(wall),
        "1.13.6c: installed hosted on the ice: {}",
        t.tail(16)
    );
    assert_eq!(
        g.vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        0,
        "every subroutine on the host ice was broken: {}",
        t.tail(16)
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run was not ended: {}",
        t.tail(16)
    );
}

/// misdirection-basic-behavior: [click][click], X[credit] removes X tags.
///
/// The reference starts the Runner with `:tags 2`; we make the tags the way
/// the game does — two copies of Lt. Todachine, each of which triggers on the
/// same rez of the approached ice (9.6.4b: one instance per occurrence per
/// source). That costs the port one extra [click] for the run, so the click
/// assertion is on the DELTA the ability charges, not the reference's
/// absolute count.
#[test]
fn misdirection_basic_behavior() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::misdirection()])
        .credits(Side::Runner, 5)
        .credits(Side::Corp, 20)
        .start(Side::Runner);
    let mis = g.id("Misdirection");
    tk::install_root(&mut g.vm, cards::lt_todachine(), ServerId::Remote(1), true);
    tk::install_root(&mut g.vm, cards::lt_todachine(), ServerId::Remote(2), true);
    tk::install_ice(&mut g.vm, cards::vanilla_ice(), ServerId::Hq, false);
    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(mis)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::take("misdirection"))
            .when(Match::declare_x(), Reply::DeclareX(2))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(g.vm.st.runner.tags, 2, "two Todachines, one rez, two tags");
    let clicks_spent = |vm: &Vm| {
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::ClickSpent { side: Side::Runner }))
            .count()
    };
    let clicks_before = clicks_spent(&g.vm);
    let credits_before = g.vm.st.runner.credits;
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.runner.tags,
        0,
        "the Runner has lost both tags: {}",
        script.transcript().tail(20)
    );
    // The ability empties the Runner's action phase, so the clicks it charged
    // are counted where they were spent rather than in a later window.
    assert_eq!(clicks_spent(&g.vm), clicks_before + 2, "the ability cost 2 clicks");
    assert_eq!(g.vm.st.runner.credits, credits_before - 2, "1.16.2c: X = 2, so 2 credits");
}

/// dirty-laundry: "Run any server" puts the attacked server to the Runner at
/// step 6.9.1a, and the 5 credits arrive only if the run ENDED successful —
/// a jacked-out run pays nothing.
#[test]
fn dirty_laundry() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::dirty_laundry(), cards::dirty_laundry()])
        .credits(Side::Runner, 5)
        .start(Side::Runner);
    let first = g.named[0].1;
    let second = g.named[1].1;
    let mut script = plan::Script::new(
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(first))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            .when(Match::jack_out().once(), Reply::JackOut(false))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::play_card(second))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            .when(Match::jack_out().once(), Reply::JackOut(true))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass),
    );
    script.run(&mut g.vm);
    // 5 − 2 (the play) + 5.
    assert_eq!(
        g.vm.st.runner.credits,
        8,
        "the successful run paid out: {}",
        script.transcript().tail(12)
    );
    script.run(&mut g.vm);
    // 8 − 2 (the second play), and nothing else: the Runner jacked out.
    assert_eq!(
        g.vm.st.runner.credits,
        6,
        "run unsuccessful; gained no credits: {}",
        script.transcript().tail(14)
    );
}

/// paper-wall: fully breaking it (6.5.7a) trashes it.
#[test]
fn paper_wall() {
    let mut g = Game::new(11)
        .hand(Side::Runner, vec![cards::corroder()])
        .credits(Side::Runner, 10)
        .credits(Side::Corp, 10)
        .start(Side::Runner);
    let cor = g.id("Corroder");
    let wall = tk::install_ice(&mut g.vm, cards::paper_wall(), ServerId::Hq, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(cor)))
            .when(Match::destination(), Reply::Destination(InstallDest::Rig))
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::paid().once(), Reply::take("break 1 barrier"))
            .when(Match::sub_targets(), Reply::Default)
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::mid_access(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    assert_eq!(
        g.vm.st.objects[&wall].zone,
        Zone::Discard(Side::Corp),
        "6.5.7a: Paper Wall was fully broken, so it trashed itself: {}",
        t.tail(14)
    );
}

/// hostile-infrastructure-basic-behavior: 1 net damage per Corp card the
/// Runner trashes, including a trash of Hostile Infrastructure itself.
#[test]
fn hostile_infrastructure_basic_behavior() {
    let mut g = Game::new(11)
        .hand(Side::Corp, vec![cards::hostile_infrastructure()])
        .credits(Side::Runner, 50)
        .start(Side::Runner);
    tk::fill_hand(&mut g.vm, Side::Runner, 5);
    tk::install_root(&mut g.vm, cards::hostile_infrastructure(), ServerId::Remote(1), true);
    let mut script = plan::Script::new(
        Plan::corp().when(Match::paid(), Reply::Pass).when(Match::reaction(), Reply::Default),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .trashes_on_access()
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Remote(1))))
            .trashes_on_access()
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
    );
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.discard[&Side::Runner].len(),
        1,
        "trashing the accessed Corp card did 1 net damage: {}",
        script.transcript().tail(14)
    );
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.discard[&Side::Runner].len(),
        2,
        "…including when the card trashed is Hostile Infrastructure itself: {}",
        script.transcript().tail(14)
    );
}

/// extract-trash-to-gain-9: gain 6, then pay the optional cost — trashing an
/// installed card — for 3 more. Net +6 over the 3-credit play cost.
#[test]
fn extract_trash_to_gain_9() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::extract()]).start(Side::Corp);
    let extract = g.id("Extract");
    let pad = tk::install_root(&mut g.vm, cards::pad_campaign(), ServerId::Remote(1), false);
    let before = g.vm.st.corp.credits;
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(extract))
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .when(Match::targets().once(), Reply::Targets(vec![pad]))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.corp.credits, before + 6, "3 in, 9 out: {}", t.tail(12));
    assert_eq!(
        g.vm.st.objects[&pad].zone,
        Zone::Discard(Side::Corp),
        "the installed card paid the cost"
    );
    assert_eq!(g.zone_of("Extract"), Zone::Discard(Side::Corp), "8.6.7g");
}

/// extract-skip-trash: declining the optional cost (1.16.11a) leaves the card
/// installed and the Corp 3 up.
#[test]
fn extract_skip_trash() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::extract()]).start(Side::Corp);
    let extract = g.id("Extract");
    let pad = tk::install_root(&mut g.vm, cards::pad_campaign(), ServerId::Remote(1), false);
    let before = g.vm.st.corp.credits;
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(extract))
            .when(Match::nested_cost().once(), Reply::PayCost(false))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.corp.credits, before + 3, "3 in, 6 out: {}", t.tail(12));
    assert_eq!(
        g.vm.st.objects[&pad].zone,
        Zone::Root(ServerId::Remote(1)),
        "nothing was trashed"
    );
}

/// extract-nothing-to-trash: with nothing installed the cost is unpayable
/// (1.16.1b), so the Corp is never asked at all.
#[test]
fn extract_nothing_to_trash() {
    let mut g = Game::new(11).hand(Side::Corp, vec![cards::extract()]).start(Side::Corp);
    let extract = g.id("Extract");
    let before = g.vm.st.corp.credits;
    let t = plan::play(
        &mut g.vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(extract))
            .when(Match::nested_cost(), Reply::Forbid)
            .stop_at_action()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default),
        Plan::runner().when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.corp.credits, before + 3, "3 in, 6 out: {}", t.tail(12));
}

/// infiltration-gain-2: the first of the two optioned effects (9.11.4g).
#[test]
fn infiltration_gain_2() {
    let mut g =
        Game::new(11).hand(Side::Runner, vec![cards::infiltration()]).start(Side::Runner);
    let inf = g.id("Infiltration");
    let before = g.vm.st.runner.credits;
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(inf))
            .when(Match::options().once(), Reply::ChooseNamed("gain 2 credits"))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
    );
    assert_eq!(g.vm.st.runner.credits, before + 2, "the Runner gains 2: {}", t.tail(10));
}

/// infiltration-expose: the other option exposes an installed Corp card
/// (10.2.2b — the Runner is shown it, and it stays unrezzed).
#[test]
fn infiltration_expose() {
    let mut g =
        Game::new(11).hand(Side::Runner, vec![cards::infiltration()]).start(Side::Runner);
    let inf = g.id("Infiltration");
    let wall = tk::install_ice(&mut g.vm, cards::ice_wall(), ServerId::Hq, false);
    let t = plan::play(
        &mut g.vm,
        Plan::corp().when(Match::paid(), Reply::Pass),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(inf))
            .when(Match::options().once(), Reply::ChooseNamed("expose 1 card"))
            .when(Match::targets().once(), Reply::Targets(vec![wall]))
            .stop_at_action()
            .when(Match::paid(), Reply::Pass),
    );
    assert!(
        g.vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardExposed { obj } if *obj == wall
        )),
        "Ice Wall protecting HQ was exposed: {}",
        t.tail(10)
    );
    assert!(!g.vm.st.objects[&wall].faceup, "8.1.5: exposing is not rezzing");
}

/// rashida-jaheem-when-there-are-enough-cards-in-r-d: the optional turn-begins
/// ability trashes its own source to gain 3 and draw 3.
#[test]
fn rashida_jaheem_when_there_are_enough_cards_in_r_d() {
    let mut g = Game::new(11).credits(Side::Corp, 5).start(Side::Runner);
    let rj = tk::install_root(&mut g.vm, cards::rashida_jaheem(), ServerId::Remote(1), true);
    let mut script = plan::Script::new(
        Plan::corp()
            // 9.6.9: "you may" — the instance is offered in the turn-begins
            // reaction window and the Corp chooses to trigger it.
            .when(Match::reaction().once(), Reply::take("rashida jaheem"))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
        Plan::runner()
            .otherwise_click_credit()
            .when(Match::paid(), Reply::Pass)
            .when(Match::reaction(), Reply::Default)
            .when(Match::discard(), Reply::Default),
    );
    let credits_before = g.vm.st.corp.credits;
    let hand_before = g.vm.st.hand[&Side::Corp].len();
    script.run(&mut g.vm);
    assert_eq!(
        g.vm.st.corp.credits,
        credits_before + 3,
        "gain 3: {}",
        script.transcript().tail(12)
    );
    // 3 drawn plus the Corp's mandatory draw for the turn (5.6.1).
    assert_eq!(g.vm.st.hand[&Side::Corp].len(), hand_before + 4, "draw 3");
    assert_eq!(
        g.vm.st.objects[&rj].zone,
        Zone::Discard(Side::Corp),
        "1.16.11a: the cost was trashing itself"
    );
}

/// Take the basic play action with this card and let the play resolve.
fn play_it(g: &mut Game, side: Side, card: ObjectId) {
    let acting = Plan::for_side(side)
        .when(Match::action().once(), Reply::play_card(card))
        .stop_at_action()
        .when(Match::paid(), Reply::Pass);
    let idle = Plan::for_side(side.other()).when(Match::paid(), Reply::Pass);
    let (corp, runner) =
        if side == Side::Corp { (acting, idle) } else { (idle, acting) };
    plan::play(&mut g.vm, corp, runner);
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
    "run-timing-with-ice-and-a-breaker",
    "replace-access-you-may-only",
    // test/clj/game/core/rules_test.clj
    "no-scoring-after-terminal",
    "purge-corp",
    // test/clj/game/cards/programs_test.clj
    "corroder",
    "mimic",
    "magnum-opus",
    "rezeki",
    "botulus",
    "imp-vs-cards-in-archives",
    "imp-can-t-be-used-when-empty-5190",
    "misdirection-basic-behavior",
    // test/clj/game/cards/{operations,events,agendas,assets,hardware,ice}_test.clj
    "hedge-fund",
    "beanstalk-royalties",
    "ipo",
    "extract-trash-to-gain-9",
    "extract-skip-trash",
    "extract-nothing-to-trash",
    "sure-gamble",
    "easy-mark",
    "diesel",
    "dirty-laundry",
    "infiltration-gain-2",
    "infiltration-expose",
    "account-siphon-use-ability",
    "account-siphon-access",
    "hostile-takeover",
    "government-takeover",
    "pad-campaign",
    "lt-todachine",
    "rashida-jaheem-when-there-are-enough-cards-in-r-d",
    "desperado",
    "ice-wall",
    "enigma",
    "tithe",
    "paper-wall",
    "hostile-infrastructure-basic-behavior",
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
        PORTED.len() >= 58,
        "DP-7c ported {} of {CORPUS_TOTAL}; the ratchet floor is 58",
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
    assert_eq!(partial, 7, "partial cards (CORPUS.md §5): {partial}");
    let _ = CardType::Agenda;
}
