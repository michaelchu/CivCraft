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

        if frame == 664 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("fundamentalism_tithes", sim.fundamentalism_tithes_playtest(),
                "Fundamentalism converted uncapped content effects into gold before commercial bonuses.".to_owned()));
        }

        if frame == 667 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("city_output_rules", sim.city_output_rules_playtest(),
                "City-center, Harbour, wonder, commercial, food-waste, and celebration output followed the reference.".to_owned()));
        }

        if frame == 670 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_barbarian_leader",
                sim.ai_barbarian_leader_playtest(),
                "Barbarian leaders sought refuge and fled threats through actual AI dispatch."
                    .to_owned(),
            ));
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

        if frame == 700 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "aircraft_fuel_threat_paths",
                sim.ai_aircraft_fuel_path_playtest(),
                "Leader escape planning respected aircraft return-fuel and refueling paths."
                    .to_owned(),
            ));
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
            self.results.push(("normal_path_actions", sim.ai_normal_path_actions_playtest(),
                "Normal path planning distinguished civilian transit, military action endpoints and actual movement legality.".to_owned()));
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

        if frame == 721 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("field_unit_unrest", sim.ai_field_unit_unrest_playtest(),
                "Bombers imposed FieldUnit unrest in their home city and a friendly airbase; fighters did not.".to_owned()));
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

        if frame == 730 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_fuel_missions", sim.ai_fuel_mission_playtest(),
                "Aircraft followed native fuel paths, strict attack bounds, return trips and refueling waits.".to_owned()));
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

        if frame == 745 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (near, far, nuclear) = sim.ai_source_hunter_launch_playtest();
            for (name, passed) in [
                ("native_hunter_near_launch", near),
                ("native_hunter_far_cutoff", far),
                ("native_hunter_nuclear_cargo", nuclear),
            ] {
                self.results.push((
                    name,
                    passed,
                    "Hunter cargo used native boarding order and missile launch path limits."
                        .to_owned(),
                ));
            }
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
            self.results.push(("native_actual_turn_repair_order", sim.native_turn_orchestration_playtest(),
                "A Barracks completed during an actual turn repaired an injured unit only at the following end-phase.".to_owned()));
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
            let (road, coworkers, precompleted, promotion) =
                sim.activity_completion_order_playtest();
            for (name, passed) in [
                ("native_per_unit_road_completion", road),
                ("native_cooperative_work_early_stop", coworkers),
                ("native_precompleted_exhausted_work", precompleted),
                ("native_worker_promotion_draw_order", promotion),
            ] {
                self.results.push((name, passed, "Work completion applied before later units refilled movement or progressed the same project; promotion followed native worker eligibility and random draws.".to_owned()));
            }
        }

        if frame == 760 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (requirements, combat, upkeep, flags) = sim.native_roster_actions_playtest();
            for (name, passed) in [
                ("native_roster_requirements_and_obsolescence", requirements),
                ("native_roster_combat_classes", combat),
                ("native_fanatic_upkeep_and_join_city", upkeep),
                ("native_roster_action_flags", flags),
            ] {
                self.results.push((name, passed, "The extended unit roster used reference requirements, upgrade chains, combat classes, action flags and government upkeep exemptions.".to_owned()));
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
            let (sprites, distinct, labels) = crate::client::native_unit_rendering_playtest();
            for (name, passed) in [
                ("native_complete_roster_sprites", sprites),
                ("native_added_unit_sprite_distinction", distinct),
                ("native_added_unit_labels", labels),
            ] {
                self.results.push((name, passed, "All 57 unit sprites were in atlas bounds with visible pixels; added units had distinct native atlas cells and labels.".to_owned()));
            }
        }

        if frame == 769 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("native_all_unit_stats_source_snapshot", sim.native_roster_stats_oracle(),
                "All eight numeric statistics for every one of 57 unit types matched an independently extracted reference ruleset snapshot.".to_owned()));
        }

        if frame == 772 && self.act_passed {
            let sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (work, rejected) = sim.transport_worker_actions_playtest();
            self.results.push((
                "native_transport_oil_platform_and_cleanup",
                work,
                "A sea Transport performed the source-allowed oil platform and cleanup actions."
                    .to_owned(),
            ));
            self.results.push((
                "native_transport_land_action_restrictions",
                rejected,
                "Sea workers rejected land-only work even at a port.".to_owned(),
            ));
            self.results.push(("native_transport_automatic_ocean_cleanup", sim.transport_auto_worker_playtest(),
                "An automated Transport navigated to a worked polluted ocean tile and completed cleanup.".to_owned()));
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
            *game = Game::new(crate::mapgen::generate(42));
            let id = game.sim.transport_hud_playtest_setup();
            game.perform_action(UiAction::SelectUnit(id));
        }
    }
}
