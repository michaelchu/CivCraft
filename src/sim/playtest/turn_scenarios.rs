//! Automated gameplay scenarios covering turn behavior.

use super::*;

impl Simulation {
    pub(crate) fn city_healing_playtest(&mut self) -> bool {
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
        self.players[HUMAN].known = vec![Tech::BRONZE_WORKING, Tech::WARRIOR_CODE];
        self.create_city(HUMAN, 10, 24);
        self.cities[0].buildings.push(Building::BARRACKS);
        let soldier = self.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        self.units.last_mut().unwrap().hp = 1;
        self.restore_player_units(HUMAN);
        let barracks_healed = self
            .unit_by_id(soldier)
            .is_some_and(|unit| unit.hp == unit.max_hit_points());
        self.cities[0]
            .buildings
            .retain(|&building| building != Building::BARRACKS);
        let unit = self
            .units
            .iter_mut()
            .find(|unit| unit.id == soldier)
            .unwrap();
        unit.hp = 1;
        unit.moved = false;
        let expected = unit.hp + Self::hitpoint_gain(unit, true);
        self.restore_player_units(HUMAN);
        barracks_healed
            && self
                .unit_by_id(soldier)
                .is_some_and(|unit| unit.hp == expected)
    }

    pub(crate) fn tribal_upkeep_playtest_setup(&mut self) {
        // This scenario specifically exercises shield upkeep. New games use
        // the ruleset's Despotism default, which converts eligible upkeep.
        self.players[HUMAN].government = Government::Despotism;
        self.players[HUMAN].revolution_target = None;
        self.players[HUMAN].revolution_turn = None;
        let Some(city) = self.cities.iter().find(|city| city.owner == HUMAN) else {
            return;
        };
        let (city_id, x, y) = (city.id, city.x, city.y);
        // Keep this scenario independent of whether the generated opening map
        // happened to produce a third unit before the upkeep checkpoint.
        for _ in 0..3 {
            let city = self.cities.iter().find(|city| city.id == city_id).unwrap();
            if self.city_unit_upkeep(city).shields >= 1 {
                break;
            }
            let id = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city_id);
            unit.home_order = id;
        }
        self.reconcile_city_workers();
    }
}
