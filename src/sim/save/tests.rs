//! Regression tests for the current campaign save format.

use super::*;

fn temporary_save(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "civcraft-{name}-{}-{}.save",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn current_campaign_round_trip_preserves_gameplay_and_topology_state() {
    let path = temporary_save("save-round-trip");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    game.cities[0].add_culture(HUMAN, 375);
    game.cities[0].add_culture(2, 125);
    game.cities[0].record_culture_building_completion(Building::TEMPLE, HUMAN, -4000);
    game.cities[0].resisting_citizens = vec![(2, 1)];
    game.cities[0].foreign_citizens = vec![(2, 1)];
    game.cities[0].foreign_nationality_tenure = vec![(2, 7, 3)];
    game.cities[0].culture_suppressed_until_majority_assimilated = true;
    game.set_random_seed(42);
    game.rand_below(1000);
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(loaded.map_wrap, MapWrap::X);
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    assert_eq!(loaded.turn, game.turn);
    assert_eq!(loaded.cities[0].culture_for_civilization(HUMAN), 375);
    assert_eq!(loaded.cities[0].culture_for_civilization(2), 125);
    assert_eq!(loaded.cities[0].resisting_citizens, [(2, 1)]);
    assert_eq!(loaded.cities[0].foreign_nationality_tenure, [(2, 7, 3)]);
    assert!(loaded.cities[0].culture_suppressed_until_majority_assimilated);
    assert_eq!(
        loaded.cities[0].culture_building_years,
        [(Building::PALACE.0, -4000), (Building::TEMPLE.0, -4000)]
    );
    assert_eq!(
        loaded.cities[0].culture_building_owners,
        [(Building::PALACE.0, HUMAN), (Building::TEMPLE.0, HUMAN)]
    );
    assert_eq!(
        bincode::serialize(&loaded.cities).unwrap(),
        bincode::serialize(&game.cities).unwrap()
    );
    assert_eq!(
        bincode::serialize(&loaded.tiles).unwrap(),
        bincode::serialize(&game.tiles).unwrap()
    );
}

#[test]
fn captured_city_culture_and_resistance_survive_save_load() {
    let path = temporary_save("captured-culture-resistance");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    game.create_city(HUMAN, 30, 24);
    let captured_city_id = game.cities[1].id;
    game.cities[1].size = 6;
    game.cities[1].add_culture(1, 300);
    game.cities[2].add_culture(HUMAN, 375);
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
    game.capture_city_if_needed(attacker, 20, 24);

    let captured_city = game.city_index_by_id(captured_city_id).unwrap();
    assert_eq!(game.cities[captured_city].owner, HUMAN);
    assert_eq!(
        game.cities[captured_city].culture_by_civilization,
        [(HUMAN, 0)]
    );
    assert_eq!(game.cities[captured_city].foreign_citizens, [(1, 5)]);
    game.advance_foreign_nationality_tenure(captured_city);
    assert_eq!(
        game.cities[captured_city].foreign_nationality_tenure,
        [(1, 0, 1)]
    );
    game.cities[captured_city].resisting_citizens = vec![(1, 2)];
    game.reconcile_city_workers();
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    let loaded_city = loaded.city_index_by_id(captured_city_id).unwrap();
    assert_eq!(loaded.cities[loaded_city].owner, HUMAN);
    assert_eq!(
        loaded.cities[loaded_city].culture_by_civilization,
        [(HUMAN, 0)]
    );
    assert_eq!(loaded.cities[loaded_city].foreign_citizens, [(1, 5)]);
    assert_eq!(
        loaded.cities[loaded_city].foreign_nationality_tenure,
        [(1, 0, 1)]
    );
    assert_eq!(loaded.cities[loaded_city].resisting_citizens, [(1, 2)]);
    assert_eq!(loaded.cities[2].culture_for_civilization(HUMAN), 375);
    assert_eq!(
        bincode::serialize(&loaded.cities).unwrap(),
        bincode::serialize(&game.cities).unwrap()
    );
}

#[test]
fn old_save_headers_and_corrupt_current_payloads_are_rejected() {
    let path = temporary_save("save-invalid");
    fs::write(&path, b"CIVCRAFT\0\x11previous payload").unwrap();
    assert!(Simulation::load_campaign(&path).is_err());

    fs::write(&path, [HEADER, b"invalid payload"].concat()).unwrap();
    let error = match Simulation::load_campaign(&path) {
        Ok(_) => panic!("corrupt current payload was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    fs::remove_file(path).unwrap();
}

#[test]
fn invalid_campaign_state_is_rejected_on_load() {
    let path = temporary_save("save-validation");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    game.climate_current = [game.map_wrap.tile_count() as i32 + 1, 0];
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());

    game.climate_current = [0, 0];
    game.cities[0].resisting_citizens = vec![(2, game.cities[0].size + 1)];
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    fs::remove_file(path).unwrap();
}
