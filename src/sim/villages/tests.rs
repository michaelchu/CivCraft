//! Regression tests for the behavior of the parent module.

use super::*;

fn approach(roll: i32) -> (Simulation, u32, (i32, i32)) {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let explorer = game
        .units
        .iter()
        .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Explorer)
        .unwrap();
    let (id, x, y) = (explorer.id, explorer.x, explorer.y);
    let (nx, ny) = offset_native_by_map(x, y, 0, -1);
    game.tile_mut(nx, ny).terrain = Terrain::Plains;
    game.tile_mut(nx, ny).hut = true;
    let seed = (1..)
        .find(|&seed| {
            game.set_random_seed(seed);
            game.rand_below(14) == roll
        })
        .unwrap();
    game.set_random_seed(seed);
    (game, id, (nx, ny))
}

#[test]
fn entering_a_village_resolves_every_reference_roll_once() {
    for roll in 0..14 {
        let (mut game, id, (x, y)) = approach(roll);
        let before = game.units.len();
        let known = game.players[HUMAN].known.len();
        game.players[HUMAN].research = 7;
        assert!(game.apply(Command::Move(id, 0, -1)));
        assert!(!game.tile(x, y).hut);
        match roll {
            0 => assert_eq!(game.players[HUMAN].gold, 75),
            1..=3 => assert_eq!(game.players[HUMAN].gold, 100),
            4 => assert_eq!(game.players[HUMAN].gold, 150),
            5..=7 => {
                assert_eq!(game.players[HUMAN].known.len(), known + 1);
                assert_eq!(game.players[HUMAN].research, 7);
            }
            8..=9 => {
                assert_eq!(game.units.len(), before + 1);
                assert_eq!(game.units.last().unwrap().kind, UnitKind::Archer);
            }
            10 => assert!(game.units.iter().all(|unit| unit.id != id)),
            11 => {
                assert!(game.cities.iter().any(|city| (city.x, city.y) == (x, y)));
                assert!(game.units.iter().any(|unit| unit.id == id));
            }
            _ => {
                let (nx, ny) = offset_native_by_map(x, y, 5, 0);
                let tile = game.tile(nx, ny);
                assert!(tile.seen && tile.known_by[HUMAN] && !tile.visible);
            }
        }
        let events = game.events.clone();
        game.enter_village(id, x, y);
        assert_eq!(game.events, events);
    }
}

#[test]
fn villages_offer_nomads_or_leave_a_city_nearby_unharmed() {
    for roll in [10, 11] {
        let (mut game, id, (x, y)) = approach(roll);
        let (cx, cy) = offset_native_by_map(x, y, 0, 1);
        game.create_city(HUMAN, cx, cy);
        let before = game.units.len();
        assert!(game.apply(Command::Move(id, 0, -1)));
        assert_eq!(game.cities.len(), 1);
        assert!(game.units.iter().any(|unit| unit.id == id));
        assert_eq!(game.units.len(), before + usize::from(roll == 11));
        if roll == 11 {
            assert_eq!(game.units.last().unwrap().kind, UnitKind::Settlers);
        }
    }
}

#[test]
fn late_hostile_village_spawns_land_raiders() {
    let (mut game, id, (x, y)) = approach(10);
    game.turn = BARBARIAN_ONSET;
    for (dx, dy) in ISOHEX_DIRS {
        let (nx, ny) = offset_native_by_map(x, y, dx, dy);
        game.tile_mut(nx, ny).terrain = Terrain::Plains;
    }
    assert!(game.apply(Command::Move(id, 0, -1)));
    let raiders = game.units.iter().filter(|unit| unit.owner == BARBARIANS);
    assert!(
        raiders
            .clone()
            .all(|unit| game.map_distance(x, y, unit.x, unit.y) == 1)
    );
    assert!((3..=6).contains(&raiders.count()));
    assert!(game.units.iter().any(|unit| unit.id == id));
}
#[test]
fn advanced_raiders_require_three_civilizations_with_the_technology() {
    for civilizations in [2, 3] {
        let (mut game, id, (x, y)) = approach(10);
        game.turn = BARBARIAN_ONSET;
        for player in &mut game.players[..civilizations] {
            player.known.push(Tech::METALLURGY);
        }
        for (dx, dy) in ISOHEX_DIRS {
            let (nx, ny) = offset_native_by_map(x, y, dx, dy);
            game.tile_mut(nx, ny).terrain = Terrain::Plains;
        }
        assert!(game.apply(Command::Move(id, 0, -1)));
        let expected = if civilizations == 3 {
            UnitKind::Cannon
        } else {
            UnitKind::Horseman
        };
        let raiders: Vec<_> = game
            .units
            .iter()
            .filter(|unit| unit.owner == BARBARIANS)
            .collect();
        assert!((3..=6).contains(&raiders.len()));
        assert!(raiders.iter().all(|unit| unit.kind == expected));
    }
}

#[test]
fn island_transport_loads_cargo_and_carries_it_to_a_landing() {
    let (mut game, id, (x, y)) = approach(10);
    game.turn = BARBARIAN_ONSET;
    for (dx, dy) in ISOHEX_DIRS {
        let (nx, ny) = offset_native_by_map(x, y, dx, dy);
        game.tile_mut(nx, ny).terrain = if (dx, dy) == (0, 1) {
            Terrain::Plains
        } else {
            Terrain::Sea
        };
    }
    assert!(game.apply(Command::Move(id, 0, -1)));
    let boat = game
        .units
        .iter()
        .find(|unit| unit.kind.transport_capacity() > 0)
        .unwrap()
        .clone();
    let cargo: Vec<_> = game
        .units
        .iter()
        .filter(|unit| unit.transport == Some(boat.id))
        .map(|unit| unit.id)
        .collect();
    assert!(!cargo.is_empty() && cargo.len() <= boat.kind.transport_capacity());
    let stayed = game
        .units
        .iter()
        .any(|unit| unit.owner == BARBARIANS && (unit.x, unit.y) == (x, y));
    assert_eq!(game.units.iter().any(|unit| unit.id == id), !stayed);
    let (sx, sy) = offset_native_by_map(boat.x, boat.y, 1, 0);
    game.tile_mut(sx, sy).terrain = Terrain::Sea;
    assert_eq!(game.try_move_unit(boat.id, 1, 0), MoveResult::Moved);
    for id in cargo {
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        assert_eq!((unit.x, unit.y), (sx, sy));
        unit.moves_left = MOVE_FRAGMENTS;
        let (lx, ly) = offset_native_by_map(sx, sy, 1, 0);
        game.tile_mut(lx, ly).terrain = Terrain::Plains;
        assert_eq!(game.try_move_unit(id, 1, 0), MoveResult::Moved);
        let unit = game.unit_by_id(id).unwrap();
        assert_eq!((unit.x, unit.y, unit.transport), (lx, ly, None));
    }
}
