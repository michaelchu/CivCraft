//! Automated gameplay scenarios covering turn behavior.

use super::*;

impl Simulation {
    pub(crate) fn government_ui_playtest_setup(&mut self) {
        // Start from an unlocked government so the UI scenario can exercise
        // a real change to Despotism while Monarchy remains tech-locked.
        let player = &mut self.players[HUMAN];
        player.government = Government::Republic;
        if !player.known.contains(&Tech::THE_REPUBLIC) {
            player.known.push(Tech::THE_REPUBLIC);
        }
        player.known.retain(|&tech| tech != Tech::MONARCHY);
        player.revolution_target = None;
        player.revolution_turn = None;
    }

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
        let unit = unit.clone();
        let expected = unit.hp + self.unit_hitpoint_gain_at(&unit, unit.x, unit.y);
        self.restore_player_units(HUMAN);
        barracks_healed
            && self
                .unit_by_id(soldier)
                .is_some_and(|unit| unit.hp == expected)
    }

    pub(crate) fn despotism_upkeep_playtest(&mut self) -> bool {
        // Conquests charges excess unit support in gold. Run this in an
        // isolated playtest copy so fixture units and turn advancement do not
        // alter the UI review campaign.
        self.players[HUMAN].government = Government::Despotism;
        self.players[HUMAN].revolution_target = None;
        self.players[HUMAN].revolution_turn = None;
        let Some(city) = self.cities.iter().find(|city| city.owner == HUMAN) else {
            return false;
        };
        let (city_id, x, y) = (city.id, city.x, city.y);
        self.players[HUMAN].gold = 100;

        let mut supported_units: Vec<u32> = self
            .units
            .iter()
            .filter(|unit| unit.owner == HUMAN && unit.kind.requires_support())
            .map(|unit| unit.id)
            .collect();
        while supported_units.len() < 5 {
            supported_units.push(self.spawn_unit(HUMAN, UnitKind::Warrior, x, y));
        }
        supported_units.sort_unstable();

        for unit in self
            .units
            .iter_mut()
            .filter(|unit| unit.owner == HUMAN && unit.kind.requires_support())
        {
            unit.home_city = None;
            unit.home_order = unit.id;
        }
        if let Some(unit) = self
            .units
            .iter_mut()
            .find(|unit| unit.id == supported_units[4])
        {
            unit.home_city = Some(city_id);
        }
        self.reconcile_city_workers();

        let city = self.cities.iter().find(|city| city.id == city_id).unwrap();
        let upkeep = self.city_unit_upkeep(city);
        let (_, _, trade) = self.city_yields(city);
        let [_, gold, luxury] = self.city_trade(city, trade);
        let disorder = self.city_in_disorder(city, luxury);
        if upkeep.gold != 1 || upkeep.shields != 0 || disorder {
            return false;
        }

        let revenue = if disorder { 0 } else { gold };
        let expected_treasury =
            self.players[HUMAN].gold + revenue - self.building_upkeep(city) - upkeep.gold;
        self.apply(super::Command::EndTurn)
            && self.players[HUMAN].gold == expected_treasury
            && self.cities.iter().any(|city| city.id == city_id)
    }
}
