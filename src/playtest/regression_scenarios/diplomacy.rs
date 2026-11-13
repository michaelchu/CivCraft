//! Diplomatic state, contact, offers, treaties, and war planning.

use super::*;

impl Playtest {
    pub(super) fn advance_diplomacy_scenarios(&mut self, frame: u32) {
        if frame == 531 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_emergency_defense_purchase", sim.ai_defense_purchase_playtest(),
                "The AI waited when short on gold, then rushed a defender in the exposed city and left the safe city's production alone.".to_owned()));
        }

        if frame == 533 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_emergency_defense_sale",
                sim.ai_defense_sale_playtest(),
                "The exposed city sold its granary to rush a defender and kept its walls."
                    .to_owned(),
            ));
        }

        if frame == 535 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_civilian_upgrade_budget", sim.ai_civilian_upgrade_playtest(),
                "Checked wartime restraint, the gold reserve, and a peacetime Worker upgrade in a city.".to_owned()));
        }

        if frame == 537 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_purchase_reserve_tax", sim.ai_reserve_playtest(),
                "An unaffordable upgrade raised the reserve and tax rate; both returned to normal after purchase.".to_owned()));
        }

        if frame == 553 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_map_exchange", sim.treaty_map_playtest(),
                "Verified payment, sea-only coverage, allied world-map transfer, and no shared live vision.".to_owned()));
        }

        if frame == 557 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "diplomacy_city_clause",
                sim.city_clause_playtest(),
                "A treaty City clause rejects the capital and transfers an eligible city during war with its treaty gold.".to_owned(),
            ));
        }

        if frame == 559 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "diplomacy_city_clause_bundle",
                sim.city_clause_bundle_playtest(),
                "A treaty City clause transfers a noncapital during war with map and gold clauses; a capital invalidates the whole offer.".to_owned(),
            ));
        }

        if frame == 563 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_defense_route_cost", sim.ai_threat_route_cost_playtest(),
                "A Horseman was out of range over mountains, two turns away on grassland, and reachable this turn on roads.".to_owned()));
        }

        if frame == 565 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_road_reinforcements", sim.ai_road_reinforcement_playtest(),
                "A Marine reached the threatened city once the road opened and left a guard behind.".to_owned()));
        }

        if frame == 569 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_map_observation_transfer", sim.map_snapshot_playtest(),
                "A sea map transferred the seller's observed ocean after the tile changed out of sight.".to_owned()));
        }

        if frame == 571 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_war_preparation", sim.ai_war_planning_playtest(),
                "AI preparation preserved peace until declaration and enforced its post-declaration treaty refusal.".to_owned()));
        }
    }
}
