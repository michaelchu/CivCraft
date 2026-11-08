//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn submarines_refuel_missiles_and_exclude_other_cargo() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (submarine, missile, _, doubled) = game.submarine_playtest_setup();
    assert!(doubled);
    assert_eq!(
        UnitKind::Submarine.required_tech().unwrap().name(),
        "Mass Production"
    );
    assert!(!UnitKind::Submarine.can_carry(UnitKind::Fighter));
    assert!(!UnitKind::Submarine.can_carry(UnitKind::Warrior));
    assert!(UnitKind::Submarine.can_carry(UnitKind::Nuclear));
    assert!(game.board_or_deboard(missile));
    game.restore_aircraft();
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == missile && unit.transport == Some(submarine) && unit.fuel == 1)
    );
}

#[test]
fn airlift_uses_airport_capacity_and_resets_next_turn() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let (first, second, destination) = game.airlift_playtest_setup();
    assert!(game.apply(Command::Airlift(first, destination)));
    assert!(!game.apply(Command::Airlift(second, destination)));
    assert!(game.apply(Command::EndTurn));
    assert!(game.apply(Command::Airlift(second, destination)));
}

#[test]
fn cruise_missiles_get_city_firepower_and_face_sdi_instead_of_sam() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (bomber, defender) = game.sam_battery_playtest_setup(true);
    let attacker = game
        .units
        .iter_mut()
        .find(|unit| unit.id == bomber)
        .unwrap();
    attacker.kind = UnitKind::CruiseMissile;
    let attacker = attacker.clone();
    let target = game
        .units
        .iter()
        .find(|unit| unit.id == defender)
        .unwrap()
        .clone();
    let defense =
        |game: &Simulation| game.defense_power_against(&target, target.x, target.y, attacker.kind);
    assert_eq!(defense(&game), 45);
    game.cities
        .last_mut()
        .unwrap()
        .buildings
        .push(Building::SDI_DEFENSE);
    assert_eq!(defense(&game), 90);
    assert_eq!(game.combat_firepower(&attacker, &target), (2, 1));
    assert_eq!(attacker.kind.fuel_capacity(), 1);
}

#[test]
fn aegis_cruisers_defend_against_air_attackers_and_sail_without_cargo() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (_, id) = game.aegis_playtest_setup();
    let cruiser = game.unit_by_id(id).unwrap();
    let (x, y) = (cruiser.x, cruiser.y);
    let defense = |attacker| game.defense_power_against(cruiser, x, y, attacker);
    assert_eq!(defense(UnitKind::Caravel), 80);
    assert_eq!(defense(UnitKind::Fighter), 400);
    assert_eq!(defense(UnitKind::Helicopter), 400);
    assert_eq!(cruiser.kind.transport_capacity(), 0);
    assert_eq!(cruiser.kind.required_tech(), Some(Tech::ROBOTICS));
    assert!(!UnitKind::Transport.can_carry(cruiser.kind));
    assert_eq!(game.movement_cost_for(cruiser, x, y), Some(MOVE_FRAGMENTS));
    let (land_x, land_y) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(land_x, land_y).terrain = Terrain::Grassland;
    let cruiser = game.unit_by_id(id).unwrap();
    assert_eq!(game.movement_cost_for(cruiser, land_x, land_y), None);
}

#[test]
fn city_defenses_use_the_attacking_units_class() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (_, defender_id) = game.sam_battery_playtest_setup(true);
    let defender = game
        .units
        .iter()
        .find(|unit| unit.id == defender_id)
        .unwrap()
        .clone();
    let city = game.cities.last_mut().unwrap();
    city.buildings.extend([
        Building::COASTAL_DEFENSE,
        Building::CITY_WALLS,
        Building::GREAT_WALL,
    ]);

    let defense = |game: &Simulation, attacker| {
        game.defense_power_against(&defender, defender.x, defender.y, attacker)
    };
    assert_eq!(defense(&game, UnitKind::Bomber), 90);
    assert_eq!(defense(&game, UnitKind::Helicopter), 90);
    assert_eq!(defense(&game, UnitKind::Caravel), 90);
    assert_eq!(defense(&game, UnitKind::Musketeers), 135);
    assert_eq!(defense(&game, UnitKind::Musketeers), 135);
}

#[test]
fn helicopters_carry_field_capable_troops_and_crash_only_when_health_runs_out() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = (10, 20);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    for (tx, ty) in [(x, y), (nx, ny)] {
        let tile = game.tile_mut(tx, ty);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.hut = false;
    }
    let helicopter = game.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
    let musketeer = game.spawn_unit(HUMAN, UnitKind::Musketeers, x, y);
    let partisan = game.spawn_unit(HUMAN, UnitKind::Partisan, x, y);
    assert!(!game.board_or_deboard(musketeer));
    assert!(game.board_or_deboard(partisan));
    assert!(game.apply(Command::Move(helicopter, 1, 0)));
    assert!(game.board_or_deboard(partisan));
    assert!(game.units.iter().any(|unit| unit.id == partisan
        && (unit.x, unit.y) == (nx, ny)
        && unit.transport.is_none()));
    assert!(game.board_or_deboard(partisan));
    for _ in 0..9 {
        game.restore_aircraft();
    }
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == helicopter && unit.hp == 3)
    );
    game.restore_aircraft();
    assert!(!game.units.iter().any(|unit| unit.id == helicopter));
    assert!(game.units.iter().any(|unit| unit.id == partisan
        && (unit.x, unit.y) == (nx, ny)
        && unit.transport.is_none()));
}

#[test]
fn bombardment_is_nonlethal_and_does_not_move_the_bomber() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (bomber, targets) = game.bombard_playtest_setup();
    let first = game
        .units
        .iter_mut()
        .find(|unit| unit.id == targets[0])
        .unwrap();
    first.hp = 2;
    assert!(game.apply(Command::Move(bomber, 1, 0)));
    assert_eq!(
        game.units
            .iter()
            .find(|unit| unit.id == targets[0])
            .unwrap()
            .hp,
        1
    );
    let aircraft = game.unit_by_id(bomber).unwrap();
    assert_eq!(
        (aircraft.x, aircraft.y, aircraft.hp, aircraft.moves_left),
        (10, 20, 30, 0)
    );
}

#[test]
fn aircraft_can_return_to_airstrips_or_crash_when_out_of_fuel() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = (10, 20);
    let (nx, ny) = offset_native_by_map(x, y, -1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).base = Some(Base::Airstrip);
    let first = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
    let second = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
    for unit in game
        .units
        .iter_mut()
        .filter(|unit| [first, second].contains(&unit.id))
    {
        unit.fuel = 1;
        if unit.id == second {
            unit.moves_left = 0;
        }
    }
    game.refresh_visibility(); // Newly placed human aircraft see the nearby airstrip.
    game.restore_aircraft();
    let saved = game.unit_by_id(first).unwrap();
    assert_eq!((saved.x, saved.y, saved.fuel), (nx, ny, 3));
    assert!(!game.units.iter().any(|unit| unit.id == second));
}

#[cfg(test)]
mod fighter_tests {
    use super::*;

    #[test]
    fn fighter_can_attack_an_airborne_bomber_and_refuels_to_two_turns() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (fighter, bomber) = game.fighter_playtest_setup();
        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        assert!(!game.apply(Command::Move(warrior, 1, 0)));
        assert!(game.apply(Command::Move(fighter, 1, 0)));
        assert!(!game.units.iter().any(|unit| unit.id == bomber));
        let aircraft = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        assert_eq!(aircraft.fuel, 2);
        aircraft.fuel = 1;
        game.tile_mut(10, 20).base = Some(Base::Airbase);
        game.restore_aircraft();
        let aircraft = game.unit_by_id(fighter).unwrap();
        assert_eq!(aircraft.fuel, 2);
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;

    #[test]
    fn carrier_production_and_emergency_landing_follow_aircraft_cargo_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (carrier, fighter) = game.carrier_playtest_setup();
        let port = Production::Unit(UnitKind::Carrier);
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(HUMAN, x, y);
        assert!(!game.can_produce(&game.cities[0], port));
        game.players[HUMAN].known.push(Tech::MASS_PRODUCTION);
        assert!(game.can_produce(&game.cities[0], port));
        game.cities.clear();
        assert!(UnitKind::Carrier.can_carry(UnitKind::Warrior));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Cannon));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Worker));
        assert!(!UnitKind::Transport.can_carry(UnitKind::Fighter));
        game.restore_aircraft();
        let saved = game.unit_by_id(fighter).unwrap();
        assert_eq!((saved.transport, saved.fuel), (Some(carrier), 2));
    }
}

#[cfg(test)]
mod stealth_tests {
    use super::*;

    #[test]
    fn desktop_airbase_sortie_fixture_uses_fighter_and_preserves_safe_return() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_bomber_playtest(), (true, true, true));
    }

    fn missile_hunt(distance: i32, kind: UnitKind) -> (Simulation, u32, u32, u32) {
        loaded_hunt(distance, kind, UnitKind::Submarine)
    }

    fn loaded_hunt(
        distance: i32,
        kind: UnitKind,
        platform: UnitKind,
    ) -> (Simulation, u32, u32, u32) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.sign_pact(1, HUMAN, Pact::War);
        game.players[1]
            .known
            .extend([Tech::ADVANCED_FLIGHT, Tech::ROCKETRY]);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        let boat = game.spawn_unit(1, platform, 10, 20);
        let missile = game.spawn_unit(1, kind, 10, 20);
        assert!(game.board_specific_ship(missile, boat));
        let target = offset_native_by_map(10, 20, distance, 0);
        let victim = game.spawn_unit(HUMAN, UnitKind::Caravel, target.0, target.1);
        game.units.last_mut().unwrap().hp = 1;
        game.set_random_seed(4);
        (game, boat, missile, victim)
    }

    #[test]
    fn hunter_launch_preserves_transport_pf_and_divided_missile_budget() {
        let (mut game, boat, missile, victim) = missile_hunt(3, UnitKind::CruiseMissile);
        game.source_hunter_launch(boat, victim);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == missile && unit.transport == Some(boat))
        );
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::CruiseMissile);
        game.source_hunter_launch(boat, victim);
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
    }

    #[test]
    fn desktop_native_hunter_launch_fixture_exercises_range_and_nuclear_cargo() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_source_hunter_launch_playtest(), (true, true, true));
    }

    #[test]
    fn carrier_bomber_boss_holds_fighter_reserve_instead_of_independent_cargo_dispatch() {
        let (mut game, boat, bomber, victim) = loaded_hunt(2, UnitKind::Bomber, UnitKind::Carrier);
        let fighter = game.spawn_unit(1, UnitKind::Fighter, 10, 20);
        assert!(game.board_specific_ship(fighter, boat));
        game.run_ai_player(1);
        assert_eq!(game.ferry_boss(boat), Some(bomber));
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && !unit.moved && unit.transport == Some(boat))
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == boat && (unit.x, unit.y) == (10, 20))
        );
    }

    #[test]
    fn carrier_without_moves_cannot_activate_its_aircraft_boss() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        game.units
            .iter_mut()
            .find(|unit| unit.id == boat)
            .unwrap()
            .moves_left = 0;
        game.run_ai_player(1);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && !unit.moved)
        );
    }

    #[test]
    fn carrier_does_not_activate_a_distant_reserved_aircraft_without_cargo() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert!(game.board_or_deboard(fighter));
        let remote = offset_native_by_map(10, 20, 5, 0);
        game.tile_mut(remote.0, remote.1).terrain = Terrain::Grassland;
        game.tile_mut(remote.0, remote.1).base = Some(Base::Airstrip);
        let actor = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        (actor.x, actor.y) = remote;
        let actor = actor.clone();
        let target = offset_native_by_map(remote.0, remote.1, 2, 0);
        let enemy = game
            .units
            .iter_mut()
            .find(|unit| unit.id == victim)
            .unwrap();
        (enemy.x, enemy.y) = target;
        assert!(game.aircraft_attack_path(&actor).is_some());
        game.release_ai_ferry(fighter);
        assert!(game.assign_ai_ferry(boat, fighter));
        let ship = game
            .units
            .iter()
            .find(|unit| unit.id == boat)
            .unwrap()
            .clone();
        let (handled, done) = game.run_source_air_ferry(&ship);
        assert!(!handled && done.is_empty());
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.moves_left == actor.moves_left)
        );
    }

    #[test]
    fn empty_air_refuel_path_does_not_embark_until_emergency_rescue() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert!(game.board_or_deboard(fighter));
        let actor = game
            .units
            .iter()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .clone();
        assert_eq!(actor.fuel, 2);
        game.run_ai_strike_aircraft(&actor);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport.is_none()
            && unit.moves_left == actor.moves_left));
        game.restore_aircraft();
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.transport.is_none() && unit.fuel == 1)
        );
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport == Some(boat)
            && unit.fuel == 2
            && unit.hp == actor.hp - 2 * percent_of(actor.kind.stats().hp, 10)));
    }

    #[test]
    fn fuel_free_helicopter_takes_airborne_loss_without_emergency_embark() {
        let (mut game, _boat, helicopter, _victim) =
            loaded_hunt(2, UnitKind::Helicopter, UnitKind::Carrier);
        assert!(game.board_or_deboard(helicopter));
        let before = game
            .units
            .iter()
            .find(|unit| unit.id == helicopter)
            .unwrap()
            .hp;
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == helicopter
            && unit.transport.is_none()
            && unit.fuel == 0
            && unit.hp == before - percent_of(UnitKind::Helicopter.stats().hp, 10)));
    }

    #[test]
    fn hunter_launch_supports_nuclear_cargo_and_cargo_append_order() {
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::CruiseMissile);
        let nuclear = game.spawn_unit(1, UnitKind::Nuclear, 10, 20);
        assert!(game.board_specific_ship(nuclear, boat));
        game.source_hunter_launch(boat, victim);
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == nuclear && unit.transport == Some(boat))
        );
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::Nuclear);
        game.source_hunter_launch(boat, victim);
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
    }

    #[test]
    fn immobile_auxiliary_threat_does_not_preempt_primary_hunter_target() {
        let (mut game, boat, missile, victim) = missile_hunt(2, UnitKind::CruiseMissile);
        let point = offset_native_by_map(10, 20, 0, 1);
        let threat = game.spawn_unit(HUMAN, UnitKind::Battleship, point.0, point.1);
        game.units.last_mut().unwrap().moves_left = 0;
        game.source_hunter_launch(boat, victim);
        assert!(game.units.iter().any(|unit| unit.id == threat));
        assert!(
            game.units
                .iter()
                .all(|unit| unit.id != victim && unit.id != missile)
        );
    }

    #[test]
    fn desktop_carrier_sortie_fixture_uses_fighter_and_recovers_aboard() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_carrier_sortie_playtest(false), (true, true, true));
    }

    #[test]
    fn desktop_carrier_does_not_reposition_to_extend_its_aircraft_sortie() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.ai_carrier_sortie_playtest(true), (true, true, true));
    }

    #[test]
    fn fuel_mission_fixture_exercises_sortie_wait_and_literal_oneattack_limit() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_fuel_mission_playtest());
    }

    #[test]
    fn default_ai_air_target_pf_is_omniscient_without_map_handicap() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let target = game.unit_by_id(victim).unwrap();
        let point = (target.x, target.y);
        game.tile_mut(point.0, point.1).known_by[plane.owner] = false;
        assert_eq!(
            game.aircraft_attack_path(&plane).map(|path| path.1),
            Some(point)
        );
    }

    #[test]
    fn known_refuel_extras_work_on_enemy_owned_tiles() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let point = (10, 20);
        game.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
        game.tile_mut(point.0, point.1).base = Some(Base::Airstrip);
        game.tile_mut(point.0, point.1).owner = Some(HUMAN);
        game.tile_mut(point.0, point.1).known_by[1] = true;
        game.sign_pact(1, HUMAN, Pact::War);
        let id = game.spawn_unit(1, UnitKind::Fighter, point.0, point.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .fuel = 1;
        assert!(game.refuel_site(1, point.0, point.1));
        game.restore_aircraft();
        assert_eq!(game.unit_by_id(id).unwrap().fuel, 2);
        game.tile_mut(point.0, point.1).known_by[1] = false;
        assert!(!game.refuel_site(1, point.0, point.1));
    }

    #[test]
    fn stealth_aircraft_replace_older_models_and_need_close_detection() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (explorer, hidden) = game.stealth_playtest_setup();
        game.units.last_mut().unwrap().kind = UnitKind::StealthFighter;
        assert!(!game.unit_visible_to_human(game.units.last().unwrap()));
        assert!(game.apply(Command::Move(explorer, 1, 0)));
        let aircraft = game.unit_by_id(hidden).unwrap();
        assert!(game.unit_visible_to_human(aircraft));

        game.create_city(HUMAN, 10, 20);
        let city = &game.cities[0];
        game.players[HUMAN]
            .known
            .extend([Tech::FLIGHT, Tech::ADVANCED_FLIGHT, Tech::STEALTH]);
        for (old, new) in [
            (UnitKind::Fighter, UnitKind::StealthFighter),
            (UnitKind::Bomber, UnitKind::StealthBomber),
        ] {
            assert!(!game.can_produce(city, Production::Unit(old)));
            assert!(game.can_produce(city, Production::Unit(new)));
            assert_eq!(game.production_successor(city, old), Some(new));
            assert_eq!(new.fuel_capacity(), old.fuel_capacity());
        }
    }
    #[test]
    fn existing_aircraft_attack_profitable_targets_but_hp_loss_blocks_offensive_builds() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.unit_by_id(id).unwrap().clone();
        let target = game.unit_by_id(victim).unwrap().clone();
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Fighter));
        assert_eq!(UnitKind::Fighter.hp_loss_percent(), 10);
        assert!(game.aircraft_target_value(&plane, target.x, target.y) > 0.0);
        assert!(game.aircraft_production_choice(&game.cities[0]).is_none());
        game.players[1].known.push(Tech::MEDICINE);
        game.cities[0].buildings.push(Building::AIRPORT);
        assert!(game.aircraft_production_choice(&game.cities[0]).is_none());
        game.run_ai_strike_aircraft(&plane);
        assert!(!game.units.iter().any(|u| u.id == victim));
        game.players[1].known.clear();
        assert!(game.aircraft_production_choice(&game.cities[0]).is_none());
    }

    #[test]
    fn airborne_plane_refuels_before_attacking_and_waits_for_full_fuel() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let home = (game.cities[0].x, game.cities[0].y);
        let open = offset_native_by_map(home.0, home.1, 1, 0);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        (plane.x, plane.y) = open;
        plane.fuel = 1;
        let snapshot = plane.clone();
        game.run_ai_strike_aircraft(&snapshot);
        let plane = game.unit_by_id(id).unwrap().clone();
        assert_eq!((plane.x, plane.y), home);
        assert!(game.units.iter().any(|u| u.id == victim));
        game.run_ai_strike_aircraft(&plane);
        assert!(game.units.iter().any(|u| u.id == victim));
    }

    #[test]
    fn standalone_nuclear_uses_native_air_manager_strict_attack_cutoff() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (fighter, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != fighter);
        let city = game.cities[0].clone();
        let target = game.unit_by_id(victim).unwrap().clone();
        let id = game.spawn_unit(1, UnitKind::Nuclear, city.x, city.y);
        let plane = game.unit_by_id(id).unwrap().clone();
        game.units.iter_mut().find(|u| u.id == victim).unwrap().kind = UnitKind::Warrior;
        assert!(game.aircraft_best_target(&plane).is_none());
        for _ in 0..4 {
            game.spawn_unit(HUMAN, UnitKind::Battleship, target.x, target.y);
        }
        assert!(game.aircraft_target_value(&plane, target.x, target.y) > 0.0);
        let mut map = super::super::fuel_paths::FuelPaths::new(&game, &plane, true).unwrap();
        assert_eq!(
            map.movement_cost(&game, target.x, target.y),
            Some(plane.moves_left)
        );
        assert!(game.aircraft_best_target(&plane).is_none());
        let mut short_range = plane.clone();
        short_range.moves_left = MOVE_FRAGMENTS;
        assert!(game.aircraft_best_target(&short_range).is_none());
        game.run_ai_nuclear_aircraft(&plane);
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == id && unit.moves_left == plane.moves_left)
        );
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(game.ai_nuclear_advisor_playtest());
    }
    #[test]
    fn aircraft_rebases_toward_targets_outside_its_current_sortie() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.unit_by_id(id).unwrap().clone();
        let base = offset_native_by_map(plane.x, plane.y, 6, 0);
        let target = offset_native_by_map(plane.x, plane.y, 12, 0);
        game.tile_mut(base.0, base.1).terrain = Terrain::Grassland;
        game.tile_mut(base.0, base.1).base = Some(Base::Airbase);
        game.tile_mut(base.0, base.1).owner = Some(1);
        let enemy = game.units.iter_mut().find(|u| u.id == victim).unwrap();
        (enemy.x, enemy.y) = target;
        game.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        assert!(game.aircraft_best_target(&plane).is_none());
        game.run_ai_strike_aircraft(&plane);
        let plane = game.unit_by_id(id).unwrap();
        assert_eq!((plane.x, plane.y), base);
        assert!(game.units.iter().any(|u| u.id == victim));
    }
    #[test]
    fn city_air_raids_need_occupiers_but_virtual_production_can_plan_ahead() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.unit_by_id(id).unwrap().clone();
        let enemy = game.units.iter_mut().find(|u| u.id == victim).unwrap();
        enemy.kind = UnitKind::Warrior;
        let target = (enemy.x, enemy.y);
        game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
        game.create_city(HUMAN, target.0, target.1);
        assert_eq!(game.aircraft_target_value(&plane, target.0, target.1), 0.0);
        let mut virtual_plane = plane.clone();
        virtual_plane.id = 0;
        assert!(game.aircraft_target_value(&virtual_plane, target.0, target.1) > 0.0);
        let occupier = game.spawn_unit(1, UnitKind::Warrior, plane.x, plane.y);
        // Nearby troops alone do not support bombing a defended city.
        assert_eq!(game.aircraft_target_value(&plane, target.0, target.1), 0.0);
        let city = game
            .cities
            .iter()
            .find(|c| (c.x, c.y) == target)
            .unwrap()
            .id;
        game.ai_ground_targets.insert(occupier, city);
        assert!(game.aircraft_target_value(&plane, target.0, target.1) > 0.0);
    }
    #[test]
    fn damaged_aircraft_prefers_fast_repair_but_finishes_imminent_home_repair() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let home = (game.cities[0].x, game.cities[0].y);
        let airport = offset_native_by_map(home.0, home.1, 4, 0);
        game.tile_mut(airport.0, airport.1).terrain = Terrain::Grassland;
        game.create_city(1, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        plane.hp = 5;
        plane.moved = true;
        let plane = plane.clone();
        assert!(
            game.aircraft_regen_turns(&plane, airport.0, airport.1)
                < game.aircraft_regen_turns(&plane, home.0, home.1)
        );
        game.run_ai_strike_aircraft(&plane);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        assert_eq!((plane.x, plane.y), airport);
        (plane.x, plane.y) = home;
        plane.hp = plane.kind.stats().hp - 1;
        plane.moved = false;
        plane.moves_left = plane.kind.stats().moves * MOVE_FRAGMENTS;
        let plane = plane.clone();
        assert!(game.strategic_airbase(&plane, &[home, airport]).is_none());
    }

    #[test]
    fn airborne_damaged_plane_selects_repair_time_before_distance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let home = (game.cities[0].x, game.cities[0].y);
        let airport = offset_native_by_map(home.0, home.1, 4, 0);
        game.tile_mut(airport.0, airport.1).terrain = Terrain::Grassland;
        game.create_city(1, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        (plane.x, plane.y) = offset_native_by_map(home.0, home.1, 1, 0);
        plane.hp = 5;
        let plane = plane.clone();
        game.run_ai_strike_aircraft(&plane);
        let plane = game.unit_by_id(id).unwrap();
        assert_eq!((plane.x, plane.y), airport);
    }

    #[test]
    fn strategic_airbase_reinforces_grave_danger_and_holds_its_own_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let plane = game.unit_by_id(id).unwrap().clone();
        let home = (plane.x, plane.y);
        let front = offset_native_by_map(home.0, home.1, 5, 0);
        let attack = offset_native_by_map(front.0, front.1, 1, 0);
        game.tile_mut(front.0, front.1).terrain = Terrain::Grassland;
        game.tile_mut(attack.0, attack.1).terrain = Terrain::Grassland;
        game.create_city(1, front.0, front.1);
        for _ in 0..3 {
            game.spawn_unit(HUMAN, UnitKind::Warrior, attack.0, attack.1);
        }
        assert!(game.aircraft_city_needs_defender(&plane, front.0, front.1, false));
        assert_eq!(
            game.strategic_airbase(&plane, &[home, front]).map(|r| r.1),
            Some(front)
        );
        let mut defender = plane.clone();
        (defender.x, defender.y) = front;
        assert!(game.strategic_airbase(&defender, &[home, front]).is_none());
        for _ in 0..2 {
            game.spawn_unit(1, UnitKind::Warrior, front.0, front.1);
        }
        assert!(!game.aircraft_city_needs_defender(&plane, front.0, front.1, false));
    }
    #[test]
    fn airlift_refreshes_contact_but_stationary_bombardment_does_not() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let source = (10, 20);
        let destination = offset_native_by_map(source.0, source.1, 6, 0);
        game.create_city(HUMAN, source.0, source.1);
        game.create_city(HUMAN, destination.0, destination.1);
        for city in &mut game.cities {
            city.buildings.push(Building::AIRPORT);
        }
        let arrival_city = game.cities[1].id;
        let visitor = game.spawn_unit(HUMAN, UnitKind::Warrior, source.0, source.1);
        let target = offset_native_by_map(destination.0, destination.1, 1, 0);
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.airlift(visitor, arrival_city));
        assert!(
            game.diplomatic_contact_until
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, destination.0, destination.1);
        game.diplomatic_contact_until.clear();
        let aircraft = game.unit_by_id(bomber).unwrap().clone();
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Fought
        );
        assert!(game.diplomatic_contact_until.is_empty());
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomatic_contact_until.clear();
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Blocked
        );
        assert!(game.diplomatic_contact_until.is_empty());
    }

    #[test]
    fn lost_paradrop_establishes_contact_at_the_actual_landing() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let origin = (10, 20);
        let landing = offset_native_by_map(origin.0, origin.1, 5, 0);
        game.create_city(HUMAN, origin.0, origin.1);
        let troop = game.spawn_unit(HUMAN, UnitKind::Paratroopers, origin.0, origin.1);
        game.spawn_unit(1, UnitKind::Warrior, landing.0, landing.1);
        game.tile_mut(landing.0, landing.1).known_by[HUMAN] = true;
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.paradrop(troop, landing.0, landing.1));
        assert!(!game.units.iter().any(|u| u.id == troop));
        assert!(
            game.diplomatic_contact_until
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
    }
}
