//! Freeciv's categorical map-generation temperature field.
//!
//! Freeciv uses a continuous intermediate value only to assign one of four
//! temperature categories. Terrain placement consumes those categories, not
//! the intermediate values.

use super::*;

const FREECIV_MAX_COLATITUDE: i32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TemperatureType {
    Frozen,
    Cold,
    Temperate,
    Tropical,
}

impl TemperatureType {
    pub(super) fn is_frozen_or_cold(self) -> bool {
        matches!(self, Self::Frozen | Self::Cold)
    }

    pub(super) fn is_not_frozen(self) -> bool {
        !matches!(self, Self::Frozen)
    }

    pub(super) fn is_hot(self) -> bool {
        matches!(self, Self::Temperate | Self::Tropical)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TemperatureMap {
    zones: Vec<TemperatureType>,
    pub(super) settings: MapGenerationSettings,
    dry_min: i32,
    dry_max: i32,
}

impl TemperatureMap {
    pub(super) fn get(&self, tile: usize) -> TemperatureType {
        self.zones[tile]
    }

    pub(super) fn is_dry(&self, generator: &Generator, tile: usize, water: &[bool]) -> bool {
        let colatitude = generator.colatitude(tile);
        if colatitude <= self.dry_min || colatitude > self.dry_max {
            return false;
        }
        let (mut ocean_neighbors, mut neighbor_count) = (0, 0);
        for neighbor in generator.neighbors(tile) {
            neighbor_count += 1;
            if water[neighbor] {
                ocean_neighbors += 1;
            }
        }
        let ocean_percent = if ocean_neighbors == 0 || neighbor_count == 0 {
            0
        } else {
            ocean_neighbors * 100 / neighbor_count
        };
        ocean_percent <= 35
    }
}

impl Generator {
    /// Build the `create_tmap()` temperature categories for the Freeciv
    /// generator. When `real` is false, Freeciv uses latitude alone; start
    /// position selection does this after the height map has been discarded.
    pub(super) fn freeciv_temperature_map(
        &self,
        real: bool,
        heights: Option<&[i32]>,
        land_mask: Option<&[bool]>,
        settings: MapGenerationSettings,
    ) -> TemperatureMap {
        let tile_count = self.wrap.tile_count();
        let mut values: Vec<_> = (0..tile_count).map(|tile| self.colatitude(tile)).collect();
        let (real_min, real_max) = values
            .iter()
            .copied()
            .fold((i32::MAX, i32::MIN), |(min, max), value| {
                (min.min(value), max.max(value))
            });

        if real {
            let heights = heights.expect("real Freeciv temperature needs the height map");
            let land_mask = land_mask.expect("real Freeciv temperature needs the land mask");
            debug_assert_eq!(heights.len(), tile_count);
            debug_assert_eq!(land_mask.len(), tile_count);

            let shore = settings.shore_level();
            let height_range = FREECIV_MAX_COLATITUDE - shore;
            for tile in 0..tile_count {
                let colatitude = values[tile];
                let (mut ocean_neighbors, mut neighbor_count) = (0, 0);
                for neighbor in self.neighbors(tile) {
                    neighbor_count += 1;
                    if !land_mask[neighbor] {
                        ocean_neighbors += 1;
                    }
                }
                // `count_terrain_class_near_tile(..., percentage=true)`
                // leaves the value at zero when there are no matching tiles.
                let ocean_percent = if ocean_neighbors == 0 || neighbor_count == 0 {
                    0
                } else {
                    ocean_neighbors * 100 / neighbor_count
                };
                let height = if height_range > 0 {
                    -0.3 * f64::from((heights[tile] - shore).max(0)) / f64::from(height_range)
                } else {
                    0.0
                };
                // Preserve the integer divisions in Freeciv's C expression.
                let temperate =
                    0.15 * f64::from(
                        (settings.temperature / 100 - colatitude / FREECIV_MAX_COLATITUDE)
                            * 2
                            * ocean_percent.min(50),
                    ) / 100.0;
                values[tile] = (f64::from(colatitude) * (1.0 + temperate) * (1.0 + height)) as i32;
            }

            if real_max - real_min >= FREECIV_MAX_COLATITUDE * 2 / 5 {
                rank_field_between(&mut values, real_min, real_max);
            }
        }

        let (frozen_level, cold_level, tropical_level) =
            freeciv_temperature_thresholds(settings.temperature, tile_count);
        let dry_min = (FREECIV_MAX_COLATITUDE * (7300 - settings.temperature * 18) / 10000).max(0);
        let dry_max = FREECIV_MAX_COLATITUDE * (7300 + settings.temperature * 17) / 10000;
        TemperatureMap {
            zones: values
                .into_iter()
                .map(|value| {
                    if value >= tropical_level {
                        TemperatureType::Tropical
                    } else if value >= cold_level {
                        TemperatureType::Temperate
                    } else if value >= frozen_level {
                        TemperatureType::Cold
                    } else {
                        TemperatureType::Frozen
                    }
                })
                .collect(),
            settings,
            dry_min,
            dry_max,
        }
    }
}

fn rank_field_between(values: &mut [i32], minimum: i32, maximum: i32) {
    if values.is_empty() {
        return;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let range = maximum - minimum;
    for value in values {
        let rank = sorted.partition_point(|other| other <= value);
        *value = minimum + rank as i32 * range / sorted.len() as i32;
    }
}

pub(super) fn freeciv_temperature_thresholds(
    temperature: i32,
    tile_count: usize,
) -> (i32, i32, i32) {
    let cold_level = (FREECIV_MAX_COLATITUDE * (60 * 7 - temperature * 6) / 700).max(0);
    let tropical_level = (FREECIV_MAX_COLATITUDE * (143 * 7 - temperature * 10) / 700)
        .clamp(0, FREECIV_MAX_COLATITUDE * 9 / 10);

    // C2C3 leaves `separatepoles` unset, so use Freeciv's server default
    // (TRUE). The current campaign topology keeps default full-range latitude
    // bounds; Freeciv's short-latitude correction therefore does not apply.
    let square_size = ((tile_count / 1000) as f64).sqrt() as i32;
    let square_size = square_size.max(1);
    let ice_base = ((100 * cold_level / 3 - FREECIV_MAX_COLATITUDE).max(0)
        + FREECIV_MAX_COLATITUDE * square_size)
        / (100 * square_size);

    (2 * ice_base, cold_level, tropical_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generator(wrap: MapWrap) -> Generator {
        Generator {
            wrap,
            settings: Civ3WorldSettings::default(),
            civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
        }
    }

    fn ruleset_settings() -> MapGenerationSettings {
        crate::rules::freeciv::RulesetData::campaign_default()
            .map_generation_settings()
            .unwrap()
    }

    #[test]
    fn c2c3_temperature_setting_produces_freeciv_zone_boundaries() {
        assert_eq!(freeciv_temperature_thresholds(60, 36 * 48), (56, 85, 572));
    }

    #[test]
    fn freeciv_ice_base_uses_default_separate_poles_for_larger_maps() {
        assert_eq!(freeciv_temperature_thresholds(60, 80 * 80), (38, 85, 572));
        assert_eq!(freeciv_temperature_thresholds(60, 100 * 100), (32, 85, 572));
    }

    #[test]
    fn latitude_only_temperature_map_matches_freeciv_start_filter_zones() {
        let generator = generator(MapWrap::X);
        let settings = ruleset_settings();
        let map = generator.freeciv_temperature_map(false, None, None, settings);
        let x = generator.wrap.width() / 2;
        assert_eq!(map.get(generator.index(x, 0)), TemperatureType::Frozen);
        assert_eq!(map.get(generator.index(x, 2)), TemperatureType::Temperate);
        assert_eq!(map.get(generator.index(x, 12)), TemperatureType::Temperate);
        assert_eq!(map.get(generator.index(x, 18)), TemperatureType::Tropical);
        assert!(!map.get(generator.index(x, 2)).is_frozen_or_cold());
    }

    #[test]
    fn dry_zone_uses_freeciv_latitude_and_adjacent_ocean_percentage() {
        let generator = generator(MapWrap::X);
        let map = generator.freeciv_temperature_map(false, None, None, ruleset_settings());
        let tile = generator.index(generator.wrap.width() / 2, 18);
        let mut water = vec![false; generator.wrap.tile_count()];
        assert!(map.is_dry(&generator, tile, &water));

        for neighbor in generator.neighbors(tile).take(4) {
            water[neighbor] = true;
        }
        assert!(!map.is_dry(&generator, tile, &water));
    }
}
