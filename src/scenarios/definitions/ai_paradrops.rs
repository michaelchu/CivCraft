//! AI defense, defended-landing refusal and capture with airborne units.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::Tech,
    sim::{
        HUMAN, Pact, Simulation, UnitKind,
        fixtures::{WorldBuilder, capture_known_tiles, give_unit_full_movement},
    },
    world::{Terrain, topology::offset_native_by_map},
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "ai-paradrops",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);
fn execute(game: &mut Simulation, checks: &mut Checks) {
    WorldBuilder::new(game).clear_units();
    game.sign_pact(1, HUMAN, Pact::War);
    let home = (10, 24);
    let front = offset_native_by_map(home.0, home.1, 6, 0);
    let enemy = offset_native_by_map(home.0, home.1, -6, 0);
    let threat = offset_native_by_map(front.0, front.1, 1, 0);
    WorldBuilder::new(game).add_technology(1, Tech::COMBINED_ARMS);
    for (x, y) in [home, front, enemy, threat] {
        let mut world = WorldBuilder::new(game);
        world.terrain((x, y), Terrain::Grassland);
        world.remove_hut((x, y));
        world.mark_known(1, (x, y));
    }
    WorldBuilder::new(game).founding_city(1, home);
    WorldBuilder::new(game).founding_city(1, front);
    WorldBuilder::new(game).founding_city(HUMAN, enemy);
    let enemy_city = game.cities().last().unwrap().id;
    WorldBuilder::new(game).city_population(enemy_city, 2, None);
    // Paradrop defense reacts to attack power, which Cannons lack.
    let horseman = WorldBuilder::new(game).unit(HUMAN, UnitKind::Horseman, threat);
    give_unit_full_movement(game, horseman, UnitKind::Horseman);
    let guard = WorldBuilder::new(game).unit(1, UnitKind::Warrior, home);
    let para = WorldBuilder::new(game).unit(1, UnitKind::Paratrooper, home);
    give_unit_full_movement(game, para, UnitKind::Paratrooper);
    WorldBuilder::new(game).refresh_visibility();
    capture_known_tiles(game, 1);
    game.run_ai_player(1);
    // Freeciv first lands in the threatened empty city, then ramps again
    // after the drop; the paratrooper may leave the city to kill the
    // adjacent attacker before the AI pass ends.
    let defended = game
        .units()
        .iter()
        .any(|unit| unit.id == para && unit.paradropped && (unit.x, unit.y) != home)
        && game.units().iter().all(|unit| unit.id != horseman)
        && game
            .units()
            .iter()
            .any(|unit| unit.id == guard && (unit.x, unit.y) == home)
        && game
            .cities()
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == front)
        && game
            .cities()
            .iter()
            .any(|city| city.owner == HUMAN && (city.x, city.y) == enemy);
    let defender = WorldBuilder::new(game).unit(HUMAN, UnitKind::Warrior, enemy);
    let raider = WorldBuilder::new(game).unit(1, UnitKind::Paratrooper, home);
    give_unit_full_movement(game, raider, UnitKind::Paratrooper);
    WorldBuilder::new(game).refresh_visibility();
    capture_known_tiles(game, 1);
    let raider_unit = game.units().last().unwrap().clone();
    game.run_ai_paratrooper(&raider_unit);
    let refused = game
        .cities()
        .iter()
        .any(|city| city.owner == HUMAN && (city.x, city.y) == enemy);
    WorldBuilder::new(game).remove_fixture_units(&[defender, raider], Some(UnitKind::Horseman));
    let raider = WorldBuilder::new(game).unit(1, UnitKind::Paratrooper, home);
    give_unit_full_movement(game, raider, UnitKind::Paratrooper);
    WorldBuilder::new(game).refresh_visibility();
    capture_known_tiles(game, 1);
    let raider_unit = game.unit_by_id(raider).unwrap().clone();
    let captured = game.run_ai_paratrooper(&raider_unit)
        && game
            .units()
            .iter()
            .any(|unit| unit.id == raider && (unit.x, unit.y) == enemy)
        && game
            .cities()
            .iter()
            .any(|city| city.owner == 1 && (city.x, city.y) == enemy);
    checks.push(("ai_paradrops", defended && refused && captured,
        format!("AI reinforced an empty threatened city before attacking: {defended}. It refused the defended landing: {refused}. It captured the enemy city once the defender left: {captured}.")));
}
