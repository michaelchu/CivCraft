use super::*;

fn city_fixture(size: i32) -> (Simulation, u32) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.ai_ground_targets.clear();
    game.ai_settlement_targets.clear();
    game.ai_guard_targets.clear();
    game.ai_unit_guards.clear();
    game.hunter_targets.clear();
    game.ai_ferries = Default::default();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.hut = false;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.known_by[1] = true;
    }
    game.players[1].government = Government::Despotism;
    game.create_city(1, 10, 24);
    game.cities[0].size = size;
    let city_id = game.cities[0].id;
    (game, city_id)
}

fn set_moves(game: &mut Simulation, id: u32) {
    game.units
        .iter_mut()
        .find(|unit| unit.id == id)
        .unwrap()
        .moves_left = 10;
}

fn add_danger(game: &mut Simulation, x: i32, y: i32) {
    game.sign_pact(1, HUMAN, Pact::War);
    let (enemy_x, enemy_y) = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();
    let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, enemy_x, enemy_y);
    set_moves(game, enemy);
}

#[test]
fn city_defender_prepass_selects_a_garrison_against_nearby_danger() {
    let (mut game, _) = city_fixture(1);
    let (x, y) = (10, 24);
    let defender = game.spawn_unit(1, UnitKind::Warrior, x, y);
    set_moves(&mut game, defender);
    let second = game.spawn_unit(1, UnitKind::Warrior, x, y);
    add_danger(&mut game, x, y);
    let marine = game.spawn_unit(HUMAN, UnitKind::Marine, 14, 24);
    set_moves(&mut game, marine);

    let selected = game.assign_ai_city_defenders(1);

    assert!(game.advisor_guard_city_assessment(&game.cities[0]).danger > 0);
    assert!(selected.contains(&defender));
    assert!(selected.contains(&second));
}

#[test]
fn city_defender_prepass_selects_a_unit_for_martial_law() {
    let (mut game, _) = city_fixture(3);
    let (x, y) = (10, 24);
    game.cities[0].specialists[2] = 2;
    let defender = game.spawn_unit(1, UnitKind::Warrior, x, y);
    let second = game.spawn_unit(1, UnitKind::Warrior, x, y);

    let selected = game.assign_ai_city_defenders(1);

    assert!(
        game.city_rule_effect(
            &game.cities[0],
            "Martial_Law_Max",
            None,
            None,
            None,
            false,
            game.players[1].government,
            None,
        ) > 0
    );
    assert!(selected.contains(&defender));
    assert!(selected.contains(&second));
}

#[test]
fn city_defender_emergency_can_take_over_an_existing_guard_role() {
    let (mut game, _) = city_fixture(1);
    let (x, y) = (10, 24);
    game.create_city(1, 18, 24);
    let other_city = game.cities[1].id;
    let defender = game.spawn_unit(1, UnitKind::Warrior, x, y);
    game.ai_guard_targets
        .insert(defender, crate::sim::guards::GuardTarget::City(other_city));
    add_danger(&mut game, x, y);

    let selected = game.assign_ai_city_defenders(1);

    assert!(selected.contains(&defender));
    assert!(!game.ai_guard_targets.contains_key(&defender));
}

#[test]
fn reserved_city_defenders_are_not_sent_on_counterattacks() {
    let (mut game, _) = city_fixture(1);
    let (x, y) = (10, 24);
    let first = game.spawn_unit(1, UnitKind::Warrior, x, y);
    let second = game.spawn_unit(1, UnitKind::Warrior, x, y);
    for id in [first, second] {
        set_moves(&mut game, id);
    }
    add_danger(&mut game, x, y);

    let reserved = game.assign_ai_city_defenders(1);
    assert!(!reserved.is_empty());
    let counterattacks = game.counterattack_from_ai_cities(1, &reserved);

    assert!(reserved.is_disjoint(&counterattacks));
    assert!(reserved.iter().all(|id| {
        game.unit_by_id(*id)
            .is_some_and(|unit| (unit.x, unit.y) == (x, y))
    }));
}

#[test]
fn city_defender_wall_scaling_uses_selected_ruleset_defend_effects() {
    let (mut game, _) = city_fixture(1);
    let (x, y) = (10, 24);
    game.tile_mut(x, y).base = Some(Base::Fortress);
    game.spawn_unit(1, UnitKind::Warrior, x, y);

    assert!(game.advisor_city_has_defense_effect(&game.cities[0]));
    let assessment = game.advisor_guard_city_assessment(&game.cities[0]);
    // Native applies a 0.8 multiplier here because no threat sets wallvalue
    // to 5; the no-wall score remains the unmodified quadratic assessment.
    assert!(assessment.defense < assessment.defense_igwall);
}
