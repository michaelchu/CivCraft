//! Automated gameplay scenarios covering turn behavior.

use super::*;

impl Simulation {
    pub(crate) fn native_turn_orchestration_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
        self.create_city(HUMAN, 10, 24);
        // Keep a rival alive so the first end-turn does not end the game.
        self.create_city(1, 22, 40);
        self.cities[0].production = Production::Building(Building::BARRACKS);
        self.cities[0].shields = self.cities[0].production.cost();
        let soldier = self.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        self.units.last_mut().unwrap().hp = 1;
        self.end_turn();
        let built_after_healing = self.cities[0].has_building(Building::BARRACKS)
            && self
                .units
                .iter()
                .find(|unit| unit.id == soldier)
                .is_some_and(|unit| unit.hp == 5);
        self.end_turn();
        built_after_healing
            && self
                .units
                .iter()
                .find(|unit| unit.id == soldier)
                .is_some_and(|unit| unit.hp == UnitKind::Warrior.stats().hp)
    }

    pub(crate) fn tribal_upkeep_playtest_setup(&mut self) {
        // This scenario specifically exercises shield upkeep. New games use
        // the ruleset's Despotism default, which converts eligible upkeep.
        self.players[HUMAN].government = Government::Tribal;
        self.players[HUMAN].revolution_target = None;
        self.players[HUMAN].revolution_turn = None;
        self.reconcile_city_workers();
    }
}
