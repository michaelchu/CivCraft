//! Regression tests for the behavior of the parent module.

use super::*;
use crate::sim::Resource;

const TILE_COUNT: usize = (MAP_W * MAP_H) as usize;

#[test]
fn map_cursor_conversion_continues_the_same_native_stream() {
    for seed in [0, 42, u64::MAX] {
        let mut map = MapRng::new(seed);
        // Cross many ring boundaries and rejection-heavy ranges.
        for _ in 0..137 {
            map.draw(2_147_483_649);
        }
        let mut game = crate::sim::FreecivRng::from_map_state(map.values, map.next).unwrap();
        for limit in [0, 1, 2, 3, 100, 2_147_483_649, u32::MAX].repeat(23) {
            assert_eq!(game.draw(limit), map.draw(limit));
        }
    }
    assert!(crate::sim::FreecivRng::from_map_state([0; 56], 56).is_err());
}

#[test]
fn generated_rivers_reach_water_or_the_poles_and_resources_are_spaced() {
    for seed in [0, 42, 12345] {
        let map = generate(seed);
        let tiles = &map.tiles;
        assert!(tiles.iter().any(|tile| tile.river));
        assert!(tiles.iter().any(|tile| tile.resource.is_some()));
        assert!(tiles.iter().any(|tile| tile.hut));
        assert!(
            map.starts
                .iter()
                .all(|&(x, y)| !tiles[(Generator { wrap: MapWrap::XY }).index(x, y)].hut)
        );

        let mut visited = vec![false; TILE_COUNT];
        for (index, tile) in tiles.iter().enumerate() {
            assert!((0..=1000).contains(&tile.altitude));
            if tile.hut {
                assert!(!tile.terrain.is_water());
                assert!(
                    (Generator { wrap: MapWrap::XY })
                        .within(index, 3)
                        .all(|next| next == index || !tiles[next].hut)
                );
            }
            if tile.resource.is_some() {
                assert!(
                    (Generator { wrap: MapWrap::XY })
                        .neighbors(index)
                        .all(|next| tiles[next].resource.is_none())
                );
                assert_ne!(tile.terrain, Terrain::DeepOcean);
            }
            if !tile.river || visited[index] {
                continue;
            }
            let mut course = vec![index];
            visited[index] = true;
            let mut cursor = 0;
            let mut has_outlet = false;
            while cursor < course.len() {
                let current = course[cursor];
                assert!(!tiles[current].terrain.is_water());
                has_outlet |= tiles[current].terrain == Terrain::Glacier;
                for next in (Generator { wrap: MapWrap::XY }).neighbors(current) {
                    has_outlet |= tiles[next].terrain.is_water();
                    if tiles[next].river && !visited[next] {
                        visited[next] = true;
                        course.push(next);
                    }
                }
                cursor += 1;
            }
            assert!(has_outlet, "river at {index} in seed {seed} has no outlet");
        }
    }
}
#[cfg(test)]
fn map_checksum(tiles: &[Tile]) -> u64 {
    let mut checksum = 0xcbf29ce484222325_u64;
    for tile in tiles {
        let terrain = match tile.terrain {
            Terrain::Ocean => b' ',
            Terrain::DeepOcean => b':',
            Terrain::Lake => b'+',
            Terrain::Glacier => b'a',
            Terrain::Desert => b'd',
            Terrain::Forest => b'f',
            Terrain::Grassland => b'g',
            Terrain::Hills => b'h',
            Terrain::Jungle => b'j',
            Terrain::Mountains => b'm',
            Terrain::Plains => b'p',
            Terrain::Swamp => b's',
            Terrain::Tundra => b't',
        };
        let mut extras = (u64::from(tile.hut) << 5) | (u64::from(tile.river) << 17);
        if let Some(resource) = tile.resource {
            extras |= 1 << (18 + resource as u32);
        }
        for value in [u64::from(terrain), tile.altitude as u64, extras] {
            checksum = (checksum ^ value).wrapping_mul(0x100000001b3);
        }
    }
    checksum
}

#[cfg(test)]
#[test]
fn default_worlds_match_civcraft_resource_goldens() {
    // CivCraft keeps Freeciv's terrain generation but uses its own resource
    // catalog and terrain pools. Resource placement affects both the extras
    // checksums and start-site scoring, so these goldens capture current
    // CivCraft output. Coordinates are in creation order.
    let cases = [
        (
            1,
            0x6214fdbf48571f05,
            [(22, 15), (20, 2), (27, 16), (1, 25), (4, 8), (24, 31)],
        ),
        (
            42,
            0x5a61be89c37ce1c0,
            [(9, 26), (12, 6), (2, 37), (27, 25), (8, 6), (4, 26)],
        ),
        (
            12345,
            0x28eb54b184b571d8,
            [(15, 19), (22, 12), (29, 23), (25, 22), (11, 23), (18, 5)],
        ),
        (
            700001,
            0x8c1b425ae87e347f,
            [(28, 57), (23, 3), (20, 46), (26, 13), (22, 56), (2, 12)],
        ),
        (
            2147483647,
            0xc9f5f1b2c9d04e2f,
            [(12, 57), (20, 40), (2, 20), (27, 35), (9, 31), (26, 54)],
        ),
    ];
    for (seed, expected, starts) in cases {
        let map = generate(seed);
        assert_eq!(
            map_checksum(&map.tiles),
            expected,
            "terrain and extras for seed {seed}"
        );
        assert_eq!(map.starts, starts, "starting positions for seed {seed}");
    }
}

#[cfg(test)]
mod wrap_generation_tests {
    use super::*;

    #[test]
    fn ivory_can_spawn_on_plains_in_default_wrap_x_campaigns() {
        let map = generate_with_wrap(42, MapWrap::X);
        assert!(map.tiles.iter().any(|tile| {
            tile.terrain == Terrain::Plains && tile.resource == Some(Resource::Ivory)
        }));
    }

    #[test]
    fn cylinders_have_ocean_edges_without_polar_land_or_invalid_start_positions() {
        for seed in [0, 1, 42, 1337, 12345, 700001, 2147483647] {
            let a = generate_with_wrap(seed, MapWrap::X);
            let b = generate_with_wrap(seed, MapWrap::X);
            assert_eq!(a.map_wrap, MapWrap::X);
            assert_eq!(a.starts, b.starts);
            assert_eq!(
                bincode::serialize(&a.tiles).unwrap(),
                bincode::serialize(&b.tiles).unwrap()
            );
            assert!(
                a.tiles
                    .iter()
                    .all(|t| !matches!(t.terrain, Terrain::Glacier | Terrain::Tundra))
            );
            for y in [0, a.map_wrap.height() - 1] {
                for x in 0..a.map_wrap.width() {
                    let tile = &a.tiles[a.map_wrap.index(x, y)];
                    assert_eq!(tile.terrain, Terrain::DeepOcean);
                    assert!(!tile.hut && !tile.river && tile.resource.is_none());
                }
            }
            assert!(
                a.starts
                    .iter()
                    .all(|&(x, y)| !a.tiles[a.map_wrap.index(x, y)].terrain.is_water())
            );
            let g = Generator { wrap: MapWrap::X };
            assert_eq!(g.colatitude(0), 0);
            assert_eq!(g.colatitude(g.wrap.tile_count() - 1), 0);
            assert!(g.colatitude(g.index(10, g.wrap.height() / 2)) > 950);
            assert!(
                g.neighbors(g.index(10, 0))
                    .all(|i| g.wrap.position(i).1 < g.wrap.height() / 2)
            );
            assert_ne!(
                bincode::serialize(&a.tiles).unwrap(),
                bincode::serialize(&generate(seed).tiles).unwrap()
            );
        }
    }
}

#[cfg(test)]
#[test]
fn generated_worlds_match_civcraft_resource_goldens_for_each_wrap() {
    for line in include_str!("../../tests/fixtures/civcraft_mapgen_vectors.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        let wrap: MapWrap = fields[0].parse().unwrap();
        let seed: u64 = fields[1].parse().unwrap();
        let width: i32 = fields[2].parse().unwrap();
        let height: i32 = fields[3].parse().unwrap();
        assert_eq!((wrap.width(), wrap.height()), (width, height));
        let map = Generator { wrap }.generate(seed);
        let checksum = map_checksum(&map.tiles);
        assert_eq!(
            checksum,
            u64::from_str_radix(fields[4], 16).unwrap(),
            "terrain/altitude/extras for {wrap} seed {seed}"
        );
        let mut starts = map.starts.to_vec();
        starts.sort_unstable();
        let expected: Vec<(i32, i32)> = fields[5]
            .split(';')
            .map(|pair| {
                let (x, y) = pair.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        assert_eq!(
            starts, expected,
            "starting locations for {wrap} seed {seed}"
        );
    }
}
