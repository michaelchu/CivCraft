//! Regression tests for the behavior of the parent module.

use super::*;

fn game() -> Simulation {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
fn worker_road_value_counts_civ3_neighbor_connections() {
    let mut game = game();
    let center = (10, 20);
    let neighbors: Vec<_> = game.map_wrap.civ3_neighbors(center.0, center.1).collect();
    assert_eq!(neighbors.len(), 8);
    let disconnected = game.worker_road_extra(center, false);
    for (x, y) in neighbors {
        game.tile_mut(x, y).road = true;
    }
    let connected = game.worker_road_extra(center, false);
    assert!(disconnected > connected);
}

#[test]
fn a_sea_worker_uses_the_actual_sea_pathfinder_and_ocean_cleanup_requests() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.transport_auto_worker_playtest());
}

#[test]
fn sea_worker_routes_reject_known_land_and_use_native_unknown_terrain_cost() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    actor.kind = UnitKind::Diplomat;
    assert!(
        !game.worker_safe_tile(1, 10, 20),
        "zero-attack hostile diplomatic actions count"
    );
    game.units.iter_mut().find(|u| u.id == enemy).unwrap().kind = UnitKind::Cannon;
    game.tile_mut(position.0, position.1).terrain = Terrain::Mountains;
    assert!(
        game.worker_safe_tile(1, 10, 20),
        "BigLand cannot threaten a non-native mountain"
    );
    game.tile_mut(10, 20).road = true;
    assert!(!game.worker_safe_tile(1, 10, 20));
    let guard = game.spawn_unit(1, UnitKind::Transport, 10, 20);
    assert!(
        !game.worker_safe_tile(1, 10, 20),
        "NonMil transport is not a guard"
    );
    game.units.iter_mut().find(|u| u.id == guard).unwrap().kind = UnitKind::Carrier;
    assert!(
        game.worker_safe_tile(1, 10, 20),
        "zero-attack military carrier is a guard"
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
fn idle_default_ai_worker_stays_put_while_human_automation_seeks_shelter() {
    let mut game = game();
    assert_eq!(game.ai_worker_planning_playtest(), (true, true, true));
    game.units.clear();
    game.cities[0].owner = 1;
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
    let id = game.spawn_unit(1, UnitKind::Migrants, 10, 20);
    let mut requests = vec![WorkerRequest {
        city,
        destination: (10, 20),
        activity: WorkKind::Road,
        want: 1,
    }];
    let processed = game.run_worker_phase_with_requests(1, &mut requests);
    assert!(
        processed.contains(&id),
        "Migrants carry the Workers ruleset flag"
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
