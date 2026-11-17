//! Scenarios first registered for the headless simulator. They keep their
//! original identifiers and run with every civilization AI-driven.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::all_ai("city-healing", city_healing),
    Scenario::all_ai("settlement-ferry", settlement_ferry),
    Scenario::all_ai("combat-matrix", combat_matrix),
    Scenario::new("ai-paradrops", ai_paradrops),
];

fn city_healing(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "city_healing",
        sim.city_healing_playtest(),
        "Barracks and ordinary city recovery used their expected rules".into(),
    ));
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

fn combat_matrix(sim: &mut Simulation, checks: &mut Checks) {
    checks.extend(sim.freeciv_combat_matrix_playtest());
}

fn ai_paradrops(sim: &mut Simulation, checks: &mut Checks) {
    let (defended, refused, captured) = sim.ai_paradrop_playtest();
    checks.push(("ai_paradrops", defended && refused && captured,
                format!("AI reinforced an empty threatened city before attacking: {defended}. It refused the defended landing: {refused}. It captured the enemy city once the defender left: {captured}.")));
}
