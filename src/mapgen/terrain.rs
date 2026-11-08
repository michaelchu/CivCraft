//! Terrain sprite selection, blending, and tile surface rendering.

use super::*;

struct PatchGrowth<'a, Eligible> {
    generator: &'a super::Generator,
    remaining: &'a mut usize,
    available: &'a mut [bool],
    terrain: &'a mut [Terrain],
    heights: &'a [i32],
    eligible: &'a Eligible,
    rng: &'a mut MapRng,
}

impl<Eligible: Fn(usize) -> bool> PatchGrowth<'_, Eligible> {
    fn grow(&mut self, tile: usize, budget: i32, biome: Terrain) {
        if *self.remaining == 0 {
            return;
        }
        self.terrain[tile] = biome;
        self.available[tile] = false;
        *self.remaining -= 1;
        let generator = self.generator;
        for next in generator.neighbors(tile) {
            let delta = (generator.colatitude(tile) - generator.colatitude(next)).abs() / 66
                + (self.heights[tile] - self.heights[next]).abs();
            if self.available[next]
                && (self.eligible)(next)
                && delta < budget
                && self.rng.below(10) > 4
            {
                self.grow(next, budget - 1 - delta, biome);
            }
        }
    }
}

impl super::Generator {
    pub(super) fn polar_height_factor(&self, tile: usize) -> i32 {
        if (FROZEN..=ICE_BASE * 5 / 2).contains(&self.colatitude(tile)) {
            10
        } else {
            1
        }
    }

    pub(super) fn restored_height(&self, heights: &[i32], tile: usize) -> i32 {
        let factor = self.polar_height_factor(tile);
        heights[tile] / factor * factor
    }

    pub(super) fn terrain_pass(
        &self,
        heights: &[i32],
        rng: &mut MapRng,
    ) -> (Vec<Terrain>, Vec<i32>) {
        let mut dummy_temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| self.colatitude(tile))
            .collect();
        rank_field(&mut dummy_temperature, 1000);
        let normalized_height = |tile: usize| {
            let (_, y) = self.wrap.position(tile);
            if !self.wrap.wraps_y()
                && (y < 4 || y >= self.wrap.height() - 4)
                && self.colatitude(tile) > ICE_BASE * 5 / 2
            {
                0
            } else {
                heights[tile] / self.polar_height_factor(tile)
            }
        };
        let mut terrain: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                if normalized_height(tile) >= SHORE {
                    return Terrain::Plains;
                }
                let depth = (SHORE - normalized_height(tile)) * 100 / SHORE;
                // The reference still tests the frozen-ocean fringe even
                // when this ruleset falls back to unfrozen water.
                if (FROZEN..COLD).contains(&dummy_temperature[tile]) {
                    rng.below(10);
                }
                let mut water = 0;
                let mut land = 0;
                for next in self.neighbors(tile) {
                    if normalized_height(next) >= SHORE {
                        land = 1;
                        break;
                    }
                    water += 1;
                }
                ocean_at_depth((depth + 30 * (water - land) / (water + land)).min(100))
            })
            .collect();

        let restored: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| self.restored_height(heights, tile))
            .collect();
        let heights = &restored;
        let mut temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let latitude = self.colatitude(tile);
                let coast = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next].is_water())
                    .count() as i32
                    * 100
                    / 6;
                let height = (-0.3 * f64::from((heights[tile] - SHORE).max(0)) / 300.0) as f32;
                let maritime = (0.15 * f64::from((60 / 100 - latitude / 1000) * 2 * coast.min(50))
                    / 100.0) as f32;
                (f64::from(latitude) * (1.0 + f64::from(maritime)) * (1.0 + f64::from(height)))
                    as i32
            })
            .collect();
        rank_field(&mut temperature, 1000);

        // Civ2Civ3 has no frozen ocean terrain. Frozen coasts can become
        // glaciers, with a broken margin where cold tiles border frozen ones.
        let original_land: Vec<_> = terrain.iter().map(|tile| !tile.is_water()).collect();
        for tile in 0..self.wrap.tile_count() {
            if original_land[tile] {
                continue;
            }
            let frozen = temperature[tile] < FROZEN
                || (temperature[tile] < COLD
                    && rng.below(10) > 7
                    && self.neighbors(tile).any(|next| temperature[next] < FROZEN));
            if frozen && self.neighbors(tile).all(|next| !original_land[next]) {
                terrain[tile] = Terrain::Plains;
            }
        }

        let mut available = vec![false; self.wrap.tile_count()];
        for tile in 0..self.wrap.tile_count() {
            if terrain[tile] != Terrain::Plains {
                continue;
            }
            let higher = self
                .within(tile, 2)
                .filter(|&next| heights[next] > heights[tile])
                .count() as i32;
            let local_peak = self.within(tile, 2).all(|next| heights[next] <= HIGHLAND)
                && self
                    .neighbors(tile)
                    .all(|next| heights[next] <= heights[tile])
                && higher <= 2
                && (HIGHLAND - SHORE) * higher <= (heights[tile] - SHORE) * 4;
            let high_plateau = self
                .within(tile, 1)
                .all(|next| heights[next] + (1000 - HIGHLAND) / 5 >= HIGHLAND);
            if (heights[tile] > HIGHLAND && (rng.below(10) > 5 || !high_plateau)) || local_peak {
                let (hills_weight, mountain_weight) = if temperature[tile] >= COLD {
                    if rng.below(10) < 4 { (30, 70) } else { (30, 0) }
                } else if rng.below(10) < 8 {
                    (0, 70)
                } else {
                    (30, 70)
                };
                terrain[tile] = if rng.below(hills_weight + mountain_weight) < hills_weight {
                    Terrain::Hills
                } else {
                    Terrain::Mountains
                };
            } else {
                available[tile] = true;
            }
        }

        let terrain_water: Vec<_> = terrain.iter().map(|tile| tile.is_water()).collect();
        let remaining = available.iter().filter(|&&open| open).count();
        // Civ2Civ3 defaults: temperature 60, wetness 50, steepness 30.
        // Quotas are relative to the land still available after relief.
        let mut patches = [
            remaining * 16 / 80,
            remaining * 4 / 80,
            remaining * 7 / 80,
            remaining * 8 / 80,
            0,
        ];
        let mut plains = remaining - patches.iter().sum::<usize>();
        while plains > 0 || patches.iter().any(|&count| count > 0) {
            for pass in 0..patches.len() {
                if patches[pass] == 0 {
                    continue;
                }
                let eligible = |tile: usize| {
                    let heat = temperature[tile];
                    let dry = (623..=832).contains(&self.colatitude(tile))
                        && self
                            .neighbors(tile)
                            .filter(|&next| terrain_water[next])
                            .count()
                            <= 2;
                    match pass {
                        0 => heat >= FROZEN,
                        1 => heat >= TROPICAL,
                        2 => heat >= COLD && !dry && heights[tile] < LOWLAND,
                        3 => heat >= FROZEN && dry && heights[tile] >= LOWLAND,
                        _ => heat >= FROZEN && heights[tile] >= LOWLAND,
                    }
                };
                if let Some(seed) = random_tile(self.wrap.tile_count(), rng, |tile| {
                    available[tile] && eligible(tile)
                }) {
                    let (biome, weight, spread) = [
                        (Terrain::Forest, 50, 60),
                        (Terrain::Jungle, 50, 50),
                        (Terrain::Swamp, 100, 50),
                        (Terrain::Desert, 100, 80),
                        (Terrain::Desert, 100, 40),
                    ][pass];
                    rng.below(weight);
                    PatchGrowth {
                        generator: self,
                        remaining: &mut patches[pass],
                        available: &mut available,
                        terrain: &mut terrain,
                        heights,
                        eligible: &eligible,
                        rng,
                    }
                    .grow(seed, spread, biome);
                } else {
                    let leftover = std::mem::take(&mut patches[pass]);
                    match pass {
                        1 | 2 => patches[0] += leftover,
                        3 => patches[4] += leftover,
                        _ => plains += leftover,
                    }
                }
            }
            if plains > 0 {
                let tile = random_tile(self.wrap.tile_count(), rng, |tile| available[tile])
                    .expect("unplaced plains quota");
                terrain[tile] = if temperature[tile] < FROZEN {
                    rng.below(100);
                    Terrain::Glacier
                } else if temperature[tile] < COLD {
                    match rng.below(80) {
                        0..20 => Terrain::Forest,
                        20..30 => Terrain::Swamp,
                        _ => Terrain::Tundra,
                    }
                } else if rng.below(100) < 50 {
                    Terrain::Grassland
                } else {
                    Terrain::Plains
                };
                available[tile] = false;
                plains -= 1;
            }
        }
        (terrain, temperature)
    }

    pub(super) fn finish_water(&self, terrain: &mut [Terrain], rng: &mut MapRng) {
        for tile in 0..terrain.len() {
            if !terrain[tile].is_water() {
                continue;
            }
            if let Some(distance) = (1..=4).find(|&radius| {
                self.within(tile, radius)
                    .any(|next| !terrain[next].is_water())
            }) {
                terrain[tile] = ocean_at_depth(distance * 25 + rng.below(15));
            }
        }
        for tile in 0..terrain.len() {
            if terrain[tile].is_water() {
                let shallow = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next] == Terrain::Ocean)
                    .count();
                let deep = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next] == Terrain::DeepOcean)
                    .count();
                let threshold = 2 * ISOHEX_DIRS.len() / 3;
                if shallow >= threshold {
                    terrain[tile] = Terrain::Ocean;
                } else if deep >= threshold {
                    terrain[tile] = Terrain::DeepOcean;
                }
            }
        }

        let mut landmass = vec![None; self.wrap.tile_count()];
        for (id, group) in self.components(terrain, false).into_iter().enumerate() {
            for tile in group {
                landmass[tile] = Some(id);
            }
        }
        for basin in self
            .components(terrain, true)
            .into_iter()
            .filter(|group| group.len() <= 14)
        {
            let mut shore = basin
                .iter()
                .flat_map(|&tile| self.neighbors(tile))
                .filter_map(|tile| landmass[tile]);
            if let Some(continent) = shore.next()
                && shore.all(|id| id == continent)
            {
                for tile in basin {
                    terrain[tile] = Terrain::Lake;
                }
            }
        }
    }
}
