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
    game.tile_mut(nx, ny).known_by.set(1, true);
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
    // Only anti-air units counter aircraft; no ground unit is ever the answer.
    assert_eq!(
        game.defender_versus(city, &air).unwrap().kind,
        UnitKind::AegisCruiser
    );
    // Against a land attacker the standing land defender is chosen; ships get
    // no fortify bonus in port.
    assert_eq!(
        game.defender_versus(city, &land).unwrap().kind,
        UnitKind::Musketman
    );
    assert!(
        game.production_city_defender(city, 100)
            .is_some_and(
                |kind| super::super::action_rules::unit_has_role(kind, "DefendGood")
                    || super::super::action_rules::unit_has_role(kind, "DefendOk")
            )
    );
}

fn island_city_with_known(known: Vec<Tech>) -> Simulation {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Sea;
    }
    game.tile_mut(10, 24).terrain = Terrain::Grassland;
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].known = known;
    // Connect the strategic resources the units below need.
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
    game
}

#[test]
fn counter_defender_is_never_older_than_the_newest_dedicated_defender() {
    let game = island_city_with_known(vec![Tech::BRONZE_WORKING, Tech::GUNPOWDER]);
    let city = &game.cities[0];
    let land = game.make_unit(0, 1, UnitKind::Swordsman, city.x, city.y);
    // Without connected Iron a Warrior is still buildable and cheap, but a
    // Musketman (Middle Ages defender) must replace it as the counter.
    assert!(UnitKind::Musketman.era() > UnitKind::Warrior.era());
    assert!(game.unit_buildable(city, UnitKind::Warrior));
    assert!(game.unit_buildable(city, UnitKind::Musketman));
    let floor = UnitKind::Musketman.era();
    let pick = game.defender_versus(city, &land).unwrap().kind;
    assert!(pick.era() >= floor, "{pick:?}");
    assert_ne!(pick, UnitKind::Warrior);
}

#[test]
fn newer_offensive_units_do_not_remove_the_standing_defender() {
    // Invention unlocks Longbowman/Berserk (Offense role) but no new defender:
    // the Ancient Spearman remains a valid counter and a counter still exists.
    let game = island_city_with_known(vec![Tech::BRONZE_WORKING, Tech::INVENTION]);
    let city = &game.cities[0];
    let land = game.make_unit(0, 1, UnitKind::Swordsman, city.x, city.y);
    assert!(game.unit_buildable(city, UnitKind::Spearman));
    assert!(game.defender_versus(city, &land).is_some());
}

#[test]
fn counter_defender_versus_aircraft_is_never_a_ground_unit() {
    let air = |game: &Simulation| {
        let city = &game.cities[0];
        game.make_unit(0, 1, UnitKind::Fighter, city.x, city.y)
    };
    // Without any anti-air unit available there is no counter at all.
    let game = island_city_with_known(vec![Tech::BRONZE_WORKING, Tech::GUNPOWDER]);
    assert!(game.defender_versus(&game.cities[0], &air(&game)).is_none());
    // With an anti-air option, the pick can always answer aircraft.
    let game = island_city_with_known(vec![Tech::BRONZE_WORKING, Tech::GUNPOWDER, Tech::ROBOTICS]);
    let pick = game.defender_versus(&game.cities[0], &air(&game)).unwrap();
    assert!(pick.kind.can_counter_air(), "{:?}", pick.kind);
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
        tile.known_by.set(1, true);
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
fn scoped_threat_cache_preserves_arrivals_across_multiple_cities() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.river = false;
    }
    game.create_city(1, 10, 24);
    game.create_city(1, 18, 24);
    game.spawn_unit(HUMAN, UnitKind::Warrior, 13, 24);
    game.spawn_unit(HUMAN, UnitKind::Warrior, 13, 24);
    game.spawn_unit(HUMAN, UnitKind::Warrior, 14, 24);
    game.spawn_unit(HUMAN, UnitKind::Horseman, 15, 24);

    let arrivals = |simulation: &Simulation| {
        simulation
            .cities
            .iter()
            .flat_map(|city| {
                simulation
                    .city_threat_arrivals(city)
                    .map(move |(unit, _, turns)| (city.id, unit.id, turns))
            })
            .collect::<Vec<_>>()
    };
    let uncached = arrivals(&game);
    game.begin_ai_advisor_cache();
    let cached = arrivals(&game);
    let repeated = arrivals(&game);
    // Stacked warriors share a map. Warriors at both positions share step costs.
    assert_eq!(game.advisor_cache.threats().route_sizes(), (3, 2));
    game.end_ai_advisor_cache();

    assert_eq!(cached, uncached);
    assert_eq!(repeated, uncached);
}

#[test]
fn naval_threat_map_can_attack_a_coastal_city_but_cannot_route_through_it() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.river = false;
        tile.base = None;
    }
    let sea = (10, 24);
    game.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
    for steps in [1, 2] {
        let (x, y) = offset_native_by_map(sea.0, sea.1, steps, 0);
        game.create_city(1, x, y);
    }
    let ship = game.make_unit(0, HUMAN, UnitKind::Transport, sea.0, sea.1);
    let arrivals = || {
        game.cities
            .iter()
            .map(|city| game.danger_route_turns(&ship, city))
            .collect::<Vec<_>>()
    };
    assert_eq!(arrivals(), [Some(0), None]);
    game.begin_ai_advisor_cache();
    assert_eq!(arrivals(), [Some(0), None]);
    assert_eq!(game.advisor_cache.threats().route_sizes(), (1, 1));
}

#[test]
fn danger_route_bound_keeps_rail_routes_at_the_horizon_across_wrap() {
    let map = crate::mapgen::generate_for_map_size(42, crate::mapgen::MapSize::Tiny);
    let mut game = Simulation::new(map);
    game.units.clear();
    game.cities.clear();
    let home = (5, 24);
    let point = |x: i32| ((home.0 - x).rem_euclid(game.map_wrap.width()), home.1);
    let inside = point(23);
    let outside = point(24);
    for distance in 0..=24 {
        let (x, y) = point(distance);
        let tile = &mut game.tiles[game.map_wrap.index(x, y)];
        tile.terrain = Terrain::Grassland;
        tile.railroad = true;
    }
    game.create_city(HUMAN, home.0, home.1);
    let city = &game.cities[0];
    let inside = game.make_unit(0, HUMAN, UnitKind::Warrior, inside.0, inside.1);
    let outside = game.make_unit(1, HUMAN, UnitKind::Warrior, outside.0, outside.1);

    assert_eq!(game.danger_route_turns(&inside, city), Some(3));
    assert_eq!(game.danger_route_turns(&outside, city), None);
    game.begin_ai_advisor_cache();
    assert_eq!(game.danger_route_turns(&inside, city), Some(3));
    assert_eq!(game.danger_route_turns(&outside, city), None);
    assert_eq!(game.advisor_cache.threats().route_sizes(), (1, 1));
}

#[test]
fn transported_troops_and_paradrops_enter_city_danger_without_illegal_moves() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_embarked_paradrop_danger_test());
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
        sea = game.map_wrap.eight_way_neighbor(sea.0, sea.1, 2).unwrap();
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
    assert!(game.ai_embarked_paradrop_danger_test());
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
