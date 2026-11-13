use super::*;
use std::collections::HashSet;

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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    effect_value(std::slice::from_ref(effect), "Tile_Claimable", &context)
        .unwrap_or_else(|error| panic!("{effect_name} evaluation failed: {error:?}"))
        > 0
}

#[test]
fn selected_c2c3_radius_expands_with_city_size() {
    let mut game = flat_game(Terrain::Grassland);
    let origin = (20, 20);
    let path = straight_path(&game, origin, 2, 4);
    game.create_city(HUMAN, origin.0, origin.1);
    reveal_all(&mut game);

    let rules = BorderRules::selected();
    assert_eq!(rules.city_radius_sq, 5);
    assert_eq!(rules.city_size_effect, 1);
    assert_eq!(rules.city_border_radius_sq(&game.cities[0]), 6);
    game.rebuild_city_ownership();
    assert_eq!(game.tile(path[2].0, path[2].1).owner, Some(HUMAN));
    assert_eq!(game.tile(path[3].0, path[3].1).owner, None);

    game.cities[0].size = 4;
    assert_eq!(rules.city_border_radius_sq(&game.cities[0]), 9);
    game.rebuild_city_ownership();
    assert_eq!(game.tile(path[3].0, path[3].1).owner, Some(HUMAN));
    assert_eq!(game.tile(path[4].0, path[4].1).owner, None);
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

    game.tile_mut(neighbor.0, neighbor.1).known_by[HUMAN] = true;
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

#[test]
fn native_c2c3_border_vectors_match_with_explicit_civ3_topology_expansion() {
    let vectors = native_border_vectors();
    let native_radius: Vec<_> = vectors
        .iter()
        .filter(|vector| vector.case == "radius" && vector.basis == "native")
        .collect();
    let topology_expansion: Vec<_> = vectors
        .iter()
        .filter(|vector| vector.case == "radius" && vector.basis == "civ3_topology")
        .collect();
    assert_eq!(native_radius.len(), 19);
    assert_eq!(topology_expansion.len(), 6);
    assert!(
        native_radius
            .iter()
            .chain(&topology_expansion)
            .all(|vector| (vector.source_x, vector.source_y) == (4, 6))
    );

    let mut game = flat_game(Terrain::Grassland);
    reveal_all(&mut game);
    game.create_city(HUMAN, 4, 6);
    let city_id = game.cities[0].id;
    game.rebuild_city_ownership();

    let expected: HashSet<_> = native_radius
        .iter()
        .chain(&topology_expansion)
        .map(|vector| (vector.x, vector.y))
        .collect();
    let actual: HashSet<_> = game
        .tiles
        .iter()
        .enumerate()
        .filter(|(_, tile)| tile.border_city == Some(city_id))
        .map(|(index, _)| game.map_wrap.position(index))
        .collect();
    assert_eq!(actual, expected);

    for vector in native_radius {
        assert_eq!(
            game.tile(vector.x, vector.y).owner,
            Some(vector.owner),
            "Freeciv native radius claim at ({},{})",
            vector.x,
            vector.y
        );
        assert_eq!(
            game.tile(vector.x, vector.y).border_city,
            Some(city_id),
            "Freeciv source at ({},{})",
            vector.x,
            vector.y
        );
    }
    for vector in topology_expansion {
        assert_eq!(game.tile(vector.x, vector.y).owner, Some(vector.owner));
        assert_eq!(game.tile(vector.x, vector.y).border_city, Some(city_id));
    }
}

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
    stronger.rebuild_city_ownership();
    assert_source(&stronger, &vector("strong"));

    let mut tied = flat_game(Terrain::Grassland);
    reveal_all(&mut tied);
    tied.create_city(HUMAN, 18, 22);
    tied.create_city(1, 20, 26);
    tied.rebuild_city_ownership();
    assert_source(&tied, &vector("tie"));

    let mut permanent = flat_game(Terrain::Grassland);
    reveal_all(&mut permanent);
    permanent.create_city(HUMAN, 24, 32);
    permanent.create_city(1, 22, 36);
    permanent.cities[1].size = 12;
    permanent.rebuild_city_ownership();
    assert_source(&permanent, &vector("permanent"));
}
