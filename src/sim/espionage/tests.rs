//! Regression tests for the behavior of the parent module.

use super::*;
use std::collections::BTreeSet;

#[test]
fn agent_prefers_a_road_to_a_closer_city_across_rough_terrain() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Mountains;
        tile.road = false;
        tile.railroad = false;
        tile.known_by[1] = true;
    }
    let origin = (10, 24);
    let point = |dx, dy| offset_native_by_map(origin.0, origin.1, dx, dy);
    let near = point(0, 3);
    let far = point(5, 0);
    game.create_city(HUMAN, near.0, near.1);
    let near_id = game.cities[0].id;
    game.create_city(HUMAN, far.0, far.1);
    let far_id = game.cities[1].id;
    for step in 0..=4 {
        let (x, y) = point(step, 0);
        game.tile_mut(x, y).road = true;
        // Enemy infrastructure is restricted. This fixture is a neutral
        // road approach to a hostile city, rather than enemy-owned roads.
        game.tile_mut(x, y).owner = None;
    }
    game.spawn_unit(1, UnitKind::Diplomat, origin.0, origin.1);
    let agent = game.units.last().unwrap().clone();
    let route = game
        .plan_city_mission(&agent, 0, false, &BTreeSet::new())
        .unwrap();
    assert_eq!(route.city, far_id);
    assert!(route.path.len() > 2);
    assert!(
        route.cost
            < game
                .route_cost_within(&agent, near.0, near.1, i32::MAX)
                .unwrap()
    );
    game.agent_city_targets.insert(agent.id, near_id);
    assert_eq!(
        game.plan_city_mission(&agent, 0, false, &BTreeSet::new())
            .unwrap()
            .city,
        near_id
    );
}

#[test]
fn offense_precedes_reassignment_but_not_a_sole_home_defender() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.hut = false;
        tile.known_by[1] = true;
    }
    let origin = (10, 24);
    let foreign = offset_native_by_map(origin.0, origin.1, 2, 0);
    let home = offset_native_by_map(origin.0, origin.1, -2, 0);
    game.create_city(HUMAN, foreign.0, foreign.1);
    game.create_city(1, home.0, home.1);
    let danger = offset_native_by_map(home.0, home.1, 0, -1);
    game.spawn_unit(HUMAN, UnitKind::Diplomat, danger.0, danger.1);
    let id = game.spawn_unit(1, UnitKind::Diplomat, origin.0, origin.1);
    game.run_ai_player(1);
    assert!(game.has_embassy(1, HUMAN));
    assert!(!game.units.iter().any(|unit| unit.id == id));

    let guard = game.spawn_unit(1, UnitKind::Diplomat, home.0, home.1);
    let unit = game.units.last().unwrap().clone();
    assert!(game.holds_counterintelligence(&unit));
    game.run_ai_player(1);
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == guard && (unit.x, unit.y) == home)
    );
}

#[test]
fn targeted_missions_use_the_chosen_building_and_apply_the_harder_roll() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let spy = game.spy_playtest_setup();
    let foreign = game.cities[0].id;
    game.cities[0]
        .buildings
        .extend([Building::GRANARY, Building::TEMPLE]);
    game.cities[0].shields = 9;
    assert!(!game.apply(Command::TargetedSabotage(
        spy,
        foreign,
        SabotageTarget::Building(Building::PALACE)
    )));
    game.set_random_seed(14); // Native rolls 18 and 14 pass targeted mission and capital.
    assert!(game.apply(Command::TargetedSabotage(
        spy,
        foreign,
        SabotageTarget::Building(Building::TEMPLE)
    )));
    assert!(!game.cities[0].has_building(Building::TEMPLE));
    assert!(game.cities[0].has_building(Building::GRANARY));
    assert_eq!(game.cities[0].shields, 9);

    let (x, y) = offset_native_by_map(game.cities[0].x, game.cities[0].y, 1, 0);
    let second = game.spawn_unit(HUMAN, UnitKind::Spy, x, y);
    game.set_random_seed(2); // Native 51 passes an 80% random mission, but fails targeted sabotage.
    assert!(game.apply(Command::TargetedSabotage(
        second,
        foreign,
        SabotageTarget::Production
    )));
    assert_eq!(game.cities[0].shields, 9);
    assert!(game.units.iter().all(|unit| unit.id != second));
}

#[test]
fn espionage_replaces_diplomat_production_and_spy_can_escape_after_repeat_theft() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let spy = game.spy_playtest_setup();
    let foreign = game.cities[0].id;
    let home = game.cities[1].clone();
    game.set_random_seed(2); // Native rolls 51, 60, 40 pass repeat theft and escape.
    assert!(game.can_produce(&home, Production::Unit(UnitKind::Spy)));
    assert!(!game.can_produce(&home, Production::Unit(UnitKind::Diplomat)));
    assert!(game.apply(Command::StealTechnology(spy, foreign)));
    assert!(game.has_tech(HUMAN, Tech::POTTERY));
    assert_eq!(game.cities[0].technology_thefts, 2);
    let escaped = game.unit_by_id(spy).unwrap();
    assert_eq!(
        (escaped.x, escaped.y, escaped.moves_left),
        (home.x, home.y, 0)
    );
}

#[test]
fn sabotage_can_destroy_a_building_or_clear_production_without_recovery() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let diplomat = game.sabotage_playtest_setup();
    let id = game.cities[0].id;
    let actor = game
        .units
        .iter()
        .find(|unit| unit.id == diplomat)
        .unwrap()
        .clone();
    game.cities[0].buildings.push(Building::GRANARY);
    game.set_random_seed(8); // Native 34, 37, 44 pass mission, select building, defeat capital.
    assert!(game.apply(Command::SabotageCity(diplomat, id)));
    assert!(game.cities[0].has_building(Building::PALACE));
    assert!(!game.cities[0].has_building(Building::GRANARY));
    assert_eq!(game.cities[0].shields, 9);

    let next = game.spawn_unit(HUMAN, UnitKind::Diplomat, actor.x, actor.y);
    game.set_random_seed(1);
    assert!(game.apply(Command::SabotageCity(next, id)));
    assert_eq!(game.cities[0].shields, 0);
    assert_eq!(game.cities[0].protected_shields, 0);
    assert!(game.cities[0].production_before_switch.is_none());
}

#[test]
fn successful_theft_grants_one_technology_and_the_city_catches_repeat_diplomats() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let diplomat = game.technology_theft_playtest_setup();
    let city = game.cities[0].id;
    let unit = game
        .units
        .iter()
        .find(|unit| unit.id == diplomat)
        .unwrap()
        .clone();
    assert!(game.apply(Command::StealTechnology(diplomat, city)));
    assert!(game.has_tech(HUMAN, Tech::POTTERY));
    assert!(game.has_tech(1, Tech::POTTERY));
    assert_eq!(game.cities[0].technology_thefts, 1);

    game.players[1].known.push(Tech::BRONZE_WORKING);
    let second = game.spawn_unit(HUMAN, UnitKind::Diplomat, unit.x, unit.y);
    assert!(game.apply(Command::StealTechnology(second, city)));
    assert!(!game.has_tech(HUMAN, Tech::BRONZE_WORKING));
    assert!(game.units.iter().all(|unit| unit.id != second));
}

#[test]
fn a_defending_diplomat_intercepts_the_mission() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let diplomat = game.technology_theft_playtest_setup();
    let (id, x, y) = (game.cities[0].id, game.cities[0].x, game.cities[0].y);
    let guard = game.spawn_unit(1, UnitKind::Diplomat, x, y);
    game.set_random_seed(2); // Native 51 defeats the attacker's capital-adjusted 25% chance.
    assert!(game.apply(Command::StealTechnology(diplomat, id)));
    assert!(!game.has_tech(HUMAN, Tech::POTTERY));
    assert_eq!(game.cities[0].technology_thefts, 0);
    assert!(game.units.iter().any(|unit| unit.id == guard));
    assert!(game.units.iter().all(|unit| unit.id != diplomat));
}
