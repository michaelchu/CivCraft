//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn a_reachable_enemy_raises_demand_for_a_defender_upgrade() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    game.spawn_unit(HUMAN, UnitKind::Swordsman, nx, ny);
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].known = vec![Tech::GUNPOWDER];
    let city = game.cities[0].clone();
    let id = game.spawn_unit(HUMAN, UnitKind::Marine, city.x, city.y);
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
    assert_eq!(production.hp, production.max_hit_points());
    assert_eq!(game.units.last().unwrap().hp, 1);
}

#[test]
fn counter_defender_selection_accounts_for_attacker_class_and_naval_city_penalty() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].known = vec![Tech::BRONZE_WORKING, Tech::GUNPOWDER, Tech::ROBOTICS];
    for (step, terrain, resource) in [
        (-1, Terrain::Hills, Resource::Aluminum),
        (-2, Terrain::Mountains, Resource::Uranium),
        (-3, Terrain::Desert, Resource::Saltpeter),
    ] {
        let (x, y) = offset_native_by_map(10, 24, step, 0);
        let tile = game.tile_mut(x, y);
        tile.terrain = terrain;
        tile.owner = Some(HUMAN);
        tile.resource = Some(resource);
        tile.road = true;
    }
    let city = &game.cities[0];
    let air = game.make_unit(0, 1, UnitKind::Fighter, city.x, city.y);
    let land = game.make_unit(0, 1, UnitKind::Swordsman, city.x, city.y);
    assert_eq!(
        game.defender_versus(city, &air).unwrap().kind,
        UnitKind::AegisCruiser
    );
    assert_eq!(
        game.defender_versus(city, &land).unwrap().kind,
        UnitKind::Musketman
    );
    assert!(
        game.production_city_defender(city, 100)
            .is_some_and(|kind| super::super::action_rules::freeciv_unit_has_role(
                kind,
                "DefendGood"
            ) || super::super::action_rules::freeciv_unit_has_role(
                kind, "DefendOk"
            ))
    );
}

#[test]
fn threat_arrival_uses_source_pf_floor_and_retains_road_turn_zero() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    game.players[1].known.push(Tech::BRONZE_WORKING);
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
    assert_eq!(
        UnitKind::Spearman.required_tech(),
        Some(Tech::BRONZE_WORKING)
    );
    assert_eq!(UnitKind::Spearman.stats().defense, 2);
}

#[test]
fn transported_troops_and_paradrops_enter_city_danger_without_illegal_moves() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_embarked_paradrop_danger_playtest());
}

#[test]
fn cargo_danger_forecast_accounts_for_transport_capacity_and_disembark() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
    assert_eq!(UnitKind::Transport.transport_capacity(), 6);
    assert_eq!(UnitKind::Transport.stats().moves, 6);
    assert_eq!(UnitKind::Warrior.transport_capacity(), 0);
}

#[test]
fn paradrop_estimate_uses_source_integer_range_and_full_movement_assumption() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
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
        let para = game.make_unit(0, HUMAN, UnitKind::Paratrooper, point.0, point.1);
        assert_eq!(game.city_danger_arrival(&para, &city), Some(expected));
    }
    let point = offset_native_by_map(home.0, home.1, 10, 0);
    let mut para = game.make_unit(0, HUMAN, UnitKind::Paratrooper, point.0, point.1);
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
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_embarked_paradrop_danger_playtest());
    game.players[1].known.extend(Tech::all());
    let (sx, sy) = offset_native_by_map(game.cities[0].x, game.cities[0].y, -1, 0);
    let saltpeter = game.tile_mut(sx, sy);
    saltpeter.terrain = Terrain::Desert;
    saltpeter.resource = Some(Resource::Saltpeter);
    saltpeter.owner = Some(1);
    saltpeter.road = true;
    let enemy = game
        .units
        .iter_mut()
        .find(|u| u.kind == UnitKind::Paratrooper)
        .unwrap();
    enemy.moves_left = 0;
    let id = enemy.id;
    for (step, terrain, resource) in [
        (-1, Terrain::Desert, Resource::Oil),
        (-2, Terrain::Hills, Resource::Rubber),
    ] {
        let (x, y) = offset_native_by_map(game.cities[0].x, game.cities[0].y, step, 0);
        let tile = game.tile_mut(x, y);
        tile.terrain = terrain;
        tile.resource = Some(resource);
        tile.owner = Some(1);
        tile.road = true;
    }
    assert!(game.city_defense_production(&game.cities[0]).is_some());
    assert_eq!(game.unit_by_id(id).unwrap().moves_left, 0);
}
