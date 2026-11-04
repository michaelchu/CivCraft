use crate::sim::{PLAYER_COUNT, Terrain, Tile, topology::*};

// These are native (array) coordinates. Display and distance calculations
// convert them to Freeciv's map coordinates with the helpers below.
const STARTS: [(i32, i32); PLAYER_COUNT] = [(6, 8), (21, 8), (10, 27), (25, 29), (6, 50), (22, 53)];

pub(crate) struct GeneratedMap {
    pub(crate) tiles: Vec<Tile>,
    pub(crate) starts: [(i32, i32); PLAYER_COUNT],
}

fn height_score(x: i32, y: i32) -> f32 {
    let h = hash2(x, y);
    let continental = (x as f32 * 0.42).sin() * 0.52
        + (y as f32 * 0.27).cos() * 0.38
        + ((x + y) as f32 * 0.17).sin() * 0.30;
    let detail = ((h % 1000) as f32 / 1000.0 - 0.5) * 0.90;
    continental + detail
}

fn land_terrain(x: i32, y: i32) -> Terrain {
    match (hash2(x, y) / 1000) % 100 {
        0..=7 => Terrain::Desert,
        8..=20 => Terrain::Forest,
        21..=43 => Terrain::Grassland,
        44..=64 => Terrain::Plains,
        65..=76 => Terrain::Hills,
        77..=83 => Terrain::Mountains,
        84..=91 => Terrain::Swamp,
        _ => Terrain::Tundra,
    }
}

pub(crate) fn generate() -> GeneratedMap {
    let tile_count = (MAP_W * MAP_H) as usize;
    debug_assert!((tile_count as i32 - MAP_TARGET_TILES).abs() <= MAP_W * 2);
    debug_assert_eq!(
        LAND_TILE_TARGET,
        (MAP_W * MAP_H * LAND_PERCENT / 100) as usize
    );
    let mut required_land = vec![false; tile_count];
    for &(sx, sy) in &STARTS {
        for dy in -2..=2 {
            for dx in -2..=2 {
                if isohex_vector_distance(dx, dy) <= 2 {
                    let (x, y) = offset_native_by_map(sx, sy, dx, dy);
                    required_land[(y * MAP_W + x) as usize] = true;
                }
            }
        }
    }

    let required_count = required_land.iter().filter(|required| **required).count();
    debug_assert!(required_count <= LAND_TILE_TARGET);
    let mut candidates: Vec<(usize, f32)> = (0..tile_count)
        .filter(|index| !required_land[*index])
        .map(|index| {
            let x = index as i32 % MAP_W;
            let y = index as i32 / MAP_W;
            (index, height_score(x, y))
        })
        .collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut is_land = required_land;
    for (index, _) in candidates
        .into_iter()
        .take(LAND_TILE_TARGET - required_count)
    {
        is_land[index] = true;
    }

    let tiles = (0..tile_count)
        .map(|index| {
            let x = index as i32 % MAP_W;
            let y = index as i32 / MAP_W;
            Tile {
                terrain: if is_land[index] {
                    if STARTS.contains(&(x, y)) {
                        // Freeciv only places starts on terrain with the
                        // Starter flag. Grassland is one such terrain.
                        Terrain::Grassland
                    } else {
                        land_terrain(x, y)
                    }
                } else {
                    Terrain::Ocean
                },
                road: false,
                irrigation: false,
                mine: false,
                owner: None,
                seen: false,
                visible: false,
                known_by: [false; PLAYER_COUNT],
            }
        })
        .collect();
    GeneratedMap {
        tiles,
        starts: STARTS,
    }
}

fn hash2(x: i32, y: i32) -> u32 {
    let mut n = (x as u32)
        .wrapping_mul(0x45d9_f3b)
        .wrapping_add((y as u32).wrapping_mul(0x119d_e1f3))
        .wrapping_add(0x7f4a_7c15);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^ (n >> 16)
}
