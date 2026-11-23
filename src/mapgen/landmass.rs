//! Per-generator steps of the shared map-generation pipeline.
//!
//! `Generator::generate_with_landmass` runs the passes every generator shares
//! (tiles, rivers, resources, huts and start positions). Everything that
//! differs between generators lives behind [`Landmass`], so adding a generator
//! means adding one implementation instead of editing the shared driver.
//! RNG draw order inside each method must stay stable: seeded output and the
//! reference fixtures depend on it.

use super::defaults::{
    MapGenerationSettings, StartPositionScoringSettings, TerrainMapgenProperties,
};
use super::temperature::TemperatureMap;
use super::terrain::TerrainSource;
use super::{
    FractalConfig, Generator, MAPGEN_MAX_ATTEMPTS, MapRng, Terrain, WorldSettings, draw_map_seed,
    fractal_land_mask, rank_field,
};
use crate::random::GameRng;
use crate::world::GeneratedTile;

/// Heights and ocean share of one landmass attempt.
pub(super) struct HeightField {
    /// Field the water depth passes rank against.
    pub(super) water_heights: Vec<i32>,
    /// Altitude stored on generated tiles.
    pub(super) heights: Vec<i32>,
    /// Land mask, for generators that derive one from the height field.
    pub(super) land_mask: Option<Vec<bool>>,
    pub(super) ocean_coverage: i32,
}

/// Inputs to resource, hut and start placement.
#[derive(Clone, Copy)]
pub(super) struct PlacementInputs<'a> {
    pub(super) map_settings: Option<MapGenerationSettings>,
    pub(super) terrain_properties: Option<&'a [TerrainMapgenProperties]>,
    pub(super) start_scoring_settings: Option<StartPositionScoringSettings>,
}

/// The steps of map generation that differ between generators.
pub(super) trait Landmass {
    /// Relief and biome noise fields consumed by the terrain pass, built once.
    fn terrain_fields(&self, generator: &Generator, map_seed: u64) -> (Vec<i32>, Vec<i32>);

    /// Temperature map used to score start positions, built once.
    fn start_temperature(&self, generator: &Generator) -> Option<TemperatureMap>;

    /// Placement inputs for resources, huts and starts.
    fn placement(&self) -> PlacementInputs<'_>;

    /// Seed of this attempt's map stream. May redraw `current_map_seed`.
    fn attempt_seed(
        &self,
        attempt: usize,
        map_seed: u64,
        current_map_seed: &mut u32,
        gameplay_rng: &mut GameRng,
    ) -> u64;

    /// Landmass heights for one attempt, consuming `rng` where the generator does.
    fn height_field(&self, generator: &Generator, water_seed: u64, rng: &mut MapRng)
    -> HeightField;

    /// Terrain-pass inputs for this attempt's heights.
    fn terrain_source(&self, generator: &Generator, field: &HeightField) -> TerrainSource<'_>;

    /// Water depth assignment that precedes river placement.
    fn assign_water(&self, generator: &Generator, terrain: &mut [Terrain], field: &HeightField);

    /// Water finishing that follows river placement.
    fn finish_water(&self, generator: &Generator, terrain: &mut [Terrain], rng: &mut MapRng);

    /// Work after a successful start-position search.
    fn after_starts(&self, generator: &Generator, tiles: &[GeneratedTile], start_rng: &mut GameRng);

    /// Handles a failed start-position search.
    fn on_failed_attempt(&self, attempt: usize, gameplay_rng: &mut GameRng, start_rng: GameRng);
}

/// Legacy OpenSimplex landmass, driven by the Civ III world choices.
pub(super) struct OpenSimplexLandmass(pub(super) WorldSettings);

impl Landmass for OpenSimplexLandmass {
    fn terrain_fields(&self, generator: &Generator, map_seed: u64) -> (Vec<i32>, Vec<i32>) {
        // OpenCiv3 seeds its relief noise independently from the landmass
        // field. Reuse the local deterministic field builder with its
        // documented seed offset as an explicit implementation approximation.
        let mut relief = generator.height_field(map_seed.wrapping_add(0xfeed), 0.2);
        let mut biome_field = generator.height_field(map_seed.wrapping_add(0xdad), 0.4);
        rank_field(&mut relief, 1000);
        rank_field(&mut biome_field, 1000);
        (relief, biome_field)
    }

    fn start_temperature(&self, _generator: &Generator) -> Option<TemperatureMap> {
        None
    }

    fn placement(&self) -> PlacementInputs<'_> {
        PlacementInputs {
            map_settings: None,
            terrain_properties: None,
            start_scoring_settings: None,
        }
    }

    fn attempt_seed(
        &self,
        attempt: usize,
        map_seed: u64,
        _current_map_seed: &mut u32,
        _gameplay_rng: &mut GameRng,
    ) -> u64 {
        map_seed.wrapping_add(0x1234_u64.wrapping_mul(attempt as u64))
    }

    fn height_field(
        &self,
        generator: &Generator,
        water_seed: u64,
        _rng: &mut MapRng,
    ) -> HeightField {
        let world = self.0;
        let mut field = generator.height_field(water_seed, world.landform.noise_scale());
        // Suppress land gradually toward the bounded polar edges before
        // applying the water percentile. This keeps the requested ocean
        // coverage while avoiding a hard, straight post-generation shelf.
        generator.taper_polar_landmass(&mut field, water_seed);
        let mut heights = field.clone();
        rank_field(&mut heights, 1000);
        HeightField {
            water_heights: field,
            heights,
            land_mask: None,
            ocean_coverage: world.ocean_coverage.percent(),
        }
    }

    fn terrain_source(&self, _generator: &Generator, _field: &HeightField) -> TerrainSource<'_> {
        TerrainSource::OpenSimplex(self.0)
    }

    fn assign_water(&self, _generator: &Generator, _terrain: &mut [Terrain], _field: &HeightField) {
    }

    fn finish_water(&self, generator: &Generator, terrain: &mut [Terrain], rng: &mut MapRng) {
        generator.finish_water(terrain, rng);
    }

    fn after_starts(
        &self,
        _generator: &Generator,
        _tiles: &[GeneratedTile],
        _start_rng: &mut GameRng,
    ) {
    }

    fn on_failed_attempt(&self, _attempt: usize, _gameplay_rng: &mut GameRng, _start_rng: GameRng) {
        // The legacy adapter retains its arithmetic retry stream.
    }
}

/// Freeciv fractal landmass with the ruleset terrain passes.
pub(super) struct FractalLandmass<'a>(pub(super) FractalConfig<'a>);

impl Landmass for FractalLandmass<'_> {
    fn terrain_fields(&self, generator: &Generator, _map_seed: u64) -> (Vec<i32>, Vec<i32>) {
        // The Freeciv terrain pass uses the actual height and temperature maps
        // instead of CivCraft's separate relief and biome noise fields.
        (
            vec![0; generator.wrap.tile_count()],
            vec![0; generator.wrap.tile_count()],
        )
    }

    fn start_temperature(&self, generator: &Generator) -> Option<TemperatureMap> {
        Some(generator.generate_temperature_map(false, None, None, self.0.settings))
    }

    fn placement(&self) -> PlacementInputs<'_> {
        PlacementInputs {
            map_settings: Some(self.0.settings),
            terrain_properties: Some(self.0.terrain_properties),
            start_scoring_settings: Some(self.0.start_scoring_settings),
        }
    }

    fn attempt_seed(
        &self,
        attempt: usize,
        _map_seed: u64,
        current_map_seed: &mut u32,
        gameplay_rng: &mut GameRng,
    ) -> u64 {
        if attempt > 0 {
            // Each native retry calls map_fractal_generate() again, which
            // draws a new seed from the restored gameplay stream.
            *current_map_seed = draw_map_seed(gameplay_rng);
        }
        u64::from(*current_map_seed)
    }

    fn height_field(
        &self,
        generator: &Generator,
        water_seed: u64,
        rng: &mut MapRng,
    ) -> HeightField {
        let settings = self.0.settings;
        let mut field =
            generator.fractal_height_field(rng, self.0.divisions, settings.landmass_percent, true);
        generator.taper_polar_landmass(&mut field, water_seed);
        rank_field(&mut field, 1000);
        let land_mask = fractal_land_mask(&field, settings);
        HeightField {
            water_heights: field.clone(),
            heights: field,
            land_mask: Some(land_mask),
            ocean_coverage: settings.ocean_coverage_percent(),
        }
    }

    fn terrain_source(&self, generator: &Generator, field: &HeightField) -> TerrainSource<'_> {
        TerrainSource::Fractal {
            zone_temperature: generator.generate_temperature_map(
                true,
                Some(&field.heights),
                field.land_mask.as_deref(),
                self.0.settings,
            ),
            terrain_properties: self.0.terrain_properties,
        }
    }

    fn assign_water(&self, generator: &Generator, terrain: &mut [Terrain], field: &HeightField) {
        generator.assign_water_depths(
            terrain,
            &field.water_heights,
            field
                .land_mask
                .as_deref()
                .expect("Freeciv fractal generation has a land mask"),
            self.0.settings.shore_level(),
            self.0.water_depths,
        );
    }

    fn finish_water(&self, generator: &Generator, terrain: &mut [Terrain], rng: &mut MapRng) {
        generator.finish_water_depths(terrain, self.0.water_depths, rng);
    }

    fn after_starts(
        &self,
        generator: &Generator,
        tiles: &[GeneratedTile],
        start_rng: &mut GameRng,
    ) {
        generator.advance_map_label_randomness(tiles, start_rng);
    }

    fn on_failed_attempt(&self, attempt: usize, gameplay_rng: &mut GameRng, start_rng: GameRng) {
        // Start-position search uses the gameplay stream after native map
        // generation restores it. A failed attempt therefore leaves those
        // draws in place before the next map-seed draw.
        *gameplay_rng = start_rng;
        if attempt + 1 >= MAPGEN_MAX_ATTEMPTS {
            panic!(
                "Freeciv-compatible map generation failed to allocate start positions in {MAPGEN_MAX_ATTEMPTS} attempts"
            );
        }
    }
}
