//! Policy preferences across research, diplomacy, war and economic recovery.
use super::{Checks, Scenario};
use crate::{
    rules::{Building, Tech},
    sim::{Pact, Simulation, fixtures::WorldBuilder},
};

pub(super) const SCENARIO: Scenario = Scenario::new("shared-strategy", execute);

/// Shared with focused strategy unit tests; all fixed policy choices live here.
pub(crate) fn setup(game: &mut Simulation) -> (u32, Tech) {
    let mut world = WorldBuilder::new(game);
    world.clear_campaign_entities();
    let mut cities = Vec::new();
    for (owner, position) in [(1, (10, 24)), (2, (18, 30))] {
        cities.push(world.founding_city(owner, position));
        world.ai_control(owner, true);
        world.known_technologies(owner, &[Tech::ALPHABET]);
        world.research_project(owner, Tech::POTTERY, 0);
        world.research_goal(owner, Tech::UNSET);
        world.last_science(owner, 20);
        world.treasury(owner, 5000);
    }
    let city = cities[0];
    game.make_diplomatic_contact(1, 2);
    game.sign_pact(1, 2, Pact::Peace);
    let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
    WorldBuilder::new(game).add_technology(2, writing);
    WorldBuilder::new(game).research_wants(1, &[(Tech::POTTERY, 100.0), (writing, 100.0)]);
    WorldBuilder::new(game).civilization_identity(1, "korea");
    (city, writing)
}

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let (city, writing) = setup(game);
    WorldBuilder::new(game).civilization_identity(1, "rome");
    let ordinary = game.select_ai_research(1).0;
    WorldBuilder::new(game).research_goal(1, Tech::UNSET);
    let ordinary_bid = game.diplomatic_technology_value(1, 2, writing, false);
    WorldBuilder::new(game).civilization_identity(1, "korea");
    let scientific = game.select_ai_research(1).0;
    WorldBuilder::new(game).research_goal(1, Tech::UNSET);
    let scientific_bid = game.diplomatic_technology_value(1, 2, writing, false);
    let peaceful = game.strategic_priorities(1);
    game.sign_pact(1, 2, Pact::War);
    let wartime = game.strategic_priorities(1);
    game.sign_pact(1, 2, Pact::Peace);
    WorldBuilder::new(game).city_buildings(city, &[Building::LIBRARY; 20]);
    WorldBuilder::new(game).treasury(1, 0);
    let crisis = game.strategic_priorities(1);
    let protected = game.diplomatic_trade_reserve(1) >= crisis.gold_reserve
        && crisis.gold_reserve > game.ai_gold_reserve(1);
    let mut recovered = game.clone();
    WorldBuilder::new(&mut recovered).treasury(1, 5000);
    let recovery = recovered.strategic_priorities(1);
    // End the scenario in a normal playable state with valid research.
    WorldBuilder::new(game).city_buildings(city, &[]);
    WorldBuilder::new(game).treasury(1, 5000);
    checks.extend([
        (
            "trait_research_choice",
            ordinary == Tech::POTTERY && scientific == writing,
            "Equal advisor wants resolve differently for Rome and scientific Korea".into(),
        ),
        (
            "consistent_trade_interest",
            matches!((ordinary_bid, scientific_bid), (Ok(a), Ok(b)) if b > a),
            "The science preference also raises willingness to buy Writing".into(),
        ),
        (
            "wartime_priority",
            peaceful.wars == 0 && wartime.wars == 1 && wartime.military > peaceful.military,
            "A contacted living enemy raises military priority".into(),
        ),
        (
            "solvency_reserve",
            crisis.economic_crisis && protected,
            "Projected losses raise the reserve shared with economic diplomacy".into(),
        ),
        (
            "optional_war_recovery",
            !crisis.can_start_optional_war() && recovery.can_start_optional_war(),
            "An insolvent nation postpones discretionary expansion until it can afford it".into(),
        ),
    ]);
}
