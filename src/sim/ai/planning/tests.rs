use super::*;
use crate::sim::{Command, HUMAN, Pact, UnitKind};
use crate::world::{Terrain, topology::offset_native_by_map};

#[test]
fn spatial_queries_refresh_outside_planning_and_between_players() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.base = None;
    }
    let point = (10, 24);
    let index = game.map_wrap.index(point.0, point.1);
    assert_eq!(game.continent_labels()[index], 0);
    assert!(
        game.planning
            .airfields(&game.tiles, game.map_wrap)
            .is_empty()
    );
    game.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
    game.tile_mut(point.0, point.1).base = Some(Base::Airstrip);
    assert_ne!(game.continent_labels()[index], 0);
    assert_eq!(
        &*game.planning.airfields(&game.tiles, game.map_wrap),
        &[point]
    );

    game.begin_ai_planning();
    let labels = game.continent_labels();
    assert!(Arc::ptr_eq(&labels, &game.continent_labels()));
    assert_eq!(
        &*game.planning.airfields(&game.tiles, game.map_wrap),
        &[point]
    );
    game.end_ai_planning();
    game.tile_mut(point.0, point.1).terrain = Terrain::Sea;
    game.tile_mut(point.0, point.1).base = None;
    game.begin_ai_planning();
    assert_eq!(game.continent_labels()[index], 0);
    assert!(
        game.planning
            .airfields(&game.tiles, game.map_wrap)
            .is_empty()
    );
    game.end_ai_planning();
}

#[test]
fn unit_planning_freezes_enemy_danger_but_recomputes_garrison_defense() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    game.sign_pact(1, HUMAN, Pact::War);
    let city = game.cities[0].clone();
    let near = offset_native_by_map(city.x, city.y, 1, 0);
    let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, near.0, near.1);
    let enemy_index = game.unit_index_by_id(enemy).unwrap();

    game.begin_ai_planning();
    assert!(game.advisor_guard_city_assessment(&city).danger > 0);
    game.units[enemy_index].kind = UnitKind::Worker;
    assert_eq!(game.advisor_guard_city_assessment(&city).danger, 0);
    game.units[enemy_index].kind = UnitKind::Warrior;
    game.planning.begin_units();
    let initial = game.advisor_guard_city_assessment(&city);
    assert!(initial.danger > 0);
    assert_eq!(initial.defense, 0);
    game.units[enemy_index].kind = UnitKind::Worker;
    game.spawn_unit(1, UnitKind::Warrior, city.x, city.y);
    let defended = game.advisor_guard_city_assessment(&city);
    assert_eq!(defended.danger, initial.danger);
    assert_eq!(defended.urgency, initial.urgency);
    assert!(defended.defense > 0);
    game.end_ai_planning();

    assert_eq!(game.advisor_guard_city_assessment(&city).danger, 0);
    game.begin_ai_planning();
    game.planning.begin_units();
    assert_eq!(game.advisor_guard_city_assessment(&city).danger, 0);
    game.end_ai_planning();
}

#[test]
fn planning_cache_population_and_clone_cleanup_preserve_campaign_and_future_turns() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let founder = game
        .units
        .iter()
        .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
        .unwrap()
        .id;
    assert!(game.apply(Command::FoundCity(founder)));
    game.enable_all_civilizations_as_ai();
    game.sync_list_order();
    let saved = game.encode_campaign().unwrap();
    let memory = bincode::serialize(&game.observed_map).unwrap();

    game.begin_ai_planning();
    let labels = game.continent_labels();
    let airfields = game
        .planning
        .airfields(&game.tiles, game.map_wrap)
        .into_owned();
    game.planning.begin_units();
    for city in &game.cities {
        game.advisor_guard_city_assessment(city);
    }
    assert_eq!(game.encode_campaign().unwrap(), saved);
    let mut clone = game.clone();
    clone.end_ai_planning();
    assert!(Arc::ptr_eq(&labels, &game.continent_labels()));
    assert_eq!(
        &*game.planning.airfields(&game.tiles, game.map_wrap),
        airfields
    );
    assert_eq!(bincode::serialize(&game.observed_map).unwrap(), memory);
    game.end_ai_planning();
    let path = std::env::temp_dir().join(format!(
        "civcraft-planning-cache-{}.save",
        std::process::id()
    ));
    game.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(loaded.encode_campaign().unwrap(), saved);
    for _ in 0..2 {
        game.simulate_turn();
        clone.simulate_turn();
        loaded.simulate_turn();
        assert_eq!(
            game.encode_campaign().unwrap(),
            clone.encode_campaign().unwrap()
        );
        assert_eq!(
            game.encode_campaign().unwrap(),
            loaded.encode_campaign().unwrap()
        );
    }
}
