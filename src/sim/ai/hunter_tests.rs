//! Regression tests for ai behavior, including edge cases and interactions with the simulation.

use super::*;

#[test]
fn native_hunter_roles_and_combat_values_match_imported_conquests_rules() {
    use crate::sim::unit_catalog::UnitAiStrategy;

    assert!(UnitKind::CruiseMissile.has_ai_strategy(UnitAiStrategy::CruiseMissile));
    assert!(UnitKind::ICBM.has_ai_strategy(UnitAiStrategy::Icbm));
    assert!(!UnitKind::Horseman.has_ai_strategy(UnitAiStrategy::CruiseMissile));
    assert!(!UnitKind::Horseman.has_ai_strategy(UnitAiStrategy::Icbm));
    let cruise = UnitKind::CruiseMissile.stats();
    let icbm = UnitKind::ICBM.stats();
    assert_eq!((cruise.attack, cruise.defense, cruise.moves), (0, 0, 1));
    assert_eq!((icbm.attack, icbm.defense, icbm.moves), (0, 0, 1));
    assert!(UnitKind::ALL_NATIVE.iter().all(|kind| {
        let rule = kind.conquests_rule();
        kind.stats().cost == rule.cost
            && kind.stats().attack == rule.attack
            && kind.stats().defense == rule.defense
    }));
}

#[test]
fn hunter_assigns_same_tile_missiles_only_when_a_real_hunt_begins() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    let (hunter, missile, target) = sim.ai_submarine_playtest_setup();
    {
        let hunter_unit = sim.units.iter_mut().find(|u| u.id == hunter).unwrap();
        hunter_unit.kind = UnitKind::NuclearSubmarine;
        hunter_unit.hp = hunter_unit.max_hit_points();
    }
    let hunter_clone = sim.unit_by_id(hunter).unwrap().clone();
    let hunter_rate = sim.unit_move_rate(&hunter_clone);
    sim.units
        .iter_mut()
        .find(|u| u.id == hunter)
        .unwrap()
        .moves_left = hunter_rate;
    let hunter_position = (
        sim.unit_by_id(hunter).unwrap().x,
        sim.unit_by_id(hunter).unwrap().y,
    );
    let intercept = offset_native_by_map(hunter_position.0, hunter_position.1, 2, 0);
    sim.previous_naval_positions.insert(target, intercept);
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
fn ordinary_horseman_does_not_enter_source_missile_hunter_phase() {
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
    let far = offset_native_by_map(start.0, start.1, 5, 0);
    // run_ai_player only dispatches missile hunters with the imported
    // CruiseMissile or Icbm AI strategy. A Horseman has neither strategy.
    let hunter = sim.spawn_unit(1, UnitKind::Horseman, start.0, start.1);
    let target = sim.spawn_unit(HUMAN, UnitKind::Cavalry, far.0, far.1);
    sim.previous_naval_positions
        .insert(target, offset_native_by_map(start.0, start.1, 10, 0));
    assert!(sim.apply(Command::EndTurn));
    assert!(!sim.hunter_targets.contains_key(&hunter));
    // The corrected begin phase may assign a normal AI route or move the
    // Horseman. This fixture only asserts it did not enter missile-hunter
    // management, whose target state is `hunter_targets`.
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
    let intercept = offset_native_by_map(hunter.x, hunter.y, 2, 0);
    sim.previous_naval_positions.insert(carrier, intercept);
    let second = sim.spawn_unit(HUMAN, UnitKind::Carrier, target.x, target.y);
    sim.units.iter_mut().find(|u| u.id == second).unwrap().hp = target.hp;
    sim.previous_naval_positions.insert(second, intercept);
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
fn imported_naval_speeds_and_sea_legality_are_used_without_asserting_ai_ranking() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let destroyer = UnitKind::Destroyer.stats();
    let nuclear_sub = UnitKind::NuclearSubmarine.stats();
    assert_eq!(
        (destroyer.attack, destroyer.defense, destroyer.moves),
        (12, 8, 8)
    );
    assert_eq!(
        (nuclear_sub.attack, nuclear_sub.defense, nuclear_sub.moves),
        (8, 4, 5)
    );
    assert!(destroyer.moves > nuclear_sub.moves);

    let start = (10, 24);
    let next = offset_native_by_map(start.0, start.1, 1, 0);
    game.tile_mut(start.0, start.1).terrain = Terrain::Sea;
    game.tile_mut(next.0, next.1).terrain = Terrain::Sea;
    let id = game.spawn_unit(HUMAN, UnitKind::Destroyer, start.0, start.1);
    let ship = game.unit_by_id(id).unwrap().clone();
    assert!(game.movement_cost_for(&ship, next.0, next.1).is_some());
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
    game.tile_mut(x, y).terrain = Terrain::Sea;
    let transport = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
    game.units.last_mut().unwrap().hp = 1;
    game.previous_naval_positions
        .insert(carrier, offset_native_by_map(sub.x, sub.y, 2, 0));
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
fn distant_ground_missions_do_not_assume_a_one_way_air_sortie_is_recoverable() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (infantry, fighter, city) = game.ai_ground_invasion_playtest_setup();
    assert_eq!(game.ai_ground_targets.get(&infantry), Some(&city));
    let target = game.city_by_id(city).unwrap();
    let unit = game.unit_by_id(infantry).unwrap();
    assert!(
        game.map_wrap
            .civ3_distance(unit.x, unit.y, target.x, target.y)
            > unit.kind.stats().moves
    );
    assert!(game.invasion_support(1, target, fighter).1 > 0);
    let plane = game.unit_by_id(fighter).unwrap();
    let home = game
        .cities
        .iter()
        .find(|base| base.owner == plane.owner && (base.x, base.y) == (plane.x, plane.y))
        .unwrap();
    let outbound = game
        .map_wrap
        .civ3_distance(home.x, home.y, target.x, target.y);
    let return_leg = game
        .map_wrap
        .civ3_distance(target.x, target.y, home.x, home.y);
    assert_eq!(plane.kind.operational_range(), 6);
    assert_eq!(outbound, 6);
    assert!(outbound + return_leg > i32::from(plane.kind.fuel_capacity()));
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
fn defender_buildability_uses_imported_technology_and_connected_resources() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.resource = None;
        tile.road = false;
    }
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].known = vec![Tech::GUNPOWDER];
    let saltpeter = offset_native_by_map(10, 24, 1, 0);
    {
        let tile = game.tile_mut(saltpeter.0, saltpeter.1);
        tile.terrain = Terrain::Hills;
        tile.owner = Some(HUMAN);
        tile.resource = Some(Resource::Saltpeter);
        tile.road = true;
    }
    assert!(game.unit_buildable(&game.cities[0], UnitKind::Musketman));
    game.tile_mut(saltpeter.0, saltpeter.1).road = false;
    assert!(!game.unit_buildable(&game.cities[0], UnitKind::Musketman));
    game.tile_mut(saltpeter.0, saltpeter.1).road = true;
    game.players[HUMAN].known.clear();
    assert!(!game.unit_buildable(&game.cities[0], UnitKind::Musketman));
    let musket = UnitKind::Musketman.stats();
    assert_eq!(
        (musket.cost, musket.attack, musket.defense, musket.moves),
        (60, 2, 4, 1)
    );
}
#[test]
fn military_pickup_preserves_a_civilian_boss_until_released() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
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
    game.spawn_unit(1, UnitKind::Marine, home.0, home.1);
    let soldier = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    let reserved_passenger = game.spawn_unit(1, UnitKind::Settler, home.0, home.1);
    let sea = offset_native_by_map(home.0, home.1, -1, 0);
    let boat = game.spawn_unit(1, UnitKind::Transport, sea.0, sea.1);
    game.refresh_ai_ferries();
    assert!(game.assign_ai_ferry(boat, reserved_passenger));
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
        ferries::FerryState::Assigned(reserved_passenger)
    );
    game.release_ai_ferry(reserved_passenger);
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
        tile.terrain = Terrain::Sea;
        tile.pollution = false;
        tile.fallout = false;
        tile.hut = false;
    }
    let position = (10, 24);
    let boat = game.spawn_unit(1, UnitKind::Transport, position.0, position.1);
    let passenger = game.spawn_unit(1, UnitKind::Marine, position.0, position.1);
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
fn roads_reduce_legal_reinforcement_move_cost_and_grave_danger_is_observable() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.owner = None;
    }
    let home = (10, 24);
    let front = offset_native_by_map(home.0, home.1, 3, 0);
    game.create_city(1, home.0, home.1);
    let actor_id = game.spawn_unit(1, UnitKind::Marine, home.0, home.1);
    let enemy = offset_native_by_map(home.0, home.1, 4, 0);
    game.spawn_unit(HUMAN, UnitKind::Cavalry, enemy.0, enemy.1);
    let actor = game.unit_by_id(actor_id).unwrap().clone();
    let (x, y) = offset_native_by_map(home.0, home.1, 1, 0);
    let unroaded = game.movement_cost_for(&actor, x, y).unwrap();
    for step in 0..=3 {
        let pos = offset_native_by_map(home.0, home.1, step, 0);
        game.tile_mut(pos.0, pos.1).road = true;
    }
    let roaded = game.movement_cost_for(&actor, x, y).unwrap();
    assert!(roaded < unroaded);
    assert!(game.city_grave_danger(&game.cities[0]) > 0);
    assert!(game.movement_cost_for(&actor, front.0, front.1).is_some());
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
    let marine = game.spawn_unit(1, UnitKind::Marine, home.0, home.1);
    game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    game.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
    let near_home = offset_native_by_map(home.0, home.1, -1, 0);
    let near_front = offset_native_by_map(front.0, front.1, 1, 0);
    game.spawn_unit(HUMAN, UnitKind::Cavalry, near_home.0, near_home.1);
    game.spawn_unit(HUMAN, UnitKind::Cavalry, near_front.0, near_front.1);
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
        tile.terrain = Terrain::Sea;
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
        tile.terrain = Terrain::Sea;
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
