use super::*;

#[test]
fn small_wonder_projects_government_center_effects_across_empire() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = true;
    }
    game.create_city(1, 5, 8);
    game.create_city(1, 20, 40);
    game.create_city(1, 25, 40);
    game.players[1].known = vec![Tech::THEOLOGY];
    game.players[1].government = Government::Despotism;
    game.players[1].rates = [80, 20, 0];
    for city in &mut game.cities {
        city.size = 4;
    }
    game.reconcile_city_workers();
    game.cities[1].production = Production::Building(Building::ECCLESIASTICAL_PALACE);
    let city = game.cities[1].clone();
    let original = game.advisor_city_output(&game.cities[2], None);
    let mut scratch = game.clone();
    let want = game.projected_wonder_want(&mut scratch, &city, Building::ECCLESIASTICAL_PALACE);
    assert!(want > 0.0);
    scratch.cities[1]
        .buildings
        .push(Building::ECCLESIASTICAL_PALACE);
    assert!(scratch.advisor_city_output(&scratch.cities[2], None) > original);
    assert_eq!(
        game.wonder_build_choice(&city).unwrap().production,
        Production::Building(Building::ECCLESIASTICAL_PALACE)
    );
    game.players[1].known.push(Tech::TRADE);
    assert!(game.unit_available(1, UnitKind::Caravan));
    assert!(
        game.wonder_helper_production_choice(&game.cities[2])
            .is_none()
    );
    game.cities[2]
        .buildings
        .push(Building::ECCLESIASTICAL_PALACE);
    assert!(game.wonder_build_choice(&city).is_none_or(
        |choice| choice.production != Production::Building(Building::ECCLESIASTICAL_PALACE)
    ));
}

#[test]
fn wonder_production_and_helpers_compete_using_empire_effects() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_wonder_advisor_playtest());
}

#[test]
fn projected_wonder_counts_other_cities_and_restores_scratch_state() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = true;
    }
    game.create_city(1, 10, 24);
    game.players[1].known = game.research_path(1, Tech(41) /* Literacy */);
    game.players[1].rates = [80, 20, 0];
    game.cities[0].buildings.push(Building::LIBRARY);
    let city = game.cities[0].clone();
    let mut scratch = game.clone();
    let before = bincode::serialize(&scratch).unwrap();
    let single = game.projected_wonder_want(&mut scratch, &city, Building::GREAT_LIBRARY);
    assert_eq!(bincode::serialize(&scratch).unwrap(), before);
    game.create_city(1, 20, 24);
    game.cities[1].size = 5;
    game.cities[1].buildings.push(Building::LIBRARY);
    game.reconcile_city_workers();
    scratch = game.clone();
    let empire = game.projected_wonder_want(&mut scratch, &city, Building::GREAT_LIBRARY);
    assert!(empire > single);
    // Empire-wide surviving science boost is still projected when the
    // building city itself has no extra workers or specialist output.
    assert!(!game.completed_wonders.contains(&Building::GREAT_LIBRARY));
    assert!(!game.cities[0].has_building(Building::GREAT_LIBRARY));
}

#[test]
fn wonder_helpers_respect_seven_turn_route_and_queued_contributions() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_wonder_advisor_playtest());
    // Fixture ends with enough helpers; moving them onto another
    // continent removes their contribution to this continent's count.
    let id = game.ai_wonder_city(1).unwrap();
    let helper_city = game
        .cities
        .iter()
        .find(|city| city.id != id)
        .unwrap()
        .clone();
    game.units
        .retain(|unit| !matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight));
    assert!(game.wonder_helper_production_choice(&helper_city).is_some());
    let wonder_city = game
        .cities
        .iter()
        .find(|city| city.id == id)
        .unwrap()
        .clone();
    for tile in &mut game.tiles {
        tile.road = false;
        tile.river = false;
    }
    let mut distant = helper_city.clone();
    (distant.x, distant.y) = offset_native_by_map(wonder_city.x, wonder_city.y, 20, 15);
    assert!(game.wonder_helper_production_choice(&distant).is_none());
}

#[test]
fn wonder_city_requires_a_helper_cluster_and_rejects_immediate_danger() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.road = true;
        tile.river = true;
        tile.hut = false;
        tile.known_by[1] = true;
    }
    for (dx, dy) in [(0, 0), (2, 0), (4, 0), (0, 2), (2, 2)] {
        let (x, y) = offset_native_by_map(10, 24, dx, dy);
        game.create_city(1, x, y);
    }
    assert_eq!(game.ai_wonder_city(1), None);
    let (x, y) = offset_native_by_map(10, 24, 4, 2);
    game.create_city(1, x, y);
    game.players[1].known.extend([Tech::TRADE, Tech::POTTERY]);
    let chosen = game.ai_wonder_city(1).expect("six connected helper cities");
    let city = game
        .cities
        .iter()
        .find(|city| city.id == chosen)
        .unwrap()
        .clone();
    let (x, y) = offset_native_by_map(city.x, city.y, 1, 0);
    game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
    assert_ne!(game.ai_wonder_city(1), Some(chosen));
}

#[test]
fn productive_wonder_city_retains_project_and_suppresses_space_parts() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
    }
    game.create_city(1, 10, 24);
    game.players[1].known = Tech::all().collect();
    game.cities[0].production = Production::Building(Building::APOLLO_PROGRAM);
    let city = game.cities[0].clone();
    assert_eq!(game.ai_wonder_city(1), Some(city.id));
    assert_eq!(
        game.space_production_want(&city, Building::SPACE_MODULE),
        24.0
    );
    game.spaceships[1].built[0] = 1;
    assert_eq!(
        game.space_production_want(&city, Building::SPACE_MODULE),
        72.0
    );
    game.cities[0].production = Production::Unit(UnitKind::Warrior);
    assert_eq!(game.ai_wonder_city(1), None);
}

#[test]
fn space_parts_need_a_competitive_empire_or_an_active_race() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Hills;
        tile.mine = true;
    }
    game.create_city(1, 10, 24);
    game.create_city(HUMAN, 20, 24);
    game.cities[1].size = 4;
    game.reconcile_city_workers();
    // Rival leads technology and manufacturing (ties go to player zero).
    game.players[HUMAN].known = Tech::all().collect();
    game.cities[0].specialists = [1, 0, 0];
    game.cities[0].worked_tiles.clear();
    let city = game.cities[0].clone();
    assert_eq!(
        game.space_production_want(&city, Building::SPACE_MODULE),
        0.0
    );
    assert_eq!(
        game.space_production_want(&city, Building::APOLLO_PROGRAM),
        10.0
    );
    game.spaceships[HUMAN].built[0] = 1;
    assert_eq!(
        game.space_production_want(&city, Building::SPACE_MODULE),
        210.0
    );
    game.spaceships[1].built[0] = 1;
    assert_eq!(
        game.space_production_want(&city, Building::SPACE_MODULE),
        630.0
    );
    game.spaceships[HUMAN].built = [0; 3];
    game.spaceships[1].built = [0; 3];
    game.players[1].known = Tech::all().collect();
    game.players[1].future_techs = 1;
    assert_eq!(
        game.space_production_want(&city, Building::SPACE_MODULE),
        210.0
    );
    assert_eq!(
        game.space_production_want(&city, Building::APOLLO_PROGRAM),
        160.0
    );
}
