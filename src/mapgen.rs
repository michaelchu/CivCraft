//! Deterministic world generation from a seed and a fixed map topology.
//!
//! Generation builds terrain elevation and climate, classifies land and water,
//! places rivers and extras, and selects starting locations. The map RNG is
//! separate from the campaign RNG stored in [`GeneratedMap`], so generation
//! draws do not shift later gameplay randomness. The submodules split terrain
//! shaping, river drainage, and placement/scoring while sharing the same
//! wrapped-coordinate rules.
//!
//! Pass the requested [`MapWrap`] to [`generate_with_wrap`] before generation:
//! changing wrapping afterward would leave tile arrays and generated features
//! inconsistent with the map topology. Reference-map fixtures rely on stable
//! iteration and random-draw order, so seemingly harmless reordering can alter
//! seeded output.

use crate::random::FreecivRng;
use crate::rules::freeciv::{
    HutPlacementSettings, MapGenerationSettings, OceanDepthSettings, RulesetData,
    StartPositionScoringSettings, TerrainMapgenProperties,
};
pub(crate) use crate::world::{GeneratedMap, GeneratedTile};
use crate::world::{Terrain, TerrainBase, topology::*};
use std::{collections::hash_map::RandomState, hash::BuildHasher};

/// Named world sizes from OpenCiv3's Civ III ruleset data. Dimensions are
/// imported game data; OpenCiv3's generation code is only an implementation
/// reference for how those dimensions are consumed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Civ3MapSize {
    Tiny,
    Small,
    #[default]
    Standard,
    Large,
    Huge,
    Custom(i32, i32),
}

impl Civ3MapSize {
    pub(crate) fn topology(self) -> Result<MapWrap, String> {
        let (width, height) = match self {
            Self::Tiny => (60, 60),
            Self::Small => (80, 80),
            Self::Standard => (100, 100),
            Self::Large => (130, 130),
            Self::Huge => (160, 160),
            Self::Custom(width, height) => (width, height),
        };
        MapWrap::with_dimensions(width, height)
    }

    pub(crate) const fn civilization_count(self) -> usize {
        match self {
            Self::Tiny => 4,
            Self::Small => 6,
            Self::Standard => 8,
            Self::Large => 12,
            Self::Huge => 16,
            // Custom dimensions have no fixed Civ III preset count; use the
            // Standard roster until the UI exposes an explicit civilization
            // selector for custom maps.
            Self::Custom(_, _) => 8,
        }
    }
}

impl std::str::FromStr for Civ3MapSize {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "tiny" => Ok(Self::Tiny),
            "small" => Ok(Self::Small),
            "standard" => Ok(Self::Standard),
            "large" => Ok(Self::Large),
            "huge" => Ok(Self::Huge),
            dimensions => {
                let (width, height) = dimensions
                    .split_once('x')
                    .and_then(|(width, height)| {
                        Some((width.parse::<i32>().ok()?, height.parse::<i32>().ok()?))
                    })
                    .ok_or_else(|| {
                        "map size must be tiny, small, standard, large, huge, or WIDTHxHEIGHT"
                            .to_owned()
                    })?;
                MapWrap::with_dimensions(width, height)?;
                Ok(Self::Custom(width, height))
            }
        }
    }
}

impl std::fmt::Display for Civ3MapSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiny => f.write_str("tiny"),
            Self::Small => f.write_str("small"),
            Self::Standard => f.write_str("standard"),
            Self::Large => f.write_str("large"),
            Self::Huge => f.write_str("huge"),
            Self::Custom(width, height) => write!(f, "{width}x{height}"),
        }
    }
}

/// Water coverage choices exposed by Civ III's world setup.
///
/// The active Freeciv generator reads its landmass percentage from the
/// selected ruleset; this Civ III UI choice is not mapped to that setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OceanCoverage {
    Percent60,
    Percent70,
    Percent80,
}

impl OceanCoverage {
    pub(crate) const fn percent(self) -> i32 {
        match self {
            Self::Percent60 => 60,
            Self::Percent70 => 70,
            Self::Percent80 => 80,
        }
    }
}

impl std::str::FromStr for OceanCoverage {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "60" => Ok(Self::Percent60),
            "70" => Ok(Self::Percent70),
            "80" => Ok(Self::Percent80),
            _ => Err("ocean coverage must be 60, 70, or 80".into()),
        }
    }
}

/// Climate choices from Civ III world setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Climate {
    Arid,
    Normal,
    Wet,
}

impl std::str::FromStr for Climate {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "arid" => Ok(Self::Arid),
            "normal" => Ok(Self::Normal),
            "wet" => Ok(Self::Wet),
            _ => Err("climate must be arid, normal, or wet".into()),
        }
    }
}

impl Default for Climate {
    fn default() -> Self {
        Self::Normal
    }
}

/// World age controls the amount of hilly and mountainous terrain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorldAge {
    Billion3,
    Billion4,
    Billion5,
}

impl WorldAge {
    fn relief_bands(self) -> (i32, i32, i32, i32, i32) {
        match self {
            Self::Billion3 => (150, 300, 600, 750, 5),
            Self::Billion4 => (200, 300, 650, 750, 3),
            Self::Billion5 => (230, 300, 680, 750, 1),
        }
    }
}

impl std::str::FromStr for WorldAge {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "3" => Ok(Self::Billion3),
            "4" => Ok(Self::Billion4),
            "5" => Ok(Self::Billion5),
            _ => Err("world age must be 3, 4, or 5 billion years".into()),
        }
    }
}

impl Default for WorldAge {
    fn default() -> Self {
        Self::Billion4
    }
}

/// Landmass scale choices exposed by Civ III's world setup.
///
/// The active Freeciv generator does not yet map these presets to Freeciv
/// `landmass` or `startpos` settings; they currently affect the OpenSimplex
/// implementation only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Landform {
    Archipelago,
    Continents,
    Pangaea,
}

impl Landform {
    // OpenCiv3's world setup uses these scales for its landmass field.
    fn noise_scale(self) -> f64 {
        match self {
            Self::Archipelago => 0.2,
            Self::Continents => 0.05,
            Self::Pangaea => 0.02,
        }
    }
}

impl std::str::FromStr for Landform {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "archipelago" => Ok(Self::Archipelago),
            "continents" => Ok(Self::Continents),
            "pangaea" => Ok(Self::Pangaea),
            _ => Err("landform must be archipelago, continents, or pangaea".into()),
        }
    }
}

impl Default for Landform {
    fn default() -> Self {
        Self::Continents
    }
}

/// Temperature options exposed by Civ III's world setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Temperature {
    Cool,
    Temperate,
    Warm,
}

impl Temperature {
    // OpenCiv3's AssignBiomes uses percentile bands and a latitude cutoff for
    // tundra. Latitude is represented here as 0 at the poles and 1000 at the
    // equator, the inverse of absolute degrees latitude.
    fn biome_bands(self, climate: Climate) -> (i32, i32, i32, i32, i32, i32, i32) {
        let (tundra_low, tundra_high, tundra_latitude, plains_high, desert_high) = match self {
            Self::Cool => (700, 930, 500, 300, 600),
            Self::Temperate => (770, 930, 344, 350, 650),
            Self::Warm => (830, 930, 267, 400, 700),
        };
        let (plains_boost, desert_boost) = if climate == Climate::Arid {
            (80, 50)
        } else {
            (0, 0)
        };
        (
            tundra_low,
            tundra_high,
            tundra_latitude,
            150,
            plains_high + plains_boost,
            450,
            desert_high + desert_boost,
        )
    }
}

impl std::str::FromStr for Temperature {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "cool" => Ok(Self::Cool),
            "temperate" => Ok(Self::Temperate),
            "warm" => Ok(Self::Warm),
            _ => Err("temperature must be cool, temperate, or warm".into()),
        }
    }
}

impl Default for Temperature {
    fn default() -> Self {
        Self::Temperate
    }
}

/// Civ III world-generation choices. Additional world-setup controls will
/// join this struct as their generation behavior is migrated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Civ3WorldSettings {
    pub(crate) ocean_coverage: OceanCoverage,
    pub(crate) climate: Climate,
    pub(crate) age: WorldAge,
    pub(crate) landform: Landform,
    pub(crate) temperature: Temperature,
}

impl Default for OceanCoverage {
    fn default() -> Self {
        Self::Percent70
    }
}

mod noise;
mod placement;
mod rivers;
mod temperature;
mod terrain;

impl GeneratedTile {
    /// Starting-site yield before campaign improvements, matching a fresh
    /// campaign tile's yield calculation.
    fn yields(&self) -> (i32, i32, i32) {
        self.terrain.yields_with_resource(self.resource, self.river)
    }
}

impl crate::world::topology::RiverEdgeTile for GeneratedTile {
    fn has_river(&self) -> bool {
        self.river
    }
    fn is_water(&self) -> bool {
        self.terrain.is_water()
    }
    fn river_edges(&self) -> u8 {
        self.river_edges
    }
    fn set_river_edges(&mut self, edges: u8) {
        self.river_edges = edges;
    }
}

/// Produces a process-local seed for a newly generated world.
pub(crate) fn random_seed() -> u64 {
    RandomState::new().hash_one("CivCraft world")
}

/// Initialize the campaign stream from Freeciv's 32-bit seed domain and draw
/// the separate map seed before the generator snapshots gameplay randomness.
/// The full `u64` remains the CivCraft world identity; only its low 32 bits
/// seed the Freeciv-compatible RNG.
fn campaign_randomness(game_seed: u64) -> (u32, FreecivRng) {
    let mut gameplay_rng = FreecivRng::new(game_seed as u32);
    let map_seed = draw_freeciv_map_seed(&mut gameplay_rng);
    (map_seed, gameplay_rng)
}

/// Matches `map_fractal_generate()` when the ruleset leaves `mapseed` unset.
/// Each call consumes one gameplay draw and retries only for the zero seed.
fn draw_freeciv_map_seed(gameplay_rng: &mut FreecivRng) -> u32 {
    loop {
        let map_seed = gameplay_rng.draw(u32::MAX) & (u32::MAX >> 1);
        if map_seed != 0 {
            return map_seed;
        }
    }
}

/// `srv_main.c` retries an unsuitable random map at most three times.
const FREECIV_MAPGEN_MAX_ATTEMPTS: usize = 3;

// Keep map draws on the same tested Freeciv primitive while maintaining an
// independent state from campaign/gameplay randomness.
struct MapRng {
    state: FreecivRng,
}

impl OceanDepthSettings {
    /// Map C2C3's depth-ranked ocean terrains to Civ III's water labels.
    /// The shallow band up to C2C3's Ocean depth becomes Civ III Coast, the
    /// next band becomes Sea, and deeper water becomes Ocean. Depth-zero lakes
    /// are still assigned separately by the small-basin pass.
    fn civ3_ocean_for_depth(self, depth: i32) -> Terrain {
        if depth < self.ocean {
            Terrain::Coast
        } else if depth < self.deep_ocean {
            Terrain::Sea
        } else {
            Terrain::Ocean
        }
    }

    fn unsafe_coast_for_civ3_terrain(self, terrain: Terrain) -> Option<bool> {
        match terrain {
            Terrain::Coast | Terrain::Lake => Some(self.lake_unsafe_coast),
            Terrain::Sea => Some(self.ocean_unsafe_coast),
            Terrain::Ocean => Some(self.deep_ocean_unsafe_coast),
            _ => None,
        }
    }
}

impl MapRng {
    fn new(seed: u64) -> Self {
        let folded_seed = seed as u32 ^ (seed >> 32) as u32;
        Self {
            state: FreecivRng::new(folded_seed),
        }
    }

    fn draw(&mut self, limit: u32) -> u32 {
        self.state.draw(limit)
    }

    fn below(&mut self, limit: i32) -> i32 {
        self.draw(limit as u32) as i32
    }
}

trait BoundedRandom {
    fn below(&mut self, limit: i32) -> i32;
}

impl BoundedRandom for MapRng {
    fn below(&mut self, limit: i32) -> i32 {
        MapRng::below(self, limit)
    }
}

impl BoundedRandom for FreecivRng {
    fn below(&mut self, limit: i32) -> i32 {
        self.draw(limit as u32) as i32
    }
}

impl Generator {
    fn land_percent(&self) -> i32 {
        100 - self.settings.ocean_coverage.percent()
    }

    fn shore(&self) -> i32 {
        1000 * self.settings.ocean_coverage.percent() / 100
    }

    fn lowland(&self) -> i32 {
        self.shore() + (1000 - self.shore()) * 28 / 100
    }

    fn index(&self, x: i32, y: i32) -> usize {
        let (x, y) = self.wrap.normalize(x, y).expect("generator tile is real");
        self.wrap.index(x, y)
    }

    fn is_polar_cap_tile(&self, tile: usize) -> bool {
        let (_, y) = self.wrap.position(tile);
        y == 0 || y == self.wrap.height() - 1
    }
}

impl Generator {
    fn neighbors(&self, tile: usize) -> impl Iterator<Item = usize> {
        self.civ3_neighbors(tile).collect::<Vec<_>>().into_iter()
    }

    fn cardinal_neighbors(&self, tile: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.wrap.position(tile);
        (0..CIV3_DIRS.len())
            .step_by(2)
            .filter_map(move |direction| {
                self.wrap
                    .civ3_neighbor(x, y, direction)
                    .map(|(nx, ny)| self.wrap.index(nx, ny))
            })
    }

    fn within(&self, tile: usize, radius: i32) -> impl Iterator<Item = usize> {
        let mut visited = vec![false; self.wrap.tile_count()];
        let mut frontier = vec![tile];
        let mut found = vec![tile];
        visited[tile] = true;
        for _ in 0..radius {
            let mut next_frontier = Vec::new();
            for current in frontier {
                for next in self.civ3_neighbors(current) {
                    if !visited[next] {
                        visited[next] = true;
                        next_frontier.push(next);
                        found.push(next);
                    }
                }
            }
            frontier = next_frontier;
            if frontier.is_empty() {
                break;
            }
        }
        found.into_iter()
    }

    fn distance(&self, a: usize, b: usize) -> i32 {
        let (ax, ay) = self.wrap.position(a);
        let (bx, by) = self.wrap.position(b);
        let grid_ax = 2 * ax + ay.rem_euclid(2);
        let grid_bx = 2 * bx + by.rem_euclid(2);
        let raw_dx = grid_bx - grid_ax;
        let raw_dy = by - ay;
        (-1..=1)
            .map(|horizontal_wrap| {
                let dx = raw_dx + horizontal_wrap * 2 * self.wrap.width();
                let dy = raw_dy;
                let diagonal_a = (dx + dy) / 2;
                let diagonal_b = (dx - dy) / 2;
                diagonal_a.abs().max(diagonal_b.abs())
            })
            .min()
            .unwrap_or(i32::MAX)
    }
}

impl Generator {
    fn height_field(&self, seed: u64, scale: f64) -> Vec<i32> {
        const NOISE_WIDTH: i32 = 128;
        const NOISE_HEIGHT: i32 = 64;
        let noise = noise::OpenSimplex3::new(seed);
        let x_radius = f64::from(NOISE_WIDTH) / (std::f64::consts::PI * 2.0);
        // OpenCiv3 generates and globally normalizes a fixed 128x64 field,
        // then nearest-samples it at map coordinates. Keeping that order
        // matters: normalizing only the samples for one map size changes the
        // height thresholds when the map is resized.
        let mut source = Vec::with_capacity((NOISE_WIDTH * NOISE_HEIGHT) as usize);
        for y in 0..NOISE_HEIGHT {
            for x in 0..NOISE_WIDTH {
                let theta = f64::from(x) / f64::from(NOISE_WIDTH) * std::f64::consts::TAU;
                let mut amplitude = 1.0;
                let mut frequency = 1.0;
                let mut value = 0.0;
                for octave in 0..8 {
                    let octave_scale = scale * frequency;
                    let offset = f64::from(octave) * 1000.0;
                    let sample_x = 1000.0 + octave_scale * x_radius * theta.sin() + offset;
                    let sample_y = 1000.0 + octave_scale * x_radius * theta.cos() + offset;
                    let sample_z = 1000.0 + octave_scale * f64::from(y) + offset;
                    value += noise.sample(sample_x, sample_y, sample_z) * amplitude;
                    amplitude *= 0.5;
                    frequency *= 2.0;
                }
                let normalized_y = f64::from(y) / f64::from(NOISE_HEIGHT - 1);
                let polar_distance = (normalized_y - 0.5).abs() * 2.0;
                value *= 1.0 - polar_distance * polar_distance;
                source.push(value);
            }
        }
        let min = source.iter().copied().fold(f64::INFINITY, f64::min);
        let max = source.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let range = max - min;
        let source: Vec<i32> = source
            .into_iter()
            .map(|value| (((value - min) / range).clamp(0.0, 1.0) * 255.0) as i32)
            .collect();

        (0..self.wrap.tile_count())
            .map(|tile| {
                let (map_x, map_y) = self.wrap.position(tile);
                let x = (map_x * NOISE_WIDTH / self.wrap.width()) as usize;
                let y = (map_y * NOISE_HEIGHT / self.wrap.height()) as usize;
                source[y * NOISE_WIDTH as usize + x]
            })
            .collect()
    }

    fn components(&self, terrain: &[Terrain], water: bool) -> Vec<Vec<usize>> {
        let mut visited = vec![false; self.wrap.tile_count()];
        let mut groups = Vec::new();
        for tile in 0..self.wrap.tile_count() {
            if visited[tile] || terrain[tile].is_water() != water {
                continue;
            }
            let mut group = vec![tile];
            visited[tile] = true;
            let mut cursor = 0;
            while cursor < group.len() {
                for next in self.neighbors(group[cursor]) {
                    if !visited[next] && terrain[next].is_water() == water {
                        visited[next] = true;
                        group.push(next);
                    }
                }
                cursor += 1;
            }
            groups.push(group);
        }
        groups
    }
}

// Freeciv scales elevation by cumulative frequency, keeping ties together.
/// Replaces raw values with percentile-like ranks while preserving ties.
fn rank_field(values: &mut [i32], maximum: i32) {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    for value in values {
        *value =
            sorted.partition_point(|other| other <= value) as i32 * maximum / sorted.len() as i32;
    }
}

const VERY_COLD: i32 = 56;
const COLD: i32 = 85;
const TROPICAL: i32 = 572;

impl Generator {
    fn colatitude(&self, tile: usize) -> i32 {
        let (_, y) = self.wrap.position(tile);
        let latitude = (1000.0 - 2000.0 * f64::from(y) / f64::from(self.wrap.height() - 1)) as i32;
        1000 - latitude.abs()
    }

    fn taper_polar_landmass(&self, heights: &mut [i32], seed: u64) {
        let width = self.wrap.width();
        let height = self.wrap.height();
        let band_depth = (f64::from(height) * 0.10).clamp(5.0, 12.0);
        // Sample a 3D noise field around a circle so the displacement is smooth
        // across the horizontal wrap seam. Two scales make broad inlets and
        // headlands without giving each longitude an independent offset.
        let boundary_noise = noise::OpenSimplex3::new(seed ^ 0x706f_6c61_725f_6564);
        let broad_radius = f64::from(width) / (std::f64::consts::TAU * 24.0);
        let detail_radius = f64::from(width) / (std::f64::consts::TAU * 11.0);

        for x in 0..width {
            let longitude = std::f64::consts::TAU * f64::from(x) / f64::from(width);
            let (cos_longitude, sin_longitude) = (longitude.cos(), longitude.sin());
            let polar_depth = |hemisphere| {
                let broad = boundary_noise.sample(
                    broad_radius * cos_longitude,
                    hemisphere,
                    broad_radius * sin_longitude,
                );
                let detail = boundary_noise.sample(
                    detail_radius * cos_longitude,
                    hemisphere + 17.3,
                    detail_radius * sin_longitude,
                );
                (band_depth + 0.65 * band_depth * broad + 0.30 * band_depth * detail)
                    .clamp(band_depth * 0.45, band_depth * 1.8)
            };
            let north_depth = polar_depth(-11.7);
            let south_depth = polar_depth(23.9);

            for y in 0..height {
                let (edge_distance, local_depth) = if y < height / 2 {
                    (f64::from(y), north_depth)
                } else {
                    (f64::from(height - 1 - y), south_depth)
                };
                let t = ((local_depth - edge_distance) / local_depth).clamp(0.0, 1.0);
                let smooth_taper = t * t * (3.0 - 2.0 * t);
                let attenuation = 1.0 - 0.95 * smooth_taper;
                let tile = self.wrap.index(x, y);
                heights[tile] = (f64::from(heights[tile]) * attenuation).round() as i32;
            }
        }
    }
}

fn random_tile<R: BoundedRandom>(
    count: usize,
    rng: &mut R,
    eligible: impl Fn(usize) -> bool,
) -> Option<usize> {
    for _ in 0..count / 10 {
        let tile = rng.below(count as i32) as usize;
        if eligible(tile) {
            return Some(tile);
        }
    }
    let candidates: Vec<_> = (0..count).filter(|&tile| eligible(tile)).collect();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[rng.below(candidates.len() as i32) as usize])
    }
}

/// Freeciv's `rand_map_pos_filtered` sampling contract, adapted to CivCraft's
/// flattened X-wrap/bounded-Y tile array. After count/10 random probes, its
/// fallback draws once from eligible positions in map order.
fn freeciv_rand_map_pos_filtered<R: BoundedRandom>(
    count: usize,
    rng: &mut R,
    eligible: impl Fn(usize) -> bool,
) -> Option<usize> {
    let max_tries = count / 10; // Freeciv ACTIVITY_FACTOR is 10.
    let mut tries = 0;
    loop {
        let tile = rng.below(count as i32) as usize;
        if eligible(tile) {
            return Some(tile);
        }
        tries += 1;
        if tries >= max_tries {
            break;
        }
    }
    let candidates: Vec<_> = (0..count).filter(|&tile| eligible(tile)).collect();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[rng.below(candidates.len() as i32) as usize])
    }
}

impl Generator {}

impl Generator {}

impl Generator {
    pub(crate) fn generate(
        &self,
        game_seed: u64,
        map_seed: u32,
        gameplay_rng: FreecivRng,
    ) -> GeneratedMap {
        self.generate_with_landmass(
            game_seed,
            map_seed,
            gameplay_rng,
            LandmassSource::OpenSimplex,
            None,
            None,
        )
    }

    fn generate_with_landmass(
        &self,
        game_seed: u64,
        map_seed: u32,
        gameplay_rng: FreecivRng,
        landmass_source: LandmassSource,
        terrain_properties: Option<&[TerrainMapgenProperties]>,
        start_scoring_settings: Option<StartPositionScoringSettings>,
    ) -> GeneratedMap {
        let map_seed_u64 = u64::from(map_seed);
        let (mut relief, mut biome_field) = match landmass_source {
            LandmassSource::OpenSimplex => {
                // OpenCiv3 seeds its relief noise independently from the landmass
                // field. Reuse the local deterministic field builder with its
                // documented seed offset as an explicit implementation approximation.
                (
                    self.height_field(map_seed_u64.wrapping_add(0xfeed), 0.2),
                    self.height_field(map_seed_u64.wrapping_add(0xdad), 0.4),
                )
            }
            LandmassSource::FreecivFractal(_, _) => {
                // The Freeciv terrain pass uses the actual height and
                // temperature maps instead of CivCraft's separate relief and
                // biome noise fields.
                (
                    vec![0; self.wrap.tile_count()],
                    vec![0; self.wrap.tile_count()],
                )
            }
        };
        if matches!(landmass_source, LandmassSource::OpenSimplex) {
            rank_field(&mut relief, 1000);
            rank_field(&mut biome_field, 1000);
        }
        let mut attempt = 0_usize;
        let mut gameplay_rng = gameplay_rng;
        let mut current_map_seed = map_seed;
        let freeciv_map_settings = match landmass_source {
            LandmassSource::OpenSimplex => None,
            LandmassSource::FreecivFractal(settings, _) => Some(settings),
        };
        let start_temperature = match landmass_source {
            LandmassSource::OpenSimplex => None,
            LandmassSource::FreecivFractal(map_settings, _) => {
                Some(self.freeciv_temperature_map(false, None, None, map_settings))
            }
        };
        loop {
            let freeciv_mapgen = freeciv_map_settings.is_some();
            if freeciv_mapgen && attempt > 0 {
                // Each native retry calls map_fractal_generate() again, which
                // draws a new seed from the restored gameplay stream.
                current_map_seed = draw_freeciv_map_seed(&mut gameplay_rng);
            }
            let water_seed = if freeciv_mapgen {
                u64::from(current_map_seed)
            } else {
                map_seed_u64.wrapping_add(0x1234_u64.wrapping_mul(attempt as u64))
            };
            // Freeciv seeds one map-generation stream, then continues it
            // through height, relief, terrain, rivers, water depth, and extras.
            let mut rng = MapRng::new(water_seed);
            let (water_heights, heights, freeciv_land_mask, ocean_coverage) = match landmass_source
            {
                LandmassSource::OpenSimplex => {
                    let mut field =
                        self.height_field(water_seed, self.settings.landform.noise_scale());
                    // Suppress land gradually toward the bounded polar edges before
                    // applying the water percentile. This keeps the requested ocean
                    // coverage while avoiding a hard, straight post-generation shelf.
                    self.taper_polar_landmass(&mut field, water_seed);
                    let mut heights = field.clone();
                    rank_field(&mut heights, 1000);
                    (field, heights, None, self.settings.ocean_coverage.percent())
                }
                LandmassSource::FreecivFractal(map_settings, _water_depths) => {
                    let field = self.freeciv_fractal_height_field(
                        &mut rng,
                        map_settings
                            .startpos
                            .fractal_divisions(self.civilization_count),
                        map_settings.landmass_percent,
                        true,
                    );
                    let land_mask = freeciv_land_mask(&field, map_settings);
                    (
                        field.clone(),
                        field,
                        Some(land_mask),
                        map_settings.ocean_coverage_percent(),
                    )
                }
            };
            let freeciv_temperature = match landmass_source {
                LandmassSource::OpenSimplex => None,
                LandmassSource::FreecivFractal(map_settings, _) => {
                    Some(self.freeciv_temperature_map(
                        true,
                        Some(&heights),
                        freeciv_land_mask.as_deref(),
                        map_settings,
                    ))
                }
            };
            let (mut terrain, mut base_terrain, temperature) = self.terrain_pass(
                &heights,
                &water_heights,
                &relief,
                &biome_field,
                freeciv_land_mask.as_deref(),
                freeciv_temperature.as_ref(),
                terrain_properties,
                ocean_coverage,
                &mut rng,
            );
            if let LandmassSource::FreecivFractal(map_settings, water_depths) = landmass_source {
                self.assign_freeciv_water_depths(
                    &mut terrain,
                    &water_heights,
                    freeciv_land_mask
                        .as_deref()
                        .expect("Freeciv fractal generation has a land mask"),
                    map_settings.shore_level(),
                    water_depths,
                );
            }
            let mut tiles: Vec<_> = terrain
                .iter()
                .copied()
                .enumerate()
                .map(|(index, terrain)| GeneratedTile {
                    terrain,
                    base_terrain: base_terrain[index],
                    altitude: heights[index],
                    hut: false,
                    river: false,
                    river_edges: 0,
                    resource: None,
                })
                .collect();
            let river_heights: Vec<_> = (0..self.wrap.tile_count())
                .map(|tile| heights[tile])
                .collect();
            self.place_rivers(
                &mut tiles,
                &river_heights,
                &temperature,
                freeciv_temperature.as_ref(),
                if freeciv_temperature.is_some() {
                    terrain_properties
                } else {
                    None
                },
                &mut rng,
            );
            match landmass_source {
                LandmassSource::OpenSimplex => {
                    self.finish_water(&mut terrain, &mut rng);
                }
                LandmassSource::FreecivFractal(_, water_depths) => {
                    self.finish_freeciv_water_depths(&mut terrain, water_depths, &mut rng);
                }
            }
            for (index, (tile, &terrain)) in tiles.iter_mut().zip(&terrain).enumerate() {
                tile.terrain = terrain;
                if terrain.is_water() {
                    base_terrain[index] = match terrain {
                        Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                        Terrain::Sea => TerrainBase::Sea,
                        Terrain::Ocean => TerrainBase::Ocean,
                        _ => base_terrain[index],
                    };
                }
                tile.base_terrain = base_terrain[index];
            }
            // In Civ III, Flood Plains are a desert terrain variant on river
            // tiles. Rivers are represented per tile here, so use the marked
            // river tiles as the closest direct equivalent.
            for (index, tile) in tiles.iter_mut().enumerate() {
                if tile.terrain == Terrain::Desert && tile.river {
                    tile.terrain = Terrain::FloodPlains;
                    terrain[index] = Terrain::FloodPlains;
                }
            }
            prune_civ3_river_edges(self.wrap, &mut tiles);
            self.place_resources(
                &mut tiles,
                &mut rng,
                freeciv_map_settings,
                terrain_properties,
            );
            self.place_huts(
                &mut tiles,
                &mut rng,
                freeciv_map_settings,
                terrain_properties,
            );
            // Freeciv restores the game-seeded stream after map generation.
            // CivCraft currently omits Freeciv nation/leader selection draws,
            // but the map-seed draw has already advanced this saved state.
            let mut start_rng = gameplay_rng.clone();
            if let Some(starts) = self.start_positions(
                &terrain,
                &tiles,
                &temperature,
                start_temperature.as_ref(),
                terrain_properties,
                start_scoring_settings,
                self.civilization_count,
                &mut start_rng,
            ) {
                if freeciv_map_settings.is_some() {
                    self.advance_c2c3_map_label_randomness(&tiles, &mut start_rng);
                }
                return GeneratedMap {
                    seed: game_seed,
                    map_seed: current_map_seed,
                    map_wrap: self.wrap,
                    game_random: start_rng,
                    tiles,
                    starts,
                    civilization_count: self.civilization_count,
                };
            }
            if freeciv_mapgen {
                // Start-position search uses the gameplay stream after native
                // map generation restores it. A failed attempt therefore
                // leaves those draws in place before the next map-seed draw.
                gameplay_rng = start_rng;
                if attempt + 1 >= FREECIV_MAPGEN_MAX_ATTEMPTS {
                    panic!(
                        "Freeciv-compatible map generation failed to allocate start positions in {FREECIV_MAPGEN_MAX_ATTEMPTS} attempts"
                    );
                }
            }
            // The legacy OpenSimplex adapter retains its arithmetic retry
            // stream; Freeciv attempts above use fresh native map seeds.
            attempt += 1;
        }
    }

    /// Freeciv's FRACTAL generator uses recursive midpoint displacement over
    /// a coarse grid, then ranks the field by cumulative frequency into
    /// `[0, 1000]`. This adapts its X-wrapped, bounded-Y topology before the
    /// CivCraft terrain, feature, and resource passes.
    /// C2C3 sets `flatpoles=0`; its latitude-sensitive pole-midpoint branch is
    /// not replicated because CivCraft applies its ocean-only edge afterward.
    /// Algorithm source: Freeciv `server/generator/height_map.c` (GPL-2.0-or-later).
    fn freeciv_fractal_height_field(
        &self,
        rng: &mut MapRng,
        divisions: i32,
        land_percent: i32,
        avoid_bounded_edges: bool,
    ) -> Vec<i32> {
        let divisions = divisions.max(2);
        let width = self.wrap.width();
        let height = self.wrap.height();
        let xmax = width;
        let ymax = height - 1;
        let step = width + height;
        let avoid_edge = (100 - land_percent) * step / 100 + step / 3;
        // Freeciv uses zero as the unset sentinel for shared midpoints.
        let mut values = vec![0; self.wrap.tile_count()];

        for x_current in 0..divisions {
            for y_current in 0..=divisions {
                let x = x_current * xmax / divisions;
                let y = y_current * ymax / divisions;
                let tile = self.wrap.index(x, y);
                let mut value = rng.below(2 * step) - (2 * step) / 2;
                if avoid_bounded_edges && self.near_bounded_edge(y) {
                    value -= avoid_edge;
                }
                values[tile] = value;
            }
        }

        for x_current in 0..divisions {
            for y_current in 0..divisions {
                let xl = x_current * xmax / divisions;
                let yt = y_current * ymax / divisions;
                let xr = (x_current + 1) * xmax / divisions;
                let yb = (y_current + 1) * ymax / divisions;
                self.freeciv_fractal_subdivide(&mut values, rng, step, xl, yt, xr, yb);
            }
        }

        for value in &mut values {
            *value = 8 * *value + rng.below(4) - 2;
        }
        rank_field(&mut values, 1000);
        values
    }

    fn freeciv_fractal_subdivide(
        &self,
        values: &mut [i32],
        rng: &mut MapRng,
        step: i32,
        xl: i32,
        yt: i32,
        xr: i32,
        yb: i32,
    ) {
        if xr - xl <= 0 || yb - yt <= 0 || (xr - xl == 1 && yb - yt == 1) {
            return;
        }
        let xmid = (xl + xr) / 2;
        let ymid = (yt + yb) / 2;
        let left = xl.rem_euclid(self.wrap.width());
        let right = xr.rem_euclid(self.wrap.width());
        let top_left = self.fractal_value(values, left, yt);
        let top_right = self.fractal_value(values, right, yt);
        let bottom_left = self.fractal_value(values, left, yb);
        let bottom_right = self.fractal_value(values, right, yb);
        let reduced_step = 2 * step / 3;

        self.set_fractal_value(values, rng, step, xmid, yt, (top_left + top_right) / 2);
        self.set_fractal_value(
            values,
            rng,
            step,
            xmid,
            yb,
            (bottom_left + bottom_right) / 2,
        );
        self.set_fractal_value(values, rng, step, xl, ymid, (top_left + bottom_left) / 2);
        self.set_fractal_value(values, rng, step, xr, ymid, (top_right + bottom_right) / 2);
        self.set_fractal_value(
            values,
            rng,
            step,
            xmid,
            ymid,
            (top_left + top_right + bottom_left + bottom_right) / 4,
        );

        self.freeciv_fractal_subdivide(values, rng, reduced_step, xl, yt, xmid, ymid);
        self.freeciv_fractal_subdivide(values, rng, reduced_step, xl, ymid, xmid, yb);
        self.freeciv_fractal_subdivide(values, rng, reduced_step, xmid, yt, xr, ymid);
        self.freeciv_fractal_subdivide(values, rng, reduced_step, xmid, ymid, xr, yb);
    }

    fn fractal_value(&self, values: &[i32], x: i32, y: i32) -> i32 {
        values[self.wrap.index(x, y)]
    }

    fn near_bounded_edge(&self, y: i32) -> bool {
        // `near_singularity()` uses the city radius (2 map tiles) on an
        // unwrapped axis. CivCraft has no wrapped-Y topology.
        y < 2 || y >= self.wrap.height() - 2
    }

    fn set_fractal_value(
        &self,
        values: &mut [i32],
        rng: &mut MapRng,
        step: i32,
        x: i32,
        y: i32,
        midpoint: i32,
    ) {
        let tile = self.wrap.index(x, y);
        // Freeciv evaluates the jitter before checking whether this midpoint
        // is shared or near a singular edge, so every invocation consumes a
        // map-stream draw even when its value is discarded.
        let jittered = midpoint + rng.below(step) - step / 2;
        if !self.near_bounded_edge(y) && values[tile] == 0 {
            values[tile] = jittered;
        }
    }
}

#[derive(Clone, Copy)]
enum LandmassSource {
    OpenSimplex,
    FreecivFractal(MapGenerationSettings, OceanDepthSettings),
}

fn freeciv_land_mask(heights: &[i32], settings: MapGenerationSettings) -> Vec<bool> {
    let shore = settings.shore_level();
    heights.iter().map(|&height| height >= shore).collect()
}

#[cfg(test)]
mod civ3_base_terrain_tests {
    use super::*;

    #[test]
    fn generated_feature_tiles_keep_their_assigned_biome_and_relief_ground() {
        let request = map_generation_request(
            42,
            MapWrap::X,
            Civ3WorldSettings::default(),
            crate::world::DEFAULT_CIVILIZATION_COUNT,
        );
        let map = generate_with_map_generator(&OpenSimplexMapGenerator, request);
        let mut feature_count = 0;
        let mut base_ground_seen = [false; 4];
        for tile in &map.tiles {
            let expected_water_ground = match tile.terrain {
                Terrain::Coast | Terrain::Lake => Some(TerrainBase::Coast),
                Terrain::Sea => Some(TerrainBase::Sea),
                Terrain::Ocean => Some(TerrainBase::Ocean),
                _ => None,
            };
            if let Some(expected) = expected_water_ground {
                assert_eq!(tile.base_terrain, expected);
            }
            if TerrainBase::is_feature_terrain(tile.terrain) {
                let index = match tile.base_terrain {
                    TerrainBase::Grassland => 0,
                    TerrainBase::Plains => 1,
                    TerrainBase::Desert => 2,
                    TerrainBase::Tundra => 3,
                    _ => panic!(
                        "land feature {:?} has non-land base terrain {:?}",
                        tile.terrain, tile.base_terrain
                    ),
                };
                feature_count += 1;
                base_ground_seen[index] = true;
            }
        }
        assert!(feature_count > 0);
        assert!(base_ground_seen.into_iter().all(|seen| seen));
    }

    #[test]
    fn custom_map_dimensions_flow_through_generation() {
        let wrap = MapWrap::with_dimensions(80, 60).unwrap();
        let map = generate_with_wrap(43, wrap);
        assert_eq!(map.map_wrap, wrap);
        assert_eq!(map.tiles.len(), 80 * 60);
        assert!(
            map.starts
                .iter()
                .all(|start| { (0..80).contains(&start.0) && (1..59).contains(&start.1) })
        );
        assert!(map.tiles[..80].iter().all(|tile| tile.terrain.is_water()));
        assert!(
            map.tiles[80 * 59..]
                .iter()
                .all(|tile| tile.terrain.is_water())
        );
    }
}

struct Generator {
    wrap: MapWrap,
    settings: Civ3WorldSettings,
    civilization_count: usize,
}

/// Inputs shared by interchangeable map-generation implementations.
pub(crate) struct MapGenerationRequest {
    /// The caller-provided campaign/game seed, preserved as a 64-bit world ID.
    pub(crate) seed: u64,
    pub(crate) map_seed: u32,
    /// Gameplay RNG after Freeciv's unconditional map-seed draw.
    pub(crate) gameplay_rng: FreecivRng,
    pub(crate) wrap: MapWrap,
    pub(crate) settings: Civ3WorldSettings,
    pub(crate) map_generation_settings: MapGenerationSettings,
    pub(crate) terrain_properties: Vec<TerrainMapgenProperties>,
    pub(crate) start_scoring_settings: StartPositionScoringSettings,
    pub(crate) civilization_count: usize,
}

fn map_generation_request(
    seed: u64,
    wrap: MapWrap,
    settings: Civ3WorldSettings,
    civilization_count: usize,
) -> MapGenerationRequest {
    let map_generation_settings = RulesetData::campaign_default()
        .map_generation_settings()
        .expect("campaign ruleset map-generation settings must be valid");
    map_generation_request_with_map_settings(
        seed,
        wrap,
        settings,
        civilization_count,
        map_generation_settings,
    )
}

fn map_generation_request_with_map_settings(
    seed: u64,
    wrap: MapWrap,
    settings: Civ3WorldSettings,
    civilization_count: usize,
    map_generation_settings: MapGenerationSettings,
) -> MapGenerationRequest {
    let (map_seed, gameplay_rng) = campaign_randomness(seed);
    let terrain_properties = RulesetData::campaign_default()
        .terrain_mapgen_properties()
        .expect("campaign ruleset terrain properties must be valid");
    let start_scoring_settings = RulesetData::campaign_default()
        .start_position_scoring_settings()
        .expect("campaign ruleset start-position scoring settings must be valid");
    MapGenerationRequest {
        seed,
        map_seed,
        gameplay_rng,
        wrap,
        settings,
        map_generation_settings,
        terrain_properties,
        start_scoring_settings,
        civilization_count,
    }
}

/// Produces a complete campaign map from validated world-generation inputs.
pub(crate) trait MapGenerator {
    fn generate(&self, request: MapGenerationRequest) -> GeneratedMap;
}

/// Seeded OpenSimplex terrain and campaign-map implementation.
pub(crate) struct OpenSimplexMapGenerator;

/// Freeciv-inspired recursive fractal landmass field with CivCraft's
/// terrain, feature, river, resource, and starting-location passes.
pub(crate) struct FreecivFractalMapGenerator;

static DEFAULT_MAP_GENERATOR: FreecivFractalMapGenerator = FreecivFractalMapGenerator;

fn default_map_generator() -> &'static dyn MapGenerator {
    &DEFAULT_MAP_GENERATOR
}

/// Historical reference preset, retained for existing oracle fixtures.
/// Generates a world using the default horizontal-wrap topology.
pub(crate) fn generate(seed: u64) -> GeneratedMap {
    generate_with_wrap(seed, MapWrap::X)
}
/// Generates a reproducible world using `wrap` from the start of generation.
pub(crate) fn generate_with_wrap(seed: u64, wrap: MapWrap) -> GeneratedMap {
    generate_with_settings(seed, wrap, Civ3WorldSettings::default())
}

pub(crate) fn generate_with_settings(
    seed: u64,
    wrap: MapWrap,
    settings: Civ3WorldSettings,
) -> GeneratedMap {
    generate_with_civilization_count(
        seed,
        wrap,
        settings,
        crate::world::DEFAULT_CIVILIZATION_COUNT,
    )
}

pub(crate) fn generate_for_civ3_size(
    seed: u64,
    map_size: Civ3MapSize,
    settings: Civ3WorldSettings,
) -> GeneratedMap {
    generate_for_civ3_size_with(default_map_generator(), seed, map_size, settings)
}

/// Generates a map through the selected implementation for a named Civ III size.
pub(crate) fn generate_for_civ3_size_with(
    map_generator: &dyn MapGenerator,
    seed: u64,
    map_size: Civ3MapSize,
    settings: Civ3WorldSettings,
) -> GeneratedMap {
    let wrap = map_size
        .topology()
        .expect("validated Civ III map size has valid dimensions");
    generate_with_map_generator(
        map_generator,
        map_generation_request(seed, wrap, settings, map_size.civilization_count()),
    )
}

fn generate_with_civilization_count(
    seed: u64,
    wrap: MapWrap,
    settings: Civ3WorldSettings,
    civilization_count: usize,
) -> GeneratedMap {
    generate_with_map_generator(
        default_map_generator(),
        map_generation_request(seed, wrap, settings, civilization_count),
    )
}

/// Generates a world through an explicitly selected map-generation implementation.
pub(crate) fn generate_with_map_generator(
    map_generator: &dyn MapGenerator,
    request: MapGenerationRequest,
) -> GeneratedMap {
    assert!((4..=16).contains(&request.civilization_count));
    map_generator.generate(request)
}

impl MapGenerator for OpenSimplexMapGenerator {
    fn generate(&self, request: MapGenerationRequest) -> GeneratedMap {
        let MapGenerationRequest {
            seed,
            map_seed,
            gameplay_rng,
            wrap,
            settings,
            map_generation_settings: _,
            terrain_properties: _,
            start_scoring_settings: _,
            civilization_count,
        } = request;
        assert!((4..=16).contains(&civilization_count));
        let generator = Generator {
            wrap,
            settings,
            civilization_count,
        };
        let map = generator.generate(seed, map_seed, gameplay_rng);
        finalize_polar_ocean_boundary(&generator, generator.shore(), map_seed, map, None)
    }
}

impl MapGenerator for FreecivFractalMapGenerator {
    fn generate(&self, request: MapGenerationRequest) -> GeneratedMap {
        let MapGenerationRequest {
            seed,
            map_seed,
            gameplay_rng,
            wrap,
            settings,
            map_generation_settings,
            terrain_properties,
            start_scoring_settings,
            civilization_count,
        } = request;
        assert!((4..=16).contains(&civilization_count));
        terrain::validate_freeciv_terrain_mappings(&terrain_properties)
            .expect("C2C3 mapgen terrains must map to Civ III terrain identities");
        let generator = Generator {
            wrap,
            settings,
            civilization_count,
        };
        let shore = map_generation_settings.shore_level();
        let water_depths = map_generation_settings.ocean_depths;
        let map = generator.generate_with_landmass(
            seed,
            map_seed,
            gameplay_rng,
            LandmassSource::FreecivFractal(map_generation_settings, water_depths),
            Some(&terrain_properties),
            Some(start_scoring_settings),
        );
        finalize_polar_ocean_boundary(&generator, shore, map_seed, map, Some(water_depths))
    }
}

fn finalize_polar_ocean_boundary(
    generator: &Generator,
    shore: i32,
    map_seed: u32,
    mut map: GeneratedMap,
    freeciv_water_depths: Option<OceanDepthSettings>,
) -> GeneratedMap {
    let wrap = generator.wrap;
    // The polar cap is a visual boundary in Civ III, not generated land.
    for tile_index in 0..wrap.tile_count() {
        if !generator.is_polar_cap_tile(tile_index) {
            continue;
        }
        let tile = &mut map.tiles[tile_index];
        tile.terrain = Terrain::Ocean;
        tile.base_terrain = TerrainBase::Ocean;
        tile.altitude = tile.altitude.min(shore - 1);
        tile.resource = None;
        tile.river = false;
        tile.river_edges = 0;
        tile.hut = false;
    }
    let mut terrain: Vec<_> = map.tiles.iter().map(|tile| tile.terrain).collect();
    if let Some(_water_depths) = freeciv_water_depths {
        generator.finish_freeciv_lakes(&mut terrain);
    } else {
        generator.finish_water(&mut terrain, &mut MapRng::new(u64::from(map_seed)));
    }
    for (tile, water) in map.tiles.iter_mut().zip(terrain) {
        tile.terrain = water;
        if water.is_water() {
            tile.base_terrain = match water {
                Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                Terrain::Sea => TerrainBase::Sea,
                Terrain::Ocean => TerrainBase::Ocean,
                _ => tile.base_terrain,
            };
        }
    }
    for y in [0, wrap.height() - 1] {
        for x in 0..wrap.width() {
            let tile = &mut map.tiles[wrap.index(x, y)];
            tile.terrain = Terrain::Ocean;
            tile.base_terrain = TerrainBase::Ocean;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
        }
    }
    prune_civ3_river_edges(wrap, &mut map.tiles);
    map
}

#[cfg(test)]
#[path = "mapgen/tests.rs"]
mod tests;
