//! Window-free strategy regression scenarios.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::new("ai-growth-expansion", ai_growth_expansion),
    Scenario::new("ai-space-strategy", ai_space_strategy),
    Scenario::new("ai-research-switching", ai_research_switching),
    Scenario::new("ai-aircraft-advisor", ai_aircraft_advisor),
    Scenario::new("nuclear-ai-role-mapping", nuclear_ai_role_mapping),
    Scenario::new("ai-ground-invasion", ai_ground_invasion),
    Scenario::new("ai-settlement-value", ai_settlement_value),
    Scenario::new(
        "ai-settlement-goal-government",
        ai_settlement_goal_government,
    ),
    Scenario::new("ai-settlement-reservations", ai_settlement_reservations),
    Scenario::new("ai-future-defenders", ai_future_defenders),
    Scenario::new("ai-government-reevaluation", ai_government_reevaluation),
    Scenario::new("ai-embarked-paradrop-danger", ai_embarked_paradrop_danger),
    Scenario::new("ai-ferry-assignment-state", ai_ferry_assignment_state),
    Scenario::new("ai-happiness-effects", ai_happiness_effects),
    Scenario::new("industrial-shield-output", industrial_shield_output),
    Scenario::new("ai-city-manager", ai_city_manager),
    Scenario::new("ai-treaty-policy", ai_treaty_policy),
    Scenario::new("ai-worker-remote-work", ai_worker_remote_work),
];

fn ai_growth_expansion(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_growth_expansion", sim.ai_growth_expansion_playtest(),
                "Aqueduct growth limits and expansion availability remained valid while the city advisor applied its production choice.".to_owned()));
}

fn ai_space_strategy(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_space_strategy", sim.ai_space_strategy_playtest(),
                "The AI warned an allied spaceship builder once, respected its warning cooldown, then prepared war when the rival launched.".to_owned()));
}

fn ai_research_switching(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_research_switching", sim.ai_research_switching_playtest(),
                "The AI preserved nearly completed research, paid for an early switch, and retained discovery overflow bulbs.".to_owned()));
}

fn ai_aircraft_advisor(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_aircraft_advisor", sim.ai_aircraft_advisor_playtest(),
                "Imported Fighter fuel/path capability and the city Oil production gate matched the Conquests prototype data.".to_owned()));
}

fn nuclear_ai_role_mapping(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("nuclear_ai_role_mapping", sim.nuclear_ai_role_playtest(),
                "The selected C2C3 Nuclear unit has no AI role; standalone automatic target selection is outside the ruleset's AI behavior.".to_owned()));
}

fn ai_ground_invasion(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_ground_invasion", sim.ai_ground_invasion_playtest(),
                "A bomber struck a city supported by an explicit distant infantry mission before the infantry could arrive.".to_owned()));
}

fn ai_settlement_value(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_settlement_value", sim.ai_settlement_value_playtest(),
                "Site values matched per-tile rounding, penalized city crowding, and rejected a polluted site that could not grow.".to_owned()));
}

fn ai_settlement_goal_government(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_settlement_goal_government", sim.ai_settlement_goal_government_playtest(),
                "Settlement projected a locked future government without changing live government or rates.".to_owned()));
}

fn ai_settlement_reservations(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_settlement_reservations", sim.ai_settlement_reservations_playtest(),
                "Multiple founders retained separate destinations and released reservations on capture or cancellation.".to_owned()));
}

fn ai_future_defenders(sim: &mut Simulation, checks: &mut Checks) {
    let forecast = sim.ai_defender_forecast_playtest();
    checks.push(("ai_future_defenders", forecast,
                "Distant defender forecast selected a buildable, trained counter-unit when the arrival exceeded one turn.".to_owned()));
}

fn ai_government_reevaluation(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_government_reevaluation", sim.ai_government_cache_playtest(),
                "Government scores stayed cached until their reference deadline while newly researched governments became available immediately.".to_owned()));
}

fn ai_embarked_paradrop_danger(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_embarked_paradrop_danger", sim.ai_embarked_paradrop_danger_playtest(),
                "City danger included transported armies and paradrop reach without executing illegal moves.".to_owned()));
}

fn ai_ferry_assignment_state(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_ferry_assignment_state", sim.ai_ferry_registry_playtest(),
                "Empty assigned ferries stayed reserved, extra cargo did not steal command, and released or dead passengers freed their boats.".to_owned()));
}

fn ai_happiness_effects(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_happiness_effects", sim.ai_happiness_effects_playtest(),
                "Happiness and health advisor wants followed individual effect values, citizen stages, and wonder ranges.".to_owned()));
}

fn industrial_shield_output(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("industrial_shield_output", sim.industrial_output_playtest(),
                "Factories and manufacturing plants increased real shield output; power sources amplified industry without stacking.".to_owned()));
}

fn ai_city_manager(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_city_manager", sim.ai_city_manager_playtest(),
                "City management chose feasible worker and specialist allocations through the reference constraint relaxation stages.".to_owned()));
}

fn ai_treaty_policy(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_treaty_policy", sim.ai_treaty_policy_playtest(),
                "Human gifts, shared vision, paid direct alliances, and Senate-blocked war preparation followed the reference treaty policies.".to_owned()));
}

fn ai_worker_remote_work(sim: &mut Simulation, checks: &mut Checks) {
    let (remote, reservations, shelter) = sim.ai_worker_planning_playtest();
    checks.extend([
                (
                    "ai_worker_remote_work",
                    remote,
                    "A worker selected useful remote cleanup instead of building a local road."
                        .to_owned(),
                ),
                (
                    "ai_worker_reservations",
                    reservations,
                    "A closer worker took a work reservation without duplicating an active job."
                        .to_owned(),
                ),
                (
                    "ai_worker_shelter",
                    shelter,
                    "An automated human or displaced worker sought an allied city when no useful work remained."
                        .to_owned(),
                ),
            ]);
}
