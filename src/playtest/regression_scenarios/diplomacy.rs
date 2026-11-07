use super::*;

impl Playtest {
    pub(super) fn advance_diplomacy_scenarios(&mut self, frame: u32) {
        if frame == 531 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_emergency_defense_purchase", sim.ai_defense_purchase_playtest(),
                "The AI waited when short on gold, then rushed a defender in the exposed city and left the safe city's production alone.".to_owned()));
        }

        if frame == 533 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_emergency_defense_sale",
                sim.ai_defense_sale_playtest(),
                "The exposed city sold its granary to rush a defender and kept its walls."
                    .to_owned(),
            ));
        }

        if frame == 535 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_civilian_upgrade_budget", sim.ai_civilian_upgrade_playtest(),
                "Checked wartime restraint, the gold reserve, and a peacetime Worker upgrade in a city.".to_owned()));
        }

        if frame == 537 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_purchase_reserve_tax", sim.ai_reserve_playtest(),
                "An unaffordable upgrade raised the reserve and tax rate; both returned to normal after purchase.".to_owned()));
        }

        if frame == 539 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_diplomat_bribery", sim.ai_bribery_playtest(),
                "The AI protected its reserve against a passive target and spent it to bribe an immediate threat.".to_owned()));
        }

        if frame == 541 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_city_counterintelligence", sim.ai_counterintelligence_playtest(),
                "A garrisoned city queued a Spy, then an arriving Diplomat stayed to protect it from a rival agent.".to_owned()));
        }

        if frame == 543 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_offensive_city_missions",
                sim.ai_city_mission_playtest(),
                "AI Diplomats approached a rival city, established an embassy, then stole Pottery."
                    .to_owned(),
            ));
        }

        if frame == 545 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_incite_revolt_budget",
                sim.ai_incite_revolt_playtest(),
                "An AI Diplomat declined an unaffordable revolt, then paid to acquire the city."
                    .to_owned(),
            ));
        }

        if frame == 547 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_city_sabotage", sim.ai_city_sabotage_playtest(),
                "An adjacent AI agent left production intact under armistice and destroyed it at war.".to_owned()));
        }

        if frame == 549 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_offensive_agent_recruitment", sim.ai_offensive_recruitment_playtest(),
                "A queued or active agent reserved its target, and the AI recruited another agent when a distinct target became available.".to_owned()));
        }

        if frame == 551 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_persistent_city_mission",
                sim.ai_persistent_mission_playtest(),
                "An agent kept its target across turns and redirected when that city disappeared."
                    .to_owned(),
            ));
        }

        if frame == 553 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_map_exchange", sim.treaty_map_playtest(),
                "Verified payment, sea-only coverage, allied world-map transfer, and no shared live vision.".to_owned()));
        }

        if frame == 555 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_map_treaty_offer", sim.ai_map_offer_playtest(),
                "The AI offered an affordable sea map, and acceptance transferred knowledge and cleared the offer.".to_owned()));
        }

        if frame == 557 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_city_purchase", sim.city_trade_playtest(),
                "A capital and stale price were rejected. A purchased city kept its population, stocks, improvements and eligible units, and relocated an incompatible visitor.".to_owned()));
        }

        if frame == 559 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "diplomacy_city_buyback",
                sim.city_buyback_playtest(),
                "The city's founding civilization bought it back for half the regular price."
                    .to_owned(),
            ));
        }

        if frame == 561 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_city_bundle", sim.city_bundle_playtest(),
                "A city, map, and gold traded together. A city that became a capital blocked the entire deal.".to_owned()));
        }

        if frame == 563 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_defense_route_cost", sim.ai_threat_route_cost_playtest(),
                "A Horseman was out of range over mountains, two turns away on grassland, and reachable this turn on roads.".to_owned()));
        }

        if frame == 565 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_road_reinforcements", sim.ai_road_reinforcement_playtest(),
                "A Marine reached the threatened city once the road opened and left a guard behind.".to_owned()));
        }

        if frame == 567 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_merchant_road_delivery", sim.ai_merchant_road_playtest(),
                "The same Caravan reached the destination and delivered in one turn only when the road was open.".to_owned()));
        }

        if frame == 569 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_map_observation_transfer", sim.map_snapshot_playtest(),
                "A sea map transferred the seller's observed ocean after the tile changed out of sight.".to_owned()));
        }

        if frame == 571 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_war_preparation", sim.ai_war_planning_playtest(),
                "AI preparation preserved peace until declaration and enforced its post-declaration treaty refusal.".to_owned()));
        }
    }
}
