use super::*;

pub(in crate::sim) fn temporary_save(label: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "civcraft-{label}-{}-{}.save",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[test]
fn large_explored_campaigns_save_reload_and_advance() {
    for dimension in [256, 512] {
        let wrap = MapWrap::with_dimensions(dimension, dimension).unwrap();
        let mut game = Simulation::new(crate::mapgen::generate_with_wrap(42, wrap));
        for tile in &mut game.tiles {
            tile.seen = true;
            tile.known_by.set(HUMAN, true);
            tile.known_by.set(PLAYER_SLOTS - 1, true);
        }
        game.remember_render_map(&[vec![true; wrap.tile_count()]]);
        let path = temporary_save("large-map");
        game.save_campaign(&path).unwrap();
        drop(game);
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(loaded.map_wrap, wrap);
        assert!(
            loaded.tiles.iter().all(|tile| {
                tile.seen && tile.known_by[HUMAN] && tile.known_by[PLAYER_SLOTS - 1]
            })
        );
        assert!((0..wrap.tile_count()).all(|index| {
            let (x, y) = wrap.position(index);
            loaded.observed_route_tile(x, y).is_some()
        }));
        let turn = loaded.turn;
        loaded.simulate_turn();
        assert_eq!(loaded.turn, turn + 1);
    }
}

#[test]
fn current_campaign_round_trip_preserves_gameplay_and_topology_state() {
    let path = temporary_save("save-round-trip");
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
        .veteran_level = 2;
    game.units
        .iter_mut()
        .find(|unit| unit.id == veteran_unit)
        .unwrap()
        .paradropped = true;
    game.units
        .iter_mut()
        .find(|unit| unit.id == veteran_unit)
        .unwrap()
        .order = Some(UnitOrder::Bombard { x: 13, y: 20 });
    let air_superiority_unit = game.spawn_unit(HUMAN, UnitKind::Fighter, 12, 20);
    let fighter = game.unit_by_id(air_superiority_unit).unwrap();
    let fighter_position = (fighter.x, fighter.y);
    let fighter = game
        .units
        .iter_mut()
        .find(|unit| unit.id == air_superiority_unit)
        .unwrap();
    fighter.order = Some(UnitOrder::AirSuperiority {
        x: fighter_position.0,
        y: fighter_position.1,
    });
    fighter.air_superiority_shot = true;
    let precision_unit = game.spawn_unit(HUMAN, UnitKind::StealthBomber, 14, 20);
    game.units
        .iter_mut()
        .find(|unit| unit.id == precision_unit)
        .unwrap()
        .order = Some(UnitOrder::PrecisionBombard { x: 15, y: 20 });
    game.cities[0].history = 375;
    game.rebuild_city_ownership();
    game.players[HUMAN].history = 125;
    let culture_without_achievement = game.civilization_culture(HUMAN);
    game.nation_scores[HUMAN]
        .achievements
        .insert("Spaceship Completed".to_owned());
    for index in 1..40 {
        game.nation_scores[HUMAN]
            .achievements
            .insert(format!("test-achievement-{index}"));
    }
    game.players[HUMAN].history += 5000;
    assert_eq!(
        game.civilization_culture(HUMAN),
        culture_without_achievement + 5000
    );
    game.refresh_score_snapshot();
    game.nation_scores[HUMAN].score_literacy = 67;
    game.cities[0].foreign_citizens = vec![(2, 1)];
    game.players[HUMAN].mobilized = true;
    game.players[HUMAN].government = Government::DEMOCRACY;
    game.players[HUMAN].revolution_target = Some(Government::REPUBLIC);
    game.players[HUMAN].revolution_turn = Some(game.turn + 2);
    game.players[HUMAN].researching_saved = Some(Tech::MONARCHY);
    game.players[HUMAN].research_saved = 53;
    game.government_adoptions[Government::REPUBLIC.index()] = 3;
    let mut government_scores = vec![0.0; Government::count()];
    government_scores[Government::REPUBLIC.index()] = 17.5;
    game.ai_government_cache.insert(
        1,
        government::GovernmentAdvisorCache {
            evaluated_turn: game.turn,
            next_evaluation: game.turn + 5,
            scores: government_scores,
        },
    );
    game.global_advances = vec![Tech::SPACE_FLIGHT, Tech::SUPERCONDUCTOR];
    game.year = 1;
    game.year_0_hack = true;
    game.set_random_seed(42);
    game.rand_below(1000);
    game.notifications[0].read = true;
    game.add_contextual_notification(
        "A city was captured.",
        notifications::NotificationCategory::City,
        notifications::NotificationPriority::Important,
        Some(game.cities[0].id),
        Some(1),
    );
    game.validate_campaign().unwrap();
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(loaded.map_wrap, MapWrap::X);
    assert_eq!(loaded.map_seed, game.map_seed);
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    assert_eq!(loaded.turn, game.turn);
    assert_eq!(loaded.unit_by_id(veteran_unit).unwrap().veteran_level, 2);
    assert!(loaded.unit_by_id(veteran_unit).unwrap().paradropped);
    assert_eq!(
        loaded.unit_by_id(veteran_unit).unwrap().order,
        Some(UnitOrder::Bombard { x: 13, y: 20 })
    );
    assert_eq!(
        loaded.unit_by_id(air_superiority_unit).unwrap().order,
        Some(UnitOrder::AirSuperiority { x: 12, y: 20 })
    );
    assert_eq!(
        loaded.unit_by_id(precision_unit).unwrap().order,
        Some(UnitOrder::PrecisionBombard { x: 15, y: 20 })
    );
    assert!(
        loaded
            .unit_by_id(air_superiority_unit)
            .unwrap()
            .air_superiority_shot
    );
    assert_eq!(loaded.global_advances, game.global_advances);
    assert_eq!(loaded.year, game.year);
    assert_eq!(loaded.year_0_hack, game.year_0_hack);
    assert_eq!(loaded.notifications, game.notifications);
    assert_eq!(loaded.next_notification_id, game.next_notification_id);
    assert_eq!(
        loaded.unread_notifications_view(),
        game.unread_notifications_view()
    );
    assert!(loaded.players[HUMAN].mobilized);
    assert_eq!(loaded.players[HUMAN].government, Government::DEMOCRACY);
    assert_eq!(
        loaded.players[HUMAN].revolution_target,
        Some(Government::REPUBLIC)
    );
    assert_eq!(loaded.players[HUMAN].revolution_turn, Some(game.turn + 2));
    assert_eq!(
        loaded.players[HUMAN].researching_saved,
        Some(Tech::MONARCHY)
    );
    assert_eq!(loaded.players[HUMAN].research_saved, 53);
    assert_eq!(loaded.government_adoptions[Government::REPUBLIC.index()], 3);
    assert_eq!(
        loaded.ai_government_cache[&1].scores[Government::REPUBLIC.index()],
        17.5
    );
    assert_eq!(loaded.cities[0].history, 375);
    assert_eq!(loaded.players[HUMAN].history, 5125);
    assert_eq!(
        loaded.nation_scores[HUMAN].achievements,
        game.nation_scores[HUMAN].achievements
    );
    assert_eq!(loaded.nation_scores[HUMAN].achievements.len(), 40);
    assert!(
        loaded.nation_scores[HUMAN]
            .achievements
            .contains("Spaceship Completed")
    );
    assert_eq!(loaded.nation_scores[HUMAN].score_literacy, 67);
    assert_eq!(loaded.cities[0].foreign_citizens, vec![(2, 1)]);
    assert_eq!(loaded.trade_routes, game.trade_routes);
    assert_eq!(
        loaded
            .diplomacy
            .get(&(HUMAN, 1))
            .and_then(|relation| relation.offer.as_ref())
            .map(|offer| offer.terms[0].cities().to_vec()),
        Some(city_ids.to_vec())
    );
    assert_eq!(
        loaded
            .diplomacy
            .get(&(HUMAN, 1))
            .and_then(|relation| relation.offer.as_ref())
            .map(|offer| offer.terms[0].maps().collect::<Vec<_>>()),
        Some(vec![TreatyMap::World, TreatyMap::Sea])
    );
    assert_eq!(
        loaded.encode_campaign().unwrap(),
        game.encode_campaign().unwrap()
    );
}

#[test]
fn captured_city_history_and_nationalities_survive_save_load() {
    let path = temporary_save("captured-history-nationalities");
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    assert_eq!(game.cities[captured_city].history, 0);
    assert_eq!(game.cities[captured_city].foreign_citizens, [(1, 5)]);
    game.reconcile_city_workers();
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    let loaded_city = loaded.city_index_by_id(captured_city_id).unwrap();
    assert_eq!(loaded.cities[loaded_city].owner, HUMAN);
    assert_eq!(loaded.cities[loaded_city].history, 0);
    assert_eq!(loaded.cities[loaded_city].foreign_citizens, [(1, 5)]);
    assert_eq!(loaded.cities[2].history, 375);
    assert_eq!(loaded.civilization_culture(1), 300);
    assert_eq!(loaded.civilization_culture(HUMAN), 375);
    assert_eq!(
        bincode::serialize(&loaded.cities).unwrap(),
        bincode::serialize(&game.cities).unwrap()
    );
}

#[test]
fn player_history_survives_city_destruction_and_save_load() {
    let path = temporary_save("national-history");
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
        b"CIVCRAFT\0\x21".as_slice(),
        b"CIVCRAFT\0\x26".as_slice(),
        b"CIVCRAFT\0\x30".as_slice(),
        b"CIVCRAFT\0\x32".as_slice(),
        // Before Unit gained the Scientific Leader flag, which shifts the
        // positional layout of every saved unit.
        b"CIVCRAFT\0\x33".as_slice(),
    ] {
        fs::write(&path, [header, b"previous payload"].concat()).unwrap();
        let error = match Simulation::load_campaign(&path) {
            Ok(_) => panic!("stale header unexpectedly loaded"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("Unsupported CivCraft save format")
        );
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
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

#[test]
fn civilization_identity_and_unit_access_survive_renaming_and_save_reload() {
    let path = temporary_save("civilization-id");
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.players[HUMAN].known = Tech::all()
        .filter(|tech| tech.is_researchable_advance())
        .collect();
    let leader = game.players[HUMAN].leader_name();
    assert_eq!(game.players[HUMAN].civilization_id.as_deref(), Some("rome"));
    assert!(game.unit_available(HUMAN, UnitKind::Worker));
    assert!(game.unit_available(HUMAN, UnitKind::Legionary));
    game.players[HUMAN].name = "Inca".into();
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(
        loaded.players[HUMAN].civilization_id.as_deref(),
        Some("rome")
    );
    assert_eq!(loaded.players[HUMAN].leader_name(), leader);
    assert!(loaded.unit_available(HUMAN, UnitKind::Worker));
    assert!(loaded.unit_available(HUMAN, UnitKind::Legionary));
}

#[test]
fn scientific_leader_flag_round_trips_through_save() {
    let path = temporary_save("scientific-leader");
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    let scientific = game.spawn_unit(HUMAN, UnitKind::Leader, 10, 24);
    let military = game.spawn_unit(HUMAN, UnitKind::Leader, 12, 24);
    game.units
        .iter_mut()
        .find(|unit| unit.id == scientific)
        .unwrap()
        .scientific = true;
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert!(loaded.unit_by_id(scientific).unwrap().scientific);
    assert!(!loaded.unit_by_id(military).unwrap().scientific);
}

#[test]
fn created_leader_round_trips_through_save() {
    let path = temporary_save("created-leader");
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    let marked = game.spawn_unit(HUMAN, UnitKind::Knight, 10, 24);
    let unmarked = game.spawn_unit(HUMAN, UnitKind::Knight, 12, 24);
    game.units
        .iter_mut()
        .find(|unit| unit.id == marked)
        .unwrap()
        .created_leader = true;
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert!(loaded.unit_by_id(marked).unwrap().created_leader);
    assert!(!loaded.unit_by_id(unmarked).unwrap().created_leader);
}

#[test]
fn multiple_pending_technology_clauses_round_trip() {
    let path = temporary_save("multi-tech");
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let terms = diplomacy::Concessions::default()
        .add_technology(Tech::ALPHABET)
        .add_technology(Tech::POTTERY)
        .add_technology(Tech::MONARCHY);
    game.diplomacy.entry((HUMAN, 1)).or_default().offer = Some(TreatyOffer {
        terms: [terms.clone(), diplomacy::Concessions::default()],
        ..TreatyOffer::default()
    });
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(
        loaded.diplomacy[&(HUMAN, 1)].offer.as_ref().unwrap().terms[0],
        terms
    );
    assert_eq!(
        loaded.encode_campaign().unwrap(),
        game.encode_campaign().unwrap()
    );
}

#[test]
fn rejects_duplicate_extra_treaty_technologies() {
    let path = temporary_save("invalid-multi-tech");
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let mut terms = diplomacy::Concessions::default().add_technology(Tech::ALPHABET);
    terms.additional_technologies.push(Tech::ALPHABET);
    game.diplomacy.entry((HUMAN, 1)).or_default().offer = Some(TreatyOffer {
        terms: [terms, diplomacy::Concessions::default()],
        ..TreatyOffer::default()
    });
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    fs::remove_file(path).unwrap();
}

#[test]
fn governor_settings_round_trip() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 12, 24);
    let id = game.cities[0].id;
    let settings = GovernorSettings::PRESETS[4].1;
    game.city_governors.insert(
        id,
        CityGovernor {
            active: true,
            settings,
        },
    );

    let path = temporary_save("governor");
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert!(loaded.governor_controls(id));
    assert_eq!(loaded.city_governors[&id].settings, settings);
}

#[test]
fn vassal_agreements_and_pending_submission_survive_save_and_resume() {
    use crate::sim::VassalKind;
    for pending in [false, true] {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let mut world = fixtures::WorldBuilder::new(&mut game);
        world.vassalage_campaign(VassalKind::Capitulation);
        world.finish();
        game.advance_diplomatic_offers_for(1);
        let offer = game.treaty_offer(1).expect("surrender offer");
        if !pending {
            assert!(game.accept_offer(1, offer));
        }
        let path = temporary_save("vassal-resume");
        game.save_campaign(&path).unwrap();
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(
            game.encode_campaign().unwrap(),
            loaded.encode_campaign().unwrap()
        );
        if pending {
            let offer = loaded.treaty_offer(1).unwrap();
            assert!(loaded.accept_offer(1, offer));
            let offer = game.treaty_offer(1).unwrap();
            assert!(game.accept_offer(1, offer));
        }
        game.simulate_turn();
        loaded.simulate_turn();
        assert_eq!(
            game.encode_campaign().unwrap(),
            loaded.encode_campaign().unwrap()
        );
    }
}

#[test]
fn corrupt_vassal_master_is_rejected_by_loader() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let mut world = fixtures::WorldBuilder::new(&mut game);
    world.vassalage_campaign(crate::sim::VassalKind::Capitulation);
    world.finish();
    let offer = game.vassal_offer_view(1).unwrap();
    assert!(game.propose_treaty(1, offer));
    game.vassalage.agreements.get_mut(&1).unwrap().master = 1;
    let path = temporary_save("invalid-vassal-master");
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    fs::remove_file(path).unwrap();
}

#[test]
fn corrupt_pending_vassal_participants_are_rejected() {
    for master in [1, usize::MAX] {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let mut world = fixtures::WorldBuilder::new(&mut game);
        world.vassalage_campaign(crate::sim::VassalKind::Capitulation);
        world.finish();
        game.advance_diplomatic_offers_for(1);
        game.diplomacy
            .get_mut(&(0, 1))
            .unwrap()
            .offer
            .as_mut()
            .unwrap()
            .vassalage
            .as_mut()
            .unwrap()
            .master = master;
        let path = temporary_save("invalid-pending-vassal");
        game.save_campaign(&path).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn loading_a_campaign_rebuilds_borders_from_culture() {
    let path = temporary_save("stale-borders");
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = crate::world::Terrain::Grassland;
        tile.known_by.fill(true);
        tile.owner = None;
        tile.border_city = None;
    }
    game.create_city(HUMAN, 20, 24);
    game.rebuild_city_ownership();
    let city_id = game.cities[0].id;
    let square = game.city_border_extent(city_id).unwrap();
    assert_eq!(square, (1, 9));

    let (cx, cy) = (game.cities[0].x, game.cities[0].y);
    let mut stale = Vec::new();
    for index in 0..game.tiles.len() {
        let (x, y) = game.map_wrap.position(index);
        if game.map_wrap.step_distance(cx, cy, x, y) <= 2 {
            game.tiles[index].owner = Some(HUMAN);
            game.tiles[index].border_city = Some(city_id);
            stale.push((x, y));
        }
    }
    assert!(stale.len() > 9);
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(loaded.city_border_extent(city_id), Some(square));
    for (x, y) in stale {
        let near = loaded.map_wrap.step_distance(cx, cy, x, y) <= 1;
        assert_eq!(
            loaded.tile(x, y).border_city,
            near.then_some(city_id),
            "tile ({x},{y})"
        );
    }
}
