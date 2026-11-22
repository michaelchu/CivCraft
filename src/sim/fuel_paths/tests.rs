//! Regression tests for the behavior of the parent module.

use super::*;

fn world() -> Simulation {
    let mut sim = Simulation::new(crate::test_support::seeded_map());
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

/// Steps east on Civ III's eight-direction grid.
fn east_by(sim: &Simulation, point: (i32, i32), tiles: usize) -> (i32, i32) {
    (0..tiles).fold(point, |(x, y), _| {
        sim.map_wrap.eight_way_neighbor(x, y, 2).unwrap()
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
fn waiting_for_refuel_preserves_the_earliest_position_for_the_mapped_fuel_budget() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = east_by(&sim, (10, 20), 1);
    let airstrip = east_by(&sim, (10, 20), 2);
    sim.tile_mut(airstrip.0, airstrip.1).base = Some(Base::Airstrip);
    // Cruise Missiles remain fueled units handled by FuelPaths.
    let mut missile = actor(&mut sim, UnitKind::CruiseMissile, start);
    missile.fuel = 1;
    missile.moves_left = MOVE_FRAGMENTS;
    let mut map = FuelPaths::new(&sim, &missile, true).unwrap();
    assert_eq!(
        map.movement_cost(&sim, airstrip.0, airstrip.1),
        Some(MOVE_FRAGMENTS)
    );
    assert_eq!(map.movement_cost(&sim, start.0, start.1), Some(0));
    let path = map.path(&sim, airstrip.0, airstrip.1).unwrap();
    let result = sim.follow_source_ai_path(missile.id, path);
    assert!(result.alive && result.moved && result.at_goal);
    let live = sim.unit_by_id(missile.id).unwrap();
    assert_eq!((live.x, live.y), airstrip);
    assert_eq!(live.moves_left, 0);
}

#[test]
fn refuel_waits_are_recorded_at_the_city_without_extending_the_mapped_fuel_range() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let airstrip = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(airstrip.0, airstrip.1).base = Some(Base::Airstrip);
    let mut missile = actor(&mut sim, UnitKind::CruiseMissile, (10, 20));
    missile.fuel = 1;
    let mut boundary = FuelPaths::new(&sim, &missile, true).unwrap();
    assert_eq!(
        boundary.movement_cost(&sim, airstrip.0, airstrip.1),
        Some(MOVE_FRAGMENTS)
    );
    let path = boundary.path(&sim, airstrip.0, airstrip.1).unwrap();
    assert!(
        !path.contains(&(0, 0)),
        "a full native mission budget needs no refuel wait"
    );
    missile.moves_left = 0;
    let mut partial = FuelPaths::new(&sim, &missile, true).unwrap();
    assert_eq!(
        partial.movement_cost(&sim, airstrip.0, airstrip.1),
        Some(MOVE_FRAGMENTS)
    );
    assert_eq!(
        partial.path(&sim, airstrip.0, airstrip.1).unwrap().first(),
        Some(&(0, 0))
    );
}

#[test]
fn fuel_goto_executes_only_the_prefix_before_refuel_wait() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = offset_native_by_map(10, 20, 1, 0);
    let airstrip = offset_native_by_map(10, 20, 2, 0);
    let target = offset_native_by_map(10, 20, 3, 0);
    sim.tile_mut(airstrip.0, airstrip.1).base = Some(Base::Airstrip);
    sim.tile_mut(target.0, target.1).base = Some(Base::Airstrip);
    let missile = actor(&mut sim, UnitKind::CruiseMissile, start);
    let live = sim
        .units
        .iter_mut()
        .find(|unit| unit.id == missile.id)
        .unwrap();
    live.moves_left = MOVE_FRAGMENTS;
    live.fuel = 1;
    let missile = live.clone();
    assert!(sim.ai_goto_is_sane(&missile, target));
    let (path, cost) = sim
        .ai_goto_path(&missile, target, super::super::normal_paths::GotoTask::None)
        .unwrap();
    assert!(path.contains(&(0, 0)));
    assert_eq!(cost, 2 * MOVE_FRAGMENTS);
    let result = sim.ai_goto_unit(
        missile.id,
        target,
        super::super::normal_paths::GotoTask::None,
    );
    assert!(result.alive && result.moved && !result.at_goal);
    let live = sim.unit_by_id(missile.id).unwrap();
    assert_eq!((live.x, live.y), airstrip);
    assert_eq!(live.moves_left, 0);
}

#[test]
fn native_bombers_have_one_mission_budget_and_use_bombard_instead_of_normal_attack() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.spawn_unit(BARBARIANS, UnitKind::Warrior, target.0, target.1);
    assert_eq!(plane.kind.fuel_capacity(), 1);
    assert_eq!(sim.unit_move_rate(&plane), MOVE_FRAGMENTS);
    assert_eq!(plane.kind.operational_range(), 10);
    assert_eq!(plane.kind.rebase_range(), 30);
    assert_eq!(plane.kind.stats().attack, 0);
    assert!(!sim.apply(Command::move_by_offset(&sim, plane.id, 1, 0)));
}

#[test]
fn fighter_attack_uses_return_requirement_and_missiles_use_selected_fuel_paths() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let mut fighter = actor(&mut sim, UnitKind::Fighter, (10, 20));
    fighter.fuel = 1;
    fighter.moves_left = MOVE_FRAGMENTS;
    let map = FuelPaths::new(&sim, &fighter, true).unwrap();
    assert!(map.attack_possible(MOVE_FRAGMENTS, 0));
    assert!(!map.attack_possible(MOVE_FRAGMENTS, MOVE_FRAGMENTS));
    let missile = actor(&mut sim, UnitKind::CruiseMissile, (10, 20));
    assert_eq!(missile.kind.fuel_capacity(), 1);
    assert!(FuelPaths::new(&sim, &missile, true).is_some());
}

#[test]
fn refuel_callback_keeps_enemy_base_and_omniscient_known_map_quirks() {
    let mut sim = world();
    let plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
    let base = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(base.0, base.1).base = Some(Base::Airstrip);
    sim.tile_mut(base.0, base.1).owner = Some(1);
    for tile in &mut sim.tiles {
        tile.known_by.set(HUMAN, false);
    }
    let map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(map.required_moves(&sim, Simulation::idx(base.0, base.1)), 0);
    assert_eq!(map.required_moves(&sim, Simulation::idx(10, 20)), -1);
    sim.tile_mut(base.0, base.1).known_by.set(HUMAN, true);
    assert_eq!(
        map.required_moves(&sim, Simulation::idx(10, 20)),
        MOVE_FRAGMENTS
    );
    assert!(sim.refuel_site(HUMAN, base.0, base.1));
}

#[test]
fn initialized_action_nodes_retain_native_cached_endpoints_when_the_live_stack_changes() {
    let mut sim = world();
    sim.sign_pact(HUMAN, 1, Pact::War);
    sim.create_city(HUMAN, 10, 20);
    let plane = actor(&mut sim, UnitKind::CruiseMissile, (10, 20));
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(target.0, target.1).terrain = Terrain::Sea;
    let ship = sim.spawn_unit(1, UnitKind::Carrier, target.0, target.1);
    sim.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
    let mut cached = FuelPaths::new(&sim, &plane, true).unwrap();
    assert!(cached.initialize(&sim, Simulation::idx(target.0, target.1)));
    sim.units.retain(|unit| unit.id != ship);
    let mut fresh = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(fresh.movement_cost(&sim, target.0, target.1), None);
    let index = Simulation::idx(target.0, target.1);
    assert!(
        cached.nodes[index].attack,
        "the original stack was an attack endpoint"
    );
    assert!(cached.initialize(&sim, index));
    assert!(
        cached.nodes[index].attack,
        "initialization preserves the cached action"
    );
    assert!(
        !fresh.nodes[index].attack,
        "a fresh map sees the changed stack"
    );
}

#[test]
fn full_allied_carrier_is_a_possible_refuel_point_and_unreachable_aircraft_do_not_shield_a_ship() {
    let mut sim = world();
    sim.sign_pact(HUMAN, 1, Pact::War);
    sim.tile_mut(10, 20).terrain = Terrain::Sea;
    let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
    let carrier = sim.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
    // The selected C2C3 Carrier record has capacity 8.
    for _ in 0..UnitKind::Carrier.transport_capacity() {
        let cargo = sim.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
        assert!(sim.board_specific_ship(cargo, carrier));
    }
    assert_eq!(
        sim.cargo_count(carrier),
        UnitKind::Carrier.transport_capacity()
    );
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
