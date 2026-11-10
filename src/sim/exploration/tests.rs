//! Regression tests for the behavior of the parent module.

use super::*;

fn world() -> Simulation {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.hut = false;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.base = None;
        tile.owner = None;
        tile.known_by.fill(false);
    }
    game
}

#[test]
fn human_warriors_and_ships_explore_but_fuelled_and_losing_hp_types_cannot_activate_it() {
    assert!(world().ai_exploration_legality_playtest());
}

#[test]
fn idle_ai_military_and_ferries_use_frontier_exploration() {
    assert!(world().ai_idle_exploration_playtest());
}

#[test]
fn nonworker_autoexploration_does_not_start_unsolicited_cleanup() {
    let mut game = world();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, 10, 20);
    game.tile_mut(10, 20).pollution = true;
    game.refresh_visibility();
    assert!(game.apply(Command::Automate(ship)));
    game.run_automated_unit(ship);
    let unit = game.unit_by_id(ship).unwrap();
    assert!(unit.work_kind.is_none());
    assert_ne!((unit.x, unit.y), (10, 20));
    assert!(game.tile(10, 20).pollution);
}

#[test]
fn idle_ferry_recovery_uses_allied_ports_and_native_regeneration_penalty() {
    let mut game = world();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.known_by[1] = true;
    }
    let ship = game.spawn_unit(1, UnitKind::Caravel, 10, 20);
    let near = offset_native_by_map(10, 20, 1, 0);
    let far = offset_native_by_map(10, 20, 0, 2);
    game.sign_pact(1, 2, Pact::Alliance);
    for (owner, point) in [(1, near), (2, far)] {
        game.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
        game.create_city(owner, point.0, point.1);
    }
    game.cities
        .iter_mut()
        .find(|city| (city.x, city.y) == far)
        .unwrap()
        .buildings
        .push(Building::PORT_FACILITY);
    game.run_idle_ai_ferry(ship);
    let unit = game.unit_by_id(ship).unwrap();
    assert_eq!((unit.x, unit.y), far);
}

#[test]
fn explorer_stack_risk_uses_current_damaged_unit_speed() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 20);
    let mut actor = game
        .units
        .iter()
        .find(|unit| unit.id == id)
        .unwrap()
        .clone();
    let target = offset_native_by_map(10, 20, 1, 0);
    let healthy = game.explorer_stack_risk(&actor, target.0, target.1);
    let healthy_rate = game.unit_move_rate(&actor);
    actor.hp = actor.kind.stats().hp / 2;
    let damaged_rate = game.unit_move_rate(&actor);
    assert!(damaged_rate > 0 && damaged_rate <= healthy_rate);
    let damaged = game.explorer_stack_risk(&actor, target.0, target.1);
    // Risk scales with the imported current move rate; damage need not
    // halve every unit's speed.
    assert_eq!(damaged * i64::from(damaged_rate), healthy * i64::from(healthy_rate));
}

#[test]
fn native_start_ignore_can_leave_a_forbidden_start_without_entering_forbidden_destinations() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    game.sign_pact(HUMAN, 1, Pact::Peace);
    game.diplomacy_playtest_tick(16);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    game.tile_mut(10, 20).owner = Some(1);
    let next = offset_native_by_map(10, 20, 1, 0);
    game.tile_mut(10, 20).known_by[HUMAN] = true;
    game.tile_mut(next.0, next.1).known_by[HUMAN] = true;
    let unit = game.unit_by_id(id).unwrap();
    assert!(!game.explorer_may_visit(unit, 10, 20));
    assert_eq!(game.explorer_search(unit, None).0, Some(next));
    assert_eq!(game.run_auto_explorer(id), ExploreResult::Okay);
    let unit = game.unit_by_id(id).unwrap();
    assert_eq!((unit.x, unit.y), next);
}

#[test]
fn explorer_and_ferries_cross_peace_borders_but_warriors_and_carriers_do_not() {
    let mut game = world();
    game.sign_pact(HUMAN, 1, Pact::Peace);
    game.diplomacy_playtest_tick(16);
    game.tile_mut(10, 20).owner = Some(1);
    for kind in [
        UnitKind::Explorer,
        UnitKind::Caravel,
        UnitKind::Warrior,
        UnitKind::Carrier,
    ] {
        let id = game.spawn_unit(HUMAN, kind, 8, 18);
        let actor = game.unit_by_id(id).unwrap();
        assert_eq!(game.explorer_may_visit(actor, 10, 20), kind.is_civilian());
    }
}

#[test]
fn native_heap_ties_and_unsigned_short_extra_cost_are_preserved() {
    let mut queue = ExplorerQueue::default();
    for tile in 0..4 {
        queue.put(10, tile);
    }
    assert_eq!(
        (0..4).map(|_| queue.pop().unwrap().1).collect::<Vec<_>>(),
        vec![0, 3, 2, 1]
    );
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    let actor = game
        .units
        .iter()
        .find(|unit| unit.id == id)
        .unwrap()
        .clone();
    let tile = offset_native_by_map(10, 20, 1, 0);
    let baseline = game.explorer_stack_risk(&actor, tile.0, tile.1);
    let adjacent = offset_native_by_map(tile.0, tile.1, 1, 0);
    game.spawn_unit(1, UnitKind::Cavalry, adjacent.0, adjacent.1);
    game.sign_pact(HUMAN, 1, Pact::War);
    let controlled = game.explorer_stack_risk(&actor, tile.0, tile.1);
    assert_eq!(controlled - baseline, PF_TURN_FACTOR * 20);
    assert_eq!(controlled as u16, baseline as u16);
}

#[test]
fn likely_native_keeps_reference_unknown_center_quirk() {
    let mut game = world();
    let point = (10, 20);
    for (x, y) in game.map_wrap.civ3_neighbors(point.0, point.1) {
        game.tile_mut(x, y).known_by[HUMAN] = true;
    }
    assert_eq!(
        game.explorer_likely_native(HUMAN, UnitKind::Explorer, point.0, point.1),
        98
    );
    game.tile_mut(point.0, point.1).terrain = Terrain::Sea;
    assert_eq!(
        game.explorer_likely_native(HUMAN, UnitKind::Explorer, point.0, point.1),
        2
    );
}

#[test]
fn scoring_requires_unknown_sight_but_known_huts_override_it() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    let unit = game.unit_by_id(id).unwrap().clone();
    for tile in &mut game.tiles {
        tile.known_by[HUMAN] = true;
    }
    assert_eq!(game.explorer_desirable(&unit, 12, 24), 0);
    game.tile_mut(12, 24).hut = true;
    assert_eq!(game.explorer_desirable(&unit, 12, 24), 60_902);
    game.tile_mut(12, 24).known_by[HUMAN] = false;
    assert!(game.explorer_desirable(&unit, 12, 24) < HUT_SCORE);
}

#[test]
fn first_search_never_targets_unknown_even_for_omniscient_ai() {
    let mut game = world();
    let id = game.spawn_unit(1, UnitKind::Explorer, 10, 20);
    let unit = game.unit_by_id(id).unwrap().clone();
    for tile in &mut game.tiles {
        tile.known_by[1] = false;
    }
    game.tile_mut(10, 20).known_by[1] = true;
    assert!(game.explorer_search(&unit, None).0.is_none());
    let next = offset_native_by_map(10, 20, 1, 0);
    game.tile_mut(next.0, next.1).known_by[1] = true;
    assert_eq!(game.explorer_search(&unit, None).0, Some(next));
}

#[test]
fn exploration_never_attacks_or_enters_nonallied_cities_or_peace_borders() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    let next = offset_native_by_map(10, 20, 1, 0);
    let enemy = game.spawn_unit(1, UnitKind::Warrior, next.0, next.1);
    let unit = game.unit_by_id(id).unwrap().clone();
    assert!(!game.explorer_may_visit(&unit, next.0, next.1));
    game.units.retain(|u| u.id != enemy);
    game.create_city(1, next.0, next.1);
    assert!(!game.explorer_may_visit(&unit, next.0, next.1));
    game.cities.clear();
    game.sign_pact(HUMAN, 1, Pact::Peace);
    game.diplomacy_playtest_tick(16);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    game.tile_mut(next.0, next.1).owner = Some(1);
    assert!(!game.explorer_may_visit(&unit, next.0, next.1));
}

#[test]
fn second_search_avoids_an_expensive_vulnerable_stack() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    let unit = game.unit_by_id(id).unwrap().clone();
    for tile in &mut game.tiles {
        tile.known_by[HUMAN] = true;
    }
    let stacked = offset_native_by_map(10, 20, 1, 0);
    let goal = offset_native_by_map(10, 20, 2, 0);
    game.spawn_unit(HUMAN, UnitKind::Battleship, stacked.0, stacked.1);
    let (_, routes) = game.explorer_search(&unit, Some(goal));
    let mut cursor = Simulation::idx(goal.0, goal.1);
    while cursor != Simulation::idx(10, 20) {
        assert_ne!(cursor, Simulation::idx(stacked.0, stacked.1));
        cursor = routes.previous[cursor].unwrap().0;
    }
    assert!(
        game.explorer_stack_risk(&unit, stacked.0, stacked.1)
            > game.explorer_stack_risk(&unit, goal.0, goal.1)
    );
}

#[test]
fn paused_exploration_clears_human_automation_without_a_dangerous_last_step() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    let next = offset_native_by_map(10, 20, 1, 0);
    let enemy = offset_native_by_map(10, 20, 2, 0);
    game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
    game.tile_mut(10, 20).known_by[HUMAN] = true;
    game.tile_mut(next.0, next.1).known_by[HUMAN] = true;
    game.tile_mut(enemy.0, enemy.1).visible = true;
    let actor = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
    actor.moves_left = 2;
    actor.automated = true;
    game.run_automated_unit(id);
    let actor = game.unit_by_id(id).unwrap();
    assert_eq!((actor.x, actor.y), (10, 20));
    assert_eq!(actor.moves_left, 2);
    assert!(!actor.automated);
}

#[test]
fn end_of_turn_danger_uses_native_combined_attack_and_terrain_defense() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    let next = offset_native_by_map(10, 20, 1, 0);
    let enemy_tile = offset_native_by_map(10, 20, 2, 0);
    game.spawn_unit(1, UnitKind::Cavalry, enemy_tile.0, enemy_tile.1);
    game.sign_pact(HUMAN, 1, Pact::War);
    game.refresh_visibility();
    game.tile_mut(enemy_tile.0, enemy_tile.1).visible = true;
    let unit = game.unit_by_id(id).unwrap();
    assert!(!game.explorer_danger_at(unit, 10, 20));
    assert!(game.explorer_danger_at(unit, next.0, next.1));
    // A city is a shelter regardless of the adjacent enemy's strength.
    game.create_city(HUMAN, next.0, next.1);
    let unit = game.unit_by_id(id).unwrap();
    assert!(!game.explorer_danger_at(unit, next.0, next.1));
}

#[test]
fn desktop_exploration_fixture_uses_real_automation_and_movement() {
    let mut game = world();
    assert!(game.ai_exploration_playtest());
}

#[test]
fn reaching_a_frontier_recurses_and_reveals_more_without_random_wandering() {
    let mut game = world();
    let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
    game.refresh_visibility();
    let before = game
        .tiles
        .iter()
        .filter(|tile| tile.known_by[HUMAN])
        .count();
    let result = game.run_auto_explorer(id);
    assert_eq!(result, ExploreResult::Okay);
    let actor = game.unit_by_id(id).unwrap();
    assert_ne!((actor.x, actor.y), (10, 20));
    assert_eq!(actor.moves_left, 0);
    assert!(
        game.tiles
            .iter()
            .filter(|tile| tile.known_by[HUMAN])
            .count()
            > before
    );
}
