//! Regression tests for the behavior of the parent module.

use super::production::Growth;
use super::*;

#[test]
fn standard_conquests_capture_drops_city_culture_without_crediting_the_captor() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    game.create_city(HUMAN, 30, 24);
    let city_id = game.cities[1].id;
    game.cities[1].add_culture(1, 300);
    game.cities[1].add_culture(HUMAN, 75);
    game.cities[2].add_culture(HUMAN, 500);
    assert_eq!(game.civilization_culture(1), 300);
    assert_eq!(game.civilization_culture(HUMAN), 500);
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);

    game.capture_city_if_needed(attacker, 20, 24);

    let city = &game.cities[game.city_index_by_id(city_id).unwrap()];
    assert_eq!(city.owner, HUMAN);
    assert_eq!(city.culture_for_civilization(1), 0);
    assert_eq!(city.culture_for_civilization(HUMAN), 0);
    assert_eq!(city.culture_by_civilization, [(HUMAN, 0)]);
    assert_eq!(game.civilization_culture(1), 0);
    assert_eq!(game.civilization_culture(HUMAN), 500);
}

#[test]
fn retain_culture_capture_keeps_the_city_culture_ledger() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    let city_id = game.cities[1].id;
    game.cities[1].add_culture(1, 300);
    game.cities[1].add_culture(HUMAN, 75);
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);

    game.capture_city_with_culture_retention(attacker, 20, 24, true);

    let city = &game.cities[game.city_index_by_id(city_id).unwrap()];
    assert_eq!(city.owner, HUMAN);
    assert_eq!(city.culture_for_civilization(1), 300);
    assert_eq!(city.culture_for_civilization(HUMAN), 75);
    assert_eq!(city.total_culture(), 375);
}

#[test]
fn fascist_capture_suppresses_culture_until_owner_nation_is_a_majority() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[HUMAN].government = Government::Fascism;
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    let city_id = game.cities[1].id;
    game.cities[1].size = 5;
    game.cities[1].buildings.push(Building::TEMPLE);
    game.cities[1].record_culture_building_completion(Building::TEMPLE, 1, game.year);
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);

    game.capture_city_if_needed(attacker, 20, 24);

    let index = game.city_index_by_id(city_id).unwrap();
    assert_eq!(game.cities[index].owner, HUMAN);
    assert!(game.cities[index].culture_suppressed_until_majority_assimilated);
    game.record_city_culture(index);
    assert_eq!(game.cities[index].culture_for_civilization(HUMAN), 0);

    game.cities[index].foreign_citizens[0].1 = 1;
    game.record_city_culture(index);
    assert!(!game.cities[index].culture_suppressed_until_majority_assimilated);
    assert_eq!(game.cities[index].culture_for_civilization(HUMAN), 0);
    game.cities[index].buildings.push(Building::LIBRARY);
    game.cities[index].record_culture_building_completion(Building::LIBRARY, HUMAN, game.year);
    game.record_city_culture(index);
    assert_eq!(game.cities[index].culture_for_civilization(HUMAN), 3);
}

#[test]
fn captured_culture_buildings_wait_for_their_builder_and_restart_age_after_recapture() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.year = 0;
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    let city_id = game.cities[1].id;
    game.cities[1].buildings.push(Building::TEMPLE);
    game.cities[1].record_culture_building_completion(Building::TEMPLE, 1, -1500);
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
    game.capture_city_if_needed(attacker, 20, 24);

    let index = game.city_index_by_id(city_id).unwrap();
    assert_eq!(
        game.cities[index].culture_building_owners,
        [(Building::TEMPLE.0, 1)]
    );
    assert_eq!(
        game.cities[index].culture_building_year(Building::TEMPLE, 0),
        0
    );
    game.record_city_culture(index);
    assert_eq!(game.cities[index].culture_for_civilization(HUMAN), 0);

    game.year = 100;
    let recapturer = game.spawn_unit(1, UnitKind::Warrior, 20, 24);
    game.capture_city_if_needed(recapturer, 20, 24);
    game.record_city_culture(index);
    assert_eq!(game.cities[index].culture_for_civilization(1), 2);

    game.year = 1100;
    game.record_city_culture(index);
    assert_eq!(game.cities[index].culture_for_civilization(1), 6);
}

#[test]
fn capture_initializes_deterministic_resistance_from_foreign_population() {
    fn capture_with_seed(seed: u64, captor_culture: i64) -> (Vec<(usize, i32)>, Vec<(usize, i32)>) {
        let mut game = Simulation::new(crate::mapgen::generate(seed));
        game.units.clear();
        game.cities.clear();
        game.create_city(1, 10, 24);
        game.create_city(1, 20, 24);
        game.create_city(HUMAN, 30, 24);
        game.cities[1].size = 6;
        game.cities[1].add_culture(1, 100);
        game.cities[2].add_culture(HUMAN, captor_culture);
        game.sign_pact(HUMAN, 1, Pact::War);
        let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
        game.capture_city_if_needed(attacker, 20, 24);
        (
            game.cities[1].foreign_citizens.clone(),
            game.cities[1].resisting_citizens.clone(),
        )
    }

    let (foreign, resisting) = capture_with_seed(42, 0);
    assert_eq!(foreign, [(1, 5)]);
    assert!(!resisting.is_empty());
    assert_eq!(resisting, capture_with_seed(42, 0).1);
    let culturally_stronger_captor = capture_with_seed(42, 300).1;
    assert!(
        resisting.iter().map(|&(_, count)| count).sum::<i32>()
            >= culturally_stronger_captor
                .iter()
                .map(|&(_, count)| count)
                .sum::<i32>(),
        "higher pre-capture captor culture should not increase resistance under the BIQ-derived bands"
    );
    assert!(
        resisting
            .iter()
            .all(|&(nationality, count)| { nationality == 1 && count > 0 && count <= 5 })
    );
}

#[test]
fn six_civilizations_found_capitals_then_use_unique_names_after_capture_and_loss() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    for owner in 0..PLAYER_COUNT {
        let civ = crate::rules::civilizations::by_id(
            game.players[owner].civilization_id.as_deref().unwrap(),
        )
        .unwrap();
        assert_eq!(game.players[owner].name, civ.name);
        assert_eq!(game.players[owner].leader_name(), civ.leader);
        game.create_city(owner, 10 + owner as i32 * 4, 20);
        let city = game.cities.last().unwrap();
        assert_eq!(city.name, civ.cities[0]);
        assert!(city.has_building(Building::PALACE));
    }
    // Capturing Rome does not let its former owner found another Rome.
    game.cities[0].owner = 1;
    game.create_city(HUMAN, 10, 30);
    assert_eq!(game.cities.last().unwrap().name, "Veii");
    game.create_city(HUMAN, 14, 30);
    assert_eq!(game.cities.last().unwrap().name, "Antium");
    game.cities.retain(|city| city.name != "Veii");
    let rng = game.random_snapshot();
    assert_eq!(game.city_name(HUMAN), "Veii");
    // Naming consumes no random draws (founding itself has game effects).
    assert_eq!(game.random_snapshot(), rng);
}

#[test]
fn lifecycle_cm_replaces_existing_workers_when_population_weights_change() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.city_worker_lifecycle_playtest(), (true, true, true));
}

#[test]
fn city_events_preserve_other_manual_workers_and_normal_build_completion_does_not_arrange() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.resource = None;
        tile.river = false;
    }
    game.create_city(HUMAN, 12, 24);
    let manual = offset_native_by_map(12, 24, 1, 0);
    game.tile_mut(manual.0, manual.1).terrain = Terrain::Mountains;
    game.cities[0].worked_tiles = vec![manual];
    game.cities[0].specialists = [0; 3];
    game.create_city(HUMAN, 30, 40);
    assert_eq!(game.cities[0].worked_tiles, vec![manual]);
    game.cities[1].size = 2;
    game.cities[1].specialists = [0, 0, 1];
    game.reduce_city_population(1);
    assert_eq!(game.cities[0].worked_tiles, vec![manual]);
    game.players[HUMAN].known.push(Tech::POTTERY);
    game.cities[0].production = Production::Building(Building::GRANARY);
    game.cities[0].shields = 100;
    game.cities[0].food = 10;
    game.process_cities();
    assert!(game.cities[0].has_building(Building::GRANARY));
    assert_eq!(game.cities[0].size, 1);
    assert_eq!(game.cities[0].worked_tiles, vec![manual]);
}

#[test]
fn population_loss_removes_specialists_before_native_city_index_workers() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 12, 24);
    let city = &mut game.cities[0];
    let mut tiles: Vec<_> = Simulation::city_radius(city)
        .filter(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
        .collect();
    tiles.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
    let first = (tiles[0].2, tiles[0].3);
    let second = (tiles[1].2, tiles[1].3);
    city.size = 5;
    city.specialists = [1, 1, 1];
    city.worked_tiles = vec![second, first];
    city.size -= 4;
    city.remove_citizen_assignments(4, MapWrap::X);
    assert_eq!(city.specialists, [0; 3]);
    assert_eq!(city.worked_tiles, vec![second]);
}

#[test]
fn founding_consumes_the_unit_and_releases_its_guard_and_task_state() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    let founder = game.spawn_unit(HUMAN, UnitKind::Settler, 10, 24);
    let guard = game.spawn_unit(HUMAN, UnitKind::Spearman, 10, 24);
    assert!(game.assign_ai_guard(guard, guards::GuardTarget::Unit(founder)));
    game.unit_goto_tiles.insert(founder, (10, 24));
    game.ai_worker_tasks.insert(founder);
    assert!(game.apply(Command::FoundCity(founder)));
    assert!(!game.unit_goto_tiles.contains_key(&founder));
    assert!(!game.ai_worker_tasks.contains(&founder));
    assert!(!game.ai_unit_guards.contains_key(&founder));
    assert!(!game.ai_guard_targets.contains_key(&guard));
    assert!(game.validate_ai_guards());
}

#[test]
fn desert_farmland_needs_a_food_bonus_and_supermarket_applies_to_worked_tiles() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = (10, 20);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.create_city(HUMAN, x, y);
    let (fx, fy) = offset_native_by_map(x, y, 1, 0);
    let tile = game.tile_mut(fx, fy);
    tile.terrain = Terrain::Desert;
    tile.irrigation = true;
    tile.river = false;
    tile.resource = None;
    game.players[HUMAN].known.extend([
        Tech::REFRIGERATION,
        Tech::ELECTRICITY,
        Tech::SANITATION,
        Tech::STEAM_POWER,
    ]);
    let worker = game.spawn_unit(HUMAN, UnitKind::Worker, fx, fy);
    assert!(!game.apply(Command::Work(worker, WorkKind::Farmland)));
    game.tile_mut(fx, fy).river = true;
    assert!(game.apply(Command::Work(worker, WorkKind::Farmland)));
    for _ in 0..5 {
        game.update_activities_and_restore();
    }
    assert!(game.tile(fx, fy).farmland);
    game.cities[0].worked_tiles = vec![(fx, fy)];
    let center_food = game.tile(x, y).yields().0 + game.tile(x, y).irrigation_food();
    let farm_food = game.tile(fx, fy).yields().0;
    assert_eq!(game.city_yields(&game.cities[0]).0, center_food + farm_food);
    game.cities[0].buildings.push(Building::SUPERMARKET);
    assert_eq!(
        game.city_yields(&game.cities[0]).0,
        center_food + center_food / 2 + farm_food + farm_food / 2
    );

    assert!(game.apply(Command::Work(worker, WorkKind::Mine)));
    for _ in 0..5 {
        game.update_activities_and_restore();
    }
    assert!(game.tile(fx, fy).mine);
    assert!(!game.tile(fx, fy).farmland);
    assert!(!game.tile(fx, fy).irrigation);
}

#[test]
fn production_uses_population_before_growth() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let id = game.cities[0].id;
    let target = Production::Unit(UnitKind::Settler);
    game.cities[0].size = 2;
    game.cities[0].food = Simulation::growth_box(2) + 20;
    game.cities[0].shields = target.cost();
    assert!(game.apply(Command::Produce(id, target)));
    game.process_cities();
    assert_eq!(game.cities[0].size, 3);
    assert!(
        !game
            .units
            .iter()
            .any(|unit| unit.kind == UnitKind::Settler && unit.home_city == Some(id))
    );

    game.process_cities();
    assert!(
        game.units
            .iter()
            .any(|unit| unit.kind == UnitKind::Settler && unit.home_city == Some(id))
    );
}

#[test]
fn completed_aqueduct_allows_growth_in_the_same_turn() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let (x, y) = offset_native_by_map(10, 20, 1, 0);
    game.tile_mut(x, y).terrain = Terrain::Lake;
    let id = game.cities[0].id;
    let target = Production::Building(Building::AQUEDUCT_LAKE);
    game.cities[0].size = 8;
    game.cities[0].food = Simulation::growth_box(8) + 20;
    assert!(game.apply(Command::Produce(id, target)));
    game.cities[0].shields = target.cost();
    game.process_cities();
    assert!(game.cities[0].has_building(Building::AQUEDUCT_LAKE));
    assert_eq!(game.cities[0].size, 9);
}

#[test]
fn sewer_system_allows_growth_past_size_sixteen() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.cities.clear();
    game.create_city(HUMAN, 10, 20);
    let city = &mut game.cities[0];
    city.buildings
        .extend([Building::AQUEDUCT, Building::SEWER_SYSTEM]);

    let mut growth_sizes = vec![city.size];
    for _ in 0..100 {
        let limit = Simulation::city_size_limit(city);
        let retained = Simulation::food_retained(city);
        if city.resolve_growth(game.map_wrap, 10, limit, retained, false) == Growth::Grew {
            growth_sizes.push(city.size);
        }
        if city.size == 17 {
            break;
        }
    }
    assert_eq!(growth_sizes, (1..=17).collect::<Vec<_>>());
    assert_eq!(city.size, 17);
}

#[test]
fn civ3_city_growth_limits_follow_aqueduct_and_hospital_thresholds() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.cities.clear();
    game.create_city(HUMAN, 10, 20);
    let city = &mut game.cities[0];
    assert_eq!(Simulation::city_size_limit(city), 6);
    city.buildings.push(Building::AQUEDUCT);
    assert_eq!(Simulation::city_size_limit(city), 12);
    city.buildings.push(Building::HOSPITAL);
    assert_eq!(Simulation::city_size_limit(city), i32::MAX);
}

#[test]
fn ordinary_food_surplus_grows_city_and_food_deficit_shrinks_it() {
    let mut growing = Simulation::new(crate::mapgen::generate(42));
    growing.units.clear();
    growing.cities.clear();
    for tile in &mut growing.tiles {
        tile.terrain = Terrain::Mountains;
        tile.resource = None;
        tile.river = false;
    }
    let (x, y) = (10, 20);
    growing.tile_mut(x, y).terrain = Terrain::Grassland;
    let food_tile = offset_native_by_map(x, y, 1, 0);
    growing.tile_mut(food_tile.0, food_tile.1).terrain = Terrain::Grassland;
    growing.create_city(HUMAN, x, y);
    let id = growing.cities[0].id;
    growing.cities[0].food = Simulation::growth_box(1) - 1;
    growing.process_cities();
    let grew = growing
        .cities
        .iter()
        .any(|city| city.id == id && city.size == 2);
    assert!(grew, "ordinary food surplus should grow a city");

    let mut shrinking = Simulation::new(crate::mapgen::generate(42));
    shrinking.units.clear();
    shrinking.cities.clear();
    for tile in &mut shrinking.tiles {
        tile.terrain = Terrain::Desert;
        tile.resource = None;
        tile.river = false;
    }
    shrinking.create_city(HUMAN, x, y);
    let id = shrinking.cities[0].id;
    shrinking.cities[0].size = 2;
    shrinking.cities[0].food = 0;
    shrinking.reconcile_city_workers();
    shrinking.process_cities();
    let shrank = shrinking
        .cities
        .iter()
        .any(|city| city.id == id && city.size == 1);
    assert!(shrank, "ordinary food deficit should shrink a city");
}

#[test]
fn overlapping_cities_reserve_worked_tiles_and_release_them_on_reassignment() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    let second = offset_native_by_map(10, 20, 3, 0);
    game.create_city(HUMAN, 10, 20);
    game.create_city(HUMAN, second.0, second.1);
    let shared = offset_native_by_map(10, 20, 1, 0);
    let first = game.cities[0].id;
    let second = game.cities[1].id;
    for id in [first, second] {
        assert!(game.apply(Command::Specialist(id, 2, 1)));
    }
    assert!(game.apply(Command::WorkCityTile(first, shared.0, shared.1)));
    assert!(!game.apply(Command::WorkCityTile(second, shared.0, shared.1)));
    assert!(game.apply(Command::WorkCityTile(first, shared.0, shared.1)));
    assert!(game.apply(Command::WorkCityTile(second, shared.0, shared.1)));

    game.spawn_unit(1, UnitKind::Warrior, shared.0, shared.1);
    game.reconcile_city_workers();
    assert!(!game.cities[1].worked_tiles.contains(&shared));
    assert_eq!(game.cities[1].worked_tiles.len(), 1);
    assert!(!game.cities[0].worked_tiles.contains(&shared));
}
