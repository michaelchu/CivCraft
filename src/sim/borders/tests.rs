use super::*;

#[test]
fn contested_border_claims_follow_civilization_culture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.city_borders_playtest(), (true, true, true));
}

#[test]
fn each_explicit_culture_threshold_claims_the_next_border_ring() {
    for (culture, expected_rank) in [(10, 2), (100, 3), (1_000, 4), (10_000, 5), (20_000, 6)] {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.border_city = None;
        }

        let origin = (10, 24);
        game.create_city(HUMAN, origin.0, origin.1);
        game.cities[0].add_culture(HUMAN, culture);
        for tile in &mut game.tiles {
            tile.known_by[HUMAN] = true;
        }

        let mut on_ring = origin;
        for _ in 0..expected_rank {
            on_ring = game
                .map_wrap
                .civ3_neighbor(on_ring.0, on_ring.1, 0)
                .expect("test ring stays inside the map");
        }
        let outside_ring = game
            .map_wrap
            .civ3_neighbor(on_ring.0, on_ring.1, 0)
            .expect("test outer ring stays inside the map");
        assert_eq!(
            game.map_wrap
                .civ3_distance(origin.0, origin.1, on_ring.0, on_ring.1),
            expected_rank
        );
        assert_eq!(
            game.map_wrap
                .civ3_distance(origin.0, origin.1, outside_ring.0, outside_ring.1),
            expected_rank + 1
        );

        game.rebuild_city_ownership();

        assert_eq!(game.tile(on_ring.0, on_ring.1).owner, Some(HUMAN));
        assert_eq!(game.tile(outside_ring.0, outside_ring.1).owner, None);
    }
}

#[test]
fn lower_rank_claim_beats_greater_civilization_culture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.border_city = None;
    }

    let mut positions = vec![(10, 24)];
    for _ in 0..4 {
        let previous = *positions.last().unwrap();
        positions.push(
            game.map_wrap
                .civ3_neighbor(previous.0, previous.1, 0)
                .expect("test claims stay inside the map"),
        );
    }
    game.create_city(HUMAN, positions[0].0, positions[0].1);
    game.create_city(1, positions[3].0, positions[3].1);
    game.cities[0].add_culture(HUMAN, 20_000);
    for tile in &mut game.tiles {
        tile.known_by[HUMAN] = true;
        tile.known_by[1] = true;
    }

    assert_eq!(
        game.map_wrap.civ3_distance(
            positions[0].0,
            positions[0].1,
            positions[4].0,
            positions[4].1
        ),
        4
    );
    assert_eq!(
        game.map_wrap.civ3_distance(
            positions[3].0,
            positions[3].1,
            positions[4].0,
            positions[4].1
        ),
        1
    );

    game.rebuild_city_ownership();

    assert_eq!(game.tile(positions[4].0, positions[4].1).owner, Some(1));
}
