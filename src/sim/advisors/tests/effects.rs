use super::*;

#[test]
fn happiness_effects_are_independent_stage_capped_and_range_aware() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_happiness_effects_playtest());
}

#[test]
fn aqueduct_health_has_value_and_science_fraction_is_preserved() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let tile = game.tile_mut(10, 24);
    tile.terrain = Terrain::Grassland;
    tile.resource = None;
    tile.irrigation = false;
    tile.mine = false;
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
