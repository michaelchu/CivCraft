//! City culture accrual and conquest retain the ruleset's history semantics.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::{
    rules::engine_rules::VictoryCondition,
    sim::{HUMAN, Pact, Simulation, UnitKind, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-history",
    1,
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
        && conquest.city_history(target) == Some(55)
        && conquest.civilization_culture(HUMAN) >= 55;
    checks.extend([
        (
            "culture_history_uses_ruleset",
            ruleset_history,
            "City processing accrues the ruleset's culture history".into(),
        ),
        (
            "culture_transfer_retains_history",
            retained,
            "Conquest retains the city's accumulated history and credits its current owner".into(),
        ),
        (
            "culture_victory_policy",
            !game.victory_enabled(VictoryCondition::Culture),
            "The selected C2C3 ruleset leaves culture victory disabled".into(),
        ),
    ]);
}
