//! Regression tests for the behavior of the parent module.

use super::*;

/// Mirrors Freeciv behavior.
#[test]
fn expired_ceasefire_retains_zero_countdown_and_suppresses_offers_as_in_reference() {
    let mut game = war_planning_game();
    game.players[HUMAN].ai = true;
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::Ceasefire;
    relation.until = game.turn;
    relation.trust = [10_000; 2];
    game.begin_ai_war_plan(1, HUMAN, 1, WarReason::Hatred);
    game.advance_diplomacy();
    assert!(game.at_war(1, HUMAN));
    game.advance_ai_war_plans_for(1);
    assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
    game.advance_ai_war_plans_for(1);
    // daidiplomacy.c only consumes zero by declaring war when not already at
    // war; its offer loop then excludes this nonnegative countdown.
    assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
    game.advance_diplomatic_offers_for(1);
    assert!(game.at_war(1, HUMAN));

    // Establish that the stored countdown is what prevents a new offer.
    game.ai_war_plans.remove(&(1, HUMAN));
    game.ai_diplomatic_memory.get_mut(&(1, HUMAN)).unwrap().spam = 0;
    game.advance_diplomatic_offers_for(1);
    assert_eq!(game.pact(1, HUMAN), Pact::Ceasefire);
}

/// Mirrors Freeciv behavior.
#[test]
fn recent_contact_can_cease_fire_on_the_declaration_turn_as_in_reference() {
    let mut game = war_planning_game();
    game.players[HUMAN].ai = true;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().first_contact = Some(game.turn - 10);
    game.trace = trace::Collector::enabled(["*".to_owned()].into(), 1000);
    game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
    assert!(game.at_war(1, HUMAN));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -20);
    // daidiplomacy.c's TURNS_BEFORE_TARGET grace values ceasefire at zero;
    // negative stubbornness excludes Peace/Alliance, not Ceasefire.
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Ceasefire), 0);
    game.ai_diplomatic_memory.get_mut(&(HUMAN, 1)).unwrap().spam = 10;
    game.ai_diplomatic_memory.get_mut(&(1, HUMAN)).unwrap().spam = 0;
    game.advance_diplomatic_offers();
    assert_eq!(game.pact(HUMAN, 1), Pact::Ceasefire);
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    let events = game.trace.drain();
    let war = events
        .iter()
        .find(|e| e.payload["action"] == "war_declared")
        .unwrap();
    let ceasefire = events
        .iter()
        .find(|e| e.payload["action"] == "offer_accepted")
        .unwrap();
    assert_eq!(war.turn, ceasefire.turn);
    assert!(war.sequence < ceasefire.sequence);
}

#[test]
fn fresh_casus_belli_starts_the_short_revenge_countdown() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = -1;
    game.record_provocation(1, HUMAN);
    game.advance_ai_war_plans();
    let plan = game.ai_war_plans[&(1, HUMAN)];
    assert!(matches!(plan.reason, WarReason::Behavior));
    assert_eq!(plan.countdown, game.map_size_factor() - 1);
    assert!(game.ai_on_war_footing(1));
    for _ in 0..game.map_size_factor() {
        game.advance_ai_war_plans();
    }
    assert!(game.at_war(1, HUMAN));
}

#[test]
fn launched_space_leader_triggers_a_two_turn_war_plan() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
    game.spaceship_fixture();
    assert!(game.launch_spaceship(HUMAN));
    game.advance_ai_war_plans();
    let plan = game.ai_war_plans[&(1, HUMAN)];
    assert!(matches!(plan.reason, WarReason::Space));
    assert_eq!(plan.countdown, 1);
    assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
    game.advance_ai_war_plans();
    assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
    game.advance_ai_war_plans();
    assert!(game.at_war(HUMAN, 1));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -10);
}

#[test]
fn hatred_prepares_war_and_stubbornness_expires_separately() {
    let mut game = war_planning_game();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by.set(1, true);
    }
    game.spawn_unit(1, UnitKind::Spearman, 12, 24);
    assert!(!game.ai_on_war_footing(1));
    game.advance_ai_war_plans();
    let delay = 4 + game.map_size_factor();
    assert_eq!(game.ai_war_countdown(1, HUMAN), delay - 1);
    assert!(game.ai_on_war_footing(1));
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    for _ in 0..delay {
        game.advance_ai_war_plans();
    }
    assert!(!game.at_war(1, HUMAN));
    assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
    let objective = game.coordinated_war_objective(1).unwrap();
    let path = crate::sim::save::tests::temporary_save("war-objective");
    game.save_campaign(&path).unwrap();
    game = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(game.coordinated_war_objective(1), Some(objective));
    game.players[1].gold = 1000;
    for _ in 0..8 {
        game.spawn_unit(
            1,
            UnitKind::Swordsman,
            objective.staging.0,
            objective.staging.1,
        );
    }
    assert!(game.war_force_ready(1, objective));
    assert!(game.strategic_priorities(1).can_start_optional_war());
    game.advance_ai_war_plans();
    assert!(game.at_war(1, HUMAN));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -20);
    assert!(!game.ai_on_war_footing(1));
    assert!(game.pact_clause_value(1, HUMAN, Pact::Ceasefire) < 0);
    for _ in 0..19 {
        game.advance_ai_war_plans();
    }
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
    assert!(game.pact_refusal(1, HUMAN, Pact::Ceasefire).is_none());
}

#[test]
fn an_unmet_faction_is_not_an_allys_enemy_or_an_obligation_to_declare_war() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 3000;
    game.sign_pact(1, 2, Pact::Alliance);
    assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
    game.refresh_diplomatic_intelligence();
    game.advance_ai_war_plans();
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    game.diplomacy.entry((HUMAN, 2)).or_default().first_contact = Some(1);
    game.refresh_diplomatic_intelligence();
    game.advance_ai_war_plans();
    assert!(matches!(
        game.ai_war_plans[&(1, HUMAN)].reason,
        WarReason::Alliance
    ));
    game.diplomacy.remove(&(HUMAN, 2));
    game.ai_war_plans.get_mut(&(1, HUMAN)).unwrap().countdown = 0;
    game.refresh_diplomatic_intelligence();
    game.advance_ai_war_plans();
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
}

#[test]
fn war_against_an_allys_enemy_is_aborted_when_the_obligation_ends() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 3000;
    game.sign_pact(1, 2, Pact::Alliance);
    game.diplomacy.entry((HUMAN, 2)).or_default().first_contact = Some(1);
    game.refresh_diplomatic_intelligence();
    game.advance_ai_war_plans();
    assert!(matches!(
        game.ai_war_plans[&(1, HUMAN)].reason,
        WarReason::Alliance
    ));
    assert!(game.ai_on_war_footing(1));
    game.sign_pact(HUMAN, 2, Pact::Peace);
    game.ai_war_plans.get_mut(&(1, HUMAN)).unwrap().countdown = 0;
    game.refresh_diplomatic_intelligence();
    game.advance_ai_war_plans();
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
}

#[test]
fn war_preparation_and_hostile_alliances_block_peacetime_love() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
    game.begin_ai_war_plan(1, HUMAN, 3, WarReason::Hatred);
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 960);
    game.ai_war_plans.clear();
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
    game.refresh_diplomatic_intelligence();
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 920);
}

#[test]
fn aggression_threshold_matches_reference_clipping_and_love_rounding() {
    assert_eq!(aggression_war_threshold(50), -2290);
    assert_eq!(aggression_war_threshold(0), aggression_war_threshold(1));
    assert_eq!(aggression_war_threshold(2501), 2000);
    assert!(aggression_war_threshold(30) < aggression_war_threshold(90));
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = -2291;
    game.advance_ai_war_plans();
    assert!(game.ai_on_war_footing(1));
}

#[test]
fn spaceship_construction_warns_before_launch_without_breaking_alliance() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_space_strategy_test());
}

#[test]
fn space_race_leader_keeps_its_own_victory_strategy() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
    // AI 1 is the first player to start a spaceship (human has none).
    game.spaceships[1].add_part(0);
    game.spaceships[2].add_part(0);
    game.sign_pact(1, 2, Pact::Peace);
    game.diplomacy.get_mut(&(1, 2)).unwrap().first_contact = Some(game.turn);
    game.advance_ai_war_plans();
    assert!(!game.ai_space_warnings.contains_key(&(1, 2)));
    assert_eq!(game.ai_war_countdown(1, 2), -1);
    assert!(game.ai_space_warnings.contains_key(&(2, 1)));
}

#[test]
fn spaceship_warnings_repeat_only_after_the_reference_cooldown() {
    let mut game = war_planning_game();
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.first_contact = Some(game.turn);
    relation.trust[1] = 9000;
    game.spaceships[HUMAN].add_part(0);
    game.advance_ai_war_plans();
    game.advance_diplomatic_offers();
    let cooldown = game.ai_space_warnings[&(1, HUMAN)];
    assert!((9..=14).contains(&cooldown));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 8000);
    for _ in 0..cooldown {
        game.advance_ai_war_plans();
        game.advance_diplomatic_offers();
    }
    assert_eq!(game.ai_space_warnings[&(1, HUMAN)], 0);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 8000);
    game.advance_ai_war_plans();
    game.advance_diplomatic_offers();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 7000);
}

#[test]
fn senate_in_anarchy_aborts_revolution_and_war_tantrum_uses_native_love() {
    let mut game = war_planning_game();
    game.players[1].government = Government::DEMOCRACY;
    game.players[1].known.push(Tech::NUCLEAR_FISSION);
    game.cities[0].buildings.push(Building::UNITED_NATIONS);
    game.completed_wonders.push(Building::UNITED_NATIONS);
    game.completed_wonders.push(Building::MANHATTAN_PROJECT);
    game.apollo_completed = false;
    game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
    assert_eq!(game.players[1].government, Government::DEMOCRACY);
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    assert!(!game.ai_pending_war_targets.contains_key(&1));
    assert_eq!(game.pact(1, HUMAN), Pact::Peace);
    game.cities[0].buildings.clear();
    game.players[1].government = Government::DESPOTISM;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1234;
    game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
    assert!(game.at_war(1, HUMAN));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -1260);
}

#[test]
fn senate_requires_a_revolution_before_the_planned_declaration() {
    let mut game = war_planning_game();
    game.players[1].government = Government::DEMOCRACY;
    game.ai_war_plans.insert(
        (1, HUMAN),
        WarPlan {
            countdown: 0,
            reason: WarReason::Behavior,
            objective: None,
            readiness_waited: 0,
        },
    );
    game.advance_ai_war_plans();
    assert_eq!(game.players[1].government, Government::ANARCHY);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert!(!game.ai_on_war_footing(1));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -20);
    assert_eq!(game.ai_pending_war_targets.get(&1), Some(&HUMAN));
    assert_eq!(game.players[1].revolution_target, Some(Government::ANARCHY));
    game.advance_ai_war_plans();
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert!(game.change_government(1, Government::DESPOTISM));
    assert!(game.at_war(HUMAN, 1));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -19);
    assert!(!game.ai_pending_war_targets.contains_key(&1));
}

#[test]
fn economic_crisis_blocks_optional_wars_but_preserves_ally_obligations() {
    let mut healthy = war_planning_game();
    healthy.players[1].gold = 5000;
    healthy.advance_ai_war_plans_for(1);
    assert!(healthy.ai_war_countdown(1, HUMAN) >= 0);

    let mut crisis = war_planning_game();
    let city = crisis
        .cities
        .iter_mut()
        .find(|city| city.owner == 1)
        .unwrap();
    city.buildings
        .extend(std::iter::repeat_n(Building::LIBRARY, 20));
    crisis.players[1].gold = 0;
    assert!(crisis.strategic_priorities(1).economic_crisis);
    crisis.advance_ai_war_plans_for(1);
    assert_eq!(crisis.ai_war_countdown(1, HUMAN), -1);

    crisis.make_diplomatic_contact(1, 2);
    crisis.sign_pact(1, 2, Pact::Alliance);
    crisis.make_diplomatic_contact(HUMAN, 2);
    crisis.sign_pact(HUMAN, 2, Pact::War);
    crisis.refresh_diplomatic_intelligence();
    crisis.advance_ai_war_plans_for(1);
    assert!(matches!(
        crisis.ai_war_plans[&(1, HUMAN)].reason,
        WarReason::Alliance
    ));
}

#[test]
fn distant_same_continent_objective_stages_beyond_eight_tiles() {
    let mut game = war_planning_game();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = true;
        tile.known_by.set(1, true);
    }
    let home = (6, 24);
    let target = (20, 24);
    assert!(game.map_distance(home.0, home.1, target.0, target.1) > 10);
    game.create_city(1, home.0, home.1);
    game.create_city(HUMAN, target.0, target.1);
    game.spawn_unit(1, UnitKind::Spearman, home.0, home.1);
    let swordsman = game.spawn_unit(1, UnitKind::Swordsman, home.0, home.1);
    game.players[1].gold = 1000;

    game.begin_ai_war_plan(1, HUMAN, 1, WarReason::Hatred);
    game.advance_ai_war_plans_for(1);
    let objective = game
        .coordinated_war_objective(1)
        .expect("distant same-continent target should produce a Land objective");
    assert_eq!(objective.kind, WarObjectiveKind::Land);

    let actor = game.unit_by_id(swordsman).unwrap().clone();
    assert!(game.run_coordinated_ground_order(&actor, objective));
    let moved = game.unit_by_id(swordsman).unwrap();
    assert_ne!((moved.x, moved.y), home);

    for _ in 0..8 {
        game.spawn_unit(
            1,
            UnitKind::Swordsman,
            objective.staging.0,
            objective.staging.1,
        );
    }
    assert!(game.war_force_ready(1, objective));
    game.advance_ai_war_plans_for(1);
    assert!(game.at_war(1, HUMAN));
}

#[test]
fn overseas_naval_objective_builds_transports_stages_and_declares_when_ready() {
    let mut game = war_planning_game();
    game.cities.clear();
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Coast;
        tile.river = false;
        tile.road = false;
        tile.owner = None;
        tile.known_by.set(1, true);
    }
    let home = (10, 20);
    let target = (18, 20);
    for (cx, cy) in [home, target] {
        game.tile_mut(cx, cy).terrain = Terrain::Grassland;
        let (dx, dy) = EIGHT_WAY_DIRS[0];
        let (nx, ny) = game.map_wrap.eight_way_offset(cx, cy, dx, dy).unwrap();
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    }

    game.create_city(1, home.0, home.1);
    let home_city = game.city_at(home.0, home.1).unwrap().id;
    game.create_city(HUMAN, target.0, target.1);
    game.players[1]
        .known
        .extend([Tech::MAP_MAKING, Tech::IRON_WORKING, Tech::BRONZE_WORKING]);
    game.players[1].gold = 1000;
    game.spawn_unit(1, UnitKind::Spearman, home.0, home.1);

    game.begin_ai_war_plan(1, HUMAN, 1, WarReason::Hatred);
    game.advance_ai_war_plans_for(1);
    let objective = game
        .coordinated_war_objective(1)
        .expect("cross-continent coastal target should produce a Naval objective");
    assert_eq!(objective.kind, WarObjectiveKind::Naval);
    assert!(!game.war_force_ready(1, objective));

    // Save/load round-trip preserves Naval WarObjectiveKind.
    let path = crate::sim::save::tests::temporary_save("naval-war-objective");
    game.save_campaign(&path).unwrap();
    game = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(game.coordinated_war_objective(1), Some(objective));

    // Without a transport yet, coordinated_muster builds a Galley at the muster port.
    let city = game.city_by_id(home_city).unwrap().clone();
    let choice = game.ai_build_choice(&city).unwrap();
    assert_eq!(choice.production, Production::Unit(UnitKind::Galley));

    // With transports loaded and staged at objective.staging, readiness passes and war declares.
    for _ in 0..4 {
        let boat = game.spawn_unit(
            1,
            UnitKind::Galley,
            objective.staging.0,
            objective.staging.1,
        );
        for _ in 0..2 {
            let troop = game.spawn_unit(
                1,
                UnitKind::Swordsman,
                objective.staging.0,
                objective.staging.1,
            );
            assert!(game.board_specific_ship(troop, boat));
        }
    }
    assert!(game.war_force_ready(1, objective));
    game.advance_ai_war_plans_for(1);
    assert!(game.at_war(1, HUMAN));
}

#[test]
fn alliance_and_space_wars_stage_during_countdown_without_blocking_declaration() {
    for reason in [WarReason::Alliance, WarReason::Space] {
        let mut game = war_planning_game();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by.set(1, true);
        }
        game.spawn_unit(1, UnitKind::Spearman, 12, 24);
        if matches!(reason, WarReason::Alliance) {
            game.sign_pact(1, 2, Pact::Alliance);
            game.diplomacy.entry((HUMAN, 2)).or_default().first_contact = Some(1);
            game.sign_pact(HUMAN, 2, Pact::War);
            game.refresh_diplomatic_intelligence();
        }
        game.begin_ai_war_plan(1, HUMAN, 1, reason);
        game.advance_ai_war_plans_for(1);
        assert!(!game.at_war(1, HUMAN));
        assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
        let objective = game
            .coordinated_war_objective(1)
            .expect("obligated war should pick a staging objective during countdown");
        assert!(!game.war_force_ready(1, objective));

        // Countdown 0 declares immediately without waiting for war_force_ready.
        game.advance_ai_war_plans_for(1);
        assert!(game.at_war(1, HUMAN));
    }
}

#[test]
fn behavior_war_waits_for_readiness_up_to_timeout_then_declares() {
    let mut game = war_planning_game();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by.set(1, true);
    }
    game.spawn_unit(1, UnitKind::Spearman, 12, 24);
    game.begin_ai_war_plan(1, HUMAN, 0, WarReason::Behavior);

    for expected_wait in 1..=BEHAVIOR_READINESS_TIMEOUT {
        game.advance_ai_war_plans_for(1);
        assert!(!game.at_war(1, HUMAN));
        assert_eq!(
            game.ai_war_plans[&(1, HUMAN)].readiness_waited,
            expected_wait
        );
        assert!(game.coordinated_war_objective(1).is_some());
    }
    // Once timeout is reached, revenge war declares anyway.
    game.advance_ai_war_plans_for(1);
    assert!(game.at_war(1, HUMAN));

    // If force becomes ready before the timeout, Behavior declares immediately.
    let mut ready = war_planning_game();
    for tile in &mut ready.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by.set(1, true);
    }
    ready.spawn_unit(1, UnitKind::Spearman, 12, 24);
    ready.begin_ai_war_plan(1, HUMAN, 0, WarReason::Behavior);
    ready.advance_ai_war_plans_for(1);
    assert!(!ready.at_war(1, HUMAN));
    let objective = ready.coordinated_war_objective(1).unwrap();
    for _ in 0..8 {
        ready.spawn_unit(
            1,
            UnitKind::Swordsman,
            objective.staging.0,
            objective.staging.1,
        );
    }
    assert!(ready.war_force_ready(1, objective));
    ready.advance_ai_war_plans_for(1);
    assert!(ready.at_war(1, HUMAN));
}

#[test]
fn overseas_naval_transport_boards_stages_and_lands_after_war_declaration() {
    let mut game = war_planning_game();
    game.cities.clear();
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Coast;
        tile.river = false;
        tile.road = false;
        tile.owner = None;
        tile.known_by.set(1, true);
    }
    let home = (10, 20);
    let target = (18, 20);
    for (cx, cy) in [home, target] {
        game.tile_mut(cx, cy).terrain = Terrain::Grassland;
        let (dx, dy) = EIGHT_WAY_DIRS[0];
        let (nx, ny) = game.map_wrap.eight_way_offset(cx, cy, dx, dy).unwrap();
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    }

    game.create_city(1, home.0, home.1);
    game.create_city(HUMAN, target.0, target.1);
    game.players[1]
        .known
        .extend([Tech::MAP_MAKING, Tech::IRON_WORKING, Tech::BRONZE_WORKING]);
    let guard = game.spawn_unit(1, UnitKind::Spearman, home.0, home.1);
    let boat = game.spawn_unit(1, UnitKind::Galley, home.0, home.1);
    let t1 = game.spawn_unit(1, UnitKind::Swordsman, home.0, home.1);
    let t2 = game.spawn_unit(1, UnitKind::Swordsman, home.0, home.1);

    game.begin_ai_war_plan(1, HUMAN, 2, WarReason::Hatred);
    game.advance_ai_war_plans_for(1);
    let objective = game.coordinated_war_objective(1).unwrap();
    assert_eq!(objective.kind, WarObjectiveKind::Naval);

    // Run transport preparation turns until it boards troops and reaches objective.staging.
    for _ in 0..8 {
        if let Some(ship) = game.unit_by_id(boat).cloned() {
            assert!(game.run_coordinated_naval_transport(&ship, objective, Some(guard)));
        }
        for unit in &mut game.units {
            unit.moves_left = unit.kind.stats().moves * 3;
        }
    }
    assert_eq!(game.unit_by_id(t1).unwrap().transport, Some(boat));
    assert_eq!(game.unit_by_id(t2).unwrap().transport, Some(boat));
    assert_eq!(game.unit_by_id(guard).unwrap().transport, None);
    let staged_ship = game.unit_by_id(boat).unwrap();
    assert_eq!((staged_ship.x, staged_ship.y), objective.staging);

    // Once war is declared, the staged transport sails to a beachhead and disembarks onto the target continent.
    game.sign_pact(1, HUMAN, Pact::War);
    let continents = game.continent_labels();
    let target_continent = continents[game.map_wrap.index(target.0, target.1)];
    for _ in 0..6 {
        if let Some(ship) = game.unit_by_id(boat).cloned() {
            game.run_coordinated_naval_transport(&ship, objective, Some(guard));
        }
        for unit in &mut game.units {
            unit.moves_left = unit.kind.stats().moves * 3;
        }
    }
    let landed = [t1, t2].into_iter().all(|id| {
        let u = game.unit_by_id(id).unwrap();
        u.transport.is_none() && continents[game.map_wrap.index(u.x, u.y)] == target_continent
    });
    assert!(
        landed,
        "staged troops should disembark onto target continent after war starts"
    );
}

#[test]
fn combined_arms_bombard_units_build_stage_and_bombard_objective() {
    let mut game = war_planning_game();
    game.cities.clear();
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.road = true;
        tile.known_by.set(1, true);
    }
    let home = (10, 24);
    let target = (16, 24);
    game.tile_mut(target.0, target.1).terrain = Terrain::Hills;
    game.create_city(1, home.0, home.1);
    let home_city = game.city_at(home.0, home.1).unwrap().id;
    game.create_city(HUMAN, target.0, target.1);
    game.players[1]
        .known
        .extend([Tech::MATHEMATICS, Tech::IRON_WORKING, Tech::BRONZE_WORKING]);
    game.players[1].gold = 1000;
    game.spawn_unit(1, UnitKind::Spearman, home.0, home.1);
    game.spawn_unit(HUMAN, UnitKind::Spearman, target.0, target.1);
    let adj = game
        .map_wrap
        .eight_way_offset(target.0, target.1, EIGHT_WAY_DIRS[0].0, EIGHT_WAY_DIRS[0].1)
        .unwrap();
    game.spawn_unit(HUMAN, UnitKind::Pikeman, adj.0, adj.1);

    game.begin_ai_war_plan(1, HUMAN, 1, WarReason::Hatred);
    game.advance_ai_war_plans_for(1);
    let objective = game.coordinated_war_objective(1).unwrap();
    assert_eq!(objective.kind, WarObjectiveKind::Land);

    // Stage 2 offensive troops at objective.staging; coordinated_muster should now request a Catapult.
    for _ in 0..2 {
        game.spawn_unit(
            1,
            UnitKind::Swordsman,
            objective.staging.0,
            objective.staging.1,
        );
    }
    let city = game.city_by_id(home_city).unwrap().clone();
    let choice = game.ai_build_choice(&city).unwrap();
    assert_eq!(choice.production, Production::Unit(UnitKind::Catapult));

    // A Catapult at home marches to objective.staging via run_coordinated_ground_order.
    let catapult = game.spawn_unit(1, UnitKind::Catapult, home.0, home.1);
    let actor = game.unit_by_id(catapult).unwrap().clone();
    assert!(game.run_coordinated_ground_order(&actor, objective));
    let moved = game.unit_by_id(catapult).unwrap();
    assert_ne!((moved.x, moved.y), home);

    // Place the Catapult at staging, declare war, and verify it approaches & bombards the target city.
    game.units.iter_mut().find(|u| u.id == catapult).unwrap().x = objective.staging.0;
    game.units.iter_mut().find(|u| u.id == catapult).unwrap().y = objective.staging.1;
    assert_eq!(game.staged_bombard_units(1, objective).len(), 1);
    game.sign_pact(1, HUMAN, Pact::War);
    for _ in 0..3 {
        let unit = game.unit_by_id(catapult).unwrap().clone();
        if game.run_ai_bombard(&unit) {
            return;
        }
        game.run_ai_bombard_approach(&unit);
        for u in &mut game.units {
            u.moves_left = u.kind.stats().moves * 3;
        }
    }
    let unit = game.unit_by_id(catapult).unwrap().clone();
    assert!(
        game.run_ai_bombard(&unit),
        "staged Catapult should reach and bombard the objective city"
    );
}
