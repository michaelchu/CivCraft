//! Regression tests for the behavior of the parent module.

use super::*;
use crate::world::Resource;

const TILE_COUNT: usize = (MAP_W * MAP_H) as usize;

fn generate_opensimplex(seed: u64, wrap: MapWrap, settings: Civ3WorldSettings) -> GeneratedMap {
    let request = map_generation_request(
        seed,
        wrap,
        settings,
        crate::world::DEFAULT_CIVILIZATION_COUNT,
    );
    generate_with_map_generator(&OpenSimplexMapGenerator, request)
}

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
fn map_rng_uses_the_shared_freeciv_primitive() {
    for seed in [0, 42, u64::MAX] {
        let mut map = MapRng::new(seed);
        let folded_seed = seed as u32 ^ (seed >> 32) as u32;
        let mut game = crate::random::FreecivRng::new(folded_seed);
        // Cross many ring boundaries and rejection-heavy ranges.
        for _ in 0..137 {
            assert_eq!(map.draw(2_147_483_649), game.draw(2_147_483_649));
        }
        for limit in [0, 1, 2, 3, 100, 2_147_483_649, u32::MAX].repeat(23) {
            assert_eq!(game.draw(limit), map.draw(limit));
        }
    }
}

#[test]
fn campaign_seed_draws_a_distinct_map_seed_and_preserves_game_rng_state() {
    for seed in [1, 42, 12345, u64::MAX] {
        let (map_seed, gameplay_rng) = campaign_randomness(seed);
        let mut expected_rng = crate::random::FreecivRng::new(seed as u32);
        let mut expected_seed = expected_rng.draw(u32::MAX) & (u32::MAX >> 1);
        while expected_seed == 0 {
            expected_seed = expected_rng.draw(u32::MAX) & (u32::MAX >> 1);
        }
        assert_eq!(map_seed, expected_seed);
        assert_eq!(gameplay_rng, expected_rng);
        assert_ne!(u64::from(map_seed), seed);
    }
}

#[test]
fn freeciv_mapgen_retries_draw_fresh_seeds_from_the_gameplay_stream() {
    let seed = 42;
    let (first, mut gameplay_rng) = campaign_randomness(seed);
    let second = draw_freeciv_map_seed(&mut gameplay_rng);
    let third = draw_freeciv_map_seed(&mut gameplay_rng);

    let mut source_rng = crate::random::FreecivRng::new(seed as u32);
    let source_seeds = (0..3)
        .map(|_| {
            loop {
                let map_seed = source_rng.draw(u32::MAX) & (u32::MAX >> 1);
                if map_seed != 0 {
                    break map_seed;
                }
            }
        })
        .collect::<Vec<_>>();

    assert_eq!([first, second, third], source_seeds.as_slice());
    assert_ne!(first, second);
    assert_ne!(second, third);
    assert_eq!(FREECIV_MAPGEN_MAX_ATTEMPTS, 3);
}

#[test]
fn generated_campaign_rng_is_restored_for_start_allocation() {
    let seed = 42;
    let (map_seed, initial_gameplay_rng) = campaign_randomness(seed);
    let map = generate(seed);
    assert_eq!(map.seed, seed);
    assert_eq!(map.map_seed, map_seed);
    assert_ne!(map.game_random, initial_gameplay_rng);
}

#[test]
fn c2c3_settings_change_freeciv_height_resolution_and_shore_mask() {
    let wrap = MapWrap::with_dimensions(36, 48).unwrap();
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: 8,
    };
    let c2c3 = crate::rules::freeciv::RulesetData::campaign_default()
        .map_generation_settings()
        .unwrap();
    let request = map_generation_request(42, wrap, Civ3WorldSettings::default(), 8);
    assert_eq!(request.map_generation_settings, c2c3);

    let mut default_rng = MapRng::new(123);
    let default_field = generator.freeciv_fractal_height_field(
        &mut default_rng,
        c2c3.startpos.fractal_divisions(8),
        c2c3.landmass_percent,
        true,
    );
    let default_land_tiles = freeciv_land_mask(&default_field, c2c3)
        .into_iter()
        .filter(|is_land| *is_land)
        .count();
    let more_land = MapGenerationSettings {
        landmass_percent: 40,
        ..c2c3
    };
    let more_land_tiles = freeciv_land_mask(&default_field, more_land)
        .into_iter()
        .filter(|is_land| *is_land)
        .count();
    assert!(more_land_tiles > default_land_tiles);

    let more_divisions = MapGenerationSettings {
        startpos: crate::rules::freeciv::StartPositionSetting::Single,
        ..c2c3
    };
    let mut single_start_rng = MapRng::new(123);
    let single_start_field = generator.freeciv_fractal_height_field(
        &mut single_start_rng,
        more_divisions.startpos.fractal_divisions(8),
        more_divisions.landmass_percent,
        true,
    );
    assert_ne!(default_field, single_start_field);
}

#[test]
fn freeciv_fractal_height_consumes_jitter_for_shared_and_bounded_midpoints() {
    fn subdivision_calls(xl: i32, yt: i32, xr: i32, yb: i32) -> usize {
        if xr - xl <= 0 || yb - yt <= 0 || (xr - xl == 1 && yb - yt == 1) {
            return 0;
        }
        let xmid = (xl + xr) / 2;
        let ymid = (yt + yb) / 2;
        1 + subdivision_calls(xl, yt, xmid, ymid)
            + subdivision_calls(xl, ymid, xmid, yb)
            + subdivision_calls(xmid, yt, xr, ymid)
            + subdivision_calls(xmid, ymid, xr, yb)
    }

    let wrap = MapWrap::with_dimensions(36, 48).unwrap();
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: 8,
    };
    let settings = crate::rules::freeciv::RulesetData::campaign_default()
        .map_generation_settings()
        .unwrap();
    let divisions = settings.startpos.fractal_divisions(8);
    let step = wrap.width() + wrap.height();
    let mut expected = crate::random::FreecivRng::new(123);

    // Initial grid, five midpoint calculations per recursive call, then
    // Freeciv's final per-tile fuzz pass.
    for _ in 0..divisions * (divisions + 1) {
        expected.draw((2 * step) as u32);
    }
    let mut calls = 0;
    for x in 0..divisions {
        for y in 0..divisions {
            calls += subdivision_calls(
                x * wrap.width() / divisions,
                y * (wrap.height() - 1) / divisions,
                (x + 1) * wrap.width() / divisions,
                (y + 1) * (wrap.height() - 1) / divisions,
            );
        }
    }
    for _ in 0..calls * 5 {
        expected.draw(step as u32);
    }
    for _ in 0..wrap.tile_count() {
        expected.draw(4);
    }

    let mut map_rng = MapRng::new(123);
    generator.freeciv_fractal_height_field(
        &mut map_rng,
        divisions,
        settings.landmass_percent,
        true,
    );
    assert_eq!(map_rng.draw(1000), expected.draw(1000));
}

#[test]
fn c2c3_ocean_depths_are_parsed_and_mapped_to_civ3_water_labels() {
    let settings = crate::rules::freeciv::RulesetData::campaign_default()
        .map_generation_settings()
        .unwrap();
    let depths = settings.ocean_depths;
    assert_eq!(
        depths,
        OceanDepthSettings {
            lake: 0,
            lake_unsafe_coast: false,
            ocean: 35,
            ocean_unsafe_coast: true,
            deep_ocean: 70,
            deep_ocean_unsafe_coast: true,
        }
    );
    // Freeciv excludes depth-zero Lake from `pick_ocean`; the 52.5 midpoint
    // ties to the first eligible C2C3 terrain, Ocean, which maps to Civ III Sea.
    assert_eq!(depths.civ3_ocean_for_depth(52), Terrain::Sea);
    assert_eq!(depths.civ3_ocean_for_depth(53), Terrain::Ocean);

    let wrap = MapWrap::with_dimensions(16, 16).unwrap();
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: 4,
    };
    let mut terrain = vec![Terrain::Grassland; wrap.tile_count()];
    terrain[wrap.index(8, 8)] = Terrain::Ocean;
    generator.finish_freeciv_lakes(&mut terrain);
    assert_eq!(terrain[wrap.index(8, 8)], Terrain::Coast);
}

#[test]
fn freeciv_ocean_depth_stages_use_height_and_the_civ3_wrap_topology() {
    let wrap = MapWrap::with_dimensions(16, 16).unwrap();
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: 4,
    };
    let depths = crate::rules::freeciv::RulesetData::campaign_default()
        .map_generation_settings()
        .unwrap()
        .ocean_depths;
    let mut heights = vec![0; wrap.tile_count()];
    let mut land_mask = vec![false; wrap.tile_count()];
    let land = wrap.index(8, 8);
    heights[land] = 700;
    land_mask[land] = true;
    let near_land = generator.neighbors(land).next().unwrap();
    heights[near_land] = 690;
    let mut terrain = vec![Terrain::Ocean; wrap.tile_count()];
    terrain[land] = Terrain::Grassland;

    generator.assign_freeciv_water_depths(&mut terrain, &heights, &land_mask, 700, depths);
    assert_eq!(terrain[near_land], Terrain::Sea);
    assert_eq!(terrain[wrap.index(0, 0)], Terrain::Ocean);

    // Depth smoothing crosses the wrapped X seam, while Y remains bounded.
    let mut topology_terrain = vec![Terrain::Ocean; wrap.tile_count()];
    topology_terrain[wrap.index(0, 0)] = Terrain::Grassland;
    generator.finish_freeciv_water_depths(&mut topology_terrain, depths, &mut MapRng::new(42));
    assert_eq!(
        topology_terrain[wrap.index(wrap.width() - 1, 0)],
        Terrain::Sea
    );
    assert_eq!(
        topology_terrain[wrap.index(0, wrap.height() - 1)],
        Terrain::Ocean
    );
}

#[test]
fn generated_rivers_reach_water_or_the_poles_and_resources_are_spaced() {
    let generator = Generator {
        wrap: MapWrap::X,
        settings: Civ3WorldSettings::default(),
        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
    };
    let freeciv_settings = RulesetData::campaign_default()
        .map_generation_settings()
        .unwrap();
    let terrain_properties = RulesetData::campaign_default()
        .terrain_mapgen_properties()
        .unwrap();
    for seed in [0, 42, 12345] {
        let map = generate(seed);
        let tiles = &map.tiles;
        assert!(tiles.iter().any(|tile| tile.river));
        assert!(
            tiles.iter().all(|tile| tile.river_edges & 0x55 == 0),
            "generated rivers use only the four junction-sheet directions for seed {seed}"
        );
        assert!(
            tiles.iter().any(|tile| tile.river_edges != 0),
            "generated course steps must reach the Civ III river-sheet renderer for seed {seed}"
        );
        assert!(tiles.iter().any(|tile| tile.resource.is_some()));
        for (tile, generated) in tiles.iter().enumerate() {
            if generated.resource.is_some() {
                assert!(
                    generator
                        .within(tile, 1)
                        .all(|neighbor| neighbor == tile || tiles[neighbor].resource.is_none())
                );
            }
        }
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
                if matches!(tile.resource, Some(Resource::Fish | Resource::Whales)) {
                    let neighborhood: Vec<_> = generator
                        .within(index, 1)
                        .map(|near| {
                            (
                                near,
                                tiles[near].terrain,
                                placement::civ3_terrain_is_unsafe_coast(
                                    tiles[near].terrain,
                                    freeciv_settings.ocean_depths,
                                    &terrain_properties,
                                ),
                            )
                        })
                        .collect();
                    let near_safe_tile = neighborhood
                        .iter()
                        .any(|(_, _, unsafe_coast)| !unsafe_coast);
                    let next_to_polar_override = neighborhood.iter().any(|(near, _, _)| {
                        let (_, y) = generator.wrap.position(*near);
                        y == 0 || y == generator.wrap.height() - 1
                    });
                    assert!(
                        near_safe_tile || next_to_polar_override,
                        "aquatic resource at {index} has no safe tile within radius one for seed {seed}; C2C3 ocean_resources={} neighbors={neighborhood:?}",
                        freeciv_settings.ocean_resources,
                    );
                }
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
        .trace_river(&tiles, &heights, spring, None, &mut MapRng::new(7))
        .expect("river reaches the nearby outlet");

    assert_eq!(course, [spring, northeast, east]);
}

fn freeciv_river_test_map(
    wrap: MapWrap,
    terrain: Terrain,
) -> (Generator, Vec<GeneratedTile>, Vec<TerrainMapgenProperties>) {
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
    };
    let tiles = vec![
        GeneratedTile {
            terrain,
            base_terrain: TerrainBase::Grassland,
            altitude: 500,
            hut: false,
            river: false,
            river_edges: 0,
            resource: None,
        };
        wrap.tile_count()
    ];
    let properties = crate::rules::freeciv::RulesetData::campaign_default()
        .terrain_mapgen_properties()
        .expect("C2C3 terrain mapgen properties parse");
    (generator, tiles, properties)
}

#[test]
fn freeciv_river_ties_follow_ruleset_order_and_wrap_x_without_wrapping_y() {
    let wrap = MapWrap::X;
    let (generator, tiles, properties) = freeciv_river_test_map(wrap, Terrain::Grassland);
    let current = wrap.index(0, 0);
    let candidates: Vec<_> = [1, 7, 3, 5]
        .into_iter()
        .filter_map(|direction| {
            wrap.civ3_neighbor(0, 0, direction)
                .map(|(x, y)| wrap.index(x, y))
        })
        .collect();
    assert_eq!(candidates.len(), 2, "polar Y edges are bounded");

    // Pick a seed whose bounded draw selects the west-edge wrapped candidate,
    // exercising both topology rules in the route adapter.
    let seed = (0..10_000_u64)
        .find(|&seed| {
            let mut rng = MapRng::new(seed);
            let selected = candidates[rng.below(candidates.len() as i32) as usize];
            wrap.position(selected).0 == wrap.width() - 1
        })
        .expect("a seed selects the wrapped candidate");
    let mut expected_rng = MapRng::new(seed);
    let expected = candidates[expected_rng.below(candidates.len() as i32) as usize];
    let mut actual_rng = MapRng::new(seed);
    let selected = generator
        .select_freeciv_river_step(
            &tiles,
            &vec![500; wrap.tile_count()],
            &properties,
            &vec![false; wrap.tile_count()],
            current,
            &mut actual_rng,
        )
        .expect("at least one river course direction remains");

    assert_eq!(selected, expected);
    assert_eq!(wrap.position(selected).0, wrap.width() - 1);
    assert_eq!(wrap.position(selected).1, 1);
}

#[test]
fn freeciv_river_density_and_spring_height_follow_c2c3_parameters() {
    let settings = crate::rules::freeciv::RulesetData::campaign_default()
        .map_generation_settings()
        .expect("C2C3 map-generation settings parse");

    assert_eq!(
        rivers::freeciv_river_parameters(settings, 36 * 48),
        (6, 58, 784),
        "36x48 retains C2C3's current density and hmap lowland threshold"
    );
    assert_eq!(
        rivers::freeciv_river_parameters(settings, 100 * 100),
        (7, 394, 784),
        "larger-map polar scaling uses the default separate-poles branch"
    );
}

#[test]
fn freeciv_river_tests_use_c2c3_mountainous_and_wet_properties() {
    let wrap = MapWrap::X;
    let (generator, mut tiles, properties) = freeciv_river_test_map(wrap, Terrain::Grassland);
    let current = wrap.index(8, 10);
    let candidates: Vec<_> = [1, 7, 3, 5]
        .into_iter()
        .filter_map(|direction| {
            let (x, y) = wrap.position(current);
            wrap.civ3_neighbor(x, y, direction)
                .map(|(x, y)| wrap.index(x, y))
        })
        .collect();
    let heights = vec![500; wrap.tile_count()];
    let blocked = vec![false; wrap.tile_count()];

    // A low height cannot override river_test_highlands: Mountains has the
    // C2C3 mountainous weight 70, while Grassland has no mountainous weight.
    tiles[candidates[0]].terrain = Terrain::Mountains;
    let mut rng = MapRng::new(11);
    let selected = generator
        .select_freeciv_river_step(&tiles, &heights, &properties, &blocked, current, &mut rng)
        .expect("a non-mountainous continuation remains");
    assert_ne!(selected, candidates[0]);

    // With earlier tests tied, the C2C3 Swamp wet=100 value wins the
    // river_test_swamp priority over dry Grassland.
    tiles[candidates[0]].terrain = Terrain::Marsh;
    let mut rng = MapRng::new(12);
    let selected = generator
        .select_freeciv_river_step(&tiles, &heights, &properties, &blocked, current, &mut rng)
        .expect("the wet terrain continuation remains");
    assert_eq!(selected, candidates[0]);
}

#[test]
fn freeciv_spring_filters_apply_adjacent_and_relaxed_terrain_thresholds() {
    let wrap = MapWrap::X;
    let (generator, mut tiles, properties) = freeciv_river_test_map(wrap, Terrain::Grassland);
    let spring = wrap.index(8, 10);
    let neighbors: Vec<_> = [1, 7, 3, 5]
        .into_iter()
        .filter_map(|direction| {
            let (x, y) = wrap.position(spring);
            wrap.civ3_neighbor(x, y, direction)
                .map(|(x, y)| wrap.index(x, y))
        })
        .collect();
    let max_tries = 32_767;

    assert!(generator.freeciv_spring_is_acceptable(&tiles, spring, &properties, 0, max_tries,));

    // The source permits at most one combined cardinal-adjacent river or
    // ocean tile, and does not relax this filter after repeated attempts.
    tiles[neighbors[0]].river = true;
    tiles[neighbors[1]].terrain = Terrain::Coast;
    assert!(!generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries,
        max_tries,
    ));
    tiles[neighbors[0]].river = false;
    tiles[neighbors[1]].terrain = Terrain::Grassland;

    // Three of four adjacent mountainous tiles is 75%, below the source's
    // 90% cutoff; all four fails until half the maximum iterations.
    for &neighbor in &neighbors[..3] {
        tiles[neighbor].terrain = Terrain::Mountains;
    }
    assert!(generator.freeciv_spring_is_acceptable(&tiles, spring, &properties, 0, max_tries,));
    tiles[neighbors[3]].terrain = Terrain::Mountains;
    assert!(!generator.freeciv_spring_is_acceptable(&tiles, spring, &properties, 0, max_tries,));
    assert!(generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 5,
        max_tries,
    ));

    // Terrain-level mountainous/frozen/dry restrictions relax separately at
    // 60%, 80%, and 90%; the temperature-category prefilter remains separate.
    for tile in &mut tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
    }
    tiles[spring].terrain = Terrain::Mountains;
    assert!(!generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 5,
        max_tries,
    ));
    assert!(generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 6,
        max_tries,
    ));

    tiles[spring].terrain = Terrain::Glacier;
    assert!(!generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 7,
        max_tries,
    ));
    assert!(generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 8,
        max_tries,
    ));

    tiles[spring].terrain = Terrain::Desert;
    assert!(!generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 8,
        max_tries,
    ));
    assert!(generator.freeciv_spring_is_acceptable(
        &tiles,
        spring,
        &properties,
        max_tries / 10 * 9,
        max_tries,
    ));
}

#[test]
fn freeciv_river_blocking_is_fatal_and_endings_use_renderable_ocean_or_frozen_tiles() {
    let wrap = MapWrap::X;
    let (generator, mut tiles, properties) = freeciv_river_test_map(wrap, Terrain::Grassland);
    let current = wrap.index(8, 10);
    let heights = vec![500; wrap.tile_count()];

    assert_eq!(
        generator.select_freeciv_river_step(
            &tiles,
            &heights,
            &properties,
            &vec![true; wrap.tile_count()],
            current,
            &mut MapRng::new(5),
        ),
        None,
        "river_test_blocked is a fatal first test"
    );

    let spring = wrap.index(9, 10);
    let (x, y) = wrap.position(spring);
    let (ocean_x, ocean_y) = wrap.civ3_neighbor(x, y, 1).unwrap();
    tiles[wrap.index(ocean_x, ocean_y)].terrain = Terrain::Coast;
    assert_eq!(
        generator.trace_river(
            &tiles,
            &heights,
            spring,
            Some(&properties),
            &mut MapRng::new(6),
        ),
        Some(vec![spring]),
        "a course-neighbor ocean tile ends the river"
    );

    tiles[wrap.index(ocean_x, ocean_y)].terrain = Terrain::Grassland;
    let polar_glacier = wrap.index(9, 0);
    tiles[polar_glacier].terrain = Terrain::Glacier;
    assert_eq!(
        generator.trace_river(
            &tiles,
            &heights,
            polar_glacier,
            Some(&properties),
            &mut MapRng::new(7),
        ),
        Some(vec![polar_glacier]),
        "C2C3 Frozen property ends a river in the polar band"
    );
}

#[cfg(test)]
#[test]
fn opensimplex_civ3_generation_does_not_emit_legacy_glacier_or_lake_terrains() {
    for wrap in [MapWrap::X] {
        for seed in [42, 12345] {
            let map = generate_opensimplex(seed, wrap, Civ3WorldSettings::default());
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
            0xf810a553cb0e8e74,
            [(2, 10), (2, 28), (6, 39), (9, 20), (15, 31), (30, 27)],
        ),
        (
            42,
            0x76818b1c17b9cd71,
            [(1, 25), (8, 22), (15, 39), (24, 5), (25, 21), (31, 19)],
        ),
        (
            12345,
            0x199b4501fd3a6108,
            [(0, 32), (12, 28), (19, 33), (23, 24), (27, 39), (32, 26)],
        ),
        (
            700001,
            0x354acb089a791363,
            [(4, 5), (8, 27), (15, 23), (16, 9), (29, 15), (34, 8)],
        ),
        (
            2147483647,
            0x8d04724c3d9210f5,
            [(0, 13), (3, 28), (11, 21), (16, 13), (21, 24), (28, 16)],
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
fn opensimplex_civ3_ocean_coverage_choices_change_water_fraction_deterministically() {
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
            let first = generate_opensimplex(42, wrap, settings);
            let second = generate_opensimplex(42, wrap, settings);
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
fn polar_land_taper_is_smooth_seeded_and_dies_out_inland() {
    let wrap = MapWrap::X;
    let generator = Generator {
        wrap,
        settings: Civ3WorldSettings::default(),
        civilization_count: crate::world::DEFAULT_CIVILIZATION_COUNT,
    };
    let mut first = vec![255; wrap.tile_count()];
    let mut repeated = first.clone();
    let mut other_seed = first.clone();
    generator.taper_polar_landmass(&mut first, 42);
    generator.taper_polar_landmass(&mut repeated, 42);
    generator.taper_polar_landmass(&mut other_seed, 1337);

    assert_eq!(first, repeated);
    assert_ne!(first, other_seed);
    for x in 0..wrap.width() {
        assert!(first[wrap.index(x, 0)] < 30);
        assert_eq!(first[wrap.index(x, 20)], 255);
    }
    let inland_edge_row = (0..wrap.width())
        .map(|x| first[wrap.index(x, 4)])
        .collect::<std::collections::HashSet<_>>();
    assert!(
        inland_edge_row.len() > 1,
        "polar taper should vary by longitude"
    );
    let row_four = (0..wrap.width())
        .map(|x| first[wrap.index(x, 4)])
        .collect::<Vec<_>>();
    let largest_neighbor_change = (0..wrap.width())
        .map(|x| row_four[x as usize].abs_diff(row_four[((x + 1) % wrap.width()) as usize]) as i32)
        .max()
        .unwrap();
    assert!(
        largest_neighbor_change <= 40,
        "the seeded boundary should remain smooth at the wrap seam: {largest_neighbor_change}"
    );
}

#[test]
fn generated_polar_coastlines_vary_across_longitudes() {
    for seed in [42, 1337, 700001] {
        let map = generate_with_wrap(seed, MapWrap::X);
        let wrap = map.map_wrap;
        for distances in [
            (0..wrap.width())
                .map(|x| {
                    (1..wrap.height() / 2)
                        .find(|&y| !map.tiles[wrap.index(x, y) as usize].terrain.is_water())
                        .unwrap_or(wrap.height() / 2)
                })
                .collect::<Vec<_>>(),
            (0..wrap.width())
                .map(|x| {
                    (1..wrap.height() / 2)
                        .find(|&distance| {
                            !map.tiles[wrap.index(x, wrap.height() - 1 - distance) as usize]
                                .terrain
                                .is_water()
                        })
                        .unwrap_or(wrap.height() / 2)
                })
                .collect::<Vec<_>>(),
        ] {
            let shallowest = *distances.iter().min().unwrap();
            let deepest = *distances.iter().max().unwrap();
            assert!(
                deepest - shallowest >= 4,
                "seed {seed} should have a nonuniform polar coastline: {distances:?}"
            );
        }
    }
}

#[test]
fn civ3_climate_choices_shift_biomes_in_the_expected_direction() {
    let wrap = MapWrap::X;
    let map_for = |climate| {
        generate_opensimplex(
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
        generate_opensimplex(
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
fn opensimplex_civ3_landform_choices_are_deterministic_and_change_landmass_scale() {
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
            let first = generate_opensimplex(42, MapWrap::X, settings);
            let second = generate_opensimplex(42, MapWrap::X, settings);
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
    // The chosen seed does not guarantee a monotonic largest-continent area;
    // component count is the stable scale signal at this map size.
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
        generate_opensimplex(
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
    fn civ3_cylinder_keeps_polar_edges_ocean_and_has_valid_starts() {
        for seed in [0, 1, 42, 1337, 12345, 700001, 2147483647] {
            let a = generate_with_wrap(seed, MapWrap::X);
            let b = generate_with_wrap(seed, MapWrap::X);
            assert_eq!(a.map_wrap, MapWrap::X);
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
        let expected: Vec<(i32, i32)> = fields[5]
            .split(';')
            .map(|pair| {
                let (x, y) = pair.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        assert_eq!(
            checksum,
            u64::from_str_radix(fields[4], 16).unwrap(),
            "terrain/altitude/extras for {wrap} seed {seed}"
        );
        assert_eq!(
            starts, expected,
            "starting locations for {wrap} seed {seed}"
        );
    }
}
