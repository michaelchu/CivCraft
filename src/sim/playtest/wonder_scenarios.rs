//! Automated gameplay scenarios covering wonder behavior.

use super::*;

impl Simulation {
    pub(crate) fn obsolete_wonders_playtest(&mut self) -> (bool, bool, bool) {
        let naval_speed = |game: &Self| game.unit_move_allowance(HUMAN, UnitKind::Destroyer);
        let science = |game: &Self| game.split_city_trade(&game.cities[0], 0, [100, 0, 0])[0];
        let base_speed = naval_speed(self);
        let base_science = science(self);
        let city = &self.cities[0];
        let science_tiles = 1 + city.worked_tiles.len().min(city.tile_workers() as usize) as i32;
        self.record_history(0);
        let base_history = self.cities[0].history;
        let wonders = [
            Building::GREAT_WALL,
            Building::LIGHTHOUSE,
            Building::MAGELLANS_EXPEDITION,
            Building::COPERNICUS_OBSERVATORY,
        ];
        self.cities[0].buildings.extend(wonders);
        self.players[1].known.extend([
            Tech::ENGINEERING,
            Tech::NUCLEAR_POWER,
            Tech::COMPUTERS,
            Tech::MACHINE_TOOLS,
        ]);
        let active = naval_speed(self) == base_speed + 2 * MOVE_FRAGMENTS
            && science(self) == base_science + science_tiles
            && self.has_active_wonder(HUMAN, Building::GREAT_WALL);
        self.record_history(0);
        let active_history = self.cities[0].history
            == 2 * base_history
                + wonders
                    .into_iter()
                    .map(Building::history_per_turn)
                    .sum::<i32>();
        self.players[HUMAN].known.push(Tech::ENGINEERING);
        let lighthouse_ended = naval_speed(self) == base_speed + MOVE_FRAGMENTS;
        self.players[HUMAN].known.extend([
            Tech::NUCLEAR_POWER,
            Tech::COMPUTERS,
            Tech::MACHINE_TOOLS,
        ]);
        // Nuclear Power's own movement bonus replaces Magellan's bonus.
        let naval_ended = lighthouse_ended && naval_speed(self) == base_speed + MOVE_FRAGMENTS;
        let prior_history = self.cities[0].history;
        self.record_history(0);
        let history_retained = self.cities[0].history == prior_history + base_history
            && wonders.iter().all(|b| self.cities[0].buildings.contains(b));
        (
            active && active_history,
            naval_ended
                && science(self) == base_science
                && !self.has_active_wonder(HUMAN, Building::GREAT_WALL),
            history_retained,
        )
    }

    pub(crate) fn theatre_output_playtest(&mut self) -> bool {
        let (x, y) = (self.cities[0].x, self.cities[0].y);
        let width = self.map_wrap.width();
        let (tx, ty) = ((x + 8).rem_euclid(width), y);
        let (rx, ry) = ((x - 8).rem_euclid(width), y);
        self.create_city(HUMAN, tx, ty);
        self.create_city(1, rx, ry);
        self.players[HUMAN].government = Government::Democracy;
        self.cities[1]
            .buildings
            .push(Building::SHAKESPEARES_THEATER);
        self.completed_wonders.push(Building::SHAKESPEARES_THEATER);
        self.cities[2].specialists[2] = 1;
        let luxury =
            |game: &Self, index| game.split_city_trade(&game.cities[index], 0, [100, 0, 0])[2];
        let theatre = luxury(self, 0) == 1 && luxury(self, 1) == 6 && luxury(self, 2) == 3;
        self.cities[1]
            .buildings
            .retain(|&b| b != Building::SHAKESPEARES_THEATER);
        theatre && luxury(self, 0) == 0 && luxury(self, 2) == 3
    }

    pub(crate) fn wonder_output_playtest(&mut self) -> (bool, bool, bool) {
        let (x, y) = (self.cities[0].x, self.cities[0].y);
        let (wx, wy) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty, river) in [(x, y, true), (wx, wy, false)] {
            let tile = self.tile_mut(tx, ty);
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = river;
            tile.road = false;
            tile.pollution = false;
            tile.fallout = false;
        }
        self.players[HUMAN].government = Government::Anarchy;
        self.cities[0].size = 2;
        self.cities[0].worked_tiles = vec![(wx, wy)];
        self.cities[0].specialists = [0; 3];
        let base_trade = self.city_yields(&self.cities[0]).2;
        self.cities[0].buildings.push(Building::COLOSSUS);
        let zero_tile_unchanged = self.city_yields(&self.cities[0]).2 == base_trade + 1;
        self.tile_mut(wx, wy).river = true;
        let trade_bonus =
            zero_tile_unchanged && self.city_yields(&self.cities[0]).2 == base_trade + 3;
        self.players[HUMAN].known.push(Tech::AUTOMOBILE);
        let colossus_obsolete = self.city_yields(&self.cities[0]).2 == base_trade + 1;

        let (sx, sy) = offset_native_by_map(x, y, 5, 0);
        let (rx, ry) = offset_native_by_map(x, y, -5, 0);
        self.create_city(HUMAN, sx, sy);
        self.create_city(1, rx, ry);
        self.players[HUMAN].government = Government::Monarchy;
        self.cities[0].buildings.extend([
            Building::HANGING_GARDENS,
            Building::JS_BACHS_CATHEDRAL,
            Building::CURE_FOR_CANCER,
            Building::MARKETPLACE,
        ]);
        let luxury =
            |game: &Self, city: usize| game.split_city_trade(&game.cities[city], 0, [100, 0, 0])[2];
        let luxury_scoped = luxury(self, 0) == 12 && luxury(self, 1) == 4 && luxury(self, 2) == 0;
        self.players[HUMAN].known.push(Tech::ELECTRICITY);
        let gardens_obsolete = luxury(self, 0) == 6 && luxury(self, 1) == 4;
        (
            trade_bonus,
            luxury_scoped,
            colossus_obsolete && gardens_obsolete,
        )
    }

    pub(crate) fn barracks_playtest(&mut self) -> (bool, bool, bool) {
        let index = self
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        let (x, y) = (self.cities[index].x, self.cities[index].y);
        self.cities[index]
            .buildings
            .extend([Building::BARRACKS, Building::SUN_TZUS_WAR_ACADEMY]);
        let stacked = self.trained_rank(&self.cities[index], UnitKind::Warrior) == 2;
        let unit = self.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().hp = 1;
        self.update_activities_and_restore();
        let healed = self
            .units
            .iter()
            .any(|unit_info| unit_info.id == unit && unit_info.hp == 10);
        self.players[HUMAN].known.push(Tech::GUNPOWDER);
        let retained = self.active_barracks(&self.cities[index])
            && self.trained_rank(&self.cities[index], UnitKind::Warrior) == 2;
        (stacked, healed, retained)
    }
}
