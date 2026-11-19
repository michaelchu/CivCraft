use super::*;
use crate::sim::{Command, HUMAN, Pact, UnitKind};
use crate::world::{Terrain, topology::offset_native_by_map};

fn threatened_city() -> (Simulation, u32, u32) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.hut = false;
        tile.road = false;
        tile.railroad = false;
        tile.base = None;
    }
    game.create_city(1, 10, 24);
    game.sign_pact(1, HUMAN, Pact::War);
    let near = offset_native_by_map(10, 24, 1, 0);
    let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, near.0, near.1);
    game.sync_list_order();
    let city = game.cities[0].id;
    (game, city, enemy)
}

#[test]
fn reinforcement_keeps_hostile_routes_but_refreshes_friendly_defense() {
    let (mut game, city_id, enemy_id) = threatened_city();
    let city = game.city_by_id(city_id).unwrap().clone();
    let enemy = game.unit_by_id(enemy_id).unwrap().clone();
    let uncached = game.city_danger_arrival(&enemy, &city);
    assert!(uncached.is_some());
    game.begin_ai_advisor_cache();
    let assessment = game.advisor_guard_city_assessment(&city);
    assert!(assessment.danger > 0);
    assert_eq!(assessment.defense, 0);
    let sizes = game.advisor_cache.threats().sizes();
    assert!(sizes.0 > 0 && sizes.1 > 0 && sizes.2 > 0);

    game.end_ai_advisor_cache_preserving_threat_routes();
    assert!(!game.advisor_cache.active());
    assert!(game.advisor_cache.threats().enabled());
    assert!(
        game.advisor_cache
            .memos()
            .guard_assessment(city_id)
            .is_none()
    );
    game.spawn_unit(1, UnitKind::Warrior, city.x, city.y);
    assert!(game.advisor_guard_city_assessment(&city).defense > 0);
    assert_eq!(game.city_danger_arrival(&enemy, &city), uncached);
    assert_eq!(game.advisor_cache.threats().sizes(), sizes);

    game.clear_ai_threat_route_cache();
    assert!(!game.advisor_cache.threats().enabled());
    assert_eq!(game.advisor_cache.threats().sizes(), (0, 0, 0));
    game.begin_ai_advisor_cache();
    assert_eq!(game.city_danger_arrival(&enemy, &city), uncached);
    game.end_ai_advisor_cache();
    assert!(!game.advisor_cache.active());
    assert_eq!(game.advisor_cache.threats().sizes(), (0, 0, 0));
}

#[test]
fn ordered_snapshot_falls_back_after_a_unit_arrives_and_founder_maps_refresh() {
    let (mut game, city_id, _) = threatened_city();
    let city = game.city_by_id(city_id).unwrap().clone();
    game.begin_ai_advisor_cache();
    assert_eq!(game.tile_units_ordered(city.x, city.y).count(), 0);
    let defender = game.spawn_unit(1, UnitKind::Warrior, city.x, city.y);
    assert_eq!(
        game.tile_units_ordered(city.x, city.y).next().unwrap().id,
        defender
    );
    game.end_ai_advisor_cache();

    game.founder_phase = true;
    let site = (30, 24);
    let index = game.map_wrap.index(site.0, site.1);
    assert_eq!(game.settlement_citymap(1, None)[index], 0);
    game.create_city(1, site.0, site.1);
    game.clear_founder_phase_maps();
    assert_eq!(game.settlement_citymap(1, None)[index], -1);
    game.founder_phase = false;
    game.clear_founder_phase_maps();
}

#[test]
fn active_advisor_caches_clone_independently_and_leave_saves_and_future_turns_unchanged() {
    let mut warm = Simulation::new(crate::test_support::seeded_map());
    let founder = warm
        .units
        .iter()
        .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settler)
        .unwrap()
        .id;
    assert!(warm.apply(Command::FoundCity(founder)));
    warm.enable_all_civilizations_as_ai();
    warm.sync_list_order();
    let saved = warm.encode_campaign().unwrap();
    let memory = bincode::serialize(&warm.observed_map).unwrap();
    warm.begin_ai_advisor_cache();
    let city = warm.cities[0].clone();
    warm.advisor_guard_city_assessment(&city);
    let labels = warm.advisor_continent_labels();
    assert_eq!(warm.encode_campaign().unwrap(), saved);
    let mut cold = warm.clone();
    cold.end_ai_advisor_cache();
    assert!(warm.advisor_cache.active());
    assert!(Arc::ptr_eq(&labels, &warm.advisor_continent_labels()));
    assert!(
        warm.advisor_cache
            .memos()
            .guard_assessment(city.id)
            .is_some()
    );
    assert_eq!(cold.encode_campaign().unwrap(), saved);
    assert_eq!(bincode::serialize(&cold.observed_map).unwrap(), memory);
    warm.end_ai_advisor_cache();
    let path = std::env::temp_dir().join(format!(
        "civcraft-advisor-cache-{}.save",
        std::process::id()
    ));
    warm.save_campaign(&path).unwrap();
    let mut loaded = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!loaded.advisor_cache.active());
    assert_eq!(loaded.advisor_cache.threats().sizes(), (0, 0, 0));
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
