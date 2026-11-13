//! Regression tests for the behavior of the parent module.

use super::*;

fn remember_all_tiles_for_human(game: &mut Simulation) {
    let visible = vec![vec![true; game.tiles.len()]; game.players.len()];
    game.remember_render_map(&visible);
}

#[test]
fn nuclear_submarines_use_the_selected_submarine_transport_role() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (submarine, missile, _) = game.cruise_missile_playtest_setup();
    let sub = game
        .units
        .iter_mut()
        .find(|unit| unit.id == submarine)
        .unwrap();
    sub.kind = UnitKind::NuclearSubmarine;
    sub.hp = UnitKind::NuclearSubmarine.stats().hp;
    let missile_unit = game
        .units
        .iter_mut()
        .find(|unit| unit.id == missile)
        .unwrap();
    missile_unit.kind = UnitKind::TacticalNuke;
    missile_unit.hp = UnitKind::TacticalNuke.stats().hp;
    assert_eq!(
        UnitKind::NuclearSubmarine.required_tech().unwrap().name(),
        "Fission"
    );
    // The Civ III catalog remains available as source data. Runtime transport
    // behavior uses the selected C2C3 Submarine role for the retained
    // Nuclear Submarine identity.
    assert_eq!(UnitKind::Submarine.transport_capacity(), 0);
    assert!(!UnitKind::Submarine.can_carry(UnitKind::CruiseMissile));
    assert_eq!(UnitKind::NuclearSubmarine.transport_capacity(), 1);
    assert!(!UnitKind::NuclearSubmarine.can_carry(UnitKind::Fighter));
    assert!(!UnitKind::NuclearSubmarine.can_carry(UnitKind::Warrior));
    assert!(UnitKind::NuclearSubmarine.can_carry(UnitKind::TacticalNuke));
    assert_eq!(
        game.ruleset_transport_capacity(UnitKind::NuclearSubmarine)
            .unwrap(),
        8
    );
    assert!(
        game.ruleset_transport_pair_allowed(UnitKind::NuclearSubmarine, UnitKind::TacticalNuke)
            .unwrap()
    );
    assert!(game.board_or_deboard(missile));
    let missile = game.unit_by_id(missile).unwrap();
    assert_eq!(missile.transport, Some(submarine));
    assert_eq!(UnitKind::TacticalNuke.fuel_capacity(), 1);
    assert_eq!(missile.fuel, UnitKind::TacticalNuke.fuel_capacity());
}

#[test]
fn ai_submarine_launches_its_loaded_cruise_missile_at_an_adjacent_carrier() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (submarine, missile, target) = game.ai_submarine_playtest_setup();
    let hunter = game.unit_by_id(submarine).unwrap().clone();
    let target_unit = game.unit_by_id(target).unwrap();
    let mut paths = super::super::normal_paths::NormalPaths::enemy_map(&game, &hunter);
    let target_cost = paths.movement_cost(&game, target_unit.x, target_unit.y);
    assert_eq!(target_cost, Some(MOVE_FRAGMENTS));
    assert_eq!(game.unit_by_id(missile).unwrap().moves_left, MOVE_FRAGMENTS);

    game.run_ai_player(1);

    assert!(game.unit_by_id(submarine).is_some());
    assert!(game.unit_by_id(missile).is_none());
    assert!(game.unit_by_id(target).is_none());
}

#[test]
fn airlift_uses_airport_capacity_and_resets_next_turn() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 10, 20);
    let (first, second, destination) = game.airlift_playtest_setup();
    assert!(game.apply(Command::Airlift(first, destination)));
    assert!(!game.apply(Command::Airlift(second, destination)));
    assert!(game.apply(Command::EndTurn));
    assert!(game.apply(Command::Airlift(second, destination)));
}

#[test]
fn airlift_uses_selected_action_and_airlift_effect() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 10, 20);
    let (unit_id, _, destination_id) = game.airlift_playtest_setup();
    let unit = game.unit_by_id(unit_id).unwrap().clone();
    let destination = game
        .cities
        .iter()
        .find(|city| city.id == destination_id)
        .unwrap()
        .clone();
    assert!(game.can_airlift(&unit, &destination));

    {
        let source = game
            .cities
            .iter_mut()
            .find(|city| (city.x, city.y) == (unit.x, unit.y))
            .unwrap();
        source.buildings.clear();
    }
    assert!(!game.can_airlift(&unit, &destination));

    game.cities
        .iter_mut()
        .find(|city| (city.x, city.y) == (unit.x, unit.y))
        .unwrap()
        .buildings
        .push(Building::AIRPORT);
    game.units
        .iter_mut()
        .find(|candidate| candidate.id == unit_id)
        .unwrap()
        .moves_left = 0;
    let no_moves = game.unit_by_id(unit_id).unwrap().clone();
    assert!(!game.can_airlift(&no_moves, &destination));

    let ship_id = game.spawn_unit(HUMAN, UnitKind::Frigate, unit.x, unit.y);
    let ship = game.unit_by_id(ship_id).unwrap().clone();
    assert!(!game.can_airlift(&ship, &destination));
}

#[test]
fn paradrop_uses_selected_actions_minimum_moves_and_unit_range() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by[HUMAN] = true;
        tile.hut = false;
    }
    let source = (10, 20);
    game.create_city(HUMAN, source.0, source.1);
    for tile in &mut game.tiles {
        tile.visible = false;
        tile.seen = true;
    }
    remember_all_tiles_for_human(&mut game);
    let in_range = offset_native_by_map(source.0, source.1, 10, 0);
    let out_of_range = offset_native_by_map(source.0, source.1, 11, 0);

    let troop = game.spawn_unit(HUMAN, UnitKind::Paratrooper, source.0, source.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == troop)
        .unwrap()
        .moves_left = 6;
    game.units
        .iter_mut()
        .find(|unit| unit.id == troop)
        .unwrap()
        .moved = true;
    assert!(game.can_begin_paradrop(game.unit_by_id(troop).unwrap()));
    assert!(game.paradrop(troop, in_range.0, in_range.1));
    assert_eq!(
        (
            game.unit_by_id(troop).unwrap().x,
            game.unit_by_id(troop).unwrap().y
        ),
        in_range
    );
    assert!(game.unit_by_id(troop).unwrap().paradropped);
    {
        let unit = game.units.iter_mut().find(|unit| unit.id == troop).unwrap();
        (unit.x, unit.y) = source;
        unit.moves_left = 6;
        unit.moved = false;
    }
    assert!(!game.can_begin_paradrop(game.unit_by_id(troop).unwrap()));
    assert!(!game.paradrop(troop, in_range.0, in_range.1));
    game.restore_player_units(HUMAN);
    assert!(!game.unit_by_id(troop).unwrap().paradropped);
    assert!(game.can_begin_paradrop(game.unit_by_id(troop).unwrap()));

    let second_troop = game.spawn_unit(HUMAN, UnitKind::Paratrooper, source.0, source.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == second_troop)
        .unwrap()
        .moves_left = 6;
    assert!(!game.paradrop(second_troop, out_of_range.0, out_of_range.1));
    game.units
        .iter_mut()
        .find(|unit| unit.id == second_troop)
        .unwrap()
        .moves_left = 5;
    assert!(!game.can_begin_paradrop(game.unit_by_id(second_troop).unwrap()));

    let unsupported_identity =
        game.spawn_unit(HUMAN, UnitKind::ModernParatrooper, source.0, source.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == unsupported_identity)
        .unwrap()
        .moves_left = 6;
    assert!(!game.can_begin_paradrop(game.unit_by_id(unsupported_identity).unwrap()));
}

#[test]
fn blind_paradrop_uses_remembered_land_then_resolves_actual_water_death() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by[HUMAN] = true;
        tile.visible = false;
        tile.seen = true;
        tile.hut = false;
    }
    let source = (10, 20);
    game.create_city(HUMAN, source.0, source.1);
    remember_all_tiles_for_human(&mut game);
    let landing = offset_native_by_map(source.0, source.1, 10, 0);
    let hidden_occupier = game.map_wrap.offset(landing.0, landing.1, 1, 0).unwrap();
    let hidden_enemy = game.spawn_unit(1, UnitKind::Warrior, hidden_occupier.0, hidden_occupier.1);
    let troop = game.spawn_unit(HUMAN, UnitKind::Paratrooper, source.0, source.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == troop)
        .unwrap()
        .moves_left = 6;

    // The player's last observation says land. The actual map has since
    // changed to water, and the target is outside current sight.
    game.tile_mut(landing.0, landing.1).terrain = Terrain::Ocean;
    game.tile_mut(landing.0, landing.1).visible = false;
    assert_eq!(
        game.render_tile(landing.0, landing.1).terrain,
        Terrain::Grassland
    );
    assert!(!game.unit_visible_to_human(game.unit_by_id(hidden_enemy).unwrap()));
    assert!(game.paradrop(troop, landing.0, landing.1));
    assert!(game.unit_by_id(troop).is_none());
    assert!(game.events.iter().any(|event| event.contains("was lost")));
    assert_eq!(
        game.render_tile(landing.0, landing.1).terrain,
        Terrain::Ocean,
        "the actual bad landing updates remembered map knowledge"
    );
    assert!(game.tile(landing.0, landing.1).known_by[HUMAN]);
    assert!(game.tile(landing.0, landing.1).seen);
    assert!(!game.tile(landing.0, landing.1).visible);
    assert!(!game.tile(hidden_occupier.0, hidden_occupier.1).visible);
    assert!(!game.unit_visible_to_human(game.unit_by_id(hidden_enemy).unwrap()));
}

#[test]
fn paradrop_action_enablers_use_remembered_target_ownership() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.known_by[HUMAN] = true;
        tile.visible = false;
        tile.seen = true;
        tile.hut = false;
    }
    let source = (10, 20);
    game.create_city(HUMAN, source.0, source.1);
    remember_all_tiles_for_human(&mut game);
    let landing = offset_native_by_map(source.0, source.1, 10, 0);
    let troop = game.spawn_unit(HUMAN, UnitKind::Paratrooper, source.0, source.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == troop)
        .unwrap()
        .moves_left = 6;

    // The actual tile is newly claimed by another peaceful player, but the
    // player's remembered tile remains unclaimed. Enabler evaluation must use
    // that remembered owner instead of reading the live owner.
    game.tile_mut(landing.0, landing.1).owner = Some(1);
    assert!(game.paradrop(troop, landing.0, landing.1));
    assert_eq!(
        (
            game.unit_by_id(troop).unwrap().x,
            game.unit_by_id(troop).unwrap().y
        ),
        landing
    );
}

#[test]
fn captured_paratrooper_keeps_used_this_turn_state() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    let id = game.spawn_unit(HUMAN, UnitKind::Paratrooper, 10, 20);
    game.units
        .iter_mut()
        .find(|unit| unit.id == id)
        .unwrap()
        .paradropped = true;
    let victim = game.unit_by_id(id).unwrap().clone();

    let captured = game.transfer_unit(&victim, 1, None);
    assert!(game.unit_by_id(captured).unwrap().paradropped);
    game.restore_player_units(1);
    assert!(!game.unit_by_id(captured).unwrap().paradropped);
}

#[test]
fn cruise_missile_uses_missile_class_city_defense_and_sdi_bonus() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (bomber, defender) = game.sam_battery_playtest_setup(true);
    let attacker = game
        .units
        .iter_mut()
        .find(|unit| unit.id == bomber)
        .unwrap();
    attacker.kind = UnitKind::CruiseMissile;
    let attacker = attacker.clone();
    let target = game
        .units
        .iter()
        .find(|unit| unit.id == defender)
        .unwrap()
        .clone();
    let defense =
        |game: &Simulation| game.defense_power_against(&target, target.x, target.y, attacker.kind);
    // The imported Mech Infantry has 12 defense. C2C3 maps Cruise Missile to
    // the Aerial Missile class, so it receives neither the SAM Air nor
    // Helicopter effect. Only the city-center fortification stage applies:
    // 12 * 15 * 150% = 270. SDI also adds its selected +100% Missile-class
    // Defend_Bonus, bringing the result to 540; its Nuke_Proof effect is a
    // separate nuclear-action rule.
    assert_eq!(defense(&game), 270);
    game.cities
        .last_mut()
        .unwrap()
        .buildings
        .push(Building::SDI_DEFENSE);
    assert_eq!(defense(&game), 540);
    assert_eq!(
        game.combat_firepower(&attacker, &target),
        (2, 1),
        "selected C2C3 CityBuster doubles Cruise Missile firepower in a city"
    );
    assert_eq!(attacker.kind.fuel_capacity(), 1);
}

#[test]
fn aegis_cruisers_defend_against_air_attackers_and_sail_without_cargo() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (_, id) = game.aegis_playtest_setup();
    let cruiser = game.unit_by_id(id).unwrap();
    let (x, y) = (cruiser.x, cruiser.y);
    let defense = |attacker| game.defense_power_against(cruiser, x, y, attacker);
    assert_eq!(defense(UnitKind::Caravel), 100);
    assert_eq!(defense(UnitKind::Fighter), 500);
    assert_eq!(defense(UnitKind::Helicopter), 500);
    assert_eq!(cruiser.kind.transport_capacity(), 0);
    assert_eq!(cruiser.kind.required_tech(), Some(Tech::ROBOTICS));
    assert!(!UnitKind::Transport.can_carry(cruiser.kind));
    assert_eq!(game.movement_cost_for(cruiser, x, y), Some(MOVE_FRAGMENTS));
    let (land_x, land_y) = offset_native_by_map(x, y, 1, 0);
    game.tile_mut(land_x, land_y).terrain = Terrain::Grassland;
    let cruiser = game.unit_by_id(id).unwrap();
    assert_eq!(game.movement_cost_for(cruiser, land_x, land_y), None);
}

#[test]
fn legacy_air_defense_strength_does_not_intercept_selected_ruleset_bombard() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = offset_native_by_map(sx, sy, 1, 0);
    game.tile_mut(sx, sy).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).owner = Some(1);
    game.sign_pact(HUMAN, 1, Pact::War);
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, sx, sy);
    let defender = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    game.spawn_unit(1, UnitKind::Flak, tx, ty);

    // Freeciv has no separate Conquests Air Defense interception roll. Flak
    // remains an ordinary unit in the target stack while Bombard resolves.
    game.native_random = None;
    game.rng = 3;
    assert_eq!(
        game.try_bombard(&game.unit_by_id(bomber).unwrap().clone(), tx, ty),
        MoveResult::Fought
    );
    assert!(game.unit_by_id(bomber).is_some());
    assert_eq!(game.unit_by_id(bomber).unwrap().moves_left, 0);
    assert!(game.unit_by_id(defender).is_some());
}

#[test]
fn city_defenses_use_the_attacking_units_class_with_imported_defender_stats() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (_, defender_id) = game.sam_battery_playtest_setup(true);
    let defender = game
        .units
        .iter()
        .find(|unit| unit.id == defender_id)
        .unwrap()
        .clone();
    let city = game.cities.last_mut().unwrap();
    city.buildings.extend([
        Building::COASTAL_DEFENSE,
        Building::CITY_WALLS,
        Building::GREAT_WALL,
    ]);

    let defense = |game: &Simulation, attacker| {
        game.defense_power_against(&defender, defender.x, defender.y, attacker)
    };
    // Mech Infantry's imported defense is 12, scaled by Civ III's combat
    // power factor of 15. C2C3 evaluates these defense effects against the
    // attacking unit class: SAM and Coastal Defense each add 100%; Ground
    // attackers also receive city defense (+50%), City Walls (+100%), and
    // Great Wall (+50%). The city-center fortification stage adds 50%.
    assert_eq!(defense(&game, UnitKind::Bomber), 540);
    assert_eq!(defense(&game, UnitKind::Helicopter), 540);
    assert_eq!(defense(&game, UnitKind::Caravel), 540);
    assert_eq!(defense(&game, UnitKind::Musketman), 810);
    // Cruise Missiles belong to the Aerial Missile class, not Ground, Air,
    // Helicopter, Sea, or Trireme, so only the city-center bonus applies.
    assert_eq!(defense(&game, UnitKind::CruiseMissile), 270);
}

#[test]
fn helicopter_transport_uses_selected_ruleset_cargo_and_city_access() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (10, 20);
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    for (tx, ty) in [(x, y), (nx, ny)] {
        let tile = game.tile_mut(tx, ty);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.hut = false;
    }
    game.create_city(HUMAN, x, y);
    let helicopter = game.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
    let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, x, y);
    let marine = game.spawn_unit(HUMAN, UnitKind::Marine, x, y);
    assert!(!game.board_or_deboard(artillery));
    assert!(game.board_or_deboard(marine));
    assert_eq!(game.unit_by_id(marine).unwrap().transport, Some(helicopter));
    assert!(game.board_or_deboard(marine));
    assert_eq!(game.unit_by_id(marine).unwrap().transport, None);
}

#[test]
fn c2c3_bombard_hits_each_reachable_target_and_keeps_them_alive() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (bomber, targets) = game.bombard_playtest_setup();
    let (tx, ty) = {
        let target = game.unit_by_id(targets[0]).unwrap();
        (target.x, target.y)
    };
    game.tile_mut(tx, ty).owner = Some(1);
    game.tile_mut(tx, ty).visible = true;
    game.sign_pact(HUMAN, 1, Pact::War);
    for target in targets {
        game.units
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap()
            .hp = 2;
    }
    game.native_random = None;
    game.rng = 0;
    assert!(game.apply(Command::Bombard(bomber, tx, ty)));
    assert_eq!(
        targets
            .iter()
            .filter(|&&id| game.unit_by_id(id).is_some())
            .count(),
        2
    );
    assert_eq!(
        targets
            .iter()
            .map(|&id| game.unit_by_id(id).unwrap().hp)
            .collect::<Vec<_>>(),
        vec![1, 1]
    );
    let aircraft = game.unit_by_id(bomber).unwrap();
    assert_eq!(
        (aircraft.x, aircraft.y, aircraft.hp, aircraft.moves_left),
        (10, 20, aircraft.max_hit_points(), 0)
    );
}

#[test]
fn c2c3_bombard_uses_selected_action_range_and_unit_mapping() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    let (far_x, far_y) = game.map_wrap.offset(x, y, 2, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).visible = true;
    game.tile_mut(tx, ty).owner = Some(1);
    game.tile_mut(far_x, far_y).terrain = Terrain::Grassland;
    game.tile_mut(far_x, far_y).visible = true;
    game.tile_mut(far_x, far_y).owner = Some(1);
    game.sign_pact(HUMAN, 1, Pact::War);
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
    game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    game.spawn_unit(1, UnitKind::Warrior, far_x, far_y);
    game.enable_visual_events();
    assert_eq!(selected_bombard_max_range(), Some(1));
    assert!(game.can_bombard_tile(game.unit_by_id(bomber).unwrap(), tx, ty));
    assert!(!game.can_bombard_tile(game.unit_by_id(bomber).unwrap(), far_x, far_y));
    let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, x, y);
    assert!(!game.can_begin_bombard(game.unit_by_id(artillery).unwrap()));
    assert!(!game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), tx, ty));

    game.native_random = None;
    game.rng = 0;
    assert!(game.apply(Command::Bombard(bomber, tx, ty)));
    assert!(matches!(
        game.take_visual_events().front(),
        Some(VisualEvent::Combat(combat)) if combat.bombard_effects
    ));
    let unit = game.unit_by_id(bomber).unwrap();
    assert_eq!((unit.x, unit.y, unit.moves_left), (x, y, 0));
}

#[test]
fn c2c3_bombard_can_target_an_empty_enemy_city_and_uses_selected_citizen_class_rules() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 1, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).visible = true;
    }
    game.create_city(HUMAN, sx, sy);
    game.create_city(1, tx, ty);
    game.sign_pact(HUMAN, 1, Pact::War);
    game.cities
        .iter_mut()
        .find(|city| (city.x, city.y) == (tx, ty))
        .unwrap()
        .size = 3;
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, sx, sy);
    let attacker = game.unit_by_id(bomber).unwrap().clone();
    let target_city = game.city_at(tx, ty).unwrap();

    assert!(game.has_selected_bombard_capability(UnitKind::Bomber));
    assert!(!game.has_selected_bombard_capability(UnitKind::Artillery));
    assert_eq!(
        selected_unit_class_flag(UnitKind::Bomber, "KillCitizen"),
        Some(false)
    );
    assert_eq!(
        selected_unit_class_flag(UnitKind::Artillery, "KillCitizen"),
        Some(true)
    );
    assert!(
        game.action_available_for_unit_against_city(&attacker, "Bombard", target_city)
            .unwrap()
    );
    assert!(game.can_bombard_tile(&attacker, tx, ty));

    assert!(
        game.apply(Command::Bombard(bomber, tx, ty)),
        "Freeciv accepts a legal city-target bombardment with no units present"
    );
    let city = game.city_at(tx, ty).unwrap();
    assert_eq!(city.size, 3, "C2C3 Bombarders are not KillCitizen classes");
    assert_eq!(game.unit_by_id(bomber).unwrap().moves_left, 0);
    assert!(game.events.iter().any(|event| event.contains("bombarded")));
}

#[test]
fn c2c3_bombard_damage_scales_rate_and_matches_per_target_rng_draws() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = offset_native_by_map(sx, sy, 1, 0);
    for (x, y) in [(sx, sy), (tx, ty)] {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.hut = false;
    }
    game.tile_mut(tx, ty).owner = Some(1);
    game.sign_pact(HUMAN, 1, Pact::War);
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, sx, sy);
    let first = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    let second = game.spawn_unit(1, UnitKind::Spearman, tx, ty);
    let unreachable = game.spawn_unit(1, UnitKind::Fighter, tx, ty);
    game.units
        .iter_mut()
        .find(|unit| unit.id == bomber)
        .unwrap()
        .hp /= 3;
    let bomb_unit = game.unit_by_id(bomber).unwrap().clone();
    let rate = (selected_bombard_unit(UnitKind::Bomber)
        .unwrap()
        .bombard_rate
        * bomb_unit.hp
        / bomb_unit.max_hit_points())
    .max(1);
    assert_eq!(rate, 1);
    assert!(!game.combat_target_reachable(UnitKind::Bomber, UnitKind::Fighter, tx, ty));

    let targets = [first, second];
    let attack = game.attack_power_against_with_base(
        &bomb_unit,
        tx,
        ty,
        UnitKind::Bomber.bombard_strength(),
    );
    let mut expected = game.clone();
    let mut expected_hp = Vec::new();
    for id in targets {
        let target = expected.unit_by_id(id).unwrap().clone();
        let defense = expected.defense_power_against(&target, tx, ty, bomb_unit.kind);
        let (firepower, _) = expected.combat_firepower(&bomb_unit, &target);
        let mut hp = target.hp;
        for _ in 0..rate {
            if expected.rand_below(attack + defense) < attack {
                hp = (hp - firepower).max(1);
            }
        }
        expected_hp.push((id, hp));
    }

    assert_eq!(game.try_bombard(&bomb_unit, tx, ty), MoveResult::Fought);
    for (id, hp) in expected_hp {
        assert_eq!(game.unit_by_id(id).unwrap().hp, hp);
    }
    assert_eq!(
        game.unit_by_id(unreachable).unwrap().hp,
        UnitKind::Fighter.stats().hp
    );
    assert_eq!(game.native_random, expected.native_random);
}

#[test]
fn aircraft_can_return_to_airstrips_or_crash_when_out_of_fuel() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (10, 20);
    let (nx, ny) = offset_native_by_map(x, y, -1, 0);
    game.tile_mut(nx, ny).terrain = Terrain::Grassland;
    game.tile_mut(nx, ny).base = Some(Base::Airstrip);
    let first = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
    let second = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
    for unit in game
        .units
        .iter_mut()
        .filter(|unit| [first, second].contains(&unit.id))
    {
        unit.fuel = 1;
        if unit.id == second {
            unit.moves_left = 0;
        }
    }
    game.refresh_visibility(); // Newly placed human aircraft see the nearby airstrip.
    game.restore_aircraft();
    let saved = game.unit_by_id(first).unwrap();
    assert_eq!(
        (saved.x, saved.y, saved.fuel),
        (nx, ny, UnitKind::Bomber.fuel_capacity())
    );
    assert!(!game.units.iter().any(|unit| unit.id == second));
}

#[cfg(test)]
mod fighter_tests {
    use super::*;

    #[test]
    fn fighter_attacks_an_aircraft_through_ruleset_target_tile_combat() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        let (x, y) = (10, 20);
        let (tx, ty) = offset_native_by_map(x, y, 1, 0);
        for (tile_x, tile_y) in [(x, y), (tx, ty)] {
            game.tile_mut(tile_x, tile_y).terrain = Terrain::Grassland;
            game.tile_mut(tile_x, tile_y).visible = true;
        }
        game.tile_mut(tx, ty).owner = Some(1);
        game.sign_pact(HUMAN, 1, Pact::War);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        let bomber = game.spawn_unit(1, UnitKind::Bomber, tx, ty);
        game.units
            .iter_mut()
            .find(|unit| unit.id == bomber)
            .unwrap()
            .hp = 1;
        assert!(game.combat_target_reachable(UnitKind::Fighter, UnitKind::Bomber, tx, ty));
        assert!(!game.combat_target_reachable(UnitKind::Bomber, UnitKind::Fighter, x, y));
        game.native_random = None;
        // A nonzero fallback RNG state exercises attacker wins (all-zero
        // xorshift state always produced defender hits).
        game.rng = 1;
        assert!(game.apply(Command::Move(fighter, 1, 0)));
        assert!(
            game.unit_by_id(bomber).is_none(),
            "fighter combat result: fighter={:?}, bomber={:?}",
            game.unit_by_id(fighter)
                .map(|unit| ((unit.x, unit.y), unit.hp)),
            game.unit_by_id(bomber)
                .map(|unit| ((unit.x, unit.y), unit.hp))
        );
        assert!(game.unit_by_id(fighter).is_some());
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;

    #[test]
    fn carrier_production_and_emergency_landing_follow_aircraft_cargo_rules() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (carrier, fighter) = game.carrier_playtest_setup();
        let port = Production::Unit(UnitKind::Carrier);
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        let harbor = game.map_wrap.civ3_neighbor(x, y, 0).unwrap();
        game.tile_mut(harbor.0, harbor.1).terrain = Terrain::Sea;
        game.create_city(HUMAN, x, y);
        let city = game.cities.last().unwrap().clone();
        assert!(!game.can_produce(&city, port));
        let oil = game.map_wrap.offset(x, y, 1, 0).unwrap();
        let oil_tile = game.tile_mut(oil.0, oil.1);
        oil_tile.terrain = Terrain::Desert;
        oil_tile.resource = Some(Resource::Oil);
        oil_tile.owner = Some(HUMAN);
        oil_tile.road = true;
        game.players[HUMAN].known.push(Tech::MASS_PRODUCTION);
        game.players[HUMAN].known.push(Tech::REFINING);
        assert!(game.can_produce(&city, port));
        game.cities.clear();
        assert!(UnitKind::Carrier.can_carry(UnitKind::Fighter));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Warrior));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Cannon));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Worker));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::CruiseMissile));
        assert!(UnitKind::Transport.can_carry(UnitKind::CruiseMissile));
        assert!(!UnitKind::Transport.can_carry(UnitKind::Fighter));
        assert_eq!(UnitKind::Carrier.transport_capacity(), 4);
        game.restore_aircraft();
        let saved = game.unit_by_id(fighter).unwrap();
        assert_eq!(
            (saved.transport, saved.fuel),
            (Some(carrier), UnitKind::Fighter.fuel_capacity())
        );
    }
}

#[cfg(test)]
mod stealth_tests {
    use super::*;

    #[test]
    fn fighter_operational_range_and_city_base_are_source_defined() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (id, target) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.unit_by_id(id).unwrap().clone();
        let target = game.unit_by_id(target).unwrap();
        assert_eq!(UnitKind::Fighter.operational_range(), 6);
        assert_eq!(plane.kind.operational_range(), 6);
        assert!(game.refuel_site(plane.owner, plane.x, plane.y));
        assert!(
            game.map_wrap
                .civ3_distance(plane.x, plane.y, target.x, target.y)
                <= plane.kind.operational_range()
        );
    }

    fn loaded_hunt(
        distance: i32,
        kind: UnitKind,
        platform: UnitKind,
    ) -> (Simulation, u32, u32, u32) {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.sign_pact(1, HUMAN, Pact::War);
        game.players[1]
            .known
            .extend([Tech::ADVANCED_FLIGHT, Tech::ROCKETRY]);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Sea;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        let boat = game.spawn_unit(1, platform, 10, 20);
        game.units.last_mut().unwrap().moves_left = platform.stats().moves * MOVE_FRAGMENTS;
        let missile = game.spawn_unit(1, kind, 10, 20);
        game.units.last_mut().unwrap().moves_left = kind.stats().moves * MOVE_FRAGMENTS;
        assert!(game.board_specific_ship(missile, boat));
        let target = offset_native_by_map(10, 20, distance, 0);
        let victim = game.spawn_unit(HUMAN, UnitKind::Caravel, target.0, target.1);
        game.units.last_mut().unwrap().hp = 1;
        game.set_random_seed(4);
        (game, boat, missile, victim)
    }

    #[test]
    fn carrier_bomber_boss_holds_fighter_reserve_instead_of_independent_cargo_dispatch() {
        let (mut game, boat, bomber, victim) = loaded_hunt(2, UnitKind::Bomber, UnitKind::Carrier);
        let fighter = game.spawn_unit(1, UnitKind::Fighter, 10, 20);
        assert!(game.board_specific_ship(fighter, boat));
        game.run_ai_player(1);
        assert_eq!(game.ferry_boss(boat), Some(bomber));
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && !unit.moved && unit.transport == Some(boat))
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == boat && (unit.x, unit.y) == (10, 20))
        );
    }

    #[test]
    fn carrier_without_moves_cannot_activate_its_aircraft_boss() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        game.units
            .iter_mut()
            .find(|unit| unit.id == boat)
            .unwrap()
            .moves_left = 0;
        game.run_ai_player(1);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && !unit.moved)
        );
    }

    #[test]
    fn carrier_does_not_activate_a_distant_reserved_aircraft_without_cargo() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert!(game.board_or_deboard(fighter));
        let remote = offset_native_by_map(10, 20, 5, 0);
        game.tile_mut(remote.0, remote.1).terrain = Terrain::Grassland;
        game.tile_mut(remote.0, remote.1).base = Some(Base::Airstrip);
        let fresh_moves = game.unit_move_rate(game.unit_by_id(fighter).unwrap());
        let actor = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        (actor.x, actor.y) = remote;
        actor.moves_left = fresh_moves;
        actor.fuel = actor.kind.fuel_capacity();
        actor.moved = false;
        let actor = actor.clone();
        let target = offset_native_by_map(remote.0, remote.1, 2, 0);
        let enemy = game
            .units
            .iter_mut()
            .find(|unit| unit.id == victim)
            .unwrap();
        (enemy.x, enemy.y) = target;
        game.release_ai_ferry(fighter);
        assert!(game.assign_ai_ferry(boat, fighter));
        let ship = game
            .units
            .iter()
            .find(|unit| unit.id == boat)
            .unwrap()
            .clone();
        let (handled, done) = game.run_source_air_ferry(&ship);
        assert!(!handled && done.is_empty());
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.moves_left == actor.moves_left)
        );
    }

    #[test]
    fn empty_air_refuel_path_does_not_embark_until_emergency_rescue() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert!(game.board_or_deboard(fighter));
        game.units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .fuel = 2;
        let actor = game
            .units
            .iter()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .clone();
        assert_eq!(actor.fuel, 2);
        game.run_ai_strike_aircraft(&actor);
        assert!(game.units.iter().any(|unit| unit.id == victim));
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport.is_none()
            && unit.moves_left == actor.moves_left));
        // C2C3 Air units lose `unit.hp * 10 / 100` off-base. On Civ III's
        // retained 3-HP scale, the source integer formula truncates this to 0.
        let nonnative_decay = actor.max_hit_points() * 10 / 100;
        game.restore_aircraft();
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.transport.is_none() && unit.fuel == 1)
        );
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport == Some(boat)
            && unit.fuel == UnitKind::Fighter.fuel_capacity()
            && unit.hp == actor.hp - 2 * nonnative_decay));
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport == Some(boat)
            && unit.fuel == UnitKind::Fighter.fuel_capacity()
            && unit.hp == unit.max_hit_points()));
    }

    #[test]
    fn fuel_free_c2c3_helicopter_does_not_emergency_embark() {
        let (mut game, _boat, helicopter, _victim) =
            loaded_hunt(2, UnitKind::Helicopter, UnitKind::Carrier);
        assert!(game.board_or_deboard(helicopter));
        let before = game
            .units
            .iter()
            .find(|unit| unit.id == helicopter)
            .unwrap()
            .hp;
        let nonnative_decay = game.unit_by_id(helicopter).unwrap().max_hit_points() * 10 / 100;
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == helicopter
            && unit.transport.is_none()
            && unit.fuel == UnitKind::Helicopter.fuel_capacity()
            && unit.hp == before - nonnative_decay));
    }

    #[test]
    fn carrier_refuels_fighter_cargo() {
        let (mut game, carrier, fighter, _) = loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        assert_eq!(UnitKind::Carrier.transport_capacity(), 4);
        assert!(game.aboard_refueling_transport(game.unit_by_id(fighter).unwrap()));
        game.units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap()
            .fuel = 1;
        game.restore_aircraft();
        let recovered = game.unit_by_id(fighter).unwrap();
        assert_eq!(recovered.transport, Some(carrier));
        assert_eq!(recovered.fuel, UnitKind::Fighter.fuel_capacity());
    }

    #[test]
    fn c2c3_bomber_bombard_rules_use_selected_unit_data() {
        let one_attack = super::super::unit_catalog::UnitFlag::OneAttack;
        assert_eq!(UnitKind::Bomber.fuel_capacity(), 3);
        assert_eq!(UnitKind::Bomber.bombard_rate(), 3);
        assert!(!UnitKind::Bomber.has_flag(one_attack));
        assert!(UnitKind::CruiseMissile.has_flag(one_attack));
        let rules = selected_bombard_unit(UnitKind::Bomber).unwrap();
        assert!(rules.can_bombard);
        assert_eq!(rules.attack, 6);
        assert_eq!(rules.bombard_rate, 3);
        // C2C3 supplies the action capability/rate. Civ III's separate
        // bombard-strength field remains the explicit content mapping.
        assert_eq!(UnitKind::Bomber.bombard_strength(), 12);
        assert!(
            !selected_bombard_unit(UnitKind::Artillery)
                .unwrap()
                .can_bombard
        );
    }

    #[test]
    fn known_refuel_extras_work_on_enemy_owned_tiles() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        let point = (10, 20);
        game.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
        game.tile_mut(point.0, point.1).base = Some(Base::Airstrip);
        game.tile_mut(point.0, point.1).owner = Some(HUMAN);
        game.tile_mut(point.0, point.1).known_by[1] = true;
        game.sign_pact(1, HUMAN, Pact::War);
        let id = game.spawn_unit(1, UnitKind::Fighter, point.0, point.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .fuel = 1;
        assert!(game.refuel_site(1, point.0, point.1));
        game.restore_aircraft();
        assert_eq!(
            game.unit_by_id(id).unwrap().fuel,
            UnitKind::Fighter.fuel_capacity()
        );
        game.tile_mut(point.0, point.1).known_by[1] = false;
        assert!(!game.refuel_site(1, point.0, point.1));
    }

    #[test]
    fn stealth_aircraft_replace_older_models_and_need_close_detection() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (_, hidden) = game.stealth_aircraft_playtest_setup();
        // Keep the source city's ordinary vision, but avoid unrelated starting
        // units providing stealth detection at the relocated target tile.
        game.units
            .retain(|unit| unit.owner != HUMAN || unit.id == hidden);
        let city = game.cities_owned_by(HUMAN).next().unwrap().clone();
        let distant = offset_native_by_map(city.x, city.y, 2, 0);
        game.tile_mut(distant.0, distant.1).terrain = Terrain::Grassland;
        let target = game
            .units
            .iter_mut()
            .find(|unit| unit.id == hidden)
            .unwrap();
        (target.x, target.y) = distant;
        target.kind = UnitKind::StealthFighter;
        game.refresh_visibility();
        assert!(game.tile(distant.0, distant.1).visible);
        assert!(!game.unit_visible_to_human(game.unit_by_id(hidden).unwrap()));
        let explorer = game.spawn_unit(HUMAN, UnitKind::Explorer, city.x, city.y);
        assert!(game.apply(Command::Move(explorer, 1, 0)));
        assert!(game.unit_visible_to_human(game.unit_by_id(hidden).unwrap()));

        game.players[HUMAN].known.extend([
            Tech::FLIGHT,
            Tech::ADVANCED_FLIGHT,
            Tech::STEALTH,
            Tech::ROCKETRY,
            Tech::REFINING,
        ]);
        assert!(game.can_produce(&city, Production::Unit(UnitKind::JetFighter)));
        assert_eq!(
            game.production_successor(&city, UnitKind::Fighter),
            Some(UnitKind::JetFighter)
        );
        assert!(game.can_produce(&city, Production::Unit(UnitKind::StealthBomber)));
        assert_eq!(game.production_successor(&city, UnitKind::Bomber), None);
    }
    #[test]
    fn fighter_uses_the_selected_attack_action_instead_of_civ3_bombard() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        let (x, y) = (10, 20);
        let (tx, ty) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(tx, ty).terrain = Terrain::Grassland;
        game.tile_mut(tx, ty).owner = Some(1);
        game.sign_pact(HUMAN, 1, Pact::War);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        game.spawn_unit(1, UnitKind::Warrior, tx, ty);
        assert!(!game.can_begin_bombard(game.unit_by_id(fighter).unwrap()));
        assert!(!game.apply(Command::Bombard(fighter, tx, ty)));
    }

    #[test]
    fn airborne_plane_refuels_before_attacking_and_waits_for_full_fuel() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let home = (game.cities[0].x, game.cities[0].y);
        let open = offset_native_by_map(home.0, home.1, 1, 0);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        (plane.x, plane.y) = open;
        plane.fuel = 1;
        let snapshot = plane.clone();
        game.run_ai_strike_aircraft(&snapshot);
        let plane = game.unit_by_id(id).unwrap().clone();
        assert_eq!((plane.x, plane.y), home);
        assert!(game.units.iter().any(|u| u.id == victim));
        game.run_ai_strike_aircraft(&plane);
        assert!(game.units.iter().any(|u| u.id == victim));
    }

    #[test]
    fn tactical_nuke_uses_imported_blast_radius_and_is_one_shot() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.sign_pact(HUMAN, 1, Pact::War);
        game.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let origin = (10, 20);
        let target = offset_native_by_map(origin.0, origin.1, 1, 0);
        game.create_city(HUMAN, origin.0, origin.1);
        game.create_city(1, target.0, target.1);
        game.tile_mut(target.0, target.1).known_by[HUMAN] = true;
        let id = game.spawn_unit(HUMAN, UnitKind::TacticalNuke, origin.0, origin.1);
        let nuke = game.unit_by_id(id).unwrap().clone();
        assert!(UnitKind::TacticalNuke.has_flag(super::super::unit_catalog::UnitFlag::OneAttack));
        assert_eq!(UnitKind::TacticalNuke.operational_range(), 0);
        assert!(UnitKind::TacticalNuke.bombard_range() > 0);
        assert!(game.can_launch_nuclear_strike_at(&nuke, target.0, target.1));
        assert!(game.apply(Command::NuclearStrike(id, target.0, target.1)));
        assert!(game.unit_by_id(id).is_none());
    }
    #[test]
    fn city_air_raids_need_occupiers_but_virtual_production_can_plan_ahead() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let plane = game.unit_by_id(id).unwrap().clone();
        let enemy = game.units.iter_mut().find(|u| u.id == victim).unwrap();
        enemy.kind = UnitKind::Warrior;
        let target = (enemy.x, enemy.y);
        game.tile_mut(target.0, target.1).terrain = Terrain::Grassland;
        game.create_city(HUMAN, target.0, target.1);
        assert_eq!(game.aircraft_target_value(&plane, target.0, target.1), 0.0);
        let mut virtual_plane = plane.clone();
        virtual_plane.id = 0;
        assert!(game.aircraft_target_value(&virtual_plane, target.0, target.1) > 0.0);
        let occupier = game.spawn_unit(1, UnitKind::Warrior, plane.x, plane.y);
        // Nearby troops alone do not support bombing a defended city.
        assert_eq!(game.aircraft_target_value(&plane, target.0, target.1), 0.0);
        let city = game
            .cities
            .iter()
            .find(|c| (c.x, c.y) == target)
            .unwrap()
            .id;
        game.ai_ground_targets.insert(occupier, city);
        assert!(game.aircraft_target_value(&plane, target.0, target.1) > 0.0);
    }
    #[test]
    fn aircraft_healing_rate_uses_selected_c2c3_hp_effects() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let home = (10, 20);
        game.create_city(HUMAN, home.0, home.1);
        let airport = (12, 20);
        game.create_city(HUMAN, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let airfield = (14, 20);
        game.tile_mut(airfield.0, airfield.1).terrain = Terrain::Grassland;
        game.tile_mut(airfield.0, airfield.1).base = Some(Base::Airbase);
        game.tile_mut(airfield.0, airfield.1).owner = Some(HUMAN);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, home.0, home.1);
        let plane = game.unit_by_id(fighter).unwrap().clone();
        let hp = plane.max_hit_points();
        assert_eq!(
            game.aircraft_repair_gain(&plane, home.0, home.1),
            hp * 33 / 100,
            "ordinary cities contribute Min_HP_Pct"
        );
        assert_eq!(game.aircraft_repair_gain(&plane, airport.0, airport.1), hp);
        assert_eq!(
            game.aircraft_repair_gain(&plane, airfield.0, airfield.1),
            hp * 34 / 100
        );

        let carrier_id = game.spawn_unit(HUMAN, UnitKind::Carrier, 18, 20);
        let mut aboard = plane.clone();
        aboard.transport = Some(carrier_id);
        assert_eq!(game.aircraft_repair_gain(&aboard, 18, 20), 0);
    }

    #[test]
    fn airborne_damaged_plane_selects_repair_time_before_distance() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        game.units.retain(|u| u.id != victim);
        let home = (game.cities[0].x, game.cities[0].y);
        let airport = offset_native_by_map(home.0, home.1, 2, 0);
        game.tile_mut(airport.0, airport.1).terrain = Terrain::Grassland;
        game.create_city(1, airport.0, airport.1);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::AIRPORT);
        let plane = game.units.iter_mut().find(|u| u.id == id).unwrap();
        (plane.x, plane.y) = offset_native_by_map(home.0, home.1, 1, 0);
        plane.hp = 1;
        plane.fuel = plane.kind.fuel_capacity();
        let plane = plane.clone();
        game.run_ai_strike_aircraft(&plane);
        let plane = game.unit_by_id(id).unwrap();
        assert_eq!((plane.x, plane.y), airport);
    }

    #[test]
    fn airfields_are_owned_aircraft_refuel_bases() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        let start = (10, 20);
        let field = (12, 20);
        game.create_city(HUMAN, start.0, start.1);
        game.tile_mut(field.0, field.1).terrain = Terrain::Grassland;
        game.tile_mut(field.0, field.1).base = Some(Base::Airbase);
        game.tile_mut(field.0, field.1).owner = Some(HUMAN);
        assert!(game.refuel_site(HUMAN, field.0, field.1));
        assert!(!game.refuel_site(1, field.0, field.1));
    }
    #[test]
    fn airlift_refreshes_contact_but_stationary_bombardment_does_not() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let source = (10, 20);
        let destination = offset_native_by_map(source.0, source.1, 6, 0);
        game.create_city(HUMAN, source.0, source.1);
        game.create_city(HUMAN, destination.0, destination.1);
        game.sign_pact(HUMAN, 1, Pact::War);
        for city in &mut game.cities {
            city.buildings.push(Building::AIRPORT);
        }
        let arrival_city = game.cities[1].id;
        let visitor = game.spawn_unit(HUMAN, UnitKind::Warrior, source.0, source.1);
        let target = offset_native_by_map(destination.0, destination.1, 1, 0);
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.tile_mut(target.0, target.1).owner = Some(1);
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.airlift(visitor, arrival_city));
        let contact_after_airlift = game.diplomatic_contact_until.clone();
        assert!(
            contact_after_airlift
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, destination.0, destination.1);
        let aircraft = game.unit_by_id(bomber).unwrap().clone();
        let target_unit = game
            .units
            .iter()
            .find(|unit| (unit.x, unit.y) == target)
            .unwrap()
            .clone();
        assert!(
            game.action_available_for_unit_against_unit(&aircraft, "Bombard", &target_unit)
                .unwrap()
        );
        assert!(!game.bombard_blocked_by_action(
            &aircraft,
            target.0,
            target.1,
            &selected_bombard_blocking_actions().unwrap(),
        ));
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Fought
        );
        assert_eq!(game.diplomatic_contact_until, contact_after_airlift);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomatic_contact_until.clear();
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Blocked
        );
        assert!(game.diplomatic_contact_until.is_empty());
    }

    #[test]
    fn lost_paradrop_establishes_contact_at_the_actual_landing() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let origin = (10, 20);
        let landing = offset_native_by_map(origin.0, origin.1, 5, 0);
        game.create_city(HUMAN, origin.0, origin.1);
        let troop = game.spawn_unit(HUMAN, UnitKind::Paratrooper, origin.0, origin.1);
        game.spawn_unit(1, UnitKind::Warrior, landing.0, landing.1);
        game.tile_mut(landing.0, landing.1).known_by[HUMAN] = true;
        game.tile_mut(landing.0, landing.1).seen = true;
        remember_all_tiles_for_human(&mut game);
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.paradrop(troop, landing.0, landing.1));
        assert!(!game.units.iter().any(|u| u.id == troop));
        assert!(
            game.diplomatic_contact_until
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
    }

    #[test]
    fn failed_paradrop_reveals_remembered_circle_but_hides_unseen_units() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.visible = false;
            tile.seen = true;
            tile.known_by[HUMAN] = true;
        }
        let origin = (10, 20);
        let landing = offset_native_by_map(origin.0, origin.1, 5, 0);
        game.create_city(HUMAN, origin.0, origin.1);
        game.tile_mut(landing.0, landing.1).owner = Some(1);
        remember_all_tiles_for_human(&mut game);
        game.sign_pact(HUMAN, 1, Pact::Peace);

        let troop = game.spawn_unit(HUMAN, UnitKind::Paratrooper, origin.0, origin.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == troop)
            .unwrap()
            .moves_left = 6;
        let hidden_city_id = 50_000;
        game.cities.push(City::new(
            hidden_city_id,
            1,
            landing.0,
            landing.1,
            "Unseen City".to_owned(),
            game.turn,
        ));
        let hidden_unit = game.spawn_unit(1, UnitKind::Warrior, landing.0, landing.1);
        game.tile_mut(landing.0, landing.1).visible = false;
        assert!(!game.known_target_is_city(HUMAN, landing.0, landing.1));
        assert!(!game.unit_visible_to_human(game.unit_by_id(hidden_unit).unwrap()));

        assert!(game.paradrop(troop, landing.0, landing.1));
        assert!(game.unit_by_id(troop).is_none());
        assert!(
            game.render_cities()
                .iter()
                .any(|city| { city.city.id == hidden_city_id && city.remembered })
        );
        assert!(!game.tile(landing.0, landing.1).visible);
        assert!(!game.unit_visible_to_human(game.unit_by_id(hidden_unit).unwrap()));
    }
}

#[test]
fn paradrop_conquer_enters_and_captures_an_undefended_city() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (troop, target, gates) = game.paradrop_playtest_setup();
    assert!(gates, "Civ III tech gate or range check failed");
    assert!(game.apply(Command::Paradrop(troop, target.0, target.1)));
    let landed = game.unit_by_id(troop).unwrap();
    assert_eq!((landed.x, landed.y), target);
    assert!(landed.paradropped);
    assert!(landed.moves_left > 0);
    assert_eq!(game.city_at(target.0, target.1).unwrap().owner, HUMAN);
    assert!(!game.can_begin_paradrop(landed));
}
