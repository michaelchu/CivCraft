//! Typed cities preparation for deterministic fixtures.

use super::WorldBuilder;
use crate::{
    rules::{Building, Genus},
    world::Terrain,
};

impl WorldBuilder<'_> {
    pub(crate) fn city_food(&mut self, city_id: u32, food: i32) {
        assert!(food >= 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].food = food;
    }

    /// Exclude globally unique wonders from an isolated production comparison.
    pub(crate) fn completed_great_wonders(&mut self) {
        self.game.completed_wonders = Building::all()
            .filter(|building| building.data().genus == Genus::GreatWonder)
            .collect();
    }

    pub(crate) fn city(&mut self, owner: usize, position: (i32, i32), size: i32) -> u32 {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario city owner {owner}"
        );
        assert!(size > 0, "scenario city size must be positive");
        assert!(
            !self.game.tile(position.0, position.1).terrain.is_water(),
            "scenario city must start on land"
        );
        assert!(
            self.game.city_at(position.0, position.1).is_none(),
            "scenario city tile already occupied"
        );
        self.game.create_city(owner, position.0, position.1);
        let city = self.game.cities.last_mut().expect("city was just created");
        city.size = size;
        city.specialists[2] = size;
        city.id
    }

    /// Install a regular city improvement already in place at the start.
    pub(crate) fn city_improvement(&mut self, city_id: u32, building: Building) {
        assert_eq!(
            building.data().genus,
            Genus::Improvement,
            "expected a regular city improvement"
        );
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city must exist");
        if let Some(reason) = self
            .game
            .building_blocker(&self.game.cities[index], building)
        {
            panic!(
                "cannot preset {} in city {city_id}: {reason}",
                building.data().name
            );
        }
        self.game.cities[index].buildings.push(building);
    }

    /// Select a deterministic initial assignment of ordinary workable tiles.
    pub(crate) fn city_workers(&mut self, city_id: u32, count: usize) {
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        let city = &self.game.cities[index];
        let worked: Vec<_> = self
            .game
            .map_city_radius(city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| self.game.tile_can_support_worker(city, x, y))
            .take(count)
            .collect();
        assert_eq!(worked.len(), count, "fixture needs enough worker tiles");
        self.game.cities[index].worked_tiles = worked;
        self.game.cities[index].specialists = [0; 3];
        self.game.cities[index].food = 0;
    }

    /// Toggle all existing governors atomically, preserving citizen assignments.
    pub(crate) fn governors_active(&mut self, active: bool) {
        for governor in self.game.city_governors.values_mut() {
            governor.active = active;
        }
    }

    /// Preset the founding governor without recomputing citizen assignments.
    pub(crate) fn founding_governor(&mut self, city_id: u32) {
        assert!(self.game.city_by_id(city_id).is_some());
        self.game.city_governors.insert(
            city_id,
            crate::sim::CityGovernor {
                active: true,
                settings: crate::sim::GovernorSettings::FOUNDING,
            },
        );
    }

    /// Override installed improvements for a controlled economic stress fixture.
    pub(crate) fn city_buildings(&mut self, city_id: u32, buildings: &[Building]) {
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].buildings = buildings.to_vec();
    }

    pub(crate) fn production_stock(
        &mut self,
        city_id: u32,
        production: crate::sim::Production,
        shields: i32,
    ) {
        assert!(shields >= 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].production = production;
        self.game.cities[index].shields = shields;
    }

    /// Place a city with the simulation's ordinary founding defaults.
    pub(crate) fn founding_city(&mut self, owner: usize, position: (i32, i32)) -> u32 {
        self.founding_city_on(owner, position, Terrain::Grassland)
    }

    pub(crate) fn founding_city_on(
        &mut self,
        owner: usize,
        position: (i32, i32),
        terrain: Terrain,
    ) -> u32 {
        assert!(owner < self.game.civilization_count);
        assert!(!terrain.is_water());
        self.game.tile_mut(position.0, position.1).terrain = terrain;
        self.game.create_city(owner, position.0, position.1);
        self.game.cities.last().expect("city was just created").id
    }

    pub(crate) fn city_population(
        &mut self,
        city_id: u32,
        size: i32,
        specialists: Option<[i32; 3]>,
    ) {
        assert!(size > 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].size = size;
        if let Some(specialists) = specialists {
            assert!(specialists.iter().all(|&n| n >= 0));
            self.game.cities[index].specialists = specialists;
        }
    }

    pub(crate) fn city_history(&mut self, city_id: u32, history: i32) {
        assert!(history >= 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].history = history;
    }
}
