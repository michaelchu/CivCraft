use super::*;

#[test]
fn filtered_city_rendering_and_markers_preserve_live_and_remembered_state() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ui_review_observed_map();
    for reobserved in [false, true] {
        if reobserved {
            game.ui_review_reobserve_map();
        }
        let all = game.render_cities();
        let filtered = game.render_cities_where(|c| (c.x, c.y) == (20, 24));
        let expected: Vec<_> = all
            .iter()
            .filter(|c| (c.city.x, c.city.y) == (20, 24))
            .cloned()
            .collect();
        assert_eq!(
            bincode::serialize(&filtered).unwrap(),
            bincode::serialize(&expected).unwrap()
        );
        assert!(game.render_cities_where(|_| false).is_empty());
        let markers: Vec<_> = game
            .render_city_markers()
            .into_iter()
            .map(|c| (c.id, c.owner, c.x, c.y))
            .collect();
        let expected: Vec<_> = all
            .iter()
            .map(|c| (c.city.id, c.city.owner, c.city.x, c.city.y))
            .collect();
        assert_eq!(markers, expected);
    }
}

#[test]
fn city_appearance_disorder_matches_full_forecast_after_policy_changes() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ui_review_observed_map();
    game.ui_review_reobserve_map();
    let guard = game.spawn_unit(1, UnitKind::Warrior, 20, 24);
    let city_id = game.cities[0].id;
    game.units
        .iter_mut()
        .find(|u| u.id == guard)
        .unwrap()
        .home_city = Some(city_id);
    for government in [
        Government::Anarchy,
        Government::Monarchy,
        Government::Republic,
        Government::Democracy,
    ] {
        for player in &mut game.players {
            player.government = government;
        }
        for rates in [[100, 0, 0], [0, 0, 100], [40, 30, 30]] {
            for player in &mut game.players {
                player.rates = rates;
            }
            for city in &game.cities {
                assert_eq!(
                    game.observed_city(city).disorder,
                    game.city_forecast(city).disorder
                );
            }
        }
    }
}

#[test]
fn fog_preserves_terrain_extras_and_city_graphics_until_reobserved() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ui_review_observed_map();
    assert!(game.ui_review_observed_map_valid(false));
    let remembered = game
        .render_cities()
        .into_iter()
        .find(|c| c.city.x == 20 && c.city.y == 24)
        .unwrap();
    assert!(remembered.art_cell.1 >= 5, "remembered city retains walls");
    assert_eq!(
        remembered.art_cell.0, 2,
        "remembered Greek city retains classical artwork"
    );
    assert_eq!(game.render_tile(19, 23).terrain, Terrain::Forest);
    assert!(!game.render_tile(20, 24).visible);
    let observed_random = game.random_snapshot();
    game.ui_review_reobserve_map();
    assert!(game.ui_review_observed_map_valid(true));
    let current = game
        .render_cities()
        .into_iter()
        .find(|c| c.city.x == 20 && c.city.y == 24)
        .unwrap();
    assert!(current.art_cell.1 < 5);
    assert_eq!(current.art_cell.0, 8);
    assert_eq!(
        observed_random,
        game.random_snapshot(),
        "observation consumes no random draws"
    );
}

#[test]
fn destroyed_or_captured_cities_remain_at_last_observed_state() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ui_review_observed_map();
    let id = game.cities[0].id;
    game.cities[0].owner = 2;
    game.refresh_visibility();
    assert_eq!(
        game.render_cities()
            .iter()
            .find(|c| c.city.id == id)
            .unwrap()
            .city
            .owner,
        1
    );
    game.cities.clear();
    game.refresh_visibility();
    assert!(game.render_cities().iter().any(|c| c.city.id == id));
    game.ui_review_reobserve_map();
    assert!(!game.render_cities().iter().any(|c| c.city.id == id));
}

#[test]
fn sea_map_copies_the_sellers_visual_snapshot_not_live_world() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.map_snapshot_playtest());
    assert_eq!(game.render_tile(23, 36).terrain, Terrain::Ocean);
    assert_eq!(game.tile(23, 36).terrain, Terrain::Grassland);
}

#[test]
fn improvements_and_occupancy_are_frozen_outside_sight() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ui_review_observed_map();
    game.ui_review_reobserve_map();
    let guard = game.spawn_unit(1, UnitKind::Warrior, 20, 24);
    game.refresh_visibility();
    if let Some(unit) = game.units.iter_mut().find(|u| u.owner == HUMAN) {
        unit.x = 4;
        unit.y = 48;
    }
    game.refresh_visibility();
    game.units.retain(|u| u.id != guard);
    let tile = game.tile_mut(20, 24);
    tile.pollution = true;
    tile.fallout = true;
    tile.hut = true;
    tile.irrigation = false;
    tile.mine = true;
    tile.resource = Some(Resource::Oasis);
    game.refresh_visibility();
    let tile = game.render_tile(20, 24);
    assert!(!tile.pollution && !tile.fallout && !tile.hut && !tile.mine);
    assert!(tile.irrigation);
    assert!(tile.resource.is_none());
    assert!(
        game.render_cities()
            .iter()
            .find(|c| c.city.x == 20 && c.city.y == 24)
            .unwrap()
            .occupied
    );
    game.ui_review_reobserve_map();
    assert!(game.render_tile(20, 24).pollution);
    assert!(
        !game
            .render_cities()
            .iter()
            .find(|c| c.city.x == 20 && c.city.y == 24)
            .unwrap()
            .occupied
    );
}

#[test]
fn an_older_trade_map_does_not_replace_a_newer_observation() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.observed_map = Default::default();
    for tile in &mut game.tiles {
        tile.seen = false;
        tile.known_by.fill(false);
    }
    game.tile_mut(23, 36).terrain = Terrain::Ocean;
    game.spawn_unit(1, UnitKind::Trireme, 23, 36);
    game.refresh_visibility();
    game.units.clear();
    game.tile_mut(23, 36).terrain = Terrain::Grassland;
    game.spawn_unit(HUMAN, UnitKind::Explorer, 23, 36);
    game.refresh_visibility();
    game.units.clear();
    game.refresh_visibility();
    game.transfer_treaty_map(1, HUMAN, TreatyMap::World);
    assert_eq!(game.render_tile(23, 36).terrain, Terrain::Grassland);
}

#[test]
fn newer_trade_maps_update_existing_visual_and_movement_knowledge_together() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.observed_map = Default::default();
    for tile in &mut game.tiles {
        tile.seen = false;
        tile.known_by.fill(false);
    }
    let (x, y) = (23, 36);
    let index = Simulation::idx(x, y);
    let tile = game.tile_mut(x, y);
    tile.terrain = Terrain::Grassland;
    tile.road = false;
    tile.railroad = false;
    tile.owner = None;
    // Both receivers already know an older, unroaded land tile.
    game.spawn_unit(HUMAN, UnitKind::Explorer, x, y);
    game.spawn_unit(2, UnitKind::Explorer, x, y);
    game.refresh_visibility();
    game.units.clear();
    let tile = game.tile_mut(x, y);
    tile.terrain = Terrain::Hills;
    tile.road = true;
    tile.railroad = true;
    tile.owner = Some(1);
    game.spawn_unit(1, UnitKind::Explorer, x, y);
    game.refresh_visibility();
    game.units.clear();
    // The world changes again after the seller leaves. Neither recipient
    // may receive this unobserved ocean or lose the recorded roads.
    let tile = game.tile_mut(x, y);
    tile.terrain = Terrain::Ocean;
    tile.road = false;
    tile.railroad = false;
    tile.owner = None;
    game.refresh_visibility();
    assert!(game.tile(x, y).seen && !game.tile(x, y).visible);
    assert!(!game.observed_route_tile(x, y).unwrap().road);
    let random = game.random_snapshot();
    game.transfer_treaty_map(1, HUMAN, TreatyMap::World);
    game.transfer_treaty_map(1, 2, TreatyMap::World);
    let rendered = game.render_tile(x, y);
    assert_eq!(rendered.terrain, Terrain::Hills);
    assert!(rendered.road && rendered.railroad);
    assert_eq!(game.displayed_tile_owner(x, y), Some(1));
    let route = game.observed_route_tile(x, y).unwrap();
    assert_eq!(route.terrain, Terrain::Hills);
    assert!(route.road && route.railroad && !route.is_water());
    let ai = game.ai_map_memory.get(&(2, index)).unwrap();
    assert_eq!(ai.route.terrain, Terrain::Hills);
    assert!(ai.route.road && ai.route.railroad && !ai.ocean);
    assert_eq!(ai.owner, Some(1));
    assert_eq!(random, game.random_snapshot());
}

#[test]
fn observed_map_round_trips_and_rejects_corrupt_owners() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.ui_review_observed_map();
    let path =
        std::env::temp_dir().join(format!("civcraft-observed-map-{}.save", std::process::id()));
    game.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    assert!(loaded.ui_review_observed_map_valid(false));
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    loaded
        .observed_map
        .tiles
        .insert((usize::MAX, 0), (0, loaded.tiles[0].clone()));
    assert!(!loaded.valid_render_memory());
    std::fs::remove_file(path).unwrap();
}
