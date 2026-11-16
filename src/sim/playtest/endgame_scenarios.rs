//! Automated gameplay scenarios covering endgame behavior.

use super::*;

impl Simulation {
    pub(crate) fn multicultural_playtest_setup(&mut self) -> (u32, u32) {
        let home = self.cities_owned_by(HUMAN).next().unwrap();
        let (home_x, home_y) = (home.x, home.y);
        let (approach_x, approach_y, target_x, target_y) = (0..self.tiles.len())
            .map(|index| self.map_wrap.position(index))
            .filter(|&(x, y)| self.map_distance(home_x, home_y, x, y) == 4)
            .find_map(|(approach_x, approach_y)| {
                let (target_x, target_y) = offset_native_by_map(approach_x, approach_y, 1, 0);
                ((0..self.map_wrap.height()).contains(&target_y)
                    && self.map_distance(home_x, home_y, target_x, target_y) == 5
                    && self.can_found_at(target_x, target_y))
                .then_some((approach_x, approach_y, target_x, target_y))
            })
            .expect("multicultural playtest needs a valid adjacent approach and city site");
        for (x, y) in [(approach_x, approach_y), (target_x, target_y)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.tile_mut(x, y).hut = false;
        }
        self.create_city(1, target_x, target_y);
        let city = self.cities.last_mut().unwrap();
        city.size = 3;
        let city_id = city.id;
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, approach_x, approach_y);
        (unit, city_id)
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
}
