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
    pub(super) fn terrain_pass(
        &self,
        heights: &[i32],
        relief: &[i32],
        biome_field: &[i32],
        rng: &mut MapRng,
    ) -> (Vec<Terrain>, Vec<TerrainBase>, Vec<i32>) {
        let mut dummy_temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| self.colatitude(tile))
            .collect();
        rank_field(&mut dummy_temperature, 1000);
        let shore = self.shore();
        let lowland = self.lowland();
        let normalized_height = |tile: usize| heights[tile];
        let mut terrain: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                if normalized_height(tile) >= shore {
                    return Terrain::Plains;
                }
                let depth = (shore - normalized_height(tile)) * 100 / shore;
                let mut water = 0;
                let mut land = 0;
                for next in self.neighbors(tile) {
                    if normalized_height(next) >= shore {
                        land = 1;
                        break;
                    }
                    water += 1;
                }
                ocean_at_depth((depth + 30 * (water - land) / (water + land)).min(100))
            })
            .collect();

        let mut temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let latitude = self.colatitude(tile);
                let coast = self
                    .neighbors(tile)
                    .filter(|&next| terrain[next].is_water())
                    .count() as i32
                    * 100
                    / 6;
                let height = (-0.3 * f64::from((heights[tile] - shore).max(0)) / 300.0) as f32;
                let maritime = (0.15 * f64::from((60 / 100 - latitude / 1000) * 2 * coast.min(50))
                    / 100.0) as f32;
                (f64::from(latitude) * (1.0 + f64::from(maritime)) * (1.0 + f64::from(height)))
                    as i32
            })
            .collect();
        rank_field(&mut temperature, 1000);

        let mut base_terrain: Vec<_> = terrain
            .iter()
            .map(|&ground| match ground {
                Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                Terrain::Sea => TerrainBase::Sea,
                Terrain::Ocean => TerrainBase::Ocean,
                _ => TerrainBase::Grassland,
            })
            .collect();
        let mut available = vec![false; self.wrap.tile_count()];
        let (hill_low, hill_high, mountain_low, mountain_high, volcano_percent) =
            self.settings.age.relief_bands();
        let (
            tundra_low,
            tundra_high,
            tundra_latitude,
            plains_low,
            plains_high,
            desert_low,
            desert_high,
        ) = self.settings.temperature.biome_bands(self.settings.climate);
        for tile in 0..self.wrap.tile_count() {
            if terrain[tile] != Terrain::Plains {
                continue;
            }
            let ground = if (tundra_low..tundra_high).contains(&biome_field[tile])
                && self.colatitude(tile) < tundra_latitude
            {
                TerrainBase::Tundra
            } else if (desert_low..desert_high).contains(&biome_field[tile]) {
                TerrainBase::Desert
            } else if (plains_low..plains_high).contains(&biome_field[tile]) {
                TerrainBase::Plains
            } else {
                TerrainBase::Grassland
            };
            base_terrain[tile] = ground;
            if (hill_low..hill_high).contains(&relief[tile]) {
                terrain[tile] = Terrain::Hills;
            } else if (mountain_low..mountain_high).contains(&relief[tile]) {
                terrain[tile] = if rng.below(100) < volcano_percent {
                    Terrain::Volcano
                } else {
                    Terrain::Mountains
                };
            } else {
                terrain[tile] = match ground {
                    TerrainBase::Tundra => Terrain::Tundra,
                    TerrainBase::Desert => Terrain::Desert,
                    TerrainBase::Plains => Terrain::Plains,
                    _ => {
                        available[tile] = true;
                        Terrain::Grassland
                    }
                };
            }
        }

        let terrain_water: Vec<_> = terrain.iter().map(|tile| tile.is_water()).collect();
        let remaining = available.iter().filter(|&&open| open).count();
        // OpenCiv3 varies forest probability, jungle latitude bands, and marsh
        // bands by climate. Scale the existing patch-growth heuristic in the
        // same direction; these shares are an approximation, not its exact
        // height-map algorithm or verified original Civ III behavior.
        let (forest_share, jungle_share, marsh_share, dry_water_limit) = match self.settings.climate
        {
            super::Climate::Wet => (24, 6, 12, 1),
            super::Climate::Normal => (16, 4, 7, 2),
            super::Climate::Arid => (8, 2, 3, 3),
        };
        // Shares are relative to the land left after relief.
        let mut patches = [
            remaining * forest_share / 80,
            remaining * jungle_share / 80,
            remaining * marsh_share / 80,
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
                            <= dry_water_limit;
                    match pass {
                        0 => heat >= VERY_COLD,
                        1 => heat >= TROPICAL,
                        _ => heat >= COLD && !dry && heights[tile] < lowland,
                    }
                };
                if let Some(seed) = random_tile(self.wrap.tile_count(), rng, |tile| {
                    available[tile] && eligible(tile)
                }) {
                    let (biome, weight, spread) = [
                        (Terrain::Forest, 50, 60),
                        (Terrain::Jungle, 50, 50),
                        (Terrain::Marsh, 100, 50),
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
                        _ => plains += leftover,
                    }
                }
            }
            if plains > 0 {
                let tile = random_tile(self.wrap.tile_count(), rng, |tile| available[tile])
                    .expect("unplaced plains quota");
                terrain[tile] = if temperature[tile] < VERY_COLD {
                    rng.below(100);
                    Terrain::Tundra
                } else if temperature[tile] < COLD {
                    match rng.below(80) {
                        0..20 => Terrain::Forest,
                        20..30 => Terrain::Marsh,
                        _ => Terrain::Tundra,
                    }
                } else if rng.below(100) < 50 {
                    // Civ III has a distinct shield-yielding Grassland variant.
                    // The manual documents its presence, but not its frequency;
                    // this 1-in-4 share is a project heuristic.
                    if rng.below(4) == 0 {
                        Terrain::GrasslandShield
                    } else {
                        Terrain::Grassland
                    }
                } else {
                    Terrain::Plains
                };
                available[tile] = false;
                plains -= 1;
            }
        }
        (terrain, base_terrain, temperature)
    }

    pub(super) fn finish_water(&self, terrain: &mut [Terrain], _rng: &mut MapRng) {
        // OpenCiv3's FixContinentalShelf pass converts water next to land to
        // Coast, then converts Ocean next to Coast to Sea. It preserves the
        // generator's initial depth classes beyond those two shelf rules.
        // This is an implementation reference; Civ III's original internal
        // algorithm is not documented in the manual.
        for tile in 0..terrain.len() {
            if !matches!(terrain[tile], Terrain::Sea | Terrain::Ocean) {
                continue;
            }
            if self.neighbors(tile).any(|next| !terrain[next].is_water()) {
                terrain[tile] = Terrain::Coast;
            }
        }
        for tile in 0..terrain.len() {
            if terrain[tile] == Terrain::Ocean
                && self
                    .neighbors(tile)
                    .any(|next| terrain[next] == Terrain::Coast)
            {
                terrain[tile] = Terrain::Sea;
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
                    // Civ III uses Coast for small inland water bodies; Lake
                    // is retained only to load older CivCraft saves.
                    terrain[tile] = Terrain::Coast;
                }
            }
        }
    }
}
