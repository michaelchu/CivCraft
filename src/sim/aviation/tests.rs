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
        1
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
fn native_submarines_cannot_load_cruise_missiles_for_a_hunt() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.tile_mut(10, 20).terrain = Terrain::Sea;
    let sub = game.spawn_unit(HUMAN, UnitKind::NuclearSubmarine, 10, 20);
    let missile = game.spawn_unit(HUMAN, UnitKind::CruiseMissile, 10, 20);
    assert!(!game.board_specific_ship(missile, sub));
    assert_eq!(game.unit_by_id(missile).unwrap().transport, None);
}

#[test]
fn ai_artillery_bombards_a_visible_enemy_stack_at_civ3_range() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 2, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty), (6, 20), (2, 2), (3, 2)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
        game.tile_mut(x, y).hut = false;
    }
    game.create_city(1, 6, 20);
    game.create_city(HUMAN, 2, 2);
    game.sign_pact(1, HUMAN, Pact::War);
    let artillery = game.spawn_unit(1, UnitKind::Artillery, sx, sy);
    let spotter = game.spawn_unit(1, UnitKind::Warrior, tx - 1, ty);
    game.units
        .iter_mut()
        .find(|unit| unit.id == spotter)
        .unwrap()
        .moves_left = 0;
    let target = game.spawn_unit(HUMAN, UnitKind::Warrior, tx, ty);
    game.refresh_visibility();
    assert!(game.unit_visible_to(1, game.unit_by_id(target).unwrap()));
    assert!(game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), tx, ty));

    game.native_random = None;
    game.rng = 0;
    let hp_before = game.unit_by_id(target).unwrap().hp;
    game.run_ai_player(1);

    assert!(game.unit_by_id(target).unwrap().hp < hp_before);
    let artillery = game.unit_by_id(artillery).unwrap();
    assert_eq!((artillery.x, artillery.y), (sx, sy));
    assert_eq!(artillery.moves_left, 0);
}

#[test]
fn ai_bombard_scoring_prefers_a_high_value_target_over_a_cheap_casualty() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (cheap_x, cheap_y) = game.map_wrap.offset(sx, sy, 1, 0).unwrap();
    let (valuable_x, valuable_y) = game.map_wrap.offset(sx, sy, 2, 0).unwrap();
    for (x, y) in [
        (sx, sy),
        (cheap_x, cheap_y),
        (valuable_x, valuable_y),
        (2, 2),
    ] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
        game.tile_mut(x, y).hut = false;
    }
    game.create_city(1, sx, sy);
    game.create_city(HUMAN, 2, 2);
    game.sign_pact(1, HUMAN, Pact::War);
    let artillery = game.spawn_unit(1, UnitKind::Artillery, sx, sy);
    game.spawn_unit(HUMAN, UnitKind::Worker, cheap_x, cheap_y);
    game.spawn_unit(HUMAN, UnitKind::MechInfantry, valuable_x, valuable_y);
    game.refresh_visibility();
    let artillery = game.unit_by_id(artillery).unwrap();

    assert!(
        game.ai_bombard_tile_value(artillery, valuable_x, valuable_y)
            > game.ai_bombard_tile_value(artillery, cheap_x, cheap_y)
    );
}

#[test]
fn ai_artillery_moves_toward_a_firing_position_when_no_shot_is_in_range() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let source = (10, 20);
    let target = game.map_wrap.offset(source.0, source.1, 6, 0).unwrap();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.hut = false;
        tile.known_by[1] = true;
    }
    game.create_city(1, source.0, source.1);
    game.create_city(HUMAN, target.0, target.1);
    game.sign_pact(1, HUMAN, Pact::War);
    let artillery = game.spawn_unit(1, UnitKind::Artillery, source.0, source.1);
    let source_distance = game
        .map_wrap
        .civ3_distance(source.0, source.1, target.0, target.1);
    assert!(source_distance > UnitKind::Artillery.bombard_range());

    game.run_ai_player(1);

    let artillery = game.unit_by_id(artillery).unwrap();
    let moved_distance = game
        .map_wrap
        .civ3_distance(artillery.x, artillery.y, target.0, target.1);
    assert!(moved_distance < source_distance);
    assert!(moved_distance > 0);
}

#[test]
fn ai_bombard_does_not_use_remembered_visibility_to_find_hidden_units() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 2, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty), (2, 2)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
        game.tile_mut(x, y).hut = false;
    }
    game.create_city(1, sx, sy);
    game.create_city(HUMAN, 2, 2);
    game.sign_pact(1, HUMAN, Pact::War);
    let artillery = game.spawn_unit(1, UnitKind::Artillery, sx, sy);
    game.tile_mut(tx, ty).known_by[1] = true;
    let target = game.spawn_unit(HUMAN, UnitKind::Submarine, tx, ty);
    assert!(!game.unit_visible_to(1, game.unit_by_id(target).unwrap()));

    assert!(!game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), tx, ty));
    let artillery_unit = game.unit_by_id(artillery).unwrap().clone();
    assert!(!game.run_ai_bombard(&artillery_unit));
    assert_eq!(
        game.unit_by_id(target).unwrap().hp,
        UnitKind::Submarine.stats().hp
    );
    assert!(game.unit_by_id(artillery).unwrap().moves_left > 0);
}

#[test]
fn ai_aircraft_cannot_start_a_bombard_mission_away_from_a_refuel_site() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (fighter, target) = game.ai_aircraft_advisor_playtest_setup();
    let away = offset_native_by_map(10, 24, 4, 0);
    {
        let plane = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        (plane.x, plane.y) = away;
    }
    let plane = game.unit_by_id(fighter).unwrap().clone();
    let target_unit = game.unit_by_id(target).unwrap();
    assert!(!game.can_bombard_tile(&plane, target_unit.x, target_unit.y));
    assert!(!game.refuel_site(plane.owner, plane.x, plane.y));

    let hp_before = target_unit.hp;
    assert!(!game.run_ai_bombard(&plane));
    assert_eq!(game.unit_by_id(target).unwrap().hp, hp_before);
    assert!(game.unit_by_id(fighter).unwrap().moves_left > 0);
}

#[test]
fn bombard_does_not_damage_a_hidden_or_unreachable_unit_in_a_visible_stack() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 2, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
        game.tile_mut(x, y).hut = false;
        game.tile_mut(x, y).visible = true;
    }
    game.sign_pact(HUMAN, 1, Pact::War);
    game.create_city(HUMAN, sx, sy);
    let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, sx, sy);
    let visible = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    let hidden = game.spawn_unit(1, UnitKind::Submarine, tx, ty);
    let unreachable = game.spawn_unit(1, UnitKind::Fighter, tx, ty);
    let hidden_hp = game.unit_by_id(hidden).unwrap().hp;
    let unreachable_hp = game.unit_by_id(unreachable).unwrap().hp;
    assert!(!game.unit_visible_to(HUMAN, game.unit_by_id(hidden).unwrap()));
    assert!(!game.combat_target_reachable(UnitKind::Artillery, UnitKind::Fighter, tx, ty));
    assert!(game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), tx, ty));

    game.native_random = None;
    game.rng = 0;
    assert_eq!(
        game.try_ranged_bombard(artillery, tx, ty),
        MoveResult::Fought
    );
    assert_eq!(game.unit_by_id(hidden).unwrap().hp, hidden_hp);
    assert_eq!(game.unit_by_id(unreachable).unwrap().hp, unreachable_hp);
    assert!(game.unit_by_id(visible).unwrap().hp < UnitKind::Warrior.stats().hp);
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
    let in_range = offset_native_by_map(
        source.0,
        source.1,
        UnitKind::Paratrooper.operational_range(),
        0,
    );
    let out_of_range = offset_native_by_map(
        source.0,
        source.1,
        UnitKind::Paratrooper.operational_range() + 1,
        0,
    );

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
    game.update_activities_and_restore();
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
    assert!(game.can_begin_paradrop(game.unit_by_id(unsupported_identity).unwrap()));
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
    let landing = offset_native_by_map(source.0, source.1, 5, 0);
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
    game.tile_mut(landing.0, landing.1).road = false;
    game.tile_mut(landing.0, landing.1).railroad = false;
    game.tile_mut(landing.0, landing.1).river = false;
    game.tile_mut(landing.0, landing.1).base = None;
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
    let landing = offset_native_by_map(source.0, source.1, 5, 0);
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
        (1, 1),
        "Native missiles do not inherit the foreign CityBuster multiplier"
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
    assert_eq!(defense(UnitKind::Fighter), 100);
    assert_eq!(defense(UnitKind::Helicopter), 100);
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
    game.create_city(HUMAN, sx, sy);
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
fn civ3_lethal_bombard_hits_each_reachable_target_and_removes_killed_units() {
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
    assert!(targets.iter().all(|&id| game.unit_by_id(id).is_none()));
    let aircraft = game.unit_by_id(bomber).unwrap();
    assert_eq!(
        (aircraft.x, aircraft.y, aircraft.hp, aircraft.moves_left),
        (10, 20, aircraft.max_hit_points(), 0)
    );
}

fn empty_terrain_bombard_setup() -> (Simulation, u32, (i32, i32)) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 1, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
    }
    game.tile_mut(tx, ty).owner = Some(1);
    game.tile_mut(tx, ty).visible = true;
    game.tile_mut(tx, ty).known_by[HUMAN] = true;
    game.create_city(HUMAN, sx, sy);
    game.sign_pact(HUMAN, 1, Pact::War);
    let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, sx, sy);
    (game, artillery, (tx, ty))
}

fn seed_for_successful_terrain_bombard(game: &Simulation, artillery: u32, x: i32, y: i32) -> u64 {
    let actor = game.unit_by_id(artillery).unwrap();
    let attack = game.attack_power_against_with_base_for_action(
        actor,
        x,
        y,
        actor.kind.bombard_strength(),
        "Bombard",
    );
    let rate = game.unit_by_id(artillery).unwrap().kind.bombard_rate();
    let defense = terrain_bombard_defense(game, x, y);
    (0..1000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            let first_hits = probe.rand_below(attack + defense) < attack;
            first_hits && (1..rate).all(|_| probe.rand_below(attack + defense) >= attack)
        })
        .expect("a deterministic seed should produce one terrain hit")
}

#[test]
fn bombard_destroys_road_and_mine_on_an_empty_terrain_tile() {
    let (mut game, artillery, (x, y)) = empty_terrain_bombard_setup();
    game.tile_mut(x, y).road = true;
    game.tile_mut(x, y).mine = true;
    game.native_random = None;
    game.rng = seed_for_successful_terrain_bombard(&game, artillery, x, y);

    assert!(game.apply(Command::Bombard(artillery, x, y)));
    assert!(!game.tile(x, y).road);
    assert!(!game.tile(x, y).mine);
}

#[test]
fn bombard_destroys_railroad_before_other_terrain_improvements() {
    let (mut game, artillery, (x, y)) = empty_terrain_bombard_setup();
    game.tile_mut(x, y).road = true;
    game.tile_mut(x, y).railroad = true;
    game.tile_mut(x, y).mine = true;
    game.native_random = None;
    game.rng = seed_for_successful_terrain_bombard(&game, artillery, x, y);

    assert!(game.apply(Command::Bombard(artillery, x, y)));
    assert!(!game.tile(x, y).railroad);
    assert!(game.tile(x, y).road);
    assert!(game.tile(x, y).mine);
}

#[test]
fn automated_bombard_repeats_after_the_unit_gets_movement_back() {
    let (mut game, artillery, (x, y)) = empty_terrain_bombard_setup();
    {
        let tile = game.tile_mut(x, y);
        tile.railroad = true;
        tile.road = true;
        tile.mine = true;
        tile.farmland = true;
        tile.base = Some(Base::Fortress);
    }
    game.native_random = None;
    let seed = (0..10_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            probe.apply(Command::AutoBombard(artillery, x, y))
                && !probe.tile(x, y).railroad
                && probe.tile(x, y).road
                && probe.tile(x, y).mine
                && {
                    probe.progress_player_unit_activities(HUMAN);
                    probe.continue_player_gotos();
                    !probe.tile(x, y).road && !probe.tile(x, y).mine
                }
        })
        .expect("a deterministic seed should produce the required shot sequence");
    game.rng = seed;

    assert!(game.apply(Command::AutoBombard(artillery, x, y)));
    assert_eq!(
        game.unit_by_id(artillery).unwrap().order,
        Some(UnitOrder::Bombard { x, y })
    );
    assert!(!game.tile(x, y).railroad);
    assert!(game.tile(x, y).road);
    assert!(game.tile(x, y).mine);

    game.progress_player_unit_activities(HUMAN);
    game.continue_player_gotos();
    assert_eq!(game.unit_by_id(artillery).unwrap().moves_left, 0);
    assert!(!game.tile(x, y).road);
    assert!(!game.tile(x, y).mine);
    assert_eq!(game.tile(x, y).base, Some(Base::Fort));
    assert!(game.unit_by_id(artillery).unwrap().order.is_some());
    let mut canceled = game.clone();
    assert!(canceled.apply(Command::CancelOrder(artillery)));
    assert_eq!(canceled.unit_by_id(artillery).unwrap().order, None);

    game.update_activities_and_restore();
    game.continue_player_gotos();
    assert_eq!(game.tile(x, y).base, None);
    assert_eq!(game.unit_by_id(artillery).unwrap().order, None);
}

#[test]
fn ai_artillery_bombards_a_known_hostile_terrain_improvement() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 2, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
    }
    game.tile_mut(tx, ty).owner = Some(HUMAN);
    game.tile_mut(tx, ty).known_by[1] = true;
    game.tile_mut(tx, ty).road = true;
    game.tile_mut(tx, ty).mine = true;
    game.sign_pact(1, HUMAN, Pact::War);
    let artillery = game.spawn_unit(1, UnitKind::Artillery, sx, sy);
    game.native_random = None;
    game.rng = seed_for_successful_terrain_bombard(&game, artillery, tx, ty);

    let actor = game.unit_by_id(artillery).unwrap().clone();
    assert!(game.run_ai_bombard(&actor));
    assert!(!game.tile(tx, ty).road);
    assert!(!game.tile(tx, ty).mine);
}

fn city_bombard_setup() -> (Simulation, u32, i32, i32) {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 1, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty)] {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.visible = true;
        tile.known_by[HUMAN] = true;
    }
    game.create_city(1, tx, ty);
    game.tile_mut(tx, ty).visible = true;
    game.tile_mut(tx, ty).known_by[HUMAN] = true;
    game.cities[0].size = 3;
    game.sign_pact(HUMAN, 1, Pact::War);
    let cannon = game.spawn_unit(HUMAN, UnitKind::Cannon, sx, sy);
    game.units.last_mut().unwrap().moves_left = UnitKind::Cannon.stats().moves * MOVE_FRAGMENTS;
    let actor = game.unit_by_id(cannon).unwrap();
    assert!(game.can_begin_bombard(actor));
    assert!(game.tile(tx, ty).visible);
    assert!(actor.moves_left >= 1);
    assert!(game.bombard_target_available_for_order(actor, tx, ty));
    assert!(game.can_bombard_tile(actor, tx, ty));
    game.native_random = None;
    (game, cannon, tx, ty)
}

#[test]
fn city_bombard_can_destroy_an_improvement() {
    let (mut game, cannon, x, y) = city_bombard_setup();
    game.cities[0]
        .buildings
        .push(crate::rules::Building::LIBRARY);
    let seed = (0..20_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            probe.try_ranged_bombard(cannon, x, y) != MoveResult::Blocked
                && !probe.cities[0]
                    .buildings
                    .contains(&crate::rules::Building::LIBRARY)
        })
        .expect("a deterministic seed should select and destroy the city building");
    game.rng = seed;

    assert_eq!(game.try_ranged_bombard(cannon, x, y), MoveResult::Fought);
    assert!(
        !game.cities[0]
            .buildings
            .contains(&crate::rules::Building::LIBRARY)
    );
    assert_eq!(game.cities[0].size, 3);
}

#[test]
fn precision_bombing_requires_smart_weapons_and_targets_city_improvements() {
    let (mut game, cannon, x, y) = city_bombard_setup();
    game.tile_mut(10, 20).base = Some(Base::Airbase);
    let bomber = game.unit_by_id(cannon).unwrap().clone();
    let bomber_id = bomber.id;
    let bomber = game
        .units
        .iter_mut()
        .find(|unit| unit.id == bomber_id)
        .unwrap();
    bomber.kind = UnitKind::StealthBomber;
    bomber.hp = UnitKind::StealthBomber.stats().hp;
    bomber.moves_left = MOVE_FRAGMENTS;
    game.cities[0]
        .buildings
        .push(crate::rules::Building::LIBRARY);
    assert!(!game.can_precision_bomb(game.unit_by_id(bomber_id).unwrap()));
    game.players[HUMAN].known.push(Tech::SMART_WEAPONS);
    assert!(game.can_precision_bomb(game.unit_by_id(bomber_id).unwrap()));
    assert_eq!(
        game.precision_bombardable_targets_in_range(bomber_id),
        vec![(x, y)]
    );

    let seed = (0..20_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            probe.try_precision_bombard(bomber_id, x, y) != MoveResult::Blocked
                && !probe.cities[0]
                    .buildings
                    .contains(&crate::rules::Building::LIBRARY)
        })
        .expect("a seed should produce a precision hit against the city improvement");
    game.rng = seed;
    assert!(game.apply(Command::AutoPrecisionBombard(bomber_id, x, y)));
    assert_eq!(
        game.unit_by_id(bomber_id).unwrap().order,
        Some(UnitOrder::PrecisionBombard { x, y })
    );
    assert!(
        !game.cities[0]
            .buildings
            .contains(&crate::rules::Building::LIBRARY)
    );
    assert_eq!(
        game.cities[0].size, 3,
        "precision bombing leaves population for when improvements are gone"
    );
    let city_id = game.cities[0].id;
    let population_seed = (0..20_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            probe.progress_player_unit_activities(HUMAN);
            probe.continue_player_gotos();
            probe
                .city_index_by_id(city_id)
                .is_some_and(|index| probe.cities[index].size == 2)
        })
        .expect("precision bombing falls through to population after improvements are gone");
    game.rng = population_seed;
    game.progress_player_unit_activities(HUMAN);
    game.continue_player_gotos();
    assert_eq!(game.cities[game.city_index_by_id(city_id).unwrap()].size, 2);
}

#[test]
fn city_bombard_can_reduce_population_without_razing_a_city() {
    let (mut game, cannon, x, y) = city_bombard_setup();
    let city_id = game.cities[0].id;
    let seed = (0..20_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            probe.try_ranged_bombard(cannon, x, y) != MoveResult::Blocked
                && probe
                    .city_index_by_id(city_id)
                    .is_some_and(|index| probe.cities[index].size == 2)
        })
        .expect("a deterministic seed should select and damage the city population");
    game.rng = seed;

    assert_eq!(game.try_ranged_bombard(cannon, x, y), MoveResult::Fought);
    assert_eq!(game.cities[game.city_index_by_id(city_id).unwrap()].size, 2);
}

#[test]
fn ai_bomber_bombards_a_known_hostile_terrain_tile_from_an_airbase() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(sx, sy, 3, 0).unwrap();
    for (x, y) in [(sx, sy), (tx, ty)] {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
    }
    game.tile_mut(sx, sy).base = Some(Base::Airbase);
    game.tile_mut(sx, sy).known_by[1] = true;
    game.tile_mut(tx, ty).owner = Some(HUMAN);
    game.tile_mut(tx, ty).known_by[1] = true;
    game.tile_mut(tx, ty).road = true;
    game.tile_mut(tx, ty).mine = true;
    game.sign_pact(1, HUMAN, Pact::War);
    let bomber = game.spawn_unit(1, UnitKind::Bomber, sx, sy);
    game.native_random = None;
    game.rng = seed_for_successful_terrain_bombard(&game, bomber, tx, ty);

    let actor = game.unit_by_id(bomber).unwrap().clone();
    assert!(game.run_ai_bombard(&actor));
    assert!(!game.tile(tx, ty).road);
    assert!(!game.tile(tx, ty).mine);
}

#[test]
fn ai_bomber_rebases_to_a_site_with_an_in_range_enemy_city() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[1].ai = true;
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.owner = None;
        tile.known_by[1] = false;
        tile.road = false;
        tile.railroad = false;
        tile.mine = false;
        tile.irrigation = false;
        tile.farmland = false;
        tile.oil_well = false;
        tile.oil_platform = false;
    }
    let source = (10, 20);
    let base = (12, 20);
    let target = (22, 20);
    for (x, y) in [source, base, target] {
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.base = None;
        tile.known_by[1] = true;
    }
    game.tile_mut(base.0, base.1).base = Some(Base::Airbase);
    game.create_city(1, source.0, source.1);
    game.create_city(HUMAN, target.0, target.1);
    game.cities.last_mut().unwrap().size = 2;
    game.sign_pact(HUMAN, 1, Pact::War);
    let bomber = game.spawn_unit(1, UnitKind::Bomber, source.0, source.1);
    let bomber_unit = game
        .units
        .iter_mut()
        .find(|unit| unit.id == bomber)
        .unwrap();
    bomber_unit.fuel = UnitKind::Bomber.fuel_capacity();
    bomber_unit.moves_left = MOVE_FRAGMENTS;
    let actor = game.unit_by_id(bomber).unwrap().clone();
    assert!(
        game.map_wrap
            .civ3_distance(source.0, source.1, target.0, target.1)
            > UnitKind::Bomber.operational_range()
    );
    assert!(
        game.map_wrap
            .civ3_distance(base.0, base.1, target.0, target.1)
            <= UnitKind::Bomber.operational_range()
    );
    let mut staged = actor.clone();
    staged.x = base.0;
    staged.y = base.1;
    staged.moves_left = actor.kind.stats().moves * MOVE_FRAGMENTS;
    assert!(game.can_bombard_tile(&staged, target.0, target.1));
    let route = game.aircraft_bombard_base_route(&actor, &[base]).unwrap();
    assert_eq!(route.1, base);
    assert!(game.aircraft_attack_path(&actor).is_none());
    assert!(game.refuel_site(1, source.0, source.1));
    assert!(game.refuel_site(1, base.0, base.1));
    assert_eq!(actor.fuel, actor.kind.fuel_capacity());
    assert!(game.can_begin_bombard(&actor));
    let mut probe = game.clone();
    assert!(!probe.run_ai_bombard(&actor));

    game.run_ai_strike_aircraft(&actor);
    assert_ne!(
        (
            game.unit_by_id(bomber).unwrap().x,
            game.unit_by_id(bomber).unwrap().y
        ),
        base,
        "the fuel route can span several turns"
    );
    assert_eq!(game.unit_goto_tiles.get(&bomber), Some(&base));
    game.restore_player_units(1);
    game.progress_player_unit_activities(1);
    game.run_ai_strike_aircraft(&game.unit_by_id(bomber).unwrap().clone());
    assert_eq!(
        (
            game.unit_by_id(bomber).unwrap().x,
            game.unit_by_id(bomber).unwrap().y
        ),
        base
    );
    game.restore_player_units(1);
    game.progress_player_unit_activities(1);
    let bomber_unit = game.unit_by_id(bomber).unwrap().clone();
    assert!(game.run_ai_bombard(&bomber_unit));
    assert_eq!(game.unit_by_id(bomber).unwrap().moves_left, 0);
}

#[test]
fn fighter_air_superiority_intercepts_within_half_range_once_per_turn() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let base = (10, 20);
    let near = game.map_wrap.offset(base.0, base.1, 2, 0).unwrap();
    let far = game.map_wrap.offset(base.0, base.1, 4, 0).unwrap();
    for (x, y) in [base, near, far] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(x, y).base = None;
        game.tile_mut(x, y).visible = true;
        game.tile_mut(x, y).known_by[HUMAN] = true;
    }
    game.create_city(HUMAN, base.0, base.1);
    game.sign_pact(HUMAN, 1, Pact::War);
    let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, base.0, base.1);
    let bomber = game.spawn_unit(1, UnitKind::Bomber, near.0, near.1);
    assert!(game.apply(Command::AirSuperiority(fighter)));
    assert_eq!(
        game.unit_by_id(fighter).unwrap().order,
        Some(UnitOrder::AirSuperiority {
            x: base.0,
            y: base.1
        })
    );
    assert!(game.try_air_superiority_interceptions(bomber, far));
    assert!(!game.unit_by_id(fighter).unwrap().air_superiority_shot);

    let initial_bomber_hp = game.unit_by_id(bomber).unwrap().hp;
    let initial_fighter_hp = game.unit_by_id(fighter).unwrap().hp;
    game.native_random = None;
    let seed = (0..10_000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = *seed;
            !probe.try_air_superiority_interceptions(bomber, near)
                || probe
                    .unit_by_id(fighter)
                    .is_some_and(|unit| unit.air_superiority_shot)
        })
        .expect("a deterministic seed should detect an ordinary air mission");
    game.rng = seed;
    let mission_survived = game.try_air_superiority_interceptions(bomber, near);
    assert!(
        !mission_survived
            || game
                .unit_by_id(fighter)
                .is_some_and(|unit| unit.air_superiority_shot)
    );
    assert!(
        game.unit_by_id(bomber)
            .is_none_or(|unit| unit.hp < initial_bomber_hp)
            || game
                .unit_by_id(fighter)
                .is_none_or(|unit| unit.hp < initial_fighter_hp)
    );

    let fighter_after_interception = game.unit_by_id(fighter).map(|unit| unit.hp);
    let bomber_after_interception = game.unit_by_id(bomber).map(|unit| unit.hp);
    game.try_air_superiority_interceptions(bomber, near);
    assert_eq!(
        game.unit_by_id(fighter).map(|unit| unit.hp),
        fighter_after_interception
    );
    assert_eq!(
        game.unit_by_id(bomber).map(|unit| unit.hp),
        bomber_after_interception,
        "a detected interceptor can only fire once per turn"
    );
    game.restore_player_units(HUMAN);
    if let Some(fighter) = game.unit_by_id(fighter) {
        assert!(!fighter.air_superiority_shot);
    }
}

#[test]
fn bombard_uses_civ3_range_and_unit_mapping() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let (x, y) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(x, y, 1, 0).unwrap();
    let (far_x, far_y) = game.map_wrap.offset(x, y, 2, 0).unwrap();
    let (out_of_range_x, out_of_range_y) = game.map_wrap.offset(x, y, 11, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).visible = true;
    game.tile_mut(tx, ty).owner = Some(1);
    game.tile_mut(far_x, far_y).terrain = Terrain::Grassland;
    game.tile_mut(far_x, far_y).visible = true;
    game.tile_mut(far_x, far_y).owner = Some(1);
    game.tile_mut(out_of_range_x, out_of_range_y).terrain = Terrain::Grassland;
    game.tile_mut(out_of_range_x, out_of_range_y).visible = true;
    game.tile_mut(out_of_range_x, out_of_range_y).owner = Some(1);
    game.sign_pact(HUMAN, 1, Pact::War);
    game.create_city(HUMAN, x, y);
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
    game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    let far_target = game.spawn_unit(1, UnitKind::Warrior, far_x, far_y);
    game.spawn_unit(1, UnitKind::Warrior, out_of_range_x, out_of_range_y);
    game.enable_visual_events();
    assert!(game.can_bombard_tile(game.unit_by_id(bomber).unwrap(), tx, ty));
    assert!(game.can_bombard_tile(game.unit_by_id(bomber).unwrap(), far_x, far_y));
    assert!(!game.can_bombard_tile(
        game.unit_by_id(bomber).unwrap(),
        out_of_range_x,
        out_of_range_y
    ));
    let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, x, y);
    assert!(game.can_begin_bombard(game.unit_by_id(artillery).unwrap()));
    assert!(game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), far_x, far_y));
    assert!(!game.can_bombard_tile(
        game.unit_by_id(artillery).unwrap(),
        out_of_range_x,
        out_of_range_y
    ));

    game.native_random = None;
    game.rng = 0;
    assert!(game.apply(Command::Bombard(bomber, tx, ty)));
    assert!(matches!(
        game.take_visual_events().front(),
        Some(VisualEvent::Combat(combat)) if !combat.bombard_effects
    ));
    let unit = game.unit_by_id(bomber).unwrap();
    assert_eq!((unit.x, unit.y, unit.moves_left), (x, y, 0));
    let target_hp = game.unit_by_id(far_target).unwrap().hp;
    assert!(game.apply(Command::Bombard(artillery, far_x, far_y)));
    assert!(game.unit_by_id(far_target).unwrap().hp < target_hp);
    assert_eq!(game.unit_by_id(artillery).unwrap().moves_left, 0);
    assert!(matches!(
        game.take_visual_events().front(),
        Some(VisualEvent::Combat(combat)) if combat.bombard_effects
    ));
}

#[test]
fn civ3_bombard_can_target_an_empty_enemy_city_and_uses_selected_citizen_class_rules() {
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

    assert!(game.has_civ3_bombard_capability(UnitKind::Bomber));
    assert!(game.has_civ3_bombard_capability(UnitKind::Artillery));
    assert_eq!(
        selected_unit_class_flag(UnitKind::Bomber, "KillCitizen"),
        Some(false)
    );
    assert_eq!(
        selected_unit_class_flag(UnitKind::Artillery, "KillCitizen"),
        Some(false)
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
    assert_eq!(
        city.size, 2,
        "Civ III city bombardment may select population"
    );
    assert_eq!(game.unit_by_id(bomber).unwrap().moves_left, 0);
    assert!(game.events.iter().any(|event| event.contains("bombarded")));
}

#[test]
fn bombard_rate_scales_with_health_and_matches_per_target_rng_draws() {
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
    game.create_city(HUMAN, sx, sy);
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
    let rate = (UnitKind::Bomber.bombard_rate() * bomb_unit.hp / bomb_unit.max_hit_points()).max(1);
    assert_eq!(rate, 1, "C2C3 reduces rate of fire with current HP");
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
        let lethal = bomb_unit.kind.has_lethal_bombard_against(target.kind);
        let mut hp = target.hp;
        for _ in 0..rate {
            if expected.rand_below(attack + defense) < attack {
                hp = if lethal {
                    (hp - firepower).max(0)
                } else {
                    (hp - firepower).max(1)
                };
                if hp == 0 {
                    break;
                }
            }
        }
        expected_hp.push((id, hp));
    }

    assert_eq!(game.try_bombard(&bomb_unit, tx, ty), MoveResult::Fought);
    for (id, hp) in expected_hp {
        if hp == 0 {
            assert!(game.unit_by_id(id).is_none());
        } else {
            assert_eq!(game.unit_by_id(id).unwrap().hp, hp);
        }
    }
    assert_eq!(
        game.unit_by_id(unreachable).unwrap().hp,
        UnitKind::Fighter.stats().hp
    );
    assert_eq!(game.native_random, expected.native_random);
}

#[test]
fn civ3_sea_lethal_bombard_can_sink_a_ship() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = offset_native_by_map(sx, sy, 1, 0);
    for (x, y) in [(sx, sy), (tx, ty)] {
        game.tile_mut(x, y).terrain = Terrain::Sea;
        game.tile_mut(x, y).visible = true;
    }
    game.sign_pact(HUMAN, 1, Pact::War);
    let dromon = game.spawn_unit(HUMAN, UnitKind::Dromon, sx, sy);
    let battleship = game.spawn_unit(1, UnitKind::Battleship, tx, ty);
    game.units
        .iter_mut()
        .find(|unit| unit.id == battleship)
        .unwrap()
        .hp = 1;
    let attacker = game.unit_by_id(dromon).unwrap().clone();
    let defender = game.unit_by_id(battleship).unwrap().clone();
    let attack =
        game.attack_power_against_with_base(&attacker, tx, ty, UnitKind::Dromon.bombard_strength());
    let defense = game.defense_power_against(&defender, tx, ty, UnitKind::Dromon);
    let seed = (1..1000)
        .find(|seed| {
            let mut probe = game.clone();
            probe.native_random = None;
            probe.rng = *seed;
            probe.rand_below(attack + defense) < attack
        })
        .unwrap();
    game.native_random = None;
    game.rng = seed;

    assert!(game.apply(Command::Bombard(dromon, tx, ty)));
    assert!(game.unit_by_id(battleship).is_none());
    assert!(game.unit_by_id(dromon).is_some());
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
        assert!(game.apply(Command::move_civ3_by_offset(&game, fighter, 1, 0)));
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
        assert!(!game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.transport == Some(boat))
        );
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == boat && (unit.x, unit.y) == (10, 20))
        );
    }

    #[test]
    fn carrier_without_moves_still_allows_stationed_aircraft_missions() {
        let (mut game, boat, fighter, victim) =
            loaded_hunt(2, UnitKind::Fighter, UnitKind::Carrier);
        game.units
            .iter_mut()
            .find(|unit| unit.id == boat)
            .unwrap()
            .moves_left = 0;
        game.run_ai_player(1);
        assert!(!game.units.iter().any(|unit| unit.id == victim));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == fighter && unit.transport == Some(boat))
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
        let (mut game, boat, helicopter, _victim) =
            loaded_hunt(2, UnitKind::Helicopter, UnitKind::Carrier);
        assert!(game.board_or_deboard(helicopter));
        let before = game
            .units
            .iter()
            .find(|unit| unit.id == helicopter)
            .unwrap()
            .hp;
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == helicopter
            && unit.transport == Some(boat)
            && unit.fuel == UnitKind::Helicopter.fuel_capacity()
            && unit.hp == before));
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
    fn civ3_bombard_units_use_installed_strength_rate_and_range_data() {
        let one_attack = super::super::unit_catalog::UnitFlag::OneAttack;
        assert_eq!(UnitKind::Bomber.fuel_capacity(), 1);
        assert_eq!(UnitKind::Bomber.bombard_rate(), 3);
        assert!(!UnitKind::Bomber.has_flag(one_attack));
        assert!(UnitKind::CruiseMissile.has_flag(one_attack));
        assert_eq!(UnitKind::Bomber.bombard_strength(), 12);
        assert_eq!(UnitKind::Bomber.operational_range(), 10);
        assert_eq!(UnitKind::Artillery.bombard_strength(), 12);
        assert_eq!(UnitKind::Artillery.bombard_rate(), 2);
        assert_eq!(UnitKind::Artillery.bombard_range(), 2);
        assert_eq!(UnitKind::StealthBomber.bombard_rate(), 3);
        assert_eq!(UnitKind::Battleship.bombard_range(), 2);
        assert!(UnitKind::Bomber.has_lethal_bombard_against(UnitKind::Warrior));
        assert!(UnitKind::Hwacha.has_lethal_bombard_against(UnitKind::Warrior));
        assert!(UnitKind::Dromon.has_lethal_bombard_against(UnitKind::Battleship));
        assert!(!UnitKind::Dromon.has_lethal_bombard_against(UnitKind::Warrior));

        let game = Simulation::new(crate::test_support::seeded_map());
        for kind in [
            UnitKind::Catapult,
            UnitKind::Cannon,
            UnitKind::Artillery,
            UnitKind::RadarArtillery,
            UnitKind::Hwacha,
            UnitKind::Trebuchet,
            UnitKind::Frigate,
            UnitKind::Ironclad,
            UnitKind::Destroyer,
            UnitKind::Battleship,
            UnitKind::AegisCruiser,
            UnitKind::Cruiser,
            UnitKind::ManOWar,
            UnitKind::Dromon,
            UnitKind::Fighter,
            UnitKind::Bomber,
            UnitKind::JetFighter,
            UnitKind::StealthFighter,
            UnitKind::F15,
            UnitKind::StealthBomber,
        ] {
            assert!(
                game.has_civ3_bombard_capability(kind),
                "{kind:?} has a Civ III bombard profile"
            );
        }
        for kind in [UnitKind::Archer, UnitKind::Longbowman, UnitKind::Guerilla] {
            assert!(
                !game.has_civ3_bombard_capability(kind),
                "{kind:?} has no Civ III bombard tile/operational range"
            );
        }
        assert!(
            !game.has_civ3_bombard_capability(UnitKind::CruiseMissile),
            "Cruise Missile remains a one-shot attack rather than a separate Bombard action"
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
        assert!(game.unit_visible_to_human(game.unit_by_id(hidden).unwrap()));
        let explorer = game.spawn_unit(HUMAN, UnitKind::Explorer, city.x, city.y);
        assert!(game.apply(Command::move_civ3_by_offset(&game, explorer, 1, 0)));
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
    fn fighter_uses_civ3_operational_range_for_bombard() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        let (x, y) = (10, 20);
        let (tx, ty) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.tile_mut(tx, ty).terrain = Terrain::Grassland;
        game.tile_mut(tx, ty).owner = Some(1);
        game.tile_mut(tx, ty).visible = true;
        game.sign_pact(HUMAN, 1, Pact::War);
        game.create_city(HUMAN, x, y);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        game.spawn_unit(1, UnitKind::Warrior, tx, ty);
        assert!(game.can_begin_bombard(game.unit_by_id(fighter).unwrap()));
        assert!(game.can_bombard_tile(game.unit_by_id(fighter).unwrap(), tx, ty));
        assert!(game.apply(Command::Bombard(fighter, tx, ty)));
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
        assert!(
            game.observed_city_owner(HUMAN, landing.0, landing.1)
                .is_none()
        );
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
