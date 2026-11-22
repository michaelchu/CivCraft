//! Ordinary Rust unit tests and private-rule fixtures.
//! This module is compiled only for cargo test; runnable scenarios live in crate::scenarios.

use super::*;

mod ai_tests;
mod aviation_tests;
mod border_tests;
#[cfg(test)]
mod campaign_turn_tests;
mod city_economy_tests;
mod combat_tests;
mod diplomacy_tests;
mod endgame_tests;
mod list_order_tests;
mod military_ai;
mod movement_tests;
mod naval_trade_tests;
mod research_tests;
mod settlement_tests;
mod turn_tests;
mod unit_action_tests;
mod unit_catalog_tests;
mod wonder_tests;
mod world_events_tests;
#[cfg(test)]
mod ai_integration_tests {
    use super::*;

    #[test]
    fn nested_transport_rescue_matches_selected_helicopter_cargo_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.nested_cargo_rescue_test(), (true, true, true));
    }

    #[test]
    fn despotism_upkeep_test_charges_gold_in_isolated_state() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(settler)));

        let source_turn = game.turn;
        let source_units = game.units.len();
        let mut isolated = game.clone();
        assert!(isolated.despotism_upkeep_test());
        assert_eq!(game.turn, source_turn);
        assert_eq!(game.units.len(), source_units);
    }

    #[test]
    fn government_ui_fixture_supports_a_valid_unlocked_transition() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.government_ui_fixture();

        assert_eq!(game.players[HUMAN].government, Government::REPUBLIC);
        assert!(!game.apply(Command::Government(Government::MONARCHY)));
        assert!(game.apply(Command::Government(Government::DESPOTISM)));
        assert_eq!(game.players[HUMAN].government, Government::ANARCHY);
        assert_eq!(
            game.players[HUMAN].revolution_target,
            Some(Government::DESPOTISM)
        );
    }

    #[test]
    fn native_seeds_preserve_sam_plague_and_disaster_scenario_preconditions() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let mut defense = Vec::new();
        let mut sizes = Vec::new();
        for protected in [false, true] {
            let mut game = fresh();
            let (bomber, defender) = game.sam_battery_fixture(protected);
            let attacker = game.unit_by_id(bomber).unwrap().clone();
            let defender = game.unit_by_id(defender).unwrap().clone();
            defense.push(game.defense_power_against(
                &defender,
                defender.x,
                defender.y,
                attacker.kind,
            ));
            let mut game = fresh();
            let settler = game
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
                .unwrap()
                .clone();
            game.create_city(HUMAN, settler.x, settler.y);
            sizes.push(game.run_plague_test(protected));
        }
        assert!(
            defense[1] > defense[0],
            "SAM Battery should increase defense against aircraft"
        );
        assert_eq!(sizes, [8, 9]);
        for nuclear in [false, true] {
            let mut game = fresh();
            let settler = game
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
                .unwrap()
                .clone();
            game.create_city(HUMAN, settler.x, settler.y);
            assert!(game.run_disaster_test(nuclear));
        }
    }
    #[test]
    fn combat_population_protection_and_native_worker_rearrangement() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.combat_population_test());
    }
    #[test]
    fn fortifications_follow_native_urgency_and_land_sea_effect_classes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_fortifications_test(), (true, true, true));
    }
    #[test]
    fn base_building_fixture_creates_its_own_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let worker = game.base_fixture();
        assert!(game.cities_owned_by(HUMAN).next().is_some());
        assert_eq!(game.unit_by_id(worker).unwrap().kind, UnitKind::Worker);
    }
    #[test]
    fn expansion_test_uses_valid_polar_rows_and_founds_the_fourth_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_expansion_test(), (true, true, true));
    }
    #[test]
    fn uprising_test_targets_the_active_map_topology() {
        for (sea, advanced) in [(false, false), (true, false), (true, true)] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            let passed = game.uprising_test(sea, advanced);
            let roster = game
                .units
                .iter()
                .map(|unit| (unit.kind, unit.transport))
                .collect::<Vec<_>>();
            assert!(passed, "sea={sea} advanced={advanced} roster={roster:?}");
        }
    }

    /// Mirrors the civ2civ3 ruleset and Civ III behavior.
    #[test]
    fn selected_ruleset_obsolete_wonder_requirements_use_the_native_research_tree() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(settler)));
        assert_eq!(game.obsolete_wonders_test(), (true, true, true));
    }

    /// Mirrors the civ2civ3 ruleset behavior.
    #[test]
    fn nuclear_ai_role_scenario_matches_the_selected_ruleset_record() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.nuclear_ai_role_test());
    }
    #[test]
    fn shakespeares_theater_trade_effect_is_local_and_enduring() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let id = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(id)));
        assert!(game.theatre_output_test());
    }

    #[test]
    fn government_scenario_checks_research_revolution_choice_and_completion() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (research, locked, chose, completed) = game.ai_government_test();
        assert!(research, "government research goal");
        assert!(
            locked,
            "Monarchy stays locked until its technology is known"
        );
        assert!(chose, "government chosen");
        assert!(completed, "revolution completes and keeps Monarchy");
    }

    #[test]
    fn ferry_research_scenario_checks_inland_coastal_and_completed_research() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (inland, coastal, researched) = game.ferry_research_test();
        assert!(inland, "inland city must not demand ferry research");
        assert!(coastal, "coastal city must demand ferry research");
        assert!(researched, "ferry technology research completes");
    }

    #[test]
    fn civilian_upgrade_scenario_has_no_paid_worker_or_settler_successor() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.civilian_upgrade_roster_test());
    }

    #[test]
    fn city_defense_scenario_checks_source_defender_and_urgent_response() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (source_defender, _safe_city_choice, urgent_response) = game.ai_city_defense_test();
        assert!(source_defender, "source defender selection");
        assert!(urgent_response, "urgent defense response");
    }

    #[test]
    fn threat_route_scenario_checks_hostile_city_entry_cost() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.ai_threat_route_cost_test());
    }

    #[test]
    fn ground_invasion_scenario_checks_a_distant_assigned_occupation_force() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.ai_ground_invasion_test());
    }

    #[test]
    fn aircraft_scenario_checks_fuel_range_and_oil_requirement() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.ai_aircraft_advisor_test());
    }

    #[test]
    fn fighter_scenario_checks_no_interception_range_limit_and_refuelling() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (no_interception, range_limit, refuelled) = game.ai_fighter_test();
        assert!(no_interception, "no unsupported interception");
        assert!(
            range_limit,
            "distant target remains outside operational range"
        );
        assert!(refuelled, "aircraft refuels at its base");
    }
}
