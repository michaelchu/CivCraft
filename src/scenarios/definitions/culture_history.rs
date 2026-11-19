//! City culture accrual and the permanent national record survive city loss.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::engine_rules::VictoryCondition,
    sim::{HUMAN, Pact, Simulation, UnitKind, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-history",
    3,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.clear_units_and_cities();
    let city = world.founding_city(HUMAN, (10, 24));
    game.process_player_cities(HUMAN);
    let ruleset_history = game.city_history(city) == Some(1)
        && game.city_culture_score(game.city_by_id(city).unwrap()) == 1
        && game.civilization_culture(HUMAN) == 1;
    let mut conquest = Simulation::new(crate::mapgen::generate(game.world_seed));
    let mut world = WorldBuilder::new(&mut conquest);
    world.clear_units_and_cities();
    world.founding_city(HUMAN, (10, 24));
    let target = world.founding_city(1, (20, 24));
    world.city_population(target, 3, None);
    world.city_history(target, 55);
    conquest.sign_pact(HUMAN, 1, Pact::War);
    let attacker = WorldBuilder::new(&mut conquest).unit(HUMAN, UnitKind::Warrior, (20, 24));
    conquest.capture_city_if_needed(attacker, 20, 24);
    let retained = conquest
        .city_by_id(target)
        .is_some_and(|city| city.owner == HUMAN)
        && conquest.city_history(target) == Some(0)
        && conquest.civilization_culture(1) >= 55
        && conquest.civilization_culture(HUMAN) == 0;
    checks.extend([
        (
            "culture_history_uses_ruleset",
            ruleset_history,
            "City processing accrues the ruleset's culture history".into(),
        ),
        (
            "culture_transfer_preserves_old_nation_total",
            retained,
            "Conquest resets the city's local culture and leaves its accumulated points with its original nation".into(),
        ),
        (
            "culture_victory_policy",
            game.victory_enabled(VictoryCondition::Culture),
            "Culture victory is a core campaign condition".into(),
        ),
    ]);
}
