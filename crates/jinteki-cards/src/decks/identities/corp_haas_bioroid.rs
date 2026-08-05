//! Corp — Haas-Bioroid identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).

use crate::edsl::*;

/// Haas-Bioroid: Engineering the Future — Identity: Megacorp.
/// "The first time you install a card each turn, gain 1[credit]."
///
/// COMPLETE. 8.5's install with no stipulation about what was installed — the
/// sentence makes none — and 9.6.5c's ordinal about the occurrence, counted
/// from the change log since the turn began. The Corp installs ice, assets,
/// upgrades and agendas, and this condition reaches all four because the
/// sentence names no type.
///
/// It is not 9.3.6g's once-per-turn flag: 9.1.6 says a player *uses* a paid
/// ability, and an entirely mandatory conditional ability is never used, so
/// nothing would ever spend the flag.
pub fn haas_bioroid_engineering_the_future() -> Card {
    card("Haas-Bioroid: Engineering the Future")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Megacorp"])
        .text("The first time you install a card each turn, gain 1[credit].")
        .when_first_each_turn(installs_a_card(Corp), [gain(Corp, 1)])
        .named("the first install of the turn")
        .build()
}

/// Sportsmetal: Go Big or Go Home — Identity: Subsidiary.
/// "Whenever an agenda is scored or stolen, gain 2[credit] or draw 2 cards."
///
/// COMPLETE. One printed sentence with two conditions, so two conditional
/// abilities with the same effect — the shape Leela Patel and Tāo Salonga
/// already take, and for the same reason: 9.6.1 gives an ability ONE primary
/// condition, and an agenda being scored (1.17.3a) and one being stolen
/// (1.17.3b) are different occurrences.
///
/// "Gain 2[credit] **or** draw 2 cards" is 9.11.4g's option choice, so it is
/// one instruction offering two, not two instructions. 9.1.1a puts the choice
/// with the ability's controller — the CORP, whose identity this is — even on
/// the half the Runner's theft meets.
pub fn sportsmetal() -> Card {
    card("Sportsmetal: Go Big or Go Home")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Subsidiary"])
        .text("Whenever an agenda is scored or stolen, gain 2[credit] or draw 2 cards.")
        .when(corp_scores_agenda(), [go_big_or_go_home()])
        .named("an agenda was scored")
        .when(runner_steals_agenda(), [go_big_or_go_home()])
        .named("an agenda was stolen")
        .build()
}

/// Sportsmetal's option choice, written once because the sentence states it
/// once and only its two conditions differ.
fn go_big_or_go_home() -> Instruction {
    choose_one([("gain 2[credit]", vec![gain(Corp, 2)]), ("draw 2 cards", vec![draw(Corp, 2)])])
}

/// Thule Subsea: Safety Below — Identity: Division.
/// "Whenever the Runner steals an agenda, do 1 core damage unless they spend
///  [click] and 2[credit]."
///
/// COMPLETE. "Unless they spend …" is 1.16.9's alternative cost put to the
/// player the sentence names: the Runner may pay to stop the damage, and the
/// payment is ONE cost with two components (1.16.2), so a Runner who cannot
/// pay both pays neither and takes the damage.
///
/// The damage is the Corp's — 10.4.2 makes the player who caused it
/// responsible, which is what decides who wins a flatline — even though the
/// occurrence that meets the condition is the Runner's theft.
pub fn thule_subsea() -> Card {
    card("Thule Subsea: Safety Below")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Whenever the Runner steals an agenda, do 1 core damage unless they spend [click] and 2[credit].")
        .when(
            runner_steals_agenda(),
            [unless_pays(Runner, clicks(1).plus_cost(credits(2)), core_damage(Corp, 1))],
        )
        .named("safety below")
        .build()
}

/// Custom Biotics: Engineered for Success — Identity: Division.
/// "You cannot include Jinteki cards in this deck."
///
/// COMPLETE. A deck-construction restriction (CR 1.4), settled before the
/// game begins and never read again — the same class of sentence as Ampère's
/// singleton rule, and the writing guide's third rule of thumb puts it in the
/// facts or nowhere rather than in an ability.
pub fn custom_biotics() -> Card {
    card("Custom Biotics: Engineered for Success")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("You cannot include Jinteki cards in this deck.")
        .build()
}

/// Cybernetics Division: Humanity Upgraded — Identity: Division.
/// "Each player's maximum hand size is reduced by 1."
///
/// COMPLETE. The same 5.7.3 declaration NBN: The World is Yours* makes, with
/// the other polarity and the other scope — which is exactly why both are
/// content on one declaration rather than two. "Each player's" reaches the
/// Corp who plays it as well as the Runner, so this identity's own discard
/// phase is shortened too.
pub fn cybernetics_division() -> Card {
    card("Cybernetics Division: Humanity Upgraded")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Each player's maximum hand size is reduced by 1.")
        .declares([each_players_max_hand_size_mod(-1)])
        .named("humanity upgraded")
        .build()
}

/// Haas-Bioroid: Precision Design — Identity: Megacorp.
/// "You get +1 maximum hand size.
///  Whenever you score an agenda, you may add 1 card from Archives to HQ."
///
/// COMPLETE. Two printed lines, and they are different kinds of sentence: the
/// first is permanently true and so a static declaration, the second happens
/// and so is a conditional ability.
///
/// "1 card from Archives" names a zone, which is what lifts 1.15.2c's
/// play-area default — and it says nothing about faceup or facedown, so a
/// card the Corp trashed (10.3.1a puts it there facedown) is as valid a
/// candidate as one the Runner did. The printed "you may" is the whole
/// ability, so it is 9.6.9's declinable conditional.
pub fn haas_bioroid_precision_design() -> Card {
    card("Haas-Bioroid: Precision Design")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Megacorp"])
        .text("You get +1 maximum hand size.")
        .text("Whenever you score an agenda, you may add 1 card from Archives to HQ.")
        .declares([max_hand_size_mod(1)])
        .named("precision design")
        .may_when(corp_scores_agenda(), [add_to_hand(choose(1, &[in_archives()]))])
        .named("an agenda was scored")
        .build()
}

/// Poétrï Luxury Brands: All the Rage — Identity: Division.
/// "Whenever you score an agenda, look at the top 3 cards of R&D. You may
///  install 1 non-agenda card from among them.
///  Whenever an agenda is stolen, you may install 1 non-agenda card from HQ."
///
/// COMPLETE. Two printed lines meeting two different conditions (1.17.3a's
/// score and 1.17.3b's steal), so two conditional abilities — and the first
/// line is TWO sentences, so two instructions: 9.11.4e keeps a look separate
/// from what follows it, and 9.11.4b keeps an install its own instruction.
/// The look therefore finishes, a checkpoint occurs, and only then is the
/// install imminent — which is what makes "from among them" mean the cards
/// this ability looked at rather than the top of R&D as it stands now.
///
/// "Non-agenda" is the ordinary description vocabulary, negated. It names no
/// zone, so on the first line "from among them" supplies the zone and on the
/// second HQ does.
pub fn poetri_luxury_brands() -> Card {
    card("Poétrï Luxury Brands: All the Rage")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Whenever you score an agenda, look at the top 3 cards of R&D. You may install 1 non-agenda card from among them.")
        .text("Whenever an agenda is stolen, you may install 1 non-agenda card from HQ.")
        .when(
            corp_scores_agenda(),
            [
                look_at(top_of_rnd(amount(3)), Corp),
                may(install(
                    choose(1, &[looked_at_by_this_ability(), non(of_type(CardType::Agenda))]),
                    InstallDest::DeclaredByInstaller,
                )),
            ],
        )
        .named("an agenda was scored")
        .may_when(
            runner_steals_agenda(),
            [install(
                choose(1, &[in_hand_of(Corp), non(of_type(CardType::Agenda))]),
                InstallDest::DeclaredByInstaller,
            )],
        )
        .named("an agenda was stolen")
        .build()
}

/// Seidr Laboratories: Destiny Defined — Identity: Division.
/// "The first time each turn the Runner loses or spends [click] during a run,
///  you may add 1 card from Archives to the top of R&D."
///
/// COMPLETE. CR 5.2.1 keeps a click SPENT and a click LOST apart — a bioroid
/// subroutine broken by clicking takes them one way, an Eli-class "lose
/// [click]" the other — and this sentence names both, so the pair is content
/// on one condition rather than two abilities that would each spend their own
/// ordinal.
///
/// "During a run" is 6.3.4's game-state test, checked when the condition
/// would be met. "1 card from Archives" names a zone, which lifts 1.15.2c's
/// play-area default, and the card goes to the TOP of R&D rather than into
/// it anywhere — 4.3.2's ordered deck is what makes that a different place.
pub fn seidr_laboratories() -> Card {
    card("Seidr Laboratories: Destiny Defined")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("The first time each turn the Runner loses or spends [click] during a run, you may add 1 card from Archives to the top of R&D.")
        .may_when_first_each_turn(
            spends_or_loses_click_during_run(Runner),
            [add_to_deck(choose(1, &[in_archives()]), true)],
        )
        .named("the first click of the run")
        .build()
}

/// Asa Group: Security Through Vigilance — Identity: Division.
/// "The first time each turn you install a card, you may install 1 non-agenda
///  card from HQ in the root of or protecting the same server."
///
/// COMPLETE. 8.5's install with no stipulation about what was installed, and
/// 9.6.5c's ordinal about that occurrence — the same condition Haas-Bioroid:
/// Engineering the Future reads, since neither sentence says which type.
///
/// "The same server" is 1.15.4's back-reference applied to a place: the
/// server the card the occurrence named is in. "In the root of OR protecting"
/// is 4.6.6b's two halves of that one server, and the Corp still declares
/// which — so the destination fixes the server and leaves the half open,
/// which is exactly what 8.5.16b's declaration is for.
///
/// An install that created no server position to speak of — the first card
/// going into a brand-new remote still names that remote once it exists — is
/// covered by the same reading; an install with no server at all leaves no
/// destination to identify, and 8.5.14 stops there.
pub fn asa_group() -> Card {
    card("Asa Group: Security Through Vigilance")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("The first time each turn you install a card, you may install 1 non-agenda card from HQ in the root of or protecting the same server.")
        .may_when_first_each_turn(
            installs_a_card(Corp),
            [install(
                choose(1, &[in_hand_of(Corp), non(of_type(CardType::Agenda))]),
                InstallDest::DeclaredByInstallerInServerOfTriggeringCard,
            )],
        )
        .named("security through vigilance")
        .build()
}

/// Cerebral Imaging: Infinite Frontiers — Identity: Division.
/// "Your maximum hand size is equal to the number of credits in your credit
///  pool."
///
/// COMPLETE. Permanently true, so a static declaration — but not the one NBN:
/// The World is Yours* and Cybernetics Division make. Those MOVE the value
/// and this one SETS it, and CR 9.12.1a keeps the two apart in so many words:
/// "first applying any effect that sets it to a specific value, then applying
/// each effect that increases the value, and finally applying each effect
/// that lowers the value". Written as a modifier it would be five plus the
/// credits rather than the credits.
///
/// The value is a 9.12.2 quantity, read continuously — the hand size falls as
/// the Corp spends and rises as they gain, which is why nothing is recorded
/// when the declaration begins.
pub fn cerebral_imaging() -> Card {
    card("Cerebral Imaging: Infinite Frontiers")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Your maximum hand size is equal to the number of credits in your credit pool.")
        .declares([max_hand_size_is(credits_in_pool_of(Corp))])
        .named("infinite frontiers")
        .build()
}

/// Haas-Bioroid: Architects of Tomorrow — Identity: Megacorp.
/// "The first time each turn the Runner passes a rezzed piece of bioroid ice,
///  you may rez 1 bioroid card, paying 4[credit] less."
///
/// COMPLETE. 6.9.4a's pass with a whole description of the ice — "a **rezzed**
/// piece of **bioroid** ice" — which is the ordinary filter vocabulary asked
/// of the occurrence rather than of a target, and 9.6.5c's ordinal about that
/// occurrence.
///
/// "Paying 4[credit] less" is 1.16.2a's reduction of the rez cost, floored at
/// zero by the same rule: the payment still happens, so a Corp who cannot
/// afford the remainder still cannot rez.
///
/// The description "1 **bioroid** card" names no zone, so 1.15.2c's play-area
/// default applies — and 8.1.2's "unrezzed" is 1.15.2b's validity for a rez
/// rather than a word the card adds: a card already faceup is not something a
/// rez can be applied to.
pub fn haas_bioroid_architects_of_tomorrow() -> Card {
    card("Haas-Bioroid: Architects of Tomorrow")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Megacorp"])
        .text("The first time each turn the Runner passes a rezzed piece of bioroid ice, you may rez 1 bioroid card, paying 4[credit] less.")
        .may_when_first_each_turn(
            passes_ice_matching(&[rezzed(), with_subtype("Bioroid"), of_type(CardType::Ice)]),
            [rez_paying_less(choose(1, &[with_subtype("Bioroid"), unrezzed()]), 4)],
        )
        .named("architects of tomorrow")
        .build()
}

/// LEO Construction: Labor Solutions — Identity: Division.
/// "Once per turn → Trash 1 rezzed bioroid card in the root of or protecting
///  the attacked server: End the run."
///
/// COMPLETE. Everything before the colon is the cost (1.16.10's trigger cost),
/// and it describes the cards the ordinary way: 4.6.6b puts the root AND the
/// ice protecting it *in* a server, so "in the root of or protecting" is one
/// location word and not two, and 6.1.2 says which server that is.
///
/// The card states no timing restriction and needs none: outside a run there
/// is no attacked server, so the description reaches nothing, the cost cannot
/// be paid, and 9.5.2 never offers the ability. That is the card's own
/// wording doing the work rather than a restriction read into it.
///
/// "Once per turn →" is 9.3.6g's flag, spent by USING the ability — which a
/// paid ability always is (9.1.6).
pub fn leo_construction() -> Card {
    card("LEO Construction: Labor Solutions")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Once per turn → Trash 1 rezzed bioroid card in the root of or protecting the attacked server: End the run.")
        .paid_once_per_turn(
            trash_cards_matching(
                1,
                &[rezzed(), with_subtype("Bioroid"), in_the_attacked_server()],
            ),
            [end_the_run()],
        )
        .named("labor solutions")
        .build()
}

/// The Foundry: Refining the Process — Identity: Division.
/// "The first time you rez a piece of ice each turn, you may search R&D for
///  another copy of that ice, reveal it, and add it to HQ. Shuffle R&D."
///
/// COMPLETE. One printed line, and 9.11.4 splits it three ways: (d) a search
/// is its own instruction, (e) a reveal ends one, and what is left is the
/// move to HQ. The splits are the rule's, not a reading of the "and"s.
///
/// "Another copy of that ice" is 2.1.4's question about the NAME, asked of the
/// card the condition named (1.15.4). "Another" needs no word of its own: the
/// ice that was rezzed is installed, and this search looks in R&D.
///
/// "Shuffle R&D" is 8.7.3 restated on the card — searching a deck shuffles it,
/// whether or not anything was found — so it is part of the search
/// instruction rather than a fourth one.
pub fn the_foundry() -> Card {
    card("The Foundry: Refining the Process")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("The first time you rez a piece of ice each turn, you may search R&D for another copy of that ice, reveal it, and add it to HQ. Shuffle R&D.")
        .may_when_first_each_turn(
            corp_rezzes_a(CardType::Ice),
            [
                search_rnd(&[a_copy_of_the_triggering_card()], 1),
                reveal(found_by_search()),
                add_to_hand(found_by_search()),
            ],
        )
        .named("refining the process")
        .build()
}

/// Thunderbolt Armaments: Peace Through Power — Identity: Division.
/// "Whenever you rez a piece of AP or destroyer ice during a run, that ice
///  gets +1 strength and gains “[subroutine] End the run unless the Runner
///  trashes 1 of their installed cards.” after its other subroutines for the
///  remainder of that run."
///
/// COMPLETE. The condition carries two stipulations of different kinds: what
/// the card IS ("a piece of **AP** or **destroyer** ice" — a printed "or"
/// between subtypes, so the disjunction word) and what the STATE is ("during
/// a run", 9.6.5c).
///
/// One sentence, so one instruction (9.11.3): the strength and the subroutine
/// arrive together, and splitting them would invent a checkpoint and a second
/// interrupt window the card does not have. 9.8.2's ordering is stated
/// outright — "after its other subroutines" — and "for the remainder of that
/// run" is one duration governing both halves.
pub fn thunderbolt_armaments() -> Card {
    card("Thunderbolt Armaments: Peace Through Power")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Whenever you rez a piece of AP or destroyer ice during a run, that ice gets +1 strength and gains “[subroutine] End the run unless the Runner trashes 1 of their installed cards.” after its other subroutines for the remainder of that run.")
        .when(
            corp_rezzes_matching(
                &[of_type(CardType::Ice), with_any_subtype(&["AP", "Destroyer"])],
                &[during_a_run()],
            ),
            [combined([
                modify_strength_of(the_triggering_card(), 1, WantedDuration::ThisRun),
                gains_subroutine(
                    the_triggering_card(),
                    false,
                    WantedDuration::ThisRun,
                    [unless_pays(
                        Runner,
                        trash_cards_matching(1, &[installed_runner_card()]),
                        end_the_run(),
                    )],
                ),
            ])],
        )
        .named("peace through power")
        .build()
}

/// Strategic Innovations: Future Forward — Identity: Division.
/// "Draft format only.
///  If you have more [haas-bioroid] cards rezzed than any other faction, when
///  the Runner's turn ends, shuffle 1 card in Archives into R&D."
///
/// COMPLETE. The format restriction, then one conditional ability whose
/// leading "if" is 9.6.5c's additional requirement inside the trigger
/// condition — asked at 5.6.3d/5.7.2d, the formal end of the Runner's turn,
/// which is when the condition would be met.
///
/// The faction partition is drawn over the REZZED cards, which is the same
/// comparison Boris "Syfr" Kovac makes over the installed ones with 8.1.2's
/// faceup stipulation added; a Corp identity says it that way because 8.1.1
/// makes rezzing the only thing that turns a Corp card faceup in the play
/// area.
///
/// "1 card in Archives" names a zone, so 1.15.2c's play-area restriction
/// lifts for it. The shuffle is what makes the card go to R&D as a card and
/// not to its top: 8.7.3's shuffle is the whole move.
pub fn strategic_innovations() -> Card {
    card("Strategic Innovations: Future Forward")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Draft format only.")
        .text("If you have more [haas-bioroid] cards rezzed than any other faction, when the Runner's turn ends, shuffle 1 card in Archives into R&D.")
        .when(
            turn_ends_if(
                Runner,
                &[more_cards_of_this_faction_than_any_other(
                    "Haas-Bioroid",
                    &[installed_corp_card(), rezzed()],
                )],
            ),
            [shuffle_into_deck(choose(1, &[in_archives()]), Corp)],
        )
        .named("future forward")
        .build()
}

/// Haas-Bioroid: Stronger Together — Identity: Megacorp.
/// "All bioroid ice has +1 strength."
///
/// COMPLETE. A permanent fact, so a declaration: 9.4.1 says a static ability
/// never resolves, and 2.5's strength is read through the 9.12.1a pipeline
/// every time anyone asks, so an ice that stops being described stops being
/// modified without anything happening.
///
/// The sentence describes its cards rather than naming one, which is the
/// whole difference from the Hush-class modification printed on a card about
/// its own host: the description is the shared filter vocabulary, so "all
/// bioroid ice" is the two words the card prints and nothing else. It is not
/// scoped to the INSTALLED ones, because the sentence does not scope it —
/// a bioroid in HQ is bioroid ice, and nothing reads the strength of a card
/// there.
///
/// DEVIATION (deviation 47's class, and the reason this sentence waited):
/// "bioroid" is read as a PRINTED subtype. The declaration is gathered while
/// the characteristics pipeline is being built, and asking that pipeline for
/// an effective subtype from inside its own input would not terminate — so a
/// piece of ice that GAINED bioroid from a third card is not described here,
/// while one that prints it is.
pub fn haas_bioroid_stronger_together() -> Card {
    card("Haas-Bioroid: Stronger Together")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Megacorp"])
        .text("All bioroid ice has +1 strength.")
        .declares([strength_mod_of(&[of_type(CardType::Ice), with_subtype("Bioroid")], 1)])
        .build()
}

/// Chronos Protocol: Haas-Bioroid — Identity: Division.
/// "Whenever the Runner trashes a card for brain damage, they remove all
///  copies of that card from the game (installed, in the heap, stack, grip, or
///  any other location). Then, they shuffle their stack."
///
/// COMPLETE. Two printed sentences, so two instructions of one conditional
/// ability (9.11.3) — "then" says the order they were already in and splits
/// nothing (9.11.4b-g are the splits, and none of them is a "then").
///
/// The condition is 10.4.2b's damage procedure, asked about with the printed
/// sentence's own stipulation: the trash IS the procedure — "for each point of
/// damage suffered, the player responsible for the damage trashes 1
/// randomly-chosen card from the grip" — so "trashes a card for brain damage"
/// and "suffers core damage" name ONE occurrence and not two, and what the
/// sentence adds is that a card was actually trashed. (Nothing on a board can
/// show that half: the only damage that trashes nothing is damage against an
/// empty grip, and 1.7.2b flatlines the Runner for it. The stipulation is
/// carried because the card carries it.) 10.4.2c is what makes "brain damage"
/// core damage; the printed "the Runner trashes" is the older wording of
/// 10.4.2b's responsible player, and since only the Runner's cards are ever
/// trashed for damage the two describe the same thing.
///
/// "That card" is 1.15.4 read of the cards the occurrence named — the trashed
/// ones. 10.4.3 trashes several simultaneously, so an occurrence can name
/// more than one, and the sentence reaches a copy of any of them.
///
/// "(installed, in the heap, stack, grip, or any other location)" is 1.15.2c's
/// other end: without a criterion naming a zone a description means the
/// installed cards, and this parenthesis is the card saying it means every
/// zone. The trashed card is itself among the copies removed — it is in the
/// heap by now, which the parenthesis names first.
pub fn chronos_protocol_haas_bioroid() -> Card {
    card("Chronos Protocol: Haas-Bioroid")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Whenever the Runner trashes a card for brain damage, they remove all copies of that card from the game (installed, in the heap, stack, grip, or any other location). Then, they shuffle their stack.")
        .when(
            trashes_a_card_for_damage(DamageKind::Core),
            [
                performed_by(
                    Runner,
                    remove_from_game(all_matching(&[
                        a_copy_of_the_triggering_card(),
                        in_any_location(),
                    ])),
                ),
                performed_by(Runner, shuffle_deck_of(Runner)),
            ],
        )
        .named("every copy of the card core damage trashed")
        .build()
}

/// Every Haas-Bioroid identity this module carries, in the order the queue
/// reached them.
pub fn identities() -> Vec<Card> {
    vec![
        strategic_innovations(),
        asa_group(),
        cerebral_imaging(),
        chronos_protocol_haas_bioroid(),
        custom_biotics(),
        cybernetics_division(),
        haas_bioroid_architects_of_tomorrow(),
        haas_bioroid_engineering_the_future(),
        haas_bioroid_precision_design(),
        haas_bioroid_stronger_together(),
        leo_construction(),
        poetri_luxury_brands(),
        seidr_laboratories(),
        sportsmetal(),
        the_foundry(),
        thule_subsea(),
        thunderbolt_armaments(),
    ]
}
