use crate::sim::{PLAYER_COUNT, Terrain, Tile, topology::*};
use std::{collections::hash_map::RandomState, hash::BuildHasher};

const TILE_COUNT: usize = (MAP_W * MAP_H) as usize;
const INITIAL_DIVISIONS: i32 = 6;

pub(crate) struct GeneratedMap {
    pub(crate) seed: u64,
    pub(crate) tiles: Vec<Tile>,
    pub(crate) starts: [(i32, i32); PLAYER_COUNT],
}

pub(crate) fn random_seed() -> u64 {
    RandomState::new().hash_one("CivCraft world")
}

// SplitMix64 keeps generation reproducible without touching the simulation RNG.
struct MapRng(u64);

impl MapRng {
    fn below(&mut self, limit: i32) -> i32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((value ^ (value >> 31)) % limit as u64) as i32
    }
}

fn index(x: i32, y: i32) -> usize {
    let (x, y) = wrap_native(x, y);
    (y * MAP_W + x) as usize
}

fn position(index: usize) -> (i32, i32) {
    (index as i32 % MAP_W, index as i32 / MAP_W)
}

fn neighbors(tile: usize) -> impl Iterator<Item = usize> {
    let (x, y) = position(tile);
    ISOHEX_DIRS.into_iter().map(move |(dx, dy)| {
        let (nx, ny) = offset_native_by_map(x, y, dx, dy);
        index(nx, ny)
    })
}

fn distance(a: usize, b: usize) -> i32 {
    let (ax, ay) = position(a);
    let (bx, by) = position(b);
    let (dx, dy) = wrapped_map_vector(ax, ay, bx, by);
    isohex_vector_distance(dx, dy)
}

// Subdivide the initial wrapped blocks using Freeciv's edge and center
// midpoint scheme, reducing the perturbation to two thirds at each level.
fn subdivide(heights: &mut [Option<i32>], rng: &mut MapRng, step: i32, rect: [i32; 4]) {
    let [left, top, right, bottom] = rect;
    if right == left || bottom == top || (right - left == 1 && bottom - top == 1) {
        return;
    }
    let mx = (left + right) / 2;
    let my = (top + bottom) / 2;
    let [nw, ne, sw, se] = [(left, top), (right, top), (left, bottom), (right, bottom)]
        .map(|(x, y)| heights[index(x, y)].unwrap());
    for (x, y, base) in [
        (mx, top, (nw + ne) / 2),
        (mx, bottom, (sw + se) / 2),
        (left, my, (nw + sw) / 2),
        (right, my, (ne + se) / 2),
        (mx, my, (nw + ne + sw + se) / 4),
    ] {
        let value = base + rng.below(step) - step / 2;
        heights[index(x, y)].get_or_insert(value);
    }
    let step = (step * 2 / 3).max(1);
    for child in [
        [left, top, mx, my],
        [left, my, mx, bottom],
        [mx, top, right, my],
        [mx, my, right, bottom],
    ] {
        subdivide(heights, rng, step, child);
    }
}

fn height_field(rng: &mut MapRng) -> Vec<i32> {
    let mut heights = vec![None; TILE_COUNT];
    let step = MAP_W + MAP_H;
    let grid = |size, axis| size * axis / INITIAL_DIVISIONS;
    for x in 0..INITIAL_DIVISIONS {
        for y in 0..INITIAL_DIVISIONS {
            heights[index(grid(MAP_W, x), grid(MAP_H, y))] = Some(rng.below(2 * step) - step);
        }
    }
    for x in 0..INITIAL_DIVISIONS {
        for y in 0..INITIAL_DIVISIONS {
            subdivide(
                &mut heights,
                rng,
                step,
                [
                    grid(MAP_W, x),
                    grid(MAP_H, y),
                    grid(MAP_W, x + 1),
                    grid(MAP_H, y + 1),
                ],
            );
        }
    }
    heights
        .into_iter()
        .map(|height| 8 * height.unwrap() + rng.below(4) - 2)
        .collect()
}

fn components(terrain: &[Terrain], water: bool) -> Vec<Vec<usize>> {
    let mut visited = vec![false; TILE_COUNT];
    let mut groups = Vec::new();
    for tile in 0..TILE_COUNT {
        if visited[tile] || terrain[tile].is_water() != water {
            continue;
        }
        let mut group = vec![tile];
        visited[tile] = true;
        let mut cursor = 0;
        while cursor < group.len() {
            for next in neighbors(group[cursor]) {
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

fn terrain_pass(heights: &[i32], rng: &mut MapRng) -> Vec<Terrain> {
    let mut ranked: Vec<_> = (0..TILE_COUNT).collect();
    ranked.sort_unstable_by_key(|&tile| (heights[tile], tile));
    let mut land = vec![false; TILE_COUNT];
    let land_count = TILE_COUNT * LAND_PERCENT as usize / 100;
    let shore = heights[ranked[TILE_COUNT - land_count]];
    let peak = heights[*ranked.last().unwrap()];
    for &tile in &ranked[TILE_COUNT - land_count..] {
        land[tile] = true;
    }

    // Smooth a second field to give nearby tiles similar rainfall.
    let mut rainfall = height_field(rng);
    let low = *rainfall.iter().min().unwrap();
    let high = *rainfall.iter().max().unwrap();
    for rain in &mut rainfall {
        *rain = 100 * (*rain - low) / (high - low).max(1);
    }
    let mut terrain: Vec<_> = (0..TILE_COUNT)
        .map(|tile| {
            let coast = neighbors(tile)
                .filter(|&next| land[next] != land[tile])
                .count() as i32;
            if !land[tile] {
                return if coast > 0 {
                    Terrain::Ocean
                } else {
                    Terrain::DeepOcean
                };
            }

            let (_, y) = position(tile);
            let latitude_heat = 100 - (2 * y - MAP_H).abs() * 100 / MAP_H;
            let altitude = (heights[tile] - shore) * 100 / (peak - shore).max(1);
            let temperature = (latitude_heat + 10) * (100 - altitude * 30 / 100) / 100;
            let moisture = rainfall[tile] + coast * 4 - altitude / 5;
            match (temperature, altitude, moisture) {
                (..=11, _, _) => Terrain::Glacier,
                (_, 83.., _) => Terrain::Mountains,
                (..=25, _, _) => Terrain::Tundra,
                (_, 58.., _) => Terrain::Hills,
                (72.., _, 66..) => Terrain::Jungle,
                (68.., _, ..=30) => Terrain::Desert,
                (40.., ..=17, 76..) => Terrain::Swamp,
                (_, _, 57..) => Terrain::Forest,
                (_, _, 40..) => Terrain::Grassland,
                _ => Terrain::Plains,
            }
        })
        .collect();
    for lake in components(&terrain, true)
        .into_iter()
        .filter(|lake| lake.len() <= 8)
    {
        for tile in lake {
            terrain[tile] = Terrain::Lake;
        }
    }
    terrain
}

fn start_positions(terrain: &[Terrain]) -> Option<[(i32, i32); PLAYER_COUNT]> {
    let city_offsets: Vec<_> = (-2..=2)
        .flat_map(|dx| (-2..=2).map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| isohex_vector_distance(dx, dy) <= 2)
        .collect();
    let quality = |tile| {
        let (x, y) = position(tile);
        city_offsets
            .iter()
            .map(|&(dx, dy)| {
                let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                let terrain = terrain[index(nx, ny)];
                let (food, production, trade) = terrain.base_yields();
                3 * food + 2 * production + trade + if terrain.is_water() { 0 } else { 4 }
            })
            .sum::<i32>()
    };
    let mut groups = components(terrain, false);
    for group in &mut groups {
        group.retain(|&tile| {
            matches!(
                terrain[tile],
                Terrain::Grassland | Terrain::Plains | Terrain::Forest | Terrain::Hills
            ) && neighbors(tile).any(|next| !terrain[next].is_water())
        });
        group.sort_unstable_by_key(|&tile| (std::cmp::Reverse(quality(tile)), tile));
    }
    groups.sort_by_key(|group| {
        std::cmp::Reverse(
            group
                .iter()
                .take(PLAYER_COUNT)
                .map(|&tile| quality(tile))
                .sum::<i32>(),
        )
    });
    let all: Vec<_> = groups.iter().flatten().copied().collect();
    // Prefer a shared continent, with distributed starts when one cannot fit.
    for candidates in [&groups[0], &all] {
        for spacing in [6, 4, 3] {
            let mut starts = Vec::with_capacity(PLAYER_COUNT);
            for &tile in candidates {
                if starts.iter().all(|&other| distance(tile, other) >= spacing) {
                    starts.push(tile);
                    if starts.len() == PLAYER_COUNT {
                        return starts
                            .into_iter()
                            .map(position)
                            .collect::<Vec<_>>()
                            .try_into()
                            .ok();
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn generate(seed: u64) -> GeneratedMap {
    let mut rng = MapRng(seed);
    loop {
        let heights = height_field(&mut rng);
        let terrain = terrain_pass(&heights, &mut rng);
        if let Some(starts) = start_positions(&terrain) {
            let tiles = terrain
                .into_iter()
                .map(|terrain| Tile {
                    terrain,
                    road: false,
                    irrigation: false,
                    mine: false,
                    owner: None,
                    seen: false,
                    visible: false,
                    known_by: [false; PLAYER_COUNT],
                })
                .collect();
            return GeneratedMap {
                seed,
                tiles,
                starts,
            };
        }
        // Retry from the same RNG stream when a world has too few viable sites.
    }
}
