//! Implements native rules behavior within the regression scenarios subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_native_rule_scenarios(&mut self, frame: u32, game: &mut Game) {
        if frame == 658 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_exploration", sim.ai_exploration_playtest(),
                "Automated exploration discovered terrain through known-map frontier scoring and legal routes.".to_owned()));
        }

        if frame == 661 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_founder_work_arbitration", sim.founder_work_arbitration_playtest(),
                "A founder compared city placement against useful terrain work and preserved an existing city mission.".to_owned()));
        }

        if frame == 667 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("city_output_rules", sim.city_output_rules_playtest(),
                "City-center, Harbour, wonder, commercial, food-waste, and celebration output followed the reference.".to_owned()));
        }

        if frame == 673 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_idle_exploration", sim.ai_idle_exploration_playtest(),
                "Idle military units and ferries explored known frontiers and revealed terrain immediately.".to_owned()));
        }

        if frame == 676 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("exploration_legality", sim.ai_exploration_legality_playtest(),
                "Eligible human ground and naval units explored; fuelled aircraft and helicopters could not enable exploration.".to_owned()));
        }

        if frame == 679 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (selected, followed, attacked) = sim.ai_formal_guard_playtest();
            self.results.extend([
                (
                    "ai_guard_selection",
                    selected,
                    "Eligible guards accepted requested charges.".to_owned(),
                ),
                (
                    "ai_guard_movement",
                    followed,
                    "Charges waited for exhausted guards and guards followed actual movement."
                        .to_owned(),
                ),
                (
                    "ai_guard_attack",
                    attacked,
                    "Guard waiting did not prevent a charge from attacking.".to_owned(),
                ),
            ]);
        }

        if frame == 682 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (speculative, capacity, retained) = sim.ai_guard_ferry_capacity_playtest();
            self.results.extend([
                (
                    "ai_guard_ferry_estimate",
                    speculative,
                    "Speculative city planning used native one-slot boat estimates.".to_owned(),
                ),
                (
                    "ai_guard_ferry_capacity",
                    capacity,
                    "Actual guarded trips selected a boat with two available places.".to_owned(),
                ),
                (
                    "ai_guard_ferry_goal",
                    retained,
                    "A trip without sufficient ferry capacity retained its destination.".to_owned(),
                ),
            ]);
        }

        if frame == 685 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("damaged_movement", sim.damaged_movement_playtest(),
                "Damage-scaled movement used whole-move class minima and added movement effects after damage.".to_owned()));
        }

        if frame == 688 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (selected, tax, emergency, native_type) = sim.ai_bodyguard_production_playtest();
            self.results.extend([
                ("ai_bodyguard_production", selected, "Production advice supplied a trained guard for a real charge.".to_owned()),
                ("ai_bodyguard_null_unit_upkeep", tax, "Hypothetical guard upkeep followed the source's unknown-unit requirement context.".to_owned()),
                ("ai_bodyguard_emergency", emergency, "An urgent local defense retained priority over a guard bid.".to_owned()),
                ("ai_bodyguard_type_choice", native_type, "Guard advice chose the native best defender before evaluating charge compatibility.".to_owned()),
            ]);
        }

        if frame == 691 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("settlement_worker_order", sim.settlement_reservation_order_playtest(),
                "Equal-yield future workers and their reservations followed native city-index order.".to_owned()));
        }

        if frame == 694 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("barbarian_output_waste", sim.barbarian_output_waste_playtest(),
                "Barbarian cities kept government base waste while receiving the source distance relief.".to_owned()));
        }

        if frame == 697 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("civilian_peace_borders", sim.civilian_border_playtest(),
                "NonMil civilians crossed peaceful territory; military units and zero-attack carriers remained restricted.".to_owned()));
        }

        if frame == 703 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "native_ai_tax_manager",
                sim.ai_tax_manager_playtest(),
                "Tax management followed native reserve, science, war and future-tech priorities."
                    .to_owned(),
            ));
        }

        if frame == 706 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (training, investment, emergency, global) =
                sim.military_training_advisor_playtest();
            for (name, passed) in [
                ("military_training_choice", training),
                ("military_training_investment", investment),
                ("military_training_emergency", emergency),
                ("military_training_global_effect", global),
            ] {
                self.results.push((
                    name,
                    passed,
                    "Military advice applied native veteran-building substitution and exceptions."
                        .to_owned(),
                ));
            }
        }

        if frame == 709 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "zero_attack_carrier_path",
                sim.ai_zero_attack_carrier_path_playtest(),
                "A zero-attack carrier's planned endpoint matched legal movement execution."
                    .to_owned(),
            ));
        }

        if frame == 712 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (taxes, workers) = sim.ai_tax_phase_playtest();
            self.results.push((
                "ai_tax_before_city_advice",
                taxes,
                "Building advice used the new tax rates to switch from Library to Marketplace."
                    .to_owned(),
            ));
            self.results.push((
                "ai_emergency_workers_before_advice",
                workers,
                "Emergency citizen allocation ran before military advice.".to_owned(),
            ));
        }

        if frame == 715 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (costs, nativity, zoc) = sim.native_movement_rules_playtest();
            for (name, passed) in [
                ("native_infrastructure_costs", costs),
                ("native_terrain_and_boarding", nativity),
                ("native_zoc_flags", zoc),
            ] {
                self.results.push((name, passed,
                    "Movement used source infrastructure restrictions, class nativity and explicit ZOC flags.".to_owned()));
            }
        }

        if frame == 718 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("source_ai_goto_profiles", sim.ai_source_goto_playtest(),
                "AI GOTO used native risk costs, action endpoints, ferry overlap and task restrictions.".to_owned()));
        }

        if frame == 724 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (arrival, owner, cities) = sim.native_list_order_playtest();
            for (name, passed) in [
                ("native_player_and_tile_order", arrival),
                ("native_unit_transfer_order", owner),
                ("native_city_transfer_order", cities),
            ] {
                self.results.push((
                    name,
                    passed,
                    "Player ownership and tile arrivals preserved separate reference list orders."
                        .to_owned(),
                ));
            }
        }

        if frame == 727 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_city_activity_order", sim.native_city_activity_order_playtest(),
                "City activity used the player list and reference random swap-removal order after nationality conversion.".to_owned()));
        }

        if frame == 733 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (quiet, first, defended, police) = sim.native_city_defense_advisor_playtest();
            for (name, passed) in [
                ("native_defense_quiet_city", quiet),
                ("native_defense_first_guard", first),
                ("native_defense_existing_guard", defended),
                ("native_defense_martial_law", police),
            ] {
                self.results.push((name, passed, "City defense priorities used source danger, urgency, defense and martial-law wants.".to_owned()));
            }
        }

        if frame == 736 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (founding, growth, reduction) = sim.city_worker_lifecycle_playtest();
            for (name, passed) in [
                ("native_worker_allocation_founding", founding),
                ("native_worker_allocation_growth", growth),
                ("native_worker_allocation_reduction", reduction),
            ] {
                self.results.push((name, passed, "City lifecycle events used the native citizen manager and size-dependent priorities.".to_owned()));
            }
        }

        if frame == 739 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_gameplay_random_stream", sim.native_random_stream_playtest(),
                "New games continued the post-start native stream; seeded calls matched literal C oracle values.".to_owned()));
        }

        if frame == 742 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_cargo_boarding_order", sim.native_cargo_order_playtest(),
                "Boarding appended cargo independently of unit creation; reboarding moved a unit to the end.".to_owned()));
        }

        if frame == 748 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (repair, fortify, exhausted, rescue) = sim.unit_turn_lifecycle_playtest();
            for (name, passed) in [
                ("native_repair_before_construction", repair),
                ("native_fortification_timing", fortify),
                ("native_exhausted_activity_progress", exhausted),
                ("native_rescue_movement_state", rescue),
            ] {
                self.results.push((name, passed, "Unit restoration preceded city construction; next-phase activity preserved native movement and healing timing.".to_owned()));
            }
        }

        if frame == 751 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_city_healing", sim.city_healing_playtest(),
                "Barracks restore a damaged home unit fully; an unbarracked city uses its regular city-center healing amount.".to_owned()));
        }

        if frame == 754 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (quiet, continent, invasion, research) = sim.native_defensive_effects_playtest();
            for (name, passed) in [
                ("native_defense_effect_baseline", quiet),
                ("native_defense_continent_threats", continent),
                ("native_defense_invasion_threats", invasion),
                ("native_defensive_research_feedback", research),
            ] {
                self.results.push((name, passed, "Defense effect rows and static defensive research wants followed the native advisor formulas.".to_owned()));
            }
        }

        if frame == 757 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (road, coworkers, precompleted, no_worker_promotion) =
                sim.activity_completion_order_playtest();
            for (name, passed) in [
                ("native_per_unit_road_completion", road),
                ("native_cooperative_work_early_stop", coworkers),
                ("native_precompleted_exhausted_work", precompleted),
                (
                    "native_workers_do_not_gain_work_veterancy",
                    no_worker_promotion,
                ),
            ] {
                self.results.push((name, passed, "Work completion applied before later units refilled movement or progressed the same project; Conquests workers did not gain ranks from working.".to_owned()));
            }
        }

        if frame == 763 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (stats, flags, order) = sim.native_unit_catalog_playtest();
            for (name, passed) in [
                ("native_complete_unit_catalog_stats", stats),
                ("native_complete_unit_catalog_flags", flags),
                ("native_complete_unit_catalog_order", order),
            ] {
                self.results.push((name, passed, "Unit catalog fixtures checked source statistics and flags; native rule order was independent of saved enum discriminants.".to_owned()));
            }
        }

        if frame == 766 && self.act_passed {
            let (mapped, distinct, labels) = crate::client::native_campaign_unit_art_playtest();
            for (name, passed) in [
                ("civ3_campaign_unit_art_roles", mapped),
                ("civ3_campaign_unit_art_role_distinction", distinct),
                ("civ3_campaign_unit_art_labels", labels),
            ] {
                self.results.push((name, passed, "All 92 campaign units map to distinct source-art roles and have display labels.".to_owned()));
            }
        }

        if frame == 772 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let worked = sim.transport_worker_actions_playtest();
            self.results.push((
                "native_transport_oil_platform",
                worked,
                "A source Worker aboard a Transport built an oil platform over the required ocean Oil resource."
                    .to_owned(),
            ));
        }

        if frame == 775 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (land, unknown, king, speed) = sim.native_hunter_tactics_playtest();
            for (name, passed) in [
                ("native_land_hunter_dispatch_and_scan", land),
                ("native_normal_hunter_omniscience", unknown),
                ("native_hunter_game_loss_target_value", king),
                ("native_hunter_type_speed_comparison", speed),
            ] {
                self.results.push((name, passed, "Existing Hunter-role units followed native PF scan order, Normal knowledge policy, GameLoss value and nominal type-speed comparisons.".to_owned()));
            }
        }

        if frame == 778 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let id = game.sim.transport_hud_playtest_setup();
            game.perform_action(UiAction::SelectUnit(id));
        }
    }
}
