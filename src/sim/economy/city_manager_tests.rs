//! Regression tests for economy behavior, including edge cases and interactions with the simulation.

use super::*;
use std::collections::HashSet;

#[test]
fn resisting_citizens_are_not_available_for_tile_work() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(HUMAN, 10, 24);
    game.cities[0].size = 4;
    game.cities[0].resisting_citizens = vec![(HUMAN, 2)];
    game.cities[0].foreign_citizens = vec![(HUMAN, 2)];
    game.reconcile_city_workers();

    assert_eq!(game.cities[0].tile_workers(), 2);
    assert_eq!(game.cities[0].worked_tiles.len(), 2);
    assert_eq!(game.cities[0].specialists.iter().sum::<i32>(), 0);
}

#[test]
fn city_manager_never_assigns_resisting_citizens_to_work_or_specialists() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(HUMAN, 10, 24);
    game.cities[0].size = 4;
    game.cities[0].foreign_citizens = vec![(1, 2)];
    game.cities[0].resisting_citizens = vec![(1, 2)];

    let partially_resisting = game.city_manager_result(&game.cities[0], &Default::default());
    assert!(
        partially_resisting.worked_tiles.len() as i32
            + partially_resisting.specialists.iter().sum::<i32>()
            <= game.cities[0].non_resisting_citizens()
    );

    game.cities[0].foreign_citizens = vec![(1, 4)];
    game.cities[0].resisting_citizens = vec![(1, 4)];
    let fully_resisting = game.city_manager_result(&game.cities[0], &Default::default());
    assert!(fully_resisting.worked_tiles.is_empty());
    assert_eq!(fully_resisting.specialists, [0; 3]);
}

#[test]
fn martial_law_caps_follow_the_selected_c2c3_ruleset() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[HUMAN].known.clear();
    game.create_city(HUMAN, 10, 24);
    let city = game.cities[0].clone();
    for government in [
        (Government::Anarchy, 3),
        (Government::Tribal, 0),
        (Government::Despotism, 3),
        (Government::Monarchy, 3),
        (Government::Communism, 3),
        (Government::Fundamentalism, 0),
        (Government::Federation, 0),
        (Government::Republic, 0),
        (Government::Democracy, 0),
    ] {
        game.players[HUMAN].government = government.0;
        assert_eq!(
            game.city_rule_effect(
                &city,
                "Martial_Law_Max",
                None,
                None,
                None,
                false,
                government.0,
                None,
            ),
            government.1,
            "{:?}",
            government.0
        );
    }
}

#[test]
fn field_units_do_not_create_freeciv_unhappiness_or_military_police_from_airbases() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[HUMAN].known.clear();
    game.players[HUMAN].government = Government::Republic;
    game.create_city(HUMAN, 10, 24);
    let baseline = game.city_feelings(&game.cities[0], 0);
    let site = offset_native_by_map(10, 24, 1, 0);
    game.tile_mut(site.0, site.1).owner = None;
    game.tile_mut(site.0, site.1).base = Some(Base::Fortress);
    for _ in 0..3 {
        game.spawn_unit(HUMAN, UnitKind::Fighter, site.0, site.1);
    }
    assert_eq!(
        game.city_feelings(&game.cities[0], 0),
        baseline,
        "fielded units do not cause legacy unit unrest"
    );
    game.tile_mut(site.0, site.1).base = Some(Base::Airbase);
    assert_eq!(
        game.city_feelings(&game.cities[0], 0),
        baseline,
        "friendly Airbases do not change city happiness"
    );
}

#[test]
fn military_police_require_an_attacking_unit_and_only_work_in_its_city() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[1].government = Government::Despotism;
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    let sea = offset_native_by_map(12, 24, 1, 0);
    game.tile_mut(sea.0, sea.1).terrain = Terrain::Sea;
    game.create_city(1, 12, 24);
    let mut city = game.cities[0].clone();
    city.size = 6;
    city.specialists = [0; 3];
    assert_eq!(game.city_feelings_after_effects(&city, 0).1, 2);
    let civilian = game.spawn_unit(1, UnitKind::Settler, 12, 24);
    assert!(!UnitKind::Settler.is_military());
    assert_eq!(game.city_feelings(&city, 0).1, 2);
    let warrior = game.spawn_unit(1, UnitKind::Warrior, 12, 24);
    assert_eq!(game.city_feelings(&city, 0).1, 1);
    for unit in game
        .units
        .iter_mut()
        .filter(|unit| unit.id == civilian || unit.id == warrior)
    {
        (unit.x, unit.y) = sea;
    }
    assert_eq!(game.city_feelings(&city, 0).1, 2);
    game.players[1].government = Government::Democracy;
    assert_eq!(
        game.city_feelings(&city, 0).1,
        2,
        "Democracy has no military police"
    );
}

#[test]
fn unit_unrest_still_changes_city_feelings_without_martial_law() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(HUMAN, 10, 24);
    game.cities[0].size = 8;
    game.cities[0].specialists = [0; 3];
    game.cities[0].buildings.push(Building::COLOSSEUM);
    let city = game.cities[0].clone();
    let city_id = city.id;

    for government in [Government::Republic, Government::Democracy] {
        game.players[HUMAN].government = government;
        game.units.clear();
        let martial_law = game.city_rule_effect(
            &city,
            "Martial_Law_Max",
            None,
            None,
            None,
            false,
            government,
            None,
        );
        assert_eq!(martial_law, 0, "{government:?} has no martial law");
        let baseline = game.city_feelings_after_effects(&city, 16);

        game.units.clear();
        for index in 0..5 {
            let x = 25 + index;
            game.tile_mut(x, 24).owner = None;
            let id = game.spawn_unit(HUMAN, UnitKind::Warrior, x, 24);
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(city_id);
            unit.home_order = id;
        }

        assert!(game.city_unit_upkeep(&city).unit_happiness > 0);
        let with_unit_unrest = game.city_feelings_after_effects(&city, 16);
        assert!(
            with_unit_unrest.1 > baseline.1 || with_unit_unrest.0 < baseline.0,
            "{government:?}: supported unit unrest must apply even with Martial_Law_Max=0; baseline={baseline:?}, with units={with_unit_unrest:?}"
        );
    }
}

#[test]
fn disorder_keeps_civ3_gold_support_out_of_food_and_shield_outputs() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[1].government = Government::Despotism;
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Forest;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    let mut city = game.cities[0].clone();
    city.size = 8;
    city.specialists = [0; 3];
    city.worked_tiles = Simulation::city_radius(&city)
        .map(|(_, _, x, y)| (x, y))
        .filter(|&(x, y)| game.tile_can_support_worker(&city, x, y))
        .take(8)
        .collect();
    let home = city.id;
    for _ in 0..6 {
        let id = game.spawn_unit(1, UnitKind::Warrior, 25, 24);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .home_city = Some(home);
    }
    let forecast = game.city_forecast(&city);
    assert!(forecast.disorder && forecast.support.gold > 0 && forecast.shields > 0);
    assert_eq!(forecast.support.food, 0);
    assert_eq!(forecast.support.shields, 0);
    assert_eq!(game.city_manager_surplus(&city, &forecast)[1], 0);
}

#[test]
fn conquests_governments_do_not_apply_fundamentalism_tithes() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[1].known = vec![Tech::MYSTICISM];
    game.players[1].government = Government::Fascism;
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    let mut city = game.cities[0].clone();
    city.specialists = [0, 0, 0];
    city.worked_tiles.clear();
    assert_eq!(game.city_trade(&city, 0)[1], 0);
    city.buildings.push(Building::TEMPLE);
    assert_eq!(game.city_trade(&city, 0)[1], 0);
    city.buildings.push(Building::MARKETPLACE);
    assert_eq!(game.city_trade(&city, 0)[1], 0);
    game.players[1].government = Government::Democracy;
    assert_eq!(game.city_trade(&city, 0)[1], 0);
}

#[test]
fn city_manager_desktop_fixture() {
    let game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_city_manager_playtest());
}

#[test]
fn city_manager_matches_small_city_lattice_oracle_across_rates_and_governments() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    let mut city = game.cities[0].clone();
    city.size = 3;
    let all: Vec<_> = Simulation::city_radius(&city)
        .map(|(_, _, x, y)| (x, y))
        .collect();
    let allowed = &all[..6];
    for (index, &(x, y)) in allowed.iter().enumerate() {
        let tile = game.tile_mut(x, y);
        tile.owner = Some(1);
        tile.terrain = [
            Terrain::Grassland,
            Terrain::Plains,
            Terrain::Hills,
            Terrain::Forest,
            Terrain::Grassland,
            Terrain::Plains,
        ][index];
        tile.road = index % 2 == 0;
    }
    let occupied: HashSet<_> = all[6..].iter().copied().collect();
    for government in [
        Government::Despotism,
        Government::Republic,
        Government::Communism,
    ] {
        game.players[1].government = government;
        for rates in [[60, 30, 10], [30, 60, 10], [20, 20, 60]] {
            game.players[1].rates = rates;
            city.specialists = [3, 0, 0];
            city.worked_tiles.clear();
            let original = game.city_manager_surplus(&city, &game.city_forecast(&city));
            let grade = |candidate: &City| {
                let forecast = game.city_forecast(candidate);
                let output = game.city_manager_surplus(candidate, &forecast);
                let stage = if !forecast.disorder && output[0] >= 1 && output[1] >= 1 {
                    0
                } else if !forecast.disorder && output[0] >= 0 && output[1] >= 0 {
                    1
                } else if !forecast.disorder
                    && [0, 1, 2, 4, 5]
                        .into_iter()
                        .all(|i| output[i] >= original[i].min(0))
                {
                    2
                } else {
                    3
                };
                let fitness = if stage == 3 {
                    output.iter().map(|&x| i64::from(x)).sum::<i64>() + i64::from(forecast.happy)
                } else {
                    i64::from(output[0]) * 10
                        + i64::from(output[1]) * 5
                        + i64::from(output[3] + output[5]) * 2
                };
                (-(stage as i32), fitness)
            };
            let yields: Vec<_> = allowed
                .iter()
                .map(|&(x, y)| game.city_tile_yields(&city, x, y, false, None))
                .collect();
            let mut best = (i32::MIN, i64::MIN);
            for mask in 0_u32..1 << allowed.len() {
                let count = mask.count_ones() as i32;
                if count > city.size {
                    continue;
                }
                // Independently apply CM's literal prerequisite rule to
                // physical subsets, rather than reusing the DP states.
                let admissible = (0..allowed.len())
                    .filter(|&i| mask & (1 << i) != 0)
                    .all(|i| {
                        let a = yields[i];
                        a != (0, 0, 0)
                            && (0..allowed.len()).all(|j| {
                                let b = yields[j];
                                !(b != a && b.0 >= a.0 && b.1 >= a.1 && b.2 >= a.2)
                                    || mask & (1 << j) != 0
                            })
                    });
                if !admissible {
                    continue;
                }
                let mut candidate = city.clone();
                candidate.worked_tiles = allowed
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, &tile)| tile)
                    .collect();
                let specialists = city.size - count;
                for scientist in 0..=specialists {
                    for entertainer in 0..=specialists - scientist {
                        candidate.specialists = [
                            scientist,
                            specialists - scientist - entertainer,
                            entertainer,
                        ];
                        best = best.max(grade(&candidate));
                    }
                }
            }
            assert_eq!(
                grade(&game.city_manager_result(&city, &occupied)),
                best,
                "{government:?}, rates={rates:?}"
            );
        }
    }
}

#[test]
fn city_manager_equal_tiles_follow_native_city_index_not_row_order() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.road = false;
        tile.river = false;
    }
    game.create_city(1, 12, 24);
    let mut city = game.cities[0].clone();
    city.size = 1;
    city.worked_tiles.clear();
    city.specialists = [1, 0, 0];
    let result = game.city_manager_result(&city, &Default::default());
    // city.c:compare_iter_index sorts Civ III rank distance, dx, dy.
    let expected = Simulation::city_radius(&city)
        .filter(|&(_, _, x, y)| game.tile_can_support_worker(&city, x, y))
        .min_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy))
        .map(|(_, _, x, y)| (x, y))
        .unwrap();
    assert_eq!(result.worked_tiles, vec![expected]);
}

#[test]
fn city_manager_equal_specialist_fitness_uses_native_output_lexicographic_order() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    game.players[1].government = Government::Communism;
    let mut city = game.cities[0].clone();
    city.size = 3;
    city.specialists = [3, 0, 0];
    city.worked_tiles.clear();
    city.buildings
        .extend([Building::LIBRARY, Building::MARKETPLACE]);
    let occupied = Simulation::city_radius(&city)
        .map(|(_, _, x, y)| (x, y))
        .collect();
    let result = game.city_manager_result(&city, &occupied);
    // Both specialists yield3 after50% bonuses, with estimated fitness6.
    // Equal depth then output order Food,Shield,Trade,Gold,Luxury,Science
    // places taxmen before scientists; DFS retains the first equal leaf.
    assert_eq!(result.specialists, [0, 3, 0]);
}

#[test]
fn native_cm_estimate_truncates_trade_and_weights_luxury_for_happiness() {
    let result = cm_estimated_fitness(
        [1, 2, 3, 0, 0, 0],
        [100, 150, 125, 150, 100, 100],
        [60, 30, 10],
        [10, 5, 0, 2, 0, 2],
    );
    //10food+15shields+2.7gold+3.6science+.3luxury; adjusted trade3.75 truncates3.
    assert!((result - 31.6).abs() < 1e-10);
}

#[test]
fn native_cm_comparator_uses_half_point_tolerance_then_depth_and_output_order() {
    use std::cmp::Ordering::*;
    let tax = [0, 0, 0, 2, 0, 0];
    let science = [0, 0, 0, 0, 0, 2];
    assert_eq!(cm_compare_types((4.0, 0, tax), (4.5, 0, science)), Less);
    assert_eq!(
        cm_compare_types((4.0, 0, tax), (4.5001, 0, science)),
        Greater
    );
    assert_eq!(cm_compare_types((4.0, 2, tax), (4.0, 0, science)), Greater);
}

fn specialists_only_lattice() -> CmLattice {
    let production = vec![[0, 0, 0, 0, 0, 2], [0, 0, 0, 2, 0, 0], [0, 0, 0, 0, 2, 0]];
    let by_stat = std::array::from_fn(|output| {
        let mut order = vec![0, 1, 2];
        order.sort_by(|&a, &b| {
            production[b][output]
                .cmp(&production[a][output])
                .then_with(|| production[b].cmp(&production[a]))
        });
        order
    });
    CmLattice {
        tiles: vec![],
        groups: vec![],
        production,
        capacity: vec![CM_INFINITY; 3],
        better: vec![vec![]; 3],
        orders: [vec![1, 0, 2], vec![2, 1, 0]],
        by_stat,
    }
}

#[test]
fn native_cm_budget_counts_nonterminal_stack_steps_and_returns_incumbent() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 12, 24);
    game.players[1].government = Government::Communism;
    let mut city = game.cities[0].clone();
    city.size = 3;
    city.worked_tiles.clear();
    city.specialists = [3, 0, 0];
    let lattice = specialists_only_lattice();
    let early = game.city_manager_query(&city, &lattice, 10, [-CM_INFINITY; 6], true, 1);
    assert!(early.aborted);
    assert_eq!(early.iterations, 2);
    assert!(early.city.is_none());
    let complete =
        game.city_manager_query(&city, &lattice, 10, [-CM_INFINITY; 6], true, CM_MAX_LOOP);
    assert!(!complete.aborted);
    assert_eq!(complete.iterations, 19);
    assert_eq!(complete.city.unwrap().specialists, [0, 0, 3]);
    city.size = 255;
    city.specialists = [255, 0, 0];
    let truncated =
        game.city_manager_query(&city, &lattice, 10, [-CM_INFINITY; 6], true, CM_MAX_LOOP);
    assert!(truncated.aborted);
    assert_eq!(truncated.iterations, 27_501);
    assert_eq!(truncated.city.unwrap().specialists.iter().sum::<i32>(), 255);
}

#[test]
fn native_cm_learns_minimum_luxury_from_disorder_and_prunes_later_choices() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.road = false;
        tile.river = false;
    }
    game.create_city(1, 12, 24);
    game.players[1].government = Government::Communism;
    game.players[1].rates = [100, 0, 0];
    let mut city = game.cities[0].clone();
    city.size = 8;
    city.worked_tiles.clear();
    city.specialists = [8, 0, 0];
    let tiles: Vec<_> = Simulation::city_radius(&city)
        .map(|(_, _, x, y)| (x, y))
        .filter(|&(x, y)| game.tile_can_support_worker(&city, x, y))
        .take(8)
        .collect();
    let production = vec![
        [2, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 2],
        [0, 0, 0, 2, 0, 0],
        [0, 0, 0, 0, 2, 0],
    ];
    let by_stat = std::array::from_fn(|output| {
        let mut order = vec![0, 1, 2, 3];
        order.sort_by(|&a, &b| {
            production[b][output]
                .cmp(&production[a][output])
                .then_with(|| production[b].cmp(&production[a]))
        });
        order
    });
    let lattice = CmLattice {
        tiles,
        groups: vec![(0..8).collect()],
        production,
        capacity: vec![8, CM_INFINITY, CM_INFINITY, CM_INFINITY],
        better: vec![vec![]; 4],
        orders: [vec![0, 2, 1, 3], vec![3, 0, 2, 1]],
        by_stat,
    };
    let result = game.city_manager_query(
        &city,
        &lattice,
        10,
        [0, 0, 0, -CM_INFINITY, 0, 0],
        false,
        CM_MAX_LOOP,
    );
    assert!(result.disorder_prunes > 0);
    assert!(!result.aborted);
}

#[test]
fn native_cm_emergency_sentinel_preserves_local_c_int_production_arithmetic() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    game.players[1].government = Government::Communism;
    let mut city = game.cities[0].clone();
    city.buildings.extend([
        Building::FACTORY,
        Building::MFG_PLANT,
        Building::POWER_PLANT,
    ]);
    let center = game.city_tile_yields(&city, city.x, city.y, true, None);
    let output = game.city_manager_raw_production(&city, [-CM_INFINITY; 6]);
    // Native C -O2 oracle: max(0,(-1000000000+centerShield)*200/100)
    // Signed int multiplication wraps on the bundled target platform.
    let expected = (-CM_INFINITY + center.1).wrapping_mul(200) / 100;
    assert_eq!(output[1], expected.max(0));
}

#[test]
fn city_manager_large_city_preserves_citizen_count_under_native_search_budget() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for (index, tile) in game.tiles.iter_mut().enumerate() {
        tile.terrain = [
            Terrain::Grassland,
            Terrain::Plains,
            Terrain::Hills,
            Terrain::Forest,
        ][index % 4];
        tile.resource = None;
        tile.road = index % 3 == 0;
        tile.river = index % 5 == 0;
    }
    game.create_city(1, 12, 24);
    let mut city = game.cities[0].clone();
    city.size = 30;
    city.specialists = [30, 0, 0];
    city.worked_tiles.clear();
    let start = std::time::Instant::now();
    let arranged = game.city_manager_result(&city, &Default::default());
    eprintln!("CM size30 elapsed {:?}", start.elapsed());
    assert_eq!(
        arranged.worked_tiles.len() as i32 + arranged.specialists.iter().sum::<i32>(),
        30
    );
}
