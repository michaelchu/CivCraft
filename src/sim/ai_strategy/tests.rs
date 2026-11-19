use super::*;

#[test]
fn assessments_are_read_only_and_missing_identity_has_neutral_preferences() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].civilization_id = None;
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    game.ai_war_plans.clear();
    game.players[1].gold = 5000;
    let before = game.clone();
    let assessment = game.strategic_priorities(1);
    assert_eq!(assessment, game.strategic_priorities(1));
    assert_eq!(
        (assessment.economy, assessment.science, assessment.military),
        (100, 100, 100)
    );
    assert_eq!(game.players[1].gold, before.players[1].gold);
    assert_eq!(game.players[1].known, before.players[1].known);
    assert_eq!(
        game.players[1].research_goal,
        before.players[1].research_goal
    );
}

#[test]
fn active_space_program_prioritizes_science_without_blocking_defense() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::scenarios::strategy_pair_fixture(&mut game);
    let before = game.strategic_priorities(1);
    game.spaceships[1].built[0] = 1;
    let space = game.strategic_priorities(1);
    assert!(space.space_program);
    assert!(space.science > before.science);
    assert!(!space.can_start_optional_war());
    game.sign_pact(1, 2, Pact::War);
    assert_eq!(game.strategic_priorities(1).posture, Posture::Defend);
}

#[test]
fn human_private_state_does_not_change_public_technology_estimates() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::scenarios::strategy_pair_fixture(&mut game);
    game.players[1].ai = false;
    let before = game.diplomatic_technology_value(1, 2, Tech::POTTERY, false);
    game.players[1].research_goal = Tech::POTTERY;
    game.players[1].researching = Tech::POTTERY;
    game.players[1].research = 100;
    game.players[1].gold = 0;
    game.spaceships[1].built[0] = 1;
    assert_eq!(
        before,
        game.diplomatic_technology_value(1, 2, Tech::POTTERY, false)
    );
}

#[test]
fn unmet_targets_and_defeated_enemies_do_not_create_military_pressure() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::scenarios::strategy_pair_fixture(&mut game);
    game.diplomacy.remove(&(1, 2));
    game.ai_war_plans.insert(
        (1, 2),
        diplomacy::WarPlan {
            countdown: 3,
            reason: diplomacy::WarReason::Hatred,
            objective: None,
        },
    );
    let unmet = game.strategic_priorities(1);
    assert_eq!(unmet.wars, 0);
    assert!(!unmet.preparing);
    game.make_diplomatic_contact(1, 2);
    assert!(game.strategic_priorities(1).preparing);
    game.sign_pact(1, 2, Pact::War);
    game.cities.retain(|city| city.owner != 2);
    let defeated = game.strategic_priorities(1);
    assert_eq!(defeated.wars, 0);
    assert!(!defeated.preparing);
}
