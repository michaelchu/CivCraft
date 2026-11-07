//! Checks for the native lifetime and accumulation of AI research feedback.
use super::*;

#[test]
fn unset_goal_roundtrips_without_becoming_an_invention_or_current_target() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.native_unset_goal_save_playtest());
}

#[test]
fn research_feedback_is_ordered_retained_copied_and_transient_on_load() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.native_research_phase_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn live_last_activities_replace_previous_phase_wants() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.add_ai_tech_want(1, Tech::ALPHABET, 1_000_000.0);
    game.run_ai_last_activities(1);
    assert!(game.ai_tech_wants.contains_key(&1));
    assert!(game.advisor_tech_want(1, Tech::ALPHABET) < 1_000_000.0);
    let before = game.ai_tech_wants[&1].clone();
    for tech in Tech::all() {
        let _ = game.advisor_tech_want(1, tech);
    }
    assert_eq!(game.ai_tech_wants[&1], before);
}
