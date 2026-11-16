//! Window-free diplomacy regression scenarios.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "ai-emergency-defense-purchase",
        ai_emergency_defense_purchase,
    ),
    Scenario::new("ai-emergency-defense-sale", ai_emergency_defense_sale),
    Scenario::new("ai-civilian-upgrade-budget", ai_civilian_upgrade_budget),
    Scenario::new("ai-purchase-reserve-tax", ai_purchase_reserve_tax),
    Scenario::new("diplomacy-map-exchange", diplomacy_map_exchange),
    Scenario::new("diplomacy-city-clause", diplomacy_city_clause),
    Scenario::new("diplomacy-city-clause-bundle", diplomacy_city_clause_bundle),
    Scenario::new("ai-defense-route-cost", ai_defense_route_cost),
    Scenario::new("ai-road-reinforcements", ai_road_reinforcements),
    Scenario::new("ai-map-observation-transfer", ai_map_observation_transfer),
    Scenario::new("ai-war-preparation", ai_war_preparation),
];

fn ai_emergency_defense_purchase(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_emergency_defense_purchase", sim.ai_defense_purchase_playtest(),
                "The AI waited when short on gold, then rushed a defender in the exposed city and left the safe city's production alone.".to_owned()));
}

fn ai_emergency_defense_sale(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "ai_emergency_defense_sale",
        sim.ai_defense_sale_playtest(),
        "The exposed city sold its granary to rush a defender and kept its walls.".to_owned(),
    ));
}

fn ai_civilian_upgrade_budget(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "ai_civilian_upgrade_budget",
        sim.ai_civilian_upgrade_playtest(),
        "Checked wartime restraint, the gold reserve, and a peacetime Worker upgrade in a city."
            .to_owned(),
    ));
}

fn ai_purchase_reserve_tax(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_purchase_reserve_tax", sim.ai_reserve_playtest(),
                "An unaffordable upgrade raised the reserve and tax rate; both returned to normal after purchase.".to_owned()));
}

fn diplomacy_map_exchange(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("diplomacy_map_exchange", sim.treaty_map_playtest(),
                "Verified payment, sea-only coverage, allied world-map transfer, and no shared live vision.".to_owned()));
}

fn diplomacy_city_clause(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
                "diplomacy_city_clause",
                sim.city_clause_playtest(),
                "A treaty City clause rejects the capital and transfers an eligible city during war with its treaty gold.".to_owned(),
            ));
}

fn diplomacy_city_clause_bundle(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
                "diplomacy_city_clause_bundle",
                sim.city_clause_bundle_playtest(),
                "A treaty City clause transfers a noncapital during war with map and gold clauses; a capital invalidates the whole offer.".to_owned(),
            ));
}

fn ai_defense_route_cost(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_defense_route_cost", sim.ai_threat_route_cost_playtest(),
                "A Horseman was out of range over mountains, two turns away on grassland, and reachable this turn on roads.".to_owned()));
}

fn ai_road_reinforcements(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_road_reinforcements", sim.ai_road_reinforcement_playtest(),
                "A Marine moved toward the higher-priority threatened city on the road route while a guard stayed home.".to_owned()));
}

fn ai_map_observation_transfer(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "ai_map_observation_transfer",
        sim.map_snapshot_playtest(),
        "A sea map transferred the seller's observed ocean after the tile changed out of sight."
            .to_owned(),
    ));
}

fn ai_war_preparation(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("ai_war_preparation", sim.ai_war_planning_playtest(),
                "AI preparation preserved peace until declaration and enforced its post-declaration treaty refusal.".to_owned()));
}
