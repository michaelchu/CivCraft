//! Focused gameplay scenarios for the parent subsystem.

use super::*;

mod ai_scenarios;
mod aviation_scenarios;
mod border_scenarios;
mod city_economy_scenarios;
mod combat_scenarios;
mod diplomacy_scenarios;
mod endgame_scenarios;
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
    fn city_healing_uses_civ3_restore_preconditions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.city_healing_playtest());
    }
    #[test]
    fn native_seeds_preserve_sam_plague_and_disaster_scenario_preconditions() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let mut defense = Vec::new();
        let mut sizes = Vec::new();
        for protected in [false, true] {
            let mut game = fresh();
            let (bomber, defender) = game.sam_battery_playtest_setup(protected);
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
            sizes.push(game.run_plague_playtest(protected));
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
    fn shakespeares_theater_trade_effect_is_local_and_enduring() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let id = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(id)));
        assert!(game.theatre_output_playtest());
    }

    #[test]
    fn integrated_advisor_scenarios() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        let government = fresh().ai_government_playtest();
        let ferry = fresh().ferry_research_playtest();
        assert_eq!(government, (true, true, true, true));
        assert_eq!(ferry, (true, true, true));
        assert!(fresh().civilian_upgrade_roster_playtest());
        assert!(fresh().ai_growth_expansion_playtest());
        let (source_defender, _safe_city_choice, urgent_response) =
            fresh().ai_city_defense_playtest();
        assert!(source_defender && urgent_response);
        assert!(fresh().ai_threat_route_cost_playtest());
        assert!(fresh().ai_ground_invasion_playtest());
        assert!(fresh().ai_aircraft_advisor_playtest());
        // Nuclear legality and imported Tactical Nuke gates have dedicated
        // mechanics tests; target/launch ranking is not a manual-defined rule.
        assert_eq!(
            UnitKind::TacticalNuke.required_tech(),
            Some(Tech::SPACE_FLIGHT)
        );
        assert_eq!(
            UnitKind::TacticalNuke.required_resources(),
            vec![Resource::Aluminum, Resource::Uranium]
        );
        assert_eq!(fresh().ai_fighter_playtest(), (true, true, true));
    }
}
