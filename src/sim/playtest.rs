//! Focused gameplay scenarios for the parent subsystem.

use super::*;

mod ai_scenarios;
mod aviation_scenarios;
mod border_scenarios;
mod city_economy_scenarios;
mod combat_scenarios;
mod diplomacy_scenarios;
mod endgame_scenarios;
mod espionage_scenarios;
mod list_order_scenarios;
mod military_ai;
mod movement_scenarios;
mod naval_trade_scenarios;
mod research_scenarios;
mod settlement_scenarios;
mod treaty_scenarios;
mod turn_scenarios;
mod unit_action_scenarios;
mod unit_catalog_scenarios;
mod wonder_scenarios;
mod world_events_scenarios;
#[cfg(test)]
mod ai_integration_tests {
    use super::*;
    #[test]
    fn actual_end_turn_heals_before_new_city_improvements() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_turn_orchestration_playtest());
    }
    #[test]
    fn native_seeds_preserve_sam_plague_and_disaster_scenario_preconditions() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let mut hp = Vec::new();
        let mut sizes = Vec::new();
        for protected in [false, true] {
            let mut game = fresh();
            let (bomber, defender) = game.sam_battery_playtest_setup(protected);
            assert!(game.apply(Command::Move(bomber, 1, 0)));
            hp.push(
                game.units
                    .iter()
                    .find(|unit| unit.id == defender)
                    .unwrap()
                    .hp,
            );
            let mut game = fresh();
            let settler = game
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
                .unwrap()
                .clone();
            game.create_city(HUMAN, settler.x, settler.y);
            sizes.push(game.run_plague_playtest(protected));
        }
        assert_eq!(hp, [17, 20]);
        assert_eq!(sizes, [8, 9]);
        for nuclear in [false, true] {
            let mut game = fresh();
            let settler = game
                .units
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
                .unwrap()
                .clone();
            game.create_city(HUMAN, settler.x, settler.y);
            assert!(game.run_disaster_playtest(nuclear));
        }
    }
    #[test]
    fn combat_population_protection_and_native_worker_rearrangement() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.combat_population_playtest());
    }
    #[test]
    fn fortifications_follow_native_urgency_and_land_sea_effect_classes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_fortifications_playtest(), (true, true, true));
    }
    #[test]
    fn counterintelligence_fixture_uses_prior_phase_research_wants() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_counterintelligence_playtest());
    }

    #[test]
    fn crusade_relieves_one_raw_military_pressure_and_gold_upkeep() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let id = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(id)));
        assert_eq!(game.theatre_and_crusade_playtest(), (true, true, true));
    }

    #[test]
    fn integrated_advisor_scenarios() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let bribery = fresh().ai_bribery_playtest();
        let government = fresh().ai_government_playtest();
        let ferry = fresh().ferry_research_playtest();
        assert!(bribery);
        assert_eq!(government, (true, true, true, true));
        assert_eq!(ferry, (true, true, true));
        assert!(fresh().ai_civilian_upgrade_playtest());
        assert!(fresh().ai_production_priorities_playtest());
        assert!(fresh().ai_trade_production_gate_playtest());
        assert_eq!(fresh().ai_city_defense_playtest(), (true, true, true));
        assert!(fresh().ai_threat_route_cost_playtest());
        assert!(fresh().ai_ground_invasion_playtest());
        assert!(fresh().ai_aircraft_advisor_playtest());
        assert!(fresh().ai_nuclear_advisor_playtest());
        assert_eq!(fresh().ai_fighter_playtest(), (true, true, true));
    }
}
