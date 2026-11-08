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

use crate::sim::{PLAYER_COUNT, PLAYER_SLOTS, Terrain, TerrainBase, Tile, topology::*};
use std::{collections::hash_map::RandomState, hash::BuildHasher};

const INITIAL_DIVISIONS: i32 = 6;

mod placement;
mod rivers;
mod terrain;

pub(crate) struct GeneratedMap {
    pub(crate) seed: u64,
    pub(crate) map_wrap: MapWrap,
    pub(crate) game_random: crate::sim::FreecivRng,
    pub(crate) tiles: Vec<Tile>,
    pub(crate) starts: [(i32, i32); PLAYER_COUNT],
}

/// Produces a process-local seed for a newly generated world.
pub(crate) fn random_seed() -> u64 {
    RandomState::new().hash_one("CivCraft world")
}

// Freeciv's 56-word additive generator, with a separate state for map work.
struct MapRng {
    values: [u32; 56],
    next: usize,
}

impl MapRng {
    fn new(seed: u64) -> Self {
        let mut values = [0_u32; 56];
        values[0] = seed as u32 ^ (seed >> 32) as u32;
        for i in 1..values.len() {
            values[i] = values[i - 1].wrapping_mul(3).wrapping_add(257);
        }
        let mut rng = Self { values, next: 0 };
        for _ in 0..10_000 {
            rng.draw(u32::MAX);
        }
        rng
    }

    fn draw(&mut self, limit: u32) -> u32 {
        if limit <= 1 {
            return 0;
        }
        let divisor = u32::MAX / limit;
        loop {
            let value = self.values[self.next].wrapping_add(self.values[(self.next + 31) % 56]);
            self.values[self.next] = value;
            self.next = (self.next + 1) % 56;
            if value < limit * divisor {
                return value / divisor;
            }
        }
    }

    fn below(&mut self, limit: i32) -> i32 {
        self.draw(limit as u32) as i32
    }
}

impl Generator {
    fn index(&self, x: i32, y: i32) -> usize {
        let (x, y) = self.wrap.normalize(x, y).expect("generator tile is real");
        self.wrap.index(x, y)
    }
}

impl Generator {
    fn neighbors(&self, tile: usize) -> impl Iterator<Item = usize> {
        self.civ3_neighbors(tile).collect::<Vec<_>>().into_iter()
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
        let vertical_wraps = self.wrap.wraps_y();
        (-1..=1)
            .flat_map(|horizontal_wrap| {
                let vertical_wraps = vertical_wraps.then_some(-1..=1).into_iter().flatten();
                std::iter::once(0)
                    .chain(vertical_wraps)
                    .map(move |vertical_wrap| {
                        let dx = raw_dx + horizontal_wrap * 2 * self.wrap.width();
                        let dy = raw_dy + vertical_wrap * self.wrap.height();
                        let diagonal_a = (dx + dy) / 2;
                        let diagonal_b = (dx - dy) / 2;
                        diagonal_a.abs().max(diagonal_b.abs())
                    })
            })
            .min()
            .unwrap_or(i32::MAX)
    }
}

// Subdivide the initial wrapped blocks using Freeciv's edge and center
// midpoint scheme, reducing the perturbation to two thirds at each level.
impl Generator {
    fn subdivide(&self, heights: &mut [Option<i32>], rng: &mut MapRng, step: i32, rect: [i32; 4]) {
        let [left, top, right, bottom] = rect;
        if right == left || bottom == top || (right - left == 1 && bottom - top == 1) {
            return;
        }
        let mx = (left + right) / 2;
        let my = (top + bottom) / 2;
        let [nw, ne, sw, se] = [(left, top), (right, top), (left, bottom), (right, bottom)]
            .map(|(x, y)| heights[self.index(x, y)].unwrap_or(0));
        for (x, y, base) in [
            (mx, top, (nw + ne) / 2),
            (mx, bottom, (sw + se) / 2),
            (left, my, (nw + sw) / 2),
            (right, my, (ne + se) / 2),
            (mx, my, (nw + ne + sw + se) / 4),
        ] {
            let tile = self.index(x, y);
            if heights[tile].is_none_or(|height| height == 0) {
                heights[tile] = Some(base + rng.below(step) - step / 2);
            }
        }
        let step = (step * 2 / 3).max(1);
        for child in [
            [left, top, mx, my],
            [left, my, mx, bottom],
            [mx, top, right, my],
            [mx, my, right, bottom],
        ] {
            self.subdivide(heights, rng, step, child);
        }
    }

    fn height_field(&self, rng: &mut MapRng) -> Vec<i32> {
        let mut heights = vec![None; self.wrap.tile_count()];
        let step = self.wrap.width() + self.wrap.height();
        let ymax = if self.wrap.wraps_y() {
            self.wrap.height()
        } else {
            self.wrap.height() - 1
        };
        let grid = |size, axis| size * axis / INITIAL_DIVISIONS;
        let near_edge = |y: i32| !self.wrap.wraps_y() && (y < 4 || y >= self.wrap.height() - 4);
        let avoid_edge = (100 - LAND_PERCENT) * step / 100 + step / 3;
        for x in 0..INITIAL_DIVISIONS {
            for y in 0..INITIAL_DIVISIONS + i32::from(!self.wrap.wraps_y()) {
                let y = grid(ymax, y);
                heights[self.index(grid(self.wrap.width(), x), y)] =
                    Some(rng.below(2 * step) - step - if near_edge(y) { avoid_edge } else { 0 });
            }
        }
        for x in 0..INITIAL_DIVISIONS {
            for y in 0..INITIAL_DIVISIONS {
                self.subdivide(
                    &mut heights,
                    rng,
                    step,
                    [
                        grid(self.wrap.width(), x),
                        grid(ymax, y),
                        grid(self.wrap.width(), x + 1),
                        grid(ymax, y + 1),
                    ],
                );
            }
        }
        heights
            .into_iter()
            .map(|height| 8 * height.unwrap_or(0) + rng.below(4) - 2)
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

const SHORE: i32 = 1000 * (100 - LAND_PERCENT) / 100;
const LOWLAND: i32 = SHORE + (1000 - SHORE) * 28 / 100;
const HIGHLAND: i32 = SHORE + (1000 - SHORE) * 70 / 100;
const VERY_COLD: i32 = 56;
const COLD: i32 = 85;
const TROPICAL: i32 = 572;

impl Generator {
    fn colatitude(&self, tile: usize) -> i32 {
        if !self.wrap.wraps_y() {
            let (_, y) = self.wrap.position(tile);
            let latitude =
                (1000.0 - 2000.0 * f64::from(y) / f64::from(self.wrap.height() - 1)) as i32;
            return 1000 - latitude.abs();
        }
        // Freeciv's torus projection folds two circular polar regions around
        // a diagonal equator in natural (staggered) coordinates.
        let (native_x, native_y) = self.wrap.position(tile);
        let x = f64::from(2 * native_x + native_y % 2) / f64::from(2 * self.wrap.width() - 1);
        let y = f64::from(native_y) / f64::from(self.wrap.height() - 1);
        let mut x = 1.0 - 2.0 * x.min(1.0 - x);
        let mut y = 2.0 * y.min(1.0 - y);
        let flipped = x + y > 1.0;
        if flipped {
            x = 1.0 - x;
            y = 1.0 - y;
        }
        let colatitude =
            1.5 * (x * x * y + x * y * y) - 0.5 * (x * x * x + y * y * y) + 1.5 * (x * x + y * y);
        let southness = if flipped {
            (2.0 - colatitude) / 2.0
        } else {
            colatitude / 2.0
        };
        let latitude = (1000.0 * (1.0 - southness) - 1000.0 * southness) as i32;
        1000 - latitude.abs()
    }
}

fn ocean_at_depth(depth: i32) -> Terrain {
    // The manual names Coast, Sea, and Ocean but does not specify generation
    // thresholds. These are project heuristics over the generated depth field;
    // finish_water guarantees an adjacent-water Coast fringe.
    match depth {
        0..=17 => Terrain::Coast,
        18..=52 => Terrain::Sea,
        _ => Terrain::Ocean,
    }
}

fn random_tile(count: usize, rng: &mut MapRng, eligible: impl Fn(usize) -> bool) -> Option<usize> {
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

impl Generator {}

impl Generator {}

impl Generator {
    pub(crate) fn generate(&self, seed: u64) -> GeneratedMap {
        let mut rng = MapRng::new(seed);
        loop {
            let mut heights = self.height_field(&mut rng);
            rank_field(&mut heights, 1000);
            let (mut terrain, temperature) = self.terrain_pass(&heights, &mut rng);
            let mut tiles: Vec<_> = terrain
                .iter()
                .copied()
                .enumerate()
                .map(|(index, terrain)| Tile {
                    terrain,
                    base_terrain: inferred_base_terrain(terrain, temperature[index]),
                    altitude: heights[index],
                    hut: false,
                    river: false,
                    river_edges: 0,
                    resource: None,
                    road: false,
                    railroad: false,
                    irrigation: false,
                    farmland: false,
                    pollution: false,
                    fallout: false,
                    base: None,
                    mine: false,
                    oil_well: false,
                    oil_platform: false,
                    owner: None,
                    border_city: None,
                    last_seen_owner: None,
                    seen: false,
                    visible: false,
                    stealth_visible: false,
                    known_by: [false; PLAYER_SLOTS],
                })
                .collect();
            let river_heights: Vec<_> = (0..self.wrap.tile_count())
                .map(|tile| heights[tile])
                .collect();
            self.place_rivers(&mut tiles, &river_heights, &temperature, &mut rng);
            self.finish_water(&mut terrain, &mut rng);
            for (tile, &terrain) in tiles.iter_mut().zip(&terrain) {
                tile.terrain = terrain;
                if terrain.is_water() {
                    tile.base_terrain = match terrain {
                        Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                        Terrain::Sea => TerrainBase::Sea,
                        Terrain::Ocean => TerrainBase::Ocean,
                        _ => tile.base_terrain,
                    };
                }
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
            self.place_resources(&mut tiles, &mut rng);
            self.place_huts(&mut tiles, &mut rng);
            let mut start_rng = MapRng::new(seed);
            // The default reference setup draws one nation and leader for each
            // of six AI slots, then consumes the map-seed draw before saving
            // the game RNG state used for start allocation.
            for _ in 0..2 * PLAYER_COUNT + 1 {
                start_rng.draw(u32::MAX);
            }
            if let Some(starts) =
                self.start_positions(&terrain, &tiles, &temperature, &mut start_rng)
            {
                let game_random =
                    crate::sim::FreecivRng::from_map_state(start_rng.values, start_rng.next)
                        .expect("map RNG cursor is valid");
                return GeneratedMap {
                    seed,
                    map_wrap: self.wrap,
                    game_random,
                    tiles,
                    starts,
                };
            }
            // Retry from the same RNG stream when a world has too few viable sites.
        }
    }
}

/// The current generator stores a single terrain value, so feature tiles lose
/// their original base ground. Keep a provisional base for the Civ III painter
/// using broad temperature bands. This is an implementation inference, not a
/// documented Civ III generation rule; the native generator should replace it.
fn inferred_base_terrain(terrain: Terrain, temperature: i32) -> TerrainBase {
    match terrain {
        Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
        Terrain::Sea => TerrainBase::Sea,
        Terrain::Ocean => TerrainBase::Ocean,
        Terrain::Glacier => TerrainBase::Ice,
        Terrain::Forest => {
            if temperature < COLD {
                TerrainBase::Tundra
            } else if temperature < TROPICAL {
                TerrainBase::Plains
            } else {
                TerrainBase::Grassland
            }
        }
        Terrain::Jungle => TerrainBase::Grassland,
        Terrain::Hills | Terrain::Mountains => {
            if temperature < VERY_COLD {
                TerrainBase::Tundra
            } else {
                TerrainBase::Plains
            }
        }
        Terrain::Marsh | Terrain::Volcano => TerrainBase::Grassland,
        Terrain::FloodPlains | Terrain::Desert => TerrainBase::Desert,
        Terrain::Plains => TerrainBase::Plains,
        Terrain::Tundra => TerrainBase::Tundra,
        Terrain::Grassland | Terrain::GrasslandShield => TerrainBase::Grassland,
    }
}

#[cfg(test)]
mod civ3_base_terrain_tests {
    use super::*;

    #[test]
    fn feature_surface_keeps_a_climate_inferred_base_ground() {
        assert_eq!(
            inferred_base_terrain(Terrain::Forest, VERY_COLD - 1),
            TerrainBase::Tundra
        );
        assert_eq!(
            inferred_base_terrain(Terrain::Forest, COLD),
            TerrainBase::Plains
        );
        assert_eq!(
            inferred_base_terrain(Terrain::Jungle, TROPICAL),
            TerrainBase::Grassland
        );
        assert_eq!(
            inferred_base_terrain(Terrain::FloodPlains, TROPICAL),
            TerrainBase::Desert
        );
        assert_eq!(
            inferred_base_terrain(Terrain::Ocean, TROPICAL),
            TerrainBase::Ocean
        );
    }

    #[test]
    fn generated_features_keep_nonwater_ground_and_water_bands_stay_aligned() {
        let map = generate_with_wrap(42, MapWrap::X);
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
                assert!(matches!(
                    tile.base_terrain,
                    TerrainBase::Grassland
                        | TerrainBase::Plains
                        | TerrainBase::Desert
                        | TerrainBase::Tundra
                ));
            }
        }
    }
}

struct Generator {
    wrap: MapWrap,
}
/// Historical reference preset, retained for existing oracle fixtures.
/// Generates a world using the default horizontal-wrap topology.
pub(crate) fn generate(seed: u64) -> GeneratedMap {
    generate_with_wrap(seed, MapWrap::XY)
}
/// Generates a reproducible world using `wrap` from the start of generation.
pub(crate) fn generate_with_wrap(seed: u64, wrap: MapWrap) -> GeneratedMap {
    let mut map = Generator { wrap }.generate(seed);
    if wrap == MapWrap::X {
        // The polar cap is a visual boundary in Civ III, not generated land.
        for y in [0, wrap.height() - 1] {
            for x in 0..wrap.width() {
                let tile = &mut map.tiles[wrap.index(x, y)];
                tile.terrain = Terrain::Ocean;
                tile.base_terrain = TerrainBase::Ocean;
                tile.altitude = tile.altitude.min(SHORE - 1);
                tile.resource = None;
                tile.river = false;
                tile.river_edges = 0;
                tile.hut = false;
            }
        }
        let generator = Generator { wrap };
        let mut terrain: Vec<_> = map.tiles.iter().map(|tile| tile.terrain).collect();
        generator.finish_water(&mut terrain, &mut MapRng::new(seed));
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
    }
    map
}

#[cfg(test)]
#[path = "mapgen/tests.rs"]
mod tests;
