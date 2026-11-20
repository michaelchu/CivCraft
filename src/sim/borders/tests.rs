use super::*;

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
fn borders_claim_every_tile_in_the_shape_seen_or_not() {
    let mut game = flat_game(Terrain::Grassland);
    let origin = (20, 24);
    game.create_city(HUMAN, origin.0, origin.1);
    game.cities[0].history = 10; // Radius 2: the 21-tile fat X.
    for tile in &mut game.tiles {
        tile.known_by.fill(false);
    }
    game.rebuild_city_ownership();
    assert_eq!(owned_tile_count(&game), 21);
    // The claim reveals the terrain it covers to its owner.
    let neighbor = game.map_wrap.civ3_neighbor(origin.0, origin.1, 2).unwrap();
    assert_eq!(game.tile(neighbor.0, neighbor.1).owner, Some(HUMAN));
    assert!(game.tile(neighbor.0, neighbor.1).known_by[HUMAN]);
}

#[test]
fn borders_claim_water_as_well_as_land() {
    let mut game = flat_game(Terrain::Ocean);
    let origin = (20, 24);
    game.tile_mut(origin.0, origin.1).terrain = Terrain::Grassland;
    game.create_city(HUMAN, origin.0, origin.1);
    game.cities[0].history = 100; // Radius 3: 45 tiles, all of them ocean but one.
    reveal_all(&mut game);
    game.rebuild_city_ownership();
    assert_eq!(owned_tile_count(&game), 45);
    let far = straight_path(&game, origin, 3, 3)[3];
    assert_eq!(game.tile(far.0, far.1).terrain, Terrain::Ocean);
    assert_eq!(game.tile(far.0, far.1).owner, Some(HUMAN));
}

/// Two cities `gap` steps apart along a map axis; returns their ids and the
/// straight path between them. Both are given enough culture to reach it.
fn contest(game: &mut Simulation, gap: usize, history: [i32; 2]) -> ([u32; 2], Vec<(i32, i32)>) {
    let route = straight_path(game, (20, 20), 3, gap);
    game.create_city(HUMAN, route[0].0, route[0].1);
    game.create_city(1, route[gap].0, route[gap].1);
    reveal_all(game);
    for (city, culture) in game.cities.iter_mut().zip(history) {
        city.history = culture;
    }
    game.rebuild_city_ownership();
    ([game.cities[0].id, game.cities[1].id], route)
}

#[test]
fn nearest_city_owns_a_contested_tile_whatever_its_size() {
    let mut game = flat_game(Terrain::Grassland);
    let (ids, route) = contest(&mut game, 5, [1_000, 1_000]);
    game.cities[0].size = 1;
    game.cities[1].size = 12;
    game.rebuild_city_ownership();
    // Two steps from the small city, three from the big one.
    assert_eq!(game.tile(route[2].0, route[2].1).border_city, Some(ids[0]));
    assert_eq!(game.tile(route[3].0, route[3].1).border_city, Some(ids[1]));
}

#[test]
fn nearest_city_wins_even_against_much_more_culture() {
    let mut game = flat_game(Terrain::Grassland);
    let (ids, route) = contest(&mut game, 5, [1_000, 1_000]);
    game.cities[1].history = 1_000_000;
    game.rebuild_city_ownership();
    assert_eq!(game.tile(route[2].0, route[2].1).border_city, Some(ids[0]));
}

#[test]
fn equidistant_tile_goes_to_higher_culture_then_older_city() {
    let mut game = flat_game(Terrain::Grassland);
    let (ids, route) = contest(&mut game, 4, [100, 100]);
    let middle = route[2];
    // Equal culture: the older (first founded) city keeps it.
    assert_eq!(game.tile(middle.0, middle.1).border_city, Some(ids[0]));
    game.cities[1].history = 101;
    game.rebuild_city_ownership();
    assert_eq!(game.tile(middle.0, middle.1).border_city, Some(ids[1]));
    game.cities[1].history = 100;
    game.rebuild_city_ownership();
    assert_eq!(game.tile(middle.0, middle.1).border_city, Some(ids[0]));
}

#[test]
fn shared_border_is_the_line_of_equal_distance_and_ignores_history() {
    let mut game = flat_game(Terrain::Grassland);
    let (ids, _) = contest(&mut game, 4, [1_000, 1_000]);
    game.cities[0].size = 2;
    game.cities[1].size = 9;
    game.rebuild_city_ownership();
    let centers = [
        (game.cities[0].x, game.cities[0].y),
        (game.cities[1].x, game.cities[1].y),
    ];
    let first_pass: Vec<_> = game.tiles.iter().map(|tile| tile.border_city).collect();
    let mut contested = 0;
    for (index, tile) in game.tiles.iter().enumerate() {
        let Some(owner) = tile.border_city else {
            continue;
        };
        let (x, y) = game.map_wrap.position(index);
        let ranks = centers.map(|(cx, cy)| game.map_wrap.civ3_culture_rank(cx, cy, x, y));
        let nearest = if ranks[0] <= ranks[1] { ids[0] } else { ids[1] };
        if ranks[0] != ranks[1] {
            // Never taken from the nearer city, never a size effect.
            assert_eq!(owner, nearest, "tile ({x},{y}) ranks {ranks:?}");
            contested += 1;
        }
    }
    assert!(contested > 0);

    // Ownership is a pure function of the cities: no dependence on earlier
    // claims or on the order the cities are scanned.
    for tile in &mut game.tiles {
        tile.border_city = None;
        tile.owner = None;
    }
    game.cities.reverse();
    game.rebuild_city_ownership();
    let second_pass: Vec<_> = game.tiles.iter().map(|tile| tile.border_city).collect();
    assert_eq!(first_pass, second_pass);
}

// The Freeciv native radius vectors (`radius_sq_city` + size) are not ported:
// border extent comes only from culture, so the shape is covered by
// `culture_tiers_grow_the_border_from_a_square_into_a_fat_x`.

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
