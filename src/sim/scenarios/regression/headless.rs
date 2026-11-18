//! Scenarios first registered for the headless simulator. They keep their
//! original identifiers and run with every civilization AI-driven.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::all_ai("city-healing", city_healing),
    Scenario::all_ai("settlement-ferry", settlement_ferry),
    super::super::naval::ai_assault::SCENARIO,
    super::super::naval::ship_combat::SCENARIO,
    Scenario::all_ai("expansion-balance", expansion_balance),
    Scenario::all_ai("lifecycle-history", lifecycle_history),
    Scenario::all_ai("great-leader", great_leader),
    Scenario::all_ai("research-history", research_history),
    Scenario::all_ai("combat-matrix", combat_matrix),
    Scenario::new("ai-paradrops", ai_paradrops),
    Scenario::new("diplomacy-parity", diplomacy_parity),
    Scenario::new("peaceful-first-contact", peaceful_first_contact),
    Scenario::new("proactive-trade", proactive_trade),
];

fn proactive_trade(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(sim.proactive_trade_scenario());
}

fn peaceful_first_contact(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(crate::sim::scenarios::diplomacy::peaceful_first_contact(
        sim,
    ));
}

fn diplomacy_parity(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(crate::sim::scenarios::diplomacy::diplomacy_parity(sim));
}

fn city_healing(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "city_healing",
        sim.city_healing_playtest(),
        "Barracks and ordinary city recovery used their expected rules".into(),
    ));
}

fn great_leader(sim: &mut Simulation, checks: &mut Checks) {
    let (unbuildable, completes, consumed) = sim.great_leader_playtest();
    checks.extend([
        (
            "leader_unbuildable",
            unbuildable,
            "No city can build a Leader even with every technology".into(),
        ),
        (
            "leader_completes_wonder",
            completes,
            "An AI Leader completes its wonder city's wonder in one turn".into(),
        ),
        (
            "leader_consumed",
            consumed,
            "Completing production consumes the Leader".into(),
        ),
    ]);
}

fn settlement_ferry(sim: &mut Simulation, checks: &mut Checks) {
    let (queued, sequenced, founded) = sim.settlement_ferry_production_playtest();
    checks.extend([
        (
            "ferry_queued",
            queued,
            "AI selected a ferry for an overseas settlement plan".into(),
        ),
        (
            "settler_sequenced",
            sequenced,
            "AI sequenced a Settler after completing the ferry".into(),
        ),
        (
            "island_founded",
            founded,
            "AI transported its founder and established the island city".into(),
        ),
    ]);
}

fn expansion_balance(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(sim.expansion_balance_playtest());
}

fn lifecycle_history(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(sim.lifecycle_history_playtest());
}

fn combat_matrix(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(sim.freeciv_combat_matrix_playtest());
}

fn ai_paradrops(sim: &mut Simulation, checks: &mut Checks) {
    let (defended, refused, captured) = sim.ai_paradrop_playtest();
    checks.push(("ai_paradrops", defended && refused && captured,
                format!("AI reinforced an empty threatened city before attacking: {defended}. It refused the defended landing: {refused}. It captured the enemy city once the defender left: {captured}.")));
}

fn research_history(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(sim.research_history_playtest());
}
