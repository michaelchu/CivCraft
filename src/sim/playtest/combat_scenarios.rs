//! Automated gameplay scenarios covering combat behavior.

use super::*;

impl Simulation {
    /// Runs representative Civ III unit matchups through the normal map-move
    /// entry point. The combat round and odds algorithms follow Freeciv's
    /// `unit_versus_unit` and `win_chance`; Civ III campaign unit records supply
    /// the units' strengths, hit points, and firepower.
    pub(crate) fn freeciv_combat_matrix_playtest(&mut self) -> Vec<(&'static str, bool, String)> {
        let cases = [
            (
                "open_grassland",
                UnitKind::Warrior,
                UnitKind::Warrior,
                Terrain::Grassland,
                false,
            ),
            (
                "forest_defense",
                UnitKind::Swordsman,
                UnitKind::Spearman,
                Terrain::Forest,
                false,
            ),
            (
                "hill_defense",
                UnitKind::Cavalry,
                UnitKind::Rifleman,
                Terrain::Hills,
                false,
            ),
            (
                "city_defense",
                UnitKind::Cavalry,
                UnitKind::Rifleman,
                Terrain::Grassland,
                true,
            ),
        ];
        let mut checks = Vec::with_capacity(cases.len());

        for (case_index, (name, attacker_kind, defender_kind, terrain, city)) in
            cases.into_iter().enumerate()
        {
            self.units.clear();
            self.cities.clear();
            for tile in &mut self.tiles {
                tile.terrain = Terrain::Grassland;
                tile.river = false;
                tile.road = false;
                tile.railroad = false;
                tile.base = None;
                tile.resource = None;
                tile.hut = false;
            }

            let x = 5 + case_index as i32 * 4;
            let y = 20;
            let (tx, ty) = self.map_wrap.civ3_neighbors(x, y).next().unwrap();
            self.tile_mut(tx, ty).terrain = terrain;
            self.sign_pact(HUMAN, 1, Pact::War);
            if city {
                self.create_city(1, tx, ty);
            }
            let attacker_id = self.spawn_unit(HUMAN, attacker_kind, x, y);
            let defender_id = self.spawn_unit(1, defender_kind, tx, ty);
            let defender = self.unit_by_id(defender_id).unwrap().clone();
            let base_defense = self.defense_power_against(&defender, x, y, attacker_kind);
            let terrain_defense = self.defense_power_against(&defender, tx, ty, attacker_kind);
            let modifier_ok = if terrain == Terrain::Grassland && !city {
                terrain_defense == base_defense
            } else {
                terrain_defense > base_defense
            };
            // Disable promotion randomness so the battle result is driven by
            // Freeciv's combat-round draws alone.
            for unit in &mut self.units {
                unit.c2c3_veteran = u8::MAX as usize;
            }
            self.native_random = None;
            self.rng = 0x0C13 + case_index as u64;
            self.refresh_visibility();

            let direction = self
                .map_wrap
                .civ3_neighbors(x, y)
                .position(|position| position == (tx, ty))
                .unwrap();
            let result = self.try_move_unit_civ3(attacker_id, direction);
            let attacker = self.unit_by_id(attacker_id);
            let defender = self.unit_by_id(defender_id);
            let attacker_hp = attacker.map_or(0, |unit| unit.hp);
            let defender_hp = defender.map_or(0, |unit| unit.hp);
            let terminal = attacker.is_none_or(|unit| unit.hp <= 0)
                || defender.is_none_or(|unit| unit.hp <= 0);
            let passed = result == MoveResult::Fought && terminal && modifier_ok;
            let outcome = match (attacker.is_some(), defender.is_some()) {
                (true, false) => format!("{} won with {attacker_hp} HP", attacker_kind.name()),
                (false, true) => format!("{} won with {defender_hp} HP", defender_kind.name()),
                (false, false) => "both units were destroyed".into(),
                (true, true) => format!("combat did not end ({attacker_hp} vs {defender_hp} HP)"),
            };
            checks.push((
                name,
                passed,
                format!(
                    "{} {} vs {} on {:?}{}: defense {base_defense}->{terrain_defense}; {result:?}; {outcome}",
                    attacker_kind.name(),
                    attacker_kind.stats().attack,
                    defender_kind.name(),
                    terrain,
                    if city { " in a city" } else { "" },
                ),
            ));
        }
        // Keep both civilizations alive after the scripted battles so the
        // runner can continue into normal turn simulation if requested.
        self.units.clear();
        self.cities.clear();
        self.game_over = None;
        for (owner, x, y) in [(HUMAN, 2, 2), (1, 5, 2)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        checks
    }

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
