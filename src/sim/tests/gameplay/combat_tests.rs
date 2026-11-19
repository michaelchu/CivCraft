//! Unit checks and fixtures for private simulation behavior.

use super::*;

impl Simulation {
    pub(crate) fn combat_population_test(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        // Isolate population and automatic rearrangement from random terrain.
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.pollution = false;
            tile.fallout = false;
        }
        let (x, y) = (12, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (wx, wy) = offset_native_by_map(x, y, 8, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(1, nx, ny);
        self.create_city(1, wx, wy);
        let target = self.cities.len() - 2;
        let wonder_city = target + 1;
        self.cities[target].size = 5;
        self.cities[target].specialists = [0, 0, 3];
        self.cities[target].worked_tiles = Self::city_radius_at(nx, ny)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&p| p != (nx, ny))
            .take(2)
            .collect();
        self.cities[target].food = Self::growth_box(5);
        self.spawn_unit(1, UnitKind::Warrior, nx, ny);

        let attacker = self.spawn_unit(HUMAN, UnitKind::Chariot, x, y);
        debug_assert!(UnitKind::Chariot.is_big_land());
        let attacker = self
            .units
            .iter()
            .find(|unit| unit.id == attacker)
            .unwrap()
            .clone();
        let light_attacker = Unit {
            kind: UnitKind::Warrior,
            ..attacker.clone()
        };
        self.apply_combat_population_loss(&light_attacker, nx, ny);
        let light_attack = self.cities[target].size == 5;
        self.cities[target].buildings.push(Building::CITY_WALLS);
        self.apply_combat_population_loss(&attacker, nx, ny);
        let walls = self.cities[target].size == 5;
        self.cities[target]
            .buildings
            .retain(|&b| b != Building::CITY_WALLS);
        self.cities[wonder_city]
            .buildings
            .push(Building::GREAT_WALL);
        self.apply_combat_population_loss(&attacker, nx, ny);
        let great_wall = self.cities[target].size == 5;
        self.cities[wonder_city].buildings.clear();
        self.apply_combat_population_loss(&attacker, nx, ny);
        let exposed = self.cities[target].size == 4
            && self.cities[target].food == Self::growth_box(4)
            // Native CM follows citizen removal. With a full granary and
            // plain Grassland, three workers feed size four and the last
            // citizen becomes a Taxman (the Palace boosts gold) rather than
            // staying an entertainer.
            && self.cities[target].specialists == [0, 1, 0]
            && self.cities[target].worked_tiles.len() == 3;
        self.cities[target].size = 1;
        self.cities[target].specialists = [0; 3];
        self.apply_combat_population_loss(&attacker, nx, ny);
        light_attack && walls && great_wall && exposed && self.cities[target].size == 1
    }
}
