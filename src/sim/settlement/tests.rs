//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn equal_yield_future_city_workers_and_reservations_follow_native_city_indices() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.settlement_reservation_order_playtest());
}

#[test]
fn founder_can_choose_useful_work_over_a_positive_city_site_and_keeps_committed_missions() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.founder_work_arbitration_playtest());
}

#[test]
fn founder_city_want_must_be_strictly_greater_than_actual_worker_want() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = Some(Resource::Wheat);
    }
    let id = game.spawn_unit(1, UnitKind::Settler, 12, 24);
    let founder = game.units.last().unwrap().clone();
    let (_, _, want) = game.best_city_site(&founder).unwrap();
    assert!(want > 0.0);
    // The worker evaluator and city result both store adv_want floats.
    assert!(!game.run_ai_founder_with_improvement(&founder, want as f32));
    assert!(!game.ai_settlement_targets.contains_key(&id));
    assert_eq!(
        game.units.last().map(|unit| (unit.x, unit.y)),
        Some((founder.x, founder.y))
    );
    assert!(game.run_ai_founder_with_improvement(&founder, want as f32 - 1.0));
    assert!(game.ai_settlement_targets.contains_key(&id) || !game.cities.is_empty());
}

#[test]
fn settlement_goal_government_desktop_fixture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_settlement_goal_government_playtest());
}

#[test]
fn exhausted_arriving_founder_keeps_its_mission_until_movement_refreshes() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.resource = Some(Resource::Wheat);
        tile.road = false;
    }
    let id = game.spawn_unit(1, UnitKind::Settler, 10, 24);
    let founder = game
        .units
        .iter()
        .find(|unit| unit.id == id)
        .unwrap()
        .clone();
    let destination = offset_native_by_map(10, 24, 1, 0);
    game.reserve_settlement_site(&founder, destination);
    game.run_ai_founder(&founder);
    assert!(game.cities.is_empty());
    let arrived = game.unit_by_id(id).unwrap();
    assert_eq!((arrived.x, arrived.y), destination);
    assert_eq!(arrived.moves_left, 0);
    assert!(game.ai_settlement_targets.contains_key(&id));
    let allowance = game.unit_move_rate(&founder);
    game.units
        .iter_mut()
        .find(|unit| unit.id == id)
        .unwrap()
        .moves_left = allowance;
    let refreshed = game
        .units
        .iter()
        .find(|unit| unit.id == id)
        .unwrap()
        .clone();
    game.run_ai_founder(&refreshed);
    assert_eq!(game.cities.len(), 1);
    assert!(!game.ai_settlement_targets.contains_key(&id));
}

#[test]
fn reservation_lifecycle_keeps_blocked_mission_but_releases_illegal_target() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.resource = Some(Resource::Wheat);
    }
    let id = game.spawn_unit(1, UnitKind::Settler, 2, 8);
    let founder = game
        .units
        .iter()
        .find(|unit| unit.id == id)
        .unwrap()
        .clone();
    game.reserve_settlement_site(&founder, (10, 24));
    assert!(game.ai_settlement_targets[&id].worker.is_some());
    game.prune_settlement_reservations(1);
    assert_eq!(game.ai_settlement_targets[&id].worker, None);
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    game.tile_mut(2, 8).terrain = Terrain::Plains;
    game.tile_mut(10, 24).terrain = Terrain::Plains;
    assert!(game.path_for_unit(&founder, 10, 24).is_empty());
    game.run_ai_founder(&founder);
    assert_eq!(game.ai_settlement_targets[&id].destination, (10, 24));
    game.tile_mut(10, 24).terrain = Terrain::Sea;
    game.run_ai_founder(&founder);
    assert!(!game.ai_settlement_targets.contains_key(&id));
    assert!(game.cities.is_empty());
}

#[test]
fn founders_reserve_destinations_workers_and_growth_then_release_on_capture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_settlement_reservations_playtest());
}

#[test]
fn settlement_uses_goal_government_without_changing_live_government() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.resource = Some(Resource::Wheat);
        tile.irrigation = true;
        tile.road = true;
        tile.river = true;
    }
    game.create_city(1, 5, 8);
    game.cities[0].size = 4;
    game.reconcile_city_workers();
    game.players[1].government = Government::Anarchy;
    game.players[1].known = game.research_path(1, Tech::MONARCHY);
    game.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
    game.players[1].rates = [60, 40, 0];
    let goal = game.ai_goal_government(1);
    assert_ne!(goal, Government::Anarchy);
    assert!(
        goal.requirement()
            .is_some_and(|tech| !game.has_tech(1, tech)),
        "goal {goal:?} should be a future government"
    );
    let point = (15, 40);
    let reservations = game.settlement_citymap(1, None);
    let current = game
        .city_site_result(1, point.0, point.1, Government::Anarchy, &reservations)
        .unwrap()
        .value;
    let projected = game
        .city_site_result(1, point.0, point.1, goal, &reservations)
        .unwrap()
        .value;
    assert_ne!(current, projected);
    assert_eq!(game.city_site_value(1, point.0, point.1), Some(projected));
    assert_eq!(game.players[1].government, Government::Anarchy);
    assert_eq!(game.players[1].rates, [60, 40, 0]);
}

#[test]
fn rough_edges_use_remaining_movement_when_amortizing_a_city_site() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Mountains;
        tile.resource = None;
        tile.road = false;
        tile.railroad = false;
        tile.owner = None;
        tile.base = None;
        tile.hut = false;
        tile.river = false;
    }
    game.players[1].government = Government::Communism;
    let home = (10, 24);
    let target = offset_native_by_map(home.0, home.1, 2, 0);
    game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
    let founder = game.make_unit(0, 1, UnitKind::Settler, home.0, home.1);
    let (site, path, score) = game.best_city_site(&founder).unwrap();
    assert_eq!(site, target);
    assert_eq!(path.len(), 2);
    let expected = game.city_site_value(1, target.0, target.1).unwrap() * (23.0_f64 / 24.0).powi(6)
        - f64::from(founder.kind.stats().cost);
    assert!((score - expected).abs() < 0.0001);
}

#[test]
fn a_virtual_city_does_not_invent_road_trade_on_a_bare_river() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(1, 4, 12);
    let mut city = game.cities[0].clone();
    city.x = 10;
    city.y = 24;
    city.buildings.clear();
    game.players[1].government = Government::Republic;
    let tile = game.tile_mut(city.x, city.y);
    tile.terrain = Terrain::Plains;
    tile.resource = None;
    tile.road = false;
    tile.railroad = false;
    tile.river = true;
    assert_eq!(
        game.city_tile_yields(&city, city.x, city.y, true, None).2,
        2
    );
}

#[test]
fn settlement_values_use_per_tile_rounding_reservations_and_actual_output() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_settlement_value_playtest());
}
