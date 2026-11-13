//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn happiness_effects_are_independent_stage_capped_and_range_aware() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_happiness_effects_playtest());
}

#[test]
fn aqueduct_health_has_value_and_science_fraction_is_preserved() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    let city = &game.cities[0];
    assert_eq!(game.building_health_want(city, Building::AQUEDUCT), 5.0);
    let mut redundant = city.clone();
    redundant.buildings.push(Building::AQUEDUCT);
    assert_eq!(
        game.building_health_want(&redundant, Building::AQUEDUCT_RIVER),
        0.0
    );
    assert_eq!(
        game.building_health_want(&redundant, Building::AQUEDUCT_LAKE),
        0.0
    );
    assert_eq!(
        game.advisor_city_output_exact(city, None),
        f64::from(game.advisor_city_output(city, None))
    );
}

#[test]
fn science_priority_truncates_to_twenty_one_before_source_float_accumulation() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let tile = game.tile_mut(10, 24);
    tile.terrain = Terrain::Grassland;
    tile.resource = None;
    tile.irrigation = false;
    tile.mine = false;
    // C2C3 grassland has no base trade; its River and Road each add one.
    tile.road = true;
    tile.river = true;
    tile.pollution = false;
    tile.fallout = false;
    game.players[1].government = Government::Communism;
    game.players[1].rates = [100, 0, 0];
    game.players[1].researching = Tech::BRONZE_WORKING;
    game.create_city(1, 10, 24);
    game.cities[0].worked_tiles.clear();
    assert_eq!(game.city_forecast(&game.cities[0]).gross_science, 2);
    assert_eq!(game.advisor_city_output_exact(&game.cities[0], None), 149.0);
    game.players[1].researching = Tech::FUTURE;
    assert_eq!(game.advisor_city_output_exact(&game.cities[0], None), 109.0);
}

#[test]
fn research_effect_wants_use_freeciv_vision_and_illness_formulas() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.cities[0].size = 9;
    let city = game.cities[0].clone();

    // daieffects.c values city and unit vision at c * amount, then
    // daitech.c applies 14/8. The Astronomy fortress requirement is possible
    // in its player+city requirement context (no specific tile is supplied).
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::ASTRONOMY),
        8 * 14 / 8
    );
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::ELECTRICITY),
        5 * 14 / 8
    );

    // C2C3 game.ruleset enables illness. Health_Pct is valued as
    // c*5 + (amount/5)*current_illness before the candidate health effect.
    let illness = game.city_illness(&city, game.city_forecast(&city).shields);
    let expected = (5 + (30 / 5) * illness) * 14 / 8;
    assert_eq!(game.city_tech_effect_want(&city, Tech::MEDICINE), expected);
}

#[test]
fn research_movement_and_upgrade_effects_count_current_civ_iii_units() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    let sea = game.spawn_unit(1, UnitKind::Caravel, 10, 24);
    let land = game.spawn_unit(1, UnitKind::Warrior, 11, 24);
    game.players[1].known.push(Tech::IRON_WORKING);
    let city = game.cities[0].clone();

    // Nuclear Power's UnitClass=Sea filter counts one C2C3 Sea-class unit.
    assert!(
        game.upgrade_target(game.unit_by_id(land).unwrap())
            .is_some()
    );
    assert!(game.unit_by_id(sea).is_some());
    assert_eq!(game.city_tech_effect_want(&city, Tech::NUCLEAR_POWER), 1);

    // Freeciv num_affected_units matches a class against the effect's entire
    // requirement vector. UnitType=Nuclear is not applicable to unit classes,
    // so the source scorer counts both owned units for these Move_Bonus rows.
    let all_units_want = 2 * 14 / 8;
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::ADVANCED_FLIGHT),
        all_units_want
    );
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::ROCKETRY),
        all_units_want
    );

    // EFT_UPGRADE_PRICE_PCT uses -upgradeable * amount / 2; the selected
    // Invention effect is -50 and this Warrior has an available successor.
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::INVENTION),
        25 * 14 / 8
    );
}

#[test]
fn research_make_content_effects_use_stage_specific_unhappiness() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.cities[0].size = 12;
    game.cities[0]
        .buildings
        .extend([Building::TEMPLE, Building::CATHEDRAL, Building::COLOSSEUM]);
    let city = game.cities[0].clone();
    let luxury = game.city_forecast(&city).luxury;
    let max_converted = game.city_min_unhappy_after_effect_stage(&city, luxury);
    let entertainers = game.city_entertainer_count(&city);
    let content_effect = 35 * 1.min(max_converted + entertainers);

    // Mysticism and Theology add one Make_Content through their respective
    // city buildings. Electricity adds five City_Vision_Radius_Sq, but its
    // candidate effect requiring Colosseum is inactive because Electricity
    // makes that building obsolete.
    assert!(content_effect > 0);
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::MYSTICISM),
        content_effect * 14 / 8
    );
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::THEOLOGY),
        content_effect * 14 / 8
    );
    assert_eq!(
        game.city_tech_effect_want(&city, Tech::ELECTRICITY),
        5 * 14 / 8
    );
}
