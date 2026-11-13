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
    fn city_healing_uses_civ3_restore_preconditions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.city_healing_playtest());
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
        for sea in [false, true] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            assert!(game.uprising_playtest(sea, false), "sea={sea}");
        }
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
