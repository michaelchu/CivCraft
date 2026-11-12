//! Pollution, contamination, and cleanup rules.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum Contamination {
    Pollution,
    Fallout,
    Craters,
}

impl Contamination {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Pollution => "pollution",
            Self::Fallout => "fallout",
            Self::Craters => "craters",
        }
    }
}

impl Tile {
    pub(crate) fn contamination_output(&self, value: i32) -> i32 {
        let value = (value - i32::from(self.craters)).max(0);
        let penalty = match (self.pollution, self.fallout) {
            (true, true) => 75,
            (true, false) | (false, true) => 50,
            _ => 0,
        };
        value - percent_of(value, penalty)
    }

    pub(crate) fn has_contamination(&self, target: Contamination) -> bool {
        match target {
            Contamination::Pollution => self.pollution,
            Contamination::Fallout => self.fallout,
            Contamination::Craters => self.craters,
        }
    }
}

impl Simulation {
    pub(super) fn pollution_risk(&self, city: &City, shields: i32) -> i32 {
        let has = |building| city.has_building(building);
        let recycling = has(Building::RECYCLING_CENTER);
        let hydro = has(Building::HYDRO_PLANT);
        let hoover = has(Building::HOOVER_DAM);
        let solar = has(Building::SOLAR_PLANT);
        let nuclear = has(Building::NUCLEAR_PLANT);
        let reduction = if hoover || solar {
            75 + 25 * i32::from(recycling)
        } else if hydro || nuclear {
            50 + 25 * i32::from(recycling)
                + 25 * i32::from(hydro && self.has_active_wonder(city.owner, Building::HOOVER_DAM))
        } else {
            50 * i32::from(recycling)
        };
        let population = [
            Building::FACTORY,
            Building::MFG_PLANT,
            Building::OFFSHORE_PLATFORM,
            Building::SUPER_HIGHWAYS,
        ]
        .into_iter()
        .filter(|&building| has(building))
        .count() as i32
            * 25
            - 50 * i32::from(has(Building::MASS_TRANSIT))
            - 25 * i32::from(self.has_active_wonder(city.owner, Building::EIFFEL_TOWER));
        (percent_of(shields, (100 - reduction).max(0)) + percent_of(city.size, population.max(0))
            - 20)
            .max(0)
    }

    pub(super) fn check_city_pollution(&mut self, city: &City, shields: i32) {
        let risk = self.pollution_risk(city, shields);
        if risk == 0 || self.rand_below(100) >= risk {
            return;
        }
        if self.place_contamination(city, Contamination::Pollution) && city.owner == HUMAN {
            self.add_event(format!("Pollution appeared near {}.", city.name));
        }
    }

    pub(super) fn place_contamination(&mut self, city: &City, target: Contamination) -> bool {
        let candidates: Vec<_> = self
            .map_city_radius(city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| {
                let tile = self.tile(x, y);
                !tile.has_contamination(target) && !tile.terrain.is_water()
            })
            .collect();
        if candidates.is_empty() {
            return false;
        }
        let (x, y) = candidates[self.rand_below(candidates.len() as i32) as usize];
        let tile = self.tile_mut(x, y);
        match target {
            Contamination::Pollution => tile.pollution = true,
            Contamination::Fallout => tile.fallout = true,
            Contamination::Craters => tile.craters = true,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pollution_can_spawn_and_cleanup_restores_output_one_extra_at_a_time() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for (_, _, x, y) in Simulation::city_radius_at(10, 20) {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, 10, 20);
        let city = game.cities[0].clone();
        assert_eq!(game.pollution_risk(&city, 20), 0);
        game.check_city_pollution(&city, 120);
        let (_, _, x, y) = Simulation::city_radius(&city)
            .find(|&(_, _, x, y)| game.tile(x, y).pollution)
            .unwrap();
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Plains;
        tile.fallout = true;
        assert_eq!(tile.contamination_output(8), 2);
        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        assert!(game.apply(Command::Work(
            worker,
            WorkKind::Clean(Contamination::Pollution)
        )));
        for _ in 0..2 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(x, y).pollution);
        game.update_activities_and_restore();
        assert!(!game.tile(x, y).pollution);
        assert_eq!(game.tile(x, y).contamination_output(8), 4);
    }

    #[test]
    fn craters_reduce_each_tile_yield_but_are_not_a_c2c3_clean_target() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
        tile.irrigation = false;
        tile.mine = false;
        tile.craters = true;
        let yields = tile.yields();
        assert_eq!(yields, (2, 0, 0));
        assert_eq!(
            (
                tile.contamination_output(yields.0),
                tile.contamination_output(yields.1),
                tile.contamination_output(yields.2),
            ),
            (1, 0, 0)
        );

        let worker = game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
        assert!(!game.begin_work(worker, WorkKind::Clean(Contamination::Craters)));
        for _ in 0..3 {
            game.update_activities_and_restore();
        }
        assert!(game.tile(x, y).craters);
        assert_eq!(game.tile(x, y).yields(), (2, 0, 0));
    }
}
