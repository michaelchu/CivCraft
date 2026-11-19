//! Unit checks and fixtures for private simulation behavior.

use super::*;

impl Simulation {
    pub(crate) fn government_ui_fixture(&mut self) {
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

    pub(crate) fn despotism_upkeep_test(&mut self) -> bool {
        // Conquests charges excess unit support in gold. Run this in an
        // isolated unit tests copy so fixture units and turn advancement do not
        // alter the UI review campaign.
        self.players[HUMAN].government = Government::Despotism;
        self.players[HUMAN].revolution_target = None;
        self.players[HUMAN].revolution_turn = None;
        let Some(city) = self.cities.iter().find(|city| city.owner == HUMAN) else {
            return false;
        };
        let (city_id, x, y) = (city.id, city.x, city.y);
        // Isolate the unit-support check from the opening UI city's current
        // population and specialist mix; a size-four city can represent the
        // C2C3 military-content rules without falling into disorder.
        if let Some(city) = self.cities.iter_mut().find(|city| city.id == city_id) {
            city.size = 4;
            city.specialists = [0; 3];
            city.food = 0;
            city.was_happy = false;
        }
        self.reconcile_city_workers();
        self.players[HUMAN].gold = 100;

        self.units.retain(|unit| unit.owner != HUMAN);
        let supported_units: Vec<u32> = (0..5)
            .map(|_| self.spawn_unit(HUMAN, UnitKind::Warrior, x, y))
            .collect();
        for unit_id in supported_units.iter().rev().take(3) {
            if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == *unit_id) {
                unit.home_city = Some(city_id);
                unit.home_order = unit.id;
            }
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
