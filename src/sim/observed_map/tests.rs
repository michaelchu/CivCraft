//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn filtered_city_rendering_and_markers_preserve_live_and_remembered_state() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::sim::scenarios::ui_review::ui_review_observed_map(&mut game);
    for reobserved in [false, true] {
        if reobserved {
            crate::sim::scenarios::ui_review::ui_review_reobserve_map(&mut game);
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
    crate::sim::scenarios::ui_review::ui_review_observed_map(&mut game);
    crate::sim::scenarios::ui_review::ui_review_reobserve_map(&mut game);
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::sim::scenarios::ui_review::ui_review_observed_map(&mut game);
    assert!(crate::sim::scenarios::ui_review::ui_review_observed_map_valid(&game, false));
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
    crate::sim::scenarios::ui_review::ui_review_reobserve_map(&mut game);
    assert!(crate::sim::scenarios::ui_review::ui_review_observed_map_valid(&game, true));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::sim::scenarios::ui_review::ui_review_observed_map(&mut game);
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
    crate::sim::scenarios::ui_review::ui_review_reobserve_map(&mut game);
    assert!(!game.render_cities().iter().any(|c| c.city.id == id));
}

#[test]
fn sea_map_copies_the_sellers_visual_snapshot_not_live_world() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.map_snapshot_playtest());
    assert_eq!(game.render_tile(23, 36).terrain, Terrain::Sea);
    assert_eq!(game.tile(23, 36).terrain, Terrain::Grassland);
}

#[test]
fn improvements_and_occupancy_are_frozen_outside_sight() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::sim::scenarios::ui_review::ui_review_observed_map(&mut game);
    crate::sim::scenarios::ui_review::ui_review_reobserve_map(&mut game);
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
    crate::sim::scenarios::ui_review::ui_review_reobserve_map(&mut game);
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
    let ai = game.ai_map_memory.get(&(2, index)).unwrap();
    assert_eq!(ai.route.terrain, Terrain::Hills);
    assert!(ai.route.road && ai.route.railroad && !ai.ocean);
    assert_eq!(ai.owner, Some(1));
    assert_eq!(random, game.random_snapshot());
}

#[test]
fn observed_map_round_trips_and_rejects_corrupt_owners() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    crate::sim::scenarios::ui_review::ui_review_observed_map(&mut game);
    let path =
        std::env::temp_dir().join(format!("civcraft-observed-map-{}.save", std::process::id()));
    game.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    assert!(crate::sim::scenarios::ui_review::ui_review_observed_map_valid(&loaded, false));
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    loaded
        .observed_map
        .tiles
        .insert((usize::MAX, 0), (0, loaded.tiles[0].clone()));
    assert!(!loaded.valid_render_memory());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn c2c3_unit_sight_values_resolve_through_civ3_sight_bindings() {
    let ruleset = RulesetData::campaign_default();
    assert_eq!(ruleset_unit_vision_radius_sq(ruleset, UnitKind::Fighter), 8);
    assert_eq!(
        ruleset_unit_vision_radius_sq(ruleset, UnitKind::Explorer),
        6
    );
    assert_eq!(
        ruleset_unit_vision_radius_sq(ruleset, UnitKind::Conquistador),
        6
    );
    assert_eq!(ruleset_unit_vision_radius_sq(ruleset, UnitKind::Leader), 8);
    assert_eq!(ruleset_unit_vision_radius_sq(ruleset, UnitKind::Warrior), 2);
    for (kind, section) in [
        (UnitKind::Horseman, "unit_horsemen"),
        (UnitKind::Pikeman, "unit_pikemen"),
        (UnitKind::Rifleman, "unit_riflemen"),
        (UnitKind::MechInfantry, "unit_mech_inf"),
    ] {
        let id = kind
            .conquests_id()
            .expect("campaign unit has a Conquests identity");
        assert_eq!(
            unit_sight_binding_for(id),
            Some(SightBinding::SightAlias(section)),
            "{id}"
        );
        assert_eq!(ruleset_unit_vision_radius_sq(ruleset, kind), 2, "{id}");
        assert_eq!(
            ruleset_unit_vision_layer(ruleset, kind),
            UnitVisionLayer::Main,
            "{id}"
        );
    }

    for kind in UnitKind::ALL_NATIVE {
        assert!(
            ruleset_unit_vision_radius_sq(ruleset, kind) >= 2,
            "missing or invalid sight binding for {kind:?}"
        );
    }

    // These Freeciv-only unit identities have no exact Civ III unit record.
    // Do not silently use Guerilla or F-15 as semantic substitutes; Civ III
    // Conquests has no Spy unit identity at all.
    assert_eq!(
        ruleset_unit_vision_radius_sq(ruleset, UnitKind::Guerilla),
        2
    );
    assert_eq!(ruleset_unit_vision_radius_sq(ruleset, UnitKind::F15), 8);

    for kind in UnitKind::ALL_NATIVE {
        let id = kind
            .conquests_id()
            .expect("every campaign unit has a Conquests identity");
        if unit_sight_binding_for(id) == Some(SightBinding::Unresolved) {
            assert_eq!(ruleset_unit_vision_radius_sq(ruleset, kind), 2, "{id}");
            let expected_layer = if kind.uses_stealth_vision() {
                UnitVisionLayer::Stealth
            } else {
                UnitVisionLayer::Main
            };
            assert_eq!(
                ruleset_unit_vision_layer(ruleset, kind),
                expected_layer,
                "{id}"
            );
        }
    }

    assert_eq!(UNMAPPED_FREECIV_UNIT_VISION.len(), 3);
    for gap in UNMAPPED_FREECIV_UNIT_VISION {
        let record = ruleset
            .content
            .iter()
            .find(|record| record.section == gap.freeciv_section)
            .unwrap_or_else(|| panic!("missing explicit Freeciv unit gap {}", gap.freeciv_section));
        assert_eq!(
            record.integer_field("vision_radius_sq").unwrap(),
            Some(i64::from(gap.freeciv_radius_sq)),
            "{}: {}",
            gap.freeciv_section,
            gap.note
        );
        assert_eq!(gap.fallback_radius_sq, 2);
    }
    assert_eq!(
        UNMAPPED_FREECIV_UNIT_VISION[0].civ3_catalog_id,
        Some("guerilla")
    );
    assert_eq!(
        UNMAPPED_FREECIV_UNIT_VISION[0].civ3_source,
        Some("Conquests/conquests.biq#PRTO[75]")
    );
    assert_eq!(
        UNMAPPED_FREECIV_UNIT_VISION[1].civ3_catalog_id, None,
        "Conquests does not have a Spy unit record"
    );
    assert_eq!(
        UNMAPPED_FREECIV_UNIT_VISION[2].civ3_catalog_id,
        Some("f_15")
    );
    assert_eq!(
        UNMAPPED_FREECIV_UNIT_VISION[2].civ3_source,
        Some("Conquests/conquests.biq#PRTO[64]")
    );
}

#[test]
fn c2c3_vision_layers_bind_stealth_and_subsurface_units() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::StealthFighter),
        UnitVisionLayer::Stealth
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::StealthBomber),
        UnitVisionLayer::Stealth
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::Submarine),
        UnitVisionLayer::Subsurface
    );
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::Fighter),
        UnitVisionLayer::Main
    );
    let unresolved_fallback = if UnitKind::Guerilla.uses_stealth_vision() {
        UnitVisionLayer::Stealth
    } else {
        UnitVisionLayer::Main
    };
    assert_eq!(
        game.unit_vision_layer_for(UnitKind::Guerilla),
        unresolved_fallback
    );
}

#[test]
fn mapped_hidden_layers_need_short_range_detection_inside_main_sight() {
    for hidden_kind in [
        UnitKind::StealthFighter,
        UnitKind::StealthBomber,
        UnitKind::Submarine,
    ] {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
fn c2c3_hex_radius_sq_uses_civ3_steps_without_crossing_poles() {
    let game = Simulation::new(crate::mapgen::generate(42));
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
