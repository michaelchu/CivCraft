//! Resource, hut, and starting-location placement during world generation.

use super::*;

impl super::Generator {
    pub(super) fn place_resources(&self, tiles: &mut [Tile], rng: &mut MapRng) {
        // Civ III's terrain chart permits Fish on Coast, and Fish plus Whales
        // on Sea and Ocean. Habitat is terrain data; placement stays generic.
        for tile in 0..tiles.len() {
            if self
                .neighbors(tile)
                .any(|next| tiles[next].resource.is_some())
                || rng.below(1000) >= 250
            {
                continue;
            }
            let terrain = tiles[tile].terrain;
            for (index, &resource) in terrain.resource_options().iter().enumerate() {
                if rng.below(index as i32 + 1) == 0 {
                    tiles[tile].resource = Some(resource);
                }
            }
        }
    }

    pub(super) fn place_huts(&self, tiles: &mut [Tile], rng: &mut MapRng) {
        let target: usize = 15 * self.wrap.tile_count() / 1000;
        let mut reserved = vec![false; self.wrap.tile_count()];
        for _ in 0..target {
            let Some(pick) = random_tile(self.wrap.tile_count(), rng, |tile| !reserved[tile])
            else {
                break;
            };
            if !tiles[pick].terrain.is_water() && self.colatitude(pick) >= 20 {
                tiles[pick].hut = true;
            }
            for tile in self.within(pick, 3) {
                reserved[tile] = true;
            }
        }
    }

    pub(super) fn start_positions(
        &self,
        terrain: &[Terrain],
        tiles: &[Tile],
        temperature: &[i32],
        civilization_count: usize,
        rng: &mut MapRng,
    ) -> Option<Vec<(i32, i32)>> {
        let yields: Vec<_> = tiles
            .iter()
            .map(|tile| {
                let (food, production, trade) = tile.yields();
                let road = tile.terrain.road_trade_bonus();
                let mine = if tile.terrain == Terrain::Glacier {
                    1
                } else {
                    tile.terrain.mine_bonus()
                };
                let improvement = tile.terrain.irrigation_bonus().max(mine);
                food + production + trade + (road + improvement) / 2
            })
            .collect();
        let peaks: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let mut better = 0;
                let mut worse = 0;
                for next in self.within(tile, 2) {
                    better += i32::from(yields[next] > yields[tile]);
                    worse += i32::from(yields[next] < yields[tile]);
                }
                if worse > better {
                    yields[tile] * 100
                } else {
                    0
                }
            })
            .collect();
        let smooth = |values: &[i32], horizontal: bool, weights: [f32; 5]| -> Vec<i32> {
            (0..self.wrap.tile_count())
                .map(|tile| {
                    let (x, y) = self.wrap.position(tile);
                    let mut total = 0.0;
                    for (delta, weight) in (-2..=2).zip(weights) {
                        let position = if horizontal {
                            self.wrap.normalize(x + delta, y)
                        } else {
                            self.wrap.normalize(x, y + delta)
                        };
                        if let Some((nx, ny)) = position {
                            total += weight * values[self.index(nx, ny)] as f32;
                        }
                    }
                    total as i32
                })
                .collect()
        };
        let horizontal = smooth(&peaks, true, [0.13, 0.19, 0.37, 0.19, 0.13]);
        let quality = smooth(&horizontal, false, [0.15, 0.21, 0.29, 0.21, 0.15]);
        let starter = |tile: usize| {
            matches!(
                terrain[tile],
                Terrain::Grassland | Terrain::Plains | Terrain::Forest | Terrain::Hills
            )
        };
        let groups = self.components(terrain, false);
        let mut landmass = vec![usize::MAX; self.wrap.tile_count()];
        let mut goodness = vec![0; groups.len()];
        for (id, group) in groups.iter().enumerate() {
            for &tile in group {
                landmass[tile] = id;
                if starter(tile) {
                    goodness[id] += quality[tile];
                }
            }
        }
        let mut ranked: Vec<_> = (0..groups.len()).collect();
        ranked.sort_by_key(|&id| std::cmp::Reverse(goodness[id]));
        let best = *ranked.first()?;
        let total = goodness.iter().sum::<i32>();
        let candidates: Vec<_> = (0..self.wrap.tile_count())
            .filter(|&tile| starter(tile))
            .collect();
        let mut scores: Vec<_> = candidates.iter().map(|&tile| quality[tile]).collect();
        rank_field(&mut scores, 1000);
        let mut score_by_tile = vec![0; self.wrap.tile_count()];
        for (&tile, score) in candidates.iter().zip(scores) {
            score_by_tile[tile] = score;
        }

        // startpos.c deliberately divides integers before assigning efactor.
        let efactor = (civilization_count / (self.wrap.tile_count() / 1000).max(1) / 4) as f64;
        let single_share = (0.5 + 0.8 * efactor) / (1.0 + efactor);
        let variable_share = (0.65 + 0.8 * efactor) / (1.0 + efactor);
        let mut starts = Vec::with_capacity(civilization_count);
        for single_continent in [true, false] {
            if single_continent
                && (goodness[best] < 1500 * civilization_count as i32
                    || f64::from(goodness[best]) < f64::from(total) * single_share)
            {
                continue;
            }
            let mut allocated = vec![0; groups.len()];
            if single_continent {
                allocated[best] = civilization_count;
            } else {
                let per_player =
                    (f64::from(total) * variable_share / civilization_count as f64) as i32;
                let mut slots = civilization_count;
                for &id in &ranked {
                    if slots == 0 {
                        break;
                    }
                    allocated[id] = (goodness[id] / per_player.max(1)).max(1) as usize;
                    slots = slots.saturating_sub(allocated[id]);
                }
            }
            let mut remaining = allocated.clone();
            let mut threshold = 900;
            while threshold > 10 {
                let eligible = |tile: usize| {
                    if score_by_tile[tile] < threshold
                        || tiles[tile].hut
                        || temperature[tile] < COLD
                    {
                        return false;
                    }
                    let id = landmass[tile];
                    remaining[id] > 0
                        && starts.iter().all(|&other| {
                            let scaled = self.distance(tile, other) * 1000 / threshold;
                            scaled >= 5
                                && (landmass[other] != id
                                    || f64::from(scaled)
                                        > ((groups[id].len() / allocated[id]) as f64).sqrt())
                        })
                };
                if let Some(tile) = random_tile(self.wrap.tile_count(), rng, eligible) {
                    remaining[landmass[tile]] -= 1;
                    starts.push(tile);
                    if starts.len() == civilization_count {
                        return Some(
                            starts
                                .into_iter()
                                .map(|tile| self.wrap.position(tile))
                                .collect(),
                        );
                    }
                } else {
                    threshold = threshold * 95 / 100;
                }
            }
        }
        None
    }
}
