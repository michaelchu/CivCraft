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
fn blocked_advisor_choice_is_rejected_without_a_frontier_substitute() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(1, 10, 24);
    game.players[1].known.clear();
    game.players[1].research = 0;
    game.ai_free_bulbs.clear();
    game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
    let (choice, _, _) = game.select_ai_research(1);
    assert!(
        !game.tech_available(1, choice),
        "fixture must prefer a deep advance: {}",
        choice.name()
    );
    let current = Tech::all()
        .find(|&tech| game.tech_available(1, tech))
        .unwrap();
    game.players[1].researching = current;
    game.manage_ai_research(1);
    assert_eq!(game.players[1].researching, current);
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
