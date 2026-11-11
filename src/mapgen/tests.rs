//! Regression tests for the behavior of the parent module.

use super::*;
use crate::world::Resource;

const TILE_COUNT: usize = (MAP_W * MAP_H) as usize;

#[test]
fn civ3_map_sizes_select_their_rosters_and_generate_matching_starts() {
    for (size, expected) in [
        (Civ3MapSize::Tiny, 4),
        (Civ3MapSize::Small, 6),
        (Civ3MapSize::Standard, 8),
        (Civ3MapSize::Large, 12),
        (Civ3MapSize::Huge, 16),
    ] {
        assert_eq!(size.civilization_count(), expected);
    }

    for (size, expected) in [
        (Civ3MapSize::Tiny, 4),
        (Civ3MapSize::Small, 6),
        (Civ3MapSize::Standard, 8),
        (Civ3MapSize::Large, 12),
        (Civ3MapSize::Huge, 16),
    ] {
        let map = generate_for_civ3_size(42, size, Civ3WorldSettings::default());
        assert_eq!(map.civilization_count, expected, "{size}");
        assert_eq!(map.starts.len(), expected, "{size}");
        assert_eq!(
            map.tiles.len(),
            size.topology().unwrap().tile_count(),
            "{size}"
        );
    }
}

#[test]
fn map_cursor_conversion_continues_the_same_native_stream() {
    for seed in [0, 42, u64::MAX] {
        let mut map = MapRng::new(seed);
        // Cross many ring boundaries and rejection-heavy ranges.
        for _ in 0..137 {
            map.draw(2_147_483_649);
        }
        let mut game = crate::random::FreecivRng::from_map_state(map.values, map.next).unwrap();
        for limit in [0, 1, 2, 3, 100, 2_147_483_649, u32::MAX].repeat(23) {
            assert_eq!(game.draw(limit), map.draw(limit));
        }
    }
    assert!(crate::random::FreecivRng::from_map_state([0; 56], 56).is_err());
}

#[test]
fn generated_rivers_reach_water_or_the_poles_and_resources_are_spaced() {
    for seed in [0, 42, 12345] {
        let map = generate(seed);
        let tiles = &map.tiles;
        assert!(tiles.iter().any(|tile| tile.river));
        assert!(
            tiles.iter().all(|tile| tile.river_edges & 0x55 == 0),
            "generated rivers use only the four junction-sheet directions for seed {seed}"
        );
        assert!(tiles.iter().any(|tile| tile.resource.is_some()));
        assert!(tiles.iter().any(|tile| tile.hut));
        assert!(map.starts.iter().all(|&(x, y)| {
            !tiles[(Generator {
                wrap: MapWrap::X,
                settings: Civ3WorldSettings::default(),
                civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
            })
            .index(x, y)]
            .hut
        }));

        let mut visited = vec![false; TILE_COUNT];
        for (index, tile) in tiles.iter().enumerate() {
            assert!((0..=1000).contains(&tile.altitude));
            if tile.terrain.is_water() {
                let (x, y) = map.map_wrap.position(index);
                for direction in 0..CIV3_DIRS.len() {
                    if tile.river_edges & (1 << direction) == 0 {
                        continue;
                    }
                    let (nx, ny) = map
                        .map_wrap
                        .civ3_neighbor(x, y, direction)
                        .expect("generated river mouth stays inside the map");
                    assert!(
                        !tiles[map.map_wrap.index(nx, ny)].terrain.is_water(),
                        "river mouth at {index} extends beyond the shoreline for seed {seed}"
                    );
                }
            }
            if tile.hut {
                assert!(!tile.terrain.is_water());
                assert!(
                    (Generator {
                        wrap: MapWrap::X,
                        settings: Civ3WorldSettings::default(),
                        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
                    })
                    .within(index, 3)
                    .all(|next| next == index || !tiles[next].hut)
                );
            }
            if tile.resource.is_some() {
                assert!(
                    (Generator {
                        wrap: MapWrap::X,
                        settings: Civ3WorldSettings::default(),
                        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
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
                    wrap: MapWrap::X,
                    settings: Civ3WorldSettings::default(),
                    civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
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

#[test]
fn river_course_can_turn_toward_a_nearby_outlet() {
    let map = generate(42);
    let wrap = map.map_wrap;
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
    };
    let mut tiles = map.tiles;
    for tile in &mut tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.river_edges = 0;
    }

    let spring = wrap.index(5, 8);
    let northeast = wrap.index(5, 7);
    let east = wrap.index(6, 8);
    let outlet = wrap.index(6, 9);
    tiles[outlet].terrain = Terrain::Coast;

    let mut heights = vec![100; wrap.tile_count()];
    heights[northeast] = 0;
    let course = generator
        .trace_river(&tiles, &heights, spring, &mut MapRng::new(7))
        .expect("river reaches the nearby outlet");

    assert_eq!(course, [spring, northeast, east]);
}

#[cfg(test)]
#[test]
fn civ3_world_generation_does_not_emit_legacy_glacier_or_lake_terrains() {
    for wrap in [MapWrap::X] {
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
fn map_checksum(tiles: &[GeneratedTile]) -> u64 {
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
            0xc99e25509c307d15,
            [(0, 9), (1, 28), (9, 36), (13, 40), (15, 32), (20, 43)],
        ),
        (
            42,
            0xcbe1d1150de36cb4,
            [(8, 23), (16, 44), (17, 21), (19, 36), (22, 15), (29, 30)],
        ),
        (
            12345,
            0x3878fb1c3a928403,
            [(1, 21), (7, 42), (15, 14), (16, 27), (28, 37), (32, 18)],
        ),
        (
            700001,
            0x67c4a644e69da62c,
            [(2, 42), (5, 22), (8, 39), (16, 10), (18, 26), (21, 39)],
        ),
        (
            2147483647,
            0x9b86547b7c1c92d8,
            [(18, 24), (18, 41), (20, 12), (26, 31), (28, 43), (32, 9)],
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
    let wrap = MapWrap::X;
    let coverages = [
        OceanCoverage::Percent60,
        OceanCoverage::Percent70,
        OceanCoverage::Percent80,
    ];
    let water_counts: Vec<_> = coverages
        .into_iter()
        .map(|ocean_coverage| {
            let settings = Civ3WorldSettings {
                ocean_coverage,
                climate: Climate::default(),
                age: WorldAge::default(),
                landform: Landform::default(),
                temperature: Temperature::default(),
            };
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
        let actual = count * 100 / MapWrap::X.tile_count();
        assert!(
            actual.abs_diff(coverage) <= 1,
            "requested {coverage}% ocean coverage but generated {actual}%"
        );
    }
}

#[test]
fn civ3_climate_choices_shift_biomes_in_the_expected_direction() {
    let wrap = MapWrap::X;
    let map_for = |climate| {
        generate_with_settings(
            42,
            wrap,
            Civ3WorldSettings {
                ocean_coverage: OceanCoverage::default(),
                climate,
                age: WorldAge::default(),
                landform: Landform::default(),
                temperature: Temperature::default(),
            },
        )
    };
    let wet = map_for(Climate::Wet);
    let arid = map_for(Climate::Arid);
    let count = |map: &GeneratedMap, terrain| {
        map.tiles
            .iter()
            .filter(|tile| tile.terrain == terrain)
            .count()
    };
    assert!(count(&wet, Terrain::Forest) > count(&arid, Terrain::Forest));
    assert!(count(&wet, Terrain::Marsh) > count(&arid, Terrain::Marsh));
    assert!(count(&arid, Terrain::Desert) > count(&wet, Terrain::Desert));
    // OpenCiv3 applies an arid desert boost; it does not define a separate
    // normal-versus-wet desert ordering.
}

#[test]
fn civ3_world_age_controls_relief_and_volcano_frequency() {
    let generate_age = |age| {
        generate_with_settings(
            42,
            MapWrap::X,
            Civ3WorldSettings {
                ocean_coverage: OceanCoverage::default(),
                climate: Climate::default(),
                age,
                landform: Landform::default(),
                temperature: Temperature::default(),
            },
        )
    };
    let young = generate_age(WorldAge::Billion3);
    let middle = generate_age(WorldAge::Billion4);
    let old = generate_age(WorldAge::Billion5);
    let relief_count = |map: &GeneratedMap| {
        map.tiles
            .iter()
            .filter(|tile| {
                matches!(
                    tile.terrain,
                    Terrain::Hills | Terrain::Mountains | Terrain::Volcano
                )
            })
            .count()
    };
    let volcano_count = |map: &GeneratedMap| {
        map.tiles
            .iter()
            .filter(|tile| tile.terrain == Terrain::Volcano)
            .count()
    };
    assert!(relief_count(&young) > relief_count(&middle));
    assert!(relief_count(&middle) > relief_count(&old));
    assert!(volcano_count(&young) > volcano_count(&old));
}

#[test]
fn civ3_landform_choices_are_deterministic_and_change_landmass_scale() {
    let forms = [
        Landform::Archipelago,
        Landform::Continents,
        Landform::Pangaea,
    ];
    let maps: Vec<_> = forms
        .into_iter()
        .map(|landform| {
            let settings = Civ3WorldSettings {
                landform,
                ..Civ3WorldSettings::default()
            };
            let first = generate_with_settings(42, MapWrap::X, settings);
            let second = generate_with_settings(42, MapWrap::X, settings);
            assert_eq!(
                bincode::serialize(&first.tiles).unwrap(),
                bincode::serialize(&second.tiles).unwrap()
            );
            first
        })
        .collect();
    let checksums: Vec<_> = maps.iter().map(|map| map_checksum(&map.tiles)).collect();
    let topology = Generator {
        wrap: MapWrap::X,
        settings: Civ3WorldSettings::default(),
        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
    };
    let land_components: Vec<_> = maps
        .iter()
        .map(|map| {
            let terrain: Vec<_> = map.tiles.iter().map(|tile| tile.terrain).collect();
            let components = topology.components(&terrain, false);
            (
                components.len(),
                components.iter().map(Vec::len).max().unwrap_or(0),
            )
        })
        .collect();
    assert_ne!(checksums[0], checksums[1]);
    assert_ne!(checksums[1], checksums[2]);
    assert_ne!(checksums[0], checksums[2]);
    assert!(land_components[0].0 > land_components[1].0);
    assert!(land_components[1].0 > land_components[2].0);
    assert!(land_components[2].1 > land_components[0].1);
}

#[test]
fn landform_parser_accepts_civ3_world_setup_choices() {
    assert_eq!("archipelago".parse(), Ok(Landform::Archipelago));
    assert_eq!("CONTINENTS".parse(), Ok(Landform::Continents));
    assert_eq!("pangaea".parse(), Ok(Landform::Pangaea));
    assert!("islands".parse::<Landform>().is_err());
}

#[test]
fn civ3_temperature_choices_shift_cold_and_dry_terrain() {
    let generate_temperature = |temperature| {
        generate_with_settings(
            42,
            MapWrap::X,
            Civ3WorldSettings {
                temperature,
                ..Civ3WorldSettings::default()
            },
        )
    };
    let cool = generate_temperature(Temperature::Cool);
    let temperate = generate_temperature(Temperature::Temperate);
    let warm = generate_temperature(Temperature::Warm);
    let count = |map: &GeneratedMap, terrain| {
        map.tiles
            .iter()
            .filter(|tile| tile.terrain == terrain)
            .count()
    };
    assert!(count(&cool, Terrain::Tundra) > count(&warm, Terrain::Tundra));
    assert!(count(&warm, Terrain::Desert) > count(&cool, Terrain::Desert));
    assert_eq!(
        bincode::serialize(&temperate.tiles).unwrap(),
        bincode::serialize(&generate_temperature(Temperature::Temperate).tiles).unwrap()
    );
}

#[test]
fn temperature_parser_accepts_civ3_world_setup_choices() {
    assert_eq!("cool".parse(), Ok(Temperature::Cool));
    assert_eq!("TEMPERATE".parse(), Ok(Temperature::Temperate));
    assert_eq!("warm".parse(), Ok(Temperature::Warm));
    assert!("hot".parse::<Temperature>().is_err());
}

#[test]
fn civ3_ocean_coverage_parser_accepts_documented_world_setup_values() {
    assert_eq!("60".parse(), Ok(OceanCoverage::Percent60));
    assert_eq!("70".parse(), Ok(OceanCoverage::Percent70));
    assert_eq!("80".parse(), Ok(OceanCoverage::Percent80));
    assert!("75".parse::<OceanCoverage>().is_err());
}

#[test]
fn civ3_climate_parser_accepts_documented_world_setup_values() {
    assert_eq!("arid".parse(), Ok(Climate::Arid));
    assert_eq!("normal".parse(), Ok(Climate::Normal));
    assert_eq!("wet".parse(), Ok(Climate::Wet));
    assert!("humid".parse::<Climate>().is_err());
}

#[test]
fn civ3_world_age_parser_accepts_documented_world_setup_values() {
    assert_eq!("3".parse(), Ok(WorldAge::Billion3));
    assert_eq!("4".parse(), Ok(WorldAge::Billion4));
    assert_eq!("5".parse(), Ok(WorldAge::Billion5));
    assert!("6".parse::<WorldAge>().is_err());
}

#[cfg(test)]
mod wrap_generation_tests {
    use super::*;

    #[test]
    fn generated_terrain_neighbors_use_reciprocal_civ3_eight_way_topology() {
        for wrap in [MapWrap::X] {
            let generator = Generator {
                wrap,
                settings: Civ3WorldSettings::default(),
                civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
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
                civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
            };
            assert_eq!(g.colatitude(0), 0);
            assert_eq!(g.colatitude(g.wrap.tile_count() - 1), 0);
            assert!(g.colatitude(g.index(10, g.wrap.height() / 2)) > 950);
            assert!(
                g.neighbors(g.index(10, 0))
                    .all(|i| g.wrap.position(i).1 < g.wrap.height() / 2)
            );
            assert_eq!(
                bincode::serialize(&a.tiles).unwrap(),
                bincode::serialize(&generate(seed).tiles).unwrap()
            );
        }
    }

    #[test]
    fn civ3_shelf_rules_put_coast_by_land_and_sea_by_coast() {
        for wrap in [MapWrap::X] {
            let generator = Generator {
                wrap,
                settings: Civ3WorldSettings::default(),
                civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
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
        let mut starts = map.starts.to_vec();
        starts.sort_unstable();
        assert_eq!(
            checksum,
            u64::from_str_radix(fields[4], 16).unwrap(),
            "terrain/altitude/extras for {wrap} seed {seed}"
        );
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
