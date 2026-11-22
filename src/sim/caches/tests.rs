use crate::rules::Tech;
use crate::sim::movement_rules::MovementTile;
use crate::sim::{Command, HUMAN, Simulation, UnitKind};

#[test]
fn entity_positions_revalidate_after_reordering_removal_and_insertion() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let first = game.units[0].id;
    let second = game.units[1].id;
    assert_eq!(game.unit_index_by_id(first), Some(0));
    assert_eq!(game.unit_index_by_id(second), Some(1));
    game.units.swap(0, 1);
    assert_eq!(game.unit_index_by_id(first), Some(1));
    assert_eq!(game.unit_index_by_id(second), Some(0));
    game.units.remove(0);
    assert!(game.unit_by_id(second).is_none());
    assert_eq!(game.unit_index_by_id(first), Some(0));
    let future = game.next_unit_id;
    assert!(game.unit_by_id(future).is_none());
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
    assert_eq!(id, future);
    assert_eq!(game.unit_by_id(future).unwrap().kind, UnitKind::Warrior);

    game.create_city(HUMAN, 10, 24);
    game.create_city(1, 22, 24);
    let first = game.cities[0].id;
    let second = game.cities[1].id;
    assert_eq!(game.city_index_by_id(first), Some(0));
    assert_eq!(game.city_index_by_id(second), Some(1));
    game.cities.swap(0, 1);
    assert_eq!(game.city_index_by_id(first), Some(1));
    game.cities.remove(1);
    assert!(game.city_by_id(first).is_none());
    assert_eq!(game.city_index_by_id(second), Some(0));
    let future = game.next_city_id;
    assert!(game.city_by_id(future).is_none());
    game.create_city(HUMAN, 10, 24);
    assert_eq!(game.city_by_id(future).unwrap().owner, HUMAN);
}

#[test]
fn movement_facts_follow_changed_descriptors_without_a_global_flush() {
    use crate::world::Terrain;
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let land = UnitKind::Warrior.unit_class();
    let sea = UnitKind::Galley.unit_class();
    let grass = MovementTile::new(Terrain::Grassland, false, false, false, None);
    let water = MovementTile::new(Terrain::Sea, false, false, false, None);
    assert!(game.cached_movement_native_to_class(grass, land));
    assert!(!game.cached_movement_native_to_class(water, land));
    assert!(game.cached_movement_native_to_class(water, sea));
    assert!(!game.cached_movement_native_to_class(grass, sea));
    let start = (10, 24);
    let target = game
        .map_wrap
        .eight_way_neighbor(start.0, start.1, 2)
        .unwrap();
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
    for (x, y) in [start, target] {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.base = None;
    }
    let flat_cost = game.raw_move_cost_for(game.unit_by_id(id).unwrap(), target.0, target.1);
    game.tile_mut(target.0, target.1).terrain = Terrain::Hills;
    let hill_cost = game.raw_move_cost_for(game.unit_by_id(id).unwrap(), target.0, target.1);
    assert!(hill_cost > flat_cost);
    game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
    assert_eq!(
        game.raw_move_cost_for(game.unit_by_id(id).unwrap(), target.0, target.1),
        flat_cost
    );
}

#[derive(Debug, PartialEq, Eq)]
struct QueryResults {
    unit_rows: Vec<Option<usize>>,
    city_rows: Vec<Option<usize>>,
    era: crate::rules::TechEra,
    path: Vec<Tech>,
    land_native: bool,
    land_cost: i32,
}

fn prime_queries(game: &Simulation) -> QueryResults {
    let units = game
        .units
        .iter()
        .map(|unit| game.unit_index_by_id(unit.id))
        .collect();
    let cities = game
        .cities
        .iter()
        .map(|city| game.city_index_by_id(city.id))
        .collect();
    let era = game.research_era(HUMAN);
    let path = game.shared_research_path(HUMAN, Tech::FEUDALISM).to_vec();
    let class = UnitKind::Warrior.unit_class();
    let grass = MovementTile::new(crate::world::Terrain::Grassland, false, false, false, None);
    let native = game.cached_movement_native_to_class(grass, class);
    let unit = &game.units[0];
    let target = game.map_wrap.eight_way_neighbor(unit.x, unit.y, 2).unwrap();
    let cost = game.raw_move_cost_for(unit, target.0, target.1);
    QueryResults {
        unit_rows: units,
        city_rows: cities,
        era,
        path,
        land_native: native,
        land_cost: cost,
    }
}

#[test]
fn warming_clearing_and_loading_caches_preserve_saves_memory_and_future_turns() {
    let mut warm = Simulation::new(crate::test_support::seeded_map());
    let founder = warm
        .units
        .iter()
        .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
        .unwrap()
        .id;
    assert!(warm.apply(Command::FoundCity(founder)));
    warm.enable_all_civilizations_as_ai();
    warm.refresh_visibility();
    assert!(
        !warm.observed_map.tiles.is_empty(),
        "the test must include saved map knowledge"
    );
    let memory = bincode::serialize(&warm.observed_map).unwrap();
    let saved = warm.encode_campaign().unwrap();
    let expected = prime_queries(&warm);
    assert_eq!(warm.encode_campaign().unwrap(), saved);

    let mut cold = warm.clone();
    cold.caches.clear();
    assert_eq!(cold.encode_campaign().unwrap(), saved);
    assert_eq!(bincode::serialize(&cold.observed_map).unwrap(), memory);
    assert_eq!(prime_queries(&cold), expected);
    cold.caches.clear();

    let path =
        std::env::temp_dir().join(format!("civcraft-query-cache-{}.save", std::process::id()));
    warm.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(bincode::serialize(&loaded.observed_map).unwrap(), memory);
    assert_eq!(loaded.encode_campaign().unwrap(), saved);
    assert_eq!(prime_queries(&loaded), expected);
    for _ in 0..2 {
        warm.simulate_turn();
        cold.simulate_turn();
        loaded.simulate_turn();
        assert_eq!(
            warm.encode_campaign().unwrap(),
            cold.encode_campaign().unwrap()
        );
        assert_eq!(
            warm.encode_campaign().unwrap(),
            loaded.encode_campaign().unwrap()
        );
    }
}
