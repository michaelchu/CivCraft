//! Regression tests for the behavior of the parent module.

use super::*;

fn prepare_zoc_test_map(game: &mut Simulation) {
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
        tile.hut = false;
        tile.owner = None;
        tile.visible = true;
        tile.seen = true;
    }
}

type CapturedUnitState = (u32, usize, UnitKind, i32, i32, Option<u32>, i32, i32);

fn capture_unit_state(game: &Simulation) -> Vec<CapturedUnitState> {
    game.units
        .iter()
        .map(|unit| {
            (
                unit.id,
                unit.owner,
                unit.kind,
                unit.x,
                unit.y,
                unit.home_city,
                unit.moves_left,
                unit.hp,
            )
        })
        .collect()
}

fn blocked_zoc_positions(game: &Simulation) -> ((i32, i32), (i32, i32), (i32, i32)) {
    let start = (10, 20);
    let destination = game.map_wrap.civ3_neighbor(start.0, start.1, 2).unwrap();
    let enemy = game
        .map_wrap
        .civ3_neighbors(start.0, start.1)
        .find(|&candidate| {
            candidate != destination
                && game.map_wrap.civ3_distance(
                    destination.0,
                    destination.1,
                    candidate.0,
                    candidate.1,
                ) == 1
        })
        .unwrap();
    assert_eq!(
        game.map_wrap
            .civ3_distance(start.0, start.1, enemy.0, enemy.1),
        1
    );
    (start, destination, enemy)
}

#[test]
fn activity_completion_is_immediate_and_old_mp_controls_only_work_count() {
    let game = Simulation::new(crate::test_support::seeded_map());
    assert_eq!(
        game.activity_completion_order_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn stealth_fighter_targets_follow_c2c3_unit_classes() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let attacker = game.spawn_unit(HUMAN, UnitKind::StealthFighter, 10, 24);
    let leader = game.spawn_unit(1, UnitKind::Leader, 11, 24);
    let attacker = game.unit_by_id(attacker).unwrap().clone();

    assert_eq!(game.best_defender_id(&attacker, 11, 24), Some(leader));
    let fighter = game.spawn_unit(1, UnitKind::Fighter, 11, 24);
    game.units.retain(|unit| unit.id != leader);
    assert_eq!(game.best_defender_id(&attacker, 11, 24), Some(fighter));
    game.units.retain(|unit| unit.id != fighter);
    assert_eq!(game.best_defender_id(&attacker, 11, 24), None);
}

#[test]
fn ordinary_attackers_keep_their_normal_defender_eligibility() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
    let defender = game.spawn_unit(1, UnitKind::Leader, 11, 24);
    let attacker = game.unit_by_id(attacker).unwrap().clone();

    assert_eq!(game.best_defender_id(&attacker, 11, 24), Some(defender));
}

#[test]
fn game_loss_units_are_last_defenders_only_on_protected_tiles() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (11, 24);
    game.create_city(1, x, y);
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
    let warrior_id = game.spawn_unit(1, UnitKind::Warrior, x, y);
    let leader_id = game.spawn_unit(1, UnitKind::Leader, x, y);
    let attacker = game.unit_by_id(attacker_id).unwrap().clone();
    let leader = game.unit_by_id(leader_id).unwrap().clone();
    assert_eq!(
        crate::sim::action_rules::freeciv_unit_type_flag(UnitKind::Leader, "GameLoss"),
        Some(true)
    );
    assert_eq!(game.defender_rank(&attacker, &leader, x, y).0, -1);
    assert_eq!(game.best_defender_id(&attacker, x, y), Some(warrior_id));

    game.cities.clear();
    game.units.retain(|unit| unit.id != warrior_id);
    assert_ne!(game.defender_rank(&attacker, &leader, x, y).0, -1);
    assert_eq!(game.best_defender_id(&attacker, x, y), Some(leader_id));
}

#[test]
fn defender_choice_uses_civ3_build_cost_when_win_chance_and_rating_tie() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker_id = game.spawn_unit(HUMAN, UnitKind::ModernArmor, x, y);
    // These BIQ units share attack, defense, movement, and HP. Their fixed
    // Civ III catalog costs differ (Enkidu Warrior 10; Spearman 20).
    let cheaper_id = game.spawn_unit(1, UnitKind::EnkiduWarrior, tx, ty);
    let dearer_id = game.spawn_unit(1, UnitKind::Spearman, tx, ty);
    let attacker = game.unit_by_id(attacker_id).unwrap().clone();
    let cheaper = game.unit_by_id(cheaper_id).unwrap().clone();
    let dearer = game.unit_by_id(dearer_id).unwrap().clone();

    assert_eq!(
        (
            cheaper.kind.stats().attack,
            cheaper.kind.stats().defense,
            cheaper.kind.stats().cost,
        ),
        (1, 2, 10)
    );
    assert_eq!(
        (
            dearer.kind.stats().attack,
            dearer.kind.stats().defense,
            dearer.kind.stats().cost,
        ),
        (1, 2, 20)
    );
    let cheaper_rank = game.defender_rank(&attacker, &cheaper, tx, ty);
    let dearer_rank = game.defender_rank(&attacker, &dearer, tx, ty);
    assert_eq!(cheaper_rank.0, dearer_rank.0);
    assert_eq!(cheaper_rank.2, dearer_rank.2);
    assert_eq!(
        game.tile_units_ordered(tx, ty).next().map(|unit| unit.id),
        Some(dearer_id),
        "the more expensive candidate is first, so cost must decide the tie"
    );
    assert_eq!(game.best_defender_id(&attacker, tx, ty), Some(cheaper_id));
}

#[test]
fn defender_choice_uses_defense_rating_after_equal_win_chance_and_cost() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker_id = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    let weaker_id = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    let stronger_id = game.spawn_unit(1, UnitKind::EnkiduWarrior, tx, ty);
    // The zero-attack Worker gives both candidates the same zero attacker-win
    // chance while retaining normal class reachability. The two defenders
    // have equal Civ III cost.
    let attacker = game.unit_by_id(attacker_id).unwrap().clone();
    let weaker = game.unit_by_id(weaker_id).unwrap().clone();
    let stronger = game.unit_by_id(stronger_id).unwrap().clone();
    let weaker_rank = game.defender_rank(&attacker, &weaker, tx, ty);
    let stronger_rank = game.defender_rank(&attacker, &stronger, tx, ty);

    assert_eq!(weaker_rank.0, stronger_rank.0);
    assert_eq!(weaker_rank.1, stronger_rank.1);
    assert!(stronger_rank.2 > weaker_rank.2);
    assert_eq!(game.best_defender_id(&attacker, tx, ty), Some(stronger_id));
}

#[test]
fn defender_choice_keeps_first_tile_list_unit_on_a_full_tie() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker_id = game.spawn_unit(HUMAN, UnitKind::ModernArmor, x, y);
    let earlier_id = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    let first_in_tile_list_id = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    let attacker = game.unit_by_id(attacker_id).unwrap().clone();
    let earlier = game.unit_by_id(earlier_id).unwrap().clone();
    let first_in_tile_list = game.unit_by_id(first_in_tile_list_id).unwrap().clone();

    assert_eq!(
        game.defender_rank(&attacker, &earlier, tx, ty),
        game.defender_rank(&attacker, &first_in_tile_list, tx, ty)
    );
    assert_eq!(
        game.tile_units_ordered(tx, ty).next().map(|unit| unit.id),
        Some(first_in_tile_list_id)
    );
    assert_eq!(
        game.best_defender_id(&attacker, tx, ty),
        Some(first_in_tile_list_id),
        "get_defender only replaces the winner for a strictly better rank"
    );
}

#[test]
fn transported_defenders_follow_selected_tp_defense_and_deboard_rules() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    let carrier = game.spawn_unit(1, UnitKind::Galleon, x, y);
    let cargo = game.spawn_unit(1, UnitKind::Warrior, x, y);
    game.units
        .iter_mut()
        .find(|unit| unit.id == cargo)
        .unwrap()
        .transport = Some(carrier);

    assert!(game.combat_unit_can_defend_here(game.unit_by_id(cargo).unwrap()));
    assert_eq!(
        game.best_defender_id(&game.make_unit(0, HUMAN, UnitKind::Warrior, x - 1, y), x, y,),
        Some(cargo)
    );

    let mut at_sea = Simulation::new(crate::test_support::seeded_map());
    at_sea.units.clear();
    at_sea.cities.clear();
    let (sx, sy) = (10, 24);
    at_sea.tile_mut(sx, sy).terrain = Terrain::Ocean;
    let carrier = at_sea.spawn_unit(1, UnitKind::Galleon, sx, sy);
    let cargo = at_sea.spawn_unit(1, UnitKind::Warrior, sx, sy);
    at_sea
        .units
        .iter_mut()
        .find(|unit| unit.id == cargo)
        .unwrap()
        .transport = Some(carrier);

    assert!(!at_sea.combat_unit_can_defend_here(at_sea.unit_by_id(cargo).unwrap()));
}

#[test]
fn tired_attack_scales_power_by_selected_ruleset_setting() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
    let mut unit = game.unit_by_id(id).unwrap().clone();
    unit.moves_left = MOVE_FRAGMENTS - 1;
    let expected = unit.kind.stats().attack * POWER_FACTOR * unit.moves_left / MOVE_FRAGMENTS;

    assert_eq!(Simulation::attack_power(&unit), expected);
    assert_eq!(
        Simulation::attack_power(&game.unit_by_id(id).unwrap().clone()),
        unit.kind.stats().attack * POWER_FACTOR
    );
}

#[test]
fn defender_reachability_uses_c2c3_target_classes_and_city_exception() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let ground = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
    let fighter = game.spawn_unit(1, UnitKind::Fighter, x + 1, y);
    let attacker = game.unit_by_id(ground).unwrap().clone();

    assert_eq!(game.best_defender_id(&attacker, x + 1, y), None);
    game.create_city(1, x + 1, y);
    assert_eq!(game.best_defender_id(&attacker, x + 1, y), Some(fighter));
}

#[test]
fn military_move_does_not_capture_non_capturable_worker() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (nx, ny) = game.map_wrap.offset(10, 24, 1, 0).unwrap();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
    let worker = game.spawn_unit(1, UnitKind::Worker, nx, ny);

    assert!(!game.apply(Command::CaptureUnits(attacker, nx, ny)));
    assert_eq!(game.try_move_unit(attacker, 1, 0), MoveResult::Fought);
    assert!(
        game.units
            .iter()
            .all(|unit| unit.kind != UnitKind::Worker || unit.owner != HUMAN)
    );
    assert!(game.unit_by_id(worker).is_none_or(|unit| unit.owner == 1));
}

#[test]
fn imported_marine_small_land_units_can_occupy_cities() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let home = (10, 24);
    let target = offset_native_by_map(home.0, home.1, 1, 0);
    game.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
    game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
    game.create_city(1, target.0, target.1);
    game.cities.last_mut().unwrap().size = 2;
    game.sign_pact(HUMAN, 1, Pact::War);
    let marine = game.spawn_unit(HUMAN, UnitKind::Marine, home.0, home.1);
    assert!(UnitKind::Marine.can_occupy_city());
    assert!(game.apply(Command::Move(marine, 1, 0)));
    assert_eq!(game.cities[0].owner, HUMAN);
}

#[test]
fn settlers_are_not_captured_by_military_move_under_c2c3() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (nx, ny) = game.map_wrap.offset(10, 24, 1, 0).unwrap();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    let attacker = game.spawn_unit(HUMAN, UnitKind::Swordsman, 10, 24);
    let settler = game.spawn_unit(1, UnitKind::Settler, nx, ny);
    game.sign_pact(HUMAN, 1, Pact::War);
    let actor = game.unit_by_id(attacker).unwrap().clone();
    let target = game.unit_by_id(settler).unwrap();
    assert!(
        !game
            .action_available_for_unit_against_unit(&actor, "Capture Units", target)
            .unwrap()
    );

    assert_eq!(game.try_move_unit(attacker, 1, 0), MoveResult::Fought);
    assert!(game.unit_by_id(settler).is_none());
    assert!(game.units.iter().all(|unit| unit.kind != UnitKind::Worker));
}

#[test]
fn worker_strength_keeps_actual_and_projected_foreign_worker_rates_aligned() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.enable_visual_events();
    game.units.clear();
    game.cities.clear();
    game.players[HUMAN].government = Government::Monarchy;
    game.players[HUMAN].known.clear();
    let native = game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
    let foreign = game.spawn_unit(HUMAN, UnitKind::Worker, 12, 24);
    game.units
        .iter_mut()
        .find(|unit| unit.id == foreign)
        .unwrap()
        .nationality = 1;
    game.tile_mut(10, 24).terrain = Terrain::Forest;
    game.tile_mut(12, 24).terrain = Terrain::Forest;

    assert!(game.begin_work(native, WorkKind::Cultivate));
    assert!(matches!(
        game.take_visual_events().front(),
        Some(VisualEvent::WorkStarted { unit, kind: WorkKind::Cultivate }) if unit.id == native
    ));
    assert!(game.begin_work(foreign, WorkKind::Cultivate));
    let native_unit = game.unit_by_id(native).unwrap().clone();
    let foreign_unit = game.unit_by_id(foreign).unwrap().clone();
    assert_eq!(game.activity_rate(&native_unit), 10);
    assert_eq!(game.activity_rate(&foreign_unit), 5);
    assert_eq!(game.project_turns_left(&native_unit), 10);
    assert_eq!(game.project_turns_left(&foreign_unit), 20);

    game.update_activities_and_restore();
    let native_unit = game.unit_by_id(native).unwrap().clone();
    let foreign_unit = game.unit_by_id(foreign).unwrap().clone();
    assert_eq!((native_unit.work_done, foreign_unit.work_done), (10, 5));
    assert_eq!(game.project_turns_left(&native_unit), 9);
    assert_eq!(game.project_turns_left(&foreign_unit), 19);

    // Freeciv activity rate is independent of Civ III government and
    // Replaceable Parts modifiers; the retained source WorkerStrength ratio
    // still applies to captive Workers.
    game.players[HUMAN].government = Government::Democracy;
    game.players[HUMAN].known.push(Tech::REPLACEABLE_PARTS);
    assert_eq!(game.activity_rate(&native_unit), 10);
    assert_eq!(game.activity_rate(&foreign_unit), 5);
}

#[test]
fn capture_eligibility_comes_from_selected_action_rules_not_civ3_technology() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.tile_mut(11, 24).terrain = Terrain::Grassland;
    game.players[HUMAN].known.clear();
    let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
    game.sign_pact(HUMAN, 1, Pact::War);
    game.spawn_unit(1, UnitKind::Artillery, 11, 24);

    assert!(!game.apply(Command::CaptureUnits(attacker_id, 11, 24)));

    game.players[HUMAN]
        .known
        .push(UnitKind::Artillery.required_tech().unwrap());
    assert!(!game.apply(Command::CaptureUnits(attacker_id, 11, 24)));
}

#[test]
fn capture_unique_preflight_rejects_conflicts_without_state_or_rng_changes() {
    for already_owned in [false, true] {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        let (x, y) = (11, 24);
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.spawn_unit(HUMAN, UnitKind::Marine, 10, 24);
        if already_owned {
            game.spawn_unit(HUMAN, UnitKind::Leader, 18, 24);
        }
        game.spawn_unit(1, UnitKind::Worker, x, y);
        game.spawn_unit(1, UnitKind::Leader, x, y);
        if !already_owned {
            game.spawn_unit(2, UnitKind::Leader, x, y);
        }

        let targets = game
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (x, y))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            crate::sim::action_rules::freeciv_unit_type_flag(UnitKind::Leader, "Unique"),
            Some(true)
        );
        let before_units = capture_unit_state(&game);
        let before_events = game.events.clone();
        let before_rng = game.random_snapshot();

        assert!(!game.capture_units_unique_preflight(HUMAN, &targets));
        assert_eq!(capture_unit_state(&game), before_units);
        assert_eq!(game.events, before_events);
        assert_eq!(game.random_snapshot(), before_rng);
    }
}

#[test]
fn capture_success_move_cost_uses_selected_effect_in_movement_fragments() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let actor_id = game.spawn_unit(HUMAN, UnitKind::Marine, 10, 24);
    game.units
        .iter_mut()
        .find(|unit| unit.id == actor_id)
        .unwrap()
        .moves_left = 18;
    let actor = game.unit_by_id(actor_id).unwrap().clone();

    let cost = game.spend_action_success_move_cost(&actor, "Capture Units");

    assert_eq!(
        cost, 6,
        "selected C2C3 Capture Units costs 6 move fragments"
    );
    assert_eq!(game.unit_by_id(actor_id).unwrap().moves_left, 12);
}

#[test]
fn capture_bounce_handles_city_tiles_and_unlisted_non_allied_units() {
    let mut city_case = Simulation::new(crate::test_support::seeded_map());
    city_case.units.clear();
    city_case.cities.clear();
    let (x, y) = (10, 24);
    for tile in &mut city_case.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
    }
    city_case.create_city(1, x, y);
    let captured = city_case.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    let before_rng = city_case.random_snapshot();

    city_case.bounce_captured_unit_if_needed(captured, &[]);

    let bounced = city_case.unit_by_id(captured).unwrap();
    assert_ne!((bounced.x, bounced.y), (x, y));
    assert_ne!(city_case.random_snapshot(), before_rng);

    let mut foreign_stack = Simulation::new(crate::mapgen::generate(43));
    foreign_stack.units.clear();
    foreign_stack.cities.clear();
    for tile in &mut foreign_stack.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
    }
    let captured = foreign_stack.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    let foreign = foreign_stack.spawn_unit(1, UnitKind::Warrior, x, y);
    let before_rng = foreign_stack.random_snapshot();

    foreign_stack.bounce_captured_unit_if_needed(captured, &[]);

    assert_ne!(
        (
            foreign_stack.unit_by_id(captured).unwrap().x,
            foreign_stack.unit_by_id(captured).unwrap().y
        ),
        (x, y)
    );
    assert!(foreign_stack.unit_by_id(foreign).is_some());
    assert_ne!(foreign_stack.random_snapshot(), before_rng);

    let mut listed_stack = Simulation::new(crate::mapgen::generate(44));
    listed_stack.units.clear();
    listed_stack.cities.clear();
    let captured = listed_stack.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    let foreign = listed_stack.spawn_unit(1, UnitKind::Warrior, x, y);
    let before_rng = listed_stack.random_snapshot();

    listed_stack.bounce_captured_unit_if_needed(captured, &[foreign]);

    assert_eq!(
        (
            listed_stack.unit_by_id(captured).unwrap().x,
            listed_stack.unit_by_id(captured).unwrap().y
        ),
        (x, y),
        "a later unit in the capture list does not force a bounce"
    );
    assert_eq!(listed_stack.random_snapshot(), before_rng);
}

#[test]
fn freeciv_bounce_uses_survivable_tiles_and_keeps_source_iteration_order() {
    let mut game = Simulation::new(crate::mapgen::generate(45));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
    }
    let (x, y) = (10, 24);
    let worker_id = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    let worker = game.unit_by_id(worker_id).unwrap().clone();
    assert!(game.unit_can_survive_at_tile(&worker, x + 1, y));

    let fighter_id = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
    let fighter = game.unit_by_id(fighter_id).unwrap().clone();
    assert!(!game.unit_can_survive_at_tile(&fighter, x + 1, y));
    game.tile_mut(x + 1, y).base = Some(Base::Airstrip);
    assert!(game.unit_can_survive_at_tile(&fighter, x + 1, y));
    game.tile_mut(x + 1, y).base = None;

    let helicopter_id = game.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
    let helicopter = game.unit_by_id(helicopter_id).unwrap().clone();
    assert!(!game.unit_can_survive_at_tile(&helicopter, x + 1, y));
    game.create_city(1, x + 1, y);
    assert!(game.unit_can_survive_at_tile(&helicopter, x + 1, y));

    let ordered = game.map_wrap.freeciv_square_radius(x, y, 2);
    assert_eq!(ordered.len(), 19);
    assert_eq!(ordered, game.map_wrap.freeciv_square_radius(x, y, 2));
}

#[test]
fn selected_roster_has_no_capturable_civ_iii_target_type() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let actor_id = game.spawn_unit(HUMAN, UnitKind::Marine, 10, 24);
    let target_id = game.spawn_unit(1, UnitKind::Worker, 11, 24);
    game.sign_pact(HUMAN, 1, Pact::War);
    let actor = game.unit_by_id(actor_id).unwrap().clone();
    let target = game.unit_by_id(target_id).unwrap().clone();

    assert_eq!(
        crate::sim::action_rules::freeciv_unit_type_flag(UnitKind::Worker, "Capturable"),
        Some(false),
        "Civ III Worker maps to C2C3 unit_worker, not the distinct unit_migrants"
    );
    assert!(
        !game
            .action_available_for_unit_against_unit(&actor, "Capture Units", &target)
            .unwrap()
    );
}

#[test]
fn enslave_result_indices_resolve_separately_from_capture_eligibility() {
    for (prototype, expected_id) in [(1, "worker"), (63, "man_o_war"), (65, "privateer")] {
        assert_eq!(
            crate::sim::civ3_unit_rules::get_by_prototype_index(prototype).map(|rule| rule.id),
            Some(expected_id)
        );
    }
    assert_eq!(
        UnitKind::JavelinThrower
            .conquests_rule()
            .enslave_result_prototype_index,
        1
    );
}

#[test]
fn combat_uses_the_selected_attack_action_and_skips_civ3_enslavement_rng() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (nx, ny) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.tile_mut(x, y).hut = false;
    game.tile_mut(nx, ny).hut = false;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::JavelinThrower, x, y);
    let defender = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    game.units
        .iter_mut()
        .find(|unit| unit.id == defender)
        .unwrap()
        .hp = 1;
    // Keep veteran checks from consuming random draws so this isolates combat
    // rounds and the mandatory occupation-chance roll.
    for unit in &mut game.units {
        unit.c2c3_veteran = u8::MAX as usize;
    }
    let attacker_unit = game.unit_by_id(attacker).unwrap().clone();
    assert_eq!(game.combat_action_for_unit(&attacker_unit), "Attack");
    assert_eq!(
        game.combat_action_for_unit(&game.make_unit(0, HUMAN, UnitKind::CruiseMissile, x, y)),
        "Suicide Attack"
    );
    let attack = game.attack_power_against(&attacker_unit, nx, ny);
    let defense = game.defense_power_against(
        game.unit_by_id(defender).unwrap(),
        nx,
        ny,
        UnitKind::JavelinThrower,
    );
    assert!(attack > 0 && defense > 0);
    let seed = (1..1000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.native_random = None;
            probe.rng = *seed;
            probe.rand_below(attack + defense) < attack
        })
        .expect("find a deterministic winning attack round");
    game.native_random = None;
    game.rng = seed;
    let mut expected_random = game.clone();
    expected_random.rand_below(attack + defense);
    expected_random.rand_below(100); // Occupy_Chance draw, even at 0%.

    game.resolve_combat(attacker, defender, nx, ny, None);

    assert!(game.unit_by_id(defender).is_none());
    assert!(
        game.units
            .iter()
            .all(|unit| { !(unit.kind == UnitKind::Worker && unit.owner == HUMAN) })
    );
    assert_eq!(
        game.random_snapshot(),
        expected_random.random_snapshot(),
        "Civ III Enslave data must not add a combat roll to the selected C2C3 mechanics"
    );
}

#[test]
fn combat_recalculates_movement_from_post_combat_hp() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (nx, ny) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Horseman, x, y);
    let defender = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    for unit in &mut game.units {
        unit.c2c3_veteran = u8::MAX as usize;
    }
    let rate_before = game.unit_move_rate(game.unit_by_id(attacker).unwrap());
    let moves_used = 1;
    game.units
        .iter_mut()
        .find(|unit| unit.id == attacker)
        .unwrap()
        .moves_left = rate_before - moves_used;
    let attacking_unit = game.unit_by_id(attacker).unwrap().clone();
    let defending_unit = game.unit_by_id(defender).unwrap().clone();
    let attack_power = game.attack_power_against(&attacking_unit, nx, ny);
    let defense_power = game.defense_power_against(&defending_unit, nx, ny, attacking_unit.kind);
    let (attack_firepower, defense_firepower) =
        game.combat_firepower(&attacking_unit, &defending_unit);

    let winning_seed = (1..10_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.native_random = None;
            probe.rng = *seed;
            let mut attacker_hp = attacking_unit.hp;
            let mut defender_hp = defending_unit.hp;
            while attacker_hp > 0 && defender_hp > 0 {
                if probe.rand_below(attack_power + defense_power) >= defense_power {
                    defender_hp -= attack_firepower;
                } else {
                    attacker_hp -= defense_firepower;
                }
            }
            attacker_hp > 0 && attacker_hp < attacking_unit.max_hit_points() && defender_hp <= 0
        })
        .expect("find a battle where the surviving attacker was damaged");
    game.native_random = None;
    game.rng = winning_seed;

    game.resolve_combat(attacker, defender, nx, ny, None);

    let attacker = game
        .unit_by_id(attacker)
        .expect("winning attacker survives");
    assert!(attacker.hp < attacker.max_hit_points());
    let expected_rate = game.unit_move_rate(attacker);
    assert_eq!(
        attacker.moves_left,
        (expected_rate - moves_used - MOVE_FRAGMENTS).max(0),
        "C2C3 DamageSlows movement is recalculated after the combat damage"
    );
}

#[test]
fn combat_recalculates_defender_movement_from_post_combat_hp() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (nx, ny) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    for (tx, ty) in [(x, y), (nx, ny)] {
        let tile = game.tile_mut(tx, ty);
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
        tile.hut = false;
    }
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker_id = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
    let defender_id = game.spawn_unit(1, UnitKind::Horseman, nx, ny);
    // Keep the units at maximum C2C3 veterancy so combat does not insert
    // promotion draws into the movement-focused scenario.
    for unit in &mut game.units {
        unit.c2c3_veteran = u8::MAX as usize;
    }
    let defender_rate_before = game.unit_move_rate(game.unit_by_id(defender_id).unwrap());
    let defender_moves_used = 1;
    game.units
        .iter_mut()
        .find(|unit| unit.id == defender_id)
        .unwrap()
        .moves_left = defender_rate_before - defender_moves_used;

    // Pick a real combat stream where the damage-slowed Horseman survives
    // after taking a hit; source do_attack recalculates both sides from the
    // post-combat HP, preserving their pre-combat moves_used.
    let seed = (1..10_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.native_random = None;
            probe.rng = *seed;
            probe.resolve_combat(attacker_id, defender_id, nx, ny, None);
            probe
                .unit_by_id(defender_id)
                .is_some_and(|defender| defender.hp > 0 && defender.hp < defender.max_hit_points())
        })
        .expect("find a battle where the damaged defender survives");
    game.native_random = None;
    game.rng = seed;

    game.resolve_combat(attacker_id, defender_id, nx, ny, None);

    let defender = game
        .unit_by_id(defender_id)
        .expect("defending Horseman survives");
    assert!(defender.hp < defender.max_hit_points());
    let defender_rate_after = game.unit_move_rate(defender);
    assert!(defender_rate_after < defender_rate_before);
    assert_eq!(
        defender.moves_left,
        (defender_rate_after - defender_moves_used).max(0),
        "Freeciv server/unithand.c::do_attack recomputes defender movement from post-combat HP"
    );
}

#[test]
fn next_phase_refills_use_live_mountain_movement_effects() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let tile = game.tile_mut(10, 24);
    tile.terrain = Terrain::Mountains;
    tile.road = false;
    tile.river = false;
    let id = game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24);
    game.units.last_mut().unwrap().moves_left = 0;
    game.units.last_mut().unwrap().moved = true;
    game.progress_player_unit_activities(HUMAN);
    let unit = game.unit_by_id(id).unwrap();
    assert_eq!(
        unit.moves_left,
        UnitKind::Chariot.stats().moves * MOVE_FRAGMENTS - MOVE_FRAGMENTS
    );
    assert!(
        unit.moved,
        "begin-phase MP restoration must not clear end-phase movement"
    );
}

#[test]
fn player_goto_continues_across_turns_until_destination() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
        tile.hut = false;
        tile.visible = true;
        tile.seen = true;
    }
    let start = (10, 24);
    let destination = game.map_wrap.offset(start.0, start.1, 4, 0).unwrap();
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
    game.remember_route_tiles(false);

    assert!(game.player_goto(id, destination.0, destination.1));
    let first_turn_position = {
        let unit = game.unit_by_id(id).unwrap();
        (unit.x, unit.y)
    };
    assert_ne!(first_turn_position, start);
    assert_ne!(first_turn_position, destination);
    assert_eq!(game.unit_goto_tiles.get(&id), Some(&destination));

    game.units
        .iter_mut()
        .find(|unit| unit.id == id)
        .unwrap()
        .moves_left = 0;
    game.progress_player_unit_activities(HUMAN);
    game.continue_player_gotos();

    let second_turn_position = {
        let unit = game.unit_by_id(id).unwrap();
        (unit.x, unit.y)
    };
    assert_ne!(second_turn_position, first_turn_position);
    assert_ne!(second_turn_position, destination);
    assert_eq!(game.unit_goto_tiles.get(&id), Some(&destination));

    for _ in 0..4 {
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .moves_left = 0;
        game.progress_player_unit_activities(HUMAN);
        game.continue_player_gotos();
        if (
            game.unit_by_id(id).unwrap().x,
            game.unit_by_id(id).unwrap().y,
        ) == destination
        {
            break;
        }
    }
    let unit = game.unit_by_id(id).unwrap();
    assert_eq!((unit.x, unit.y), destination);
    assert!(!game.unit_goto_tiles.contains_key(&id));
}

#[test]
fn end_phase_repairs_precede_city_builds_and_next_phase_activities() {
    let game = Simulation::new(crate::test_support::seeded_map());
    assert_eq!(
        game.unit_turn_lifecycle_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn native_movement_costs_nativity_boarding_and_zoc_flags() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert_eq!(game.native_movement_rules_playtest(), (true, true, true));
}

#[test]
fn moved_units_retain_the_matching_civ3_flic_direction_row() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
        tile.hut = false;
        tile.visible = true;
        tile.seen = true;
    }
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
    game.units.last_mut().unwrap().moves_left = 100;
    game.enable_visual_events();
    assert_eq!(game.try_move_unit(id, 0, -1), MoveResult::Moved);
    assert_eq!(game.unit_by_id(id).unwrap().facing, 4);
    let Some(VisualEvent::Move { unit, .. }) = game.take_visual_events().pop_front() else {
        panic!("movement publishes an animation event")
    };
    assert_eq!(unit.facing, 4);
}

#[test]
fn civ3_move_commands_cover_all_eight_neighbors_and_facing_rows() {
    let expected_rows = [5, 4, 3, 2, 1, 0, 7, 6];
    for (direction, &expected_row) in expected_rows.iter().enumerate() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
        }
        let start = (10, 20);
        let destination = game
            .map_wrap
            .civ3_neighbor(start.0, start.1, direction)
            .unwrap();
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);

        assert!(game.apply(Command::MoveCiv3(id, direction)));
        let moved = game.unit_by_id(id).unwrap();
        assert_eq!((moved.x, moved.y), destination);
        assert_eq!(moved.facing, expected_row);
    }
}

#[test]
fn civ3_move_command_rejects_an_invalid_direction_without_moving() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    assert!(!game.apply(Command::MoveCiv3(id, CIV3_DIRS.len())));
    assert_eq!(
        (
            game.unit_by_id(id).unwrap().x,
            game.unit_by_id(id).unwrap().y
        ),
        (10, 20)
    );
}

#[test]
fn big_land_has_no_terrain_defense_and_nuclear_is_not_air_attacker() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    let p = (10, 24);
    game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    game.tile_mut(p.0, p.1).base = None;
    for kind in [UnitKind::Catapult, UnitKind::Chariot, UnitKind::Cannon] {
        let unit = game.make_unit(0, HUMAN, kind, p.0, p.1);
        let plain = game.defense_power_against(&unit, p.0, p.1, UnitKind::Warrior);
        game.tile_mut(p.0, p.1).terrain = Terrain::Hills;
        assert_eq!(
            game.defense_power_against(&unit, p.0, p.1, UnitKind::Warrior),
            plain
        );
        game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    }
    let aegis = game.make_unit(0, HUMAN, UnitKind::AegisCruiser, p.0, p.1);
    let nuclear = game.defense_power_against(&aegis, p.0, p.1, UnitKind::TacticalNuke);
    assert_eq!(
        game.defense_power_against(&aegis, p.0, p.1, UnitKind::Bomber),
        nuclear * 5
    );
}

#[test]
fn mountain_movement_effect_and_river_defense_follow_native_classes() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    let p = (10, 24);
    let tile = game.tile_mut(p.0, p.1);
    tile.terrain = Terrain::Mountains;
    tile.road = false;
    tile.river = false;
    tile.base = None;
    let horse = game.make_unit(0, HUMAN, UnitKind::Horseman, p.0, p.1);
    assert_eq!(game.unit_move_rate(&horse), 6);
    game.tile_mut(p.0, p.1).river = true;
    assert_eq!(game.unit_move_rate(&horse), 12);
    let chariot = game.make_unit(0, HUMAN, UnitKind::Chariot, p.0, p.1);
    assert_eq!(game.unit_move_rate(&chariot), 6);
    game.tile_mut(p.0, p.1).road = true;
    assert_eq!(game.unit_move_rate(&chariot), 12);
    game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    game.tile_mut(p.0, p.1).river = false;
    let legion = game.make_unit(0, HUMAN, UnitKind::Swordsman, p.0, p.1);
    let plain = game.defense_power_against(&legion, p.0, p.1, UnitKind::Warrior);
    game.tile_mut(p.0, p.1).river = true;
    assert_eq!(
        game.defense_power_against(&legion, p.0, p.1, UnitKind::Warrior),
        plain + plain / 4
    );
    assert_eq!(
        game.defense_power_against(&chariot, p.0, p.1, UnitKind::Warrior),
        chariot.kind.stats().defense * POWER_FACTOR
    );
}

#[test]
fn combat_terrain_defense_uses_selected_c2c3_terrain_and_class_rules() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    let (x, y) = (10, 24);
    {
        let tile = game.tile_mut(x, y);
        tile.base = None;
        tile.river = false;
        tile.terrain = Terrain::Marsh;
    }

    let warrior = game.make_unit(0, HUMAN, UnitKind::Warrior, x, y);
    let base = warrior.kind.stats().defense * POWER_FACTOR;
    // Civ III Marsh's 20% is not reused: C2C3 Swamp is 25%, yielding
    // floor(base * 125 / 100) in common/combat.c::get_defense_power.
    assert_eq!(
        game.defense_power_against(&warrior, x, y, UnitKind::Warrior),
        base * 125 / 100
    );

    let big_land = game.make_unit(0, HUMAN, UnitKind::Chariot, x, y);
    let base = big_land.kind.stats().defense * POWER_FACTOR;
    assert_eq!(
        game.defense_power_against(&big_land, x, y, UnitKind::Warrior),
        base,
        "C2C3 Big Land has no TerrainDefense flag"
    );

    game.tile_mut(x, y).terrain = Terrain::Volcano;
    let base = warrior.kind.stats().defense * POWER_FACTOR;
    assert_eq!(
        game.defense_power_against(&warrior, x, y, UnitKind::Warrior),
        base * 200 / 100,
        "Volcano combat defense uses the explicit C2C3 Mountains adapter"
    );
}

#[test]
fn combat_defense_counts_upgraded_base_underlays_from_selected_effects() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    let (x, y) = (10, 24);
    {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.base = None;
    }
    let defender = game.make_unit(0, HUMAN, UnitKind::Warrior, x, y);
    let base_power = defender.kind.stats().defense * POWER_FACTOR;

    game.tile_mut(x, y).base = Some(Base::Fort);
    assert_eq!(
        game.defense_power_against(&defender, x, y, UnitKind::Warrior),
        base_power * 150 / 100
    );
    game.tile_mut(x, y).base = Some(Base::Fortress);
    assert_eq!(
        game.defense_power_against(&defender, x, y, UnitKind::Warrior),
        base_power * 200 / 100,
        "Fortress retains C2C3's underlying Fort defense effect"
    );

    game.tile_mut(x, y).base = Some(Base::Airbase);
    assert_eq!(
        game.defense_power_against(&defender, x, y, UnitKind::Bomber),
        base_power * 200 / 100,
        "Airbase retains both Airstrip and Airbase defense against aerial units"
    );
}

#[test]
fn destroyer_defense_multiplier_uses_the_selected_submarine_flag() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    let (x, y) = (10, 24);
    game.tile_mut(x, y).terrain = Terrain::Ocean;
    game.tile_mut(x, y).river = false;
    game.tile_mut(x, y).base = None;
    let destroyer = game.make_unit(0, HUMAN, UnitKind::Destroyer, x, y);
    let base = destroyer.kind.stats().defense * POWER_FACTOR;
    assert_eq!(
        game.defense_power_against(&destroyer, x, y, UnitKind::Submarine),
        base * 2
    );
    assert_eq!(
        game.defense_power_against(&destroyer, x, y, UnitKind::NuclearSubmarine),
        base,
        "the Nuclear Submarine transport role alias does not inherit unit-type flags"
    );
    assert_eq!(
        game.defense_power_against(&destroyer, x, y, UnitKind::Carrier),
        base
    );
}

#[test]
fn combat_occupancy_uses_selected_chance_and_consumes_its_rng_draw() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).hut = false;
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
    let defender = game.spawn_unit(1, UnitKind::Worker, tx, ty);
    game.units
        .iter_mut()
        .find(|unit| unit.id == attacker)
        .unwrap()
        .veteran = 3; // No legacy promotion roll after the source-combat draw.
    game.units
        .iter_mut()
        .find(|unit| unit.id == defender)
        .unwrap()
        .hp = 0; // Skip the combat rounds; isolate the post-win occupy roll.
    game.native_random = None;
    game.rng = 91;

    let mut expected = game.clone();
    expected.rand_below(100); // pinned unithand.c occupancy chance roll
    let expected_rng = expected.rng;
    game.resolve_combat(attacker, defender, tx, ty, None);

    assert_eq!(
        (
            game.unit_by_id(attacker).unwrap().x,
            game.unit_by_id(attacker).unwrap().y
        ),
        (tx, ty)
    );
    assert_eq!(game.rng, expected_rng);
}

#[test]
fn combat_firepower_uses_selected_c2c3_unit_flags() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (x, y) = (10, 24);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.create_city(1, x, y);

    let cruise = game.make_unit(0, HUMAN, UnitKind::CruiseMissile, x, y);
    let warrior = game.make_unit(1, 1, UnitKind::Warrior, x, y);
    assert_eq!(
        game.combat_firepower(&cruise, &warrior),
        (2, 1),
        "C2C3 CityBuster doubles the Cruise Missile's attack firepower in a city"
    );

    let bomber = game.make_unit(2, HUMAN, UnitKind::Bomber, x, y);
    let caravel = game.make_unit(3, 1, UnitKind::Caravel, x, y);
    assert_eq!(
        game.combat_firepower(&bomber, &caravel),
        (2, 1),
        "C2C3 BadCityDefender doubles city attack firepower and applies its configured defender cap"
    );
}

#[test]
fn civilian_peace_borders_follow_ruleset_flags_and_still_exclude_foreign_stacks() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.civilian_border_playtest());
}

#[test]
fn movement_rates_scale_damage_before_effects_and_use_whole_move_class_minima() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.damaged_movement_playtest());
    game.players[HUMAN]
        .known
        .retain(|&tech| tech != Tech::NUCLEAR_POWER);
    let mut horse = game.make_unit(0, HUMAN, UnitKind::Horseman, 10, 24);
    let horse_minimum = MOVE_FRAGMENTS.min(horse.kind.stats().moves * MOVE_FRAGMENTS);
    let horse_base = horse.kind.stats().moves * MOVE_FRAGMENTS;
    let horse_bonus = game.unit_move_rate(&horse) - horse_base;
    horse.hp = 5;
    assert_eq!(
        game.unit_move_rate(&horse),
        (horse_base * horse.hp / horse.max_hit_points() + horse_bonus).max(horse_minimum)
    );
    horse.hp = 1;
    assert_eq!(
        game.unit_move_rate(&horse),
        (horse_base * horse.hp / horse.max_hit_points() + horse_bonus).max(horse_minimum)
    );
    let mut ship = game.make_unit(0, HUMAN, UnitKind::Destroyer, 10, 24);
    let ship_base = ship.kind.stats().moves * MOVE_FRAGMENTS;
    let ship_bonus = game.unit_move_rate(&ship) - ship_base;
    let ship_minimum = (2 * MOVE_FRAGMENTS).min(ship_base);
    ship.hp = 1;
    assert_eq!(
        game.unit_move_rate(&ship),
        (ship_base * ship.hp / ship.max_hit_points() + ship_bonus).max(ship_minimum)
    );
    ship.hp = ship.max_hit_points() * 2 / 3;
    assert_eq!(
        game.unit_move_rate(&ship),
        (ship_base * ship.hp / ship.max_hit_points() + ship_bonus).max(ship_minimum)
    );
    let before_nuclear = game.unit_move_rate(&ship);
    game.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
    assert_eq!(game.unit_move_rate(&ship), before_nuclear + MOVE_FRAGMENTS);
    for kind in [UnitKind::Worker, UnitKind::Bomber] {
        let mut unit = game.make_unit(0, HUMAN, kind, 10, 24);
        let healthy = game.unit_move_rate(&unit);
        unit.hp = 1;
        assert_eq!(game.unit_move_rate(&unit), healthy);
    }
}

#[test]
fn wounded_ground_and_sea_units_restore_the_same_rates_used_by_pathfinding() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Sea;
    game.tile_mut(12, 24).terrain = Terrain::Grassland;
    let ship = game.spawn_unit(HUMAN, UnitKind::Destroyer, 10, 24);
    let horse = game.spawn_unit(HUMAN, UnitKind::Horseman, 12, 24);
    for unit in &mut game.units {
        unit.hp = 1;
        unit.moves_left = 0;
        unit.moved = true;
    }
    game.update_activities_and_restore();
    for (id, _) in [(ship, UnitKind::Destroyer), (horse, UnitKind::Horseman)] {
        let unit = game.unit_by_id(id).unwrap();
        assert!(unit.moves_left > 0);
        assert_eq!(unit.moves_left, game.unit_move_rate(unit));
    }
}

#[test]
fn newly_created_triremes_get_lighthouse_speed_in_their_actual_location() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    game.create_city(HUMAN, 10, 24);
    game.cities[0].buildings.push(Building::LIGHTHOUSE);
    game.tile_mut(12, 24).terrain = Terrain::Sea;
    game.tile_mut(12, 24).river = false;
    let id = game.spawn_unit(HUMAN, UnitKind::Galley, 12, 24);
    let unit = game.unit_by_id(id).unwrap();
    assert_eq!(unit.moves_left, 24);
    assert_eq!(unit.moves_left, game.unit_move_rate(unit));
}

#[test]
fn workers_pool_matching_progress_only_when_they_have_movement() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 10, 20);
    let (first, second) = game.cooperative_work_playtest_setup();
    for id in [first, second] {
        assert!(game.apply(Command::Work(id, WorkKind::Irrigation)));
    }
    game.units
        .iter_mut()
        .find(|unit| unit.id == first)
        .unwrap()
        .moves_left = 0;
    game.update_activities_and_restore();
    let idle = game.unit_by_id(first).unwrap();
    assert_eq!(idle.work_done, 0);
    assert_eq!(game.project_turns_left(idle), 2);
    game.update_activities_and_restore();
    assert!(!game.tile(10, 20).irrigation);
    game.update_activities_and_restore();
    assert!(game.tile(10, 20).irrigation);
    assert!(
        game.units
            .iter()
            .filter(|unit| [first, second].contains(&unit.id))
            .all(|unit| unit.work_kind.is_none() && unit.work_done == 0)
    );
}

#[test]
fn pathfinding_routes_around_control_and_friendly_stacks_allow_entry() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
    }
    let (warrior, _, (x, y)) = game.zone_control_playtest_setup();
    let direct = game.path_to(warrior, x, y);
    assert!(
        direct.len() > 1,
        "pathfinding should route around enemy control"
    );
    let start = game.unit_by_id(warrior).unwrap();
    assert_ne!(
        game.map_wrap
            .civ3_offset(start.x, start.y, direct[0].0, direct[0].1),
        Some((x, y))
    );
    game.set_random_seed(42);
    let rng_before_zoc_attack = game.random_snapshot();
    let hp_before_zoc_attack = game.unit_by_id(warrior).unwrap().hp;
    assert!(!game.apply(Command::Move(warrior, 1, 0)));
    assert_eq!(game.random_snapshot(), rng_before_zoc_attack);
    assert_eq!(game.unit_by_id(warrior).unwrap().hp, hp_before_zoc_attack);
    assert_eq!(
        (
            game.unit_by_id(warrior).unwrap().x,
            game.unit_by_id(warrior).unwrap().y
        ),
        (10, 20)
    );
    game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    let friendly_mover = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    let path = game.path_to(friendly_mover, x, y);
    assert_eq!(path.len(), 1);
    let start = game.unit_by_id(friendly_mover).unwrap();
    assert_eq!(
        game.map_wrap
            .civ3_offset(start.x, start.y, path[0].0, path[0].1),
        Some((x, y))
    );
    assert!(game.apply(Command::Move(friendly_mover, 1, 0)));
}

#[test]
fn freeciv_zoc_blocks_a_step_between_two_enemy_controlled_tiles() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    prepare_zoc_test_map(&mut game);
    let (start, destination, enemy) = blocked_zoc_positions(&game);
    let mover = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
    game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
    let unit = game.unit_by_id(mover).unwrap().clone();

    assert!(game.zone_blocks_step(&unit, start, destination));
    assert!(!game.apply(Command::MoveCiv3(mover, 2)));
    assert_eq!(
        (
            game.unit_by_id(mover).unwrap().x,
            game.unit_by_id(mover).unwrap().y
        ),
        start
    );
}

#[test]
fn freeciv_zoc_allows_one_step_into_enemy_control_from_a_safe_tile() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    prepare_zoc_test_map(&mut game);
    let start = (10, 20);
    let destination = game.map_wrap.civ3_neighbor(start.0, start.1, 2).unwrap();
    let enemy = game
        .map_wrap
        .civ3_neighbor(destination.0, destination.1, 2)
        .unwrap();
    assert_eq!(
        game.map_wrap
            .civ3_distance(start.0, start.1, enemy.0, enemy.1),
        2
    );
    assert_eq!(
        game.map_wrap
            .civ3_distance(destination.0, destination.1, enemy.0, enemy.1),
        1
    );
    let mover = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
    game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
    let unit = game.unit_by_id(mover).unwrap().clone();

    assert!(!game.zone_blocks_step(&unit, start, destination));
    assert!(game.apply(Command::MoveCiv3(mover, 2)));
    assert_eq!(
        (
            game.unit_by_id(mover).unwrap().x,
            game.unit_by_id(mover).unwrap().y
        ),
        destination
    );
}

#[test]
fn zoc_legality_does_not_preempt_the_existing_attack_action_path() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    prepare_zoc_test_map(&mut game);
    let start = (10, 20);
    let destination = game.map_wrap.civ3_neighbor(start.0, start.1, 2).unwrap();
    let attacker = game.spawn_unit(HUMAN, UnitKind::ModernArmor, start.0, start.1);
    game.spawn_unit(1, UnitKind::Warrior, destination.0, destination.1);
    let unit = game.unit_by_id(attacker).unwrap().clone();
    game.sign_pact(HUMAN, 1, Pact::War);

    assert!(game.zone_blocks_step(&unit, start, destination));
    game.set_random_seed(42);
    let random_before_attack = game.random_snapshot();
    assert!(game.apply(Command::MoveCiv3(attacker, 2)));
    assert_ne!(game.random_snapshot(), random_before_attack);
}

#[test]
fn freeciv_zoc_allied_stack_city_nozoc_terrain_and_igzoc_exemptions_apply() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    prepare_zoc_test_map(&mut game);
    let (start, destination, enemy) = blocked_zoc_positions(&game);
    let mover = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
    game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
    let allied_stack_mover = game.unit_by_id(mover).unwrap().clone();

    game.spawn_unit(HUMAN, UnitKind::Worker, destination.0, destination.1);
    assert!(!game.zone_blocks_step(&allied_stack_mover, start, destination));

    game.units.retain(|unit| unit.kind != UnitKind::Worker);
    game.create_city(HUMAN, start.0, start.1);
    assert!(!game.zone_blocks_step(&allied_stack_mover, start, destination));
    game.cities.clear();
    game.create_city(HUMAN, destination.0, destination.1);
    assert!(!game.zone_blocks_step(&allied_stack_mover, start, destination));

    game.cities.clear();
    game.tile_mut(destination.0, destination.1).terrain = Terrain::Sea;
    assert!(!game.zone_blocks_step(&allied_stack_mover, start, destination));
    game.tile_mut(destination.0, destination.1).terrain = Terrain::Grassland;

    let explorer_id = game.spawn_unit(HUMAN, UnitKind::Explorer, start.0, start.1);
    let explorer = game.unit_by_id(explorer_id).unwrap().clone();
    assert!(!game.zone_blocks_step(&explorer, start, destination));

    let carrier_id = game.spawn_unit(HUMAN, UnitKind::Carrier, start.0, start.1);
    let carrier = game.unit_by_id(carrier_id).unwrap().clone();
    assert!(!game.zone_blocks_step(&carrier, start, destination));
}

#[test]
fn freeciv_route_zoc_uses_only_observed_enemy_units() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    prepare_zoc_test_map(&mut game);
    let (start, destination, enemy) = blocked_zoc_positions(&game);
    let mover_id = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
    let enemy_id = game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
    game.remember_route_tiles(false);
    game.tile_mut(enemy.0, enemy.1).visible = false;
    let mover = game.unit_by_id(mover_id).unwrap().clone();

    assert!(game.zone_blocks_route(&mover, start, destination, false));
    assert!(!game.zone_blocks_route(&mover, start, destination, true));
    game.tile_mut(enemy.0, enemy.1).visible = true;
    assert!(game.zone_blocks_route(&mover, start, destination, true));

    game.units.retain(|unit| unit.id != enemy_id);
    game.create_city(1, enemy.0, enemy.1);
    game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
    game.remember_route_tiles(false);
    assert!(game.zone_blocks_route(&mover, start, destination, true));
    game.units
        .retain(|unit| unit.id != enemy_id && (unit.x, unit.y) != enemy);
    assert!(!game.zone_blocks_route(&mover, start, destination, true));
    game.tile_mut(enemy.0, enemy.1).visible = false;
    assert!(game.zone_blocks_route(&mover, start, destination, true));
}

#[test]
fn a_winning_attacker_cannot_occupy_a_city_with_a_second_defender() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (12, 12);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.create_city(1, nx, ny);
    game.cities.last_mut().unwrap().size = 2;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::ModernArmor, x, y);
    let first = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    let second = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    game.units
        .iter_mut()
        .find(|unit| unit.id == first)
        .unwrap()
        .hp = 1;
    game.set_random_seed(42);
    game.resolve_combat(attacker, first, nx, ny, None);
    assert!(!game.units.iter().any(|unit| unit.id == first));
    assert!(game.units.iter().any(|unit| unit.id == second));
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == attacker && (unit.x, unit.y) == (x, y))
    );
    assert!(
        game.cities
            .iter()
            .any(|city| (city.x, city.y, city.owner) == (nx, ny, 1))
    );
    game.units
        .iter_mut()
        .find(|unit| unit.id == attacker)
        .unwrap()
        .moves_left = MOVE_FRAGMENTS;
    game.units
        .iter_mut()
        .find(|unit| unit.id == second)
        .unwrap()
        .hp = 1;
    assert!(game.apply(Command::Move(attacker, 1, 0)));
    assert!(!game.units.iter().any(|unit| unit.id == second));
    assert!(
        game.unit_by_id(attacker)
            .is_some_and(|unit| (unit.x, unit.y) == (nx, ny))
    );
    assert!(
        game.cities
            .iter()
            .any(|city| (city.x, city.y, city.owner) == (nx, ny, HUMAN))
    );
}

#[test]
fn defeating_a_stack_defender_kills_all_reachable_hostile_units() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (12, 12);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    let attacker = game.spawn_unit(HUMAN, UnitKind::ModernArmor, x, y);
    let first = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    let second = game.spawn_unit(1, UnitKind::Worker, nx, ny);
    game.units
        .iter_mut()
        .find(|unit| unit.id == first)
        .unwrap()
        .hp = 1;
    game.set_random_seed(42);
    game.resolve_combat(attacker, first, nx, ny, None);
    assert!(!game.units.iter().any(|unit| unit.id == first));
    assert!(!game.units.iter().any(|unit| unit.id == second));
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == attacker && (unit.x, unit.y) == (nx, ny))
    );
}

#[test]
fn stack_losses_leave_units_the_winner_cannot_reach() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (12, 12);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    let attacker = game.spawn_unit(HUMAN, UnitKind::ModernArmor, x, y);
    let ground_defender = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    let unreachable_aircraft = game.spawn_unit(1, UnitKind::Fighter, nx, ny);
    game.units
        .iter_mut()
        .find(|unit| unit.id == ground_defender)
        .unwrap()
        .hp = 1;
    game.set_random_seed(42);

    game.resolve_combat(attacker, ground_defender, nx, ny, None);

    assert!(game.unit_by_id(ground_defender).is_none());
    assert!(game.unit_by_id(unreachable_aircraft).is_some());
    assert!(
        game.unit_by_id(attacker)
            .is_some_and(|unit| (unit.x, unit.y) == (x, y))
    );
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn unload_all_deboards_every_eligible_cargo_unit() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        let carrier = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
        let cargo = [
            game.spawn_unit(HUMAN, UnitKind::Warrior, x, y),
            game.spawn_unit(HUMAN, UnitKind::Worker, x, y),
        ];
        for id in cargo {
            game.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .transport = Some(carrier);
        }

        assert!(game.apply(Command::UnloadAll(carrier)));
        assert_eq!(game.cargo_count(carrier), 0);
        assert!(
            cargo
                .iter()
                .all(|id| game.unit_by_id(*id).unwrap().transport.is_none())
        );
    }

    #[test]
    fn rescue_prefers_idle_carriers_then_uses_future_speed_to_break_ties() {
        for fast_ship_working in [false, true] {
            let mut game = Simulation::new(crate::test_support::seeded_map());
            game.units.clear();
            let (x, y) = (10, 20);
            game.tile_mut(x, y).terrain = Terrain::Sea;
            let lost = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
            let slow = game.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
            let fast = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
            let passenger = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            assert!(game.board_specific_ship(passenger, lost));
            for ship in game
                .units
                .iter_mut()
                .filter(|unit| [slow, fast].contains(&unit.id))
            {
                ship.moves_left = MOVE_FRAGMENTS;
                if ship.id == fast && fast_ship_working {
                    ship.work_kind = Some(WorkKind::Clean(Contamination::Fallout));
                    ship.work_left = 1;
                }
            }

            game.remove_units_with_cargo(vec![lost]);

            let expected = if fast_ship_working { slow } else { fast };
            assert_eq!(
                game.units
                    .iter()
                    .find(|unit| unit.id == passenger)
                    .unwrap()
                    .transport,
                Some(expected)
            );
        }
    }

    #[test]
    fn combustion_transport_carries_its_imported_capacity_of_heavy_units() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Sea;
        game.create_city(HUMAN, x, y);
        let oil = offset_native_by_map(x, y, -1, 0);
        let oil_tile = game.tile_mut(oil.0, oil.1);
        oil_tile.terrain = Terrain::Desert;
        oil_tile.resource = Some(Resource::Oil);
        oil_tile.owner = Some(HUMAN);
        oil_tile.road = true;
        let id = game.cities[0].id;
        let caravel = Production::Unit(UnitKind::Caravel);
        let transport = Production::Unit(UnitKind::Transport);
        game.players[HUMAN].known.push(Tech::ASTRONOMY);
        assert!(game.apply(Command::Produce(id, caravel)));
        assert!(!game.apply(Command::Produce(id, transport)));
        game.players[HUMAN].known.push(Tech::COMBUSTION);
        game.players[HUMAN].known.push(Tech::REFINING);
        assert!(!game.apply(Command::Produce(id, caravel)));
        game.cities[0].shields = transport.cost();
        game.process_cities();
        let ship = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Transport)
            .unwrap()
            .id;
        assert!(game.apply(Command::Move(ship, 1, 0)));
        let transport_capacity = game
            .ruleset_transport_capacity(UnitKind::Transport)
            .unwrap();
        for _ in 0..transport_capacity {
            let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
            assert!(game.apply(Command::Move(cannon, 1, 0)));
        }
        let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        assert!(!game.apply(Command::Move(cannon, 1, 0)));
        assert_eq!(game.cargo_count(ship), transport_capacity);
    }

    #[test]
    fn ports_build_transports_only_with_their_tech_and_upgrade_obsolete_production() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (x, y) = (24, 15);
        for (tx, ty) in game.map_wrap.civ3_neighbors(x, y) {
            game.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, x, y);
        let id = game.cities.last().unwrap().id;
        let caravel = Production::Unit(UnitKind::Caravel);
        let galleon = Production::Unit(UnitKind::Galleon);
        game.players[HUMAN].known.push(Tech::ASTRONOMY);
        assert!(!game.apply(Command::Produce(id, caravel)));
        let (sx, sy) = game.map_wrap.civ3_neighbors(x, y).next().unwrap();
        game.tile_mut(sx, sy).terrain = Terrain::Lake;
        assert!(game.apply(Command::Produce(id, caravel)));
        assert!(!game.apply(Command::Produce(id, galleon)));
        game.players[HUMAN].known.push(Tech::MAGNETISM);
        assert!(!game.apply(Command::Produce(id, caravel)));
        game.cities.last_mut().unwrap().shields = galleon.cost();
        game.process_cities();
        assert!(
            game.units
                .iter()
                .any(|unit| { unit.kind == UnitKind::Galleon && unit.home_city == Some(id) })
        );
    }

    #[test]
    fn adjacent_embark_respects_cargo_class_and_capacity() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Sea;
        let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, sx, sy);
        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        assert!(game.apply(Command::Move(warrior, 1, 0)));
        assert_eq!(game.cargo_count(ship), 1);
        for _ in 0..2 {
            let cargo = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            assert!(game.apply(Command::Move(cargo, 1, 0)));
            assert!(!game.apply(Command::BoardOrDeboard(cargo)));
        }
        assert_eq!(game.cargo_count(ship), 3);
        let cargo = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        assert!(!game.apply(Command::Move(cargo, 1, 0)));
    }
}
