//! Regression tests for the behavior of the parent module.

use super::*;

mod helpers;

fn world() -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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

/// Steps east on Civ III's eight-direction grid.
fn east_by(game: &Simulation, point: (i32, i32), tiles: usize) -> (i32, i32) {
    (0..tiles).fold(point, |(x, y), _| {
        game.map_wrap.eight_way_neighbor(x, y, 2).unwrap()
    })
}

#[test]
fn carrier_path_fixture_uses_imported_attack_capability() {
    assert!(world().ai_zero_attack_carrier_path_test());
}

#[test]
fn attack_sanity_omits_empty_city_attack_flag_but_default_map_treats_it_as_an_endpoint() {
    let mut game = world();
    game.sign_pact(HUMAN, 1, Pact::War);
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    let city = east_by(&game, (10, 20), 1);
    let beyond = east_by(&game, (10, 20), 2);
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
    game.tile_mut(bridge.0, bridge.1).known_by.set(HUMAN, false);
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
fn explorer_hostile_endpoint_uses_imported_terrain_movement_cost() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    let target = offset_native_by_map(10, 20, 1, 0);
    game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
    let actor = game.unit_by_id(id).unwrap();
    let expected = game.movement_cost_for(actor, target.0, target.1).unwrap();
    assert_eq!(expected, 2); // Imported Explorer movement cost on grassland.
    assert_eq!(
        game.leader_paths(actor, None).costs[Simulation::idx(target.0, target.1)],
        expected
    );
}

#[test]
fn refuge_sanity_gate_is_optimistic_about_unknown_non_native_terrain() {
    let mut game = world();
    for tile in &mut game.tiles {
        tile.known_by.set(BARBARIANS, false);
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
fn tactical_nuke_uses_its_imported_movement_rate_in_reverse_paths() {
    let mut game = world();
    game.players[HUMAN].known.clear();
    let id = game.spawn_unit(HUMAN, UnitKind::TacticalNuke, 10, 20);
    let actor = game.unit_by_id(id).unwrap();
    assert_eq!(
        game.unit_move_rate(actor),
        UnitKind::TacticalNuke.stats().moves * MOVE_FRAGMENTS
    );
    assert!(game.unit_move_rate(actor) > 0);
    let target = offset_native_by_map(10, 20, 1, 0);
    let index = Simulation::idx(target.0, target.1);
    assert_eq!(
        game.leader_paths(actor, Some((index, BARBARIANS))).costs[index],
        game.movement_cost_for(actor, target.0, target.1).unwrap()
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
    let enemy = game.spawn_unit(HUMAN, UnitKind::Settler, 10, 20);
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

fn army_empire(owner: usize, count: usize) -> (Simulation, u32) {
    let mut game = world();
    for index in 0..count {
        game.create_city(
            owner,
            8 + (index % 6) as i32 * 4,
            12 + (index / 6) as i32 * 6,
        );
    }
    let leader = game.spawn_unit(owner, UnitKind::Leader, 8, 12);
    (game, leader)
}

#[test]
fn form_army_consumes_the_leader_and_preserves_owner_home_and_remaining_moves() {
    let (mut game, leader) = army_empire(HUMAN, 4);
    let home = game.cities[0].id;
    game.units
        .iter_mut()
        .find(|unit| unit.id == leader)
        .unwrap()
        .moves_left = 1;
    let rng = game.random_snapshot();
    assert!(game.can_form_army(game.unit_by_id(leader).unwrap()));
    assert!(game.unit_action_view(leader).unwrap().form_army);
    assert!(game.apply(Command::FormArmy(leader)));
    assert!(game.unit_by_id(leader).is_none());
    let army = game
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Army)
        .unwrap();
    assert_eq!(
        (army.owner, army.x, army.y, army.home_city),
        (HUMAN, 8, 12, Some(home))
    );
    assert_eq!(army.moves_left, 1);
    assert_eq!(game.random_snapshot(), rng);
    assert!(!game.apply(Command::FormArmy(leader)));
}

#[test]
fn army_formation_checks_city_ownership_capacity_transport_and_moves() {
    let (mut game, leader) = army_empire(HUMAN, 3);
    assert!(!game.apply(Command::FormArmy(leader)));
    assert!(game.unit_by_id(leader).is_some());
    game.create_city(1, 30, 30); // Foreign cities do not provide Army capacity.
    assert!(!game.apply(Command::FormArmy(leader)));
    game.create_city(HUMAN, 24, 24);
    let index = game
        .units
        .iter()
        .position(|unit| unit.id == leader)
        .unwrap();
    game.units[index].moves_left = 0;
    assert!(!game.apply(Command::FormArmy(leader)));
    game.units[index].moves_left = MOVE_FRAGMENTS;
    game.units[index].transport = Some(9999);
    assert!(!game.apply(Command::FormArmy(leader)));
    game.units[index].transport = None;
    game.units[index].x = 9;
    assert!(!game.apply(Command::FormArmy(leader)));
    game.units[index].x = 8;
    assert!(game.apply(Command::FormArmy(leader)));
    let second = game.spawn_unit(HUMAN, UnitKind::Leader, 8, 12);
    assert!(!game.apply(Command::FormArmy(second)));
    for x in [4, 12, 20, 28] {
        game.create_city(HUMAN, x, 36);
    }
    assert!(game.apply(Command::FormArmy(second)));
}

#[test]
fn army_cannot_be_produced_or_queued_even_with_all_technologies_and_military_academy() {
    let (mut game, _) = army_empire(HUMAN, 4);
    game.players[HUMAN].known = Tech::all().collect();
    let city = game.cities[0].id;
    game.cities[0].buildings.push(Building::MILITARY_ACADEMY);
    assert!(!game.unit_available(HUMAN, UnitKind::Army));
    assert!(!game.can_produce(&game.cities[0], Production::Unit(UnitKind::Army)));
    assert!(!game.apply(Command::Produce(city, Production::Unit(UnitKind::Army))));
    assert!(!game.apply(Command::EditWorklist {
        city_id: city,
        edit: WorklistEdit::Append(Production::Unit(UnitKind::Army))
    }));
    let enemy_leader = game.spawn_unit(1, UnitKind::Leader, 8, 12);
    assert!(!game.apply(Command::FormArmy(enemy_leader)));
    assert!(game.unit_by_id(enemy_leader).is_some());
}

#[test]
fn pentagon_requires_three_owned_armies_and_rechecks_after_an_army_is_lost() {
    let (mut game, _) = army_empire(HUMAN, 4);
    game.players[HUMAN].known = Tech::all().collect();
    assert_eq!(
        game.building_blocker(&game.cities[0], Building::PENTAGON)
            .as_deref(),
        Some("Needs 3 armies")
    );
    for _ in 0..3 {
        game.spawn_unit(1, UnitKind::Army, 32, 24);
    }
    assert!(!game.can_produce(&game.cities[0], Production::Building(Building::PENTAGON)));
    let first = game.spawn_unit(HUMAN, UnitKind::Army, 8, 12);
    game.spawn_unit(HUMAN, UnitKind::Army, 8, 12);
    assert!(!game.can_produce(&game.cities[0], Production::Building(Building::PENTAGON)));
    game.spawn_unit(HUMAN, UnitKind::Army, 8, 12);
    assert!(game.can_produce(&game.cities[0], Production::Building(Building::PENTAGON)));
    game.remove_units_with_cargo_because(vec![first], lifecycle::UnitRemoval::Combat);
    assert!(!game.can_produce(&game.cities[0], Production::Building(Building::PENTAGON)));
}

#[test]
fn a_formed_army_round_trips_through_campaign_saves() {
    let (mut game, leader) = army_empire(HUMAN, 4);
    assert!(game.apply(Command::FormArmy(leader)));
    let path =
        std::env::temp_dir().join(format!("civcraft-formed-army-{}.save", std::process::id()));
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        game.encode_campaign().unwrap(),
        loaded.encode_campaign().unwrap()
    );
    assert_eq!(
        loaded
            .units
            .iter()
            .filter(|unit| unit.kind == UnitKind::Army)
            .count(),
        1
    );
    assert!(!loaded.unit_available(HUMAN, UnitKind::Army));
}
