//! Regression tests for the behavior of the parent module.

use super::*;

fn game() -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.river = false;
        tile.resource = None;
        tile.road = false;
        tile.railroad = false;
        tile.irrigation = false;
        tile.farmland = false;
        tile.mine = false;
        tile.pollution = false;
        tile.fallout = false;
        tile.hut = false;
        tile.base = None;
    }
    game
}

fn plan(old: f32, new: f32, worked: bool, delay: i32) -> WorkerPlan {
    WorkerPlan {
        destination: (10, 20),
        activity: WorkKind::Irrigation,
        path: vec![],
        arrival: 0,
        delay,
        old,
        new,
        extra: 0.0,
        worked,
        request: None,
    }
}

#[test]
fn worker_road_bonus_rewards_network_connectors_not_isolated_spurs() {
    let mut game = game();
    let center = (10, 20);
    assert_eq!(game.worker_road_extra(center, false), 0.0);

    // A road to the north can connect through the candidate tile to the
    // ordinary, fast-build terrain on the far side.
    let north = offset_native_by_map(center.0, center.1, 0, -1);
    game.tile_mut(north.0, north.1).road = true;
    assert!(game.worker_road_extra(center, false) > 0.0);
}

#[test]
fn worker_road_bonus_ignores_connectors_whose_destinations_are_too_slow() {
    let mut game = game();
    let center = (10, 20);
    let north = offset_native_by_map(center.0, center.1, 0, -1);
    game.tile_mut(north.0, north.1).road = true;
    for offset in [(-1, 1), (0, 1), (1, 1)] {
        let (x, y) = offset_native_by_map(center.0, center.1, offset.0, offset.1);
        game.tile_mut(x, y).terrain = Terrain::Mountains;
    }

    assert_eq!(game.worker_road_extra(center, false), 0.0);
}

#[test]
fn only_the_imported_worker_has_worker_action_flags() {
    assert!(UnitKind::Worker.stats().can_work);
    assert!(!UnitKind::Transport.stats().can_work);
}

#[test]
fn sea_worker_routes_reject_known_land_and_use_native_unknown_terrain_cost() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.known_by[HUMAN] = true;
    }
    let start = (10, 24);
    let sea = offset_native_by_map(start.0, start.1, 1, 0);
    let land = offset_native_by_map(start.0, start.1, 0, 1);
    game.tile_mut(start.0, start.1).terrain = Terrain::Sea;
    game.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
    game.tile_mut(sea.0, sea.1).known_by[HUMAN] = false;
    game.spawn_unit(HUMAN, UnitKind::Transport, start.0, start.1);
    let routes = game.worker_routes(game.units.last().unwrap());
    assert_eq!(routes.costs[Simulation::idx(land.0, land.1)], i32::MAX);
    assert_eq!(
        routes.costs[Simulation::idx(sea.0, sea.1)],
        2 * MOVE_FRAGMENTS
    );
}

#[test]
fn default_ai_plans_remote_work_displaces_reservations_and_shelters() {
    let mut game = game();
    assert_eq!(game.ai_worker_planning_playtest(), (true, true, true));
}

#[test]
fn worker_phase_reuses_city_tile_projection_values() {
    let mut game = game();
    game.create_city(1, 10, 20);
    let first = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let second = game.spawn_unit(1, UnitKind::Worker, 11, 20);
    let first = game.unit_by_id(first).unwrap().clone();
    let second = game.unit_by_id(second).unwrap().clone();
    let mut state = WorkMap::default();

    game.trace.enable_profiling();
    game.trace.begin_turn_profile(1, 1, 2);
    assert!(game.worker_plan(&first, &mut state, &[]).is_some());
    assert!(game.worker_plan(&second, &mut state, &[]).is_some());
    game.trace.finish_turn_profile();

    let profiles = game.trace.drain_profiles();
    let operations = &profiles[0].operations;
    assert!(operations["worker.value_cache_hits"] > 0);
    assert_eq!(
        operations["worker.value_cache_hits"],
        operations["worker.projections_avoided"]
    );
    assert!(operations["worker.safety_cache_hits"] > 0);
}

#[test]
fn unworked_candidates_compare_future_total_until_worked_tiles_compete() {
    let mut best = None;
    consider_action(&mut best, plan(20.0, 100.0, false, 1));
    consider_action(&mut best, plan(90.0, 110.0, false, 20));
    assert_eq!(best.as_ref().unwrap().new, 110.0);
    consider_action(&mut best, plan(80.0, 105.0, true, 1));
    assert!(best.as_ref().unwrap().worked);
    consider_action(&mut best, plan(50.0, 90.0, false, 1));
    assert!(
        best.as_ref().unwrap().worked,
        "unworked gain is halved against worked gain"
    );
    consider_action(&mut best, plan(50.0, 110.0, false, 1));
    assert!(!best.as_ref().unwrap().worked);
}

#[test]
fn worker_safety_uses_hostile_actions_native_terrain_and_civ3_radius() {
    let mut game = game();
    let position = (10, 20);
    let wrap = game.map_wrap;
    let step = |from: (i32, i32)| wrap.civ3_offset(from.0, from.1, 2, 0).unwrap();
    let far = step(step(step(position)));
    let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, far.0, far.1);
    assert!(
        game.worker_safe_tile(1, 10, 20),
        "a hostile three Civ III steps away is outside the safety radius"
    );
    let near = step(position);
    let actor = game.units.iter_mut().find(|u| u.id == enemy).unwrap();
    (actor.x, actor.y) = near;
    actor.kind = UnitKind::Tank;
    game.tile_mut(position.0, position.1).terrain = Terrain::Mountains;
    assert!(
        !game.worker_safe_tile(1, 10, 20),
        "nearby imported Tank can threaten the worksite"
    );
    game.tile_mut(10, 20).road = true;
    assert!(!game.worker_safe_tile(1, 10, 20));
    let guard = game.spawn_unit(1, UnitKind::Settler, 10, 20);
    assert!(
        !game.worker_safe_tile(1, 10, 20),
        "nonmilitary Settler is not a guard"
    );
    game.units.iter_mut().find(|u| u.id == guard).unwrap().kind = UnitKind::Carrier;
    assert!(
        game.worker_safe_tile(1, 10, 20),
        "positive-attack Carrier is a guard"
    );
}

#[test]
fn worker_routes_do_not_cross_foreign_territory_and_clip_last_move() {
    let mut game = game();
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let actor = game.unit_by_id(id).unwrap().clone();
    let neighbor = offset_native_by_map(10, 20, 1, 0);
    game.tile_mut(neighbor.0, neighbor.1).terrain = Terrain::Mountains;
    let routes = game.worker_routes(&actor);
    assert_eq!(
        routes.costs[Simulation::idx(neighbor.0, neighbor.1)],
        actor.moves_left
    );
    game.tile_mut(neighbor.0, neighbor.1).owner = Some(HUMAN);
    assert!(game.worker_routes(&actor).path(neighbor).is_none());
}

#[test]
fn target_limited_worker_routes_match_full_search_for_costs_and_paths() {
    let mut game = game();
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let actor = game.unit_by_id(id).unwrap().clone();
    let targets: HashSet<_> = [(10, 20), (11, 20), (14, 21)]
        .into_iter()
        .map(|(x, y)| game.map_wrap.index(x, y))
        .collect();

    let full = game.worker_routes(&actor);
    let limited = game.worker_routes_to(&actor, Some(targets.clone()));

    for target in targets {
        let position = game.map_wrap.position(target);
        assert_eq!(limited.costs[target], full.costs[target]);
        assert_eq!(limited.path(position), full.path(position));
    }
    assert!(limited.expanded_nodes < full.expanded_nodes);
}

#[test]
fn target_limited_worker_routes_exhaust_search_for_unreachable_targets() {
    let mut game = game();
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let actor = game.unit_by_id(id).unwrap().clone();
    for tile in &mut game.tiles {
        tile.owner = Some(HUMAN);
    }
    let start = (10, 20);
    game.tile_mut(start.0, start.1).owner = None;
    let target = offset_native_by_map(start.0, start.1, 1, 0);
    let targets = HashSet::from([game.map_wrap.index(target.0, target.1)]);

    let full = game.worker_routes(&actor);
    let limited = game.worker_routes_to(&actor, Some(targets));

    assert_eq!(
        limited.costs[game.map_wrap.index(target.0, target.1)],
        i32::MAX
    );
    assert_eq!(limited.expanded_nodes, full.expanded_nodes);
    assert_eq!(limited.path(target), full.path(target));
}

#[test]
fn same_worker_task_preserves_goal_and_displaced_null_goal_is_not_recreated() {
    let mut game = game();
    game.create_city(1, 10, 20);
    game.tile_mut(10, 20).road = false;
    let city = game.cities.last().unwrap().id;
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let mut requests = vec![WorkerRequest {
        city,
        destination: (10, 20),
        activity: WorkKind::Road,
        want: 1,
    }];
    game.run_worker_phase_with_requests(1, &mut requests);
    assert!(game.ai_worker_tasks.contains(&id));
    assert_eq!(game.unit_goto_tiles.get(&id), Some(&(10, 20)));
    game.units
        .iter_mut()
        .find(|u| u.id == id)
        .unwrap()
        .clear_work();
    let near = offset_native_by_map(10, 20, 1, 0);
    game.tile_mut(10, 20).road = true;
    requests.push(WorkerRequest {
        city,
        destination: near,
        activity: WorkKind::Road,
        want: 1,
    });
    game.run_worker_phase_with_requests(1, &mut requests);
    assert_eq!(
        game.unit_goto_tiles.get(&id),
        Some(&(10, 20)),
        "same AUT_AUTO_WORKER role does not rewrite its goto"
    );
    game.unit_goto_tiles.remove(&id); // native displacement clears goto, not role
    game.units
        .iter_mut()
        .find(|u| u.id == id)
        .unwrap()
        .clear_work();
    game.units
        .iter_mut()
        .find(|u| u.id == id)
        .unwrap()
        .moves_left = 6;
    game.tile_mut(near.0, near.1).road = false;
    requests.clear();
    requests.push(WorkerRequest {
        city,
        destination: near,
        activity: WorkKind::Road,
        want: 1,
    });
    game.run_worker_phase_with_requests(1, &mut requests);
    assert!(!game.unit_goto_tiles.contains_key(&id));
}

#[test]
fn idle_default_ai_worker_stays_put_without_generic_shelter_fallback() {
    let mut game = game();
    // The remote-plan/reservation/human-shelter scenario is covered above.
    // Build just its job-free end state for this distinct AI fallback check.
    for tile in &mut game.tiles {
        tile.road = true;
        tile.irrigation = true;
        tile.farmland = true;
        tile.mine = true;
        tile.oil_well = false;
    }
    game.create_city(1, 20, 20);
    for tile in &mut game.tiles {
        tile.owner = None;
    }
    game.players[HUMAN].known.clear();
    let near = offset_native_by_map(20, 20, 1, 0);
    let id = game.spawn_unit(1, UnitKind::Worker, near.0, near.1);
    game.players[1].known.clear();
    game.run_ai_worker_phase(1);
    let actor = game.unit_by_id(id).unwrap();
    assert_eq!(
        (actor.x, actor.y),
        near,
        "default AI callback has no generic seek-shelter fallback"
    );
}

#[test]
fn explicit_city_request_starts_work_and_removes_only_fulfilled_request() {
    let mut game = game();
    game.create_city(1, 10, 20);
    let city = game.cities.last().unwrap().id;
    game.tile_mut(10, 20).road = false;
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let mut requests = vec![WorkerRequest {
        city,
        destination: (10, 20),
        activity: WorkKind::Road,
        want: 1,
    }];
    let processed = game.run_worker_phase_with_requests(1, &mut requests);
    assert!(
        processed.contains(&id),
        "Workers carry the Workers ruleset flag"
    );
    assert!(requests.is_empty());
    let worker = game.unit_by_id(id).unwrap();
    assert_eq!(worker.work_kind, Some(WorkKind::Road));
    assert!(worker.work_left > 0);
}

#[test]
fn closer_worker_can_displace_an_arrival_reservation_but_not_active_work() {
    let mut game = game();
    let far = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let near = game.spawn_unit(1, UnitKind::Worker, 11, 20);
    let actor = game.unit_by_id(near).unwrap().clone();
    let mut state = WorkMap::default();
    state.reservations.insert((12, 20), (far, 2));
    assert!(game.worker_can_reserve(&actor, (12, 20), 1, &state));
    assert!(!game.worker_can_reserve(&actor, (12, 20), 3, &state));
    game.tile_mut(11, 20).pollution = true;
    assert!(game.begin_work(near, WorkKind::Clean(Contamination::Pollution)));
    let before = bincode::serialize(game.unit_by_id(near).unwrap()).unwrap();
    game.run_auto_worker(near);
    assert_eq!(
        before,
        bincode::serialize(game.unit_by_id(near).unwrap()).unwrap()
    );
}

#[test]
fn virtual_evaluation_restores_tiles_and_shelter_does_not_randomly_wander() {
    let mut game = game();
    game.create_city(1, 10, 20);
    let city = game.cities.last().unwrap().clone();
    let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
    let actor = game.unit_by_id(id).unwrap().clone();
    let before = bincode::serialize(&game.tiles).unwrap();
    let mut irrigated = game.tile(10, 20).clone();
    irrigated.irrigation = true;
    game.worker_tile_value(&city, (10, 20), irrigated);
    assert_eq!(before, bincode::serialize(&game.tiles).unwrap());
    game.cities.clear();
    let before = bincode::serialize(&game).unwrap();
    game.run_auto_worker(id);
    assert_eq!(
        before,
        bincode::serialize(&game).unwrap(),
        "no useful work or shelter preserves RNG and position"
    );
    assert_eq!(actor.id, id);
}
