//! Automated gameplay scenarios covering border behavior.

use super::*;

impl Simulation {
    pub(crate) fn fogged_border_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
        }
        let (x, y) = (10, 24);
        let coast = offset_native_by_map(x, y, 1, 0);
        let sea = offset_native_by_map(x, y, 2, 0);
        let lake = offset_native_by_map(x, y, 0, -2);
        for step in 1..=8 {
            let (sx, sy) = offset_native_by_map(x, y, step, 0);
            self.tile_mut(sx, sy).terrain = Terrain::Ocean;
        }
        // Enlarge the connected sea so it cannot qualify as an enclosed lake.
        for step in 3..=8 {
            for dy in 0..=4 {
                let (sx, sy) = offset_native_by_map(x, y, step, dy);
                self.tile_mut(sx, sy).terrain = Terrain::Ocean;
            }
        }
        self.tile_mut(lake.0, lake.1).terrain = Terrain::Lake;
        self.create_city(1, x, y);
        let water = self.tile(coast.0, coast.1).owner == Some(1)
            && self.tile(sea.0, sea.1).owner.is_none()
            && self.tile(lake.0, lake.1).owner == Some(1);

        let scout = self.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        self.refresh_visibility();
        let saw_claim = self.displayed_tile_owner(x, y) == Some(1);
        self.units.retain(|unit| unit.id != scout);
        self.refresh_visibility();
        self.cities.clear();
        self.rebuild_city_ownership();
        let remembered =
            self.tile(x, y).owner.is_none() && self.displayed_tile_owner(x, y) == Some(1);
        self.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
        self.refresh_visibility();
        let updated = self.displayed_tile_owner(x, y).is_none();
        (water, saw_claim && remembered && updated)
    }

    pub(crate) fn city_borders_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
        }
        let (x, y) = (10, 24);
        let point = |step| offset_native_by_map(x, y, step, 0);
        let (rx, ry) = point(4);
        self.create_city(HUMAN, x, y);
        self.create_city(1, rx, ry);
        let owner = |game: &Self, step| {
            let (x, y) = point(step);
            game.tile(x, y).owner
        };
        let tie = owner(self, 2) == Some(HUMAN);
        self.cities[1].size = 12;
        self.rebuild_city_ownership();
        let fog_restricted = owner(self, 8).is_none();
        for tile in &mut self.tiles {
            tile.known_by[1] = true;
        }
        self.rebuild_city_ownership();
        let strength = owner(self, 2) == Some(1)
            && owner(self, 1) == Some(HUMAN)
            && owner(self, 0) == Some(HUMAN);
        let expanded = owner(self, 8) == Some(1);
        self.cities[1].size = 1;
        self.rebuild_city_ownership();
        (
            tie,
            strength && fog_restricted,
            expanded && owner(self, 8).is_none(),
        )
    }
}
