//! Regression tests for the behavior of the parent module.

use super::*;

fn world() -> Simulation {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.prune_unit_ai_state();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.owner = None;
        tile.hut = false;
        tile.known_by.fill(true);
    }
    game
}

fn civ3_east(game: &Simulation, point: (i32, i32), tiles: usize) -> (i32, i32) {
    (0..tiles).fold(point, |(x, y), _| {
        game.map_wrap.civ3_neighbor(x, y, 2).unwrap()
    })
}

#[test]
fn nonmil_traversal_and_zero_attack_carrier_endpoints_preserve_real_execution_legality() {
    assert!(world().ai_normal_path_actions_playtest());
}

#[test]
fn attack_sanity_omits_empty_city_attack_flag_but_default_map_treats_it_as_an_endpoint() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    let city = civ3_east(&game, (10, 20), 1);
    let beyond = civ3_east(&game, (10, 20), 2);
    game.create_city(1, city.0, city.1);
    let actor = game.unit_by_id(id).unwrap();
    let index = Simulation::idx(beyond.0, beyond.1);
    assert_eq!(
        game.leader_paths(actor, None).costs[index],
        3 * MOVE_FRAGMENTS
    );
    assert_eq!(
        game.leader_paths_mode(actor, None, true).costs[index],
        2 * MOVE_FRAGMENTS
    );
}

#[test]
fn sanity_pf_assumes_unknown_source_scope_is_native_even_for_zero_attack_carrier() {
    let mut game = world();
    game.tile_mut(10, 20).terrain = Terrain::Sea;
    let bridge = offset_native_by_map(10, 20, 1, 0);
    let target = offset_native_by_map(10, 20, 2, 0);
    game.tile_mut(target.0, target.1).terrain = Terrain::Sea;
    game.tile_mut(bridge.0, bridge.1).known_by[HUMAN] = false;
    let id = game.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
    game.spawn_unit(BARBARIANS, UnitKind::Caravel, target.0, target.1);
    let actor = game.unit_by_id(id).unwrap();
    let index = Simulation::idx(target.0, target.1);
    assert_eq!(game.leader_paths(actor, None).costs[index], i32::MAX);
    assert_eq!(
        game.leader_paths_mode(actor, None, true).costs[index],
        3 * MOVE_FRAGMENTS
    );
}

#[test]
fn explorer_diplomatic_endpoint_costs_one_move_instead_of_ignored_terrain_cost() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    let target = offset_native_by_map(10, 20, 1, 0);
    game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
    let actor = game.unit_by_id(id).unwrap();
    assert_eq!(
        game.leader_paths(actor, None).costs[Simulation::idx(target.0, target.1)],
        MOVE_FRAGMENTS
    );
    assert_eq!(game.movement_cost_for(actor, target.0, target.1), Some(2));
}

#[test]
fn desktop_leader_fixture_exercises_refuge_and_escape() {
    assert!(world().ai_barbarian_leader_playtest());
}

#[test]
fn leader_sentries_in_a_survivable_stack_but_idles_without_any_threat() {
    let mut game = world();
    let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
    game.run_barbarian_leader(leader);
    assert_eq!(game.units[0].state, UnitState::Idle);
    assert_eq!((game.units[0].x, game.units[0].y), (10, 20));
    game.spawn_unit(BARBARIANS, UnitKind::Settlers, 10, 20);
    game.run_barbarian_leader(leader);
    assert_eq!(game.units[0].state, UnitState::Sentry);
    assert_eq!(game.units[0].moves_left, 2 * MOVE_FRAGMENTS);
}

#[test]
fn leader_seeks_the_nearest_reachable_friendly_nonleader_without_a_strength_requirement() {
    let mut game = world();
    let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
    let close = offset_native_by_map(10, 20, 2, 0);
    let far = offset_native_by_map(10, 20, 0, 4);
    game.spawn_unit(BARBARIANS, UnitKind::Warrior, far.0, far.1);
    game.spawn_unit(BARBARIANS, UnitKind::Worker, close.0, close.1);
    game.run_barbarian_leader(leader);
    let unit = game.unit_by_id(leader).unwrap();
    assert_eq!((unit.x, unit.y), close);
}

#[test]
fn leader_keeps_reverse_aircraft_threat_but_does_not_invent_unreachable_escape_costs() {
    let mut game = world();
    let enemy = game.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
    let point = offset_native_by_map(10, 20, 1, 0);
    let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, point.0, point.1);
    let actor = game.unit_by_id(enemy).unwrap();
    let index = Simulation::idx(point.0, point.1);
    assert!(game.leader_paths(actor, Some((index, BARBARIANS))).costs[index] < i32::MAX);
    game.run_barbarian_leader(leader);
    let unit = game.unit_by_id(leader).unwrap();
    // No cities, bases, or carriers exist: native PF_FUEL cannot leave
    // the fighter's start, although reverse PF_NORMAL selected it.
    assert_eq!((unit.x, unit.y), point);
    assert_eq!(unit.state, UnitState::Idle);
    assert_eq!(unit.moves_left, 2 * MOVE_FRAGMENTS);
}

#[test]
fn leader_flees_increasing_enemy_path_cost_and_does_not_randomly_wander() {
    let mut game = world();
    let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
    let enemy = offset_native_by_map(10, 20, -1, 0);
    game.spawn_unit(HUMAN, UnitKind::Diplomat, enemy.0, enemy.1);
    game.run_barbarian_leader(leader);
    let unit = game.unit_by_id(leader).unwrap();
    assert!(game.map_distance(unit.x, unit.y, enemy.0, enemy.1) > 1);
    assert_eq!(unit.moves_left, 0);
}

#[test]
fn leader_generated_zoc_can_limit_finite_enemy_cost_escape_before_moves_run_out() {
    let mut game = world();
    let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
    let enemy = offset_native_by_map(10, 20, -1, 0);
    game.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
    game.run_barbarian_leader(leader);
    let actor = game.unit_by_id(leader).unwrap();
    assert_eq!(actor.moves_left, MOVE_FRAGMENTS);
    assert_eq!(actor.state, UnitState::Idle);
    assert!(game.map_distance(actor.x, actor.y, enemy.0, enemy.1) > 1);
}

#[test]
fn refuge_sanity_gate_is_optimistic_about_unknown_non_native_terrain() {
    let mut game = world();
    for tile in &mut game.tiles {
        tile.known_by[BARBARIANS] = false;
    }
    game.tile_mut(10, 20).terrain = Terrain::Sea;
    let id = game.spawn_unit(BARBARIANS, UnitKind::Caravel, 10, 20);
    let boat = game.unit_by_id(id).unwrap();
    let target = offset_native_by_map(10, 20, 2, 0);
    let index = Simulation::idx(target.0, target.1);
    assert_eq!(game.leader_paths(boat, None).costs[index], i32::MAX);
    assert!(game.leader_paths_mode(boat, None, true).costs[index] < i32::MAX);
}

#[test]
fn damaged_enemy_reverse_horizon_uses_its_current_rate_not_full_health_type_speed() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Horseman, 10, 20);
    let mut actor = game
        .units
        .iter()
        .find(|unit| unit.id == id)
        .unwrap()
        .clone();
    let target = offset_native_by_map(10, 20, 5, 0);
    let index = Simulation::idx(target.0, target.1);
    assert_eq!(game.unit_move_rate(&actor), 2 * MOVE_FRAGMENTS);
    assert_eq!(
        game.leader_paths(&actor, Some((index, BARBARIANS))).costs[index],
        5 * MOVE_FRAGMENTS
    );
    actor.hp = actor.kind.stats().hp / 2;
    actor.moves_left = 0;
    assert_eq!(game.unit_move_rate(&actor), MOVE_FRAGMENTS);
    assert_eq!(
        game.leader_paths(&actor, Some((index, BARBARIANS))).costs[index],
        i32::MAX
    );
}

#[test]
fn immobile_nuclear_type_does_not_become_a_reverse_threat_via_a_fake_minimum_rate() {
    let mut game = world();
    game.players[HUMAN].known.clear();
    let id = game.spawn_unit(HUMAN, UnitKind::Nuclear, 10, 20);
    let actor = game.unit_by_id(id).unwrap();
    assert_eq!(game.unit_move_rate(actor), 0);
    let target = offset_native_by_map(10, 20, 1, 0);
    let index = Simulation::idx(target.0, target.1);
    assert_eq!(
        game.leader_paths(actor, Some((index, BARBARIANS))).costs[index],
        i32::MAX
    );
}

#[test]
fn reverse_query_assumes_full_moves_and_rejects_four_complete_turns() {
    let mut game = world();
    let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    let mut actor = game
        .units
        .iter()
        .find(|unit| unit.id == enemy)
        .unwrap()
        .clone();
    actor.moves_left = 0;
    let near = offset_native_by_map(10, 20, 3, 0);
    let far = offset_native_by_map(10, 20, 4, 0);
    let near_index = Simulation::idx(near.0, near.1);
    let far_index = Simulation::idx(far.0, far.1);
    assert_eq!(
        game.leader_paths(&actor, Some((near_index, BARBARIANS)))
            .costs[near_index],
        3 * MOVE_FRAGMENTS
    );
    assert_eq!(
        game.leader_paths(&actor, Some((far_index, BARBARIANS)))
            .costs[far_index],
        i32::MAX
    );
}

#[test]
fn reverse_query_keeps_the_native_permissive_unarmed_and_nonnative_target_quirks() {
    let mut game = world();
    let enemy = game.spawn_unit(HUMAN, UnitKind::Settlers, 10, 20);
    let actor = game
        .units
        .iter()
        .find(|unit| unit.id == enemy)
        .unwrap()
        .clone();
    let target = offset_native_by_map(10, 20, 1, 0);
    game.tile_mut(target.0, target.1).terrain = Terrain::Sea;
    let target_index = Simulation::idx(target.0, target.1);
    assert_eq!(
        game.leader_paths(&actor, Some((target_index, BARBARIANS)))
            .costs[target_index],
        MOVE_FRAGMENTS
    );
}

#[test]
fn leader_releases_ferry_command_to_mobile_nonleader_cargo() {
    let mut game = world();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    let boat = game.spawn_unit(BARBARIANS, UnitKind::Caravel, 10, 20);
    let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
    let warrior = game.spawn_unit(BARBARIANS, UnitKind::Warrior, 10, 20);
    assert!(game.board_specific_ship(leader, boat));
    assert!(game.board_specific_ship(warrior, boat));
    game.refresh_ai_ferries();
    assert!(game.assign_ai_ferry(boat, leader));
    game.run_barbarian_leader(leader);
    assert_eq!(game.ferry_boss(boat), Some(warrior));
    assert!(!game.ai_ferries.passengers.contains_key(&leader));
}
