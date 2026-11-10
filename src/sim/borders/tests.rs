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
