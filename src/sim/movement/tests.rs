//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn activity_completion_is_immediate_and_old_mp_controls_only_work_count() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.activity_completion_order_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn next_phase_refills_use_live_mountain_movement_effects() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let tile = game.tile_mut(10, 24);
    tile.terrain = Terrain::Mountains;
    tile.road = false;
    tile.river = false;
    let id = game.spawn_unit(HUMAN, UnitKind::Chariot, 10, 24);
    game.units.last_mut().unwrap().moves_left = 0;
    game.units.last_mut().unwrap().moved = true;
    game.progress_player_unit_activities(HUMAN);
    let unit = game.unit_by_id(id).unwrap();
    assert_eq!(
        unit.moves_left,
        UnitKind::Chariot.stats().moves * MOVE_FRAGMENTS - MOVE_FRAGMENTS
    );
    assert!(
        unit.moved,
        "begin-phase MP restoration must not clear end-phase movement"
    );
}

#[test]
fn end_phase_repairs_precede_city_builds_and_next_phase_activities() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.unit_turn_lifecycle_playtest(),
        (true, true, true, true)
    );
}

#[test]
fn native_movement_costs_nativity_boarding_and_zoc_flags() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(game.native_movement_rules_playtest(), (true, true, true));
}

#[test]
fn big_land_has_no_terrain_defense_and_nuclear_is_not_air_attacker() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.cities.clear();
    let p = (10, 24);
    game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    game.tile_mut(p.0, p.1).base = None;
    for kind in [UnitKind::Catapult, UnitKind::Chariot, UnitKind::Cannon] {
        let unit = game.make_unit(0, HUMAN, kind, p.0, p.1);
        let plain = game.defense_power_against(&unit, p.0, p.1, UnitKind::Warrior);
        game.tile_mut(p.0, p.1).terrain = Terrain::Hills;
        assert_eq!(
            game.defense_power_against(&unit, p.0, p.1, UnitKind::Warrior),
            plain
        );
        game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    }
    let aegis = game.make_unit(0, HUMAN, UnitKind::AegisCruiser, p.0, p.1);
    let nuclear = game.defense_power_against(&aegis, p.0, p.1, UnitKind::Nuclear);
    assert_eq!(
        game.defense_power_against(&aegis, p.0, p.1, UnitKind::Bomber),
        nuclear * 5
    );
}

#[test]
fn mountain_movement_effect_and_river_defense_follow_native_classes() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.cities.clear();
    let p = (10, 24);
    let tile = game.tile_mut(p.0, p.1);
    tile.terrain = Terrain::Mountains;
    tile.road = false;
    tile.river = false;
    tile.base = None;
    let horse = game.make_unit(0, HUMAN, UnitKind::Horseman, p.0, p.1);
    assert_eq!(game.unit_move_rate(&horse), 6);
    game.tile_mut(p.0, p.1).river = true;
    assert_eq!(game.unit_move_rate(&horse), 12);
    let chariot = game.make_unit(0, HUMAN, UnitKind::Chariot, p.0, p.1);
    assert_eq!(game.unit_move_rate(&chariot), 6);
    game.tile_mut(p.0, p.1).road = true;
    assert_eq!(game.unit_move_rate(&chariot), 12);
    game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
    game.tile_mut(p.0, p.1).river = false;
    let legion = game.make_unit(0, HUMAN, UnitKind::Legion, p.0, p.1);
    let plain = game.defense_power_against(&legion, p.0, p.1, UnitKind::Warrior);
    game.tile_mut(p.0, p.1).river = true;
    assert_eq!(
        game.defense_power_against(&legion, p.0, p.1, UnitKind::Warrior),
        plain + plain / 4
    );
    assert_eq!(
        game.defense_power_against(&chariot, p.0, p.1, UnitKind::Warrior),
        chariot.kind.stats().defense * POWER_FACTOR
    );
}

#[test]
fn civilian_peace_borders_follow_ruleset_flags_and_still_exclude_foreign_stacks() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.civilian_border_playtest());
}

#[test]
fn movement_rates_scale_damage_before_effects_and_use_whole_move_class_minima() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.damaged_movement_playtest());
    game.players[HUMAN]
        .known
        .retain(|&tech| tech != Tech::NUCLEAR_POWER);
    let mut horse = game.make_unit(0, HUMAN, UnitKind::Horseman, 10, 24);
    horse.hp = 5;
    assert_eq!(game.unit_move_rate(&horse), 6);
    horse.hp = 1;
    assert_eq!(game.unit_move_rate(&horse), 6);
    let mut ship = game.make_unit(0, HUMAN, UnitKind::Destroyer, 10, 24);
    ship.hp = 1;
    assert_eq!(game.unit_move_rate(&ship), 12);
    ship.hp = 15;
    assert_eq!(game.unit_move_rate(&ship), 18);
    game.players[HUMAN].known.push(Tech::NUCLEAR_POWER);
    assert_eq!(game.unit_move_rate(&ship), 24);
    for kind in [UnitKind::Freight, UnitKind::Bomber] {
        let mut unit = game.make_unit(0, HUMAN, kind, 10, 24);
        let healthy = game.unit_move_rate(&unit);
        unit.hp = 1;
        assert_eq!(game.unit_move_rate(&unit), healthy);
    }
}

#[test]
fn wounded_ground_and_sea_units_restore_the_same_rates_used_by_pathfinding() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Ocean;
    game.tile_mut(12, 24).terrain = Terrain::Grassland;
    let ship = game.spawn_unit(HUMAN, UnitKind::Destroyer, 10, 24);
    let horse = game.spawn_unit(HUMAN, UnitKind::Horseman, 12, 24);
    for unit in &mut game.units {
        unit.hp = 1;
        unit.moves_left = 0;
        unit.moved = true;
    }
    game.update_activities_and_restore();
    for (id, rate) in [(ship, 12), (horse, 6)] {
        let unit = game.unit_by_id(id).unwrap();
        assert_eq!(unit.moves_left, rate);
        assert_eq!(unit.moves_left, game.unit_move_rate(unit));
    }
}

#[test]
fn newly_created_triremes_get_lighthouse_speed_in_their_actual_location() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.cities.clear();
    game.create_city(HUMAN, 10, 24);
    game.cities[0].buildings.push(Building::LIGHTHOUSE);
    game.tile_mut(12, 24).terrain = Terrain::Ocean;
    game.tile_mut(12, 24).river = false;
    let id = game.spawn_unit(HUMAN, UnitKind::Trireme, 12, 24);
    let unit = game.unit_by_id(id).unwrap();
    assert_eq!(unit.moves_left, 24);
    assert_eq!(unit.moves_left, game.unit_move_rate(unit));
}

#[test]
fn capture_requires_an_unescorted_stack_and_spends_one_move_without_advancing() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let (actor, merchant, _) = game.merchant_capture_playtest_setup();
    let (x, y) = offset_native_by_map(10, 20, 1, 0);
    let escort = game.spawn_unit(1, UnitKind::Phalanx, x, y);
    let warrior = game
        .units
        .iter()
        .find(|unit| unit.id == actor)
        .unwrap()
        .clone();
    assert!(!game.try_capture_units(&warrior, x, y));
    game.remove_units_with_cargo(vec![escort]);
    game.spawn_unit(1, UnitKind::Caravan, x, y);
    assert!(game.apply(Command::Move(actor, 1, 0)));
    let warrior = game.unit_by_id(actor).unwrap();
    assert_eq!((warrior.x, warrior.y, warrior.moves_left), (10, 20, 0));
    assert!(!game.units.iter().any(|unit| unit.id == merchant));
    assert_eq!(
        game.units
            .iter()
            .filter(|unit| unit.owner == HUMAN
                && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight))
            .count(),
        2
    );
}

#[test]
fn workers_pool_matching_progress_only_when_they_have_movement() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let (first, second) = game.cooperative_work_playtest_setup();
    for id in [first, second] {
        assert!(game.apply(Command::Work(id, WorkKind::Irrigation)));
    }
    assert!(game.apply(Command::Wait(first)));
    game.update_activities_and_restore();
    let idle = game.unit_by_id(first).unwrap();
    assert_eq!(idle.work_done, 0);
    assert_eq!(game.project_turns_left(idle), 2);
    game.update_activities_and_restore();
    assert!(!game.tile(10, 20).irrigation);
    game.update_activities_and_restore();
    assert!(game.tile(10, 20).irrigation);
    assert!(
        game.units
            .iter()
            .filter(|unit| [first, second].contains(&unit.id))
            .all(|unit| unit.work_kind.is_none() && unit.work_done == 0)
    );
}

#[test]
fn pathfinding_routes_around_control_and_friendly_stacks_allow_entry() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
    }
    let (warrior, _, (x, y)) = game.zone_control_playtest_setup();
    assert!(!game.apply(Command::Move(warrior, 1, 0)));
    let detour = game.path_to(warrior, x, y);
    assert!(detour.len() > 1);
    assert_ne!(detour[0], (1, 0));
    game.spawn_unit(HUMAN, UnitKind::Worker, x, y);
    assert_eq!(game.path_to(warrior, x, y), [(1, 0)]);
    assert!(game.apply(Command::Move(warrior, 1, 0)));
}

#[test]
fn a_winning_attacker_cannot_occupy_a_city_with_a_second_defender() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = (12, 12);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.create_city(1, nx, ny);
    let attacker = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
    let first = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    let second = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    game.units
        .iter_mut()
        .find(|unit| unit.id == first)
        .unwrap()
        .hp = 1;
    game.resolve_combat(attacker, first, nx, ny);
    assert!(!game.units.iter().any(|unit| unit.id == first));
    assert!(game.units.iter().any(|unit| unit.id == second));
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == attacker && (unit.x, unit.y) == (x, y))
    );
    assert!(
        game.cities
            .iter()
            .any(|city| (city.x, city.y, city.owner) == (nx, ny, 1))
    );
    game.units
        .iter_mut()
        .find(|unit| unit.id == second)
        .unwrap()
        .hp = 1;
    game.units
        .iter_mut()
        .find(|unit| unit.id == attacker)
        .unwrap()
        .moves_left = MOVE_FRAGMENTS;
    game.resolve_combat(attacker, second, nx, ny);
    assert!(
        game.cities
            .iter()
            .any(|city| (city.x, city.y, city.owner) == (nx, ny, HUMAN))
    );
}

#[test]
fn an_unprotected_stack_loses_hostile_units_with_its_defender() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = (12, 12);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    let attacker = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
    let first = game.spawn_unit(1, UnitKind::Warrior, nx, ny);
    let second = game.spawn_unit(1, UnitKind::Worker, nx, ny);
    game.units
        .iter_mut()
        .find(|unit| unit.id == first)
        .unwrap()
        .hp = 1;
    game.resolve_combat(attacker, first, nx, ny);
    assert!(
        !game
            .units
            .iter()
            .any(|unit| unit.id == first || unit.id == second)
    );
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == attacker && (unit.x, unit.y) == (nx, ny))
    );
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn rescue_prefers_idle_carriers_then_uses_future_speed_to_break_ties() {
        for fast_ship_working in [false, true] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            game.units.clear();
            let (x, y) = (10, 20);
            game.tile_mut(x, y).terrain = Terrain::Ocean;
            let lost = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
            let slow = game.spawn_unit(HUMAN, UnitKind::Caravel, x, y);
            let fast = game.spawn_unit(HUMAN, UnitKind::Transport, x, y);
            let passenger = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            assert!(game.board_specific_ship(passenger, lost));
            for ship in game
                .units
                .iter_mut()
                .filter(|unit| [slow, fast].contains(&unit.id))
            {
                ship.moves_left = MOVE_FRAGMENTS;
                if ship.id == fast && fast_ship_working {
                    ship.work_kind = Some(WorkKind::Clean(Contamination::Fallout));
                    ship.work_left = 1;
                }
            }

            game.remove_units_with_cargo(vec![lost]);

            let expected = if fast_ship_working { slow } else { fast };
            assert_eq!(
                game.units
                    .iter()
                    .find(|unit| unit.id == passenger)
                    .unwrap()
                    .transport,
                Some(expected)
            );
        }
    }

    #[test]
    fn engineering_replaces_earlier_ferries_and_transport_carries_heavy_units() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Ocean;
        game.create_city(HUMAN, x, y);
        let id = game.cities[0].id;
        let caravel = Production::Unit(UnitKind::Caravel);
        let transport = Production::Unit(UnitKind::Transport);
        game.players[HUMAN].known.push(Tech::MAGNETISM);
        assert!(game.apply(Command::Produce(id, caravel)));
        assert!(!game.apply(Command::Produce(id, transport)));
        game.players[HUMAN].known.push(Tech::ENGINEERING);
        assert!(!game.apply(Command::Produce(id, caravel)));
        game.cities[0].shields = transport.cost();
        game.process_cities();
        let ship = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Transport)
            .unwrap()
            .id;
        assert!(game.apply(Command::Move(ship, 1, 0)));
        for _ in 0..8 {
            let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
            assert!(game.apply(Command::Move(cannon, 1, 0)));
        }
        let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        assert!(!game.apply(Command::Move(cannon, 1, 0)));
        assert_eq!(game.cargo_count(ship), 8);
        game.tile_mut(sx, sy).fallout = true;
        assert!(game.apply(Command::Work(ship, WorkKind::Clean(Contamination::Fallout))));
        for _ in 0..3 {
            game.update_activities_and_restore();
        }
        assert!(!game.tile(sx, sy).fallout);
        assert_eq!(game.cargo_count(ship), 8);
    }

    #[test]
    fn ports_build_transports_only_with_their_tech_and_upgrade_obsolete_production() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        for (dx, dy) in ISOHEX_DIRS {
            let (tx, ty) = offset_native_by_map(x, y, dx, dy);
            game.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, x, y);
        let id = game.cities.last().unwrap().id;
        let caravel = Production::Unit(UnitKind::Caravel);
        let galleon = Production::Unit(UnitKind::Galleon);
        game.players[HUMAN].known.push(Tech::MAGNETISM);
        assert!(!game.apply(Command::Produce(id, caravel)));
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(sx, sy).terrain = Terrain::Lake;
        assert!(game.apply(Command::Produce(id, caravel)));
        assert!(!game.apply(Command::Produce(id, galleon)));
        game.players[HUMAN].known.push(Tech::NAVIGATION);
        assert!(!game.apply(Command::Produce(id, caravel)));
        game.cities.last_mut().unwrap().shields = galleon.cost();
        game.process_cities();
        assert!(
            game.units
                .iter()
                .any(|unit| { unit.kind == UnitKind::Galleon && unit.home_city == Some(id) })
        );
    }

    #[test]
    fn adjacent_embark_respects_cargo_class_and_capacity() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        let (sx, sy) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(sx, sy).terrain = Terrain::Ocean;
        let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, sx, sy);
        let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, x, y);
        assert!(!game.apply(Command::Move(cannon, 1, 0)));
        for _ in 0..3 {
            let cargo = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
            assert!(game.apply(Command::Move(cargo, 1, 0)));
            assert!(!game.apply(Command::BoardOrDeboard(cargo)));
        }
        assert_eq!(game.cargo_count(ship), 3);
        let cargo = game.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
        assert!(!game.apply(Command::Move(cargo, 1, 0)));
    }
}
