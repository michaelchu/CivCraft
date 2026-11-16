//! Deterministic scenarios used by the headless runner.

use super::*;

impl Simulation {
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

    pub(crate) fn settlement_ferry_production_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.completed_wonders = Building::all()
            .filter(|building| building.data().genus == crate::rules::Genus::GreatWonder)
            .collect();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let island = offset_native_by_map(home.0, home.1, 6, 0);
        self.tile_mut(home.0, home.1).terrain = Terrain::Forest;
        self.tile_mut(island.0, island.1).terrain = Terrain::Grassland;
        let productive = offset_native_by_map(island.0, island.1, 1, 0);
        let productive_tile = self.tile_mut(productive.0, productive.1);
        productive_tile.terrain = Terrain::Grassland;
        productive_tile.known_by[1] = true;
        self.tile_mut(island.0, island.1).known_by[1] = true;
        for (_, _, x, y) in self.map_wrap.city_radius(island.0, island.1) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.river = true;
            tile.known_by[1] = true;
        }
        for (_, _, x, y) in self.map_wrap.city_radius(home.0, home.1) {
            if (x, y) == home {
                continue;
            }
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.river = true;
        }
        self.create_city(1, home.0, home.1);
        self.cities[0].size = 3;
        self.cities[0].food = 20;
        self.reconcile_city_workers();
        let city = self.cities[0].id;
        let worked = self.cities[0].worked_tiles.clone();
        for (_, _, x, y) in self.map_wrap.city_radius(home.0, home.1) {
            if (x, y) == home || worked.contains(&(x, y)) {
                continue;
            }
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Sea;
            tile.resource = None;
            tile.irrigation = false;
            tile.river = false;
        }
        self.reconcile_city_workers();
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let before = self.choose_ai_production(1, city);
        self.players[1].known.push(Tech::MAP_MAKING);
        let production = self.choose_ai_production(1, city);
        let boat = Production::Unit(UnitKind::Galley);
        let queued = before != boat && production == boat;
        if !queued {
            return (false, false, false);
        }
        self.cities[0].production = production;
        self.cities[0].shields = production.cost();
        self.process_cities();
        let settler = Production::Unit(UnitKind::Settler);
        let sequenced = self.units.iter().any(|unit| unit.kind == UnitKind::Galley)
            && self.cities[0].production == settler;
        if !sequenced {
            return (true, false, false);
        }
        self.cities[0].shields = settler.cost();
        self.process_cities();
        for _ in 0..8 {
            self.update_activities_and_restore();
            self.run_ai_player(1);
            if self.cities.len() > 1 {
                break;
            }
        }
        let founded = self.cities.iter().any(|city| {
            city.owner == 1
                && self
                    .map_wrap
                    .civ3_distance(city.x, city.y, island.0, island.1)
                    <= 2
        });
        (queued, sequenced, founded)
    }

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
        self.units.clear();
        self.cities.clear();
        self.game_over = None;
        for (owner, x, y) in [(HUMAN, 2, 2), (1, 5, 2)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        checks
    }
}
