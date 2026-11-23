//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn lost_worker_assignment_is_replaced_using_production_yields() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.create_city(HUMAN, 20, 24);
    let id = game.cities.last().unwrap().id;
    let positions: Vec<_> = game
        .map_city_radius(game.city_by_id(id).unwrap())
        .filter(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
        .map(|(_, _, x, y)| (x, y))
        .collect();
    for &(x, y) in &positions {
        game.tile_mut(x, y).owner = None;
    }
    let (coast, forest) = (positions[0], positions[1]);
    for ((x, y), terrain) in [(coast, Terrain::Coast), (forest, Terrain::Forest)] {
        let tile = game.tile_mut(x, y);
        tile.terrain = terrain;
        tile.owner = Some(HUMAN);
        tile.resource = None;
        tile.river = false;
    }
    let city = game.cities.last_mut().unwrap();
    city.size = 1;
    city.specialists = [0; 3];
    city.worked_tiles.clear();
    game.reconcile_city_workers();
    assert_eq!(game.city_by_id(id).unwrap().worked_tiles, [coast]);
}

fn kill_citizen_unit_kind() -> UnitKind {
    UnitKind::all()
        .find(|&kind| {
            crate::sim::movement_rules::MovementRules::campaign()
                .unit_class_movement_data_for_kind(kind)
                .expect("campaign unit class is mapped")
                .kills_citizens
        })
        .expect("C2C3 has a KillCitizen unit class")
}

#[test]
fn combat_population_loss_uses_killcitizen_and_selected_protection_effects() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 20, 24);
    game.cities[0].size = 3;
    game.cities[0].buildings.push(Building::CITY_WALLS);
    let attacker_id = game.spawn_unit(HUMAN, kill_citizen_unit_kind(), 19, 24);
    let attacker = game.unit_by_id(attacker_id).unwrap().clone();

    game.apply_combat_population_loss(&attacker, 20, 24);
    assert_eq!(game.cities[0].size, 3, "City Walls protect the population");

    game.cities[0]
        .buildings
        .retain(|&building| building != Building::CITY_WALLS);
    game.cities[0].buildings.push(Building::GREAT_WALL);
    game.apply_combat_population_loss(&attacker, 20, 24);
    assert_eq!(game.cities[0].size, 3, "Great Wall protects the population");

    game.cities[0]
        .buildings
        .retain(|&building| building != Building::GREAT_WALL);
    game.apply_combat_population_loss(&attacker, 20, 24);
    assert_eq!(game.cities[0].size, 2);

    game.cities[0].size = 1;
    game.apply_combat_population_loss(&attacker, 20, 24);
    assert_eq!(game.cities[0].size, 1, "C2C3 protects size-one cities");
}

#[test]
fn melee_population_loss_respects_class_flags_and_cannot_destroy_a_city() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 20, 24);
    game.cities[0].size = 3;
    let warrior_id = game.spawn_unit(HUMAN, UnitKind::Warrior, 19, 24);
    let warrior = game.unit_by_id(warrior_id).unwrap().clone();
    assert!(
        !crate::sim::movement_rules::MovementRules::campaign()
            .unit_class_movement_data_for_kind(warrior.kind)
            .unwrap()
            .kills_citizens
    );
    game.apply_combat_population_loss(&warrior, 20, 24);
    assert_eq!(game.cities[0].size, 3, "Land has no KillCitizen flag");

    assert!(game.reduce_city_population_from_bombard(game.cities[0].id, HUMAN));
    assert_eq!(game.cities[0].size, 2);

    game.cities[0].size = 1;
    assert!(!game.reduce_city_population_from_bombard(game.cities[0].id, HUMAN));
    assert_eq!(
        game.cities[0].size, 1,
        "Bombard cannot destroy a size-one city"
    );
}

#[test]
fn conquest_destroys_size_one_city_without_loot_or_transfer() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    game.cities[1].size = 1;
    game.players[1].gold = 500;
    game.players[HUMAN].gold = 50;
    let city_id = game.cities[1].id;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Swordsman, 20, 24);

    game.capture_city_if_needed(attacker, 20, 24);

    assert!(game.city_index_by_id(city_id).is_none());
    assert_eq!(game.players[1].gold, 500);
    assert_eq!(game.players[HUMAN].gold, 50);
}

#[test]
fn conquest_transfers_city_loots_gold_raze_and_savepalace_by_selected_rules() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.create_city(1, 20, 24);
    game.create_city(1, 30, 24);
    let city_id = game.cities[1].id;
    let old_owner_cities = game
        .player_cities_ordered(1)
        .filter(|city| city.id != city_id)
        .map(|city| city.id)
        .collect::<Vec<_>>();
    game.cities[0]
        .buildings
        .retain(|&building| building != Building::PALACE);
    game.cities[1].size = 4;
    game.cities[1].food = 7;
    game.cities[1].shields = 30;
    game.cities[1].buildings.extend([
        Building::PALACE,
        Building::TEMPLE,
        Building::GREAT_LIBRARY,
        Building::LIBRARY,
        Building::CITY_WALLS,
    ]);
    game.players[1].gold = 200;
    game.players[HUMAN].gold = 0;
    for player in &mut game.players {
        player.known.clear();
        player.future_techs = 0;
    }
    game.set_random_seed(42);
    let mut expected_random = game.clone();
    let loot = expected_random.rand_below(11) + 200 * 4 / 200;
    expected_random.rand_below(100); // Conquest_Tech_Pct = 100.
    let razechance = crate::sim::combat_rules::selected().razechance;
    let city_walls_razed = expected_random.rand_below(100) < razechance;
    let library_razed = expected_random.rand_below(100) < razechance;
    let temple_razed = expected_random.rand_below(100) < razechance;
    let saved_palace_city = old_owner_cities[expected_random.rand_below(2) as usize];
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Swordsman, 20, 24);

    game.capture_city_if_needed(attacker, 20, 24);

    let captured_index = game.city_index_by_id(city_id).unwrap();
    let captured = &game.cities[captured_index];
    assert_eq!(captured.owner, HUMAN);
    assert_eq!(captured.size, 3);
    assert_eq!(captured.food, 7, "conquest preserves stored food");
    assert_eq!(captured.shields, 0, "C2C3 razing clears shield stock");
    assert_eq!(game.players[HUMAN].gold, loot);
    assert_eq!(game.players[1].gold, 200 - loot);
    assert_eq!(game.players[HUMAN].gold + game.players[1].gold, 200);
    assert!(captured.has_building(Building::PALACE));
    assert!(captured.has_building(Building::GREAT_LIBRARY));
    assert_eq!(
        captured.has_building(Building::CITY_WALLS),
        !city_walls_razed
    );
    assert_eq!(captured.has_building(Building::LIBRARY), !library_razed);
    assert_eq!(captured.has_building(Building::TEMPLE), !temple_razed);
    assert!(
        game.city_by_id(saved_palace_city)
            .unwrap()
            .has_building(Building::PALACE)
    );
    assert_eq!(crate::sim::combat_rules::selected().razechance, 10);
    assert!(crate::sim::combat_rules::selected().savepalace);
    assert_eq!(game.random_snapshot(), expected_random.random_snapshot());
}

#[test]
fn conquest_steals_an_available_technology_using_selected_effect_and_transfer_settings() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 20, 24);
    game.cities[0].size = 3;
    game.players[1].gold = 0;
    game.players[HUMAN].gold = 0;
    for player in &mut game.players {
        player.known.clear();
        player.future_techs = 0;
    }
    let tech = Tech::all()
        .find(|&tech| {
            tech.is_researchable_advance()
                && tech
                    .catalog_prerequisites()
                    .is_some_and(|prerequisites| prerequisites.is_empty())
        })
        .expect("the C2C3 tree has a root technology");
    game.players[1].known.push(tech);
    let mut expected_random = game.clone();
    expected_random.rand_below(1); // Zero-loot roll does not advance Freeciv RNG.
    expected_random.rand_below(100); // Conquest_Tech_Pct roll.
    expected_random.rand_below(1); // Exactly one eligible advance.
    expected_random.rand_below(100); // tech_transfer always rolls receiver loss.
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Swordsman, 20, 24);

    game.capture_city_if_needed(attacker, 20, 24);

    assert!(game.has_tech(HUMAN, tech));
    assert!(game.has_tech(1, tech), "default donor loss chance is zero");
    assert_eq!(game.random_snapshot(), expected_random.random_snapshot());
}

fn conquest_fixture() -> (Simulation, u32, u32, (i32, i32)) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let center = (20, 24);
    let mut bounce_area = game
        .map_wrap
        .square_radius_in_source_order(center.0, center.1, 2)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    bounce_area.extend(
        game.map_wrap
            .tiles_within_steps(center.0, center.1, 2)
            .into_iter()
            .map(|(x, y, _)| (x, y)),
    );
    for (x, y) in bounce_area {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
    }
    game.create_city(1, center.0, center.1);
    game.cities[0].size = 3;
    game.sign_pact(HUMAN, 1, Pact::War);
    let city_id = game.cities[0].id;
    let attacker = game.spawn_unit(HUMAN, UnitKind::Swordsman, center.0, center.1);
    (game, city_id, attacker, center)
}

#[test]
fn conquest_invokes_partisans_after_transfer_and_population_loss() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
    }

    let center = (20, 24);
    game.create_city(1, center.0, center.1);
    let city_id = game.cities[0].id;
    game.cities[0].size = 5;
    game.cities[0]
        .buildings
        .retain(|&building| building != Building::PALACE);
    // The city starts at 60% defeated-player nationality after transfer
    // (3/5), below C2C3's 75% threshold. The population-loss draw must remove
    // a citizen of nation 2 to make the surviving city eligible (3/4).
    game.cities[0].foreign_citizens = vec![(2, 2)];
    assert_eq!(game.cities[0].domestic_citizens(), 3);
    assert_eq!(game.cities[0].foreign_citizens, [(2, 2)]);

    for player in &mut game.players {
        player.known.clear();
        player.future_techs = 0;
    }
    game.players[1].government = Government::COMMUNISM;
    game.players[1]
        .known
        .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
    // C2C3 Guerilla Warfare is adapted to Civ III Nationalism in the selected
    // technology crosswalk, while the Civ III research tree stays authoritative.
    game.players[HUMAN]
        .known
        .extend([Tech::COMMUNISM, Tech::GUNPOWDER, Tech::NATIONALISM]);
    game.players[1].gold = 0;
    game.players[HUMAN].gold = 0;
    game.sign_pact(HUMAN, 1, Pact::War);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Swordsman, center.0, center.1);

    let (seed, captured) = (1..=256)
        .find_map(|seed| {
            let mut candidate = game.clone();
            candidate.set_random_seed(seed);
            candidate.capture_city_if_needed(attacker, center.0, center.1);
            let city = candidate.city_by_id(city_id)?;
            (city.size == 4
                && city.foreign_citizens == [(1, 3), (2, 1)]
                && candidate.units.iter().any(|unit| {
                    unit.owner == 1
                        && unit.kind == UnitKind::Guerilla
                        && unit.state == UnitState::Fortified
                }))
            .then_some((seed, candidate))
        })
        .expect(
            "a deterministic seed leaves 75% defeated-player nationality and inspires Partisans",
        );

    let captured_city = captured.city_by_id(city_id).unwrap();
    assert_eq!(captured_city.owner, HUMAN, "the city has transferred first");
    assert_eq!(captured_city.size, 4, "conquest removes one population");
    assert_eq!(captured_city.foreign_citizens, [(1, 3), (2, 1)]);
    assert!(captured.units.iter().any(|unit| {
        unit.owner == 1 && unit.kind == UnitKind::Guerilla && unit.state == UnitState::Fortified
    }));
    let expected_event = format!("The loss of {} has inspired partisans.", captured_city.name);
    assert!(captured.events.iter().any(|event| event == &expected_event));
    assert!(seed > 0);
}

#[test]
fn native_leaders_do_not_inherit_foreign_unique_or_king_flags() {
    assert_eq!(
        crate::sim::action_rules::unit_type_flag(UnitKind::Leader, "Unique"),
        Some(false)
    );
    assert_eq!(
        crate::sim::action_rules::unit_type_flag(UnitKind::Leader, "GameLoss"),
        Some(false)
    );
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    let first = game.spawn_unit(HUMAN, UnitKind::Leader, 10, 24);
    let second = game.spawn_unit(HUMAN, UnitKind::Leader, 12, 24);
    assert!(game.unit_by_id(first).is_some());
    assert!(game.unit_by_id(second).is_some());
}

#[test]
fn conquest_bounces_hostile_third_party_units_but_keeps_allied_units_at_center() {
    let (mut hostile_game, _, attacker, center) = conquest_fixture();
    let hostile_id = hostile_game.spawn_unit(2, UnitKind::Warrior, center.0, center.1);

    hostile_game.capture_city_if_needed(attacker, center.0, center.1);

    let hostile = hostile_game.unit_by_id(hostile_id).unwrap();
    assert_eq!(hostile.owner, 2);
    assert_ne!((hostile.x, hostile.y), center);

    let (mut allied_game, _city_id, attacker, center) = conquest_fixture();
    allied_game.sign_pact(HUMAN, 2, Pact::Alliance);
    let allied_id = allied_game.spawn_unit(2, UnitKind::Warrior, center.0, center.1);

    allied_game.capture_city_if_needed(attacker, center.0, center.1);

    let allied = allied_game.unit_by_id(allied_id).unwrap();
    assert_eq!(allied.owner, 2);
    assert_eq!((allied.x, allied.y), center);
}

#[test]
fn conquest_transfer_bounces_units_from_other_nonallied_cities() {
    let (mut game, _, attacker, _) = conquest_fixture();
    let foreign_city = (10, 24);
    let mut bounce_area = game
        .map_wrap
        .square_radius_in_source_order(foreign_city.0, foreign_city.1, 2)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    bounce_area.extend(
        game.map_wrap
            .tiles_within_steps(foreign_city.0, foreign_city.1, 2)
            .into_iter()
            .map(|(x, y, _)| (x, y)),
    );
    for (x, y) in bounce_area {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
    }
    game.create_city(2, foreign_city.0, foreign_city.1);
    let illegal_city_unit =
        game.spawn_unit(HUMAN, UnitKind::Worker, foreign_city.0, foreign_city.1);

    game.capture_city_if_needed(attacker, 20, 24);

    let relocated = game
        .unit_by_id(illegal_city_unit)
        .expect("an illegal city occupant with a safe tile keeps its ID");
    assert_eq!(relocated.owner, HUMAN);
    assert_ne!((relocated.x, relocated.y), foreign_city);
    assert!(
        game.city_at(relocated.x, relocated.y)
            .is_none_or(|city| { game.allied(HUMAN, city.owner) })
    );
}

#[test]
fn conquest_transfer_unloads_unallied_cargo_before_bouncing_illegal_city_occupants() {
    let (mut game, _, attacker, _) = conquest_fixture();
    let foreign_city = (10, 24);
    let mut bounce_area = game
        .map_wrap
        .square_radius_in_source_order(foreign_city.0, foreign_city.1, 2)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    bounce_area.extend(
        game.map_wrap
            .tiles_within_steps(foreign_city.0, foreign_city.1, 2)
            .into_iter()
            .map(|(x, y, _)| (x, y)),
    );
    for (x, y) in bounce_area {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
    }
    game.tile_mut(11, 24).terrain = Terrain::Coast;
    game.create_city(2, foreign_city.0, foreign_city.1);
    let carrier_id = game.spawn_unit(HUMAN, UnitKind::Transport, foreign_city.0, foreign_city.1);
    let cargo_id = game.spawn_unit(1, UnitKind::Warrior, foreign_city.0, foreign_city.1);
    let cargo_index = game.unit_index_by_id(cargo_id).unwrap();
    game.units[cargo_index].transport = Some(carrier_id);
    game.record_transport_change(cargo_id, Some(carrier_id));

    game.capture_city_if_needed(attacker, 20, 24);

    let carrier = game
        .unit_by_id(carrier_id)
        .expect("the illegal-city carrier has a survivable bounce destination");
    let cargo = game
        .unit_by_id(cargo_id)
        .expect("unloaded cargo is separately bounced and keeps its identity");
    assert_ne!((carrier.x, carrier.y), foreign_city);
    assert_ne!((cargo.x, cargo.y), foreign_city);
    assert_eq!(cargo.owner, 1);
    assert_eq!(cargo.transport, None);
}

#[test]
fn conquest_transfer_bounces_both_players_from_newly_hostile_mixed_stacks() {
    let (mut game, _, attacker, _) = conquest_fixture();
    let stack = (18, 24);
    let giver_unit = game.spawn_unit(1, UnitKind::Warrior, stack.0, stack.1);
    let taker_unit = game.spawn_unit(HUMAN, UnitKind::Worker, stack.0, stack.1);

    game.capture_city_if_needed(attacker, 20, 24);

    assert!(game.unit_by_id(giver_unit).is_some());
    assert!(game.unit_by_id(taker_unit).is_some());
    assert!(game.units.iter().all(|left| {
        game.units.iter().all(|right| {
            left.id == right.id
                || (left.x, left.y) != (right.x, right.y)
                || game.allied(left.owner, right.owner)
        })
    }));
    assert_ne!(
        game.unit_by_id(giver_unit).map(|unit| (unit.x, unit.y)),
        Some(stack)
    );
    assert_ne!(
        game.unit_by_id(taker_unit).map(|unit| (unit.x, unit.y)),
        Some(stack)
    );
}

/// Mirrors Freeciv behavior.
#[test]
fn conquest_city_transfer_bounce_uses_reference_order_and_preserves_unit_id() {
    let (mut game, city_id, _attacker, center) = conquest_fixture();
    let bounced_id = game.spawn_unit(2, UnitKind::Worker, center.0, center.1);
    let bounced = game.unit_by_id(bounced_id).unwrap().clone();

    let allied_destination =
        |simulation: &Simulation, x, y| {
            simulation.cities.iter().all(|city| {
                (city.x, city.y) != (x, y) || simulation.allied(bounced.owner, city.owner)
            }) && simulation.units.iter().all(|other| {
                (other.x, other.y) != (x, y) || simulation.allied(bounced.owner, other.owner)
            })
        };
    let ordered_destinations = game
        .map_wrap
        .square_radius_in_source_order(center.0, center.1, 2)
        .into_iter()
        .filter(|&(x, y)| (x, y) != center)
        .filter(|&(x, y)| game.unit_can_survive_at_tile(&bounced, x, y))
        .filter(|&(x, y)| allied_destination(&game, x, y))
        .collect::<Vec<_>>();
    let old_order_destinations = game
        .map_wrap
        .tiles_within_steps(center.0, center.1, 2)
        .into_iter()
        .filter(|&(_, _, distance)| distance != 0)
        .filter(|&(x, y, _)| game.movement_cost_for(&bounced, x, y).is_some())
        .filter(|&(x, y, _)| allied_destination(&game, x, y))
        .map(|(x, y, _)| (x, y))
        .collect::<Vec<_>>();
    assert!(!ordered_destinations.is_empty());
    assert!(!old_order_destinations.is_empty());

    // Choose a stable seed that distinguishes Freeciv's survivability and
    // square-iteration results from the former Civ III radius/movement-cost
    // candidates, so this catches routing back through the old helper.
    let (seed, expected_destination) = (1..=1024)
        .find_map(|seed| {
            let mut ordered_rng = game.clone();
            ordered_rng.set_random_seed(seed);
            let ordered_destination = ordered_destinations
                [ordered_rng.rand_below(ordered_destinations.len() as i32) as usize];

            let mut old_rng = game.clone();
            old_rng.set_random_seed(seed);
            let old_destination = old_order_destinations
                [old_rng.rand_below(old_order_destinations.len() as i32) as usize];
            (ordered_destination != old_destination).then_some((seed, ordered_destination))
        })
        .expect("some seed distinguishes Freeciv and Civ III bounce iteration order");

    game.set_random_seed(seed);
    let mut expected_rng = game.clone();
    expected_rng.rand_below(ordered_destinations.len() as i32);
    let expected_random = expected_rng.random_snapshot();

    game.transfer_conquered_city_units(city_id, 1, HUMAN, center);

    let bounced = game
        .unit_by_id(bounced_id)
        .expect("bouncing preserves the unit's identity");
    assert_eq!((bounced.x, bounced.y), expected_destination);
    assert_eq!(bounced.owner, 2);
    assert_eq!(bounced.transport, None);
    assert_eq!(game.random_snapshot(), expected_random);
}

#[test]
fn conquest_rehomes_supported_units_in_old_cities_and_removes_other_outside_units() {
    let (mut game, city_id, attacker, center) = conquest_fixture();
    let old_owner_city = (10, 24);
    game.create_city(1, old_owner_city.0, old_owner_city.1);
    let home_city_id = game
        .cities
        .iter()
        .find(|city| (city.x, city.y) == old_owner_city)
        .unwrap()
        .id;
    let at_old_city = game.spawn_unit(1, UnitKind::Warrior, old_owner_city.0, old_owner_city.1);
    let outside = game.spawn_unit(1, UnitKind::Warrior, 14, 24);
    for id in [at_old_city, outside] {
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.home_city = Some(city_id);
    }

    game.capture_city_if_needed(attacker, center.0, center.1);

    let rehomed = game
        .unit_by_id(at_old_city)
        .expect("unit in old city survives");
    assert_eq!(rehomed.owner, 1);
    assert_eq!(rehomed.home_city, Some(home_city_id));
    assert!(
        game.unit_by_id(outside).is_none(),
        "kill_outside=0 removes supported units outside another city"
    );
}

#[test]
fn six_civilizations_found_capitals_then_use_unique_names_after_capture_and_loss() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    let game = Simulation::new(crate::test_support::seeded_map());
    assert_eq!(game.city_worker_lifecycle_test(), (true, true, true));
}

#[test]
fn city_events_preserve_other_manual_workers_and_normal_build_completion_does_not_arrange() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (10, 20);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.create_city(HUMAN, x, y);
    let (fx, fy) = offset_native_by_map(x, y, 1, 0);
    let tile = game.tile_mut(fx, fy);
    tile.terrain = Terrain::Desert;
    tile.irrigation = true;
    tile.river = false;
    tile.resource = None;
    game.players[HUMAN]
        .known
        .extend([Tech::ELECTRICITY, Tech::SANITATION, Tech::STEAM_POWER]);
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
    // Civ III farmland adds half again its food on its own, with no building.
    assert_eq!(
        game.city_yields(&game.cities[0]).0,
        center_food + farm_food + farm_food / 2
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
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

/// Clears every river and lake from a city's neighborhood so size caps apply.
fn dry_city_site(game: &mut Simulation, x: i32, y: i32) {
    for (tx, ty) in std::iter::once((x, y)).chain(game.map_wrap.eight_way_neighbors(x, y)) {
        let tile = game.tile_mut(tx, ty);
        tile.river = false;
        if tile.terrain == Terrain::Lake {
            tile.terrain = Terrain::Grassland;
        }
    }
}

#[test]
fn completed_aqueduct_allows_growth_in_the_same_turn() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 10, 20);
    dry_city_site(&mut game, 10, 20);
    let id = game.cities[0].id;
    game.discover(HUMAN, Tech::CONSTRUCTION);
    let target = Production::Building(Building::AQUEDUCT);
    game.cities[0].size = 6;
    game.cities[0].food = Simulation::growth_box(6) + 20;
    assert!(game.apply(Command::Produce(id, target)));
    game.cities[0].shields = target.cost();
    game.process_cities();
    assert!(game.cities[0].has_building(Building::AQUEDUCT));
    assert_eq!(game.cities[0].size, 7);
}

/// Civ III Conquests Civilopedia: Aqueduct beyond 6, Hospital or Shakespeare's
/// Theater beyond 12, and a city beside a river or lake never needs an Aqueduct.
#[test]
fn civ3_city_size_caps_follow_aqueduct_hospital_and_fresh_water() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.clear();
    game.create_city(HUMAN, 10, 20);
    dry_city_site(&mut game, 10, 20);
    assert_eq!(game.city_size_limit(&game.cities[0]), 6);
    game.cities[0].buildings.push(Building::AQUEDUCT);
    assert_eq!(game.city_size_limit(&game.cities[0]), 12);
    game.cities[0].buildings.push(Building::HOSPITAL);
    assert_eq!(game.city_size_limit(&game.cities[0]), 16);
    game.cities[0].buildings.clear();
    game.cities[0]
        .buildings
        .push(Building::SHAKESPEARES_THEATER);
    assert_eq!(game.city_size_limit(&game.cities[0]), 16);
    game.cities[0].buildings.clear();
    game.tile_mut(10, 20).river = true;
    assert_eq!(game.city_size_limit(&game.cities[0]), 12);
}

#[test]
fn ordinary_food_surplus_grows_city_and_food_deficit_shrinks_it() {
    let mut growing = Simulation::new(crate::test_support::seeded_map());
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

    let mut shrinking = Simulation::new(crate::test_support::seeded_map());
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.sign_pact(HUMAN, 1, Pact::War);
    game.units.clear();
    let second = offset_native_by_map(10, 20, 3, 0);
    game.create_city(HUMAN, 10, 20);
    game.create_city(HUMAN, second.0, second.1);
    let shared = offset_native_by_map(10, 20, 1, 0);
    let first = game.cities[0].id;
    let second = game.cities[1].id;
    for id in [first, second] {
        assert!(game.apply(Command::Specialist {
            city_id: id,
            index: 2,
            delta: 1,
        }));
    }
    assert!(game.apply(Command::WorkCityTile {
        city_id: first,
        x: shared.0,
        y: shared.1,
    }));
    assert!(!game.apply(Command::WorkCityTile {
        city_id: second,
        x: shared.0,
        y: shared.1,
    }));
    assert!(game.apply(Command::WorkCityTile {
        city_id: first,
        x: shared.0,
        y: shared.1,
    }));
    assert!(game.apply(Command::WorkCityTile {
        city_id: second,
        x: shared.0,
        y: shared.1,
    }));

    game.spawn_unit(1, UnitKind::Warrior, shared.0, shared.1);
    game.reconcile_city_workers();
    assert!(!game.cities[1].worked_tiles.contains(&shared));
    assert_eq!(game.cities[1].worked_tiles.len(), 1);
    assert!(!game.cities[0].worked_tiles.contains(&shared));
}

#[test]
fn coinage_constant_names_the_convert_building() {
    let data = Building::COINAGE.data();
    assert_eq!(data.name, "Coinage");
    assert_eq!(data.genus, Genus::Convert);
}

#[test]
fn ai_city_that_cannot_build_its_item_switches_to_something_buildable() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    // A civilization ID unknown to the catalog matches no unit mask, so it can
    // build no units; with no technology few buildings are open either.
    game.players[1].name = "Atlantis".into();
    game.players[1].civilization_id = Some("atlantis".into());
    game.players[1].ai = true;
    game.create_city(1, 20, 24);
    let id = game.cities[0].id;
    assert!(!game.unit_available(1, UnitKind::Warrior));
    game.cities[0].production = Production::Unit(UnitKind::Warrior);
    game.cities[0].shields = 0;

    game.process_player_cities(1);

    let city = game.city_by_id(id).unwrap();
    assert!(
        game.can_produce(city, city.production),
        "AI production must be buildable, got {:?}",
        city.production
    );
    let fallback = game.choose_ai_production(1, id);
    assert!(game.can_produce(game.city_by_id(id).unwrap(), fallback));
}

#[test]
fn ai_fallback_is_coinage_when_nothing_else_is_buildable() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.players[1].name = "Atlantis".into();
    game.players[1].civilization_id = Some("atlantis".into());
    game.players[1].ai = true;
    game.create_city(1, 20, 24);
    let city = game.cities[0].clone();
    assert_eq!(
        game.ai_fallback_production(&city),
        Production::Building(Building::COINAGE)
    );
    assert!(game.can_produce(&city, Production::Building(Building::COINAGE)));
}

#[test]
fn leader_can_never_be_produced_or_queued() {
    for civilization in crate::rules::civilizations::all() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.players[1].name = civilization.name.clone();
        game.players[1].known = Tech::all().collect();
        game.create_city(1, 20, 24);
        let city = game.cities[0].clone();
        assert!(
            !game.can_produce(&city, Production::Unit(UnitKind::Leader)),
            "{} must not be able to build a Leader",
            civilization.name
        );
        let view = game.city_manager_view(city.id).unwrap();
        let leader = view
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Leader)
            .expect("the city screen lists every native unit");
        assert!(
            !leader.available,
            "{} sees a buildable Leader",
            civilization.name
        );
        assert!(
            !leader.can_queue,
            "{} can queue a Leader",
            civilization.name
        );
    }
}

#[test]
fn leader_is_not_capturable() {
    assert_eq!(
        crate::sim::action_rules::unit_type_flag(UnitKind::Leader, "Capturable"),
        Some(false)
    );
}

fn leader_test_game() -> (Simulation, u32) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.players[HUMAN].known = Tech::all().collect();
    game.create_city(HUMAN, 20, 24);
    let id = game.cities[0].id;
    (game, id)
}

#[test]
fn leader_completes_only_legal_buildings() {
    for building in [Building::PYRAMIDS, Building::TEMPLE] {
        let (mut game, city) = leader_test_game();
        let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
        game.cities[0].production = Production::Building(building);
        game.cities[0].shields = 0;
        assert!(game.complete_production(leader, city), "{building:?}");
        assert_eq!(
            game.cities[0].shields,
            Production::Building(building).cost()
        );
        assert!(game.cities[0].bought_production);
        assert!(game.unit_by_id(leader).is_none(), "the Leader is expended");
    }

    let refused: Vec<(&str, Production)> = vec![
        ("Coinage", Production::Building(Building::COINAGE)),
        ("a unit", Production::Unit(UnitKind::Warrior)),
        (
            "a spaceship part",
            Production::Building(
                Building::all()
                    .find(|b| b.data().genus == crate::rules::Genus::Special)
                    .unwrap(),
            ),
        ),
    ];
    for (label, production) in refused {
        let (mut game, city) = leader_test_game();
        let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
        game.cities[0].production = production;
        game.cities[0].shields = 0;
        assert!(!game.complete_production(leader, city), "{label}");
        assert_eq!(game.cities[0].shields, 0, "{label}");
        assert!(game.unit_by_id(leader).is_some(), "{label}");
    }

    let (mut game, city) = leader_test_game();
    let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
    game.cities[0].production = Production::Building(Building::TEMPLE);
    game.cities[0].buildings.push(Building::TEMPLE);
    assert!(!game.complete_production(leader, city), "already built");
    game.cities[0].buildings.clear();
    game.cities[0].bought_production = true;
    assert!(!game.complete_production(leader, city), "already completed");

    let (mut game, _) = leader_test_game();
    game.create_city(1, 30, 24);
    let foreign = game.cities[1].id;
    let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 30, 24);
    game.cities[1].production = Production::Building(Building::TEMPLE);
    assert!(
        !game.complete_production(leader, foreign),
        "another civilization's city"
    );
}

#[test]
fn completed_production_cannot_be_redirected() {
    let (mut game, city) = leader_test_game();
    let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
    game.cities[0].production = Production::Building(Building::TEMPLE);
    game.cities[0].shields = 0;
    let gold = game.players[HUMAN].gold;
    assert!(game.complete_production(leader, city));
    assert!(
        !game.can_change_production(&game.cities[0], Production::Building(Building::LIBRARY)),
        "the stock cannot move to another item"
    );
    game.process_player_cities(HUMAN);
    assert!(game.players[HUMAN].gold <= gold + 50);
}

#[test]
fn completed_item_finishes_at_turn_end() {
    let (mut game, city) = leader_test_game();
    let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
    game.cities[0].production = Production::Building(Building::TEMPLE);
    game.cities[0].shields = 0;
    assert!(game.complete_production(leader, city));
    game.process_player_cities(HUMAN);
    assert!(game.cities[0].has_building(Building::TEMPLE));
}

#[test]
fn leader_in_a_traded_city_is_lost_not_handed_over() {
    let (mut game, _) = leader_test_game();
    let inside = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
    game.units
        .iter_mut()
        .find(|u| u.id == inside)
        .unwrap()
        .moves_left = 3;
    game.transfer_city_because(0, 1, lifecycle::TransferReason::Trade);
    assert!(
        game.units
            .iter()
            .all(|unit| unit.kind != UnitKind::Leader || unit.owner == HUMAN),
        "a Leader never changes owner"
    );
    assert!(
        game.unit_by_id(inside).is_none(),
        "the Leader in the city is lost"
    );

    let (mut game, _) = leader_test_game();
    let outside = game.spawn_unit(HUMAN, UnitKind::Leader, 40, 24);
    game.transfer_city_because(0, 1, lifecycle::TransferReason::Trade);
    assert_eq!(
        game.unit_by_id(outside).map(|unit| unit.owner),
        Some(HUMAN),
        "a Leader outside the city is unaffected"
    );
}

#[test]
fn civil_war_never_hands_a_leader_to_the_rebels() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for (x, y) in [(10, 24), (20, 24), (30, 24)] {
        game.create_city(1, x, y);
        game.spawn_unit(1, UnitKind::Leader, x, y);
    }
    game.split_civil_war(1);
    assert!(
        game.units
            .iter()
            .all(|unit| unit.kind != UnitKind::Leader || unit.owner == 1),
        "a rebel civilization must not own a Leader"
    );
}
