//! Regression tests for the behavior of the parent module.

use super::*;

fn world() -> Simulation {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    sim.cities.clear();
    sim.prune_unit_ai_state();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.base = None;
        tile.hut = false;
        tile.known_by.fill(true);
    }
    sim
}

fn actor(sim: &mut Simulation, kind: UnitKind, position: (i32, i32)) -> Unit {
    let id = sim.spawn_unit(HUMAN, kind, position.0, position.1);
    sim.unit_by_id(id).unwrap().clone()
}

fn civ3_east(sim: &Simulation, point: (i32, i32), tiles: usize) -> (i32, i32) {
    (0..tiles).fold(point, |(x, y), _| {
        sim.map_wrap.civ3_neighbor(x, y, 2).unwrap()
    })
}



#[test]
fn type_fuel_selects_the_map_even_when_instance_fuel_is_malformed() {
    let mut sim = world();
    let mut ship = actor(&mut sim, UnitKind::Caravel, (10, 20));
    ship.fuel = 1;
    assert!(FuelPaths::new(&sim, &ship, true).is_none());
    let mut plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
    plane.fuel = 0;
    let map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(map.nodes[map.start].moves, moves_field(map.initial_bundle));
    assert_eq!(moves_field(-1), 4095);
    assert_eq!(required_field(-1), -1);
    assert_eq!(required_field(4096), -4096);
}

#[test]
fn an_isolated_aircraft_without_any_refuel_point_cannot_leave_its_start() {
    let mut sim = world();
    let plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.spawn_unit(BARBARIANS, UnitKind::Warrior, target.0, target.1);
    let mut map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(map.movement_cost(&sim, 10, 20), Some(0));
    assert_eq!(map.movement_cost(&sim, target.0, target.1), None);
}

#[test]
fn waiting_for_refuel_preserves_the_earliest_position_separately_from_more_fuel() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = civ3_east(&sim, (10, 20), 1);
    // Four tiles out and four back requires the Conquests Stealth Bomber's
    // 16-point operational range; a Fighter's range is only 6.
    let mut plane = actor(&mut sim, UnitKind::StealthBomber, start);
    plane.fuel = 1;
    plane.moves_left = MOVE_FRAGMENTS;
    let target = civ3_east(&sim, (10, 20), 4);
    let earlier = civ3_east(&sim, (10, 20), 2);
    let mut map = FuelPaths::new(&sim, &plane, true).unwrap();
    // Return one step, wait at the city to refuel, then fly to the goal.
    assert_eq!(
        map.movement_cost(&sim, target.0, target.1),
        Some(6 * MOVE_FRAGMENTS)
    );
    assert_eq!(
        map.movement_cost(&sim, earlier.0, earlier.1),
        Some(4 * MOVE_FRAGMENTS)
    );
    assert_eq!(map.movement_cost(&sim, start.0, start.1), Some(0));
    assert!(
        map.positions
            .iter()
            .any(|position| position.previous.is_some())
    );
    let path = map.path(&sim, target.0, target.1).unwrap();
    assert_eq!(path.iter().filter(|&&step| step == (0, 0)).count(), 1);
    let mut point = start;
    for &(dx, dy) in &path {
        if (dx, dy) != (0, 0) {
            point = sim.map_wrap.civ3_offset(point.0, point.1, dx, dy).unwrap();
        }
    }
    assert_eq!(point, target);
    let live = sim
        .units
        .iter_mut()
        .find(|unit| unit.id == plane.id)
        .unwrap();
    live.fuel = plane.fuel;
    live.moves_left = plane.moves_left;
    let result = sim.follow_source_ai_path(plane.id, path);
    assert!(result.alive && result.moved && !result.at_goal);
    let live = sim.unit_by_id(plane.id).unwrap();
    assert_eq!((live.x, live.y), (10, 20));
    assert_eq!(live.moves_left, 0);
}

#[test]
fn refuel_waits_are_recorded_at_the_city_but_not_added_to_path_cost() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let mut plane = actor(&mut sim, UnitKind::StealthBomber, (10, 20));
    plane.fuel = 1;
    let target = offset_native_by_map(10, 20, 8, 0);
    let mut boundary = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(
        boundary.movement_cost(&sim, target.0, target.1),
        Some(8 * MOVE_FRAGMENTS)
    );
    assert_eq!(
        boundary.path(&sim, target.0, target.1).unwrap().first(),
        Some(&(0, 0))
    );
    // With no movement left, the route still records the city wait but the
    // native PF cost excludes that zero-movement boundary.
    plane.moves_left = 0;
    let mut partial = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(
        partial.movement_cost(&sim, target.0, target.1),
        Some(8 * MOVE_FRAGMENTS)
    );
    assert_eq!(
        partial.path(&sim, target.0, target.1).unwrap().first(),
        Some(&(0, 0))
    );
}

#[test]
fn fuel_goto_executes_only_the_prefix_before_refuel_wait() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = offset_native_by_map(10, 20, 1, 0);
    let target = offset_native_by_map(10, 20, 4, 0);
    let plane = actor(&mut sim, UnitKind::StealthBomber, start);
    let live = sim
        .units
        .iter_mut()
        .find(|unit| unit.id == plane.id)
        .unwrap();
    live.moves_left = MOVE_FRAGMENTS;
    live.fuel = 1;
    let plane = live.clone();
    assert!(sim.ai_goto_is_sane(&plane, target));
    let (path, cost) = sim
        .ai_goto_path(&plane, target, super::super::normal_paths::GotoTask::None)
        .unwrap();
    assert!(path.contains(&(0, 0)));
    assert_eq!(cost, 5 * MOVE_FRAGMENTS);
    let result = sim.ai_goto_unit(plane.id, target, super::super::normal_paths::GotoTask::None);
    assert!(result.alive && result.moved && !result.at_goal);
    let live = sim.unit_by_id(plane.id).unwrap();
    assert_eq!((live.x, live.y), (10, 20));
    assert_eq!(live.moves_left, 0);
}

#[test]
fn one_attack_bomber_returns_to_refuel_instead_of_attacking_on_last_fuel_turn() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = offset_native_by_map(10, 20, 1, 0);
    let target = offset_native_by_map(10, 20, 2, 0);
    let mut plane = actor(&mut sim, UnitKind::Bomber, start);
    sim.spawn_unit(BARBARIANS, UnitKind::Warrior, target.0, target.1);
    let rate = sim.unit_move_rate(&plane);
    plane.fuel = 1;
    let mut low = FuelPaths::new(&sim, &plane, true).unwrap();
    assert!(!low.attack_possible(rate, MOVE_FRAGMENTS));
    assert_eq!(low.movement_cost(&sim, target.0, target.1), Some(3 * rate));
    plane.fuel = plane.kind.fuel_capacity();
    let mut full = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(full.movement_cost(&sim, target.0, target.1), Some(rate));
}

#[test]
fn fighter_attack_uses_return_requirement_and_unfuelled_missiles_skip_fuel_paths() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let mut fighter = actor(&mut sim, UnitKind::Fighter, (10, 20));
    fighter.fuel = 1;
    fighter.moves_left = MOVE_FRAGMENTS;
    let map = FuelPaths::new(&sim, &fighter, true).unwrap();
    assert!(map.attack_possible(MOVE_FRAGMENTS, 0));
    assert!(!map.attack_possible(MOVE_FRAGMENTS, MOVE_FRAGMENTS));
    let missile = actor(&mut sim, UnitKind::CruiseMissile, (10, 20));
    assert_eq!(missile.kind.fuel_capacity(), 0);
    assert!(FuelPaths::new(&sim, &missile, true).is_none());
}

#[test]
fn refuel_callback_keeps_enemy_base_and_omniscient_known_map_quirks() {
    let mut sim = world();
    let plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
    let base = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(base.0, base.1).base = Some(Base::Airstrip);
    sim.tile_mut(base.0, base.1).owner = Some(1);
    for tile in &mut sim.tiles {
        tile.known_by[HUMAN] = false;
    }
    let map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(map.required_moves(&sim, Simulation::idx(base.0, base.1)), 0);
    assert_eq!(map.required_moves(&sim, Simulation::idx(10, 20)), -1);
    sim.tile_mut(base.0, base.1).known_by[HUMAN] = true;
    assert_eq!(
        map.required_moves(&sim, Simulation::idx(10, 20)),
        MOVE_FRAGMENTS
    );
    assert!(sim.refuel_site(HUMAN, base.0, base.1));
}

#[test]
fn initialized_action_nodes_retain_native_cached_endpoints_when_the_live_stack_changes() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(target.0, target.1).terrain = Terrain::Sea;
    let ship = sim.spawn_unit(1, UnitKind::Carrier, target.0, target.1);
    sim.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
    let mut cached = FuelPaths::new(&sim, &plane, true).unwrap();
    assert!(cached.initialize(&sim, Simulation::idx(target.0, target.1)));
    sim.units.retain(|unit| unit.id != ship);
    let mut fresh = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(fresh.movement_cost(&sim, target.0, target.1), None);
    assert_eq!(
        cached.movement_cost(&sim, target.0, target.1),
        Some(sim.unit_move_rate(&plane))
    );
}

#[test]
fn full_allied_carrier_is_a_possible_refuel_point_and_unreachable_aircraft_do_not_shield_a_ship() {
    let mut sim = world();
    sim.tile_mut(10, 20).terrain = Terrain::Sea;
    let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
    let carrier = sim.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
    // The installed Conquests Carrier has capacity 4.
    for _ in 0..4 {
        let cargo = sim.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
        assert!(sim.board_specific_ship(cargo, carrier));
    }
    assert_eq!(sim.cargo_count(carrier), 4);
    assert!(!sim.board_specific_ship(plane.id, carrier));
    let map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert!(map.possible_base(&sim, 10, 20));
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(target.0, target.1).terrain = Terrain::Sea;
    let ship = sim.spawn_unit(1, UnitKind::Carrier, target.0, target.1);
    sim.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
    assert_eq!(map.attack_at(&sim, target.0, target.1, true), Ok(true));
    sim.units.retain(|unit| unit.id != ship);
    assert_eq!(map.attack_at(&sim, target.0, target.1, true), Err(()));
}
