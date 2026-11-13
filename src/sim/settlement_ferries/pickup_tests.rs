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
    let mut game = Simulation::new(crate::test_support::seeded_map());
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

#[test]
fn ferry_tech_want_requires_the_local_city_search_to_need_a_boat() {
    let shore = (10, 24);
    let water = (11, 24);

    let mut no_local_site = ocean_game();
    no_local_site.players[1].known = vec![Tech::ALPHABET];
    no_local_site.tile_mut(shore.0, shore.1).terrain = Terrain::Grassland;
    no_local_site.create_city(1, shore.0, shore.1);
    assert!(no_local_site.is_port(no_local_site.cities.last().unwrap()));
    assert_eq!(no_local_site.ferry_tech_want(1, Tech::MAP_MAKING), 500);

    let mut good_local_site = ocean_game();
    good_local_site.players[1].known = vec![Tech::ALPHABET];
    for tile in &mut good_local_site.tiles {
        tile.terrain = Terrain::Grassland;
    }
    good_local_site.tile_mut(water.0, water.1).terrain = Terrain::Sea;
    good_local_site.create_city(1, shore.0, shore.1);
    assert!(good_local_site.is_port(good_local_site.cities.last().unwrap()));
    assert_eq!(good_local_site.ferry_tech_want(1, Tech::MAP_MAKING), 0);
}

#[test]
fn settlement_ferry_candidates_follow_selected_c2c3_roles() {
    use super::super::action_rules::freeciv_unit_has_role;

    assert_eq!(
        Simulation::settlement_ferry_unit_kinds().collect::<Vec<_>>(),
        vec![
            UnitKind::Galley,
            UnitKind::Caravel,
            UnitKind::Galleon,
            UnitKind::Transport,
        ]
    );
    assert!(freeciv_unit_has_role(UnitKind::Galley, "Ferryboat"));
    assert!(!freeciv_unit_has_role(UnitKind::Galley, "BarbarianBoat"));
    assert!(freeciv_unit_has_role(UnitKind::Caravel, "Ferryboat"));
    assert!(freeciv_unit_has_role(UnitKind::Caravel, "BarbarianBoat"));
    assert!(freeciv_unit_has_role(UnitKind::Galleon, "Ferryboat"));
    assert!(freeciv_unit_has_role(UnitKind::Galleon, "BarbarianBoat"));
    assert!(freeciv_unit_has_role(UnitKind::Transport, "Ferryboat"));
    assert!(!freeciv_unit_has_role(UnitKind::Transport, "BarbarianBoat"));
    assert!(!freeciv_unit_has_role(UnitKind::Carrack, "Ferryboat"));
    assert!(!freeciv_unit_has_role(UnitKind::Carrack, "BarbarianBoat"));
    assert!(!freeciv_unit_has_role(UnitKind::Dromon, "Ferryboat"));
    assert!(!freeciv_unit_has_role(UnitKind::Dromon, "BarbarianBoat"));
    assert!(!freeciv_unit_has_role(UnitKind::Curragh, "Ferryboat"));
    assert!(!freeciv_unit_has_role(UnitKind::Curragh, "BarbarianBoat"));
}

#[test]
fn map_making_port_builds_a_ferry_before_settling_a_worthwhile_overseas_site() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert_eq!(
        game.settlement_ferry_production_playtest(),
        (true, true, true),
        "the AI should queue the ferry, then the founder, and settle the island"
    );
}
