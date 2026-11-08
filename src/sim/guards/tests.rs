//! Regression tests for the behavior of the parent module.

use super::*;
fn game() -> Simulation {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    game.ai_guard_targets.clear();
    game.ai_unit_guards.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.river = false;
        tile.river_edges = 0;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
    }
    game
}

#[test]
fn sane_rendezvous_retains_escort_when_omniscient_policy_cannot_pass_enemy() {
    let mut game = game();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.known_by[1] = false;
    }
    let start = (10, 24);
    let point = |step| offset_native_by_map(start.0, start.1, step, 0);
    for step in 0..=4 {
        let p = point(step);
        game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    }
    let finish = point(4);
    let charge = game.spawn_unit(1, UnitKind::Catapult, finish.0, finish.1);
    let guard = game.spawn_unit(1, UnitKind::Musketeers, start.0, start.1);
    let barrier = point(2);
    game.spawn_unit(HUMAN, UnitKind::Warrior, barrier.0, barrier.1);
    game.tile_mut(barrier.0, barrier.1).known_by[1] = false;
    assert!(game.assign_ai_guard(guard, GuardTarget::Unit(charge)));
    let actor = game.unit_by_id(guard).unwrap();
    assert!(game.ai_goto_is_sane(actor, finish));
    assert!(game.ai_goto_path(actor, finish, GotoTask::Escort).is_none());
    assert!(game.run_ai_guard(guard));
    assert_eq!(
        game.ai_guard_targets.get(&guard),
        Some(&GuardTarget::Unit(charge))
    );
    assert_eq!(game.unit_goto_tiles.get(&guard), Some(&finish));
    assert!(
        game.units
            .iter()
            .any(|u| u.id == guard && (u.x, u.y) == start)
    );
}

#[test]
fn guard_links_are_typed_reciprocal_and_dismissed_after_death_or_transfer() {
    let mut game = game();
    let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 20);
    let guard = game.spawn_unit(1, UnitKind::Phalanx, 10, 20);
    game.create_city(1, 10, 20);
    game.cities.last_mut().unwrap().id = charge; // local unit/city IDs overlap
    game.request_ai_guard(charge);
    assert!(game.assign_ai_guard(guard, GuardTarget::Unit(charge)));
    assert!(game.validate_ai_guards());
    assert!(game.assign_ai_guard(guard, GuardTarget::City(charge)));
    assert_eq!(game.ai_guard_state(charge), GuardState::None);
    assert!(game.validate_ai_guards());
    game.cities.last_mut().unwrap().owner = HUMAN;
    game.refresh_ai_guards();
    assert!(!game.ai_guard_targets.contains_key(&guard));
    game.assign_ai_guard(guard, GuardTarget::Unit(charge));
    game.forget_ai_guard_unit(guard);
    assert_eq!(
        game.ai_guard_state(charge),
        GuardState::None,
        "native clears rather than re-requests when guard dies"
    );
    assert!(game.validate_ai_guards());
}

#[test]
fn charge_selection_obeys_role_speed_native_classes_and_defense_delta() {
    let mut game = game();
    let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 20);
    let guard = game.spawn_unit(1, UnitKind::Musketeers, 10, 20);
    game.request_ai_guard(charge);
    let actor = game.units.last().unwrap().clone();
    assert_eq!(
        game.look_for_ai_charge(&actor),
        Some((GuardTarget::Unit(charge), 91.0))
    );
    assert!(game.run_ai_guard(guard));
    assert_eq!(game.ai_guard_state(charge), GuardState::Assigned(guard));
    assert!(!Simulation::guard_can_follow(
        UnitKind::Cannon,
        UnitKind::Settlers
    ));
    assert!(
        !Simulation::guard_can_follow(UnitKind::AegisCruiser, UnitKind::Trireme),
        "Trireme's River-native extra is not native to Sea"
    );
    assert!(Simulation::guard_can_follow(
        UnitKind::AegisCruiser,
        UnitKind::Transport
    ));
    assert!(
        !Simulation::guard_can_follow(UnitKind::StealthFighter, UnitKind::Settlers),
        "terrain coverage alone does not imply native-extra coverage"
    );
    game.clear_ai_charge(guard);
    game.request_ai_guard(charge);
    game.units.iter_mut().find(|u| u.id == charge).unwrap().kind = UnitKind::Diplomat;
    assert!(
        game.look_for_ai_charge(&actor).is_none(),
        "a one-move guard cannot follow a two-move charge"
    );
}

#[test]
fn stored_generic_goal_wins_and_guard_intercepts_ahead_of_naval_charge() {
    let mut game = game();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    let point = |step| offset_native_by_map(10, 20, step, 0);
    let charge_at = point(5);
    let goal = point(1);
    let charge = game.spawn_unit(1, UnitKind::Transport, charge_at.0, charge_at.1);
    let guard = game.spawn_unit(1, UnitKind::AegisCruiser, 10, 20);
    game.assign_ai_guard(guard, GuardTarget::Unit(charge));
    let old_mission = point(9);
    game.tile_mut(old_mission.0, old_mission.1).terrain = Terrain::Grassland;
    game.create_city(HUMAN, old_mission.0, old_mission.1);
    game.ai_ground_targets
        .insert(charge, game.cities.last().unwrap().id);
    game.unit_goto_tiles.insert(charge, goal);
    let actor = game.unit_by_id(guard).unwrap().clone();
    assert_eq!(
        game.ai_guard_rendezvous(&actor, GuardTarget::Unit(charge)),
        Some(goal)
    );
    assert!(game.run_ai_guard(guard));
    assert!(
        game.units
            .iter()
            .any(|u| u.id == guard && (u.x, u.y) == goal)
    );
    assert_eq!(game.unit_goto_tiles.get(&guard), Some(&goal));
}

#[test]
fn city_rescue_competes_by_danger_deficit_and_dismisses_when_defended() {
    let mut game = game();
    game.create_city(1, 10, 20);
    let city = game.cities.last().unwrap().id;
    let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 20);
    let guard = game.spawn_unit(1, UnitKind::Musketeers, 10, 20);
    let near = offset_native_by_map(10, 20, 1, 0);
    for _ in 0..2 {
        game.spawn_unit(HUMAN, UnitKind::Marines, near.0, near.1);
    }
    game.request_ai_guard(charge);
    let actor = game.unit_by_id(guard).unwrap().clone();
    assert_eq!(
        game.look_for_ai_charge(&actor).map(|(target, _)| target),
        Some(GuardTarget::City(city))
    );
    assert!(game.run_ai_guard(guard));
    assert_eq!(
        game.ai_guard_targets.get(&guard),
        Some(&GuardTarget::City(city))
    );
    game.spawn_unit(1, UnitKind::Musketeers, 10, 20);
    assert!(game.run_ai_guard(guard));
    assert_eq!(
        game.ai_guard_targets.get(&guard),
        Some(&GuardTarget::Unit(charge)),
        "enough quadratic city defense releases rescue guard to wanted charge"
    );
}

#[test]
fn injured_charge_speed_uses_native_current_unit_rate() {
    let mut game = game();
    let charge = game.spawn_unit(1, UnitKind::Horseman, 10, 20);
    game.spawn_unit(1, UnitKind::Phalanx, 10, 20);
    let actor = game.units.last().unwrap().clone();
    game.request_ai_guard(charge);
    assert!(game.look_for_ai_charge(&actor).is_none());
    game.units.iter_mut().find(|u| u.id == charge).unwrap().hp = 1;
    assert_eq!(
        game.look_for_ai_charge(&actor).map(|(target, _)| target),
        Some(GuardTarget::Unit(charge))
    );
}

#[test]
fn destination_danger_requests_a_guard_and_safe_destination_releases_it() {
    let mut game = game();
    let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 20);
    let enemy = game.spawn_unit(HUMAN, UnitKind::Legion, 11, 20);
    assert!(game.ai_guard_for_destination(charge, 11, 20));
    assert_eq!(game.ai_guard_state(charge), GuardState::Wanted);
    game.units.retain(|u| u.id != enemy);
    assert!(!game.ai_guard_for_destination(charge, 11, 20));
    assert_eq!(game.ai_guard_state(charge), GuardState::None);
    let guard = game.spawn_unit(1, UnitKind::Phalanx, 10, 20);
    game.assign_ai_guard(guard, GuardTarget::Unit(charge));
    assert!(
        game.ai_guard_for_destination(charge, 11, 20),
        "colocated guard remains wanted even when destination becomes safe"
    );
}

#[test]
fn guard_city_assessment_uses_integer_vulnerability_and_quadratic_defense() {
    let mut game = game();
    game.create_city(1, 10, 20);
    let near = offset_native_by_map(10, 20, 1, 0);
    game.spawn_unit(HUMAN, UnitKind::Warrior, near.0, near.1);
    let city = game.cities.last().unwrap().clone();
    let data = game.advisor_guard_city_assessment(&city);
    assert_eq!(
        (data.danger, data.urgency, data.grave, data.defense),
        (4, 11, 1, 0)
    );
    game.spawn_unit(1, UnitKind::Phalanx, 10, 20);
    let data = game.advisor_guard_city_assessment(&city);
    assert_eq!(
        data.defense, 100,
        "native fortified rating10 squared; class-specific effects are uncertain in generic city query"
    );
    game.units.retain(|u| u.owner == HUMAN);
    game.cities
        .last_mut()
        .unwrap()
        .buildings
        .push(Building::CITY_WALLS);
    let city = game.cities.last().unwrap();
    assert_eq!(game.advisor_guard_city_assessment(city).danger, 1);
}

#[test]
fn movement_waits_for_guard_follows_it_and_attacks_before_waiting() {
    let mut game = game();
    let charge = game.spawn_unit(1, UnitKind::Catapult, 10, 20);
    let guard = game.spawn_unit(1, UnitKind::Musketeers, 10, 20);
    game.assign_ai_guard(guard, GuardTarget::Unit(charge));
    game.units
        .iter_mut()
        .find(|u| u.id == guard)
        .unwrap()
        .moves_left = 0;
    assert_eq!(game.try_move_unit(charge, 1, 0), MoveResult::Blocked);
    game.units
        .iter_mut()
        .find(|u| u.id == guard)
        .unwrap()
        .moves_left = 6;
    assert_eq!(game.try_move_unit(charge, 1, 0), MoveResult::Moved);
    let actor = game.unit_by_id(charge).unwrap().clone();
    assert!(
        game.units
            .iter()
            .any(|u| u.id == guard && (u.x, u.y) == (actor.x, actor.y))
    );
    game.units
        .iter_mut()
        .find(|u| u.id == charge)
        .unwrap()
        .moves_left = 6;
    let target = offset_native_by_map(actor.x, actor.y, 1, 0);
    let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
    game.units.last_mut().unwrap().hp = 1;
    game.set_random_seed(1);
    assert_eq!(game.try_move_unit(charge, 1, 0), MoveResult::Fought);
    assert!(
        game.units.iter().all(|u| u.id != enemy),
        "stationary exhausted guard does not block legal attack"
    );
}

#[test]
fn ferry_waits_for_guard_then_loads_it_with_the_boss() {
    let mut game = game();
    game.create_city(1, 10, 20);
    let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 20);
    let boat = game.spawn_unit(1, UnitKind::Caravel, 10, 20);
    let guard = game.spawn_unit(1, UnitKind::Phalanx, 12, 20);
    game.assign_ai_guard(guard, GuardTarget::Unit(charge));
    assert!(game.board_specific_ship(charge, boat));
    assert!(
        !game.ai_guard_prepare_ferry(charge, boat),
        "guard used its movement to approach, so boss waits"
    );
    let actor = game.units.iter_mut().find(|u| u.id == guard).unwrap();
    (actor.x, actor.y) = (10, 20);
    actor.moves_left = 6;
    assert!(game.ai_guard_prepare_ferry(charge, boat));
    assert_eq!(game.unit_by_id(guard).unwrap().transport, Some(boat));
}
