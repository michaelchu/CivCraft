//! Focused gameplay scenarios for the parent subsystem.

use super::*;
use std::ops::{Deref, DerefMut};

/// Mutable campaign access reserved for deterministic gameplay automation.
///
/// Keeping this token distinct from `ClientSimulation` prevents production
/// client code from treating campaign entities as its normal read API.
pub(crate) struct PlaytestHarness<'a> {
    simulation: &'a mut Simulation,
}

impl<'a> PlaytestHarness<'a> {
    pub(crate) fn new(simulation: &'a mut Simulation) -> Self {
        Self { simulation }
    }
}

impl Deref for PlaytestHarness<'_> {
    type Target = Simulation;

    fn deref(&self) -> &Self::Target {
        self.simulation
    }
}

impl DerefMut for PlaytestHarness<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.simulation
    }
}

/// Read-only campaign inspection reserved for automated assertions.
pub(crate) struct PlaytestState<'a> {
    simulation: &'a Simulation,
}

impl<'a> PlaytestState<'a> {
    pub(crate) fn new(simulation: &'a Simulation) -> Self {
        Self { simulation }
    }
}

impl<'a> PlaytestState<'a> {
    pub(crate) fn units(&self) -> &'a [Unit] {
        &self.simulation.units
    }

    pub(crate) fn cities(&self) -> &'a [City] {
        &self.simulation.cities
    }

    pub(crate) fn players(&self) -> &'a [Player] {
        &self.simulation.players
    }

    pub(crate) fn tile(&self, x: i32, y: i32) -> &'a Tile {
        self.simulation.tile(x, y)
    }

    pub(crate) fn events(&self) -> &'a [String] {
        self.simulation.events()
    }

    pub(crate) fn game_over(&self) -> Option<&'a str> {
        self.simulation.game_over()
    }

    pub(crate) fn turn(&self) -> i32 {
        self.simulation.turn()
    }

    pub(crate) fn map_distance(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        self.simulation.map_distance(x1, y1, x2, y2)
    }

    pub(crate) fn pact(&self, first: usize, second: usize) -> Pact {
        self.simulation.pact(first, second)
    }

    pub(crate) fn pact_turns(&self, first: usize, second: usize) -> i32 {
        self.simulation.pact_turns(first, second)
    }

    pub(crate) fn pact_offer(&self, owner: usize) -> Option<Pact> {
        self.simulation.pact_offer(owner)
    }

    pub(crate) fn treaty_offer(&self, owner: usize) -> Option<TreatyOffer> {
        self.simulation.treaty_offer(owner)
    }

    pub(crate) fn city_yields(&self, city: &City) -> (i32, i32, i32) {
        self.simulation.city_yields(city)
    }

    pub(crate) fn city_trade(&self, city: &City, trade: i32) -> [i32; 3] {
        self.simulation.city_trade(city, trade)
    }

    pub(crate) fn building_upkeep(&self, city: &City) -> i32 {
        self.simulation.building_upkeep(city)
    }

    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
        self.simulation.city_in_disorder(city, luxury)
    }

    pub(crate) fn city_unit_upkeep(&self, city: &City) -> super::upkeep::UnitUpkeep {
        self.simulation.city_unit_upkeep(city)
    }

    pub(crate) fn cargo_count(&self, carrier: u32) -> usize {
        self.simulation.cargo_count(carrier)
    }

    pub(crate) fn project_turns_left(&self, unit: &Unit) -> i32 {
        self.simulation.project_turns_left(unit)
    }

    pub(crate) fn can_begin_paradrop(&self, unit: &Unit) -> bool {
        self.simulation.can_begin_paradrop(unit)
    }

    pub(crate) fn ship_assembly(&self) -> ([usize; 3], [bool; 32], [usize; 2], [usize; 3], bool) {
        self.simulation.ship_assembly()
    }

    pub(crate) fn spaceship_status(&self, owner: usize) -> Option<String> {
        self.simulation.spaceship_status(owner)
    }

    pub(crate) fn score_standings(&self) -> Vec<(usize, i32)> {
        self.simulation.score_standings()
    }
}

mod ai_scenarios;
mod aviation_scenarios;
mod border_scenarios;
#[cfg(test)]
mod campaign_turn_scenarios;
mod city_economy_scenarios;
mod combat_scenarios;
mod culture_scenarios;
mod diplomacy_scenarios;
mod endgame_scenarios;
mod list_order_scenarios;
mod military_ai;
mod movement_scenarios;
mod naval_trade_scenarios;
mod research_scenarios;
mod settlement_scenarios;
mod turn_scenarios;
mod unit_action_scenarios;
mod unit_catalog_scenarios;
mod wonder_scenarios;
mod world_events_scenarios;
#[cfg(test)]
mod ai_integration_tests {
    use super::*;

    #[test]
    fn nested_transport_rescue_matches_selected_helicopter_cargo_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.nested_cargo_rescue_playtest(), (true, true, true));
    }

    #[test]
    fn city_healing_uses_civ3_restore_preconditions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.city_healing_playtest());
    }

    #[test]
    fn settlement_ferry_fixture_isolates_and_completes_the_overseas_sequence() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.settlement_ferry_production_playtest(),
            (true, true, true)
        );
    }

    #[test]
    fn despotism_upkeep_playtest_charges_gold_in_isolated_state() {
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
        assert!(isolated.despotism_upkeep_playtest());
        assert_eq!(game.turn, source_turn);
        assert_eq!(game.units.len(), source_units);
    }

    #[test]
    fn government_ui_playtest_setup_supports_a_valid_unlocked_transition() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.government_ui_playtest_setup();

        assert_eq!(game.players[HUMAN].government, Government::Republic);
        assert!(!game.apply(Command::Government(Government::Monarchy)));
        assert!(game.apply(Command::Government(Government::Despotism)));
        assert_eq!(game.players[HUMAN].government, Government::Anarchy);
        assert_eq!(
            game.players[HUMAN].revolution_target,
            Some(Government::Despotism)
        );
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
    fn base_building_fixture_creates_its_own_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let worker = game.base_playtest_setup();
        assert!(game.cities_owned_by(HUMAN).next().is_some());
        assert_eq!(game.unit_by_id(worker).unwrap().kind, UnitKind::Worker);
    }
    #[test]
    fn expansion_playtest_uses_valid_polar_rows_and_founds_the_fourth_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_expansion_playtest(), (true, true, true));
    }
    #[test]
    fn uprising_playtest_targets_the_active_map_topology() {
        for (sea, advanced) in [(false, false), (true, false), (true, true)] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            let passed = game.uprising_playtest(sea, advanced);
            let roster = game
                .units
                .iter()
                .map(|unit| (unit.kind, unit.transport))
                .collect::<Vec<_>>();
            assert!(passed, "sea={sea} advanced={advanced} roster={roster:?}");
        }
    }

    #[test]
    fn selected_c2c3_obsolete_wonder_requirements_use_the_civ3_research_tree() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(settler)));
        assert_eq!(game.obsolete_wonders_playtest(), (true, true, true));
    }

    #[test]
    fn nuclear_ai_role_scenario_matches_the_selected_c2c3_record() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.nuclear_ai_role_playtest());
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
    fn government_scenario_checks_research_revolution_choice_and_completion() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (research, locked, chose, completed) = game.ai_government_playtest();
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
        let (inland, coastal, researched) = game.ferry_research_playtest();
        assert!(inland, "inland city must not demand ferry research");
        assert!(coastal, "coastal city must demand ferry research");
        assert!(researched, "ferry technology research completes");
    }

    #[test]
    fn civilian_upgrade_scenario_has_no_paid_worker_or_settler_successor() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.civilian_upgrade_roster_playtest());
    }

    #[test]
    fn city_defense_scenario_checks_source_defender_and_urgent_response() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (source_defender, _safe_city_choice, urgent_response) = game.ai_city_defense_playtest();
        assert!(source_defender, "source defender selection");
        assert!(urgent_response, "urgent defense response");
    }

    #[test]
    fn threat_route_scenario_checks_hostile_city_entry_cost() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.ai_threat_route_cost_playtest());
    }

    #[test]
    fn ground_invasion_scenario_checks_a_distant_assigned_occupation_force() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.ai_ground_invasion_playtest());
    }

    #[test]
    fn aircraft_scenario_checks_fuel_range_and_oil_requirement() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        assert!(game.ai_aircraft_advisor_playtest());
    }

    #[test]
    fn fighter_scenario_checks_no_interception_range_limit_and_refuelling() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (no_interception, range_limit, refuelled) = game.ai_fighter_playtest();
        assert!(no_interception, "no unsupported interception");
        assert!(
            range_limit,
            "distant target remains outside operational range"
        );
        assert!(refuelled, "aircraft refuels at its base");
    }
}
