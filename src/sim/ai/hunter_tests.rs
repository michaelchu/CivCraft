//! Regression tests for ai behavior, including edge cases and interactions with the simulation.

use super::*;

#[test]
fn native_hunter_tactical_fixture_uses_land_pf_order_gameloss_and_base_type_speed() {
    let sim = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        sim.native_hunter_tactics_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn hunter_assigns_same_tile_missiles_only_when_a_real_hunt_begins() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    let (hunter, missile, target) = sim.ai_submarine_playtest_setup();
    sim.cities.clear();
    sim.units
        .iter_mut()
        .find(|u| u.id == missile)
        .unwrap()
        .transport = None;
    sim.record_transport_change(missile, None);
    sim.units
        .iter_mut()
        .find(|u| u.id == missile)
        .unwrap()
        .moves_left = 0;
    let actor = sim.unit_by_id(hunter).unwrap().clone();
    let mut no_target = sim.clone();
    no_target.remove_units_with_cargo(vec![target]);
    assert_eq!(no_target.manage_hunter(&actor, &mut Default::default()), 0);
    assert_eq!(
        no_target
            .units
            .iter()
            .find(|u| u.id == missile)
            .unwrap()
            .transport,
        None
    );
    assert_ne!(sim.manage_hunter(&actor, &mut Default::default()), 0);
    assert_eq!(
        sim.units
            .iter()
            .find(|u| u.id == missile)
            .unwrap()
            .transport,
        Some(hunter)
    );
}

#[test]
fn land_hunter_target_and_interception_history_survive_actual_end_turn() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    sim.cities.clear();
    sim.hunter_targets.clear();
    sim.previous_naval_positions.clear();
    for player in &mut sim.players {
        player.ai = false;
    }
    sim.players[1].ai = true;
    sim.sign_pact(1, HUMAN, Pact::War);
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.hut = false;
        tile.base = None;
    }
    let start = (20, 30);
    let far = offset_native_by_map(start.0, start.1, 10, 0);
    let hunter = sim.spawn_unit(1, UnitKind::Horseman, start.0, start.1);
    let target = sim.spawn_unit(HUMAN, UnitKind::Leader, far.0, far.1);
    sim.hunter_targets.insert(hunter, target);
    assert!(sim.apply(Command::EndTurn));
    assert_eq!(sim.hunter_targets.get(&hunter), Some(&target));
    let actor = sim.unit_by_id(hunter).unwrap();
    assert_ne!(
        (actor.x, actor.y),
        start,
        "actual land hunter phase advances toward its distant victim"
    );
    assert_eq!(
        sim.previous_naval_positions.get(&hunter),
        Some(&(actor.x, actor.y))
    );
    assert_eq!(sim.previous_naval_positions.get(&target), Some(&far));
}

#[test]
fn hunter_stack_uses_native_prepend_order_and_keeps_raw_original_threat() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    let (hunter_id, _, carrier) = sim.ai_submarine_playtest_setup();
    let hunter = sim
        .units
        .iter()
        .find(|u| u.id == hunter_id)
        .unwrap()
        .clone();
    let target = sim.unit_by_id(carrier).unwrap().clone();
    let second = sim.spawn_unit(HUMAN, UnitKind::Carrier, target.x, target.y);
    sim.units.iter_mut().find(|u| u.id == second).unwrap().hp = target.hp;
    assert_eq!(
        sim.hunter_target(&hunter, &Default::default()).map(|v| v.0),
        Some(second)
    );
    // Original is compared without distance discount. The first unit in
    // the same stack therefore cannot displace the previous target.
    sim.hunter_targets.insert(hunter_id, carrier);
    assert_eq!(
        sim.hunter_target(&hunter, &Default::default()).map(|v| v.0),
        Some(carrier)
    );
    sim.sign_pact(1, HUMAN, Pact::Peace);
    assert_eq!(sim.hunter_target(&hunter, &Default::default()), None);
}

#[test]
fn faster_ship_is_huntable_only_on_an_intercept_vector() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (hunter, _, carrier) = game.ai_submarine_playtest_setup();
    let sub = game
        .units
        .iter()
        .find(|unit| unit.id == hunter)
        .unwrap()
        .clone();
    let target = game
        .units
        .iter()
        .find(|unit| unit.id == carrier)
        .unwrap()
        .clone();
    let destroyer = game.spawn_unit(HUMAN, UnitKind::Destroyer, target.x, target.y);
    let faster = game.units.last().unwrap().clone();
    game.previous_naval_positions
        .insert(destroyer, (faster.x, faster.y));
    assert_eq!(game.hunt_value(&sub, &faster), None);
    let mut damaged = faster.clone();
    damaged.hp = 1;
    assert!(
        game.hunt_value(&sub, &damaged).is_none(),
        "source chase eligibility compares type speed, even when damage slows actual movement"
    );
    let farther = offset_native_by_map(sub.x, sub.y, 2, 0);
    game.previous_naval_positions.insert(destroyer, farther);
    assert!(game.hunt_value(&sub, &faster).is_some());
    assert_eq!(
        game.hunter_target(&sub, &[carrier, destroyer].into_iter().collect()),
        None
    );
}

#[test]
fn active_hunter_passes_over_a_cheap_ship_for_a_carrier() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (hunter, _, carrier) = game.ai_submarine_playtest_setup();
    let sub = game
        .units
        .iter()
        .find(|unit| unit.id == hunter)
        .unwrap()
        .clone();
    let (x, y) = offset_native_by_map(sub.x, sub.y, 0, 1);
    game.tile_mut(x, y).terrain = Terrain::Ocean;
    let transport = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
    game.units.last_mut().unwrap().hp = 1;
    assert_eq!(
        game.hunter_target(&sub, &Default::default())
            .map(|target| target.0),
        Some(carrier)
    );
    game.remove_units_with_cargo(vec![carrier]);
    assert!(game.units.iter().any(|unit| unit.id == transport));
    assert_eq!(game.hunter_target(&sub, &Default::default()), None);
}

#[test]
fn normal_targets_handicap_suppresses_hunter_and_ammunition_production() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.normal_hunter_production_playtest(), (true, true, true));
}

#[test]
fn distant_ground_missions_coordinate_air_raids_before_infantry_moves() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (infantry, bomber, city) = game.ai_ground_invasion_playtest_setup();
    assert_eq!(game.ai_ground_targets.get(&infantry), Some(&city));
    let target = game.city_by_id(city).unwrap();
    let unit = game.unit_by_id(infantry).unwrap();
    assert!(game.map_distance(unit.x, unit.y, target.x, target.y) > unit.kind.stats().moves);
    assert!(game.invasion_support(1, target, bomber).1 > 0);
    game.run_ai_player(1);
    assert!(game.unit_by_id(bomber).unwrap().moved);
    assert_eq!(game.ai_ground_targets.get(&infantry), Some(&city));
}

#[test]
fn local_occupation_marks_only_undefended_cities_and_obeys_actual_movement_radius() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (infantry, _, city) = game.ai_ground_invasion_playtest_setup();
    game.ai_ground_targets.clear();
    let target = game.city_by_id(city).unwrap().clone();
    let adjacent = offset_native_by_map(target.x, target.y, -1, 0);
    let unit = game.units.iter_mut().find(|u| u.id == infantry).unwrap();
    (unit.x, unit.y) = adjacent;
    assert_eq!(game.invasion_support(1, &target, 0).1, 0);
    game.units.retain(|u| u.owner != HUMAN);
    assert!(game.invasion_support(1, &target, 0).1 > 0);
    let outside = offset_native_by_map(target.x, target.y, -2, 0);
    let unit = game.units.iter_mut().find(|u| u.id == infantry).unwrap();
    (unit.x, unit.y) = outside;
    assert_eq!(game.invasion_support(1, &target, 0).1, 0);
}

#[test]
fn peace_and_defensive_reservations_cancel_ground_missions() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (infantry, _, city) = game.ai_ground_invasion_playtest_setup();
    game.refresh_ground_invasions(1, &[infantry].into_iter().collect());
    assert!(!game.ai_ground_targets.contains_key(&infantry));
    game.ai_ground_targets.insert(infantry, city);
    game.sign_pact(1, HUMAN, Pact::Peace);
    game.refresh_ground_invasions(1, &Default::default());
    assert!(!game.ai_ground_targets.contains_key(&infantry));
    let target = game.city_by_id(city).unwrap();
    assert_eq!(game.invasion_support(1, target, 0), (0, 0));
}
#[test]
fn defended_overseas_force_keeps_its_invasion_until_capture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let result = game.overseas_assault_playtest(true);
    assert_eq!(
        result,
        (true, true, true),
        "plans{:?} units{:?} cities{:?}",
        game.ai_ground_targets,
        game.units
            .iter()
            .map(|u| (u.id, u.owner, u.kind, u.x, u.y, u.hp, u.transport))
            .collect::<Vec<_>>(),
        game.cities
            .iter()
            .map(|c| (c.id, c.owner, c.x, c.y))
            .collect::<Vec<_>>()
    );
}
#[test]
fn distant_attacks_forecast_buildable_upgraded_defenders_and_training() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_defender_forecast_playtest());
}
#[test]
fn military_pickup_preserves_a_civilian_boss_until_released() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Ocean;
        tile.hut = false;
        tile.resource = None;
    }
    let home = (10, 24);
    let target = offset_native_by_map(home.0, home.1, 9, 0);
    let shore = offset_native_by_map(home.0, home.1, 8, 0);
    for (x, y) in [home, shore, target] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).known_by[1] = true;
    }
    game.create_city(1, home.0, home.1);
    game.create_city(HUMAN, target.0, target.1);
    game.spawn_unit(1, UnitKind::Marines, home.0, home.1);
    let soldier = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    let merchant = game.spawn_unit(1, UnitKind::Caravan, home.0, home.1);
    let sea = offset_native_by_map(home.0, home.1, -1, 0);
    let boat = game.spawn_unit(1, UnitKind::Transport, sea.0, sea.1);
    game.refresh_ai_ferries();
    assert!(game.assign_ai_ferry(boat, merchant));
    assert!(
        game.dispatch_assault_transports(1, |_| false, |_| false)
            .is_empty()
    );
    assert!(
        game.units
            .iter()
            .find(|u| u.id == soldier)
            .unwrap()
            .transport
            .is_none()
    );
    assert_eq!(
        game.ai_ferries.boats[&boat].state,
        ferries::FerryState::Assigned(merchant)
    );
    game.release_ai_ferry(merchant);
    assert!(
        game.dispatch_assault_transports(1, |_| false, |_| false)
            .contains(&boat)
    );
    assert_eq!(
        game.ai_ferries.boats[&boat].state,
        ferries::FerryState::Assigned(soldier)
    );
}

#[test]
fn worker_transport_dispatch_keeps_its_passenger_and_booking_without_ferry_management() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Ocean;
        tile.pollution = false;
        tile.fallout = false;
        tile.hut = false;
    }
    let position = (10, 24);
    let boat = game.spawn_unit(1, UnitKind::Transport, position.0, position.1);
    let passenger = game.spawn_unit(1, UnitKind::Marines, position.0, position.1);
    game.units
        .iter_mut()
        .find(|u| u.id == passenger)
        .unwrap()
        .transport = Some(boat);
    game.refresh_ai_ferries();
    assert!(game.assign_ai_ferry(boat, passenger));
    game.run_ai_player(1);
    let ship = game.unit_by_id(boat).unwrap();
    assert_eq!((ship.x, ship.y), position);
    assert!(
        game.units
            .iter()
            .any(|u| u.id == passenger && u.transport == Some(boat))
    );
    assert_eq!(
        game.ai_ferries.boats[&boat].state,
        ferries::FerryState::Assigned(passenger)
    );
}

#[test]
fn military_delivery_releases_the_persisted_ferry_boss() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.overseas_assault_playtest(false), (true, true, true));
    assert!(!game.ai_ferries.boats.is_empty());
    assert!(
        game.ai_ferries
            .boats
            .values()
            .all(|boat| boat.state == ferries::FerryState::Available)
    );
}
#[test]
fn road_reinforcement_prioritizes_grave_front_and_keeps_a_home_guard() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_road_reinforcement_playtest());
}

#[test]
fn grave_home_danger_keeps_its_strong_defender_even_with_a_threatened_neighbor() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.road = true;
        tile.railroad = false;
        tile.river = false;
    }
    let home = (10, 24);
    let front = offset_native_by_map(home.0, home.1, 3, 0);
    game.create_city(1, home.0, home.1);
    game.create_city(1, front.0, front.1);
    let marine = game.spawn_unit(1, UnitKind::Marines, home.0, home.1);
    game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    game.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
    let near_home = offset_native_by_map(home.0, home.1, -1, 0);
    let near_front = offset_native_by_map(front.0, front.1, 1, 0);
    game.spawn_unit(HUMAN, UnitKind::Cannon, near_home.0, near_home.1);
    game.spawn_unit(HUMAN, UnitKind::Cannon, near_front.0, near_front.1);
    assert!(game.city_grave_danger(&game.cities[0]) > 0);
    assert!(!game.reinforce_ai_cities(1).contains(&marine));
    assert!(
        game.units
            .iter()
            .any(|u| u.id == marine && (u.x, u.y) == home)
    );
}

#[test]
fn amphibious_waypoint_keeps_the_passengers_final_land_goal() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Ocean;
        tile.owner = None;
        tile.hut = false;
        tile.known_by[1] = true;
    }
    let start = (10, 24);
    let waypoint = offset_native_by_map(start.0, start.1, 5, 0);
    let land = offset_native_by_map(waypoint.0, waypoint.1, 1, 0);
    game.tile_mut(land.0, land.1).terrain = Terrain::Grassland;
    let boat = game.spawn_unit(1, UnitKind::Transport, start.0, start.1);
    game.units.last_mut().unwrap().moves_left = MOVE_FRAGMENTS;
    game.ai_ferry_goto_waypoint(boat, waypoint, land);
    let live = game.unit_by_id(boat).unwrap();
    assert_ne!((live.x, live.y), start);
    assert_ne!((live.x, live.y), waypoint);
    assert_eq!(game.unit_goto_tiles.get(&boat), Some(&land));
    // Ordinary ferry pickup fills its parameter with the pickup itself.
    game.ai_goto_unit(boat, start, GotoTask::None);
    assert_eq!(game.unit_goto_tiles.get(&boat), Some(&start));
}

#[test]
fn damaged_military_ferry_waits_in_city_until_recovered() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Ocean;
    }
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(1, 10, 24);
    let boat = game.spawn_unit(1, UnitKind::Transport, 10, 24);
    game.units.last_mut().unwrap().hp = 1;
    assert!(!game.military_ferry_ready(boat));
    let unit = game.units.last_mut().unwrap();
    unit.hp = unit.kind.stats().hp;
    assert!(game.military_ferry_ready(boat));
}
