//! Regression tests for advisors behavior and its interaction with the simulation.

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
    game.players[1].known.push(Tech::ECONOMICS);
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
fn wonder_city_requires_a_helper_cluster_and_rejects_immediate_danger() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.players[1].known = Tech::all().collect();
    let city = game.cities[0].clone();
    assert_eq!(
        game.building_blocker(&city, Building::SPACE_MODULE)
            .as_deref(),
        Some("Needs the Apollo Program")
    );
    game.apollo_completed = true;
    game.cities[0].buildings.extend([
        Building::FACTORY,
        Building::LIBRARY,
        Building::UNIVERSITY,
        Building::RESEARCH_LAB,
    ]);
    let city = game.cities[0].clone();
    assert_eq!(game.building_blocker(&city, Building::SPACE_MODULE), None);
}

#[test]
fn productive_wonder_city_retains_project_and_suppresses_space_parts() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.players[1].known = Tech::all().collect();
    game.cities[0].production = Production::Building(Building::APOLLO_PROGRAM);
    let city = game.cities[0].clone();
    assert_eq!(Building::APOLLO_PROGRAM.data().genus, Genus::SmallWonder);
    assert_eq!(
        game.building_blocker(&city, Building::SPACE_MODULE)
            .as_deref(),
        Some("Needs the Apollo Program")
    );
    game.apollo_completed = true;
    game.cities[0].buildings.extend([
        Building::FACTORY,
        Building::LIBRARY,
        Building::UNIVERSITY,
        Building::RESEARCH_LAB,
    ]);
    let city = game.cities[0].clone();
    assert_eq!(game.building_blocker(&city, Building::SPACE_MODULE), None);
}

#[test]
fn space_parts_need_a_competitive_empire_or_an_active_race() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.players[1].known = Tech::all().collect();
    game.apollo_completed = true;
    game.cities[0].buildings.extend([
        Building::FACTORY,
        Building::LIBRARY,
        Building::UNIVERSITY,
        Building::RESEARCH_LAB,
    ]);
    let city = game.cities[0].clone();
    assert_eq!(game.building_blocker(&city, Building::SPACE_MODULE), None);
    game.spaceships[1].built[2] = 12;
    assert_eq!(
        game.building_blocker(&city, Building::SPACE_MODULE)
            .as_deref(),
        Some("Spaceship part unavailable")
    );
}
