//! Regression tests for settlement ferries behavior, including edge cases and interactions with the simulation.

use super::*;

#[test]
fn actual_ferry_reselects_two_slots_for_guard_without_changing_city_goal() {
    let mut game = ocean_game();
    assert_eq!(game.ai_guard_ferry_capacity_playtest(), (true, true, true));
}

#[test]
fn founder_can_plan_with_a_partly_loaded_available_ferry_but_not_a_full_one() {
    let mut game = ocean_game();
    let home = (10, 24);
    let island = offset_native_by_map(home.0, home.1, 8, 0);
    for (x, y) in [home, island] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
    }
    game.create_city(1, home.0, home.1);
    let founder_id = game.spawn_unit(1, UnitKind::Settler, home.0, home.1);
    let ship = game.spawn_unit(1, UnitKind::Galley, home.0, home.1);
    let cargo = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == cargo)
        .unwrap()
        .transport = Some(ship);
    let founder = game
        .units
        .iter()
        .find(|unit| unit.id == founder_id)
        .unwrap()
        .clone();
    assert_eq!(
        game.best_settlement_ferry(&founder, |_| false)
            .map(|(id, _)| id),
        Some(ship)
    );
    let second = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == second)
        .unwrap()
        .transport = Some(ship);
    assert!(game.best_settlement_ferry(&founder, |_| false).is_none());
}

fn ocean_game() -> Simulation {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.owner = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.hut = false;
    }
    game
}

#[test]
fn coastal_ports_and_embarkation_use_all_civ3_directions() {
    for direction in 0..CIV3_DIRS.len() {
        let mut game = ocean_game();
        let shore = (10, 24);
        let water = game
            .map_wrap
            .civ3_neighbor(shore.0, shore.1, direction)
            .unwrap();
        game.tile_mut(shore.0, shore.1).terrain = Terrain::Grassland;
        game.create_city(1, shore.0, shore.1);
        assert!(game.is_port(game.cities.last().unwrap()));
        let passenger = game.spawn_unit(1, UnitKind::Settler, shore.0, shore.1);
        let ship = game.spawn_unit(1, UnitKind::Galleon, water.0, water.1);

        assert!(
            game.embark_adjacent_ship(passenger, ship),
            "Civilization III direction {direction} should permit coastal embarkation"
        );
        assert_eq!(
            game.unit_by_id(passenger).unwrap().transport,
            Some(ship),
            "Civilization III direction {direction} should board the adjacent ship"
        );
    }
}
