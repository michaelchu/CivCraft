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
    game.create_city(1, 20, 20);
    game.trade_routes.push(trade_routes::TradeRoute {
        from_city_id: game.cities[0].id,
        to_city_id: game.cities[1].id,
        goods_rule_name: "Goods".to_owned(),
    });
    game.sign_pact(HUMAN, 1, Pact::Peace);
    let city_ids = [game.cities[0].id, game.cities[1].id];
    let pending_terms = diplomacy::Concessions::default()
        .set_city(Some(city_ids[0]))
        .set_city(Some(city_ids[1]))
        .set_map(Some(TreatyMap::World))
        .set_map(Some(TreatyMap::Sea));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(TreatyOffer {
        terms: [pending_terms, diplomacy::Concessions::default()],
        ..TreatyOffer::default()
    });
    game.cities[0].size = 2;
    let veteran_unit = game.spawn_unit(HUMAN, UnitKind::Warrior, 11, 20);
    game.units
        .iter_mut()
        .find(|unit| unit.id == veteran_unit)
        .unwrap()
        .c2c3_veteran = 2;
    game.units
        .iter_mut()
        .find(|unit| unit.id == veteran_unit)
        .unwrap()
        .paradropped = true;
    game.cities[0].history = 375;
    game.players[HUMAN].history = 125;
    game.cities[0].foreign_citizens = vec![(2, 1)];
    game.players[HUMAN].mobilized = true;
    game.players[HUMAN].government = Government::Fundamentalism;
    game.players[HUMAN].revolution_target = Some(Government::Federation);
    game.players[HUMAN].revolution_turn = Some(game.turn + 2);
    game.players[HUMAN].researching_saved = Some(Tech::MONARCHY);
    game.players[HUMAN].research_saved = 53;
    game.government_adoptions[Government::Federation as usize] = 3;
    let mut government_scores = [0.0; 11];
    government_scores[Government::Federation as usize] = 17.5;
    game.ai_government_cache.insert(
        1,
        government::GovernmentAdvisorCache {
            evaluated_turn: game.turn,
            next_evaluation: game.turn + 5,
            scores: government_scores,
        },
    );
    game.global_advances = vec![Tech::SPACE_FLIGHT, Tech::SUPERCONDUCTORS];
    game.year = 1;
    game.year_0_hack = true;
    game.set_random_seed(42);
    game.rand_below(1000);
    game.validate_campaign().unwrap();
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(loaded.map_wrap, MapWrap::X);
    assert_eq!(loaded.map_seed, game.map_seed);
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    assert_eq!(loaded.turn, game.turn);
    assert_eq!(loaded.unit_by_id(veteran_unit).unwrap().c2c3_veteran, 2);
    assert!(loaded.unit_by_id(veteran_unit).unwrap().paradropped);
    assert_eq!(loaded.global_advances, game.global_advances);
    assert_eq!(loaded.year, game.year);
    assert_eq!(loaded.year_0_hack, game.year_0_hack);
    assert!(loaded.players[HUMAN].mobilized);
    assert_eq!(loaded.players[HUMAN].government, Government::Fundamentalism);
    assert_eq!(
        loaded.players[HUMAN].revolution_target,
        Some(Government::Federation)
    );
    assert_eq!(loaded.players[HUMAN].revolution_turn, Some(game.turn + 2));
    assert_eq!(
        loaded.players[HUMAN].researching_saved,
        Some(Tech::MONARCHY)
    );
    assert_eq!(loaded.players[HUMAN].research_saved, 53);
    assert_eq!(
        loaded.government_adoptions[Government::Federation as usize],
        3
    );
    assert_eq!(
        loaded.ai_government_cache[&1].scores[Government::Federation as usize],
        17.5
    );
    assert_eq!(loaded.cities[0].history, 375);
    assert_eq!(loaded.players[HUMAN].history, 125);
    assert_eq!(loaded.cities[0].foreign_citizens, [(2, 1)]);
    assert_eq!(loaded.trade_routes, game.trade_routes);
    let loaded_offer = loaded.diplomacy[&(HUMAN, 1)].offer.as_ref().unwrap();
    assert_eq!(loaded_offer.terms[0].cities(), &city_ids);
    assert_eq!(
        loaded_offer.terms[0].maps().collect::<Vec<_>>(),
        [TreatyMap::World, TreatyMap::Sea]
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
fn captured_city_history_and_nationalities_survive_save_load() {
    let path = temporary_save("captured-history-nationalities");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    game.create_city(HUMAN, 30, 24);
    let captured_city_id = game.cities[1].id;
    game.cities[1].size = 6;
    game.cities[1].history = 300;
    game.cities[2].history = 375;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
    game.capture_city_if_needed(attacker, 20, 24);

    let captured_city = game.city_index_by_id(captured_city_id).unwrap();
    assert_eq!(game.cities[captured_city].owner, HUMAN);
    assert_eq!(game.cities[captured_city].history, 300);
    assert_eq!(game.cities[captured_city].foreign_citizens, [(1, 5)]);
    game.reconcile_city_workers();
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    let loaded_city = loaded.city_index_by_id(captured_city_id).unwrap();
    assert_eq!(loaded.cities[loaded_city].owner, HUMAN);
    assert_eq!(loaded.cities[loaded_city].history, 300);
    assert_eq!(loaded.cities[loaded_city].foreign_citizens, [(1, 5)]);
    assert_eq!(loaded.cities[2].history, 375);
    assert_eq!(loaded.civilization_culture(1), 0);
    assert_eq!(loaded.civilization_culture(HUMAN), 675);
    assert_eq!(
        bincode::serialize(&loaded.cities).unwrap(),
        bincode::serialize(&game.cities).unwrap()
    );
}

#[test]
fn player_history_survives_city_destruction_and_save_load() {
    let path = temporary_save("national-history");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].history = 300;
    game.cities.remove(0);
    game.rebuild_city_ownership();
    assert_eq!(game.civilization_culture(HUMAN), 300);
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert!(loaded.cities.is_empty());
    assert_eq!(loaded.civilization_culture(HUMAN), 300);
}

#[test]
fn old_save_headers_and_corrupt_current_payloads_are_rejected() {
    let path = temporary_save("save-invalid");
    for header in [
        b"CIVCRAFT\0\x18".as_slice(),
        b"CIVCRAFT\0\x19".as_slice(),
        b"CIVCRAFT\0\x1A".as_slice(),
        b"CIVCRAFT\0\x1B".as_slice(),
        b"CIVCRAFT\0\x1C".as_slice(),
        b"CIVCRAFT\0\x1D".as_slice(),
        b"CIVCRAFT\0\x1E".as_slice(),
        b"CIVCRAFT\0\x1F".as_slice(),
        b"CIVCRAFT\0\x20".as_slice(),
    ] {
        fs::write(&path, [header, b"previous payload"].concat()).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
    }

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
    game.cities[0].history = -1;
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());

    game.cities[0].history = 0;
    game.trade_routes.push(trade_routes::TradeRoute {
        from_city_id: game.cities[0].id,
        to_city_id: game.cities[0].id + 1,
        goods_rule_name: "Goods".to_owned(),
    });
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());

    game.trade_routes.clear();
    game.create_city(1, 20, 20);
    game.trade_routes.push(trade_routes::TradeRoute {
        from_city_id: game.cities[0].id,
        to_city_id: game.cities[1].id,
        goods_rule_name: "Unknown_Good".to_owned(),
    });
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    fs::remove_file(path).unwrap();
}
