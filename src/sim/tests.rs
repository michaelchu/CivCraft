//! Regression tests for the behavior of the parent module.

#[test]
fn railroad_research_upgrades_centers_and_enables_worker_tracks() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let worker = game
        .units
        .iter()
        .find(|u| u.owner == HUMAN && u.kind == UnitKind::Worker)
        .unwrap()
        .clone();
    let (x, y) = (worker.x, worker.y);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.create_city(HUMAN, nx, ny);
    let tile = game.tile_mut(x, y);
    tile.terrain = Terrain::Hills;
    tile.road = true;
    tile.river = false;
    tile.mine = true;
    let initial = tile.yields();
    assert!(!game.apply(Command::Work(worker.id, WorkKind::Railroad)));
    game.discover(HUMAN, Tech::STEAM_POWER);
    assert!(game.tile(nx, ny).railroad);
    assert!(game.apply(Command::Work(worker.id, WorkKind::Railroad)));
    for _ in 0..2 {
        game.update_activities_and_restore();
        assert!(!game.tile(x, y).railroad);
    }
    game.update_activities_and_restore();
    assert!(game.tile(x, y).railroad);
    assert_eq!(
        game.tile(x, y).yields(),
        (initial.0, initial.1 + initial.1 / 2, initial.2)
    );
    assert_eq!(game.movement_cost_for(&worker, nx, ny), Some(1));
}

use super::*;

#[test]
fn civ3_map_roster_controls_active_players_and_score_standings() {
    for (size, expected) in [
        (crate::mapgen::Civ3MapSize::Tiny, 4),
        (crate::mapgen::Civ3MapSize::Huge, 16),
    ] {
        let map = crate::mapgen::generate_for_civ3_size(
            42,
            size,
            crate::mapgen::Civ3WorldSettings::default(),
        );
        let mut game = Simulation::new(map);
        assert_eq!(game.civilization_count, expected);
        assert_eq!(game.score_standings().len(), expected);
        for owner in 0..expected {
            assert!(game.player_alive(owner));
            assert_eq!(
                game.players[owner].known,
                crate::rules::civilizations::CIVILIZATIONS[owner]
                    .starting_techs
                    .to_vec()
            );
        }
        for owner in expected..MAX_CIVILIZATIONS {
            assert!(!game.player_alive(owner));
        }
        if size == crate::mapgen::Civ3MapSize::Tiny {
            game.end_turn();
            assert_eq!(game.turn, 2);
        }
    }
}

#[test]
fn newly_founded_city_claims_only_its_center_and_nearest_eight_tiles() {
    for wrap in [MapWrap::X, MapWrap::XY] {
        let mut game = Simulation::new(crate::mapgen::generate_with_wrap(42, wrap));
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by = [true; PLAYER_SLOTS];
            tile.owner = None;
            tile.border_city = None;
        }
        let (cx, cy) = (10, 24);
        game.create_city(HUMAN, cx, cy);

        let distances = game.map_wrap.civ3_distances_from(cx, cy);
        let mut first_ring = 0;
        let mut worked_candidate = None;
        for (index, tile) in game.tiles.iter().enumerate() {
            let rank = distances[index];
            if rank == 1 {
                first_ring += 1;
                assert_eq!(tile.owner, Some(HUMAN), "wrap {wrap:?}");
                worked_candidate.get_or_insert(game.map_wrap.position(index));
            } else if rank > 1 {
                assert_eq!(
                    tile.owner, None,
                    "unexpected claim at index {index}, rank {rank}, wrap {wrap:?}"
                );
            }
        }
        assert_eq!(first_ring, 8, "wrap {wrap:?}");
        let (inside_x, inside_y) = worked_candidate.unwrap();
        assert!(game.can_assign_city_tile(&game.cities[0], inside_x, inside_y));
        let outside = distances
            .iter()
            .position(|&rank| rank == 2)
            .map(|index| game.map_wrap.position(index))
            .unwrap();
        assert!(!game.can_assign_city_tile(&game.cities[0], outside.0, outside.1));
    }
}

#[test]
fn native_and_map_coordinates_round_trip() {
    for native_y in 0..MAP_H {
        for native_x in 0..MAP_W {
            let (map_x, map_y) = native_to_map(native_x, native_y);
            assert_eq!(map_to_native(map_x, map_y), (native_x, native_y));
        }
    }
}

#[test]
fn isohex_neighbors_and_wrapping_match_freeciv_distance() {
    assert!(!is_isohex_step(1, -1));
    assert!(!is_isohex_step(-1, 1));
    assert!(!is_isohex_step(2, 0));

    for &(start_x, start_y) in &[
        (0, 0),
        (MAP_W - 1, MAP_H - 1),
        crate::mapgen::generate(42).starts[0],
    ] {
        let mut neighbors = Vec::new();
        for (dx, dy) in ISOHEX_DIRS {
            let neighbor = offset_native_by_map(start_x, start_y, dx, dy);
            assert_eq!(
                isohex_vector_distance(
                    wrapped_map_vector(start_x, start_y, neighbor.0, neighbor.1).0,
                    wrapped_map_vector(start_x, start_y, neighbor.0, neighbor.1).1,
                ),
                1
            );
            neighbors.push(neighbor);
        }
        neighbors.sort_unstable();
        neighbors.dedup();
        assert_eq!(neighbors.len(), 6);
    }
}

#[test]
fn generated_world_uses_reference_dimensions_and_start_positions() {
    assert_eq!(MAP_TARGET_TILES, 2_000);
    assert_eq!(MAP_LINEAR_SCALE, 16);
    assert_eq!((MAP_W, MAP_H), (32, 64));
    let first = crate::mapgen::generate(42);
    let terrain = |map: &crate::mapgen::GeneratedMap| {
        map.tiles
            .iter()
            .map(|tile| (tile.terrain, tile.river, tile.resource))
            .collect::<Vec<_>>()
    };
    let replay = crate::mapgen::generate(42);
    assert_eq!(terrain(&first), terrain(&replay));
    assert_eq!(first.starts, replay.starts);
    assert_ne!(terrain(&first), terrain(&crate::mapgen::generate(12345)));

    for seed in 0..32 {
        let map = crate::mapgen::generate(seed);
        let starts = map.starts.clone();
        let game = Simulation::new(map);
        assert_eq!(game.tiles.len(), (MAP_W * MAP_H) as usize);
        for (i, &(x, y)) in starts.iter().enumerate() {
            assert!(game.can_found_at(x, y));
            assert!(starts[..i].iter().all(|&(other_x, other_y)| {
                game.map_distance(x, y, other_x, other_y) >= CITY_MIN_DISTANCE
            }));
        }
    }
}

#[test]
fn wrapped_path_uses_one_legal_civ3_step() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
    }
    let unit_id = game.units[0].id;
    game.units[0].x = 0;
    game.units[0].y = 0;
    let (target_x, target_y) = game.map_wrap.civ3_neighbor(0, 0, 2).unwrap();
    assert_eq!(game.path_to(unit_id, target_x, target_y), vec![(2, 0)]);
    assert_eq!(game.map_wrap.civ3_distance(0, 0, target_x, target_y), 1);
}

#[test]
fn healing_uses_city_minimum_and_fortified_regeneration() {
    let game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = crate::mapgen::generate(42).starts[0];
    let mut unit = game
        .units
        .iter()
        .find(|unit| unit.kind == UnitKind::Warrior)
        .cloned()
        .unwrap_or_else(|| {
            let mut unit = Unit::new(999, HUMAN, UnitKind::Warrior, x, y);
            unit.hp = 1;
            unit
        });
    unit.hp = 1;
    unit.moved = false;
    unit.state = UnitState::Idle;
    assert_eq!(Simulation::hitpoint_gain(&unit, false, false), 1);
    assert_eq!(Simulation::hitpoint_gain(&unit, true, false), 4);
    unit.state = UnitState::Fortified;
    assert_eq!(Simulation::hitpoint_gain(&unit, true, false), 5);
    unit.moved = true;
    assert_eq!(Simulation::hitpoint_gain(&unit, true, false), 0);

    // The compiled hp_gain_coord performs integer max-HP / 10 before
    // applying HP_Regen_2. Settlers have 20 maximum HP, so they gain 2
    // in the field and 6 minimum-city HP plus 2 regeneration in a city.
    unit.kind = UnitKind::Settlers;
    unit.hp = 1;
    unit.state = UnitState::Idle;
    unit.moved = false;
    assert_eq!(Simulation::hitpoint_gain(&unit, false, false), 2);
    assert_eq!(Simulation::hitpoint_gain(&unit, true, false), 8);
}

#[test]
fn defense_stages_and_research_costs_match_civ2civ3() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = crate::mapgen::generate(42).starts[0];
    game.tile_mut(x, y).terrain = Terrain::Forest;
    game.tile_mut(x, y).river = false;
    game.tile_mut(x, y).river_edges = 0;
    let mut defender = Unit::new(999, HUMAN, UnitKind::Warrior, x, y);
    defender.hp = 10;
    defender.moves_left = MOVE_FRAGMENTS;

    // Forest first: 10 * 125% = 12. Field fortification is a later
    // stage: 12 * 150% = 18.
    assert_eq!(game.defense_power(&defender, x, y), 12);
    defender.state = UnitState::Fortified;
    assert_eq!(game.defense_power(&defender, x, y), 18);

    // A city adds a 50% Defend_Bonus and gives can-fortify ground units
    // an automatic 50% Fortify_Defense_Bonus. Integer stages are
    // 12 * 150% = 18, then 18 * 150% = 27.
    defender.state = UnitState::Idle;
    game.cities
        .push(City::new(999, HUMAN, x, y, "Test".to_owned(), game.turn));
    assert_eq!(game.defense_power(&defender, x, y), 27);

    // Linear precalculation is 10 bulbs per recursive requirement; the
    // ruleset's unconditional 300% Tech_Cost_Pct effect is then applied.
    assert_eq!(Tech::ALPHABET.cost(), 30);
    assert_eq!(Tech::IRON_WORKING.cost(), 90);
}

#[test]
fn combat_odds_and_defender_selection_match_native_rules() {
    let even_fight = combat_win_chance(10, 10, 1, 10, 10, 1);
    assert!((even_fight - 0.5).abs() < 1.0e-12);

    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let mut attacker = Unit::new(1, HUMAN, UnitKind::Warrior, 0, 0);
    attacker.hp = 10;
    attacker.moves_left = MOVE_FRAGMENTS;
    let mut fragile_defender = attacker.clone();
    fragile_defender.id = 2;
    fragile_defender.owner = 1;
    fragile_defender.x = 1;
    fragile_defender.hp = 1;
    let mut healthy_defender = fragile_defender.clone();
    healthy_defender.id = 3;
    healthy_defender.hp = 10;
    game.units = vec![attacker.clone(), fragile_defender, healthy_defender];
    game.tile_mut(1, 0).terrain = Terrain::Grassland;

    assert_eq!(game.best_defender_id(&attacker, 1, 0), Some(3));
}

#[test]
fn granary_formula_uses_ruleset_initial_values_and_foodbox() {
    assert_eq!(Simulation::growth_box(0), 0);
    assert_eq!(Simulation::growth_box(1), 20);
    assert_eq!(Simulation::growth_box(6), 30);
    assert_eq!(Simulation::growth_box(8), 40);
    assert_eq!(Simulation::growth_box(20), 40);
}

#[test]
fn famine_destroys_size_one_city() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let settler = game
        .units
        .iter()
        .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
        .map(|unit| unit.id)
        .expect("human settler");
    assert!(game.found_city(settler));
    game.cities[0].food = -100;
    game.process_cities();
    assert!(game.cities.iter().all(|city| city.owner != HUMAN));
}

#[test]
fn irrigation_needs_water_and_conflicts_with_mines() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = crate::mapgen::generate(42).starts[0];
    game.tile_mut(x, y).terrain = Terrain::Hills;
    game.tile_mut(x, y).river = false;
    let adjacent: Vec<_> = game.map_wrap.civ3_neighbors(x, y).collect();
    for &(neighbor_x, neighbor_y) in &adjacent {
        let neighbor = game.tile_mut(neighbor_x, neighbor_y);
        neighbor.terrain = Terrain::Grassland;
        neighbor.irrigation = false;
        neighbor.river = false;
    }
    assert!(!game.can_irrigate_at(HUMAN, x, y));
    let (water_x, water_y) = adjacent[0];
    game.tile_mut(water_x, water_y).terrain = Terrain::Sea;
    assert!(game.can_irrigate_at(HUMAN, x, y));

    let worker_index = game
        .units
        .iter()
        .position(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Worker)
        .expect("human worker");
    game.units[worker_index].x = x;
    game.units[worker_index].y = y;
    game.units[worker_index].work_kind = Some(WorkKind::Irrigation);
    game.units[worker_index].work_left = 1;
    game.tile_mut(x, y).mine = true;
    game.update_activities_and_restore();
    assert!(game.tile(x, y).irrigation);
    assert!(!game.tile(x, y).mine);

    game.units[worker_index].work_kind = Some(WorkKind::Mine);
    game.units[worker_index].work_left = 1;
    game.update_activities_and_restore();
    assert!(game.tile(x, y).mine);
    assert!(!game.tile(x, y).irrigation);
}
