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
        tile.known_by.set(1, true);
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
    assert_eq!(choice.production, Production::Unit(UnitKind::Settler));
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

// Retain the isolated-worker path as an oracle for the player-wide pass,
// including its interleaved military feedback and production switches.
fn uncached_city_defenses(
    game: &mut Simulation,
    owner: usize,
) -> std::collections::BTreeMap<u32, AiBuildChoice> {
    let mut choices = std::collections::BTreeMap::new();
    let ids: Vec<_> = game
        .player_cities_ordered(owner)
        .map(|city| city.id)
        .collect();
    let wonder_city = game.ai_wonder_city(owner);
    for id in ids {
        let city = game.city_by_id(id).unwrap().clone();
        game.record_ai_military_tech_wants(&city);
        if let Some(choice) = game.ai_build_choice_in_place(&city, wonder_city) {
            choices.insert(id, choice);
            if city.production != choice.production {
                game.change_production(id, choice.production);
            }
        }
    }
    choices
}

#[test]
fn city_production_shares_worker_values_with_exact_uncached_results() {
    let (mut shared, _) = worker_advisor_city(true);
    shared.create_city(1, 18, 24);
    shared.create_city(1, 26, 24);
    for city in &mut shared.cities {
        city.size = 3;
        city.worked_tiles.clear();
        city.specialists = [0; 3];
    }
    shared.reconcile_city_workers();
    let mut isolated = shared.clone();

    // A second pass must not retain any infrastructure values from before
    // changes to terrain, technology, government, or worker occupancy.
    for pass in 0..2 {
        for game in [&mut shared, &mut isolated] {
            if pass == 1 {
                game.players[1].known.push(Tech::STEAM_POWER);
                game.players[1].government = Government::Republic;
                game.tile_mut(11, 24).road = true;
                let worker = game.make_unit(900, 1, UnitKind::Worker, 19, 24);
                game.units.push(worker);
                game.reconcile_city_workers();
            }
            game.begin_ai_advisor_cache();
            game.trace.enable_profiling();
            game.trace
                .begin_turn_profile(game.turn, game.cities.len(), game.units.len());
        }
        let shared_choices = shared.prepare_ai_city_defenses(1);
        let isolated_choices = uncached_city_defenses(&mut isolated, 1);
        assert_eq!(shared_choices.len(), isolated_choices.len());
        for (id, choice) in &shared_choices {
            assert_eq!(choice.production, isolated_choices[id].production);
            assert_eq!(choice.want, isolated_choices[id].want);
        }
        // Includes gameplay RNG and retained advice, not the passive profiler.
        assert_eq!(
            shared.encode_campaign().unwrap(),
            isolated.encode_campaign().unwrap()
        );
        shared.trace.finish_turn_profile();
        isolated.trace.finish_turn_profile();
        let shared_profiles = shared.trace.drain_profiles();
        let isolated_profiles = isolated.trace.drain_profiles();
        let shared_ops = &shared_profiles[0].operations;
        let isolated_ops = &isolated_profiles[0].operations;
        for counter in [
            "worker.value_cache_misses",
            "worker.projection_cache_misses",
        ] {
            assert!(shared_ops[counter] < isolated_ops[counter], "{counter}");
        }
        assert_eq!(
            shared_ops["worker.candidates_considered"],
            isolated_ops["worker.candidates_considered"]
        );
        shared.end_ai_advisor_cache();
        isolated.end_ai_advisor_cache();
    }
}
