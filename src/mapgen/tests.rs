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
        assert!(map.starts.iter().all(|&(x, y)| {
            !tiles[(Generator {
                wrap: MapWrap::XY,
                settings: Civ3WorldSettings::default(),
            })
            .index(x, y)]
            .hut
        }));

        let mut visited = vec![false; TILE_COUNT];
        for (index, tile) in tiles.iter().enumerate() {
            assert!((0..=1000).contains(&tile.altitude));
            if tile.hut {
                assert!(!tile.terrain.is_water());
                assert!(
                    (Generator {
                        wrap: MapWrap::XY,
                        settings: Civ3WorldSettings::default(),
                    })
                    .within(index, 3)
                    .all(|next| next == index || !tiles[next].hut)
                );
            }
            if tile.resource.is_some() {
                assert!(
                    (Generator {
                        wrap: MapWrap::XY,
                        settings: Civ3WorldSettings::default(),
                    })
                    .neighbors(index)
                    .all(|next| tiles[next].resource.is_none())
                );
                assert!(
                    tile.terrain
                        .resource_options()
                        .contains(&tile.resource.unwrap())
                );
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
                has_outlet |= tiles[current].terrain == Terrain::Tundra;
                for next in (Generator {
                    wrap: MapWrap::XY,
                    settings: Civ3WorldSettings::default(),
                })
                .civ3_neighbors(current)
                {
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
#[test]
fn civ3_world_generation_does_not_emit_legacy_glacier_or_lake_terrains() {
    for wrap in [MapWrap::X, MapWrap::XY] {
        for seed in [42, 12345] {
            let map = generate_with_wrap(seed, wrap);
            assert!(
                map.tiles
                    .iter()
                    .all(|tile| { !matches!(tile.terrain, Terrain::Glacier | Terrain::Lake) })
            );
        }
    }
}

#[cfg(test)]
fn map_checksum(tiles: &[Tile]) -> u64 {
    let mut checksum = 0xcbf29ce484222325_u64;
    for tile in tiles {
        let terrain = match tile.terrain {
            Terrain::Sea => b' ',
            Terrain::Ocean => b':',
            Terrain::Coast => b'c',
            Terrain::Lake => b'+',
            Terrain::Glacier => b'a',
            Terrain::Desert => b'd',
            Terrain::Forest => b'f',
            Terrain::Grassland => b'g',
            Terrain::GrasslandShield => b'q',
            Terrain::Hills => b'h',
            Terrain::Jungle => b'j',
            Terrain::Mountains => b'm',
            Terrain::Plains => b'p',
            Terrain::Marsh => b's',
            Terrain::Tundra => b't',
            Terrain::FloodPlains => b'u',
            Terrain::Volcano => b'v',
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
    // These capture generated terrain, resource placement, and sorted starts
    // using Civ III's eight-direction topology.
    let cases: [(u64, u64, [(i32, i32); 6]); 5] = [
        (
            1,
            0xeb916cf67f024044,
            [(5, 20), (20, 40), (21, 10), (23, 24), (23, 60), (30, 6)],
        ),
        (
            42,
            0xb2902b18950323c0,
            [(2, 43), (6, 19), (12, 6), (13, 28), (20, 41), (25, 22)],
        ),
        (
            12345,
            0x1b2f3192ba21e8e4,
            [(4, 39), (5, 12), (11, 49), (14, 18), (17, 43), (26, 8)],
        ),
        (
            700001,
            0x84a7eedd9e6cafd1,
            [(6, 7), (11, 39), (17, 20), (22, 35), (28, 10), (28, 50)],
        ),
        (
            2147483647,
            0x61807fce6d563209,
            [(3, 49), (9, 37), (15, 54), (20, 43), (24, 30), (30, 23)],
        ),
    ];
    for (seed, expected, expected_starts) in cases {
        let map = generate(seed);
        let mut starts = map.starts.to_vec();
        starts.sort_unstable();
        assert_eq!(
            map_checksum(&map.tiles),
            expected,
            "terrain and extras for seed {seed}"
        );
        assert_eq!(
            starts, expected_starts,
            "starting positions for seed {seed}"
        );
    }
}

#[test]
fn civ3_ocean_coverage_choices_change_water_fraction_deterministically() {
    let wrap = MapWrap::XY;
    let coverages = [
        OceanCoverage::Percent60,
        OceanCoverage::Percent70,
        OceanCoverage::Percent80,
    ];
    let water_counts: Vec<_> = coverages
        .into_iter()
        .map(|ocean_coverage| {
            let settings = Civ3WorldSettings { ocean_coverage };
            let first = generate_with_settings(42, wrap, settings);
            let second = generate_with_settings(42, wrap, settings);
            assert_eq!(
                bincode::serialize(&first.tiles).unwrap(),
                bincode::serialize(&second.tiles).unwrap()
            );
            first
                .tiles
                .iter()
                .filter(|tile| tile.terrain.is_water())
                .count()
        })
        .collect();
    assert!(water_counts[0] < water_counts[1]);
    assert!(water_counts[1] < water_counts[2]);
    for (count, coverage) in water_counts.into_iter().zip([60_usize, 70, 80]) {
        let actual = count * 100 / MapWrap::XY.tile_count();
        assert!(
            actual.abs_diff(coverage) <= 1,
            "requested {coverage}% ocean coverage but generated {actual}%"
        );
    }
}

#[test]
fn civ3_ocean_coverage_parser_accepts_documented_world_setup_values() {
    assert_eq!("60".parse(), Ok(OceanCoverage::Percent60));
    assert_eq!("70".parse(), Ok(OceanCoverage::Percent70));
    assert_eq!("80".parse(), Ok(OceanCoverage::Percent80));
    assert!("75".parse::<OceanCoverage>().is_err());
}

#[cfg(test)]
mod wrap_generation_tests {
    use super::*;

    #[test]
    fn generated_terrain_neighbors_use_reciprocal_civ3_eight_way_topology() {
        for wrap in [MapWrap::X, MapWrap::XY] {
            let generator = Generator {
                wrap,
                settings: Civ3WorldSettings::default(),
            };
            let y = wrap.height() / 2;
            for x in [0, wrap.width() / 2, wrap.width() - 1] {
                let center = generator.index(x, y);
                let neighbors: Vec<_> = generator.neighbors(center).collect();
                assert_eq!(neighbors.len(), 8, "{wrap} neighbor count at {x},{y}");
                let unique: std::collections::HashSet<_> = neighbors.iter().copied().collect();
                assert_eq!(unique.len(), 8, "duplicate neighbor at {wrap} {x},{y}");
                for &neighbor in &neighbors {
                    assert!(
                        generator
                            .civ3_neighbors(neighbor)
                            .any(|tile| tile == center)
                    );
                    assert_eq!(generator.distance(center, neighbor), 1);
                }
                assert_eq!(
                    generator
                        .within(center, 1)
                        .collect::<std::collections::HashSet<_>>(),
                    unique.into_iter().chain([center]).collect()
                );
            }
        }
    }

    #[test]
    fn ivory_can_spawn_on_plains_in_default_wrap_x_campaigns() {
        let map = generate_with_wrap(42, MapWrap::X);
        assert!(map.tiles.iter().any(|tile| {
            tile.terrain == Terrain::Plains && tile.resource == Some(Resource::Ivory)
        }));
    }

    #[test]
    fn civ3_cylinder_reserves_poles_for_polar_caps_and_has_valid_starts() {
        for seed in [0, 1, 42, 1337, 12345, 700001, 2147483647] {
            let a = generate_with_wrap(seed, MapWrap::X);
            let b = generate_with_wrap(seed, MapWrap::X);
            assert_eq!(a.map_wrap, MapWrap::X);
            assert!(a.tiles.iter().all(|tile| tile.terrain != Terrain::Glacier));
            assert_eq!(a.starts, b.starts);
            assert_eq!(
                bincode::serialize(&a.tiles).unwrap(),
                bincode::serialize(&b.tiles).unwrap()
            );
            for y in [0, a.map_wrap.height() - 1] {
                for x in 0..a.map_wrap.width() {
                    let edge = &a.tiles[a.map_wrap.index(x, y)];
                    assert_eq!(edge.terrain, Terrain::Ocean);
                    assert!(edge.resource.is_none() && !edge.river && !edge.hut);
                }
            }
            assert!(
                a.starts
                    .iter()
                    .all(|&(x, y)| !a.tiles[a.map_wrap.index(x, y)].terrain.is_water())
            );
            let g = Generator {
                wrap: MapWrap::X,
                settings: Civ3WorldSettings::default(),
            };
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

    #[test]
    fn civ3_shelf_rules_put_coast_by_land_and_sea_by_coast() {
        for wrap in [MapWrap::X, MapWrap::XY] {
            let generator = Generator {
                wrap,
                settings: Civ3WorldSettings::default(),
            };
            let land = generator.index(wrap.width() / 2, wrap.height() / 2);
            let mut terrain = vec![Terrain::Ocean; wrap.tile_count()];
            terrain[land] = Terrain::Grassland;
            generator.finish_water(&mut terrain, &mut MapRng::new(0));

            for (tile, &water) in terrain.iter().enumerate() {
                let adjacent: Vec<_> = generator.neighbors(tile).collect();
                if water.is_water() && adjacent.iter().any(|&next| !terrain[next].is_water()) {
                    assert_eq!(
                        water,
                        Terrain::Coast,
                        "shoreline at {:?}",
                        wrap.position(tile)
                    );
                }
                if water == Terrain::Ocean {
                    assert!(
                        adjacent.iter().all(|&next| terrain[next] != Terrain::Coast),
                        "ocean directly borders coast at {:?} on {wrap}",
                        wrap.position(tile)
                    );
                }
            }
            assert!(terrain.contains(&Terrain::Coast));
            assert!(terrain.contains(&Terrain::Sea));
            assert!(terrain.contains(&Terrain::Ocean));
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
        let map = generate_with_wrap(seed, wrap);
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
