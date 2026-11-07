use super::*;

#[cfg(test)]
mod ai_state_tests {
    use super::*;

    #[test]
    fn old_faction_labels_migrate_without_renaming_existing_cities_or_changing_rng() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-roster-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for (player, civ) in game
            .players
            .iter_mut()
            .zip(crate::rules::civilizations::CIVILIZATIONS.iter())
        {
            player.name = civ.legacy_name.to_owned();
        }
        game.create_city(HUMAN, 10, 20);
        game.cities[0].name = "My Custom Capital".to_owned();
        let random = game.random_snapshot();
        game.save_campaign(&path).unwrap();
        let loaded = Simulation::load_campaign(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        for (player, civ) in loaded
            .players
            .iter()
            .zip(crate::rules::civilizations::CIVILIZATIONS.iter())
        {
            assert_eq!(player.name, civ.name);
            assert_eq!(player.leader_name(), civ.leader);
        }
        assert_eq!(loaded.cities[0].name, "My Custom Capital");
        assert_eq!(loaded.random_snapshot(), random);
    }

    #[test]
    fn v13_preserves_native_rng_and_cargo_and_legacy_campaign_streams() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-rng-cargo-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_cargo_order_playtest());
        game.set_random_seed(42);
        for bound in [2, 3, i32::MAX, 100, 1000] {
            game.rand_below(bound);
        }
        game.save_campaign(&path).unwrap();
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        assert_eq!(game.random_snapshot(), loaded.random_snapshot());
        assert_eq!(game.cargo_order, loaded.cargo_order);
        for bound in [0, 1, 1000, 3, i32::MAX, 2] {
            assert_eq!(game.rand_below(bound), loaded.rand_below(bound));
        }
        let data = fs::read(&path).unwrap();
        let mut payload: (
            CampaignPayload<Strategy9>,
            Extras11,
            list_order::ListOrder,
            Extras13,
            observed_map::ObservedMap,
            MapWrap,
        ) = codec().deserialize(&data[HEADER.len()..]).unwrap();
        // V13 remains readable, including its full native RNG and cargo state.
        let saved_random = Simulation::load_campaign(&path).unwrap().random_snapshot();
        let mut previous_v13 = HEADER_V13.to_vec();
        codec()
            .serialize_into(
                &mut previous_v13,
                &(&payload.0, &payload.1, &payload.2, &payload.3),
            )
            .unwrap();
        fs::write(&path, previous_v13).unwrap();
        let migrated_v13 = Simulation::load_campaign(&path).unwrap();
        assert_eq!(migrated_v13.random_snapshot(), saved_random);
        assert_eq!(migrated_v13.cargo_order, loaded.cargo_order);
        assert!(migrated_v13.valid_render_memory());
        let valid_cargo = payload.3.1.clone();
        let ids = payload.3.1.values_mut().next().unwrap();
        ids.push(ids[0]);
        let mut corrupt = HEADER.to_vec();
        codec().serialize_into(&mut corrupt, &payload).unwrap();
        fs::write(&path, corrupt).unwrap();
        assert!(
            Simulation::load_campaign(&path)
                .err()
                .unwrap()
                .to_string()
                .contains("cargo order")
        );
        payload.3.1 = valid_cargo;
        payload.3.0 = Some(FreecivRng::default());
        let mut corrupt = HEADER.to_vec();
        codec().serialize_into(&mut corrupt, &payload).unwrap();
        fs::write(&path, corrupt).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        payload.0.0.rng = 1;
        let mut legacy = HEADER_V12.to_vec();
        codec()
            .serialize_into(&mut legacy, &(payload.0, payload.1, payload.2))
            .unwrap();
        fs::write(&path, legacy).unwrap();
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        assert!(loaded.native_random.is_none());
        assert_eq!(loaded.rand_below(100), 61); // Frozen scalar generator's first result.
        let state = loaded.random_snapshot();
        loaded.save_campaign(&path).unwrap();
        let mut upgraded = Simulation::load_campaign(&path).unwrap();
        assert_eq!(state, upgraded.random_snapshot());
        assert_eq!(loaded.rand_below(1000), upgraded.rand_below(1000));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn v12_preserves_independent_orders_and_rejects_invalid_stamps() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-list-order-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.native_list_order_playtest(), (true, true, true));
        game.sync_list_order();
        game.save_campaign(&path).unwrap();
        let loaded = Simulation::load_campaign(&path).unwrap();
        assert_eq!(loaded.list_order, game.list_order);
        assert_eq!(
            loaded
                .player_units_ordered(1)
                .map(|u| u.id)
                .collect::<Vec<_>>(),
            game.player_units_ordered(1)
                .map(|u| u.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            loaded
                .tile_units_ordered(10, 24)
                .map(|u| u.id)
                .collect::<Vec<_>>(),
            game.tile_units_ordered(10, 24)
                .map(|u| u.id)
                .collect::<Vec<_>>()
        );
        let data = fs::read(&path).unwrap();
        let mut payload: (
            CampaignPayload<Strategy9>,
            Extras11,
            list_order::ListOrder,
            Extras13,
            observed_map::ObservedMap,
            MapWrap,
        ) = codec().deserialize(&data[HEADER.len()..]).unwrap();
        payload.2.next = 0;
        let mut data = HEADER.to_vec();
        codec().serialize_into(&mut data, &payload).unwrap();
        fs::write(&path, data).unwrap();
        assert!(
            Simulation::load_campaign(&path)
                .err()
                .unwrap()
                .to_string()
                .contains("list order")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn v11_rejects_nonreciprocal_guards_and_unknown_city_celebration_history() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-invalid-guard-history-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let guard = game.spawn_unit(1, UnitKind::Phalanx, 10, 24);
        let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 24);
        game.ai_guard_targets
            .insert(guard, guards::GuardTarget::Unit(charge));
        game.save_campaign(&path).unwrap();
        let error = Simulation::load_campaign(&path)
            .err()
            .expect("one-sided guard link accepted");
        assert!(error.to_string().contains("bodyguard"));
        game.ai_unit_guards
            .insert(charge, guards::GuardState::Assigned(guard));
        game.city_rapture.insert(u32::MAX, 3);
        game.save_campaign(&path).unwrap();
        let error = Simulation::load_campaign(&path)
            .err()
            .expect("unknown city's history accepted");
        assert!(error.to_string().contains("celebration"));
        game.city_rapture.clear();
        game.create_city(HUMAN, 10, 24);
        game.city_rapture.insert(game.cities.last().unwrap().id, -1);
        game.save_campaign(&path).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_cached_environment_count_is_rejected_on_load() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.climate_current = [MAP_W * MAP_H + 1, 0];
        let path = std::env::temp_dir().join(format!(
            "civcraft-invalid-climate-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        game.save_campaign(&path).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn ordered_hunter_maps_keep_legacy_map_wire_format() {
        #[derive(serde::Serialize)]
        struct Canonical {
            #[serde(serialize_with = "serialize_unit_map")]
            positions: std::collections::HashMap<u32, (i32, i32)>,
        }
        let entries = [(17, (4, 8)), (3, (10, 12)), (51, (16, 20))];
        let original: std::collections::HashMap<_, _> = entries.into_iter().collect();
        let legacy_bytes = codec().serialize(&original).unwrap();
        let legacy_decoded: std::collections::HashMap<u32, (i32, i32)> =
            codec().deserialize(&legacy_bytes).unwrap();
        let reversed = entries.into_iter().rev().collect();
        let canonical = codec()
            .serialize(&Canonical {
                positions: original.clone(),
            })
            .unwrap();
        assert_eq!(
            canonical,
            codec()
                .serialize(&Canonical {
                    positions: reversed
                })
                .unwrap()
        );
        // A one-field struct has the exact historical map encoding; no new
        // save header, sequence tag or per-entry metadata is introduced.
        let decoded: std::collections::HashMap<u32, (i32, i32)> =
            codec().deserialize(&canonical).unwrap();
        assert_eq!(decoded, legacy_decoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn strategic_state_loaded_replays_and_native_task_resets_migrate_legacy_saves() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.save_campaign_playtest().unwrap(), (true, true, true));
    }

    #[test]
    fn invalid_space_countdown_is_rejected_before_advancing() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let path = std::env::temp_dir().join(format!(
            "civcraft-invalid-ai-timer-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        game.ai_space_warnings.insert((1, HUMAN), i32::MIN);
        game.save_campaign(&path).unwrap();
        let error = match Simulation::load_campaign(&path) {
            Ok(_) => panic!("invalid space warning was accepted"),
            Err(error) => error,
        };
        fs::remove_file(path).unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("space warning"));
    }
}

#[cfg(test)]
mod wrap_save_tests {
    use super::*;
    #[test]
    fn new_saves_preserve_the_campaign_wrap_setting() {
        let path = std::env::temp_dir().join(format!("civcraft-wrap-{}.save", std::process::id()));
        for wrap in [MapWrap::X, MapWrap::XY] {
            let game = Simulation::new(crate::mapgen::generate_with_wrap(42, wrap));
            game.save_campaign(&path).unwrap();
            let loaded = Simulation::load_campaign(&path).unwrap();
            assert_eq!(loaded.map_wrap, wrap);
            assert_eq!(loaded.random_snapshot(), game.random_snapshot());
            assert_eq!(
                bincode::serialize(&loaded.tiles).unwrap(),
                bincode::serialize(&game.tiles).unwrap()
            );
        }
        fs::remove_file(path).unwrap();
    }
}
