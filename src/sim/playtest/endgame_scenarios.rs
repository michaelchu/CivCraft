//! Automated gameplay scenarios covering endgame behavior.

use super::*;

impl Simulation {
    pub(crate) fn science_wonder_playtest(&mut self) -> (bool, i32, i32) {
        let home = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        let (x, y) = (self.cities[home].x, self.cities[home].y);
        let (nx, ny) = offset_native_by_map(x, y, 5, 0);
        self.create_city(1, nx, ny);
        let rival = self.cities.len() - 1;
        self.cities[home].buildings.push(Building::LIBRARY);
        self.cities[rival].buildings.push(Building::LIBRARY);
        let before = self.players[HUMAN].known.len();
        self.finish_building(self.cities[home].id, HUMAN, Building::GREAT_LIBRARY, 300);
        (
            self.players[HUMAN].known.len() == before + 1,
            self.split_city_trade(&self.cities[home], 10, [100, 0, 0])[0],
            self.split_city_trade(&self.cities[rival], 10, [100, 0, 0])[0],
        )
    }

    pub(crate) fn amphitheater_playtest(&mut self) -> (bool, bool, i32) {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        self.players[HUMAN].government = Government::Republic;
        self.cities[index].size = 8;
        self.cities[index].specialists = [0; 3];
        self.cities[index].buildings.push(Building::AMPHITHEATER);
        let before = self.city_mood(&self.cities[index], 0).0;
        self.players[HUMAN].known.push(Tech::ELECTRICITY);
        let after = self.city_mood(&self.cities[index], 0).0;
        let history = Building::AMPHITHEATER.history_per_turn();
        (before, after, history)
    }

    pub(crate) fn partisan_playtest_setup(&mut self) -> (u32, u32) {
        let (unit, city) = self.multicultural_playtest_setup();
        self.players[1].government = Government::Communism;
        self.players[1]
            .known
            .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
        self.players[HUMAN].known.push(Tech::GUERILLA_WARFARE);
        let city_info = self.city_by_id(city).unwrap();
        for (_, _, x, y) in Self::city_radius(city_info) {
            self.tile_mut(x, y).terrain = Terrain::Hills;
        }
        (unit, city)
    }

    pub(crate) fn multicultural_playtest_setup(&mut self) -> (u32, u32) {
        let home = self.cities_owned_by(HUMAN).next().unwrap();
        let approach = offset_native_by_map(home.x, home.y, 4, 0);
        let target = offset_native_by_map(home.x, home.y, 5, 0);
        for (x, y) in [approach, target] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.tile_mut(x, y).hut = false;
        }
        self.create_city(1, target.0, target.1);
        let city = self.cities.last_mut().unwrap();
        city.size = 3;
        let city_id = city.id;
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, approach.0, approach.1);
        (unit, city_id)
    }

    pub(crate) fn achievement_playtest_setup(&mut self) -> i32 {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::APOLLO_PROGRAM, 600);
        let before = self.civilization_score(HUMAN);
        self.check_achievements();
        let scored = self.civilization_score(HUMAN);
        self.check_achievements();
        if self.civilization_score(HUMAN) == scored {
            scored - before
        } else {
            0
        }
    }

    pub(crate) fn turn_limit_playtest_setup(&mut self) {
        self.turn = super::score::TURN_LIMIT;
        self.nation_scores[HUMAN].units_built = 1000;
    }

    pub(crate) fn manual_spaceship_playtest_build_structural(&mut self) {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::SPACE_STRUCTURAL, 150);
    }

    pub(crate) fn spaceship_playtest_setup(&mut self) {
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .id;
        self.finish_building(city, HUMAN, Building::APOLLO_PROGRAM, 600);
        for (building, count) in [
            (Building::SPACE_MODULE, 3),
            (Building::SPACE_COMPONENT, 2),
            (Building::SPACE_STRUCTURAL, 8),
        ] {
            for _ in 0..count {
                self.finish_building(city, HUMAN, building, building.data().cost);
            }
        }
        self.year = 1880;
    }

    pub(crate) fn spaceship_playtest_finish_flight(&mut self) {
        for _ in 0..30 {
            if !self.apply(Command::EndTurn) {
                break;
            }
        }
    }
}
