//! Regression tests for the behavior of the parent module.

use super::production::Growth;
use super::*;

fn kill_citizen_unit_kind() -> UnitKind {
    UnitKind::ALL_NATIVE
        .into_iter()
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
fn combat_population_loss_respects_class_flags_and_bombard_effect_context() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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

    let bombard_id = game.spawn_unit(HUMAN, kill_citizen_unit_kind(), 19, 24);
    let bombard = game.unit_by_id(bombard_id).unwrap().clone();
    game.apply_bombard_population_loss(&bombard, 20, 24);
    assert_eq!(game.cities[0].size, 2);

    game.cities[0].size = 1;
    game.apply_bombard_population_loss(&bombard, 20, 24);
    assert_eq!(
        game.cities[0].size, 1,
        "Bombard cannot destroy a size-one city"
    );
}

#[test]
fn conquest_destroys_size_one_city_without_loot_or_transfer() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    assert_eq!(captured.has_building(Building::PALACE), true);
    assert_eq!(captured.has_building(Building::GREAT_LIBRARY), true);
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
            tech.is_civ3_advance()
                && tech
                    .civ3_prerequisites()
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let center = (20, 24);
    let mut bounce_area = game
        .map_wrap
        .freeciv_square_radius(center.0, center.1, 2)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    bounce_area.extend(
        game.map_wrap
            .civ3_radius(center.0, center.1, 2)
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
fn conquest_transfers_center_garrison_and_resolves_unique_game_loss_conflicts() {
    let (mut game, city_id, attacker, center) = conquest_fixture();
    let garrison_id = game.spawn_unit(1, UnitKind::Warrior, center.0, center.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == garrison_id)
        .unwrap()
        .home_city = Some(city_id);

    game.capture_city_if_needed(attacker, center.0, center.1);

    let transferred = game
        .unit_by_id(garrison_id)
        .expect("city transfer preserves the unit identity");
    assert_eq!(transferred.owner, HUMAN);
    assert_eq!((transferred.x, transferred.y), center);
    assert_eq!(transferred.home_city, Some(city_id));

    let (mut game, _city_id, attacker, center) = conquest_fixture();
    assert_eq!(
        crate::sim::action_rules::freeciv_unit_type_flag(UnitKind::Leader, "Unique"),
        Some(true)
    );
    assert_eq!(
        crate::sim::action_rules::freeciv_unit_type_flag(UnitKind::Leader, "GameLoss"),
        Some(true)
    );
    let existing_unique = game.spawn_unit(HUMAN, UnitKind::Leader, 5, 20);
    let conflicting_unique = game.spawn_unit(1, UnitKind::Leader, center.0, center.1);

    game.capture_city_if_needed(attacker, center.0, center.1);

    assert!(game.unit_by_id(existing_unique).is_some());
    assert_eq!(
        game.unit_by_id(conflicting_unique).map(|unit| unit.owner),
        Some(1),
        "a conflicting GameLoss unit is bounced, not transferred or destroyed"
    );
    assert_ne!(
        game.unit_by_id(conflicting_unique)
            .map(|unit| (unit.x, unit.y)),
        Some(center)
    );
    assert!(game.units.iter().all(|unit| {
        !(unit.owner == HUMAN && unit.kind == UnitKind::Leader && unit.id != existing_unique)
    }));
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
        .freeciv_square_radius(foreign_city.0, foreign_city.1, 2)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    bounce_area.extend(
        game.map_wrap
            .civ3_radius(foreign_city.0, foreign_city.1, 2)
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
        .freeciv_square_radius(foreign_city.0, foreign_city.1, 2)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    bounce_area.extend(
        game.map_wrap
            .civ3_radius(foreign_city.0, foreign_city.1, 2)
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

#[test]
fn conquest_city_transfer_bounce_uses_freeciv_order_and_preserves_unit_id() {
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
    let freeciv_destinations = game
        .map_wrap
        .freeciv_square_radius(center.0, center.1, 2)
        .into_iter()
        .filter(|&(x, y)| (x, y) != center)
        .filter(|&(x, y)| game.unit_can_survive_at_tile(&bounced, x, y))
        .filter(|&(x, y)| allied_destination(&game, x, y))
        .collect::<Vec<_>>();
    let old_civ3_destinations = game
        .map_wrap
        .civ3_radius(center.0, center.1, 2)
        .into_iter()
        .filter(|&(_, _, distance)| distance != 0)
        .filter(|&(x, y, _)| game.movement_cost_for(&bounced, x, y).is_some())
        .filter(|&(x, y, _)| allied_destination(&game, x, y))
        .map(|(x, y, _)| (x, y))
        .collect::<Vec<_>>();
    assert!(!freeciv_destinations.is_empty());
    assert!(!old_civ3_destinations.is_empty());

    // Choose a stable seed that distinguishes Freeciv's survivability and
    // square-iteration results from the former Civ III radius/movement-cost
    // candidates, so this catches routing back through the old helper.
    let (seed, expected_destination) = (1..=1024)
        .find_map(|seed| {
            let mut freeciv_rng = game.clone();
            freeciv_rng.set_random_seed(seed);
            let freeciv_destination = freeciv_destinations
                [freeciv_rng.rand_below(freeciv_destinations.len() as i32) as usize];

            let mut old_rng = game.clone();
            old_rng.set_random_seed(seed);
            let old_destination = old_civ3_destinations
                [old_rng.rand_below(old_civ3_destinations.len() as i32) as usize];
            (freeciv_destination != old_destination).then_some((seed, freeciv_destination))
        })
        .expect("some seed distinguishes Freeciv and Civ III bounce iteration order");

    game.set_random_seed(seed);
    let mut expected_rng = game.clone();
    expected_rng.rand_below(freeciv_destinations.len() as i32);
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
    game.cities[0]
        .buildings
        .extend([Building::AQUEDUCT, Building::SEWER_SYSTEM]);

    let mut growth_sizes = vec![game.cities[0].size];
    for _ in 0..100 {
        let city = game.cities[0].clone();
        let limit = game.city_size_limit(&city);
        let growth_retained = game.growth_food_retained(&city);
        let shrink_retained = game.shrink_food_retained(&city);
        if game.cities[0].resolve_growth(
            game.map_wrap,
            10,
            limit,
            growth_retained,
            shrink_retained,
            false,
            false,
        ) == Growth::Grew
        {
            growth_sizes.push(game.cities[0].size);
        }
        if game.cities[0].size == 17 {
            break;
        }
    }
    assert_eq!(growth_sizes, (1..=17).collect::<Vec<_>>());
    assert_eq!(game.cities[0].size, 17);
}

#[test]
fn c2c3_city_growth_limits_come_from_size_effects() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.cities.clear();
    game.create_city(HUMAN, 10, 20);
    assert_eq!(game.city_size_limit(&game.cities[0]), 8);
    game.cities[0].buildings.push(Building::AQUEDUCT);
    assert_eq!(game.city_size_limit(&game.cities[0]), 16);
    game.cities[0].buildings.push(Building::SEWER_SYSTEM);
    assert_eq!(game.city_size_limit(&game.cities[0]), i32::MAX);
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
