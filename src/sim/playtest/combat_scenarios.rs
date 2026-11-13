//! Automated gameplay scenarios covering combat behavior.

use super::*;

impl Simulation {
    pub(crate) fn marines_playtest_setup(&mut self) -> (u32, u32, u32, bool) {
        let (x, y) = (12, 24);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let (hx, hy) = offset_native_by_map(x, y, -5, 0);
        self.tile_mut(x, y).terrain = Terrain::Sea;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.tile_mut(hx, hy).terrain = Terrain::Grassland;
        self.create_city(HUMAN, hx, hy);
        self.create_city(1, nx, ny);
        self.cities.last_mut().unwrap().size = 2;
        let rubber = self
            .map_wrap
            .civ3_neighbors(hx, hy)
            .find(|&(rx, ry)| self.city_at(rx, ry).is_none())
            .expect("marine playtest needs a nearby rubber tile");
        let rubber_tile = self.tile_mut(rubber.0, rubber.1);
        rubber_tile.terrain = Terrain::Forest;
        rubber_tile.resource = Some(Resource::Rubber);
        rubber_tile.owner = Some(HUMAN);
        rubber_tile.road = true;
        let target = Production::Unit(UnitKind::Marine);
        let locked = !self.can_produce(&self.cities[0], target);
        self.players[HUMAN].known.push(Tech::AMPHIBIOUS_WARFARE);
        let available = self.can_produce(&self.cities[0], target);
        let boat = self.spawn_unit(HUMAN, UnitKind::Galleon, x, y);
        let infantry = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().transport = Some(boat);
        let marines = self.spawn_unit(HUMAN, UnitKind::Marine, x, y);
        self.units.last_mut().unwrap().transport = Some(boat);
        let guard = self.spawn_unit(1, UnitKind::Worker, nx, ny);
        self.sign_pact(HUMAN, 1, Pact::War);
        self.refresh_visibility();
        (marines, infantry, guard, locked && available)
    }

    pub(crate) fn combat_population_playtest(&mut self) -> bool {
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
