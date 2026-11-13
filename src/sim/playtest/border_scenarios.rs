//! Automated gameplay scenarios covering border behavior.

use super::*;

impl Simulation {
    pub(crate) fn fogged_border_playtest(&mut self) -> (bool, bool) {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
            tile.owner = None;
            tile.border_city = None;
            tile.last_seen_owner = None;
        }
        let (x, y) = (10, 24);
        let wrap = self.map_wrap;
        // Build and sample the geometry in the same map-coordinate space.
        // The open-water region is large and not enclosed; the one-tile lake
        // remains surrounded by land.
        let coast = wrap.offset(x, y, 1, 0).unwrap();
        let sea = wrap.offset(x, y, 2, 0).unwrap();
        let lake = wrap.offset(x, y, -1, -1).unwrap();
        for dx in 1..=8 {
            for dy in -4..=4 {
                let (sx, sy) = wrap.offset(x, y, dx, dy).unwrap();
                self.tile_mut(sx, sy).terrain = Terrain::Sea;
            }
        }
        self.tile_mut(lake.0, lake.1).terrain = Terrain::Lake;
        for (tx, ty) in [(x, y), coast, sea, lake] {
            self.tile_mut(tx, ty).known_by[1] = true;
        }
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
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [false; PLAYER_SLOTS];
            tile.owner = None;
            tile.border_city = None;
            tile.last_seen_owner = None;
        }
        // Keep this culture-claim scenario independent of the unresolved rule
        // for claiming terrain a civilization has not discovered.
        for tile in &mut self.tiles {
            tile.known_by[HUMAN] = true;
            tile.known_by[1] = true;
        }
        let (x, y) = (10, 24);
        let wrap = self.map_wrap;
        let mut points = vec![(x, y)];
        for _ in 0..8 {
            let previous = *points.last().unwrap();
            points.push(wrap.civ3_offset(previous.0, previous.1, 1, 1).unwrap());
        }
        let point = |step: usize| points[step];
        let (rx, ry) = point(4);
        self.create_city(HUMAN, x, y);
        self.create_city(1, rx, ry);
        self.cities[0].size = 1;
        self.cities[1].size = 1;
        self.rebuild_city_ownership();
        let owner = |game: &Self, step| {
            let (x, y) = point(step);
            game.tile(x, y).owner
        };
        let tie = owner(self, 2) == Some(HUMAN);
        self.cities[1].history = 1_000_000;
        self.rebuild_city_ownership();
        let history_does_not_change_border_strength = owner(self, 2) == Some(HUMAN);

        let mut size_growth = Simulation::new(crate::mapgen::generate(42));
        size_growth.units.clear();
        size_growth.cities.clear();
        for tile in &mut size_growth.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by[HUMAN] = true;
            tile.owner = None;
            tile.border_city = None;
        }
        let origin = (40, 24);
        let mut path = vec![origin];
        for _ in 0..4 {
            let previous = *path.last().unwrap();
            path.push(
                size_growth
                    .map_wrap
                    .civ3_neighbor(previous.0, previous.1, 2)
                    .expect("city border path stays on the map"),
            );
        }
        size_growth.create_city(HUMAN, origin.0, origin.1);
        size_growth.rebuild_city_ownership();
        let small_city = size_growth.tile(path[3].0, path[3].1).owner.is_none();
        size_growth.cities[0].size = 4;
        size_growth.rebuild_city_ownership();
        let grown_city = size_growth.tile(path[3].0, path[3].1).owner == Some(HUMAN);

        (
            tie,
            history_does_not_change_border_strength,
            small_city && grown_city,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fogged_border_playtest_separates_known_coast_lake_and_open_sea() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.fogged_border_playtest(), (true, true));
    }

    #[test]
    fn city_border_playtest_uses_selected_c2c3_strength_and_radius() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.city_borders_playtest(), (true, true, true));
    }
}
