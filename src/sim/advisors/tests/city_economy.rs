//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn stable_city_advice_is_reused_until_its_persisted_schedule_is_due() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(1, 10, 24);
    let city_id = game.cities[0].id;
    game.prepare_ai_city_defenses(1);
    let first = game.city_by_id(city_id).unwrap().clone();
    assert!(first.ai_building_next >= game.turn + 5);
    assert!(first.ai_settlement_next >= game.turn + 5);

    game.prepare_ai_city_defenses(1);
    let reused = game.city_by_id(city_id).unwrap();
    assert_eq!(reused.ai_building_next, first.ai_building_next);
    assert_eq!(reused.ai_settlement_next, first.ai_settlement_next);

    let bytes = bincode::serialize(&game).unwrap();
    let mut loaded: Simulation = bincode::deserialize(&bytes).unwrap();
    assert_eq!(loaded.cities[0].ai_building_next, first.ai_building_next);
    loaded.turn = first.ai_building_next.max(first.ai_settlement_next);
    loaded.prepare_ai_city_defenses(1);
    assert!(loaded.cities[0].ai_building_next > loaded.turn);
    assert!(loaded.cities[0].ai_settlement_next > loaded.turn);
}

#[test]
fn gold_spending_preserves_final_nondefense_advice() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (city, _) = game.hunter_production_playtest_setup();
    let position = {
        let port = game.city_by_id(city).unwrap();
        (port.x, port.y)
    };
    let target = game
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Battleship)
        .map(|unit| (unit.x, unit.y))
        .unwrap();
    game.players[1]
        .known
        .extend([Tech::LITERATURE, Tech::CONSTRUCTION, Tech::ROBOTICS]);
    game.players[1].rates = [0, 100, 0];
    for (_, _, x, y) in Simulation::city_radius_at(position.0, position.1) {
        if (x, y) == target {
            continue;
        }
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.road = true;
        tile.river = true;
        tile.irrigation = true;
    }
    game.cities.iter_mut().find(|c| c.id == city).unwrap().size = 6;
    game.reconcile_city_workers();
    let port = game.city_by_id(city).unwrap();
    let choice = game.ai_build_choice(port).unwrap();
    let defense = game.city_defense_production(port);
    assert_ne!(Some(choice.production), defense);
    game.cities
        .iter_mut()
        .find(|c| c.id == city)
        .unwrap()
        .production = choice.production;
    game.players[1].gold = 1000;
    game.spend_ai_defense_gold(1);
    assert_eq!(
        game.cities
            .iter()
            .find(|c| c.id == city)
            .unwrap()
            .production,
        choice.production
    );
}

#[test]
fn domestic_wants_penalize_discarding_invested_unit_production() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(1, 10, 24);
    let mut city = game.cities[0].clone();
    city.production = Production::Unit(UnitKind::Warrior);
    city.shields = 0;
    let uninvested = game.domestic_improvement_want(&city, Building::GRANARY);
    city.shields = 60;
    let invested = game.domestic_improvement_want(&city, Building::GRANARY);
    assert!(invested < uninvested);
    city.production = Production::Building(Building::TEMPLE);
    assert_eq!(
        game.domestic_improvement_want(&city, Building::GRANARY),
        uninvested
    );
}

#[test]
fn growth_and_expansion_options_remain_available_to_city_advisor() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_growth_expansion_playtest());
}

#[test]
fn production_compares_domestic_and_military_wants_on_one_scale() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.river = true;
        tile.resource = Some(Resource::Wheat);
        tile.irrigation = true;
        tile.hut = false;
        tile.known_by[1] = true;
    }
    game.create_city(1, 10, 24);
    game.cities[0].size = 3;
    game.cities[0].food = Simulation::growth_box(3) - 1;
    game.players[1]
        .known
        .extend([Tech::POTTERY, Tech::GUNPOWDER]);
    // Musketman has a Conquests Saltpeter requirement, which must be
    // connected before military advice can consider its current roster.
    let saltpeter = offset_native_by_map(10, 24, -1, 0);
    let tile = game.tile_mut(saltpeter.0, saltpeter.1);
    tile.terrain = Terrain::Desert;
    tile.resource = Some(Resource::Saltpeter);
    tile.owner = Some(1);
    tile.road = true;
    game.sign_pact(1, HUMAN, Pact::War);
    game.ai_war_plans.insert(
        (1, HUMAN),
        super::super::diplomacy::WarPlan {
            countdown: 2,
            reason: super::super::diplomacy::WarReason::Spoils,
        },
    );
    game.spawn_unit(1, UnitKind::Musketman, 10, 24);
    let (x, y) = offset_native_by_map(10, 24, 4, 0);
    game.create_city(HUMAN, x, y);
    let domestic = game.domestic_build_choice(&game.cities[0]).unwrap();
    let military = game.military_build_advice(&game.cities[0]).1;
    let chosen = game.ai_build_choice(&game.cities[0]).unwrap();
    assert!(chosen.want >= domestic.want);
    assert!(military.is_none_or(|advice| chosen.want >= advice.want));
    assert_eq!(
        game.choose_ai_production(1, game.cities[0].id),
        chosen.production
    );

    game.ai_war_plans.insert(
        (1, HUMAN),
        super::super::diplomacy::WarPlan {
            countdown: 2,
            reason: super::super::diplomacy::WarReason::Space,
        },
    );
    if let Some(military) = game.military_build_advice(&game.cities[0]).1 {
        assert_eq!(
            game.ai_build_choice(&game.cities[0]).unwrap().production,
            military.production,
            "war footing retains available military advice"
        );
    }
}

#[test]
fn routine_rush_preserves_reserve_and_requires_invested_shields() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.players[1].known.push(Tech::POTTERY);
    game.create_city(1, 20, 20);
    game.turn += 1;
    game.cities[0].production = Production::Building(Building::GRANARY);
    game.players[1].gold = 500;
    game.spend_ai_defense_gold(1);
    assert!(!game.cities[0].bought_production);

    game.cities[0].shields = 1;
    let price = game.rush_cost(&game.cities[0]).unwrap();
    let reserve = 2 * game.ai_gold_reserve(1);
    game.players[1].gold = price + reserve - 1;
    game.spend_ai_defense_gold(1);
    assert!(!game.cities[0].bought_production);

    game.players[1].gold += 1;
    game.spend_ai_defense_gold(1);
    assert!(game.cities[0].bought_production);
    assert_eq!(game.players[1].gold, reserve);
}

#[test]
fn routine_settler_rush_requires_no_war_preparation_and_enough_population() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.create_city(1, 20, 20);
    game.turn += 1;
    game.cities[0].production = Production::Unit(UnitKind::Settler);
    game.cities[0].shields = 1;
    game.cities[0].size = UnitKind::Settler.stats().pop_cost;
    game.players[1].gold = 500;
    game.spend_ai_defense_gold(1);
    assert!(!game.cities[0].bought_production);

    game.cities[0].size += 1;
    game.create_city(HUMAN, 5, 5);
    game.sign_pact(1, HUMAN, Pact::War);
    game.ai_war_plans.insert(
        (1, HUMAN),
        super::super::diplomacy::WarPlan {
            countdown: 0,
            reason: super::super::diplomacy::WarReason::Space,
        },
    );
    game.spend_ai_defense_gold(1);
    assert!(!game.cities[0].bought_production);

    game.sign_pact(1, HUMAN, Pact::Peace);
    game.ai_war_plans.remove(&(1, HUMAN));
    let price = game.rush_cost(&game.cities[0]).unwrap();
    let reserve = game.ai_gold_reserve(1);
    game.players[1].gold = price + reserve;
    game.spend_ai_defense_gold(1);
    assert!(game.cities[0].bought_production);
    assert_eq!(game.players[1].gold, reserve);
}

#[test]
fn city_output_priorities_raise_the_bid_for_a_library_technology() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let literature = Tech::LITERATURE;
    game.players[HUMAN].known = vec![Tech::ALPHABET, literature];
    for (_, _, x, y) in Simulation::city_radius_at(20, 20) {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.road = true;
        tile.river = true;
        tile.irrigation = true;
    }
    game.players[1].known = game
        .research_path(1, literature)
        .into_iter()
        .filter(|&tech| tech != literature)
        .collect();
    let no_cities = game.tech_sell_price(HUMAN, 1, literature).unwrap();
    game.create_city(1, 20, 20);
    game.cities.last_mut().unwrap().size = 3;
    game.cities.last_mut().unwrap().specialists = [0; 3];
    game.reconcile_city_workers();
    assert_eq!(
        game.tech_sell_price(HUMAN, 1, literature).unwrap(),
        no_cities
    );
    game.ai_tech_wants.insert(1, game.advisor_tech_wants(1));
    assert!(game.tech_sell_price(HUMAN, 1, literature).unwrap() > no_cities);
    assert!(!game.has_tech(1, literature));
}

#[test]
fn offensive_choice_amortizes_balanced_risk_even_with_existing_shield_stock() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(1, 10, 24);
    game.players[1]
        .known
        .extend([Tech::THE_WHEEL, Tech::HORSEBACK_RIDING]);
    let city = &game.cities[0];
    assert_eq!(UnitKind::Chariot.required_tech(), Some(Tech::THE_WHEEL));
    assert_eq!(
        UnitKind::Chariot.required_resources(),
        vec![Resource::Horses]
    );
    assert_eq!(UnitKind::Chariot.stats().moves, 2);
    assert!(game.unit_available(city.owner, UnitKind::Chariot));
}
