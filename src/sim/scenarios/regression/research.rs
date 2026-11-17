//! Window-free research regression scenarios.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "native-counter-research-quiet-city",
        native_counter_research_quiet_city,
    ),
    Scenario::new(
        "native-research-positive-float-accumulation",
        native_research_positive_float_accumulation,
    ),
    Scenario::new(
        "native-research-retained-phase-snapshot",
        native_research_retained_phase_snapshot,
    ),
    Scenario::new(
        "native-offensive-known-target-policy",
        native_offensive_known_target_policy,
    ),
    Scenario::new(
        "native-research-gift-clears-matching-goal",
        native_research_gift_clears_matching_goal,
    ),
    Scenario::new(
        "native-unset-goal-save-and-command-rules",
        native_unset_goal_save_and_command_rules,
    ),
];

fn native_counter_research_quiet_city(sim: &mut Simulation, checks: &mut Checks) {
    let (quiet, first, context) = sim.counter_defender_research_playtest();
    for (name, passed) in [
        ("native_counter_research_quiet_city", quiet),
        ("native_counter_research_first_missing_defender", first),
        ("native_counter_research_threat_context", context),
    ] {
        checks.push((name, passed, "A quiet city requested no counter research; a reachable threat requested the native missing defender technology with post-specific urgency.".to_owned()));
    }
}

fn native_research_positive_float_accumulation(sim: &mut Simulation, checks: &mut Checks) {
    let (positive, signed, large) = sim.native_research_arithmetic_playtest();
    for (name, passed) in [
        ("native_research_positive_float_accumulation", positive),
        ("native_research_signed_float_accumulation", signed),
        ("native_research_large_float_rounding", large),
    ] {
        checks.push((name, passed, "Research selection matched independent C vectors with truncation at every int-plus-float assignment.".to_owned()));
    }
}

fn native_research_retained_phase_snapshot(sim: &mut Simulation, checks: &mut Checks) {
    let (retained, order, copied, loaded) = sim.native_research_phase_playtest();
    for (name, passed) in [
        ("native_research_retained_phase_snapshot", retained),
        ("native_research_ordered_feedback", order),
        ("native_research_civil_war_copy", copied),
        ("native_research_load_zero_snapshot", loaded),
    ] {
        checks.push((name, passed, "AI research feedback persisted between phases, accumulated in order, copied on player splits and reset on load.".to_owned()));
    }
}

fn native_offensive_known_target_policy(sim: &mut Simulation, checks: &mut Checks) {
    let (hidden, learned) = sim.offensive_research_playtest();
    for (name, passed) in [
        ("native_offensive_known_target_policy", hidden),
        ("native_offensive_learned_tech_feedback_ends", learned),
    ] {
        checks.push((name, passed, "Offensive target research feedback is limited to targets known by the player, and stops after its technology is learned.".to_owned()));
    }
}

fn native_research_gift_clears_matching_goal(sim: &mut Simulation, checks: &mut Checks) {
    let (gift, negative, valid) = sim.native_unset_research_goal_playtest();
    for (name, passed) in [
        ("native_research_gift_clears_matching_goal", gift),
        ("native_research_negative_wants_clear_goal", negative),
        ("native_unset_goal_stays_out_of_inventions", valid),
    ] {
        checks.push((name, passed, "Unset research goals remained distinct from Future Tech and could not become inventions or active research targets.".to_owned()));
    }
}

fn native_unset_goal_save_and_command_rules(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("native_unset_goal_save_and_command_rules", sim.native_unset_goal_save_playtest(),
                "An unset goal roundtripped in the existing save shape; unset current/known technologies and explicit invalid goal commands were rejected.".to_owned()));
}
