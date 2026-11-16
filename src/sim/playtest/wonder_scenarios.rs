//! Automated gameplay scenarios covering wonder behavior.

use super::*;

impl Simulation {
    pub(crate) fn obsolete_wonders_playtest(&mut self) -> (bool, bool, bool) {
        let naval_speed = |game: &Self| {
            let city = &game.cities[0];
            let ship = game.make_unit(0, HUMAN, UnitKind::Destroyer, city.x, city.y);
            game.unit_move_rate(&ship)
        };
        let science = |game: &Self| game.split_city_trade(&game.cities[0], 0, [100, 0, 0])[0];
        let base_speed = naval_speed(self);
        let base_science = science(self);
        let city = &self.cities[0];
        let science_tiles = 1 + city.worked_tiles.len().min(city.tile_workers() as usize) as i32;
        self.record_city_history(0);
        let base_history = self.cities[0].history;
        let wonders = [
            Building::GREAT_WALL,
            Building::LIGHTHOUSE,
            Building::MAGELLANS_EXPEDITION,
            Building::COPERNICUS_OBSERVATORY,
        ];
        self.cities[0].buildings.extend(wonders);
        // In these records the final FALSE is `survives`, while the
        // requirement's `present` defaults to TRUE. The selected C2C3
        // wonders are therefore active until their owner learns the listed
        // technology.
        let active_before_discoveries = naval_speed(self) == base_speed + 2 * MOVE_FRAGMENTS
            && science(self) == base_science + science_tiles
            && self.has_active_wonder(HUMAN, Building::GREAT_WALL);
        self.record_city_history(0);
        let history_recorded = self.cities[0].history > base_history;
        self.players[1].known.extend([
            Tech::ENGINEERING,
            Tech::NUCLEAR_POWER,
            Tech::COMPUTERS,
            Tech::ROBOTICS,
        ]);
        let rivals_did_not_obsolete = active_before_discoveries
            && history_recorded
            && naval_speed(self) == base_speed + 2 * MOVE_FRAGMENTS
            && science(self) == base_science + science_tiles
            && self.has_active_wonder(HUMAN, Building::GREAT_WALL);
        self.players[HUMAN].known.extend([
            Tech::METALLURGY,
            Tech::REPLACEABLE_PARTS,
            Tech::MAGNETISM,
            Tech::COMPUTERS,
        ]);
        // The native Conquests catalog makes Great Wall obsolete at
        // Metallurgy, Lighthouse at Magnetism, Magellan's Expedition at
        // Replaceable Parts, and Copernicus' Observatory at Computers.
        let owner_techs_obsoleted_wonders = naval_speed(self) == base_speed + MOVE_FRAGMENTS
            && science(self) == base_science
            && !self.has_active_wonder(HUMAN, Building::GREAT_WALL);
        self.record_city_history(0);
        let history_accumulated = self.cities[0].history > base_history
            && wonders.iter().all(|b| self.cities[0].buildings.contains(b));
        (
            rivals_did_not_obsolete,
            owner_techs_obsoleted_wonders,
            history_accumulated,
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
}
