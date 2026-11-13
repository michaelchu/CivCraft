//! Regression tests for the behavior of the parent module.

use super::*;

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
    game.spaceship_playtest_setup();
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
    assert!(!game.ai_on_war_footing(1));
    game.advance_ai_war_plans();
    let delay = 4 + game.map_size_factor();
    assert_eq!(game.ai_war_countdown(1, HUMAN), delay - 1);
    assert!(game.ai_on_war_footing(1));
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    for _ in 0..delay {
        game.advance_ai_war_plans();
    }
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_space_strategy_playtest());
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
    game.players[1].government = Government::Democracy;
    game.players[1].known.push(Tech::NUCLEAR_FISSION);
    game.cities[0].buildings.push(Building::UNITED_NATIONS);
    game.completed_wonders.push(Building::UNITED_NATIONS);
    game.completed_wonders.push(Building::MANHATTAN_PROJECT);
    game.apollo_completed = false;
    game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
    assert_eq!(game.players[1].government, Government::Democracy);
    assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
    assert!(!game.ai_pending_war_targets.contains_key(&1));
    assert_eq!(game.pact(1, HUMAN), Pact::Peace);
    game.cities[0].buildings.clear();
    game.players[1].government = Government::Despotism;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1234;
    game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
    assert!(game.at_war(1, HUMAN));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -1260);
}

#[test]
fn senate_requires_a_revolution_before_the_planned_declaration() {
    let mut game = war_planning_game();
    game.players[1].government = Government::Democracy;
    game.ai_war_plans.insert(
        (1, HUMAN),
        WarPlan {
            countdown: 0,
            reason: WarReason::Hatred,
        },
    );
    game.advance_ai_war_plans();
    assert_eq!(game.players[1].government, Government::Anarchy);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert!(!game.ai_on_war_footing(1));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -20);
    assert_eq!(game.ai_pending_war_targets.get(&1), Some(&HUMAN));
    assert_eq!(game.players[1].revolution_target, Some(Government::Anarchy));
    game.advance_ai_war_plans();
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert!(game.change_government(1, Government::Despotism));
    assert!(game.at_war(HUMAN, 1));
    assert_eq!(game.ai_war_countdown(1, HUMAN), -19);
    assert!(!game.ai_pending_war_targets.contains_key(&1));
}
