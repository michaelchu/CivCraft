use super::*;
use crate::rules::engine_rules::{EffectSet, RulesetData, effect_value};

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeBorderVector {
    case: String,
    x: i32,
    y: i32,
    owner: usize,
    source_x: i32,
    source_y: i32,
    basis: String,
}

fn native_border_vectors() -> Vec<NativeBorderVector> {
    include_str!("../../../tests/fixtures/freeciv_border_vectors.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 7, "invalid native border vector: {line}");
            NativeBorderVector {
                case: fields[0].to_owned(),
                x: fields[1].parse().expect("vector x coordinate"),
                y: fields[2].parse().expect("vector y coordinate"),
                owner: fields[3].parse().expect("vector owner"),
                source_x: fields[4].parse().expect("source x coordinate"),
                source_y: fields[5].parse().expect("source y coordinate"),
                basis: fields[6].to_owned(),
            }
        })
        .collect()
}

fn flat_game(terrain: Terrain) -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = terrain;
        tile.owner = None;
        tile.border_city = None;
        tile.known_by.fill(false);
    }
    game
}

fn reveal_all(game: &mut Simulation) {
    for tile in &mut game.tiles {
        tile.known_by.fill(true);
    }
}

fn straight_path(
    game: &Simulation,
    start: (i32, i32),
    direction: usize,
    steps: usize,
) -> Vec<(i32, i32)> {
    let mut path = vec![start];
    while path.len() <= steps {
        let previous = *path.last().unwrap();
        path.push(
            game.map_wrap
                .civ3_neighbor(previous.0, previous.1, direction)
                .expect("test path stays inside the map"),
        );
    }
    path
}

fn selected_effect_active(
    game: &Simulation,
    regions: &BorderRegions,
    owner: usize,
    target: (i32, i32),
    source: (i32, i32),
    effect_name: &str,
) -> bool {
    let rules = RulesetData::campaign_default();
    let effect = rules
        .effects
        .iter()
        .find(|effect| effect.rule_name.eq_ignore_ascii_case(effect_name))
        .unwrap_or_else(|| panic!("missing selected C2C3 effect {effect_name}"));
    let ordinary = SimulationRequirementContext::for_player_at(game, owner, Some(target));
    let context = BorderTileClaimContext {
        simulation: game,
        ordinary,
        regions,
        target,
        source,
    };
    effect_value(
        &EffectSet::from(vec![effect.clone()]),
        EffectKind::TileClaimable,
        &context,
    )
    .unwrap_or_else(|error| panic!("{effect_name} evaluation failed: {error:?}"))
        > 0
}

fn owned_tile_count(game: &Simulation) -> usize {
    game.tiles
        .iter()
        .filter(|tile| tile.owner == Some(HUMAN))
        .count()
}

#[test]
fn culture_tiers_grow_the_border_from_a_square_into_a_fat_x() {
    let mut game = flat_game(Terrain::Grassland);
    let origin = (20, 24);
    game.create_city(HUMAN, origin.0, origin.1);
    reveal_all(&mut game);

    // Thresholds 0/10/100/1,000/10,000 give radius 1..=5: the 3×3 square,
    // then the 21-tile fat X, then each side one tile further out.
    for (history, tiles) in [(0, 9), (10, 21), (100, 45), (1_000, 77), (10_000, 117)] {
        game.cities[0].history = history;
        game.rebuild_city_ownership();
        assert_eq!(owned_tile_count(&game), tiles, "culture {history}");
    }
}

#[test]
fn city_size_does_not_expand_the_border() {
    let mut game = flat_game(Terrain::Grassland);
    game.create_city(HUMAN, 20, 24);
    reveal_all(&mut game);
    game.rebuild_city_ownership();
    let square = owned_tile_count(&game);
    assert_eq!(square, 9);

    game.cities[0].size = 12;
    game.rebuild_city_ownership();
    assert_eq!(owned_tile_count(&game), square);
}

#[test]
fn fat_x_extends_one_tile_along_each_diagonal_edge_not_the_screen_axes() {
    let mut game = flat_game(Terrain::Grassland);
    let origin = (20, 24);
    game.create_city(HUMAN, origin.0, origin.1);
    reveal_all(&mut game);
    game.cities[0].history = 10; // Radius 2: the 21-tile fat X.
    game.rebuild_city_ownership();

    // Two steps along a screen axis stay inside the square's reach only for
    // one tile; the X's arms run along the diagonals (3,1) and (2,2).
    let east = straight_path(&game, origin, 2, 2);
    assert_eq!(game.tile(east[1].0, east[1].1).owner, Some(HUMAN));
    assert_eq!(game.tile(east[2].0, east[2].1).owner, None);
    let diagonal = straight_path(&game, origin, 3, 2);
    assert_eq!(game.tile(diagonal[2].0, diagonal[2].1).owner, Some(HUMAN));
}

#[test]
fn enabled_borders_only_claim_known_tiles_beyond_the_city_source() {
    let mut game = flat_game(Terrain::Grassland);
    let origin = (20, 20);
    let neighbor = game.map_wrap.civ3_neighbor(origin.0, origin.1, 2).unwrap();
    game.create_city(HUMAN, origin.0, origin.1);
    for tile in &mut game.tiles {
        tile.known_by.fill(false);
    }
    game.rebuild_city_ownership();
    assert_eq!(game.tile(origin.0, origin.1).owner, Some(HUMAN));
    assert_eq!(game.tile(neighbor.0, neighbor.1).owner, None);

    game.tile_mut(neighbor.0, neighbor.1)
        .known_by
        .set(HUMAN, true);
    game.rebuild_city_ownership();
    assert_eq!(game.tile(neighbor.0, neighbor.1).owner, Some(HUMAN));
}

#[test]
fn stronger_claims_can_expand_beyond_permanent_tiles_and_ties_keep_map_order() {
    let mut game = flat_game(Terrain::Grassland);
    let route = straight_path(&game, (20, 20), 2, 4);
    game.create_city(HUMAN, route[0].0, route[0].1);
    game.create_city(1, route[4].0, route[4].1);
    reveal_all(&mut game);
    game.cities[0].size = 1;
    game.cities[1].size = 5;
    for city in &mut game.cities {
        city.history = 100; // Radius 3 reaches the tile midway between them.
    }
    let old_city = game.cities[0].id;
    game.tile_mut(route[2].0, route[2].1).border_city = Some(old_city);
    game.tile_mut(route[2].0, route[2].1).owner = Some(HUMAN);
    game.rebuild_city_ownership();
    assert_eq!(game.tile(route[2].0, route[2].1).owner, Some(1));

    let mut tied = flat_game(Terrain::Grassland);
    let route = straight_path(&tied, (20, 20), 2, 4);
    tied.create_city(HUMAN, route[0].0, route[0].1);
    tied.create_city(1, route[4].0, route[4].1);
    reveal_all(&mut tied);
    for city in &mut tied.cities {
        city.history = 100;
    }
    for tile in &mut tied.tiles {
        tile.owner = None;
        tile.border_city = None;
    }
    tied.rebuild_city_ownership();
    assert_eq!(tied.tile(route[2].0, route[2].1).owner, Some(HUMAN));

    let mut permanent = flat_game(Terrain::Grassland);
    let route = straight_path(&permanent, (20, 20), 2, 2);
    permanent.create_city(HUMAN, route[0].0, route[0].1);
    permanent.create_city(1, route[2].0, route[2].1);
    reveal_all(&mut permanent);
    permanent.cities[0].size = 1;
    permanent.cities[1].size = 12;
    let old_city = permanent.cities[0].id;
    permanent.tile_mut(route[1].0, route[1].1).border_city = Some(old_city);
    permanent.tile_mut(route[1].0, route[1].1).owner = Some(HUMAN);
    assert_eq!(BorderRules::selected().permanent_radius_sq, -3);
    permanent.rebuild_city_ownership();
    assert_eq!(permanent.tile(route[1].0, route[1].1).owner, Some(HUMAN));
}

#[test]
fn tile_claimable_matches_selected_region_and_distance_requirements() {
    let source = (20, 20);
    let path = {
        let game = flat_game(Terrain::Grassland);
        straight_path(&game, source, 2, 3)
    };

    let land = flat_game(Terrain::Grassland);
    let regions = BorderRegions::from_simulation(&land);
    assert!(selected_effect_active(
        &land,
        &regions,
        HUMAN,
        path[2],
        source,
        "claim_land"
    ));

    let mut disconnected_land = flat_game(Terrain::Ocean);
    disconnected_land.tile_mut(source.0, source.1).terrain = Terrain::Grassland;
    disconnected_land.tile_mut(path[2].0, path[2].1).terrain = Terrain::Grassland;
    let regions = BorderRegions::from_simulation(&disconnected_land);
    assert!(!selected_effect_active(
        &disconnected_land,
        &regions,
        HUMAN,
        path[2],
        source,
        "claim_land"
    ));

    let mut ocean = flat_game(Terrain::Ocean);
    ocean.tile_mut(source.0, source.1).terrain = Terrain::Grassland;
    let regions = BorderRegions::from_simulation(&ocean);
    assert!(selected_effect_active(
        &ocean,
        &regions,
        HUMAN,
        path[1],
        source,
        "claim_ocean_adj"
    ));
    assert!(!selected_effect_active(
        &ocean,
        &regions,
        HUMAN,
        path[2],
        source,
        "claim_ocean_adj"
    ));

    let mut lake = flat_game(Terrain::Grassland);
    lake.tile_mut(path[2].0, path[2].1).terrain = Terrain::Ocean;
    let regions = BorderRegions::from_simulation(&lake);
    assert!(selected_effect_active(
        &lake,
        &regions,
        HUMAN,
        path[2],
        source,
        "claim_ocean_lake"
    ));

    let mut large_lake = flat_game(Terrain::Grassland);
    let water_path = straight_path(&large_lake, path[2], 2, 20);
    for &(x, y) in &water_path[1..] {
        large_lake.tile_mut(x, y).terrain = Terrain::Ocean;
    }
    large_lake.tile_mut(path[2].0, path[2].1).terrain = Terrain::Ocean;
    let regions = BorderRegions::from_simulation(&large_lake);
    assert!(!selected_effect_active(
        &large_lake,
        &regions,
        HUMAN,
        path[2],
        source,
        "claim_ocean_lake"
    ));

    let mut bay = flat_game(Terrain::Grassland);
    let channel = straight_path(&bay, path[2], 2, 22);
    for &(x, y) in &channel[1..] {
        bay.tile_mut(x, y).terrain = Terrain::Ocean;
    }
    bay.tile_mut(path[2].0, path[2].1).terrain = Terrain::Ocean;
    let regions = BorderRegions::from_simulation(&bay);
    assert!(selected_effect_active(
        &bay,
        &regions,
        HUMAN,
        path[2],
        source,
        "claim_ocean_bay"
    ));
}

// The Freeciv native radius vectors (`radius_sq_city` + size) are not ported:
// border extent comes only from culture, so the shape is covered by
// `culture_tiers_grow_the_border_from_a_square_into_a_fat_x`.

#[test]
fn native_c2c3_border_vectors_match_strength_tie_and_permanent_claims() {
    let vectors = native_border_vectors();
    let vector = |case: &str| {
        vectors
            .iter()
            .find(|vector| vector.case == case)
            .unwrap_or_else(|| panic!("missing native {case} border vector"))
            .clone()
    };
    let assert_source = |game: &Simulation, vector: &NativeBorderVector| {
        let tile = game.tile(vector.x, vector.y);
        assert_eq!(
            tile.owner,
            Some(vector.owner),
            "native {} owner",
            vector.case
        );
        let city = game
            .cities
            .iter()
            .find(|city| city.x == vector.source_x && city.y == vector.source_y)
            .unwrap_or_else(|| panic!("missing native {} source city", vector.case));
        assert_eq!(
            tile.border_city,
            Some(city.id),
            "native {} claimer",
            vector.case
        );
    };

    let mut stronger = flat_game(Terrain::Grassland);
    reveal_all(&mut stronger);
    stronger.create_city(HUMAN, 18, 8);
    stronger.create_city(1, 20, 12);
    stronger.cities[1].size = 5;
    for city in &mut stronger.cities {
        city.history = 1_000; // Radius 4 reaches the contested tile.
    }
    stronger.rebuild_city_ownership();
    assert_source(&stronger, &vector("strong"));

    let mut tied = flat_game(Terrain::Grassland);
    reveal_all(&mut tied);
    tied.create_city(HUMAN, 18, 22);
    tied.create_city(1, 20, 26);
    for city in &mut tied.cities {
        city.history = 1_000; // Radius 4 reaches the contested tile.
    }
    tied.rebuild_city_ownership();
    assert_source(&tied, &vector("tie"));

    let mut permanent = flat_game(Terrain::Grassland);
    reveal_all(&mut permanent);
    permanent.create_city(HUMAN, 24, 32);
    permanent.create_city(1, 22, 36);
    permanent.cities[1].size = 12;
    for city in &mut permanent.cities {
        city.history = 1_000; // Radius 4 reaches the contested tile.
    }
    permanent.rebuild_city_ownership();
    assert_source(&permanent, &vector("permanent"));
}

#[test]
fn cultural_claims_discover_owned_tiles_without_granting_current_sight() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    sim.cities.clear();
    sim.observed_map = Default::default();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by.fill(false);
        tile.seen = false;
        tile.visible = false;
        tile.owner = None;
        tile.border_city = None;
    }
    sim.create_city(HUMAN, 20, 24);
    sim.cities.last_mut().unwrap().history = 100;
    sim.rebuild_city_ownership();
    sim.refresh_visibility();
    // Radius 3 along east, east, south-east (outside the 21-tile fat X's
    // reach of 2 and the city's sight radius).
    let mut position = (20, 24);
    for direction in [2, 2, 3] {
        position = sim
            .map_wrap
            .civ3_neighbor(position.0, position.1, direction)
            .unwrap();
    }
    let (x, y) = position;
    assert_eq!(sim.tile(x, y).owner, Some(HUMAN));
    let rendered = sim.render_tile_view(x, y);
    assert!(rendered.seen);
    assert!(!rendered.visible);
    assert_eq!(
        sim.human_known_tile(x, y).unwrap().terrain,
        Terrain::Grassland
    );
    assert_eq!(sim.tile(x, y).last_seen_owner, Some(HUMAN));
    assert!(!sim.render_tile_view(30, 24).seen);
}
