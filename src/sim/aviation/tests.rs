//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn nuclear_submarines_carry_one_tactical_nuke_and_no_other_cargo() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    // Conquests BIQ cargo capacities: Submarine 0; Nuclear Submarine 1.
    assert_eq!(UnitKind::Submarine.transport_capacity(), 0);
    assert!(!UnitKind::Submarine.can_carry(UnitKind::CruiseMissile));
    assert_eq!(UnitKind::NuclearSubmarine.transport_capacity(), 1);
    assert!(!UnitKind::NuclearSubmarine.can_carry(UnitKind::Fighter));
    assert!(!UnitKind::NuclearSubmarine.can_carry(UnitKind::Warrior));
    assert!(UnitKind::NuclearSubmarine.can_carry(UnitKind::TacticalNuke));
    assert!(game.board_or_deboard(missile));
    let missile = game.unit_by_id(missile).unwrap();
    assert_eq!(missile.transport, Some(submarine));
    assert_eq!(missile.fuel, 0);
}

#[test]
fn airlift_uses_airport_capacity_and_resets_next_turn() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    let (first, second, destination) = game.airlift_playtest_setup();
    assert!(game.apply(Command::Airlift(first, destination)));
    assert!(!game.apply(Command::Airlift(second, destination)));
    assert!(game.apply(Command::EndTurn));
    assert!(game.apply(Command::Airlift(second, destination)));
}

#[test]
fn cruise_missile_bombardment_on_a_sea_unit_is_unaffected_by_sdi() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    // The imported Mech Infantry has 12 defense. The city attack/fortification
    // stages produce 405 power (12 * 15 * 150%), and SDI only intercepts
    // NuclearStrike; it does not alter regular bombardment defense.
    assert_eq!(defense(&game), 405);
    game.cities
        .last_mut()
        .unwrap()
        .buildings
        .push(Building::SDI_DEFENSE);
    assert_eq!(defense(&game), 405);
    assert_eq!(game.combat_firepower(&attacker, &target), (1, 1));
    assert_eq!(attacker.kind.fuel_capacity(), 0);
}

#[test]
fn aegis_cruisers_defend_against_air_attackers_and_sail_without_cargo() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
fn mobile_sam_air_defense_can_shoot_down_a_bombing_aircraft() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = offset_native_by_map(sx, sy, 1, 0);
    game.tile_mut(sx, sy).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, sx, sy);
    let defender = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    game.spawn_unit(1, UnitKind::MobileSAM, tx, ty);
    let defender_hp = game.unit_by_id(defender).unwrap().hp;
    assert_eq!(
        crate::sim::civ3_unit_rules::get("mobile_sam")
            .unwrap()
            .air_defense,
        4
    );

    // For Mobile SAM (Air Defense 4) versus Bomber defense 2, the source
    // combat roll has values 0..24. Rng state 1 yields a roll below 4.
    game.rng = 1;
    assert_eq!(
        game.try_bombard(&game.unit_by_id(bomber).unwrap().clone(), tx, ty),
        MoveResult::Fought
    );
    assert!(game.unit_by_id(bomber).is_none());
    assert_eq!(game.unit_by_id(defender).unwrap().hp, defender_hp);
}

#[test]
fn aircraft_survives_when_unit_air_defense_roll_fails_and_bombardment_continues() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    let (sx, sy) = (10, 20);
    let (tx, ty) = offset_native_by_map(sx, sy, 1, 0);
    game.tile_mut(sx, sy).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.sign_pact(HUMAN, 1, Pact::War);
    let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, sx, sy);
    let defender = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    game.spawn_unit(1, UnitKind::Flak, tx, ty);

    // Flak has Air Defense 2; state 2 yields roll 2, which is not below 2.
    game.rng = 2;
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    // power factor of 15. SAM and Coastal Defense each add 100%; the active
    // city fortification stage adds 50%. Ground attacks additionally get
    // City Walls (+100%) and the Great Wall (+50%).
    assert_eq!(defense(&game, UnitKind::Bomber), 540);
    assert_eq!(defense(&game, UnitKind::Helicopter), 540);
    assert_eq!(defense(&game, UnitKind::Caravel), 540);
    assert_eq!(defense(&game, UnitKind::Musketman), 810);
    assert_eq!(defense(&game, UnitKind::CruiseMissile), 810);
}

#[test]
fn helicopter_transport_uses_foot_only_cargo_and_city_access() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    let partisan = game.spawn_unit(HUMAN, UnitKind::Guerilla, x, y);
    assert!(!game.board_or_deboard(artillery));
    assert!(game.board_or_deboard(partisan));
    assert_eq!(
        game.unit_by_id(partisan).unwrap().transport,
        Some(helicopter)
    );
    assert!(game.board_or_deboard(partisan));
    assert_eq!(game.unit_by_id(partisan).unwrap().transport, None);
}

#[test]
fn conquests_air_bombard_can_be_lethal_and_does_not_move_the_bomber() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (bomber, targets) = game.bombard_playtest_setup();
    for target in targets {
        game.units
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap()
            .hp = 2;
    }
    assert!(game.apply(Command::Move(bomber, 1, 0)));
    assert_eq!(
        targets
            .iter()
            .filter(|&&id| game.unit_by_id(id).is_some())
            .count(),
        1
    );
    let aircraft = game.unit_by_id(bomber).unwrap();
    assert_eq!(
        (aircraft.x, aircraft.y, aircraft.hp, aircraft.moves_left),
        (10, 20, aircraft.max_hit_points(), 0)
    );
}

#[test]
fn conquests_artillery_bombards_within_its_source_range_without_moving() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    let (x, y) = (10, 20);
    let (tx, ty) = game.map_wrap.offset(x, y, 2, 0).unwrap();
    game.tile_mut(x, y).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).terrain = Terrain::Grassland;
    game.tile_mut(tx, ty).visible = true;
    game.sign_pact(HUMAN, 1, Pact::War);
    let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, x, y);
    let defender = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
    game.enable_visual_events();
    let unit = game
        .units
        .iter_mut()
        .find(|unit| unit.id == artillery)
        .unwrap();
    unit.moves_left = 3 * MOVE_FRAGMENTS;
    assert_eq!(UnitKind::Artillery.bombard_range(), 2);
    assert!(game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), tx, ty));
    let (far_x, far_y) = game.map_wrap.offset(x, y, 3, 0).unwrap();
    game.tile_mut(far_x, far_y).visible = true;
    assert!(!game.can_bombard_tile(game.unit_by_id(artillery).unwrap(), far_x, far_y));
    assert!(game.apply(Command::Bombard(artillery, tx, ty)));
    assert!(matches!(
        game.take_visual_events().front(),
        Some(VisualEvent::Combat(combat)) if combat.bombard_effects
    ));
    let unit = game.unit_by_id(artillery).unwrap();
    assert_eq!((unit.x, unit.y, unit.moves_left), (x, y, 0));
    assert!(game.unit_by_id(defender).unwrap().hp < 3);
}

#[test]
fn flagged_bombard_creates_craters_only_after_clearing_a_non_city_tile() {
    fn setup() -> (Simulation, u32, u32, (i32, i32)) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let (sx, sy) = (10, 20);
        let (tx, ty) = offset_native_by_map(sx, sy, 1, 0);
        game.tile_mut(sx, sy).terrain = Terrain::Grassland;
        let target_tile = game.tile_mut(tx, ty);
        target_tile.terrain = Terrain::Grassland;
        target_tile.base = None;
        target_tile.road = false;
        target_tile.railroad = false;
        target_tile.irrigation = false;
        target_tile.farmland = false;
        target_tile.mine = false;
        target_tile.oil_well = false;
        target_tile.oil_platform = false;
        game.sign_pact(HUMAN, 1, Pact::War);
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, sx, sy);
        let defender = game.spawn_unit(1, UnitKind::Warrior, tx, ty);
        game.units
            .iter_mut()
            .find(|unit| unit.id == defender)
            .unwrap()
            .hp = 1;
        game.native_random = None;
        game.rng = 0; // Every bombard check succeeds.
        (game, bomber, defender, (tx, ty))
    }

    let (mut game, bomber, defender, (tx, ty)) = setup();
    assert_eq!(
        crate::sim::civ3_unit_rules::get("bomber")
            .unwrap()
            .create_craters,
        1
    );
    assert_eq!(
        game.try_bombard(&game.unit_by_id(bomber).unwrap().clone(), tx, ty),
        MoveResult::Fought
    );
    assert!(game.unit_by_id(defender).is_none());
    assert!(game.tile(tx, ty).craters);

    let (mut occupied, bomber, _, (tx, ty)) = setup();
    occupied.spawn_unit(1, UnitKind::Warrior, tx, ty);
    assert_eq!(
        occupied.try_bombard(&occupied.unit_by_id(bomber).unwrap().clone(), tx, ty),
        MoveResult::Fought
    );
    assert!(!occupied.tile(tx, ty).craters);

    let (mut improved, bomber, _, (tx, ty)) = setup();
    improved.tile_mut(tx, ty).road = true;
    assert_eq!(
        improved.try_bombard(&improved.unit_by_id(bomber).unwrap().clone(), tx, ty),
        MoveResult::Fought
    );
    assert!(!improved.tile(tx, ty).craters);

    let (mut city, bomber, _, (tx, ty)) = setup();
    // A city square is ineligible even when its final unit is removed.
    city.create_city(1, tx, ty);
    assert_eq!(
        city.try_bombard(&city.unit_by_id(bomber).unwrap().clone(), tx, ty),
        MoveResult::Fought
    );
    assert!(!city.tile(tx, ty).craters);
}

#[test]
fn aircraft_can_return_to_airstrips_or_crash_when_out_of_fuel() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    assert_eq!((saved.x, saved.y, saved.fuel), (nx, ny, 10));
    assert!(!game.units.iter().any(|unit| unit.id == second));
}

#[cfg(test)]
mod fighter_tests {
    use super::*;

    #[test]
    fn air_superiority_fighter_intercepts_air_missions_with_source_probability_and_half_range() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        game.create_city(HUMAN, x, y);
        game.sign_pact(HUMAN, 1, Pact::War);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        assert!(game.apply(Command::AirSuperiority(fighter)));
        assert_eq!(
            game.unit_by_id(fighter).unwrap().state,
            UnitState::AirSuperiority
        );

        let inside = game.map_wrap.offset(x, y, 3, 0).unwrap();
        let outside = game.map_wrap.offset(x, y, 4, 0).unwrap();
        assert_eq!(game.map_wrap.civ3_distance(x, y, inside.0, inside.1), 3);
        assert_eq!(game.map_wrap.civ3_distance(x, y, outside.0, outside.1), 4);
        assert_eq!(
            game.air_superiority_interceptors(1, inside.0, inside.1)
                .len(),
            1
        );
        assert!(
            game.air_superiority_interceptors(1, outside.0, outside.1)
                .is_empty()
        );

        assert_eq!(
            Simulation::air_mission_interception_chance(UnitKind::Bomber),
            50
        );
        assert_eq!(
            Simulation::air_mission_interception_chance(UnitKind::StealthBomber),
            5
        );
        assert!(UnitKind::Fighter.can_attack_aircraft());
        assert!(UnitKind::JetFighter.can_attack_aircraft());
        assert!(UnitKind::F15.can_attack_aircraft());
        assert!(!UnitKind::StealthFighter.can_attack_aircraft());
    }

    #[test]
    fn air_superiority_interception_resolves_before_the_ground_bombardment() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let (x, y) = (10, 24);
        let (target_x, target_y) = game.map_wrap.offset(x, y, 2, 0).unwrap();
        let (bomber_x, bomber_y) = game.map_wrap.offset(x, y, 4, 0).unwrap();
        for (tx, ty) in [(x, y), (target_x, target_y), (bomber_x, bomber_y)] {
            game.tile_mut(tx, ty).terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, x, y);
        game.create_city(1, bomber_x, bomber_y);
        game.sign_pact(HUMAN, 1, Pact::War);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, x, y);
        assert!(game.apply(Command::AirSuperiority(fighter)));
        let bomber = game.spawn_unit(1, UnitKind::Bomber, bomber_x, bomber_y);
        game.units
            .iter_mut()
            .find(|unit| unit.id == bomber)
            .unwrap()
            .hp = 1;
        let ground_unit = game.spawn_unit(HUMAN, UnitKind::Warrior, target_x, target_y);
        game.rng = 2; // 22% interception roll, then an attacking combat roll.

        assert_eq!(
            game.try_bombard(
                &game.unit_by_id(bomber).unwrap().clone(),
                target_x,
                target_y
            ),
            MoveResult::Fought
        );
        assert!(game.unit_by_id(bomber).is_none());
        assert!(game.unit_by_id(ground_unit).is_some());
        assert_eq!(
            game.unit_by_id(ground_unit).unwrap().hp,
            UnitKind::Warrior.stats().hp
        );
    }

    #[test]
    fn ai_air_defense_fighter_patrols_and_intercepts_an_incoming_bomber_mission() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.sign_pact(HUMAN, 1, Pact::War);
        game.sign_pact(HUMAN, BARBARIANS, Pact::War);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let bomber_pos = offset_native_by_map(home.0, home.1, 2, 0);
        game.create_city(1, home.0, home.1);
        let fighter = game.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        game.units.last_mut().unwrap().moves_left =
            UnitKind::Fighter.stats().moves * MOVE_FRAGMENTS;
        let ground_unit = game.spawn_unit(BARBARIANS, UnitKind::Warrior, home.0, home.1);
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, bomber_pos.0, bomber_pos.1);
        game.units.last_mut().unwrap().moves_left = UnitKind::Bomber.stats().moves * MOVE_FRAGMENTS;

        game.run_ai_player(1);
        let patrol = game.unit_by_id(fighter).unwrap();
        assert_eq!(patrol.state, UnitState::AirSuperiority);
        assert_eq!(patrol.moves_left, 0);
        assert_eq!(
            game.air_superiority_interceptors(HUMAN, home.0, home.1)
                .len(),
            1
        );

        game.native_random = None;
        game.rng = 2; // 22% interception roll, below the stock 50% chance.
        assert!(game.at_war(HUMAN, 1));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == ground_unit && (unit.x, unit.y) == home)
        );
        assert_eq!(
            game.try_bombard(&game.unit_by_id(bomber).unwrap().clone(), home.0, home.1),
            MoveResult::Fought
        );
        assert_eq!(
            game.unit_by_id(ground_unit).unwrap().hp,
            UnitKind::Warrior.stats().hp
        );
        assert_eq!(
            game.unit_by_id(fighter).unwrap().state,
            UnitState::AirSuperiority
        );
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;

    #[test]
    fn carrier_production_and_emergency_landing_follow_aircraft_cargo_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (carrier, fighter) = game.carrier_playtest_setup();
        let port = Production::Unit(UnitKind::Carrier);
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
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

#[test]
fn rebase_uses_imported_action_flag_operational_range_and_base_destinations() {
    use crate::sim::topology::MapWrap;

    assert!(UnitKind::Fighter.can_rebase());
    assert!(UnitKind::Bomber.can_rebase());
    assert!(!UnitKind::CruiseMissile.can_rebase());
    assert!(!UnitKind::TacticalNuke.can_rebase());
    assert_eq!(UnitKind::Fighter.operational_range(), 6);

    let mut game = Simulation::new(crate::mapgen::generate_with_wrap(
        42,
        MapWrap::with_dimensions(80, 80).unwrap(),
    ));
    game.units.clear();
    game.cities.clear();
    game.prune_unit_ai_state();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.known_by[HUMAN] = true;
    }
    let home = (10, 20);
    game.create_city(HUMAN, home.0, home.1);
    let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, home.0, home.1);
    let range_rules = crate::sim::air_mission_rules::air_mission_rules();
    assert_eq!(range_rules.rebase_operational_range_multiplier, 6);
    let range =
        range_rules.rebase_operational_range_multiplier * UnitKind::Fighter.operational_range();
    let far_city = game
        .map_wrap
        .civ3_radius(home.0, home.1, range)
        .into_iter()
        .find(|&(_, _, distance)| distance == range)
        .map(|(x, y, _)| (x, y))
        .expect("custom map has a tile at the full Rebase limit");
    game.create_city(HUMAN, far_city.0, far_city.1);
    let plane = game.unit_by_id(fighter).unwrap().clone();
    assert!(game.can_rebase_to(&plane, far_city.0, far_city.1));
    let beyond_range = game
        .map_wrap
        .civ3_radius(home.0, home.1, range + 1)
        .into_iter()
        .find(|&(_, _, distance)| distance == range + 1)
        .map(|(x, y, _)| (x, y))
        .expect("custom map has a tile beyond the Rebase limit");
    game.create_city(HUMAN, beyond_range.0, beyond_range.1);
    assert!(!game.can_rebase_to(&plane, beyond_range.0, beyond_range.1));
    assert!(game.apply(Command::Rebase(fighter, far_city.0, far_city.1)));
    let landed = game.unit_by_id(fighter).unwrap();
    assert_eq!((landed.x, landed.y), far_city);
    assert!(landed.moved);
    assert_eq!(landed.moves_left, 0);
    assert_eq!(landed.transport, None);
}

#[test]
fn rebase_accepts_owned_airfields_and_compatible_carriers_but_rejects_illegal_bases() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.prune_unit_ai_state();
    let home = (10, 20);
    game.create_city(HUMAN, home.0, home.1);
    let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, home.0, home.1);
    let airfield = offset_native_by_map(home.0, home.1, 2, 0);
    game.tile_mut(airfield.0, airfield.1).terrain = Terrain::Grassland;
    game.tile_mut(airfield.0, airfield.1).base = Some(Base::Airbase);
    game.tile_mut(airfield.0, airfield.1).owner = Some(HUMAN);
    let plane = game.unit_by_id(fighter).unwrap().clone();
    assert!(game.can_rebase_to(&plane, airfield.0, airfield.1));
    game.tile_mut(airfield.0, airfield.1).owner = Some(1);
    assert!(!game.can_rebase_to(&plane, airfield.0, airfield.1));

    let deck = offset_native_by_map(home.0, home.1, 4, 0);
    game.tile_mut(deck.0, deck.1).terrain = Terrain::Sea;
    let carrier = game.spawn_unit(HUMAN, UnitKind::Carrier, deck.0, deck.1);
    assert!(game.can_rebase_to(&plane, deck.0, deck.1));
    assert!(game.apply(Command::Rebase(fighter, deck.0, deck.1)));
    assert_eq!(game.unit_by_id(fighter).unwrap().transport, Some(carrier));

    let blocked = offset_native_by_map(home.0, home.1, 5, 0);
    game.tile_mut(blocked.0, blocked.1).terrain = Terrain::Grassland;
    game.create_city(1, blocked.0, blocked.1);
    let mut second = game.unit_by_id(fighter).unwrap().clone();
    second.moved = false;
    second.moves_left = UnitKind::Fighter.stats().moves * MOVE_FRAGMENTS;
    second.transport = None;
    assert!(!game.can_rebase_to(&second, blocked.0, blocked.1));
}

#[cfg(test)]
mod stealth_tests {
    use super::*;

    #[test]
    fn fighter_operational_range_and_city_base_are_source_defined() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
            && unit.hp == actor.hp - 2 * percent_of(actor.kind.stats().hp, 10).max(1)));
        game.restore_aircraft();
        assert!(game.units.iter().any(|unit| unit.id == fighter
            && unit.transport == Some(boat)
            && unit.fuel == UnitKind::Fighter.fuel_capacity()
            && unit.hp == unit.max_hit_points()));
    }

    #[test]
    fn helicopter_uses_source_operational_range_and_does_not_emergency_embark_at_fuel_five() {
        let (mut game, _boat, helicopter, _victim) =
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
            && unit.transport.is_none()
            && unit.fuel == UnitKind::Helicopter.fuel_capacity() - 1
            && unit.hp == before - percent_of(UnitKind::Helicopter.stats().hp, 10).max(1)));
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
    fn conquests_bomber_uses_imported_range_and_bombard_rate_without_legacy_oneattack_flag() {
        let one_attack = super::super::unit_catalog::UnitFlag::OneAttack;
        assert_eq!(UnitKind::Bomber.fuel_capacity(), 10);
        assert_eq!(UnitKind::Bomber.bombard_rate(), 3);
        assert!(!UnitKind::Bomber.has_flag(one_attack));
        assert!(UnitKind::CruiseMissile.has_flag(one_attack));
    }

    #[test]
    fn imported_rebase_flags_allow_aircraft_missions_and_reject_land_units() {
        assert!(UnitKind::Fighter.can_rebase());
        assert!(UnitKind::Bomber.can_rebase());
        assert!(UnitKind::JetFighter.can_rebase());
        assert!(!UnitKind::Warrior.can_rebase());
        assert!(!UnitKind::CruiseMissile.can_rebase());
    }

    #[test]
    fn known_refuel_extras_work_on_enemy_owned_tiles() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (_, hidden) = game.stealth_aircraft_playtest_setup();
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
    fn fighter_lethal_bombard_target_class_comes_from_conquests_flags() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (id, victim) = game.ai_aircraft_advisor_playtest_setup();
        let target = game
            .units
            .iter_mut()
            .find(|unit| unit.id == victim)
            .unwrap();
        target.kind = UnitKind::Battleship;
        target.hp = UnitKind::Battleship.stats().hp;
        let plane = game.unit_by_id(id).unwrap().clone();
        let target = game.unit_by_id(victim).unwrap().clone();
        game.tile_mut(target.x, target.y).terrain = Terrain::Sea;
        assert!(UnitKind::Fighter.has_lethal_bombard_against(UnitKind::Battleship));
        assert!(!UnitKind::Fighter.has_lethal_bombard_against(UnitKind::Warrior));
        assert_ne!(
            game.try_bombard(&plane, target.x, target.y),
            MoveResult::Blocked
        );
    }

    #[test]
    fn airborne_plane_refuels_before_attacking_and_waits_for_full_fuel() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
    fn aircraft_healing_rate_matches_civilopedia_airport_and_field_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        assert_eq!(game.aircraft_repair_gain(&plane, home.0, home.1), 1);
        assert_eq!(
            game.aircraft_repair_gain(&plane, airport.0, airport.1),
            plane.max_hit_points()
        );
        assert_eq!(
            game.aircraft_repair_gain(&plane, airfield.0, airfield.1),
            plane.max_hit_points()
        );
    }

    #[test]
    fn airborne_damaged_plane_selects_repair_time_before_distance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
    fn tactical_nukes_cannot_rebase_and_airfields_are_owned_air_mission_bases() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let start = (10, 20);
        let field = (12, 20);
        game.create_city(HUMAN, start.0, start.1);
        game.tile_mut(field.0, field.1).terrain = Terrain::Grassland;
        game.tile_mut(field.0, field.1).base = Some(Base::Airbase);
        game.tile_mut(field.0, field.1).owner = Some(HUMAN);
        let fighter = game.spawn_unit(HUMAN, UnitKind::Fighter, start.0, start.1);
        let nuke = game.spawn_unit(HUMAN, UnitKind::TacticalNuke, start.0, start.1);
        assert!(game.refuel_site(HUMAN, field.0, field.1));
        assert!(game.can_rebase_to(game.unit_by_id(fighter).unwrap(), field.0, field.1));
        assert!(!game.can_begin_rebase(game.unit_by_id(nuke).unwrap()));
    }
    #[test]
    fn airlift_refreshes_contact_but_stationary_bombardment_does_not() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
        for city in &mut game.cities {
            city.buildings.push(Building::AIRPORT);
        }
        let arrival_city = game.cities[1].id;
        let visitor = game.spawn_unit(HUMAN, UnitKind::Warrior, source.0, source.1);
        let target = offset_native_by_map(destination.0, destination.1, 1, 0);
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.turn += 30;
        game.diplomatic_contact_until.clear();
        assert!(game.airlift(visitor, arrival_city));
        assert!(
            game.diplomatic_contact_until
                .get(&(HUMAN, 1))
                .is_some_and(|until| *until > game.turn)
        );
        let bomber = game.spawn_unit(HUMAN, UnitKind::Bomber, destination.0, destination.1);
        game.diplomatic_contact_until.clear();
        let aircraft = game.unit_by_id(bomber).unwrap().clone();
        assert_eq!(
            game.try_bombard(&aircraft, target.0, target.1),
            MoveResult::Fought
        );
        assert!(game.diplomatic_contact_until.is_empty());
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
        let mut game = Simulation::new(crate::mapgen::generate(42));
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
}
