//! Barracks and ordinary unit recovery through the normal recovery rules.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::{Building, Tech},
    sim::{HUMAN, Simulation, UnitKind, fixtures::WorldBuilder},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "city-healing",
    1,
    ScenarioKind::Actions {
        control: Control::AllAi,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(game);
    world.grassland_campaign(true);
    world.known_technologies(HUMAN, &[Tech::BRONZE_WORKING, Tech::WARRIOR_CODE]);
    let city = world.founding_city(HUMAN, (10, 24));
    let mut buildings = game.city_by_id(city).unwrap().buildings.clone();
    buildings.push(Building::BARRACKS);
    let mut world = WorldBuilder::new(game);
    world.city_buildings(city, &buildings);
    let soldier = world.unit(HUMAN, UnitKind::Warrior, (10, 24));
    world.unit_hit_points(soldier, 1);
    game.restore_player_units(HUMAN);
    let barracks_healed = game
        .unit_by_id(soldier)
        .is_some_and(|unit| unit.hp == unit.max_hit_points());
    buildings.retain(|&building| building != Building::BARRACKS);
    let mut world = WorldBuilder::new(game);
    world.city_buildings(city, &buildings);
    world.unit_hit_points(soldier, 1);
    world.unit_moved(soldier, false);
    let unit = game.unit_by_id(soldier).unwrap();
    let expected = unit.hp + game.unit_hitpoint_gain_at(unit, unit.x, unit.y);
    game.restore_player_units(HUMAN);
    let ordinary_healed = game
        .unit_by_id(soldier)
        .is_some_and(|unit| unit.hp == expected);
    checks.push((
        "city_healing",
        barracks_healed && ordinary_healed,
        "Barracks and ordinary city recovery used their expected rules".into(),
    ));
}
