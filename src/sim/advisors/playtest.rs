//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    pub(crate) fn ai_happiness_effects_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.completed_wonders.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
        }
        self.create_city(1, 10, 24);
        self.cities[0].size = 5;
        self.players[1].government = Government::Monarchy;
        self.players[1].known.push(Tech::MYSTICISM);
        let independent = self.building_happiness_want(&self.cities[0], Building::TEMPLE) > 0.0;
        self.cities[0].size = 4;
        self.players[1].government = Government::Republic;
        let id = self.cities[0].id;
        let field = offset_native_by_map(10, 24, 8, 0);
        for _ in 0..4 {
            self.spawn_unit(1, UnitKind::Warrior, field.0, field.1);
            self.units.last_mut().unwrap().home_city = Some(id);
        }
        let wartime = self.building_happiness_want(&self.cities[0], Building::TEMPLE) == 0.0
            && self.building_happiness_want(&self.cities[0], Building::STATUE_OF_ZEUS) > 0.0
            && self.building_happiness_want(&self.cities[0], Building::POLICE_STATION) > 0.0;
        self.players[1].government = Government::Democracy;
        self.cities[0].buildings.push(Building::POLICE_STATION);
        let removal =
            self.building_happiness_want(&self.cities[0], Building::WOMENS_SUFFRAGE) == 0.0;
        self.units.clear();
        self.cities[0].buildings.clear();
        self.cities[0].size = 1;
        self.players[1].government = Government::Monarchy;
        for step in [4, 8] {
            let (x, y) = offset_native_by_map(10, 24, step, 0);
            self.create_city(1, x, y);
            self.cities.last_mut().unwrap().size = 10;
        }
        let ranged = self.building_happiness_want(&self.cities[0], Building::HANGING_GARDENS) > 0.0
            && self.building_happiness_want(&self.cities[0], Building::MICHELANGELOS_CHAPEL) == 0.0;
        independent && wartime && removal && ranged
    }

    #[cfg(test)]
    pub(crate) fn ai_wonder_advisor_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.completed_wonders.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        self.players[1].known = self.research_path(1, Tech::LITERATURE);
        self.players[1]
            .known
            .extend([Tech::ECONOMICS, Tech::POTTERY]);
        self.players[1].government = Government::Monarchy;
        self.players[1].rates = [80, 20, 0];
        for (dx, dy) in [(0, 0), (3, 0), (6, 0), (0, 3), (3, 3), (6, 3)] {
            let (x, y) = offset_native_by_map(10, 24, dx, dy);
            self.create_city(1, x, y);
            let city = self.cities.last_mut().unwrap();
            city.size = 5;
            city.buildings.extend([Building::LIBRARY, Building::TEMPLE]);
            self.spawn_unit(1, UnitKind::Warrior, x, y);
        }
        self.reconcile_city_workers();
        let Some(id) = self.ai_wonder_city(1) else {
            return false;
        };
        let city = self
            .cities
            .iter()
            .find(|city| city.id == id)
            .unwrap()
            .clone();
        let Some(choice) = self.wonder_build_choice(&city) else {
            return false;
        };
        let Production::Building(_) = choice.production else {
            return false;
        };
        let chosen = self.choose_ai_production(1, id) == choice.production;
        self.change_production(id, choice.production);
        chosen
    }
}
