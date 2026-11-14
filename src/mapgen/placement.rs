//! Resource, hut, and starting-location placement during world generation.

use super::temperature::TemperatureMap;
use super::*;
use crate::world::Resource;

/// Freeciv's `startpos.c::get_tile_value` starts with native terrain output,
/// adds the best buildable improvement output on a virtual roaded tile, then
/// grants half of that improvement delta. CivCraft keeps its Civ III resource
/// records, so resource and river output is carried across as a delta from
/// the mapped terrain's Civ III base yield.
fn start_tile_value(
    tile: &GeneratedTile,
    freeciv_terrain_properties: Option<&[TerrainMapgenProperties]>,
    scoring_settings: Option<StartPositionScoringSettings>,
) -> i32 {
    if let (Some(terrains), Some(scoring)) = (freeciv_terrain_properties, scoring_settings)
        && let Some((freeciv_terrain, civ3_base_terrain)) =
            super::terrain::start_scoring_properties(tile.terrain, terrains)
    {
        let current_yields = tile.yields();
        let retained_civ3_delta = current_yields.0 + current_yields.1 + current_yields.2
            - civ3_base_terrain.base_yields().0
            - civ3_base_terrain.base_yields().1
            - civ3_base_terrain.base_yields().2;
        let base = freeciv_terrain.food
            + freeciv_terrain.shield
            + freeciv_terrain.trade
            + retained_civ3_delta;
        let road = if freeciv_terrain.road_time > 0 {
            freeciv_terrain.road_trade_incr_pct * scoring.road_trade_increment / 100
        } else {
            0
        };
        let desert_river_irrigation = if civ3_base_terrain == Terrain::Desert
            && tile.river
            && tile.resource != Some(Resource::Oasis)
        {
            scoring.desert_river_irrigation_percent
        } else {
            0
        };
        let irrigation = if freeciv_terrain.irrigation_time > 0 {
            freeciv_terrain.irrigation_food_incr
                * (scoring.irrigation_percent + desert_river_irrigation)
                / 100
        } else {
            0
        };
        let mining = if freeciv_terrain.mining_time > 0 {
            freeciv_terrain.mining_shield_incr * scoring.mining_percent / 100
        } else {
            0
        };
        let best_improvement = irrigation.max(mining);
        // In Freeciv the virtual road is only used when comparing a buildable
        // irrigation/mine result against the original, unroaded tile.
        let improvement_delta =
            if freeciv_terrain.irrigation_time > 0 || freeciv_terrain.mining_time > 0 {
                road + best_improvement
            } else {
                0
            };
        return base + improvement_delta.max(0) / 2;
    }

    // OpenSimplex has no selected Freeciv ruleset. Retain its Civ III scoring
    // behavior for that alternate generator.
    let (food, production, trade) = tile.yields();
    let mine = if tile.terrain == Terrain::Glacier {
        1
    } else {
        tile.terrain.mine_bonus()
    };
    let improvement = tile.terrain.irrigation_bonus().max(mine);
    food + production + trade + (tile.terrain.road_trade_bonus() + improvement) / 2
}

/// Freeciv's `pick_resource` is reservoir sampling over positive resource
/// weights. CivCraft keeps its Civ III candidate catalog, with equal weights
/// until that catalog supplies explicit weights of its own.
fn pick_weighted_resource<R, I>(resources: I, rng: &mut R) -> Option<Resource>
where
    R: BoundedRandom,
    I: IntoIterator<Item = (Resource, i32)>,
{
    let mut total_weight = 0;
    let mut selected = None;
    for (resource, weight) in resources {
        if weight <= 0 {
            continue;
        }
        total_weight += weight;
        if weight > rng.below(total_weight) {
            selected = Some(resource);
        }
    }
    selected
}

fn pick_equal_weight_resource<R: BoundedRandom>(
    resources: &[Resource],
    rng: &mut R,
) -> Option<Resource> {
    pick_weighted_resource(resources.iter().copied().map(|resource| (resource, 1)), rng)
}

pub(super) fn civ3_terrain_is_unsafe_coast(
    terrain: Terrain,
    ocean_depths: OceanDepthSettings,
    terrain_properties: &[TerrainMapgenProperties],
) -> bool {
    if let Some(unsafe_coast) = ocean_depths.unsafe_coast_for_civ3_terrain(terrain) {
        return unsafe_coast;
    }
    super::terrain::start_scoring_properties(terrain, terrain_properties)
        .map(|(properties, _)| properties.unsafe_coast)
        .unwrap_or(false)
}

fn freeciv_hut_is_eligible(
    generator: &Generator,
    tile: usize,
    terrain: Terrain,
    settings: HutPlacementSettings,
) -> bool {
    if settings.land_only && terrain.is_water() {
        return false;
    }
    let (_, y) = generator.wrap.position(tile);
    // CivCraft has no independent latitude-bound settings. Its current world
    // model uses Freeciv's full -1000..=1000 range on the bounded Y axis.
    let signed_latitude = 1000 - 2000 * y / (generator.wrap.height() - 1);
    (settings.minimum_latitude..=settings.maximum_latitude).contains(&signed_latitude)
}

/// Freeciv `create_start_positions()` uses the startpos=ALL island quota,
/// falling back to VARIABLE when the best island fails either goodness test.
/// This corresponds to mapgen.c dispatching C2C3 `startpos=DEFAULT` + FRACTAL
/// to ALL before calling startpos.c.
fn freeciv_start_island_allocations(
    goodness: &[i32],
    player_count: usize,
    map_tile_count: usize,
) -> Option<Vec<usize>> {
    let mut ranked: Vec<_> = (0..goodness.len()).collect();
    ranked.sort_by_key(|&id| std::cmp::Reverse(goodness[id]));
    let &best = ranked.first()?;
    let total_goodies = goodness.iter().sum::<i32>();
    // startpos.c computes efactor using integer division before assigning it
    // to float; map_size_checked() is max(map_num_tiles() / 1000, 1).
    let efactor = (player_count / (map_tile_count / 1000).max(1) / 4) as f64;
    let all_share = (0.5 + 0.8 * efactor) / (1.0 + efactor);
    let mut allocated = vec![0; goodness.len()];

    if goodness[best] >= 1500 * player_count as i32
        && f64::from(goodness[best]) >= f64::from(total_goodies) * all_share
    {
        allocated[best] = player_count;
        return Some(allocated);
    }

    let variable_share = (0.65 + 0.8 * efactor) / (1.0 + efactor);
    let min_goodies_per_player =
        (f64::from(total_goodies) * variable_share / player_count as f64) as i32;
    let denominator = min_goodies_per_player.max(1);
    let mut to_place = player_count as i32;
    for &id in &ranked {
        if to_place <= 0 {
            break;
        }
        let starters = (goodness[id] / denominator).max(1) as usize;
        allocated[id] = starters;
        // Freeciv may overshoot the remaining player count on the last island;
        // the original quota remains relevant to its same-island spacing rule.
        to_place -= starters as i32;
    }
    Some(allocated)
}

/// C2C3 `script.lua::place_map_labels` visits the River extra before checking
/// the source terrain name. The remaining indices follow its `selected_*`
/// draw order exactly.
fn c2c3_map_label_category(tile: &GeneratedTile) -> Option<usize> {
    if tile.river {
        return Some(0);
    }
    Some(
        match super::terrain::c2c3_map_label_terrain_for_civ3(tile.terrain)? {
            super::terrain::C2c3MapLabelTerrain::DeepOcean => 1,
            super::terrain::C2c3MapLabelTerrain::Ocean => 2,
            super::terrain::C2c3MapLabelTerrain::Lake => 3,
            super::terrain::C2c3MapLabelTerrain::Swamp => 4,
            super::terrain::C2c3MapLabelTerrain::Glacier => 5,
            super::terrain::C2c3MapLabelTerrain::Tundra => 6,
            super::terrain::C2c3MapLabelTerrain::Desert => 7,
            super::terrain::C2c3MapLabelTerrain::Plains => 8,
            super::terrain::C2c3MapLabelTerrain::Grassland => 9,
            super::terrain::C2c3MapLabelTerrain::Jungle => 10,
            super::terrain::C2c3MapLabelTerrain::Forest => 11,
            super::terrain::C2c3MapLabelTerrain::Hills => 12,
            super::terrain::C2c3MapLabelTerrain::Mountains => 13,
        },
    )
}

impl super::Generator {
    /// Advances the game RNG through C2C3's `map_generated` label hook.
    /// Labels are intentionally not retained; the hook is reproduced only to
    /// preserve its conditional setup draws before player shuffling.
    pub(super) fn advance_c2c3_map_label_randomness<R: BoundedRandom>(
        &self,
        tiles: &[GeneratedTile],
        rng: &mut R,
    ) {
        const CATEGORY_COUNT: usize = 14;
        let mut counts = [0_i32; CATEGORY_COUNT];
        for tile in tiles {
            if let Some(category) = c2c3_map_label_category(tile) {
                counts[category] += 1;
            }
        }

        // This order is the ruleset script's `selected_*` sequence. Its
        // probability roll occurs for every category, including empty ones.
        let mut selected = [0_i32; CATEGORY_COUNT];
        for category in 0..CATEGORY_COUNT {
            if rng.below(100) < counts[category] {
                selected[category] = rng.below(counts[category]) + 1;
            }
        }

        // `whole_map_iterate()` scans in tile-index order. Reproduce the
        // selected-category countdown and adjacency branches without creating
        // or storing any presentation label.
        for (tile_index, tile) in tiles.iter().enumerate() {
            let Some(category) = c2c3_map_label_category(tile) else {
                continue;
            };
            if selected[category] == 0 {
                continue;
            }
            selected[category] -= 1;
            if selected[category] != 0 {
                continue;
            }

            let terrain = super::terrain::c2c3_map_label_terrain_for_civ3(tile.terrain);
            match category {
                0 => {
                    if !matches!(
                        terrain,
                        Some(
                            super::terrain::C2c3MapLabelTerrain::Hills
                                | super::terrain::C2c3MapLabelTerrain::Mountains
                                | super::terrain::C2c3MapLabelTerrain::Tundra
                        )
                    ) {
                        let _ = rng.below(100);
                    }
                }
                1 => {
                    // Deep Ocean: `surrounded_by` only changes text.
                    let _ = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::DeepOcean,
                    );
                }
                2 => {
                    // Ocean: surrounding/adjacency only changes text.
                    let _ = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Ocean,
                    );
                    let _ = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Glacier,
                    );
                    let _ = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::DeepOcean,
                    );
                }
                3 => {
                    // Lake: these branches make no random calls.
                    let _ = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Lake,
                    );
                    let _ = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Lake,
                    );
                }
                4 => {
                    if !self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Swamp,
                    ) && self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Ocean,
                    ) {
                        // The isolated-Swamp branch returns before this roll.
                    }
                }
                5 => {
                    if !self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Glacier,
                    ) && self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Glacier,
                    ) {
                        let _ = self.adjacent_to_c2c3_terrain(
                            tiles,
                            tile_index,
                            super::terrain::C2c3MapLabelTerrain::Ocean,
                        );
                    }
                }
                6 => {} // Tundra has a single non-random label.
                7 => {
                    let surrounded = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Desert,
                    );
                    let adjacent = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Desert,
                    );
                    if !surrounded && adjacent {
                        let _ = rng.below(100);
                    }
                }
                8 | 9 => {
                    if !self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Ocean,
                    ) {
                        let _ = rng.below(100);
                    }
                }
                10 => {
                    let _ = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Jungle,
                    );
                    let _ = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Ocean,
                    );
                }
                11 => {
                    let _ = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Mountains,
                    );
                    let _ = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Forest,
                    );
                }
                12 => {
                    if self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Hills,
                    ) {
                        let _ = rng.below(100);
                    } else {
                        let _ = self.adjacent_to_c2c3_terrain(
                            tiles,
                            tile_index,
                            super::terrain::C2c3MapLabelTerrain::Mountains,
                        );
                    }
                }
                13 => {
                    let surrounded = self.surrounded_by_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Mountains,
                    );
                    let adjacent_mountains = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Mountains,
                    );
                    let adjacent_ocean = self.adjacent_to_c2c3_terrain(
                        tiles,
                        tile_index,
                        super::terrain::C2c3MapLabelTerrain::Ocean,
                    );
                    if !surrounded && adjacent_mountains && !adjacent_ocean {
                        let _ = rng.below(100);
                    }
                }
                _ => unreachable!("C2C3 map label category index is in range"),
            }
        }
    }

    fn adjacent_to_c2c3_terrain(
        &self,
        tiles: &[GeneratedTile],
        tile: usize,
        terrain: super::terrain::C2c3MapLabelTerrain,
    ) -> bool {
        self.neighbors(tile).any(|neighbor| {
            super::terrain::c2c3_map_label_terrain_for_civ3(tiles[neighbor].terrain)
                == Some(terrain)
        })
    }

    fn surrounded_by_c2c3_terrain(
        &self,
        tiles: &[GeneratedTile],
        tile: usize,
        terrain: super::terrain::C2c3MapLabelTerrain,
    ) -> bool {
        self.neighbors(tile).all(|neighbor| {
            super::terrain::c2c3_map_label_terrain_for_civ3(tiles[neighbor].terrain)
                == Some(terrain)
        })
    }

    pub(super) fn place_resources<R: BoundedRandom>(
        &self,
        tiles: &mut [GeneratedTile],
        rng: &mut R,
        freeciv_settings: Option<MapGenerationSettings>,
        terrain_properties: Option<&[TerrainMapgenProperties]>,
    ) {
        if let Some(settings) = freeciv_settings {
            let Some(properties) = terrain_properties else {
                panic!("Freeciv resource generation requires parsed terrain properties");
            };
            self.place_freeciv_resources(tiles, rng, settings, properties);
        } else {
            self.place_opensimplex_resources(tiles, rng);
        }
    }

    fn place_freeciv_resources<R: BoundedRandom>(
        &self,
        tiles: &mut [GeneratedTile],
        rng: &mut R,
        settings: MapGenerationSettings,
        terrain_properties: &[TerrainMapgenProperties],
    ) {
        for tile in 0..tiles.len() {
            // Freeciv checks radius one before it draws the per-tile chance.
            if self
                .within(tile, 1)
                .any(|neighbor| tiles[neighbor].resource.is_some())
            {
                continue;
            }
            if rng.below(1000) >= settings.specials_per_thousand {
                continue;
            }

            // This intentionally follows the chance roll. Freeciv consumes a
            // roll for unsafe ocean tiles before rejecting their resources.
            if tiles[tile].terrain.is_water()
                && !settings.ocean_resources
                && !self.freeciv_near_safe_coast(tiles, tile, settings, terrain_properties)
            {
                continue;
            }

            if let Some(resource) =
                pick_equal_weight_resource(tiles[tile].terrain.resource_options(), rng)
            {
                tiles[tile].resource = Some(resource);
            }
        }
    }

    fn freeciv_near_safe_coast(
        &self,
        tiles: &[GeneratedTile],
        tile: usize,
        settings: MapGenerationSettings,
        terrain_properties: &[TerrainMapgenProperties],
    ) -> bool {
        // Freeciv's square_iterate(radius=1) includes the center and its eight
        // neighbors. `within` adapts that local neighborhood to CivCraft's
        // X-wrapped, bounded-Y staggered grid.
        self.within(tile, 1).any(|neighbor| {
            !civ3_terrain_is_unsafe_coast(
                tiles[neighbor].terrain,
                settings.ocean_depths,
                terrain_properties,
            )
        })
    }

    fn place_opensimplex_resources<R: BoundedRandom>(
        &self,
        tiles: &mut [GeneratedTile],
        rng: &mut R,
    ) {
        use crate::world::Resource::{Fish, Whales};

        // Keep ordinary resources on the generic terrain-based roll. Aquatic
        // resources use bounded map-wide targets below so their total does not
        // grow with the size of the ocean.
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
                if matches!(resource, Fish | Whales) {
                    continue;
                }
                if rng.below(index as i32 + 1) == 0 {
                    tiles[tile].resource = Some(resource);
                }
            }
        }

        for (resource, target) in [
            (Fish, self.civilization_count * 2),
            (Whales, self.civilization_count),
        ] {
            let mut candidates: Vec<_> = (0..tiles.len())
                .filter(|&tile| {
                    tiles[tile].terrain.resource_options().contains(&resource)
                        && self.neighbors(tile).any(|next| {
                            is_stable_land(self, tiles, next)
                                || self
                                    .neighbors(next)
                                    .any(|near| is_stable_land(self, tiles, near))
                        })
                })
                .collect();

            // Shuffle eligible tiles with the map RNG, then take the target
            // number that satisfy the same one-tile spacing as land resources.
            for i in (1..candidates.len()).rev() {
                let j = rng.below((i + 1) as i32) as usize;
                candidates.swap(i, j);
            }
            let mut placed = 0;
            for tile in candidates {
                if placed >= target {
                    break;
                }
                if tiles[tile].resource.is_some()
                    || self
                        .neighbors(tile)
                        .any(|next| tiles[next].resource.is_some())
                {
                    continue;
                }
                tiles[tile].resource = Some(resource);
                placed += 1;
            }
        }
    }

    pub(super) fn place_huts<R: BoundedRandom>(
        &self,
        tiles: &mut [GeneratedTile],
        rng: &mut R,
        freeciv_settings: Option<MapGenerationSettings>,
        terrain_properties: Option<&[TerrainMapgenProperties]>,
    ) {
        if let Some(settings) = freeciv_settings {
            let Some(_properties) = terrain_properties else {
                panic!("Freeciv hut generation requires parsed terrain properties");
            };
            self.place_freeciv_huts(tiles, rng, settings);
        } else {
            self.place_opensimplex_huts(tiles, rng);
        }
    }

    fn place_freeciv_huts<R: BoundedRandom>(
        &self,
        tiles: &mut [GeneratedTile],
        rng: &mut R,
        settings: MapGenerationSettings,
    ) {
        let mut remaining = settings.huts_per_thousand * self.wrap.tile_count() as i32 / 1000;
        let mut reserved = vec![false; self.wrap.tile_count()];
        for _ in 0..self.wrap.tile_count() * 2 {
            if remaining <= 0 {
                break;
            }
            let Some(pick) = random_tile(self.wrap.tile_count(), rng, |tile| !reserved[tile])
            else {
                break;
            };
            // Freeciv consumes one hut attempt even when no generated Hut
            // extra is eligible on the chosen tile, then reserves the region.
            remaining -= 1;
            if freeciv_hut_is_eligible(self, pick, tiles[pick].terrain, settings.hut_placement) {
                // C2C3 has one generated Hut extra. `rand_extra_for_tile()`
                // still calls `fc_rand(1)`, which returns zero without
                // advancing the source RNG state.
                let _hut_option = rng.below(1);
                tiles[pick].hut = true;
            }
            for tile in self.within(pick, 3) {
                reserved[tile] = true;
            }
        }
    }

    fn place_opensimplex_huts<R: BoundedRandom>(&self, tiles: &mut [GeneratedTile], rng: &mut R) {
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

    #[expect(
        clippy::too_many_arguments,
        reason = "Start scoring consumes independent terrain fields, settings, and the deterministic RNG."
    )]
    pub(super) fn start_positions(
        &self,
        terrain: &[Terrain],
        tiles: &[GeneratedTile],
        temperature: &[i32],
        freeciv_temperature: Option<&TemperatureMap>,
        freeciv_terrain_properties: Option<&[TerrainMapgenProperties]>,
        scoring_settings: Option<StartPositionScoringSettings>,
        civilization_count: usize,
        rng: &mut FreecivRng,
    ) -> Option<Vec<(i32, i32)>> {
        let yields: Vec<_> = tiles
            .iter()
            .map(|tile| start_tile_value(tile, freeciv_terrain_properties, scoring_settings))
            .collect();
        let peaks: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let mut better = 0;
                let mut worse = 0;
                // Freeciv compares over CITY_MAP_DEFAULT_RADIUS_SQ; the
                // campaign adapter expresses that as Civ III's clipped,
                // horizontally wrapped 21-tile city radius.
                let (x, y) = self.wrap.position(tile);
                for (_, _, nx, ny) in self.wrap.city_radius(x, y) {
                    let next = self.wrap.index(nx, ny);
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
        let ruleset_starters =
            freeciv_terrain_properties.map(super::terrain::civ3_starter_terrains);
        let starter = |tile: usize| match &ruleset_starters {
            Some(starters) => starters.contains(&terrain[tile]),
            None => matches!(
                terrain[tile],
                Terrain::Grassland | Terrain::Plains | Terrain::Forest | Terrain::Hills
            ),
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
        let allocated = freeciv_start_island_allocations(
            &goodness,
            civilization_count,
            self.wrap.tile_count(),
        )?;
        let candidates: Vec<_> = (0..self.wrap.tile_count())
            .filter(|&tile| starter(tile))
            .collect();
        let mut scores: Vec<_> = candidates.iter().map(|&tile| quality[tile]).collect();
        rank_field(&mut scores, 1000);
        let mut score_by_tile = vec![0; self.wrap.tile_count()];
        for (&tile, score) in candidates.iter().zip(scores) {
            score_by_tile[tile] = score;
        }

        let mut remaining = allocated.clone();
        let mut starts = Vec::with_capacity(civilization_count);
        let mut threshold = 900;
        while threshold > 10 {
            let eligible = |tile: usize| {
                if score_by_tile[tile] < threshold
                    || tiles[tile].hut
                    || freeciv_temperature
                        .map(|map| map.get(tile).is_frozen_or_cold())
                        .unwrap_or(temperature[tile] < COLD)
                {
                    return false;
                }
                let id = landmass[tile];
                remaining[id] > 0
                    && starts.iter().all(|&other| {
                        // Freeciv checks real_map_distance() here. Generator's
                        // distance adapter uses the campaign's X-wrapped,
                        // bounded-Y staggered grid for the same scaling tests.
                        let scaled = self.distance(tile, other) * 1000 / threshold;
                        scaled >= 5
                            && (landmass[other] != id
                                || f64::from(scaled)
                                    > ((groups[id].len() / allocated[id]) as f64).sqrt())
                    })
            };
            if let Some(tile) = freeciv_rand_map_pos_filtered(self.wrap.tile_count(), rng, eligible)
            {
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
                // startpos.c performs `data.min_value *= 0.95` on an int.
                threshold = (f64::from(threshold) * 0.95) as i32;
            }
        }
        None
    }
}

fn is_stable_land(generator: &Generator, tiles: &[GeneratedTile], tile: usize) -> bool {
    // The ocean-only polar cap is applied after placement, so its land must
    // not make an aquatic resource candidate appear coastal.
    !generator.is_polar_cap_tile(tile) && !tiles[tile].terrain.is_water()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct ScriptedRng {
        values: Vec<i32>,
        next_value: usize,
        limits: Vec<i32>,
    }

    impl BoundedRandom for ScriptedRng {
        fn below(&mut self, limit: i32) -> i32 {
            self.limits.push(limit);
            let value = self.values.get(self.next_value).copied().unwrap_or(0);
            self.next_value += 1;
            if limit <= 1 {
                0
            } else {
                value.rem_euclid(limit)
            }
        }
    }

    fn tile(terrain: Terrain, resource: Option<Resource>, river: bool) -> GeneratedTile {
        GeneratedTile {
            terrain,
            base_terrain: TerrainBase::default(),
            altitude: 0,
            hut: false,
            river,
            river_edges: 0,
            resource,
        }
    }

    /// Oracle for the per-category source loop in the pinned C2C3 Lua hook:
    /// each 1..100 roll is immediately followed by the selected-index roll
    /// when the probability succeeds. A zero count has no index roll.
    fn c2c3_label_selection_oracle(counts: [i32; 14]) -> Vec<i32> {
        let mut bounds = Vec::new();
        for count in counts {
            bounds.push(100);
            if count > 0 {
                bounds.push(count);
            }
        }
        bounds
    }

    fn freeciv_generator(
        wrap: MapWrap,
    ) -> (
        Generator,
        MapGenerationSettings,
        Vec<TerrainMapgenProperties>,
    ) {
        let rules = RulesetData::campaign_default();
        (
            Generator {
                wrap,
                settings: Civ3WorldSettings::default(),
                civilization_count: 4,
            },
            rules.map_generation_settings().unwrap(),
            rules.terrain_mapgen_properties().unwrap(),
        )
    }

    #[test]
    fn c2c3_label_hook_matches_source_draw_oracle_on_a_plain_map() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, _, _) = freeciv_generator(wrap);
        let tiles = vec![tile(Terrain::Plains, None, false); wrap.tile_count()];
        let mut recorded = ScriptedRng::default();

        generator.advance_c2c3_map_label_randomness(&tiles, &mut recorded);

        // Pinned data/civ2civ3/script.lua rolls a chance for each category,
        // immediately chooses one of 256 Plains, then rolls for non-coastal
        // Plains.
        let mut category_counts = [0; 14];
        category_counts[8] = wrap.tile_count() as i32;
        let mut source_draw_bounds = c2c3_label_selection_oracle(category_counts);
        source_draw_bounds.push(100);
        assert_eq!(recorded.limits, source_draw_bounds);

        // The same source trace advances the gameplay RNG by the same amount,
        // including Freeciv's non-advancing bound-1 draw behavior if present.
        let mut actual = crate::random::FreecivRng::new(42);
        let mut oracle = crate::random::FreecivRng::new(42);
        generator.advance_c2c3_map_label_randomness(&tiles, &mut actual);
        for bound in source_draw_bounds {
            oracle.draw(bound as u32);
        }
        assert_eq!(actual, oracle);
    }

    #[test]
    fn c2c3_label_hook_uses_the_selected_hill_in_map_order() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, _, _) = freeciv_generator(wrap);
        let mut tiles = vec![tile(Terrain::Plains, None, false); wrap.tile_count()];
        let isolated_hill = wrap.index(0, 0);
        let paired_hill = wrap.index(8, 8);
        let paired_neighbor = generator
            .neighbors(paired_hill)
            .find(|&neighbor| neighbor > paired_hill)
            .expect("a bounded-Y neighbor follows the selected hill in tile order");
        for index in [isolated_hill, paired_hill, paired_neighbor] {
            tiles[index].terrain = Terrain::Hills;
        }

        // The `random(1, hills)` result is the one-based occurrence selected
        // during whole_map_iterate(). First select the isolated hill: only
        // Plains consumes a conditional roll. Then select the paired hill:
        // C2C3's Hills branch consumes one additional random(1, 100).
        let first_target_values = vec![0; 16];
        let mut second_target_values = vec![0; 16];
        second_target_values[14] = 1;
        let mut first_target = ScriptedRng {
            values: first_target_values,
            ..ScriptedRng::default()
        };
        let mut second_target = ScriptedRng {
            values: second_target_values,
            ..ScriptedRng::default()
        };
        generator.advance_c2c3_map_label_randomness(&tiles, &mut first_target);
        generator.advance_c2c3_map_label_randomness(&tiles, &mut second_target);

        let mut category_counts = [0; 14];
        category_counts[8] = wrap.tile_count() as i32 - 3;
        category_counts[12] = 3;
        let mut common_source_draw_bounds = c2c3_label_selection_oracle(category_counts);
        common_source_draw_bounds.push(100);
        assert_eq!(first_target.limits, common_source_draw_bounds);
        common_source_draw_bounds.push(100);
        assert_eq!(second_target.limits, common_source_draw_bounds);
    }

    #[test]
    fn c2c3_label_hook_matches_source_conditional_branch_draws() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, _, _) = freeciv_generator(wrap);
        let mut tiles = vec![tile(Terrain::Plains, None, false); wrap.tile_count()];

        // The Lua hook counts River before terrain. This selected river tile
        // retains Desert underneath, so its branch draws once.
        let river_desert = wrap.index(0, 0);
        tiles[river_desert].terrain = Terrain::Desert;
        tiles[river_desert].river = true;

        // Deep Ocean is fully surrounded. Ocean touches Glacier. These
        // adjacency-only paths and the Lake/Swamp/Glacier branches add no RNG.
        let deep_center = wrap.index(5, 8);
        for index in std::iter::once(deep_center).chain(generator.neighbors(deep_center)) {
            tiles[index].terrain = Terrain::Ocean;
        }
        let ocean = wrap.index(12, 10);
        tiles[ocean].terrain = Terrain::Sea;
        let glacier = generator.neighbors(ocean).next().unwrap();
        tiles[glacier].terrain = Terrain::Glacier;
        tiles[wrap.index(2, 12)].terrain = Terrain::Coast;
        tiles[wrap.index(6, 12)].terrain = Terrain::Marsh;
        tiles[wrap.index(8, 12)].terrain = Terrain::Tundra;

        // The first Desert, Grassland, Plains, Hills, and Mountains targets
        // take the source's random branches. Other terrain labels use only
        // adjacency tests and therefore consume no conditional draw.
        let desert = wrap.index(4, 4);
        let desert_neighbor = generator
            .neighbors(desert)
            .find(|&neighbor| neighbor > desert)
            .unwrap();
        tiles[desert].terrain = Terrain::Desert;
        tiles[desert_neighbor].terrain = Terrain::Desert;
        tiles[wrap.index(10, 4)].terrain = Terrain::Grassland;
        tiles[wrap.index(10, 10)].terrain = Terrain::Jungle;
        tiles[wrap.index(0, 8)].terrain = Terrain::Forest;
        let hills = wrap.index(12, 4);
        let hills_neighbor = generator
            .neighbors(hills)
            .find(|&neighbor| neighbor > hills)
            .unwrap();
        tiles[hills].terrain = Terrain::Hills;
        tiles[hills_neighbor].terrain = Terrain::Hills;
        let mountains = wrap.index(13, 12);
        let mountains_neighbor = generator
            .neighbors(mountains)
            .find(|&neighbor| neighbor > mountains)
            .unwrap();
        tiles[mountains].terrain = Terrain::Mountains;
        tiles[mountains_neighbor].terrain = Terrain::Mountains;

        let mut rng = ScriptedRng::default();
        generator.advance_c2c3_map_label_randomness(&tiles, &mut rng);

        // Oracle from the pinned Lua's 14 probability rolls, 14 category
        // target rolls in rule order, and six selected-label random branches:
        // River, Desert, Plains, Grassland, Hills, and Mountains.
        let category_counts = [1, 9, 1, 1, 1, 1, 1, 2, 232, 1, 1, 1, 2, 2];
        let mut source_draw_bounds = c2c3_label_selection_oracle(category_counts);
        source_draw_bounds.extend([100; 6]);
        assert_eq!(rng.limits, source_draw_bounds);
    }

    #[test]
    fn c2c3_label_adjacency_wraps_x_and_keeps_y_bounded() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, _, _) = freeciv_generator(wrap);

        // The first Plains tile sees source Ocean across the X seam, so the
        // label hook skips its otherwise-required conditional roll.
        let mut wrapped_tiles = vec![tile(Terrain::Plains, None, false); wrap.tile_count()];
        wrapped_tiles[wrap.index(wrap.width() - 1, 0)].terrain = Terrain::Sea;
        let mut wrapped_rng = ScriptedRng::default();
        generator.advance_c2c3_map_label_randomness(&wrapped_tiles, &mut wrapped_rng);
        let mut wrapped_counts = [0; 14];
        wrapped_counts[2] = 1;
        wrapped_counts[8] = wrap.tile_count() as i32 - 1;
        assert_eq!(
            wrapped_rng.limits,
            c2c3_label_selection_oracle(wrapped_counts)
        );

        // A tile near the opposite pole does not become adjacent through Y.
        // The first Plains therefore keeps the source's random(1, 100) call.
        let mut bounded_tiles = vec![tile(Terrain::Plains, None, false); wrap.tile_count()];
        bounded_tiles[wrap.index(0, wrap.height() - 2)].terrain = Terrain::Sea;
        let mut bounded_rng = ScriptedRng::default();
        generator.advance_c2c3_map_label_randomness(&bounded_tiles, &mut bounded_rng);
        let bounded_counts = wrapped_counts;
        let mut bounded_draws = c2c3_label_selection_oracle(bounded_counts);
        bounded_draws.push(100);
        assert_eq!(bounded_rng.limits, bounded_draws);
    }

    #[test]
    fn freeciv_resource_chance_draw_is_skipped_near_existing_resources() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, mut settings, properties) = freeciv_generator(wrap);
        settings.specials_per_thousand = 0;
        let mut tiles = vec![tile(Terrain::Ocean, None, false); wrap.tile_count()];
        let resource_tile = wrap.index(8, 8);
        tiles[resource_tile].resource = Some(Resource::Fish);
        let skipped: Vec<_> = generator.within(resource_tile, 1).collect();
        let expected_rolls = wrap.tile_count() - skipped.len();
        let mut rng = ScriptedRng::default();

        generator.place_resources(&mut tiles, &mut rng, Some(settings), Some(&properties));

        assert_eq!(rng.limits, vec![1000; expected_rolls]);
        assert!(
            tiles
                .iter()
                .filter(|tile| tile.resource.is_some())
                .all(|tile| tile.terrain == Terrain::Ocean)
        );
    }

    #[test]
    fn freeciv_ocean_resource_gate_runs_after_chance_and_respects_setting() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, mut settings, properties) = freeciv_generator(wrap);
        settings.specials_per_thousand = 1000;
        let mut tiles = vec![tile(Terrain::Ocean, None, false); wrap.tile_count()];
        let mut rng = ScriptedRng::default();

        generator.place_resources(&mut tiles, &mut rng, Some(settings), Some(&properties));

        assert!(tiles.iter().all(|tile| tile.resource.is_none()));
        assert_eq!(rng.limits, vec![1000; wrap.tile_count()]);

        settings.ocean_resources = true;
        let mut tiles = vec![tile(Terrain::Ocean, None, false); wrap.tile_count()];
        let mut rng = ScriptedRng::default();
        generator.place_resources(&mut tiles, &mut rng, Some(settings), Some(&properties));

        assert_eq!(tiles[0].resource, Some(Resource::Whales));
        assert_eq!(&rng.limits[..3], &[1000, 1, 2]);
    }

    #[test]
    fn freeciv_resource_picker_uses_sequential_weighted_replacement() {
        let mut rng = ScriptedRng {
            values: vec![1, 3],
            ..ScriptedRng::default()
        };
        let selected =
            pick_weighted_resource([(Resource::Fish, 2), (Resource::Whales, 3)], &mut rng);

        assert_eq!(selected, Some(Resource::Fish));
        assert_eq!(rng.limits, [2, 5]);
    }

    #[test]
    fn freeciv_huts_consume_ineligible_attempts_and_reserve_radius_three() {
        let wrap = MapWrap::with_dimensions(16, 16).unwrap();
        let (generator, settings, properties) = freeciv_generator(wrap);
        let tiles = &mut vec![tile(Terrain::Plains, None, false); wrap.tile_count()];
        let mut rng = ScriptedRng::default();

        assert!(!freeciv_hut_is_eligible(
            &generator,
            wrap.index(0, 0),
            Terrain::Plains,
            settings.hut_placement,
        ));
        assert!(freeciv_hut_is_eligible(
            &generator,
            wrap.index(0, 1),
            Terrain::Plains,
            settings.hut_placement,
        ));
        assert!(!freeciv_hut_is_eligible(
            &generator,
            wrap.index(0, 0),
            Terrain::Coast,
            settings.hut_placement,
        ));

        generator.place_huts(tiles, &mut rng, Some(settings), Some(&properties));

        let first_reserved = generator.within(wrap.index(0, 0), 3).count();
        assert_eq!(rng.limits[0], wrap.tile_count() as i32);
        assert_eq!(
            rng.limits[26],
            (wrap.tile_count() - first_reserved) as i32,
            "the first polar attempt reserves radius three before the next filtered fallback"
        );
        assert_eq!(
            rng.limits
                .iter()
                .filter(|&&limit| limit == wrap.tile_count() as i32)
                .count(),
            51,
            "all three hut attempts are consumed even when candidates are polar"
        );
        assert!(!tiles[wrap.index(0, 0)].hut);
    }

    #[test]
    fn start_tile_value_uses_c2c3_outputs_and_retains_civ3_resources() {
        let rules = RulesetData::campaign_default();
        let terrain_properties = rules.terrain_mapgen_properties().unwrap();
        let scoring = rules.start_position_scoring_settings().unwrap();

        assert_eq!(scoring.road_trade_increment, 1);
        assert_eq!(scoring.irrigation_percent, 100);
        assert_eq!(scoring.desert_river_irrigation_percent, 100);
        assert_eq!(scoring.mining_percent, 100);

        let grassland = tile(Terrain::Grassland, None, false);
        assert_eq!(
            start_tile_value(&grassland, Some(&terrain_properties), Some(scoring)),
            3,
            "C2C3 Grassland is 2 base output plus half of a road-and-irrigation delta"
        );

        let cattle_grassland = tile(Terrain::Grassland, Some(Resource::Cattle), false);
        assert_eq!(
            start_tile_value(&cattle_grassland, Some(&terrain_properties), Some(scoring)),
            6,
            "the native C2C3 terrain score should retain Civ III's resource output delta"
        );

        let hills = tile(Terrain::Hills, None, false);
        assert_eq!(
            start_tile_value(&hills, Some(&terrain_properties), Some(scoring)),
            3,
            "Hills use C2C3's better mine increment, with no road trade bonus"
        );

        let plain_desert = tile(Terrain::Desert, None, false);
        let desert_river = tile(Terrain::Desert, None, true);
        let desert_oasis_river = tile(Terrain::Desert, Some(Resource::Oasis), true);
        assert_eq!(
            start_tile_value(&plain_desert, Some(&terrain_properties), Some(scoring)),
            2
        );
        assert_eq!(
            start_tile_value(&desert_river, Some(&terrain_properties), Some(scoring)),
            3,
            "C2C3's extra Nile irrigation output applies only to Desert+River without Oasis"
        );
        assert_eq!(
            start_tile_value(
                &desert_oasis_river,
                Some(&terrain_properties),
                Some(scoring)
            ),
            6,
            "the Oasis resource suppresses the Nile bonus while preserving Civ III output"
        );

        let flood_plains = tile(Terrain::FloodPlains, None, true);
        assert_eq!(
            start_tile_value(&flood_plains, Some(&terrain_properties), Some(scoring)),
            5,
            "Civ III Flood Plains retain their output delta over Freeciv Desert"
        );
    }

    #[test]
    fn start_site_local_comparison_radius_adapts_to_x_wrap_and_bounded_y() {
        let wrap = MapWrap::with_dimensions(20, 20).unwrap();
        let center = wrap.index(0, 10);
        let radius: Vec<_> = wrap
            .city_radius(0, 10)
            .map(|(_, _, x, y)| wrap.index(x, y))
            .collect();
        assert_eq!(radius.len(), 21);
        assert!(radius.iter().all(|&tile| {
            let (x, y) = wrap.position(tile);
            (0..wrap.height()).contains(&y) && (0..wrap.width()).contains(&x)
        }));
        assert!(radius.contains(&center));
        assert!(
            radius
                .iter()
                .any(|&tile| wrap.position(tile).0 == wrap.width() - 1)
        );

        let polar_radius: Vec<_> = wrap
            .city_radius(0, 0)
            .map(|(_, _, x, y)| wrap.index(x, y))
            .collect();
        assert!(polar_radius.len() < 21);
        assert!(polar_radius.iter().all(|&tile| wrap.position(tile).1 >= 0));
    }

    #[test]
    fn freeciv_default_fractal_uses_all_when_the_best_island_passes_both_tests() {
        let allocations = freeciv_start_island_allocations(&[20_000, 500], 6, 36 * 48).unwrap();
        assert_eq!(allocations, [6, 0]);
    }

    #[test]
    fn freeciv_all_fallback_uses_variable_quotas_and_preserves_overshoot() {
        // For six players on this map, efactor is 1. The best island passes
        // the 1500-per-player test but fails the 65% total-goodies test, so
        // VARIABLE assigns 4 + 3 slots. The second island overshoots the
        // remaining two players; the later island receives no slot.
        let allocations =
            freeciv_start_island_allocations(&[10_000, 7_000, 1_000], 6, 36 * 48).unwrap();
        assert_eq!(allocations, [4, 3, 0]);
    }

    #[test]
    fn freeciv_filtered_map_draw_returns_the_first_valid_index() {
        let mut expected_rng = FreecivRng::new(42);
        let expected = expected_rng.draw(32) as usize;
        let mut actual_rng = FreecivRng::new(42);
        assert_eq!(
            freeciv_rand_map_pos_filtered(32, &mut actual_rng, |tile| tile == expected),
            Some(expected)
        );
        assert_eq!(actual_rng, expected_rng);
    }

    #[test]
    fn freeciv_filtered_map_fallback_draws_from_valid_map_order() {
        let count = 30;
        let mut expected_rng = FreecivRng::new(314);
        let rejected: Vec<_> = (0..count / 10)
            .map(|_| expected_rng.draw(count as u32) as usize)
            .collect();
        let candidates: Vec<_> = (0..count)
            .filter(|tile| !rejected.contains(tile))
            .take(4)
            .collect();
        let expected = candidates[expected_rng.draw(candidates.len() as u32) as usize];

        let mut actual_rng = FreecivRng::new(314);
        assert_eq!(
            freeciv_rand_map_pos_filtered(count, &mut actual_rng, |tile| {
                candidates.contains(&tile)
            }),
            Some(expected)
        );
        assert_eq!(actual_rng, expected_rng);
    }
}
