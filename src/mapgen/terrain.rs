//! Terrain sprite selection, blending, and tile surface rendering.

use super::temperature::{TemperatureMap, TemperatureType};
use super::*;
use std::collections::VecDeque;

// Values copied from the pinned Freeciv `mapgen_utils.c` implementation and
// `common/terrain.h`; C2C3's terrain-specific depth values remain ruleset data.
const FREECIV_OCEAN_DEPTH_MAXIMUM: i32 = 100;
const FREECIV_OCEAN_DEPTH_STEP: i32 = 25;
const FREECIV_OCEAN_DEPTH_RANDOM_RANGE: i32 = 15;
const FREECIV_OCEAN_DISTANCE_MAX: usize =
    (FREECIV_OCEAN_DEPTH_MAXIMUM / FREECIV_OCEAN_DEPTH_STEP) as usize;

#[derive(Clone, Copy)]
enum TerrainMapgenProperty {
    Mountainous,
    Green,
    Foliage,
    Tropical,
    Temperate,
    Cold,
    Frozen,
    Wet,
    Dry,
}

impl TerrainMapgenProperty {
    fn value(self, terrain: &TerrainMapgenProperties) -> i32 {
        match self {
            Self::Mountainous => terrain.mountainous,
            Self::Green => terrain.green,
            Self::Foliage => terrain.foliage,
            Self::Tropical => terrain.tropical,
            Self::Temperate => terrain.temperate,
            Self::Cold => terrain.cold,
            Self::Frozen => terrain.frozen,
            Self::Wet => terrain.wet,
            Self::Dry => terrain.dry,
        }
    }
}

fn civ3_terrain_for_freeciv_rule(rule_name: &str) -> Option<Terrain> {
    Some(match rule_name.to_ascii_lowercase().as_str() {
        "desert" => Terrain::Desert,
        "forest" => Terrain::Forest,
        "grassland" => Terrain::Grassland,
        "hills" => Terrain::Hills,
        "jungle" => Terrain::Jungle,
        "mountains" => Terrain::Mountains,
        "plains" => Terrain::Plains,
        "swamp" => Terrain::Marsh,
        "tundra" => Terrain::Tundra,
        "glacier" => Terrain::Glacier,
        _ => return None,
    })
}

/// C2C3 terrain identities relevant to its `place_map_labels` hook. These are
/// temporary generation identities only; map labels are presentation data and
/// are not stored in `GeneratedMap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum C2c3MapLabelTerrain {
    Lake,
    Ocean,
    DeepOcean,
    Swamp,
    Glacier,
    Tundra,
    Desert,
    Plains,
    Grassland,
    Jungle,
    Forest,
    Hills,
    Mountains,
}

/// Recover the C2C3 rule terrain represented by a generated Civ III terrain.
/// This is deliberately explicit because some source and presentation names
/// differ: Swamp is rendered as Marsh, inland Lake basins as Coast, and the
/// C2C3 Ocean/Deep Ocean depths as Civ III Sea/Ocean. Flood Plains retain the
/// source Desert identity on river tiles.
pub(super) fn c2c3_map_label_terrain_for_civ3(terrain: Terrain) -> Option<C2c3MapLabelTerrain> {
    Some(match terrain {
        Terrain::Coast | Terrain::Lake => C2c3MapLabelTerrain::Lake,
        Terrain::Sea => C2c3MapLabelTerrain::Ocean,
        Terrain::Ocean => C2c3MapLabelTerrain::DeepOcean,
        Terrain::Marsh => C2c3MapLabelTerrain::Swamp,
        Terrain::Glacier => C2c3MapLabelTerrain::Glacier,
        Terrain::Tundra => C2c3MapLabelTerrain::Tundra,
        Terrain::Desert | Terrain::FloodPlains => C2c3MapLabelTerrain::Desert,
        Terrain::Plains => C2c3MapLabelTerrain::Plains,
        Terrain::Grassland => C2c3MapLabelTerrain::Grassland,
        Terrain::Jungle => C2c3MapLabelTerrain::Jungle,
        Terrain::Forest => C2c3MapLabelTerrain::Forest,
        Terrain::Hills => C2c3MapLabelTerrain::Hills,
        Terrain::Mountains => C2c3MapLabelTerrain::Mountains,
        // Civ III-only terrain variants have no generated C2C3 identity.
        Terrain::GrasslandShield | Terrain::Volcano => return None,
    })
}

pub(super) fn validate_freeciv_terrain_mappings(
    terrains: &[TerrainMapgenProperties],
) -> Result<(), String> {
    for terrain in terrains {
        if civ3_terrain_for_freeciv_rule(&terrain.rule_name).is_none()
            && (terrain.starter
                || [
                    terrain.mountainous,
                    terrain.green,
                    terrain.foliage,
                    terrain.tropical,
                    terrain.temperate,
                    terrain.cold,
                    terrain.frozen,
                    terrain.wet,
                    terrain.dry,
                ]
                .into_iter()
                .any(|weight| weight > 0))
        {
            return Err(format!(
                "Freeciv terrain `{}` has mapgen properties or Starter eligibility but no Civ III terrain mapping",
                terrain.rule_name
            ));
        }
    }
    Ok(())
}

pub(super) fn civ3_starter_terrains(terrains: &[TerrainMapgenProperties]) -> Vec<Terrain> {
    terrains
        .iter()
        .filter(|terrain| terrain.starter)
        .filter_map(|terrain| civ3_terrain_for_freeciv_rule(&terrain.rule_name))
        .collect()
}

/// Resolve CivCraft's generated terrain identity back to the C2C3 terrain
/// record used to generate it. Flood Plains and Grassland (Shield) retain
/// Civ III presentation variants, while their Freeciv scoring base comes
/// from Desert and Grassland respectively.
pub(super) fn start_scoring_properties(
    terrain: Terrain,
    terrains: &[TerrainMapgenProperties],
) -> Option<(&TerrainMapgenProperties, Terrain)> {
    let source_terrain = match terrain {
        Terrain::FloodPlains => Terrain::Desert,
        Terrain::GrasslandShield => Terrain::Grassland,
        terrain => terrain,
    };
    terrains
        .iter()
        .find(|properties| {
            civ3_terrain_for_freeciv_rule(&properties.rule_name) == Some(source_terrain)
        })
        .map(|properties| (properties, source_terrain))
}

/// Apply Freeciv's property-weighted terrain selection to the C2C3 records,
/// then translate the chosen identity into CivCraft's retained Civ III terrain
/// catalog. Selection constraints are relaxed in the same order as
/// `mapgen_utils.c::pick_terrain` when a ruleset has no eligible terrain.
fn pick_freeciv_terrain(
    terrains: &[TerrainMapgenProperties],
    target: Option<TerrainMapgenProperty>,
    prefer: Option<TerrainMapgenProperty>,
    avoid: Option<TerrainMapgenProperty>,
    rng: &mut MapRng,
) -> Option<Terrain> {
    let eligible = |terrain: &&TerrainMapgenProperties| {
        civ3_terrain_for_freeciv_rule(&terrain.rule_name).is_some()
            && !avoid.is_some_and(|property| property.value(terrain) > 0)
            && !prefer.is_some_and(|property| property.value(terrain) == 0)
    };
    let total_weight = terrains
        .iter()
        .filter(eligible)
        .map(|terrain| target.map_or(1, |property| property.value(terrain)))
        .sum::<i32>();

    if total_weight <= 0 {
        if prefer.is_some() {
            return pick_freeciv_terrain(terrains, target, None, avoid, rng);
        }
        if avoid.is_some() {
            return pick_freeciv_terrain(terrains, target, prefer, None, rng);
        }
        if target.is_some() {
            return pick_freeciv_terrain(terrains, None, prefer, avoid, rng);
        }
        return None;
    }

    let mut draw = rng.below(total_weight);
    for terrain in terrains.iter().filter(eligible) {
        let weight = target.map_or(1, |property| property.value(terrain));
        if draw < weight {
            return civ3_terrain_for_freeciv_rule(&terrain.rule_name);
        }
        draw -= weight;
    }
    None
}

pub(super) fn freeciv_terrain_percentages(
    settings: MapGenerationSettings,
    tile_count: usize,
) -> (i32, i32, i32, i32, i32, i32) {
    const MAX_COLATITUDE: i32 = 1000;
    let (frozen_level, _, tropical_level) =
        super::temperature::freeciv_temperature_thresholds(settings.temperature, tile_count);
    let ice_base = frozen_level / 2;

    let polar = 2 * ice_base * settings.landmass_percent / MAX_COLATITUDE;
    let mountain_factor = (100.0 - polar as f32 - 30.0 * 0.8) / 10_000.0;
    let factor = (100.0 - polar as f32 - settings.steepness as f32 * 0.8) / 10_000.0;
    let mountain = (mountain_factor * settings.steepness as f32 * 90.0) as i32;
    let mut forest = (factor * (settings.wetness * 40 + 700) as f32) as i32;
    let jungle = forest * (MAX_COLATITUDE - tropical_level) / (MAX_COLATITUDE * 2);
    forest -= jungle;
    let swamp =
        (factor * (settings.wetness * 12 - 150 + settings.temperature * 10).max(0) as f32) as i32;
    let desert = (factor
        * (settings.temperature * 15 - 250 + (100 - settings.wetness) * 10).max(0) as f32)
        as i32;
    let river = (100 - polar) * (3 + settings.wetness / 12) / 100;
    (mountain, forest, jungle, swamp, desert, river)
}

fn freeciv_ocean_depth_majority_threshold() -> usize {
    (2 * CIV3_DIRS.len()).div_ceil(3)
}

struct PatchGrowth<'a, Eligible> {
    generator: &'a super::Generator,
    remaining: &'a mut usize,
    available: &'a mut [bool],
    terrain: &'a mut [Terrain],
    heights: &'a [i32],
    eligible: &'a Eligible,
    rng: &'a mut MapRng,
    cardinal_growth: bool,
}

// Finds a water threshold over the current scalar field. OpenSimplex keeps its
// sampled 8-bit values; the Freeciv fractal path uses its CDF-adjusted values.
// Water occupies heights strictly below the returned threshold.
fn find_sea_level(heights: &[i32], percent_water: i32) -> i32 {
    let mut low = heights.iter().copied().min().unwrap_or(0);
    let mut high = heights.iter().copied().max().unwrap_or(0);
    let mut threshold = (low + high) / 2;
    while low < high - 1 {
        let water_tiles = heights.iter().filter(|&&height| height < threshold).count();
        let current_percent = (water_tiles * 100 / heights.len()) as i32;
        if current_percent < percent_water {
            low = threshold;
        } else {
            high = threshold;
        }
        threshold = (low + high) / 2;
    }
    threshold
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
        let neighbors: Vec<_> = if self.cardinal_growth {
            generator.cardinal_neighbors(tile).collect()
        } else {
            generator.neighbors(tile).collect()
        };
        for next in neighbors {
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
    #[expect(
        clippy::too_many_arguments,
        reason = "Terrain generation consumes independent map fields and the deterministic RNG."
    )]
    pub(super) fn terrain_pass(
        &self,
        heights: &[i32],
        water_heights: &[i32],
        relief: &[i32],
        biome_field: &[i32],
        land_mask: Option<&[bool]>,
        freeciv_temperature: Option<&TemperatureMap>,
        freeciv_terrain_properties: Option<&[TerrainMapgenProperties]>,
        ocean_coverage: i32,
        rng: &mut MapRng,
    ) -> (Vec<Terrain>, Vec<TerrainBase>, Vec<i32>) {
        let mut dummy_temperature: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| self.colatitude(tile))
            .collect();
        rank_field(&mut dummy_temperature, 1000);
        let sea_level = find_sea_level(water_heights, ocean_coverage);
        let medium_water_level = find_sea_level(water_heights, ocean_coverage * 9 / 10);
        let deep_water_level = find_sea_level(water_heights, ocean_coverage * 7 / 10);
        let shore = 1000 * ocean_coverage / 100;
        let lowland = shore + (1000 - shore) * 28 / 100;
        let mut terrain: Vec<_> = (0..self.wrap.tile_count())
            .map(|tile| {
                let height = water_heights[tile];
                let is_land = land_mask
                    .map(|mask| mask[tile])
                    .unwrap_or(height > sea_level);
                if is_land {
                    return Terrain::Plains;
                }
                if height > medium_water_level {
                    Terrain::Coast
                } else if height > deep_water_level {
                    Terrain::Sea
                } else {
                    Terrain::Ocean
                }
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
        if let (Some(temperature_map), Some(terrain_properties)) =
            (freeciv_temperature, freeciv_terrain_properties)
        {
            self.assign_freeciv_relief(
                &mut terrain,
                &mut available,
                heights,
                temperature_map,
                terrain_properties,
                rng,
            );
        } else {
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
        let source_percentages =
            freeciv_temperature
                .zip(freeciv_terrain_properties)
                .map(|(temperature, _)| {
                    freeciv_terrain_percentages(temperature.settings, self.wrap.tile_count())
                });
        let source_lowland = source_percentages.map(|(_, _, _, swamp, _, _)| {
            let shore = freeciv_temperature
                .expect("source percentage inputs include a temperature map")
                .settings
                .shore_level();
            shore + 4 * swamp * (1000 - shore) / 100
        });
        let lowland = source_lowland.unwrap_or(lowland);

        // Freeciv's mapgen quotas are percentages of the non-mountain land.
        // OpenSimplex retains its separate Civ III world-setup heuristics.
        let mut patches = [0_usize; 5];
        if let Some((mountain, forest, jungle, swamp, desert, _)) = source_percentages {
            let denominator = (100 - mountain).max(1) as usize;
            patches = [
                remaining * forest.max(0) as usize / denominator,
                remaining * jungle.max(0) as usize / denominator,
                remaining * swamp.max(0) as usize / denominator,
                remaining * desert.max(0) as usize / denominator,
                0,
            ];
        } else {
            patches[..3].copy_from_slice(&[
                remaining * forest_share / 80,
                remaining * jungle_share / 80,
                remaining * marsh_share / 80,
            ]);
        }
        let mut plains = remaining.saturating_sub(patches.iter().sum::<usize>());
        while plains > 0 || patches.iter().any(|&count| count > 0) {
            for pass in 0..patches.len() {
                if patches[pass] == 0 {
                    continue;
                }
                let eligible = |tile: usize| {
                    let heat = temperature[tile];
                    let source_zone = freeciv_temperature.map(|map| map.get(tile));
                    let dry = freeciv_temperature
                        .map(|map| map.is_dry(self, tile, &terrain_water))
                        .unwrap_or_else(|| {
                            (623..=832).contains(&self.colatitude(tile))
                                && self
                                    .neighbors(tile)
                                    .filter(|&next| terrain_water[next])
                                    .count()
                                    <= dry_water_limit
                        });
                    match pass {
                        0 => source_zone
                            .map(TemperatureType::is_not_frozen)
                            .unwrap_or(heat >= VERY_COLD),
                        1 => source_zone
                            .map(|zone| zone == TemperatureType::Tropical)
                            .unwrap_or(heat >= TROPICAL),
                        2 => {
                            source_zone
                                .map(TemperatureType::is_hot)
                                .unwrap_or(heat >= COLD)
                                && !dry
                                && heights[tile] < lowland
                        }
                        3 => {
                            source_zone.is_some_and(TemperatureType::is_not_frozen)
                                && dry
                                && heights[tile] >= lowland
                        }
                        _ => {
                            source_zone.is_some_and(TemperatureType::is_not_frozen)
                                && heights[tile] >= lowland
                        }
                    }
                };
                if let Some(seed) = random_tile(self.wrap.tile_count(), rng, |tile| {
                    available[tile] && eligible(tile)
                }) {
                    let (legacy_biome, weight, spread) = [
                        (Terrain::Forest, 50, 60),
                        (Terrain::Jungle, 50, 50),
                        (Terrain::Marsh, 100, 50),
                        (Terrain::Desert, 80, 80),
                        (Terrain::Desert, 40, 40),
                    ][pass];
                    let biome = if let Some(terrain_properties) = freeciv_terrain_properties {
                        let (target, prefer, avoid) = match pass {
                            0 => (
                                Some(TerrainMapgenProperty::Foliage),
                                Some(TerrainMapgenProperty::Temperate),
                                Some(TerrainMapgenProperty::Tropical),
                            ),
                            1 => (
                                Some(TerrainMapgenProperty::Foliage),
                                Some(TerrainMapgenProperty::Tropical),
                                Some(TerrainMapgenProperty::Cold),
                            ),
                            _ => (
                                Some(TerrainMapgenProperty::Dry),
                                Some(TerrainMapgenProperty::Tropical),
                                if pass == 3 {
                                    Some(TerrainMapgenProperty::Cold)
                                } else {
                                    Some(TerrainMapgenProperty::Wet)
                                },
                            ),
                        };
                        let (target, prefer, avoid) = if pass == 2 {
                            (
                                Some(TerrainMapgenProperty::Wet),
                                None,
                                Some(TerrainMapgenProperty::Foliage),
                            )
                        } else {
                            (target, prefer, avoid)
                        };
                        pick_freeciv_terrain(terrain_properties, target, prefer, avoid, rng)
                            .expect("C2C3 terrain properties must select a mapped land terrain")
                    } else {
                        rng.below(weight);
                        legacy_biome
                    };
                    PatchGrowth {
                        generator: self,
                        remaining: &mut patches[pass],
                        available: &mut available,
                        terrain: &mut terrain,
                        heights,
                        eligible: &eligible,
                        rng,
                        cardinal_growth: freeciv_temperature.is_some(),
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
                terrain[tile] = if let (Some(temperature), Some(properties)) =
                    (freeciv_temperature, freeciv_terrain_properties)
                {
                    let (target, prefer) = match temperature.get(tile) {
                        TemperatureType::Frozen => (TerrainMapgenProperty::Frozen, None),
                        TemperatureType::Cold => (TerrainMapgenProperty::Cold, None),
                        TemperatureType::Temperate | TemperatureType::Tropical => (
                            TerrainMapgenProperty::Temperate,
                            Some(TerrainMapgenProperty::Green),
                        ),
                    };
                    pick_freeciv_terrain(
                        properties,
                        Some(target),
                        prefer,
                        Some(TerrainMapgenProperty::Mountainous),
                        rng,
                    )
                    .expect("C2C3 temperature zone must select a mapped plains terrain")
                } else if temperature[tile] < VERY_COLD {
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
                if freeciv_terrain_properties.is_some() {
                    base_terrain[tile] = match terrain[tile] {
                        Terrain::Grassland => TerrainBase::Grassland,
                        Terrain::Plains => TerrainBase::Plains,
                        Terrain::Desert => TerrainBase::Desert,
                        Terrain::Tundra => TerrainBase::Tundra,
                        Terrain::Glacier => TerrainBase::Ice,
                        _ => base_terrain[tile],
                    };
                }
                available[tile] = false;
                plains -= 1;
            }
        }
        (terrain, base_terrain, temperature)
    }

    fn assign_freeciv_relief(
        &self,
        terrain: &mut [Terrain],
        available: &mut [bool],
        heights: &[i32],
        temperature: &TemperatureMap,
        terrain_properties: &[TerrainMapgenProperties],
        rng: &mut MapRng,
    ) {
        let shore = temperature.settings.shore_level();
        let mountain_level = (1000 - shore) * (100 - temperature.settings.steepness) / 100 + shore;

        for tile in 0..terrain.len() {
            if terrain[tile].is_water() {
                continue;
            }
            let too_high = heights[tile] > mountain_level
                && (rng.below(10) > 5
                    || !self.freeciv_terrain_is_too_high(tile, mountain_level, heights));
            let too_flat = self.freeciv_area_is_too_flat(tile, mountain_level, heights, shore);
            if too_high || too_flat {
                let (prefer, avoid) = if temperature.get(tile).is_hot() {
                    if rng.below(10) < 4 {
                        (None, None)
                    } else {
                        (Some(TerrainMapgenProperty::Green), None)
                    }
                } else if rng.below(10) < 8 {
                    (None, Some(TerrainMapgenProperty::Green))
                } else {
                    (None, None)
                };
                terrain[tile] = pick_freeciv_terrain(
                    terrain_properties,
                    Some(TerrainMapgenProperty::Mountainous),
                    prefer,
                    avoid,
                    rng,
                )
                .expect("C2C3 mountainous property must select a mapped land terrain");
            } else {
                terrain[tile] = Terrain::Grassland;
                available[tile] = true;
            }
        }
    }

    fn freeciv_terrain_is_too_high(
        &self,
        tile: usize,
        mountain_level: i32,
        heights: &[i32],
    ) -> bool {
        let allowance = (1000 - mountain_level) / 5;
        self.within(tile, 1)
            .all(|neighbor| heights[neighbor] + allowance >= mountain_level)
    }

    fn freeciv_area_is_too_flat(
        &self,
        tile: usize,
        mountain_level: i32,
        heights: &[i32],
        shore: i32,
    ) -> bool {
        let mut higher_than_me = 0;
        for neighbor in self.within(tile, 2) {
            if heights[neighbor] > mountain_level {
                return false;
            }
            if heights[neighbor] > heights[tile] {
                if self.distance(tile, neighbor) == 1 {
                    return false;
                }
                higher_than_me += 1;
                if higher_than_me > 2 {
                    return false;
                }
            }
        }
        (mountain_level - shore) * higher_than_me <= (heights[tile] - shore) * 4
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

        self.finish_lakes(terrain);
    }

    /// Assign the initial C2C3 ocean terrain using Freeciv `make_land`'s
    /// depth estimate and neighbor correction. C2C3 `Lake` has depth zero and
    /// is excluded by Freeciv's `pick_ocean` minimum of one; its mapped Civ III
    /// label is Coast and is applied to enclosed lakes by `finish_lakes`.
    pub(super) fn assign_freeciv_water_depths(
        &self,
        terrain: &mut [Terrain],
        heights: &[i32],
        land_mask: &[bool],
        shore_level: i32,
        depths: OceanDepthSettings,
    ) {
        debug_assert_eq!(terrain.len(), self.wrap.tile_count());
        debug_assert_eq!(heights.len(), self.wrap.tile_count());
        debug_assert_eq!(land_mask.len(), self.wrap.tile_count());
        debug_assert!(shore_level > 0);

        for tile in 0..terrain.len() {
            if land_mask[tile] {
                continue;
            }

            let mut depth = (shore_level - heights[tile]) * 100 / shore_level;
            let mut ocean_neighbors = 0;
            let mut land_neighbors = 0;
            for neighbor in self.neighbors(tile) {
                if land_mask[neighbor] {
                    land_neighbors += 1;
                    break;
                }
                ocean_neighbors += 1;
            }
            depth +=
                30 * (ocean_neighbors - land_neighbors) / (ocean_neighbors + land_neighbors).max(1);
            depth = depth.min(FREECIV_OCEAN_DEPTH_MAXIMUM);
            terrain[tile] = depths.civ3_ocean_for_depth(depth);
        }
    }

    /// Reproduce Freeciv's two `smooth_water_depth` passes on CivCraft's
    /// horizontal-wrap, bounded-Y topology. Distances use CivCraft's neighbor
    /// graph; the source uses its ISO-hex real-distance helper.
    pub(super) fn finish_freeciv_water_depths(
        &self,
        terrain: &mut [Terrain],
        depths: OceanDepthSettings,
        rng: &mut MapRng,
    ) {
        let distance_to_land = self.distance_to_land(terrain);
        for tile in 0..terrain.len() {
            if !matches!(terrain[tile], Terrain::Sea | Terrain::Ocean) {
                continue;
            }
            let distance = distance_to_land[tile];
            if distance <= FREECIV_OCEAN_DISTANCE_MAX {
                // Freeciv requests a terrain at `dist * 25 + fc_rand(15)`.
                // MapRng draws the same zero-based range from the map stream.
                let depth = distance as i32 * FREECIV_OCEAN_DEPTH_STEP
                    + rng.below(FREECIV_OCEAN_DEPTH_RANDOM_RANGE);
                terrain[tile] = depths.civ3_ocean_for_depth(depth);
            }
        }

        // Freeciv keeps an ocean class when at least two thirds of its valid
        // neighbors agree. Round up for CivCraft's eight directions, so a
        // discrete neighbor count cannot fall below the source proportion.
        let majority = freeciv_ocean_depth_majority_threshold();
        for tile in 0..terrain.len() {
            if !matches!(terrain[tile], Terrain::Sea | Terrain::Ocean) {
                continue;
            }
            for ocean_type in [Terrain::Sea, Terrain::Ocean] {
                let count = self
                    .neighbors(tile)
                    .filter(|&neighbor| terrain[neighbor] == ocean_type)
                    .count();
                if count >= majority {
                    terrain[tile] = ocean_type;
                    break;
                }
            }
        }

        self.finish_lakes(terrain);
    }

    /// C2C3 `regenerate_lakes` uses lake_max_size=14. CivCraft has no separate
    /// Civ III lake terrain, so map those inland freshwater basins to Coast.
    pub(super) fn finish_freeciv_lakes(&self, terrain: &mut [Terrain]) {
        self.finish_lakes(terrain);
    }

    fn distance_to_land(&self, terrain: &[Terrain]) -> Vec<usize> {
        let unreachable = FREECIV_OCEAN_DISTANCE_MAX + 1;
        let mut distances = vec![unreachable; terrain.len()];
        let mut queue = VecDeque::new();
        for (tile, &ground) in terrain.iter().enumerate() {
            if !ground.is_water() {
                distances[tile] = 0;
                queue.push_back(tile);
            }
        }
        while let Some(tile) = queue.pop_front() {
            let next_distance = distances[tile] + 1;
            if next_distance > FREECIV_OCEAN_DISTANCE_MAX {
                continue;
            }
            for neighbor in self.neighbors(tile) {
                if next_distance < distances[neighbor] {
                    distances[neighbor] = next_distance;
                    queue.push_back(neighbor);
                }
            }
        }
        distances
    }

    fn finish_lakes(&self, terrain: &mut [Terrain]) {
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

#[cfg(test)]
mod freeciv_terrain_tests {
    use super::*;

    fn settings() -> MapGenerationSettings {
        crate::rules::freeciv::RulesetData::campaign_default()
            .map_generation_settings()
            .unwrap()
    }

    #[test]
    fn c2c3_map_terrain_percentages_follow_freeciv_defaults() {
        assert_eq!(
            freeciv_terrain_percentages(settings(), 36 * 48),
            (20, 16, 4, 7, 8, 6)
        );
    }

    #[test]
    fn c2c3_starter_flags_drive_civ3_terrain_eligibility() {
        let terrains = crate::rules::freeciv::RulesetData::campaign_default()
            .terrain_mapgen_properties()
            .unwrap();
        assert_eq!(
            civ3_starter_terrains(&terrains),
            [
                Terrain::Forest,
                Terrain::Grassland,
                Terrain::Hills,
                Terrain::Plains,
            ]
        );
    }

    #[test]
    fn c2c3_property_selection_preserves_civ3_terrain_identity_mapping() {
        let terrains = crate::rules::freeciv::RulesetData::campaign_default()
            .terrain_mapgen_properties()
            .unwrap();
        let frozen = pick_freeciv_terrain(
            &terrains,
            Some(TerrainMapgenProperty::Frozen),
            None,
            None,
            &mut MapRng::new(1),
        );
        assert_eq!(frozen, Some(Terrain::Glacier));

        let swamp = terrains
            .iter()
            .find(|terrain| terrain.rule_name == "Swamp")
            .unwrap();
        assert_eq!(
            civ3_terrain_for_freeciv_rule(&swamp.rule_name),
            Some(Terrain::Marsh)
        );
    }

    #[test]
    fn c2c3_label_adapter_preserves_source_terrain_names() {
        use C2c3MapLabelTerrain as Source;

        for (civ3, source) in [
            (Terrain::Coast, Source::Lake),
            (Terrain::Sea, Source::Ocean),
            (Terrain::Ocean, Source::DeepOcean),
            (Terrain::Marsh, Source::Swamp),
            (Terrain::Glacier, Source::Glacier),
            (Terrain::Tundra, Source::Tundra),
            (Terrain::Desert, Source::Desert),
            (Terrain::FloodPlains, Source::Desert),
            (Terrain::Plains, Source::Plains),
            (Terrain::Grassland, Source::Grassland),
            (Terrain::Jungle, Source::Jungle),
            (Terrain::Forest, Source::Forest),
            (Terrain::Hills, Source::Hills),
            (Terrain::Mountains, Source::Mountains),
        ] {
            assert_eq!(c2c3_map_label_terrain_for_civ3(civ3), Some(source));
        }
        assert_eq!(
            c2c3_map_label_terrain_for_civ3(Terrain::GrasslandShield),
            None
        );
        assert_eq!(c2c3_map_label_terrain_for_civ3(Terrain::Volcano), None);
    }

    #[test]
    fn unmapped_generated_terrain_is_reported_explicitly() {
        let mut terrains = crate::rules::freeciv::RulesetData::campaign_default()
            .terrain_mapgen_properties()
            .unwrap();
        let desert = terrains
            .iter_mut()
            .find(|terrain| terrain.rule_name == "Desert")
            .unwrap();
        desert.rule_name = "CivCraft Desert Variant".to_owned();
        assert!(validate_freeciv_terrain_mappings(&terrains).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::{find_sea_level, freeciv_ocean_depth_majority_threshold};

    #[test]
    fn freeciv_ocean_smoothing_requires_at_least_two_thirds_of_eight_neighbors() {
        assert_eq!(freeciv_ocean_depth_majority_threshold(), 6);
    }

    #[test]
    fn sea_levels_match_reference_threshold_search() {
        let heights: Vec<_> = (0..=255).collect();
        assert_eq!(find_sea_level(&heights, 60), 153);
        assert_eq!(find_sea_level(&heights, 54), 138);
        assert_eq!(find_sea_level(&heights, 42), 107);
    }
}
