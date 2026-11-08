//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn native_defense_effects_and_static_research_match_independent_numeric_cases() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.native_defensive_effects_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn defense_effect_capital_and_ocean_city_preserve_native_integer_divisions() {
    let mut context = DefenseEffectContext {
        capital: false,
        ocean_city: false,
        coastal: false,
        continent_threat: false,
        ocean_threat: false,
        invasions: false,
    };
    assert_eq!(
        Simulation::native_defense_effect_value(&context, true, false),
        3.0
    );
    context.capital = true;
    assert_eq!(
        Simulation::native_defense_effect_value(&context, false, false),
        11.0
    );
    context.capital = false;
    context.ocean_city = true;
    assert_eq!(
        Simulation::native_defense_effect_value(&context, false, true),
        7.0
    );
    context.ocean_threat = true;
    assert_eq!(
        Simulation::native_defense_effect_value(&context, false, true),
        19.0
    );
}

#[test]
fn static_defender_research_is_gated_by_real_production_need_and_keeps_police_value() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.native_defensive_effects_playtest(),
        (true, true, true, true)
    );
    game.units.clear();
    assert_eq!(
        game.production_defender_tech_want(&game.cities[0], Tech::BRONZE_WORKING),
        0.0
    );
    game.cities[0].size = 3;
    game.cities[0].specialists[2] = 1;
    let police = game.production_defender_tech_want(&game.cities[0], Tech::BRONZE_WORKING);
    assert!((police - 174.27983).abs() < 0.001);
    game.players[1].known.push(Tech::BRONZE_WORKING);
    assert_eq!(
        game.production_defender_tech_want(&game.cities[0], Tech::BRONZE_WORKING),
        0.0
    );
}

#[test]
fn high_tax_actual_upkeep_does_not_make_null_context_type_upkeep_positive() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
    }
    game.players[1].known = vec![Tech::BRONZE_WORKING];
    game.players[1].government = Government::Despotism;
    game.players[1].rates = [40, 60, 0];
    game.create_city(1, 10, 24);
    game.cities[0].buildings.push(Building::BARRACKS);
    let city = game.cities[0].clone();
    let charge = game.spawn_unit(1, UnitKind::Catapult, 10, 24);
    game.request_ai_guard(charge);
    for _ in 0..20 {
        game.spawn_unit(1, UnitKind::Warrior, 10, 24);
        game.units.last_mut().unwrap().home_city = Some(city.id);
    }
    assert!(!game.city_gold_upkeep(&city).is_empty());
    assert_eq!(
        game.bodyguard_production_choice(&city, false)
            .map(|choice| choice.0),
        Some(UnitKind::Phalanx)
    );
}

#[test]
fn defense_baseline_counts_untrusted_armistice_and_trusted_ceasefire_cities() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.native_defensive_effects_playtest(),
        (true, true, true, true)
    );
    game.units.clear();
    let other = offset_native_by_map(10, 24, 4, 0);
    game.create_city(HUMAN, other.0, other.1);
    game.sign_pact(1, HUMAN, Pact::Peace);
    assert_eq!(game.pact(1, HUMAN), Pact::Armistice);
    assert!(game.advisor_player_dangerous(1, HUMAN));
    assert_eq!(
        game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
        83.0
    );
    game.diplomacy_playtest_friendship();
    // The reusable friendship setup also creates a disliked alternate
    // rival. Isolate this baseline comparison to the treaty partner.
    game.units.retain(|unit| unit.owner != 2);
    game.cities.retain(|city| city.owner != 2);
    assert!(!game.advisor_player_dangerous(1, HUMAN));
    assert_eq!(
        game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
        3.0
    );
    game.sign_pact(1, HUMAN, Pact::Ceasefire);
    assert!(game.advisor_player_dangerous(1, HUMAN));
    assert_eq!(
        game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
        83.0
    );
}

#[test]
fn desktop_city_defense_fixture_checks_independent_native_values() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.native_city_defense_advisor_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn city_defense_percent_matches_literal_native_feedback_and_urgency() {
    let assessment = |danger, defense, urgency| GuardCityAssessment {
        danger,
        defense,
        urgency,
        grave: 0,
        defense_igwall: defense,
    };
    assert_eq!(
        Simulation::native_city_defense_danger(assessment(10, 400, 3), 1),
        25
    );
    assert_eq!(
        Simulation::native_city_defense_danger(assessment(30, 400, 3), 1),
        203
    );
    assert_eq!(
        Simulation::native_city_defense_danger(assessment(30, 400, 0), 1),
        100
    );
    assert_eq!(
        Simulation::native_city_defense_danger(assessment(30, 0, 11), 0),
        211
    );
    assert_eq!(
        Simulation::native_city_defense_danger(assessment(30, 400, 3), 0),
        0
    );
    assert_eq!(
        Simulation::native_city_defense_danger(assessment(0, 0, 0), 1),
        0
    );
}

#[test]
fn quiet_empty_city_has_no_defense_bid_but_entertainer_can_request_police() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.river = false;
        tile.base = None;
    }
    game.create_city(1, 10, 24);
    assert!(game.native_city_defense_choice(&game.cities[0]).0.is_none());
    game.cities[0].size = 3;
    game.cities[0].specialists[2] = 1;
    let (choice, martial) = game.native_city_defense_choice(&game.cities[0]);
    assert_eq!(martial, 35.0);
    assert_eq!(choice.unwrap().want, 35.0);
    game.players[1].government = Government::Communism;
    assert_eq!(game.city_martial_law_want(&game.cities[0]), 35.0);
    // Native unit_list_size counts civilians too for the maximum check.
    for _ in 0..3 {
        game.spawn_unit(1, UnitKind::Caravan, 10, 24);
    }
    assert_eq!(game.city_martial_law_want(&game.cities[0]), 0.0);
}

#[test]
fn defense_building_uses_native_provider_order_and_counts_all_tile_units() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.native_city_defense_advisor_playtest(),
        (true, true, true, true)
    );
    game.units.clear();
    game.cities[0].size = 1;
    game.cities[0].specialists = [0; 3];
    game.players[1].known.push(Tech::MASONRY);
    game.spawn_unit(1, UnitKind::Phalanx, 10, 24);
    for _ in 0..11 {
        game.spawn_unit(1, UnitKind::Caravan, 10, 24);
    }
    let threat = offset_native_by_map(10, 24, 1, 0);
    game.spawn_unit(HUMAN, UnitKind::Cannon, threat.0, threat.1);
    let city = &game.cities[0];
    assert_eq!(
        game.defense_building_provider(city, None),
        Some(Building::CITY_WALLS)
    );
    let assessment = game.advisor_guard_city_assessment(city);
    assert_eq!(assessment.urgency, 11);
    assert_eq!(assessment.defense, 100);
    let choice = game.native_city_defense_choice(city).0.unwrap();
    assert_eq!(
        choice.production,
        Production::Building(Building::CITY_WALLS)
    );
    assert_eq!(choice.want, 105.5);
}

#[test]
fn big_land_city_defense_does_not_receive_terrain_defense() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.tile_mut(10, 24).river = false;
    game.tile_mut(10, 24).base = None;
    game.create_city(1, 10, 24);
    let city = game.cities[0].clone();
    for kind in [UnitKind::Chariot, UnitKind::Catapult, UnitKind::Cannon] {
        game.units.clear();
        game.spawn_unit(1, kind, 10, 24);
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        let plain = game.advisor_guard_city_assessment(&city).defense;
        game.tile_mut(10, 24).terrain = Terrain::Hills;
        assert!(plain > 0);
        assert_eq!(
            game.advisor_guard_city_assessment(&city).defense,
            plain,
            "provided Big Land class omits TerrainDefense"
        );
    }
}

#[test]
fn suicide_attack_actions_contribute_missile_city_danger() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.base = None;
    }
    game.create_city(1, 10, 24);
    let city = game.cities[0].clone();
    let near = offset_native_by_map(10, 24, 1, 0);
    game.spawn_unit(HUMAN, UnitKind::CruiseMissile, near.0, near.1);
    let danger = game.advisor_guard_city_assessment(&city);
    assert_eq!(
        (danger.danger, danger.urgency, danger.grave),
        (3600, 0, 0),
        "Suicide Attack has native ACTRES_ATTACK"
    );
    game.units.clear();
    game.players[HUMAN].known.push(Tech::ADVANCED_FLIGHT);
    game.spawn_unit(HUMAN, UnitKind::Nuclear, near.0, near.1);
    assert_eq!(
        game.advisor_guard_city_assessment(&city).danger,
        108900,
        "native tactical action assessment scans enablers without action-blocker checks"
    );
    let coast = offset_native_by_map(10, 24, -1, 0);
    game.tile_mut(coast.0, coast.1).terrain = Terrain::Ocean;
    game.spawn_unit(1, UnitKind::AegisCruiser, 10, 24);
    assert_eq!(
        game.advisor_guard_city_assessment(&city).danger,
        108900,
        "Nuclear lacks the AirAttacker flag; Aegis bonus must not apply"
    );
    game.units
        .iter_mut()
        .find(|u| u.owner == HUMAN)
        .unwrap()
        .kind = UnitKind::CruiseMissile;
    assert_eq!(
        game.advisor_guard_city_assessment(&city).danger,
        289,
        "Cruise Missile has AirAttacker; native specific defense bonus is damped to250%"
    );
}
