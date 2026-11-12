//! Territory ownership and border interaction rules.

use super::culture::city_cultural_border_rank;
use super::*;

#[cfg(test)]
mod tests;

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
        self.rebuild_city_ownership_inner(None);
    }

    pub(super) fn rebuild_city_ownership_for_city_event(&mut self, city_id: u32) {
        self.rebuild_city_ownership_inner(Some(city_id));
    }

    fn rebuild_city_ownership_inner(&mut self, city_event: Option<u32>) {
        self.refresh_ai_ferries();
        self.refresh_ai_guards();
        self.city_rapture
            .retain(|id, _| self.cities.iter().any(|city| city.id == *id));
        self.ai_settlement_targets.retain(|id, reservation| {
            self.units
                .iter()
                .any(|unit| unit.id == *id && unit.owner == reservation.owner)
        });
        self.ai_ground_targets
            .retain(|_, id| self.cities.iter().any(|city| city.id == *id));
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
            for (x, y) in self.map_wrap.civ3_neighbors(
                self.map_wrap.position(index).0,
                self.map_wrap.position(index).1,
            ) {
                let neighbor = self.map_wrap.index(x, y);
                if self.tiles[neighbor].terrain.is_water() {
                    continue;
                }
                let land = regions[neighbor];
                mixed[region] |= surrounders[region].is_some_and(|other| other != land);
                surrounders[region] = Some(land);
            }
        }

        let mut claims = vec![None; self.tiles.len()];
        let city_distances: Vec<_> = self
            .cities
            .iter()
            .map(|city| self.map_wrap.civ3_distances_from(city.x, city.y))
            .collect();
        let civilization_culture: Vec<_> = (0..self.players.len())
            .map(|civilization| self.civilization_culture(civilization))
            .collect();
        let claim_strength = |city: &City| {
            // The Civilopedia identifies civilization culture as the deciding
            // factor when rival civilizations have equally ranked claims.
            civilization_culture[city.owner]
        };
        let claimable = |city: &City, index: usize, rank: i32| {
            if rank == 0 {
                return true;
            }
            // Conquests Civilopedia gives the city-culture thresholds; the
            // CivWiki/CivFanatics border investigation supplies the candidate
            // ring/rank interpretation. Later rings remain a documented lead.
            // A retained-culture capture preserves the city's cultural reach
            // even when those points were earned under its former owner.
            // The editor option describes retaining city culture; the precise
            // border-resolution behavior still lacks a controlled game check.
            let max_rank = city_cultural_border_rank(city.total_culture());
            if rank == i32::MAX || rank > max_rank {
                return false;
            }
            let source = regions[self.map_wrap.index(city.x, city.y)];
            let target = regions[index];
            if source == target
                || rank <= 1
                || (sizes[target] <= 20 && !mixed[target] && surrounders[target] == Some(source))
            {
                return true;
            }
            let (x, y) = self.map_wrap.position(index);
            let adjacent = || {
                self.map_wrap
                    .civ3_neighbors(x, y)
                    .map(|(x, y)| regions[self.map_wrap.index(x, y)])
            };
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
                let (city_index, city) = self
                    .cities
                    .iter()
                    .enumerate()
                    .find(|(_, candidate)| candidate.id == city.id)
                    .unwrap();
                let rank = city_distances[city_index][index];
                if claimable(city, index, rank) {
                    claims[index] = Some((
                        city.id,
                        city.owner,
                        rank,
                        claim_strength(city),
                        city.total_culture(),
                        city.founded_turn,
                    ));
                }
            }
        }
        for (city_index, city) in self.cities.iter().enumerate() {
            let max_rank = city_cultural_border_rank(city.total_culture());
            let radius = max_rank * 2 + 1;
            let (mx, my) = native_to_map(city.x, city.y);
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (nx, ny) = map_to_native(mx + dx, my + dy);
                    let Some((x, y)) = self.map_wrap.normalize(nx, ny) else {
                        continue;
                    };
                    let index = self.map_wrap.index(x, y);
                    let rank = city_distances[city_index][index];
                    let strength = claim_strength(city);
                    let city_culture = city.total_culture();
                    let replaces = claims[index].is_none_or(
                        |(_, _, old_rank, old_strength, old_city_culture, old_founded_turn)| {
                            rank == 0
                                || (old_rank != 0
                                    && (rank < old_rank
                                        || (rank == old_rank
                                            && (strength > old_strength
                                                || (strength == old_strength
                                                    && (city_culture > old_city_culture
                                                        || (city_culture == old_city_culture
                                                            && city.founded_turn
                                                                < old_founded_turn)))))))
                        },
                    );
                    if claimable(city, index, rank) && replaces {
                        claims[index] = Some((
                            city.id,
                            city.owner,
                            rank,
                            strength,
                            city_culture,
                            city.founded_turn,
                        ));
                    }
                }
            }
        }
        for (tile, claim) in self.tiles.iter_mut().zip(claims) {
            tile.border_city = claim.map(|(id, _, _, _, _, _)| id);
            tile.owner = claim.map(|(_, owner, _, _, _, _)| owner);
        }
        self.remember_visible_borders();
        if let Some(city_id) = city_event {
            self.arrange_city_event_workers(city_id);
        } else {
            self.reconcile_city_workers();
        }
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
                let (x, y) = self.map_wrap.position(tile);
                for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                    let next = self.map_wrap.index(nx, ny);
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
