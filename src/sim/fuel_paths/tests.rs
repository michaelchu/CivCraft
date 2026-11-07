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

#[test]
fn desktop_fuel_fixture_observes_real_leader_dispatch_before_and_after_a_refuel_point() {
    assert!(world().ai_aircraft_fuel_path_playtest());
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
    sim.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, target.0, target.1);
    let mut map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(map.movement_cost(&sim, 10, 20), Some(0));
    assert_eq!(map.movement_cost(&sim, target.0, target.1), None);
}

#[test]
fn waiting_and_revisiting_preserve_the_earliest_position_separately_from_more_fuel() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = offset_native_by_map(10, 20, 1, 0);
    let mut plane = actor(&mut sim, UnitKind::Fighter, start);
    plane.fuel = 1;
    plane.moves_left = 3 * MOVE_FRAGMENTS;
    let target = offset_native_by_map(10, 20, 4, 0);
    let earlier = offset_native_by_map(10, 20, 2, 0);
    let mut map = FuelPaths::new(&sim, &plane, true).unwrap();
    // Return one step, spend two MP waiting, then fly four steps.
    assert_eq!(
        map.movement_cost(&sim, target.0, target.1),
        Some(7 * MOVE_FRAGMENTS)
    );
    assert_eq!(
        map.movement_cost(&sim, earlier.0, earlier.1),
        Some(MOVE_FRAGMENTS)
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
    let mut visited = Vec::new();
    for &(dx, dy) in &path {
        point = offset_native_by_map(point.0, point.1, dx, dy);
        visited.push(point);
    }
    assert_eq!(point, target);
    assert!(visited.contains(&start)); // B -> A, wait, B -> C -> D -> E.
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
    assert_eq!(live.moves_left, 2 * MOVE_FRAGMENTS);
}

#[test]
fn full_turn_boundary_refills_implicitly_but_partial_turn_refill_pays_its_wait() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let mut plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
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
    plane.moves_left = 3 * MOVE_FRAGMENTS;
    let mut partial = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(
        partial.movement_cost(&sim, target.0, target.1),
        Some(11 * MOVE_FRAGMENTS)
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
    let plane = actor(&mut sim, UnitKind::Fighter, start);
    let live = sim
        .units
        .iter_mut()
        .find(|unit| unit.id == plane.id)
        .unwrap();
    live.moves_left = 3 * MOVE_FRAGMENTS;
    live.fuel = 1;
    let plane = live.clone();
    assert!(sim.ai_goto_is_sane(&plane, target));
    let (path, cost) = sim
        .ai_goto_path(&plane, target, super::super::normal_paths::GotoTask::None)
        .unwrap();
    assert!(path.contains(&(0, 0)));
    assert_eq!(cost, 7 * MOVE_FRAGMENTS);
    let result = sim.ai_goto_unit(plane.id, target, super::super::normal_paths::GotoTask::None);
    assert!(result.alive && result.moved && !result.at_goal);
    let live = sim.unit_by_id(plane.id).unwrap();
    assert_eq!((live.x, live.y), (10, 20));
    assert_eq!(live.moves_left, 2 * MOVE_FRAGMENTS);
}

#[test]
fn one_attack_bomber_returns_to_refuel_instead_of_attacking_on_last_fuel_turn() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let start = offset_native_by_map(10, 20, 1, 0);
    let target = offset_native_by_map(10, 20, 2, 0);
    let mut plane = actor(&mut sim, UnitKind::Bomber, start);
    sim.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, target.0, target.1);
    let rate = sim.unit_move_rate(&plane);
    plane.fuel = 1;
    let mut low = FuelPaths::new(&sim, &plane, true).unwrap();
    assert!(!low.attack_possible(rate, MOVE_FRAGMENTS));
    assert_eq!(low.movement_cost(&sim, target.0, target.1), Some(2 * rate));
    plane.fuel = plane.kind.fuel_capacity();
    let mut full = FuelPaths::new(&sim, &plane, true).unwrap();
    assert_eq!(full.movement_cost(&sim, target.0, target.1), Some(rate));
}

#[test]
fn fighter_attack_uses_source_return_requirement_and_missiles_can_be_suicidal() {
    let mut sim = world();
    sim.create_city(HUMAN, 10, 20);
    let mut fighter = actor(&mut sim, UnitKind::Fighter, (10, 20));
    fighter.fuel = 1;
    fighter.moves_left = MOVE_FRAGMENTS;
    let map = FuelPaths::new(&sim, &fighter, true).unwrap();
    assert!(map.attack_possible(MOVE_FRAGMENTS, 0));
    assert!(!map.attack_possible(MOVE_FRAGMENTS, MOVE_FRAGMENTS));
    let missile = actor(&mut sim, UnitKind::CruiseMissile, (10, 20));
    let map = FuelPaths::new(&sim, &missile, true).unwrap();
    assert!(map.attack_possible(MOVE_FRAGMENTS, 10 * MOVE_FRAGMENTS));
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
    sim.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
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
    sim.tile_mut(10, 20).terrain = Terrain::Ocean;
    let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
    let carrier = sim.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
    for _ in 0..8 {
        let cargo = sim.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
        assert!(sim.board_specific_ship(cargo, carrier));
    }
    assert_eq!(sim.cargo_count(carrier), 8);
    assert!(!sim.board_specific_ship(plane.id, carrier));
    let map = FuelPaths::new(&sim, &plane, true).unwrap();
    assert!(map.possible_base(&sim, 10, 20));
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
    let ship = sim.spawn_unit(1, UnitKind::Carrier, target.0, target.1);
    sim.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
    assert_eq!(map.attack_at(&sim, target.0, target.1, true), Ok(true));
    sim.units.retain(|unit| unit.id != ship);
    assert_eq!(map.attack_at(&sim, target.0, target.1, true), Err(()));
}
