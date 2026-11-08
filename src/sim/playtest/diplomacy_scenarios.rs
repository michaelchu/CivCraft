//! Automated gameplay scenarios covering diplomacy behavior.

use super::*;

impl Simulation {
    pub(crate) fn map_snapshot_playtest(&mut self) -> bool {
        self.units.clear();
        let (x, y) = (23, 36);
        let index = self.map_wrap.index(x, y);
        self.tile_mut(x, y).terrain = Terrain::Ocean;
        self.spawn_unit(1, UnitKind::Trireme, x, y);
        self.refresh_visibility();
        self.units.clear();
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.refresh_visibility();
        self.tile_mut(x, y).seen = false;
        self.tile_mut(x, y).known_by[HUMAN] = false;
        self.transfer_treaty_map(1, HUMAN, TreatyMap::Sea);
        self.tile(x, y).seen
            && !self.tile(x, y).visible
            && self.route_memory[index].is_some_and(|tile| tile.is_water())
    }

    pub(crate) fn embassy_playtest_setup(&mut self) -> u32 {
        let (x, y) = (10, 20);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.create_city(1, x, y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.spawn_unit(HUMAN, UnitKind::Diplomat, nx, ny)
    }

    pub(crate) fn diplomacy_playtest_tick(&mut self, turns: i32) {
        for _ in 0..turns {
            self.turn += 1;
            self.advance_diplomacy();
        }
    }

    pub(crate) fn diplomacy_playtest_technologies(&mut self, exchange: bool) {
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        if exchange {
            let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
            self.players[HUMAN].known.push(Tech::POTTERY);
            self.players[1].known.push(writing);
            self.players[HUMAN].gold = 100;
            self.players[1].last_science = 4;
        }
    }

    pub(crate) fn diplomacy_playtest_allied_exchange(&mut self) {
        let currency = Tech::all().find(|tech| tech.name() == "Currency").unwrap();
        self.players[HUMAN]
            .known
            .extend([Tech::BRONZE_WORKING, currency]);
        self.players[1]
            .known
            .extend([Tech::BRONZE_WORKING, Tech::WARRIOR_CODE]);
        // Isolate the quote consumer with an explicitly controlled prior-
        // phase adv_want snapshot. This is not a claim that a locked
        // Marketplace produces this want in the native building advisor.
        // Choose a minimal premium that survives both native integer halves
        // (gold equivalent, then alliance discount) throughout these ticks.
        let until = self.turn + 13;
        let base_bulbs = 3 * self.target_cost(1, currency);
        let premium_want = 4 - base_bulbs.rem_euclid(4);
        let mut retained = vec![0.0; Tech::all().count()];
        retained[currency.0] = (until.max(1) * premium_want) as f32 + 0.25;
        self.ai_tech_wants.insert(1, retained);
        self.diplomacy_playtest_tick(13);
    }

    pub(crate) fn diplomacy_playtest_hidden_city(&self, owner: usize) -> Option<(i32, i32)> {
        self.cities
            .iter()
            .find(|city| city.owner == owner && !self.tile(city.x, city.y).visible)
            .map(|city| (city.x, city.y))
    }

    pub(crate) fn diplomacy_playtest_unit(&mut self, rival: usize) -> Option<(u32, i32, i32)> {
        let city = self.cities_owned_by(rival).next()?;
        let (x, y) = (city.x, city.y);
        let (start_x, start_y) = offset_native_by_map(x, y, -1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(start_x, start_y).terrain = Terrain::Grassland;
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, start_x, start_y);
        Some((id, x, y))
    }

    /// Set up the late-game conditions used by the desktop self-test.
    pub(crate) fn civil_war_playtest(&mut self) -> Option<u32> {
        let city = self.cities_owned_by(HUMAN).next()?;
        let (x, y) = (city.x, city.y);
        let (capital_x, capital_y) = offset_native_by_map(x, y, 1, 0);
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (capital_x, capital_y))
        {
            return None;
        }
        self.units
            .retain(|unit| (unit.x, unit.y) != (capital_x, capital_y));
        self.tile_mut(capital_x, capital_y).terrain = Terrain::Grassland;
        // Use a fresh rival so the target is its primary capital.
        let rival = 1;
        for city in self.cities.iter_mut().filter(|city| city.owner == rival) {
            city.buildings
                .retain(|&building| building != Building::PALACE);
        }
        self.create_city(rival, capital_x, capital_y);
        let capital = self.cities.last_mut()?;
        if !capital.buildings.contains(&Building::PALACE) {
            capital.buildings.push(Building::PALACE);
        }
        capital.size = 8;
        for index in 0..self.tiles.len() {
            if self
                .cities
                .iter()
                .filter(|city| city.owner == rival)
                .count()
                >= 10
            {
                break;
            }
            let (nx, ny) = self.map_wrap.position(index);
            if self.map_distance(x, y, nx, ny) > 5 && self.can_found_at(nx, ny) {
                self.create_city(rival, nx, ny);
                self.cities.last_mut()?.size = 8;
            }
        }
        self.players[rival].government = Government::Tribal;
        self.players[rival].rates = [60, 40, 0];
        self.players[HUMAN].government = Government::Democracy;
        self.refresh_visibility();
        Some(self.spawn_unit(HUMAN, UnitKind::Warrior, x, y))
    }
}
