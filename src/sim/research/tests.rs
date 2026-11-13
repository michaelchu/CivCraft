//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn gated_resource_becomes_visible_after_required_technology_is_discovered() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.players[HUMAN].known.clear();

    assert!(!sim.resource_visible_to(HUMAN, Resource::Oil));

    sim.discover(HUMAN, Tech::REFINING);

    assert!(sim.resource_visible_to(HUMAN, Resource::Oil));
}

#[test]
fn unset_goal_is_distinct_and_clears_on_gift_and_negative_advisor_values() {
    let sim = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        sim.native_unset_research_goal_playtest(),
        (true, true, true)
    );
}

#[test]
fn human_gift_clears_goal_without_allowing_unset_as_active_research() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.players[HUMAN].known.clear();
    sim.players[HUMAN].research_goal = Tech::POTTERY;
    sim.players[HUMAN].researching = Tech::BRONZE_WORKING;
    sim.players[HUMAN].research = 9;
    sim.discover(HUMAN, Tech::POTTERY);
    assert_eq!(sim.players[HUMAN].research_goal, Tech::UNSET);
    assert_eq!(sim.players[HUMAN].researching, Tech::BRONZE_WORKING);
    assert_eq!(sim.players[HUMAN].research, 9);
    assert!(!sim.apply(Command::Research(Tech::UNSET)));
    assert_eq!(sim.players[HUMAN].research_goal, Tech::UNSET);
    assert!(sim.apply(Command::Research(Tech::ALPHABET)));
    assert_eq!(sim.players[HUMAN].research_goal, Tech::ALPHABET);
}

#[test]
fn actual_human_science_completion_clears_goal_and_retains_valid_automatic_target() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.players[HUMAN].known.clear();
    sim.players[HUMAN].research_goal = Tech::POTTERY;
    sim.players[HUMAN].researching = Tech::POTTERY;
    sim.players[HUMAN].research = 0;
    let cost = sim.target_cost(HUMAN, Tech::POTTERY);
    sim.resolve_research_turn(HUMAN, cost);
    assert!(sim.has_tech(HUMAN, Tech::POTTERY));
    assert_eq!(sim.players[HUMAN].research_goal, Tech::UNSET);
    assert_ne!(sim.players[HUMAN].researching, Tech::UNSET);
    assert!(sim.tech_available(HUMAN, sim.players[HUMAN].researching));
}

#[test]
fn goal_less_research_matches_freeciv_random_frontier_and_draw_order() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].research_goal = Tech::UNSET;
    game.players[HUMAN].researching = Tech::UNSET;

    let mut expected = game.clone();
    let mut expected_choice = Tech::FUTURE;
    let mut count = 0;
    for tech in Tech::all() {
        if expected.tech_available(HUMAN, tech) {
            count += 1;
            if expected.rand_below(count) == 0 {
                expected_choice = tech;
            }
        }
    }

    let actual = game.choose_research_target(HUMAN);

    assert_eq!(actual, expected_choice);
    assert!(game.tech_available(HUMAN, actual));
    assert_eq!(game.random_snapshot(), expected.random_snapshot());
}

#[test]
fn campaign_starts_with_no_hard_coded_goal_and_a_ruleset_progression_target() {
    let game = Simulation::new(crate::mapgen::generate(42));
    for owner in 0..game.civilization_count {
        assert_eq!(game.players[owner].research_goal, Tech::UNSET);
        assert!(game.tech_available(owner, game.players[owner].researching));
    }
}

#[test]
fn future_research_retains_its_project_but_clears_its_advisor_goal() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.players[1].known = Tech::all().collect();
    sim.players[1].researching = Tech::FUTURE;
    sim.players[1].research_goal = Tech::FUTURE;
    assert_eq!(sim.select_ai_research(1), (Tech::FUTURE, 1, 1));
    assert_eq!(sim.players[1].research_goal, Tech::UNSET);
    sim.discover(1, Tech::FUTURE);
    assert_eq!(sim.players[1].researching, Tech::FUTURE);
    assert_eq!(sim.players[1].research_goal, Tech::UNSET);
    assert_eq!(sim.players[1].future_techs, 1);
}

#[test]
fn native_research_integer_and_float_compound_vectors_match_c() {
    let sim = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        sim.native_research_arithmetic_playtest(),
        (true, true, true)
    );
    let paths = vec![
        vec![],
        vec![Tech(1)],
        vec![Tech(1), Tech(2)],
        vec![Tech(1), Tech(3)],
    ];
    assert_eq!(
        Simulation::source_research_scores(&[1.75, -1.5, 7.75, -3.125], &paths),
        (vec![0, 0, 7, -3], vec![-1, 0, 3, -1])
    );
    // Native choice.want is assigned an INTEGER per-city quotient.
    let (values, _) = Simulation::source_research_scores(
        &[-4.75, 2.5, -7.75, 3.125],
        &[
            vec![Tech(0)],
            vec![Tech(1)],
            vec![Tech(0), Tech(1), Tech(2)],
            vec![Tech(1), Tech(3)],
        ],
    );
    assert_eq!(
        values.iter().map(|value| value / 2).collect::<Vec<_>>(),
        vec![-3, 0, -3, 1]
    );
}

#[test]
fn player_research_keeps_the_selected_goal_after_an_advance() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known.clear();
    game.players[HUMAN].research_goal = Tech::GUNPOWDER;
    game.players[HUMAN].researching = game.research_target(HUMAN);
    let first = game.players[HUMAN].researching;
    game.discover(HUMAN, first);
    let expected = game.research_target(HUMAN);
    assert_eq!(game.next_ai_research_target(HUMAN), expected);
    assert_eq!(game.players[HUMAN].research_goal, Tech::GUNPOWDER);
}

#[test]
fn switching_respects_investment_and_exempts_discovery_overflow() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_research_switching_playtest());
}

#[test]
fn c2c3_tech_switch_suspends_one_project_and_restores_its_bulbs() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known.clear();
    game.players[HUMAN].researching = Tech::ALPHABET;
    game.players[HUMAN].research = 40;

    game.choose_research_tech(HUMAN, Tech::POTTERY, 0);
    assert_eq!(game.players[HUMAN].researching, Tech::POTTERY);
    assert_eq!(game.players[HUMAN].researching_saved, Some(Tech::ALPHABET));
    assert_eq!(game.players[HUMAN].research_saved, 40);
    assert_eq!(game.players[HUMAN].research, 0);

    game.choose_research_tech(HUMAN, Tech::ALPHABET, 0);
    assert_eq!(game.players[HUMAN].researching, Tech::ALPHABET);
    assert_eq!(game.players[HUMAN].researching_saved, None);
    assert_eq!(game.players[HUMAN].research_saved, 0);
    assert_eq!(game.players[HUMAN].research, 40);
}

#[test]
fn suspended_research_accumulates_city_science_before_technology_upkeep() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    game.players[HUMAN].known = Tech::all().collect();
    game.players[HUMAN].researching = Tech::FUTURE;
    game.players[HUMAN].researching_saved = Some(Tech::ALPHABET);
    game.players[HUMAN].research_saved = 40;
    game.players[HUMAN].research = 100;
    let upkeep = game.technology_upkeep(HUMAN);

    game.apply_research_upkeep(HUMAN, 20);

    assert_eq!(game.players[HUMAN].research_saved, 60);
    assert_eq!(game.players[HUMAN].research, 120 - upkeep);
    assert_eq!(game.players[HUMAN].researching_saved, Some(Tech::ALPHABET));
}

#[test]
fn losing_a_technology_does_not_retarget_the_active_project() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known = vec![Tech::ALPHABET];
    game.players[HUMAN].research_goal = Tech::UNSET;
    game.players[HUMAN].researching = Tech::BRONZE_WORKING;
    game.players[HUMAN].research = 0;

    game.apply_research_upkeep(HUMAN, -1);

    assert!(!game.has_tech(HUMAN, Tech::ALPHABET));
    assert_eq!(game.players[HUMAN].researching, Tech::BRONZE_WORKING);
}

#[test]
fn switching_keeps_free_discovery_bulbs_out_of_the_penalty() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known.clear();
    game.players[HUMAN].researching = Tech::ALPHABET;
    game.players[HUMAN].research = 17;

    game.choose_research_tech(HUMAN, Tech::POTTERY, 12);
    assert_eq!(game.players[HUMAN].researching_saved, Some(Tech::ALPHABET));
    assert_eq!(game.players[HUMAN].research_saved, 17);
    assert_eq!(game.players[HUMAN].research, 12);
}

#[test]
fn later_era_research_waits_for_critical_advances_in_prior_eras() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known.clear();

    assert_eq!(Tech::FEUDALISM.era(), crate::rules::TechEra::MiddleAges);
    assert_eq!(game.research_era(HUMAN), crate::rules::TechEra::Ancient);
    assert!(Tech::FEUDALISM.civ3_prerequisites().unwrap().is_empty());
    assert!(!game.tech_available(HUMAN, Tech::FEUDALISM));
    let path = game.research_path(HUMAN, Tech::FEUDALISM);
    assert!(path.contains(&Tech::FEUDALISM));
    assert!(
        Tech::all()
            .filter(
                |tech| tech.era() == crate::rules::TechEra::Ancient && tech.required_for_civ3_era()
            )
            .all(|tech| path.contains(&tech))
    );

    let modern_path = game.research_path(HUMAN, Tech::COMPUTERS);
    assert!(Tech::all()
        .filter(|tech| tech.era() < crate::rules::TechEra::Modern
            && tech.required_for_civ3_era())
        .all(|tech| modern_path.contains(&tech)));
}

#[test]
fn research_cannot_be_switched_to_a_later_era_technology_early() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known.clear();
    game.players[HUMAN].researching = Tech::ALPHABET;
    game.players[HUMAN].research = 37;

    assert!(!game.tech_available(HUMAN, Tech::FEUDALISM));
    game.choose_research_tech(HUMAN, Tech::FEUDALISM, 0);

    assert_eq!(game.players[HUMAN].researching, Tech::ALPHABET);
    assert_eq!(game.players[HUMAN].research, 37);
    assert_eq!(game.players[HUMAN].researching_saved, None);
}

#[test]
fn era_advances_only_after_every_critical_advance_of_each_age_is_known() {
    use crate::rules::TechEra;

    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known.clear();

    for (era, next_era) in [
        (TechEra::Ancient, TechEra::MiddleAges),
        (TechEra::MiddleAges, TechEra::Industrial),
        (TechEra::Industrial, TechEra::Modern),
    ] {
        let required: Vec<_> = Tech::all()
            .filter(|tech| tech.era() == era && tech.required_for_civ3_era())
            .collect();
        assert!(!required.is_empty(), "expected critical advances for {era:?}");

        for &tech in &required[..required.len() - 1] {
            game.discover(HUMAN, tech);
            assert_eq!(game.research_era(HUMAN), era);
        }
        game.discover(HUMAN, *required.last().unwrap());
        assert!(Tech::all()
            .filter(|tech| tech.era() == era && tech.optional_in_civ3())
            .all(|tech| !game.has_tech(HUMAN, tech)));
        assert_eq!(game.research_era(HUMAN), next_era);
    }
}

#[test]
fn research_selection_rejects_a_technology_with_unknown_biq_prerequisites() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known.clear();
    game.players[1].research = 0;
    game.players[1].researching = Tech::ALPHABET;
    assert!(!game.tech_available(1, Tech::COMBINED_ARMS));

    game.choose_research_tech(1, Tech::COMBINED_ARMS, 0);
    assert_eq!(game.players[1].researching, Tech::ALPHABET);
    assert_eq!(game.players[1].research, 0);
}

#[test]
fn completed_project_follows_goal_without_running_advisor_selection() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known.clear();
    game.players[1].research_goal = Tech::GUNPOWDER;
    game.players[1].researching = Tech::BRONZE_WORKING;
    game.discover(1, Tech::BRONZE_WORKING);
    assert_eq!(game.players[1].research_goal, Tech::GUNPOWDER);
    assert!(game.tech_available(1, game.players[1].researching));
    assert!(
        game.research_path(1, Tech::GUNPOWDER)
            .contains(&game.players[1].researching)
    );
}

#[test]
fn completed_goal_uses_reservoir_sampling_in_reference_draw_order() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known.clear();
    game.players[1].research_goal = Tech::BRONZE_WORKING;
    game.players[1].researching = Tech::BRONZE_WORKING;
    let initial_rng = game.random_snapshot();
    game.players[1].known.push(Tech::BRONZE_WORKING);
    let mut sampled = Tech::FUTURE;
    let mut count = 0;
    for tech in Tech::all() {
        if game.tech_available(1, tech) {
            count += 1;
            if game.rand_below(count) == 0 {
                sampled = tech;
            }
        }
    }
    let expected_rng = game.random_snapshot();
    game.restore_random(initial_rng);
    game.players[1].known.clear();
    game.discover(1, Tech::BRONZE_WORKING);
    assert_eq!(game.players[1].researching, sampled);
    assert_eq!(game.random_snapshot(), expected_rng);
    assert_eq!(game.players[1].research_goal, Tech::UNSET);
}

#[test]
fn unrelated_discovery_does_not_exempt_existing_research() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ai_free_bulbs.clear();
    game.players[1].researching = Tech::BRONZE_WORKING;
    game.players[1].research = 12;
    game.discover(1, Tech::POTTERY);
    assert!(!game.ai_free_bulbs.contains_key(&1));
    game.discover(1, Tech::BRONZE_WORKING);
    assert_eq!(game.ai_free_bulbs.get(&1), Some(&12));
}

#[test]
fn discovery_overflow_can_complete_the_next_goal_in_the_same_update() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known = Tech::all()
        .filter(|&tech| tech != Tech::BRONZE_WORKING && tech != Tech::ALPHABET)
        .collect();
    game.players[1].research_goal = Tech::ALPHABET;
    game.players[1].researching = Tech::BRONZE_WORKING;
    game.players[1].research =
        game.target_cost(1, Tech::BRONZE_WORKING) + game.target_cost(1, Tech::ALPHABET) + 7;
    let upkeep = game.technology_upkeep(1);
    game.resolve_research_turn(1, upkeep);
    assert!(game.has_tech(1, Tech::BRONZE_WORKING));
    assert!(game.has_tech(1, Tech::ALPHABET));
    assert_eq!(game.players[1].researching, Tech::FUTURE);
    assert_eq!(game.players[1].research, 7);
    assert_eq!(game.ai_free_bulbs[&1], 7);
}

#[test]
fn completed_ai_tree_continues_with_future_technology() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known = Tech::all().collect();
    game.players[1].researching = Tech::FUTURE;
    assert_eq!(game.next_ai_research_target(1), Tech::FUTURE);
}
