//! Regression tests for advisors behavior and its interaction with the simulation.

use super::*;

#[test]
fn gold_spending_preserves_final_nondefense_advice() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (city, _) = game.hunter_production_playtest_setup();
    let position = {
        let port = game.city_by_id(city).unwrap();
        (port.x, port.y)
    };
    game.players[1]
        .known
        .extend([Tech::LITERATURE, Tech::CONSTRUCTION, Tech::ROBOTICS]);
    game.players[1].rates = [0, 100, 0];
    for (_, _, x, y) in Simulation::city_radius_at(position.0, position.1) {
        if game.tile(x, y).terrain.is_water() {
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
    assert_eq!(choice.production, Production::Unit(UnitKind::Settlers));
    assert_eq!(defense, Some(Production::Unit(UnitKind::AegisCruiser)));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
fn growth_constrained_city_prefers_aqueduct_to_available_expansion() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_production_priorities_playtest());
}

#[test]
fn production_compares_domestic_and_military_wants_on_one_scale() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    game.spawn_unit(1, UnitKind::Musketeers, 10, 24);
    let (x, y) = offset_native_by_map(10, 24, 4, 0);
    game.create_city(HUMAN, x, y);
    let domestic = game.domestic_build_choice(&game.cities[0]).unwrap();
    let military = game.military_build_advice(&game.cities[0]).1.unwrap();
    let chosen = game.ai_build_choice(&game.cities[0]).unwrap();
    assert!(chosen.want >= domestic.want);
    assert!(chosen.want >= military.want);
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
    assert_eq!(
        game.ai_build_choice(&game.cities[0]).unwrap().production,
        military.production,
        "war footing retains military advice, including its training-building replacement"
    );
}

#[test]
fn routine_rush_preserves_reserve_and_requires_invested_shields() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.create_city(1, 20, 20);
    game.turn += 1;
    game.cities[0].production = Production::Unit(UnitKind::Settlers);
    game.cities[0].shields = 1;
    game.cities[0].size = UnitKind::Settlers.stats().pop_cost;
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (selected, advanced, blocked) = game.offensive_production_playtest();
    assert!(selected && advanced && blocked);
}
