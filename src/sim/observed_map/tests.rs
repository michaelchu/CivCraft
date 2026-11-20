//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn apollo_program_reveals_the_same_ocean_to_rendering_and_routes() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    let origin = (10, 24);
    let destination = offset_native_by_map(origin.0, origin.1, 8, 0);
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.hut = false;
    }
    sim.tile_mut(destination.0, destination.1).terrain = Terrain::Ocean;
    sim.create_city(HUMAN, origin.0, origin.1);
    sim.cities
        .last_mut()
        .unwrap()
        .buildings
        .push(Building::APOLLO_PROGRAM);
    let unit = sim.spawn_unit(HUMAN, UnitKind::Warrior, origin.0, origin.1);
    sim.refresh_visibility();

    let tile = sim.render_tile_view(destination.0, destination.1);
    assert!(tile.seen && !tile.visible);
    assert_eq!(tile.terrain, Terrain::Ocean);
    assert_eq!(
        sim.observed_route_tile(destination.0, destination.1)
            .unwrap()
            .terrain,
        tile.terrain
    );
    assert!(
        sim.preview_goto(unit, destination.0, destination.1)
            .is_none()
    );
}
#[test]
fn sea_map_uses_live_ocean_for_eligibility_and_the_sellers_last_snapshot_for_contents() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    sim.observed_map = Default::default();
    for tile in &mut sim.tiles {
        tile.known_by.fill(false);
        tile.seen = false;
    }
    let (x, y) = (40, 32);
    sim.tile_mut(10, 24).terrain = Terrain::Grassland;
    sim.tile_mut(x, y).terrain = Terrain::Grassland;
    sim.create_city(1, 10, 24);
    sim.cities
        .last_mut()
        .unwrap()
        .buildings
        .push(Building::APOLLO_PROGRAM);
    sim.refresh_visibility();
    assert!(sim.human_known_tile(x, y).is_none());
    assert_eq!(
        sim.remembered_tile(1, x, y).unwrap().terrain,
        Terrain::Grassland
    );

    sim.transfer_treaty_map(1, HUMAN, TreatyMap::Sea);
    assert!(sim.human_known_tile(x, y).is_none());

    sim.tile_mut(x, y).terrain = Terrain::Ocean;
    sim.transfer_treaty_map(1, HUMAN, TreatyMap::Sea);
    assert_eq!(
        sim.human_known_tile(x, y).unwrap().terrain,
        Terrain::Grassland
    );
    sim.refresh_visibility();
    assert_eq!(
        sim.observed_route_tile(x, y).unwrap().terrain,
        Terrain::Grassland
    );
}

use crate::rules::engine_rules::RulesetData;

#[test]
fn filtered_city_rendering_and_markers_preserve_live_and_remembered_state() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::sim::ui_review::ui_review_observed_map(&mut game);
    for reobserved in [false, true] {
        if reobserved {
            crate::sim::ui_review::ui_review_reobserve_map(&mut game);
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::sim::ui_review::ui_review_observed_map(&mut game);
    crate::sim::ui_review::ui_review_reobserve_map(&mut game);
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
fn fog_preserves_terrain_extras_and_civ3_city_appearance_until_reobserved() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::sim::ui_review::ui_review_observed_map(&mut game);
    assert!(crate::sim::ui_review::ui_review_observed_map_valid(
        &game, false
    ));
    let owner = game.city_at(20, 24).unwrap().owner;
    let expected_style = game.players[owner]
        .civilization_id
        .as_deref()
        .and_then(crate::rules::civilizations::by_id)
        .map_or(2, |civilization| civilization.civ3_city_style);
    let remembered = game
        .render_cities()
        .into_iter()
        .find(|c| c.city.x == 20 && c.city.y == 24)
        .unwrap();
    assert_eq!(
        remembered.civ3_style, expected_style,
        "remembered city retains its owner's Civ III style"
    );
    assert!(remembered.city.buildings.contains(&Building::CITY_WALLS));
    assert_eq!(game.render_tile(19, 23).terrain, Terrain::Forest);
    assert!(!game.render_tile(20, 24).visible);
    let observed_random = game.random_snapshot();
    crate::sim::ui_review::ui_review_reobserve_map(&mut game);
    assert!(crate::sim::ui_review::ui_review_observed_map_valid(
        &game, true
    ));
    let current = game
        .render_cities()
        .into_iter()
        .find(|c| c.city.x == 20 && c.city.y == 24)
        .unwrap();
    assert_eq!(current.civ3_style, expected_style);
    assert_eq!(
        observed_random,
        game.random_snapshot(),
        "observation consumes no random draws"
    );
}

#[test]
fn destroyed_or_captured_cities_remain_at_last_observed_state() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::sim::ui_review::ui_review_observed_map(&mut game);
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
    crate::sim::ui_review::ui_review_reobserve_map(&mut game);
    assert!(!game.render_cities().iter().any(|c| c.city.id == id));
}

#[test]
fn sea_map_copies_the_sellers_visual_snapshot_not_live_world() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.map_snapshot_test());
    assert_eq!(game.render_tile(23, 36).terrain, Terrain::Sea);
    assert_eq!(game.tile(23, 36).terrain, Terrain::Ocean);
}

#[test]
fn improvements_and_occupancy_are_frozen_outside_sight() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::sim::ui_review::ui_review_observed_map(&mut game);
    crate::sim::ui_review::ui_review_reobserve_map(&mut game);
    let guard = game.spawn_unit(1, UnitKind::Warrior, 20, 24);
    game.refresh_visibility();
    if let Some(unit) = game.units.iter_mut().find(|u| u.owner == HUMAN) {
        unit.x = 4;
        unit.y = 24;
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
    crate::sim::ui_review::ui_review_reobserve_map(&mut game);
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.observed_map = Default::default();
    for tile in &mut game.tiles {
        tile.seen = false;
        tile.known_by.fill(false);
    }
    game.tile_mut(23, 36).terrain = Terrain::Sea;
    game.spawn_unit(1, UnitKind::Galley, 23, 36);
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.observed_map = Default::default();
    for tile in &mut game.tiles {
        tile.seen = false;
        tile.known_by.fill(false);
    }
    let (x, y) = (23, 36);
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
    tile.terrain = Terrain::Sea;
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
    let ai = game.known_tile_for(2, x, y).unwrap();
    assert_eq!(ai.terrain, Terrain::Hills);
    assert!(ai.road && ai.railroad && !ai.terrain.is_water());
    assert_eq!(ai.owner, Some(1));
    assert_eq!(random, game.random_snapshot());
}

#[test]
fn observed_map_round_trips_and_rejects_corrupt_owners() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    crate::sim::ui_review::ui_review_observed_map(&mut game);
    let path =
        std::env::temp_dir().join(format!("civcraft-observed-map-{}.save", std::process::id()));
    game.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    assert!(crate::sim::ui_review::ui_review_observed_map_valid(
        &loaded, false
    ));
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    loaded
        .observed_map
        .tiles
        .insert((usize::MAX, 0), (0, loaded.tiles[0].clone()));
    assert!(!loaded.valid_render_memory());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unit_sight_comes_from_native_radar_flags() {
    let ruleset = RulesetData::campaign_default();
    for kind in UnitKind::ALL_NATIVE {
        let radar = kind.conquests_rule().has_capability("Radar");
        assert_eq!(
            ruleset_unit_vision_radius_sq(ruleset, kind),
            if radar { 8 } else { 2 }
        );
    }
    assert_eq!(
        ruleset_unit_vision_radius_sq(ruleset, UnitKind::Explorer),
        2
    );
    assert_eq!(ruleset_unit_vision_radius_sq(ruleset, UnitKind::Leader), 2);
}

#[test]
fn native_submarines_are_hidden_but_stealth_aircraft_use_main_sight() {
    let game = Simulation::new(crate::test_support::seeded_map());
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::StealthFighter),
        UnitVisionLayer::Main
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::StealthBomber),
        UnitVisionLayer::Main
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::Submarine),
        UnitVisionLayer::Subsurface
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::Fighter),
        UnitVisionLayer::Main
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::Guerilla),
        UnitVisionLayer::Main
    );
}

#[test]
fn mapped_hidden_layers_need_short_range_detection_inside_main_sight() {
    for hidden_kind in [UnitKind::Submarine] {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.seen = false;
            tile.visible = false;
            tile.stealth_visible = false;
            tile.known_by.fill(false);
        }

        // The Explorer sees the target tile at distance two, while its
        // Freeciv invisible/subsurface detection radius is capped at two
        // squared movement points (only distance one on this grid).
        game.spawn_unit(HUMAN, UnitKind::RadarArtillery, 10, 20);
        let target_id = game.spawn_unit(1, hidden_kind, 12, 20);
        game.refresh_visibility();
        assert!(
            game.tile(12, 20).visible,
            "{hidden_kind:?} tile is in main sight"
        );
        assert!(
            !game.unit_visible_to_human(game.unit_by_id(target_id).unwrap()),
            "{hidden_kind:?} remains hidden outside layer detection range"
        );

        // Any main-vision source also supplies the C2C3 short-range detector
        // for both Freeciv hidden layers.
        game.spawn_unit(HUMAN, UnitKind::Warrior, 11, 20);
        game.refresh_visibility();
        assert!(
            game.unit_visible_to_human(game.unit_by_id(target_id).unwrap()),
            "{hidden_kind:?} is detected within radius-squared 2"
        );
    }
}

#[test]
fn vision_uses_c2c3_effects_for_units_cities_and_reveal() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.seen = false;
        tile.known_by.fill(false);
    }

    let worker = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
    assert_eq!(
        game.unit_vision_radius_sq_for(HUMAN, UnitKind::Warrior, 10, 20),
        2
    );
    assert!(!game.pf_main_sight(HUMAN, 12, 20));
    game.tile_mut(10, 20).terrain = Terrain::Mountains;
    assert_eq!(
        game.unit_vision_radius_sq_for(HUMAN, UnitKind::Warrior, 10, 20),
        6
    );
    assert!(game.pf_main_sight(HUMAN, 12, 20));
    game.tile_mut(10, 20).terrain = Terrain::Grassland;
    game.tile_mut(10, 20).base = Some(Base::Fortress);
    assert_eq!(
        game.unit_vision_radius_sq_for(HUMAN, UnitKind::Warrior, 10, 20),
        2
    );
    game.players[HUMAN].known.push(Tech::ASTRONOMY);
    assert_eq!(
        game.unit_vision_radius_sq_for(HUMAN, UnitKind::Warrior, 10, 20),
        10
    );
    game.tile_mut(10, 20).base = Some(Base::Airbase);
    assert_eq!(
        game.unit_vision_radius_sq_for(HUMAN, UnitKind::Warrior, 10, 20),
        10
    );
    assert!(game.units.iter().any(|unit| unit.id == worker));

    game.create_city(HUMAN, 15, 20);
    let city = game.cities.len() - 1;
    assert_eq!(game.city_vision_radius_sq(HUMAN), 5);
    assert!(!game.pf_main_sight(HUMAN, 18, 20));
    game.players[HUMAN].known.push(Tech::ELECTRICITY);
    assert_eq!(game.city_vision_radius_sq(HUMAN), 10);
    assert!(game.pf_main_sight(HUMAN, 18, 20));

    game.cities[city].buildings.push(Building::APOLLO_PROGRAM);
    assert!(game.reveals_map(HUMAN));
    game.refresh_visibility();
    assert!(game.tiles.iter().all(|tile| tile.known_by[HUMAN]));
    assert!(game.tiles.iter().all(|tile| tile.seen));

    // Refresh only rebuilds current visibility. A former global reveal stays
    // in the player's map memory after its source wonder is lost.
    game.cities[city].buildings.clear();
    game.refresh_visibility();
    assert!(game.tiles.iter().all(|tile| tile.known_by[HUMAN]));
    assert!(game.tiles.iter().all(|tile| tile.seen));

    game.cities[city].buildings.push(Building::INTERNET);
    assert!(game.reveals_cities(HUMAN));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.known_by.fill(false);
        tile.seen = false;
    }
    game.create_city(HUMAN, 5, 5);
    game.cities[0].buildings.push(Building::INTERNET);
    game.create_city(1, 30, 30);
    game.refresh_visibility();
    let distant_city_tile = game.map_wrap.index(30, 30);
    assert!(game.tiles[distant_city_tile].known_by[HUMAN]);
    assert!(game.tiles[distant_city_tile].seen);
}

#[test]
fn map_show_circle_uses_base_unit_radius_without_revealing_units() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.visible = false;
        tile.seen = false;
        tile.known_by.fill(false);
    }

    let center = (10, 20);
    game.tile_mut(center.0, center.1).base = Some(Base::Airbase);
    let unit_type_radius =
        ruleset_unit_vision_radius_sq(RulesetData::campaign_default(), UnitKind::Fighter);
    let effect_radius =
        game.unit_vision_radius_sq_for(HUMAN, UnitKind::Fighter, center.0, center.1);
    assert!(
        effect_radius > unit_type_radius,
        "Airbase adds normal unit sight"
    );

    let hidden_unit_tile = game.map_wrap.civ3_neighbor(center.0, center.1, 2).unwrap();
    let hidden_unit = game.spawn_unit(1, UnitKind::Warrior, hidden_unit_tile.0, hidden_unit_tile.1);
    let outside_radius = (unit_type_radius as f64).sqrt().ceil() as i32 + 1;
    let outside_base_circle = (0..outside_radius).fold(center, |position, _| {
        game.map_wrap
            .civ3_neighbor(position.0, position.1, 2)
            .unwrap()
    });
    assert!(!game.unit_visible_to_human(game.unit_by_id(hidden_unit).unwrap()));

    game.show_map_circle_to_player(HUMAN, center.0, center.1, UnitKind::Fighter);

    assert!(game.tile(hidden_unit_tile.0, hidden_unit_tile.1).seen);
    assert!(!game.tile(hidden_unit_tile.0, hidden_unit_tile.1).visible);
    assert!(!game.unit_visible_to_human(game.unit_by_id(hidden_unit).unwrap()));
    assert!(!game.tile(outside_base_circle.0, outside_base_circle.1).seen);
    assert!(
        !game
            .tile(outside_base_circle.0, outside_base_circle.1)
            .known_by[HUMAN]
    );
}

#[test]
fn c2c3_hex_radius_sq_uses_civ3_steps_without_crossing_poles() {
    let game = Simulation::new(crate::test_support::seeded_map());
    let origin = (20, 20);
    let one_step = game.map_wrap.civ3_neighbor(origin.0, origin.1, 2).unwrap();
    let two_steps = game
        .map_wrap
        .civ3_neighbor(one_step.0, one_step.1, 2)
        .unwrap();
    let three_steps = game
        .map_wrap
        .civ3_neighbor(two_steps.0, two_steps.1, 2)
        .unwrap();
    assert!(game.within_vision_radius_sq(origin, one_step, 2));
    assert!(game.within_vision_radius_sq(origin, two_steps, 5));
    assert!(!game.within_vision_radius_sq(origin, three_steps, 5));

    let north = (origin.0, 0);
    let south = (origin.0, game.map_wrap.height() - 1);
    assert!(
        game.map_wrap
            .civ3_radius(north.0, north.1, 3)
            .iter()
            .all(|(_, y, _)| *y >= 0)
    );
    assert!(
        game.map_wrap
            .civ3_radius(south.0, south.1, 3)
            .iter()
            .all(|(_, y, _)| *y < game.map_wrap.height())
    );
}

fn empty_observation_game() -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.observed_map = ObservedMap::default();
    for tile in &mut game.tiles {
        tile.known_by.fill(false);
        tile.seen = false;
        tile.visible = false;
        tile.stealth_visible = false;
        tile.terrain = Terrain::Grassland;
    }
    game
}

#[test]
fn ai_targeting_has_its_apollo_observation() {
    let mut game = empty_observation_game();
    game.create_city(1, 10, 24);
    game.cities[0].buildings.push(Building::APOLLO_PROGRAM);
    game.refresh_visibility();
    assert!(!game.has_main_sight_at(1, 40, 32));
    assert!(game.tile(40, 32).known_by[1]);
    assert!(game.remembered_tile(1, 40, 32).is_some());
    assert_eq!(
        game.known_tile_for(1, 40, 32).unwrap().terrain,
        Terrain::Grassland
    );
    let neighbor = game.map_wrap.position(game.map_wrap.index(40, 32));
    assert!(game.safe_ocean_at(neighbor.0, neighbor.1, Some(1)));
}

#[test]
fn ai_targeting_does_not_mix_last_seen_and_live_improvements() {
    let mut game = empty_observation_game();
    game.tile_mut(10, 24).base = Some(Base::Fortress);
    let id = game.spawn_unit(1, UnitKind::Warrior, 10, 24);
    game.refresh_visibility();
    game.units.retain(|unit| unit.id != id);
    game.tile_mut(10, 24).base = None;
    game.tile_mut(10, 24).mine = true;
    game.refresh_visibility();
    assert!(!game.has_main_sight_at(1, 10, 24));
    let actual = game.known_tile_for(1, 10, 24).unwrap();
    let expected = game.remembered_tile(1, 10, 24).unwrap();
    assert_eq!((actual.base, actual.mine), (expected.base, expected.mine));
}

#[test]
fn civil_war_inherits_private_observations() {
    let mut game = empty_observation_game();
    for (x, y) in [(10, 24), (20, 24), (30, 24)] {
        game.create_city(1, x, y);
    }
    let (x, y) = (40, 32);
    let scout = game.spawn_unit(1, UnitKind::Explorer, x, y);
    game.refresh_visibility();
    game.units.retain(|unit| unit.id != scout);
    game.tile_mut(x, y).terrain = Terrain::Ocean;
    game.refresh_visibility();
    assert_eq!(
        game.remembered_tile(1, x, y).unwrap().terrain,
        Terrain::Grassland
    );
    let rebel = game.players.len();
    game.split_civil_war(1);
    assert!(game.players.len() > rebel);
    assert!(game.tile(x, y).known_by[rebel]);
    assert_eq!(
        game.remembered_tile(rebel, x, y).map(|tile| tile.terrain),
        Some(Terrain::Grassland)
    );
}

#[test]
fn shared_vision_discovers_cities_and_retains_their_public_memory() {
    let mut sim = Simulation::new(crate::mapgen::generate(42));
    sim.units.clear();
    sim.observed_map = Default::default();
    for tile in &mut sim.tiles {
        tile.known_by.fill(false);
        tile.seen = false;
        tile.visible = false;
        tile.terrain = Terrain::Grassland;
    }
    sim.create_city(1, 20, 24);
    let id = sim.cities.last().unwrap().id;
    sim.refresh_visibility();
    assert!(!sim.render_city_markers().iter().any(|c| c.id == id));
    sim.set_scenario_diplomacy(1, HUMAN, Pact::Alliance, sim.turn, [0, 0]);
    sim.grant_scenario_vision(1, HUMAN);
    sim.refresh_visibility();
    assert!(sim.rendered_city_views().iter().any(|c| c.id == id));
    assert!(sim.render_tile_view(20, 24).visible);
    assert!(sim.cancel_shared_vision(1, HUMAN));
    assert!(!sim.render_tile_view(20, 24).visible);
    assert!(sim.rendered_city_views().iter().any(|c| c.id == id));
}
