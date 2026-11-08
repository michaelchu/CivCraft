//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn desktop_goto_fixture_exercises_risk_detour_and_final_attack() {
    assert!(world().ai_source_goto_playtest());
}

fn world() -> Simulation {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    sim.cities.clear();
    sim.prune_unit_ai_state();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.base = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.hut = false;
        tile.known_by.fill(true);
    }
    sim
}
fn unit(sim: &mut Simulation, owner: usize, kind: UnitKind) -> Unit {
    let id = sim.spawn_unit(owner, kind, 10, 20);
    sim.unit_by_id(id).unwrap().clone()
}

#[test]
fn source_goto_profiles_distinguish_ferry_hunter_worker_and_action_endpoints() {
    let mut sim = world();
    let carrier = unit(&mut sim, 1, UnitKind::Carrier);
    let normal = sim.source_goto_profile(&carrier, GotoTask::Escort);
    assert!(normal.0 == Style::Overlap && normal.1 == TilePolicy::NoFights && normal.2);
    let hunter = sim.source_goto_profile(&carrier, GotoTask::Hunter);
    assert!(hunter.0 == Style::Default && hunter.1 == TilePolicy::NoIntermediateFights);
    let worker = unit(&mut sim, 1, UnitKind::Worker);
    assert!(sim.source_goto_profile(&worker, GotoTask::Attack).1 == TilePolicy::NoFights);
    let explorer = unit(&mut sim, 1, UnitKind::Explorer);
    let target = offset_native_by_map(10, 20, 1, 0);
    sim.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
    assert!(
        sim.ai_goto_path(&explorer, target, GotoTask::Trade)
            .is_some()
    );
    assert!(
        NormalPaths::new(
            &sim,
            &explorer,
            Style::Default,
            TilePolicy::NoFights,
            true,
            false
        )
        .movement_cost(&sim, target.0, target.1)
        .is_none()
    );
}

#[test]
fn native_risk_paths_can_spend_more_movement_to_avoid_a_vulnerable_stack() {
    let mut sim = world();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Sea;
    }
    let actor = unit(&mut sim, 1, UnitKind::Caravel);
    let stack = offset_native_by_map(10, 20, 1, 0);
    let target = offset_native_by_map(10, 20, 2, 0);
    for _ in 0..6 {
        sim.spawn_unit(1, UnitKind::Battleship, stack.0, stack.1);
    }
    let plain = NormalPaths::new(
        &sim,
        &actor,
        Style::Overlap,
        TilePolicy::NoFights,
        true,
        false,
    )
    .movement_cost(&sim, target.0, target.1);
    assert_eq!(plain, Some(2 * MOVE_FRAGMENTS));
    let (path, cost) = sim.ai_goto_path(&actor, target, GotoTask::None).unwrap();
    assert_eq!(cost, 3 * MOVE_FRAGMENTS);
    let mut position = (10, 20);
    for step in path {
        position = offset_native_by_map(position.0, position.1, step.0, step.1);
        assert_ne!(position, stack);
    }
    assert_eq!(position, target);
    let mut barbarian = actor.clone();
    barbarian.owner = BARBARIANS;
    for unit in &mut sim.units {
        unit.owner = BARBARIANS;
    }
    assert_eq!(
        sim.ai_goto_path(&barbarian, target, GotoTask::None)
            .unwrap()
            .1,
        2 * MOVE_FRAGMENTS
    );
}

#[test]
fn overlap_stops_after_one_nonnative_tile_and_sanity_still_checks_known_map() {
    let mut sim = world();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Sea;
    }
    let actor = unit(&mut sim, 1, UnitKind::Caravel);
    let center = offset_native_by_map(10, 20, 2, 0);
    for point in terrain_work::neighbors(center.0, center.1).chain(std::iter::once(center)) {
        sim.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
    }
    let outer = offset_native_by_map(10, 20, 1, 0);
    let mut map = NormalPaths::new(
        &sim,
        &actor,
        Style::Overlap,
        TilePolicy::NoFights,
        true,
        false,
    );
    assert_eq!(
        map.movement_cost(&sim, outer.0, outer.1),
        Some(sim.unit_move_rate(&actor))
    );
    assert_eq!(map.movement_cost(&sim, center.0, center.1), None);
    assert!(sim.ai_goto_path(&actor, outer, GotoTask::None).is_none());
}

#[test]
fn potential_transport_capacity_is_a_planning_scope_but_execution_checks_real_space() {
    let mut sim = world();
    let actor = unit(&mut sim, 1, UnitKind::Warrior);
    let sea = offset_native_by_map(10, 20, 1, 0);
    sim.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
    let boat = sim.spawn_unit(1, UnitKind::Trireme, sea.0, sea.1);
    for _ in 0..2 {
        let cargo = sim.spawn_unit(1, UnitKind::Warrior, sea.0, sea.1);
        assert!(sim.board_specific_ship(cargo, boat));
    }
    let mut map = NormalPaths::new(
        &sim,
        &actor,
        Style::Default,
        TilePolicy::Default,
        true,
        false,
    );
    assert_eq!(map.movement_cost(&sim, sea.0, sea.1), Some(MOVE_FRAGMENTS));
    assert_eq!(sim.try_move_unit(actor.id, 1, 0), MoveResult::Blocked);
}

#[test]
fn distant_goto_preserves_reference_zoc_instead_of_intended_long_path_relaxation() {
    let mut sim = world();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Sea;
        tile.known_by[1] = false;
    }
    for step in 0..=8 {
        let p = offset_native_by_map(10, 20, step, 0);
        sim.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
        sim.tile_mut(p.0, p.1).known_by[1] = true;
    }
    let enemy = offset_native_by_map(10, 20, 3, 1);
    sim.tile_mut(enemy.0, enemy.1).terrain = Terrain::Grassland;
    sim.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
    let actor = unit(&mut sim, 1, UnitKind::Warrior);
    let goal = offset_native_by_map(10, 20, 8, 0);
    assert!(
        NormalPaths::new(
            &sim,
            &actor,
            Style::Attack,
            TilePolicy::Default,
            false,
            false
        )
        .movement_cost(&sim, goal.0, goal.1)
        .is_some()
    );
    assert!(sim.ai_goto_path(&actor, goal, GotoTask::Attack).is_none());
}

#[test]
fn source_risk_uses_extra_attributes_not_fort_defense_effects() {
    let mut sim = world();
    let actor = unit(&mut sim, 1, UnitKind::Warrior);
    let p = offset_native_by_map(10, 20, 1, 0);
    let ordinary = sim.explorer_stack_risk(&actor, p.0, p.1);
    sim.tile_mut(p.0, p.1).base = Some(Base::Fortress);
    assert_eq!(sim.explorer_stack_risk(&actor, p.0, p.1), ordinary);
    sim.tile_mut(p.0, p.1).river = true;
    assert_eq!(sim.pf_extra_class_defense(actor.kind, p.0, p.1), 25);
    assert_eq!(sim.explorer_stack_risk(&actor, p.0, p.1), ordinary); // 25/100 truncates.
}

#[test]
fn ai_goto_executes_final_combat_and_restores_nonferry_goal_but_sets_ferry_goal() {
    let mut sim = world();
    let actor = unit(&mut sim, 1, UnitKind::Legion);
    let target = offset_native_by_map(10, 20, 1, 0);
    let leader = sim.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, target.0, target.1);
    sim.unit_goto_tiles.insert(actor.id, (20, 40));
    let result = sim.ai_goto_unit(actor.id, target, GotoTask::Attack);
    assert!(result.alive && result.moved);
    assert!(sim.units.iter().all(|unit| unit.id != leader));
    assert_eq!(sim.unit_goto_tiles.get(&actor.id), Some(&(20, 40)));
    sim.units.clear();
    sim.prune_unit_ai_state();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Sea;
    }
    let boat = unit(&mut sim, 1, UnitKind::Caravel);
    sim.unit_goto_tiles.insert(boat.id, (20, 40));
    let result = sim.ai_goto_unit(boat.id, target, GotoTask::None);
    assert!(result.alive && result.moved && result.at_goal);
    assert_eq!(sim.unit_goto_tiles.get(&boat.id), Some(&target));
}
