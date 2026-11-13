//! Worker production choices use the existing campaign worker planner.

use super::*;

fn worker_advisor_city(food_positive: bool) -> (Simulation, u32) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.players[1].known = [Tech::POTTERY, Tech::ENGINEERING, Tech::ELECTRICITY].to_vec();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Plains;
        tile.resource = Some(Resource::Wheat);
        tile.owner = Some(1);
        tile.known_by[1] = true;
        tile.river = true;
        tile.road = false;
        tile.railroad = false;
        tile.irrigation = true;
        tile.farmland = false;
        tile.mine = false;
        tile.pollution = false;
        tile.fallout = false;
        tile.craters = false;
        tile.hut = false;
        tile.base = None;
    }
    game.create_city(1, 10, 24);
    let city = &mut game.cities[0];
    city.size = 2;
    city.specialists = [0; 3];
    city.worked_tiles = vec![(11, 24)];
    if !food_positive {
        for position in [(10, 24), (11, 24)] {
            let tile = game.tile_mut(position.0, position.1);
            tile.terrain = Terrain::Desert;
            tile.resource = None;
            tile.river = false;
            tile.irrigation = false;
        }
    }
    let city_id = game.cities[0].id;
    (game, city_id)
}

#[test]
fn useful_work_city_can_choose_a_worker_for_production() {
    let (game, city_id) = worker_advisor_city(true);
    let city = game.city_by_id(city_id).unwrap();
    let forecast = game.city_forecast(city);
    assert!(
        forecast.food_surplus(city.size, forecast.support.food) > 0,
        "food {}, support {}, size {}",
        forecast.food,
        forecast.support.food,
        city.size
    );

    let worker = game.worker_production_choice(city);
    assert_eq!(worker.map(|(kind, _)| kind), Some(UnitKind::Worker));
    let choice = game.ai_build_choice(city).unwrap();
    assert_eq!(choice.production, Production::Unit(UnitKind::Worker));
}

#[test]
fn food_ineligible_city_does_not_choose_worker_production() {
    let (game, city_id) = worker_advisor_city(false);
    let city = game.city_by_id(city_id).unwrap();
    let forecast = game.city_forecast(city);
    assert!(forecast.food_surplus(city.size, forecast.support.food) <= 0);
    assert!(game.worker_production_choice(city).is_none());
    assert_ne!(
        game.ai_build_choice(city).map(|choice| choice.production),
        Some(Production::Unit(UnitKind::Worker))
    );
}
