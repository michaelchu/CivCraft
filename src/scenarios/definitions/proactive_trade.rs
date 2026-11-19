//! Cash purchases, policy preferences, export safeguards and trade cadence.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::Tech,
    sim::{Pact, Simulation, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "proactive-trade",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);

/// Two allies with a single technology available for a cash-only exchange.
fn setup(game: &mut Simulation) -> Tech {
    let mut world = WorldBuilder::new(game);
    world.clear_campaign_entities();
    let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
    for (owner, position) in [(1, (10, 24)), (2, (18, 30))] {
        world.founding_city(owner, position);
        world.ai_control(owner, true);
        world.known_technologies(owner, &[Tech::ALPHABET]);
        world.research_project(owner, Tech::POTTERY, 0);
        world.research_goal(owner, Tech::UNSET);
        world.last_science(owner, 20);
    }
    world.add_technology(2, writing);
    world.treasury(2, 0);
    game.make_diplomatic_contact(1, 2);
    game.sign_pact(1, 2, Pact::Alliance);
    WorldBuilder::new(game).diplomatic_state(1, 2, Pact::Alliance, [6000; 2]);
    writing
}

pub(super) fn execute(game: &mut Simulation, checks: &mut Checks) {
    let writing = setup(game);
    WorldBuilder::new(game).civilization_identity(1, "rome");
    let ordinary = game
        .diplomatic_technology_value(1, 2, writing, false)
        .unwrap();
    WorldBuilder::new(game).civilization_identity(1, "korea");
    let scientific = game
        .diplomatic_technology_value(1, 2, writing, false)
        .unwrap();
    WorldBuilder::new(game).research_goal(1, writing);
    let goal = game
        .diplomatic_technology_value(1, 2, writing, false)
        .unwrap();
    let reserve = game.diplomatic_trade_reserve(1);
    WorldBuilder::new(game).treasury(1, reserve);
    let reserve_protected = !game.has_technology_trade(1, 2);
    WorldBuilder::new(game).treasury(1, 5000);
    let mut risk = game.clone();
    WorldBuilder::new(&mut risk).known_technologies(1, &[Tech::ALPHABET, Tech::BRONZE_WORKING]);
    WorldBuilder::new(&mut risk).known_technologies(
        2,
        &[
            Tech::ALPHABET,
            writing,
            Tech::BRONZE_WORKING,
            Tech::IRON_WORKING,
        ],
    );
    WorldBuilder::new(&mut risk).diplomatic_state(1, 2, Pact::Peace, [6000, 0]);
    let security = risk.diplomatic_technology_value(2, 1, Tech::IRON_WORKING, true)
        == Err("military_security");
    game.advance_diplomatic_offers_for(1);
    let purchased =
        game.has_tech(1, writing) && game.players()[1].gold < 5000 && game.players()[2].gold > 0;
    let conserved = game.players()[1].gold + game.players()[2].gold == 5000;
    let after = game.players()[1].gold;
    game.advance_proactive_trades_for(1);
    let no_duplicate = game.players()[1].gold == after;
    checks.extend([
        (
            "allied_cash_purchase",
            purchased,
            "An ally trades despite the old target filter and without a technology to swap".into(),
        ),
        (
            "reserve_protected",
            reserve_protected,
            "Technology purchases preserve the treasury reserve".into(),
        ),
        (
            "trait_and_goal_priorities",
            scientific > ordinary && goal > scientific,
            "Scientific traits and research goals affect valuation".into(),
        ),
        (
            "military_security",
            security,
            "Military unlocks are not exported to a distrusted nonally".into(),
        ),
        (
            "gold_conserved",
            conserved,
            "Technology sales transfer rather than create gold".into(),
        ),
        (
            "trade_cooldown",
            no_duplicate,
            "The pair cannot be charged again by a repeated phase call".into(),
        ),
    ]);
}
