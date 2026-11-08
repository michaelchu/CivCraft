//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn merchant_actual_ferry_capacity_reselects_for_guard_and_keeps_mission() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.diplomacy.clear();
    game.ai_ferries = Default::default();
    game.ai_merchant_missions.clear();
    game.ai_guard_targets.clear();
    game.ai_unit_guards.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.owner = None;
        tile.hut = false;
        tile.river = false;
    }
    let home = (10, 24);
    let destination = offset_native_by_map(home.0, home.1, 8, 0);
    for (x, y) in [home, destination] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
    }
    game.create_city(1, home.0, home.1);
    let origin = game.cities.last().unwrap().id;
    game.create_city(1, destination.0, destination.1);
    let city = game.cities.last().unwrap().id;
    game.cities.last_mut().unwrap().production = Production::Building(Building::PYRAMIDS);
    let merchant = game.spawn_unit(1, UnitKind::Caravan, home.0, home.1);
    game.units.last_mut().unwrap().home_city = Some(origin);
    game.ai_merchant_missions.insert(
        merchant,
        MerchantMission {
            city,
            wonder: true,
            owner: 1,
        },
    );
    let small = game.spawn_unit(1, UnitKind::Trireme, home.0, home.1);
    game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    game.units.last_mut().unwrap().transport = Some(small);
    let farther = offset_native_by_map(home.0, home.1, -2, 0);
    let roomy = game.spawn_unit(1, UnitKind::Trireme, farther.0, farther.1);
    let actor = game
        .units
        .iter()
        .find(|u| u.id == merchant)
        .unwrap()
        .clone();
    assert_eq!(
        game.ferry_delivery(&actor, 0, &BTreeMap::new())
            .map(|p| p.ship),
        Some(small)
    );
    let guard = game.spawn_unit(1, UnitKind::Partisan, home.0, home.1);
    game.assign_ai_guard(guard, guards::GuardTarget::Unit(merchant));
    let actual = game.load_ai_merchant_ferries(1);
    assert_eq!(actual.get(&roomy), Some(&merchant));
    assert!(!actual.contains_key(&small));
    assert_eq!(
        game.ai_merchant_missions.get(&merchant).map(|m| m.city),
        Some(city)
    );
    game.remove_units_with_cargo(vec![roomy]);
    assert!(game.load_ai_merchant_ferries(1).is_empty());
    assert_eq!(
        game.ai_merchant_missions.get(&merchant).map(|m| m.city),
        Some(city)
    );
}

#[test]
fn merchant_values_reciprocal_route_losses_and_unstarted_wonders() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_merchant_valuation_playtest());
}

#[test]
fn marketplace_opportunity_cost_includes_the_journey_discount() {
    assert!(worthwhile_marketplace(100.0, 50, 5, 0));
    assert!(!worthwhile_marketplace(100.0, 50, 5, 5));
}

#[test]
fn marketplace_deliveries_must_repay_production_opportunity_cost() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.create_city(1, 10, 24);
    game.create_city(HUMAN, 14, 24);
    game.sign_pact(1, HUMAN, Pact::Peace);
    let home = game.cities[0].clone();
    let dest = game.cities[1].clone();
    game.trade_routes.push((home.id, dest.id));
    let merchant = game.make_unit(0, 1, UnitKind::Freight, home.x, home.y);
    assert!(game.delivery_gold(&home, &dest) < merchant.kind.stats().cost);
    assert_eq!(game.merchant_trade_value(&merchant, &home, &dest, 0), 0.0);
}

#[test]
fn ferry_landing_counts_only_the_movement_available_to_the_caravan() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.road = false;
        tile.railroad = false;
    }
    let sea = (10, 24);
    let landing = offset_native_by_map(sea.0, sea.1, 1, 0);
    let destination = offset_native_by_map(sea.0, sea.1, 2, 0);
    game.tile_mut(landing.0, landing.1).terrain = Terrain::Mountains;
    game.tile_mut(destination.0, destination.1).terrain = Terrain::Grassland;
    game.create_city(HUMAN, destination.0, destination.1);
    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    let mut ship = game.make_unit(0, 1, UnitKind::Transport, sea.0, sea.1);
    ship.moves_left = 0;
    let allowance = game.unit_move_allowance(ship.owner, ship.kind);
    let mut merchant = game.make_unit(0, 1, UnitKind::Caravan, sea.0, sea.1);
    let labels = game.continent_labels();
    let city = &game.cities[0];

    assert_eq!(
        game.inland_ferry_approach(&merchant, &ship, city, allowance, 0, &labels),
        Some((1, sea.0, sea.1, landing))
    );
    merchant.moves_left = 0;
    assert_eq!(
        game.inland_ferry_approach(&merchant, &ship, city, allowance, 0, &labels)
            .map(|(turns, _, _, _)| turns),
        Some(2)
    );
}

#[test]
fn generic_merchant_ferry_fixtures_pick_up_and_deliver_with_native_galleons() {
    for mode in 0..3 {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|u| u.owner == HUMAN && u.kind.stats().can_found)
            .unwrap()
            .id;
        assert!(game.apply(Command::FoundCity(settler)));
        let (merchant, ferry, home, destination) = match mode {
            0 => game.ai_merchant_ferry_playtest_setup(),
            1 => game.ai_ferry_dispatch_playtest_setup(),
            _ => game.ai_inland_ferry_playtest_setup(),
        };
        assert_eq!(game.unit_by_id(ferry).unwrap().kind, UnitKind::Galleon);
        let position = |game: &Simulation, id| game.unit_by_id(id).map(|u| (u.x, u.y));
        let initial = position(&game, ferry);
        let waiting = position(&game, merchant);
        let mut turns = 0;
        while turns < [1, 5, 4][mode] && !game.merchant_playtest_route_exists(home, destination) {
            assert!(game.apply(Command::EndTurn));
            turns += 1;
            if mode == 1 && turns == 1 {
                assert_ne!(position(&game, ferry), initial);
                assert_eq!(position(&game, merchant), waiting);
            }
        }
        assert_eq!(turns, [1, 3, 1][mode], "ferry scenario mode {mode}");
        assert!(
            game.merchant_playtest_route_exists(home, destination),
            "mode{mode} {turns}"
        );
        assert!(game.units.iter().all(|u| u.id != merchant));
        assert!(
            game.units
                .iter()
                .any(|u| u.id == ferry && (mode != 2 || game.tile(u.x, u.y).terrain.is_water()))
        );
    }
}

#[test]
fn coastal_merchants_board_and_deliver_without_standing_in_a_city() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_merchant_coastal_pickup_playtest());
}

#[test]
fn exhausted_merchants_do_not_change_tasks_or_consume_random_draws() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(1, 10, 24);
    game.cities[0].production = Production::Building(Building::PYRAMIDS);
    let id = game.spawn_unit(1, UnitKind::Freight, 10, 24);
    let unit = game.units.last_mut().unwrap();
    unit.home_city = Some(game.cities[0].id);
    unit.moves_left = 0;
    let rng = game.random_snapshot();
    game.load_ai_merchant_ferries(1);
    assert_eq!(game.random_snapshot(), rng);
    assert!(!game.ai_merchant_missions.contains_key(&id));
    assert!(!game.merchant_mission_attempts.contains(&id));
}

#[test]
fn a_captured_merchant_keeps_its_birth_age_and_loses_the_old_mission() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(1, 10, 24);
    game.turn = 2;
    let old = game.spawn_unit(1, UnitKind::Freight, 10, 24);
    let victim = game.units.last().unwrap().clone();
    game.ai_merchant_missions.insert(
        old,
        MerchantMission {
            owner: 1,
            city: game.cities[0].id,
            wonder: true,
        },
    );
    game.turn = 18;
    game.transfer_unit(&victim, HUMAN, None);
    let gained = game.units.last().unwrap();
    assert_ne!(gained.id, old);
    assert_eq!(game.merchant_birth_turns.get(&gained.id), Some(&2));
    assert!(!game.merchant_birth_turns.contains_key(&old));
    assert!(!game.ai_merchant_missions.contains_key(&old));
}

#[test]
fn lost_home_recovery_chooses_a_city_on_the_current_continent() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.hut = false;
    }
    for y in 18..=42 {
        for x in 0..=8 {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
    }
    game.create_city(1, 6, 40);
    game.tile_mut(11, 24).terrain = Terrain::Grassland;
    game.create_city(1, 11, 24);
    let id = game.spawn_unit(1, UnitKind::Freight, 8, 24);
    game.units.last_mut().unwrap().home_city = Some(u32::MAX);
    let merchant = game.units.last().unwrap().clone();
    game.prepare_merchant_mission(&merchant);
    let merchant = game.unit_by_id(id).unwrap();
    assert_ne!((merchant.x, merchant.y), (8, 24));
    assert!(merchant.x <= 8);
}

#[test]
fn merchant_assignments_retain_destinations_and_apply_foreign_scope_and_boat_patience() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_merchant_missions_playtest());
}

#[test]
fn ai_freight_can_help_a_wonder_under_construction() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(1, 10, 20);
    game.cities[0].production = Production::Building(Building::PYRAMIDS);
    let freight = game.spawn_unit(1, UnitKind::Freight, 10, 20);
    game.units.last_mut().unwrap().home_city = Some(game.cities[0].id);
    let unit = game.units.last().unwrap().clone();
    game.run_ai_merchant(&unit);
    assert_eq!(game.cities[0].shields, 50);
    assert!(game.units.iter().all(|unit| unit.id != freight));
}

#[test]
fn repeat_deliveries_pay_gold_and_plague_spreads_only_over_active_routes() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let (caravan, _, home, foreign) = game.merchant_playtest_setup();
    let owner = game.players[HUMAN].gold;
    let repeat = game
        .units
        .iter()
        .find(|unit| unit.id == caravan)
        .unwrap()
        .clone();
    assert!(game.apply(Command::DeliverGoods(caravan, foreign)));
    assert_eq!(game.trade_routes, [(home, foreign)]);
    assert!(game.players[HUMAN].gold > owner);
    let after_first = game.players[HUMAN].gold;
    let duplicate = game.spawn_unit(HUMAN, UnitKind::Freight, repeat.x, repeat.y);
    game.units.last_mut().unwrap().home_city = Some(home);
    assert!(game.apply(Command::DeliverGoods(duplicate, foreign)));
    assert_eq!(game.trade_routes, [(home, foreign)]);
    assert!(game.players[HUMAN].gold > after_first);

    let home_index = game.cities.iter().position(|city| city.id == home).unwrap();
    let partner = game
        .cities
        .iter_mut()
        .find(|city| city.id == foreign)
        .unwrap();
    partner.last_plague = Some(game.turn);
    game.cities[home_index].size = 9;
    assert_eq!(game.trade_illness(&game.cities[home_index]), 4);
    game.turn += 5;
    assert_eq!(game.trade_illness(&game.cities[home_index]), 0);
    assert!(game.cancel_pact(HUMAN, 1));
    assert!(game.trade_routes.is_empty());
}
