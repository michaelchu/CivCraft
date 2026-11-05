use super::{terrain_work::neighbors, *};

const CITY_WORK_RADIUS_SQ: i32 = 5;
const PERMANENT_RADIUS_SQ: i32 = CITY_WORK_RADIUS_SQ - 3;

impl Simulation {
    pub(crate) fn displayed_tile_owner(&self, x: i32, y: i32) -> Option<usize> {
        let tile = self.tile(x, y);
        if tile.visible {
            tile.owner
        } else if tile.seen {
            tile.last_seen_owner
        } else {
            None
        }
    }

    pub(super) fn remember_visible_borders(&mut self) {
        for tile in self.tiles.iter_mut().filter(|tile| tile.visible) {
            tile.last_seen_owner = tile.owner;
        }
    }

    pub(super) fn rebuild_city_ownership(&mut self) {
        let (regions, sizes) = self.border_regions();
        let mut surrounders = vec![None; sizes.len()];
        let mut mixed = vec![false; sizes.len()];
        for (index, _) in self
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| tile.terrain.is_water())
        {
            let region = regions[index];
            for (x, y) in neighbors(Self::xy(index).0, Self::xy(index).1) {
                let neighbor = Self::idx(x, y);
                if self.tiles[neighbor].terrain.is_water() {
                    continue;
                }
                let land = regions[neighbor];
                mixed[region] |= surrounders[region].is_some_and(|other| other != land);
                surrounders[region] = Some(land);
            }
        }

        let mut claims = vec![None; self.tiles.len()];
        let claim_strength = |city: &City, index: usize| {
            let (x, y) = Self::xy(index);
            let distance_sq = self.map_distance(city.x, city.y, x, y).pow(2);
            (distance_sq, (city.size + 2).pow(2) / distance_sq.max(1))
        };
        let claimable = |city: &City, index: usize, distance_sq: i32| {
            if distance_sq == 0 {
                return true;
            }
            if distance_sq > CITY_WORK_RADIUS_SQ + city.size.min(26)
                || !self.tiles[index].known_by[city.owner]
            {
                return false;
            }
            let source = regions[Self::idx(city.x, city.y)];
            let target = regions[index];
            if source == target
                || distance_sq <= 2
                || (sizes[target] <= 20 && !mixed[target] && surrounders[target] == Some(source))
            {
                return true;
            }
            let (x, y) = Self::xy(index);
            let adjacent = || neighbors(x, y).map(|(x, y)| regions[Self::idx(x, y)]);
            adjacent().any(|region| sizes[region] <= 3)
                && adjacent().any(|region| region == source)
                && adjacent().all(|region| region == source || region == target)
        };

        for (index, tile) in self.tiles.iter().enumerate() {
            if let Some(city) = self
                .cities
                .iter()
                .find(|city| Some(city.id) == tile.border_city)
            {
                let (distance_sq, strength) = claim_strength(city, index);
                if claimable(city, index, distance_sq) {
                    claims[index] = Some((city.id, city.owner, distance_sq, strength));
                }
            }
        }
        for city in &self.cities {
            let radius = (CITY_WORK_RADIUS_SQ + city.size.min(26)).isqrt();
            let (mx, my) = native_to_map(city.x, city.y);
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (x, y) = map_to_native(mx + dx, my + dy);
                    if !(0..MAP_H).contains(&y) {
                        continue;
                    }
                    let index = Self::idx(x, y);
                    let (distance_sq, strength) = claim_strength(city, index);
                    if claimable(city, index, distance_sq)
                        && (distance_sq == 0
                            || claims[index].is_none_or(|(_, _, old_distance, old_strength)| {
                                old_distance > PERMANENT_RADIUS_SQ && strength > old_strength
                            }))
                    {
                        claims[index] = Some((city.id, city.owner, distance_sq, strength));
                    }
                }
            }
        }
        for (tile, claim) in self.tiles.iter_mut().zip(claims) {
            tile.border_city = claim.map(|(id, _, _, _)| id);
            tile.owner = claim.map(|(_, owner, _, _)| owner);
        }
        self.remember_visible_borders();
        self.reconcile_city_workers();
    }

    fn border_regions(&self) -> (Vec<usize>, Vec<usize>) {
        let mut regions = vec![usize::MAX; self.tiles.len()];
        let mut sizes = Vec::new();
        for index in 0..self.tiles.len() {
            if regions[index] != usize::MAX {
                continue;
            }
            let water = self.tiles[index].terrain.is_water();
            let region = sizes.len();
            sizes.push(0);
            regions[index] = region;
            let mut pending = vec![index];
            while let Some(tile) = pending.pop() {
                sizes[region] += 1;
                let (x, y) = Self::xy(tile);
                for (nx, ny) in neighbors(x, y) {
                    let next = Self::idx(nx, ny);
                    if regions[next] == usize::MAX && self.tiles[next].terrain.is_water() == water {
                        regions[next] = region;
                        pending.push(next);
                    }
                }
            }
        }
        (regions, sizes)
    }
}
