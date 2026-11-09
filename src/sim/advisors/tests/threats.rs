//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn a_reachable_enemy_raises_demand_for_a_defender_upgrade() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].known = vec![Tech::BRONZE_WORKING];
    game.players[1].known.clear();
    let (x, y) = (20, 20);
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.create_city(1, x, y);
    game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
    let peaceful_price = game
        .tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
        .unwrap();
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).known_by[1] = true;
    game.spawn_unit(HUMAN, UnitKind::Legion, nx, ny);
    assert_eq!(
        game.tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
            .unwrap(),
        peaceful_price
    );
    game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
    assert!(
        game.tech_sell_price(HUMAN, 1, Tech::BRONZE_WORKING)
            .unwrap()
            > peaceful_price
    );
}

#[test]
fn defender_forecast_preserves_stronger_veterans_and_production_heals_estimates() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].known = vec![Tech::GUNPOWDER];
    let city = game.cities[0].clone();
    let id = game.spawn_unit(HUMAN, UnitKind::Marines, city.x, city.y);
    game.units.last_mut().unwrap().veteran = 3;
    let attacker = game.make_unit(0, 1, UnitKind::Chariot, city.x, city.y);
    assert_eq!(
        game.anticipated_city_defender(&city, &attacker, 3, false)
            .unwrap()
            .id,
        id
    );
    game.units.last_mut().unwrap().hp = 1;
    let actual = game
        .anticipated_city_defender(&city, &attacker, 1, false)
        .unwrap();
    let production = game
        .anticipated_city_defender(&city, &attacker, 1, true)
        .unwrap();
    assert_eq!(actual.hp, 1);
    assert_eq!(production.hp, production.kind.stats().hp);
    assert_eq!(game.units.last().unwrap().hp, 1);
}

#[test]
fn counter_defender_selection_accounts_for_attacker_class_and_naval_city_penalty() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].known = vec![Tech::GUNPOWDER, Tech::ROBOTICS];
    let city = &game.cities[0];
    let air = game.make_unit(0, 1, UnitKind::Fighter, city.x, city.y);
    let land = game.make_unit(0, 1, UnitKind::Legion, city.x, city.y);
    assert_eq!(
        game.defender_versus(city, &air).unwrap().kind,
        UnitKind::AegisCruiser
    );
    assert_eq!(
        game.defender_versus(city, &land).unwrap().kind,
        UnitKind::Musketeers
    );
    assert_eq!(
        game.production_city_defender(city, 100),
        Some(UnitKind::AegisCruiser),
        "production ranks generic type desirability, while the tactical land counter is Musketeers"
    );
}

#[test]
fn threat_arrival_uses_source_pf_floor_and_retains_road_turn_zero() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.river = false;
        tile.railroad = false;
        tile.known_by[1] = true;
    }
    let home = (10, 24);
    game.create_city(1, home.0, home.1);
    let attack = offset_native_by_map(home.0, home.1, 1, 0);
    let id = game.spawn_unit(HUMAN, UnitKind::Warrior, attack.0, attack.1);
    let turn = game
        .city_threat_arrivals(&game.cities[0])
        .find(|(u, _, _)| u.id == id)
        .unwrap()
        .2;
    assert_eq!(turn, 1);
    game.tile_mut(attack.0, attack.1).road = true;
    let turn = game
        .city_threat_arrivals(&game.cities[0])
        .find(|(u, _, _)| u.id == id)
        .unwrap()
        .2;
    // Enemy-owned infrastructure is unavailable to the approaching
    // Warrior: the city's final step still costs a full movement point.
    assert_eq!(turn, 1);
    game.units.iter_mut().find(|u| u.id == id).unwrap().kind = UnitKind::Horseman;
    let turn = game
        .city_threat_arrivals(&game.cities[0])
        .find(|(u, _, _)| u.id == id)
        .unwrap()
        .2;
    assert_eq!(turn, 0);
    let third = offset_native_by_map(home.0, home.1, 3, 0);
    let unit = game.units.iter_mut().find(|u| u.id == id).unwrap();
    unit.kind = UnitKind::Warrior;
    (unit.x, unit.y) = third;
    game.tile_mut(attack.0, attack.1).road = false;
    let turn = game
        .city_threat_arrivals(&game.cities[0])
        .find(|(u, _, _)| u.id == id)
        .unwrap()
        .2;
    assert_eq!(turn, 3);
    assert!(game.city_defense_production(&game.cities[0]).is_some());
}

#[test]
fn transported_troops_and_paradrops_enter_city_danger_without_illegal_moves() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_embarked_paradrop_danger_playtest());
}

#[test]
fn cargo_uses_carrier_speed_and_disembark_delay_and_can_outpace_walking() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.river = false;
        tile.base = None;
    }
    let home = (10, 24);
    game.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
    game.create_city(1, home.0, home.1);
    let mut sea = home;
    for _ in 0..4 {
        sea = game.map_wrap.civ3_neighbor(sea.0, sea.1, 2).unwrap();
    }
    let ferry = game.spawn_unit(HUMAN, UnitKind::Transport, sea.0, sea.1);
    let troop = game.spawn_unit(HUMAN, UnitKind::Warrior, sea.0, sea.1);
    assert!(game.board_specific_ship(troop, ferry));
    let city = game.cities[0].clone();
    let soldier = game.unit_by_id(troop).unwrap();
    assert!(game.danger_route_turns(soldier, &city).is_none());
    assert_eq!(game.city_danger_arrival(soldier, &city), Some(1));
    let far = (0, 0);
    for unit in game
        .units
        .iter_mut()
        .filter(|u| u.id == troop || u.id == ferry)
    {
        (unit.x, unit.y) = far;
    }
    let soldier = game.unit_by_id(troop).unwrap();
    assert!(game.city_danger_arrival(soldier, &city).is_none());
}

#[test]
fn paradrop_estimate_uses_source_integer_range_and_full_movement_assumption() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
    }
    let home = (10, 24);
    game.tile_mut(home.0, home.1).terrain = Terrain::Grassland;
    game.create_city(1, home.0, home.1);
    let city = game.cities[0].clone();
    for (distance, expected) in [(9, 0), (10, 1), (20, 2)] {
        let point = offset_native_by_map(home.0, home.1, distance, 0);
        let para = game.make_unit(0, HUMAN, UnitKind::Paratroopers, point.0, point.1);
        assert_eq!(game.city_danger_arrival(&para, &city), Some(expected));
    }
    let point = offset_native_by_map(home.0, home.1, 10, 0);
    let mut para = game.make_unit(0, HUMAN, UnitKind::Paratroopers, point.0, point.1);
    for step in 0..=10 {
        let (x, y) = offset_native_by_map(home.0, home.1, step, 0);
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.railroad = true;
    }
    para.moves_left = 0;
    assert_eq!(game.city_danger_arrival(&para, &city), Some(1));
}

#[test]
fn danger_counter_choice_assumes_the_next_full_turn_without_mutating_the_enemy() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_embarked_paradrop_danger_playtest());
    game.players[1]
        .known
        .extend([Tech::GUNPOWDER, Tech::AMPHIBIOUS_WARFARE]);
    let enemy = game
        .units
        .iter_mut()
        .find(|u| u.kind == UnitKind::Paratroopers)
        .unwrap();
    enemy.moves_left = 0;
    let id = enemy.id;
    assert_eq!(
        game.city_defense_production(&game.cities[0]),
        Some(Production::Unit(UnitKind::Marines))
    );
    assert_eq!(game.unit_by_id(id).unwrap().moves_left, 0);
}
