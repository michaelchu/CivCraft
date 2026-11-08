//! Implements strategy behavior within the regression scenarios subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_strategy_scenarios(&mut self, frame: u32) {
        if frame == 574 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_production_priorities", sim.ai_production_priorities_playtest(),
                "A growth-capped city prioritized its Aqueduct over expansion and used that priority for production and spending.".to_owned()));
        }

        if frame == 577 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_space_strategy", sim.ai_space_strategy_playtest(),
                "The AI warned an allied spaceship builder once, respected its warning cooldown, then prepared war when the rival launched.".to_owned()));
        }

        if frame == 580 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_research_switching", sim.ai_research_switching_playtest(),
                "The AI preserved nearly completed research, paid for an early switch, and retained discovery overflow bulbs.".to_owned()));
        }

        if frame == 583 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_aircraft_advisor", sim.ai_aircraft_advisor_playtest(),
                "The native HP-loss gate blocked offensive aircraft production while an existing Fighter attacked a profitable surface target.".to_owned()));
        }

        if frame == 586 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_nuclear_advisor", sim.ai_nuclear_advisor_playtest(),
                "Standalone nuclear targeting preserved the reference's strict movement-cost cutoff, despite a valuable target.".to_owned()));
        }

        if frame == 589 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_merchant_valuation", sim.ai_merchant_valuation_playtest(),
                "Trade valuation charged the owned partner's lost back route and valued wonder assistance using its arrival-time rush-price reduction.".to_owned()));
        }

        if frame == 592 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_ground_invasion", sim.ai_ground_invasion_playtest(),
                "A bomber struck a city supported by an explicit distant infantry mission before the infantry could arrive.".to_owned()));
        }

        if frame == 595 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_wonder_advisor", sim.ai_wonder_advisor_playtest(),
                "Empire-wide benefits selected a wonder and its helper advisor stopped requesting caravans when contributions covered construction.".to_owned()));
        }

        if frame == 598 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_diplomatic_love", sim.ai_diplomatic_love_playtest(),
                "Peace improved trust, provocation caused resentment, and an accepted gift changed the relationship using reference valuations.".to_owned()));
        }

        if frame == 601 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_settlement_value", sim.ai_settlement_value_playtest(),
                "Site values matched per-tile rounding, penalized city crowding, and rejected a polluted site that could not grow.".to_owned()));
        }

        if frame == 604 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_settlement_goal_government", sim.ai_settlement_goal_government_playtest(),
                "Settlement projected a locked future government without changing live government or rates.".to_owned()));
        }

        if frame == 607 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_settlement_reservations", sim.ai_settlement_reservations_playtest(),
                "Multiple founders retained separate destinations and released reservations on capture or cancellation.".to_owned()));
        }

        if frame == 610 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_future_defenders", sim.ai_defender_forecast_playtest(),
                "Distant attacks anticipated trained counter-units while preserving stronger veteran garrisons and live hit points.".to_owned()));
        }

        if frame == 613 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_observer_diplomacy_cadence", sim.ai_observer_incident_cadence_playtest(),
                "Diplomatic trespass used observer visibility and offers followed the reference cooldown.".to_owned()));
        }

        if frame == 616 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_action_incidents", sim.ai_incident_action_playtest(),
                "A real bribe changed diplomatic love and casus belli; a stale-price rejection caused no incident.".to_owned()));
        }

        if frame == 619 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_merchant_missions", sim.ai_merchant_missions_playtest(),
                "Merchants applied allied-only searches, retained valid destinations, and gave up waiting for absent boats after the reference deadline.".to_owned()));
        }

        if frame == 622 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_merchant_coastal_pickup", sim.ai_merchant_coastal_pickup_playtest(),
                "A Freight waiting on ordinary coastal land boarded a ferry and delivered to an overseas ally.".to_owned()));
        }

        if frame == 625 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_government_reevaluation", sim.ai_government_cache_playtest(),
                "Government scores stayed cached until their reference deadline while newly researched governments became available immediately.".to_owned()));
        }

        if frame == 628 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_embarked_paradrop_danger", sim.ai_embarked_paradrop_danger_playtest(),
                "City danger included transported armies and paradrop reach without executing illegal moves.".to_owned()));
        }

        if frame == 631 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("embassy_contact_lifetime", sim.embassy_contact_playtest(),
                "Physical contact expired, renewed through interaction, and bilateral embassy clauses kept diplomacy available.".to_owned()));
        }

        if frame == 634 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_ferry_assignment_state", sim.ai_ferry_registry_playtest(),
                "Empty assigned ferries stayed reserved, extra cargo did not steal command, and released or dead passengers freed their boats.".to_owned()));
        }

        if frame == 637 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_happiness_effects", sim.ai_happiness_effects_playtest(),
                "Happiness and health advisor wants followed individual effect values, citizen stages, and wonder ranges.".to_owned()));
        }

        if frame == 640 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("industrial_shield_output", sim.industrial_output_playtest(),
                "Factories and manufacturing plants increased real shield output; power sources amplified industry without stacking.".to_owned()));
        }

        if frame == 643 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_diplomatic_intelligence", sim.ai_diplomatic_intelligence_playtest(),
                "Cached enemy-alliance intelligence blocked peace until the next intelligence refresh, then allowed negotiation.".to_owned()));
        }

        if frame == 646 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_city_manager", sim.ai_city_manager_playtest(),
                "City management chose feasible worker and specialist allocations through the reference constraint relaxation stages.".to_owned()));
        }

        if frame == 649 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_treaty_policy", sim.ai_treaty_policy_playtest(),
                "Human gifts, shared vision, paid direct alliances, and Senate-blocked war preparation followed the reference treaty policies.".to_owned()));
        }

        if frame == 652 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("world_seam_work", sim.world_seam_work_playtest(),
                "City radius, territory, ports, and completed irrigation used neighbors across the wrapped world seam.".to_owned()));
        }

        if frame == 655 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (remote, reservations, shelter) = sim.ai_worker_planning_playtest();
            self.results.extend([
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
    }
}
