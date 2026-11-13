//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn default_government_uses_selected_ruleset_unit_upkeep() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    // nations.ruleset31; nation.c:init_government_of_nation; includes
    // nonplayable Barbarian/Pirate nations without an explicit override.
    assert!(
        game.players
            .iter()
            .all(|player| player.government == Government::Despotism)
    );
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    for _ in 0..3 {
        let id = game.spawn_unit(1, UnitKind::Warrior, 12, 24);
        let home = game.cities[0].id;
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.home_city = Some(home);
        unit.home_order = id;
    }
    let support = game.city_unit_upkeep(&game.cities[0]);
    assert_eq!(support.shields, 0);
    assert_eq!(support.food, 0);
    assert_eq!(support.gold, 1); // Despotism's two free gold slots leave one paid.
}

#[test]
fn barbarians_keep_the_current_government_output_waste_rules() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.barbarian_output_waste_playtest());
    let mut city = game.cities[0].clone();
    assert_eq!(
        game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
        (90, 80)
    );
    game.players[BARBARIANS]
        .known
        .extend([Tech::ECONOMICS, Tech::THE_CORPORATION]);
    assert_eq!(
        game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
        (90, 80)
    );
    city.buildings.push(Building::PALACE);
    assert_eq!(
        game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
        (95, 90)
    );
}

#[test]
fn selected_ruleset_governments_have_no_fundamentalism_tithe_effect() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.players[1].government = Government::Democracy;
    game.players[1].known = vec![Tech::MYSTICISM];
    game.players[1].rates = [0, 100, 0];
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(1, 12, 24);
    let baseline = game.cities[0].clone();
    let mut temple = baseline.clone();
    temple.buildings.push(Building::TEMPLE);
    assert_eq!(
        game.city_forecast(&temple).gross_gold - game.city_forecast(&baseline).gross_gold,
        0
    );
}

#[test]
fn cached_government_scores_follow_reference_cadence_but_unlocks_apply_immediately() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert!(game.ai_government_cache_playtest());
}

#[test]
fn government_cache_interval_uses_city_count_with_five_and_twenty_turn_bounds() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.turn = 0;
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
    }
    for index in 0..21 {
        game.create_city(1, 2 + (index % 6) * 5, 4 + (index / 6) * 12);
    }
    game.refresh_government_advisor(1);
    assert_eq!(game.ai_government_cache[&1].next_evaluation, 20);
    game.cities.truncate(6);
    game.turn = 19;
    game.refresh_government_advisor(1);
    assert_eq!(game.ai_government_cache[&1].next_evaluation, 20);
    game.turn = 20;
    game.refresh_government_advisor(1);
    assert_eq!(game.ai_government_cache[&1].next_evaluation, 26);
    game.cities.truncate(5);
    game.turn = 26;
    game.refresh_government_advisor(1);
    assert_eq!(game.ai_government_cache[&1].next_evaluation, 31);
}

#[test]
fn government_player_bonus_uses_c2c3_fanatics_and_partisans_effects() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.players[1].known.clear();
    assert_eq!(game.government_player_bonus(1, Government::Communism), 0);
    assert_eq!(game.government_player_bonus(1, Government::Democracy), 0);
    assert_eq!(
        game.government_player_bonus(1, Government::Fundamentalism),
        3
    );
    game.players[1]
        .known
        .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
    game.players[HUMAN].known.push(Tech::NATIONALISM);
    assert_eq!(game.government_player_bonus(1, Government::Communism), 3);
    assert_eq!(game.government_player_bonus(1, Government::Democracy), 3);
    assert_eq!(
        game.government_player_bonus(1, Government::Fundamentalism),
        3
    );
    assert_eq!(
        game.government_player_bonus(BARBARIANS, Government::Communism),
        0
    );
}

#[test]
fn government_source_effects_include_negative_requirements_and_veteran_class_counts() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.players[1].government = Government::Communism;
    let city = game.cities[0].clone();
    let base = game.advisor_city_output_exact(&city, None);
    // Source: Veteran_Build(3) minus excluded free-military effects(3+4).
    assert_eq!(game.government_advisor_city_value(&city) - base, -4.0);
    game.spawn_unit(1, UnitKind::Warrior, 10, 24);
    assert_eq!(
        game.government_advisor_city_value(&city) - game.advisor_city_output_exact(&city, None),
        -3.0
    );
}

#[test]
fn anarchy_waives_normal_technology_upkeep() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 10, 10);
    game.players[HUMAN].known = Tech::all().collect();
    game.players[HUMAN].future_techs = 100;
    let upkeep = game.technology_upkeep(HUMAN);
    assert!(upkeep > 0);
    let futures = game.players[HUMAN].future_techs;
    game.apply_research_upkeep(HUMAN, 0);
    assert_eq!(game.players[HUMAN].future_techs, futures - 1);
    game.players[HUMAN].future_techs = 0;
    game.players[HUMAN].government = Government::Anarchy;
    assert_eq!(game.technology_upkeep(HUMAN), 0);
}

#[test]
fn revolution_respects_unlocks_anarchy_and_rate_caps() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.players[HUMAN].government = Government::Despotism;
    assert!(!game.apply(Command::Government(Government::Monarchy)));
    game.players[HUMAN].known.push(Tech::MONARCHY);
    assert!(game.apply(Command::Government(Government::Monarchy)));
    let finish = game.players[HUMAN].revolution_turn.unwrap();
    assert!((1..=5).contains(&(finish - game.turn)));
    assert_eq!(
        game.players[HUMAN].government,
        Government::during_revolution()
    );
    assert!(!game.apply(Command::RaiseRate(1)));
    game.turn = finish;
    game.finish_revolutions();
    assert_eq!(game.players[HUMAN].government, Government::Monarchy);
    assert!(!game.apply(Command::Government(Government::Monarchy)));
}

#[test]
fn zero_turn_ai_government_change_is_committed_at_the_phase_boundary() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.players[1].ai = true;
    game.players[1].known.push(Tech::MONARCHY);
    game.players[1].rates = [20, 80, 0];

    assert!(game.change_government(1, Government::Monarchy));
    assert_eq!(game.players[1].government, Government::during_revolution());
    assert_eq!(
        game.players[1].revolution_target,
        Some(Government::Monarchy)
    );
    assert_eq!(game.players[1].revolution_turn, Some(game.turn));

    game.finish_revolutions();
    assert_eq!(game.players[1].government, Government::Monarchy);
    // Default Freeciv AI has no H_RATES handicap and keeps its rates above
    // the government maximum.
    assert_eq!(game.players[1].rates, [20, 80, 0]);
}

#[test]
fn ruleset_no_anarchy_effect_allows_and_defers_a_human_government_change() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.create_city(HUMAN, 10, 20);
    game.cities[0].buildings.push(Building::STATUE_OF_LIBERTY);

    assert!(game.change_government(HUMAN, Government::Federation));
    assert_eq!(
        game.players[HUMAN].government,
        Government::during_revolution()
    );
    assert_eq!(
        game.players[HUMAN].revolution_target,
        Some(Government::Federation)
    );
    assert_eq!(game.players[HUMAN].revolution_turn, Some(game.turn));

    game.finish_revolutions();
    assert_eq!(game.players[HUMAN].government, Government::Federation);
}

#[test]
fn human_rate_reallocation_preserves_luxury_then_maximizes_science() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.players[HUMAN].government = Government::during_revolution();
    game.players[HUMAN].revolution_target = Some(Government::Despotism);
    game.players[HUMAN].revolution_turn = Some(game.turn);
    game.players[HUMAN].rates = [20, 80, 0];

    game.finish_revolutions();

    assert_eq!(game.players[HUMAN].government, Government::Despotism);
    assert_eq!(game.players[HUMAN].rates, [60, 40, 0]);
}

#[test]
fn eliminated_players_do_not_finish_or_count_government_revolutions() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.cities.retain(|city| city.owner != 1);
    game.units.retain(|unit| unit.owner != 1);
    game.players[1].government = Government::during_revolution();
    game.players[1].revolution_target = Some(Government::Monarchy);
    game.players[1].revolution_turn = Some(game.turn);
    let adoptions = game.government_adoptions[Government::Monarchy as usize];

    game.finish_revolutions();

    assert_eq!(game.players[1].government, Government::during_revolution());
    assert_eq!(
        game.players[1].revolution_target,
        Some(Government::Monarchy)
    );
    assert_eq!(
        game.government_adoptions[Government::Monarchy as usize],
        adoptions
    );
}

#[test]
fn random_quick_revolution_length_handles_the_engine_minimum_setting() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    assert_eq!(
        game.random_quick_revolution_length(Government::Monarchy, 1),
        1
    );
}

#[test]
fn random_quick_adoption_acceleration_counts_used_civilization_and_barbarian_slots() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    let target = Government::Monarchy;
    let length = 5;
    game.government_adoptions[target as usize] = 2;
    let players = game.civilization_count as i32 + 1;
    let change_speed = ((players / 2) / (length - 1)).max(1);
    let max_turns = (length - 2 / change_speed).max(1);
    let mut expected = game.clone();
    let expected_turns = expected.rand_below(max_turns) + 1;

    assert_eq!(
        game.random_quick_revolution_length(target, length),
        expected_turns
    );
    assert_eq!(game.random_snapshot(), expected.random_snapshot());
}

#[test]
fn revolution_recomputes_trade_output_without_changing_saved_rates() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(HUMAN, 10, 24);
    game.players[HUMAN].rates = [60, 40, 0];
    game.players[HUMAN].known.push(Tech::MONARCHY);
    assert!(game.change_government(HUMAN, Government::Monarchy));
    assert_eq!(
        game.players[HUMAN].government,
        Government::during_revolution()
    );
    let city = game.cities[0].clone();

    let actual = game.city_trade(&city, 10);
    let expected = game.split_city_trade(&city, 10, [0, 0, 100]);

    assert_eq!(actual, expected);
    assert_eq!(game.players[HUMAN].rates, [60, 40, 0]);
    assert_eq!(actual[0], 0, "Anarchy produces no science from trade");
    assert_eq!(actual[1], 0, "Anarchy produces no gold from trade");
    assert_eq!(
        actual[2], 5,
        "C2C3's -50% Anarchy luxury output bonus applies after trade allocation"
    );
}

#[test]
fn republic_revolution_threshold_triggers_on_the_third_disorder_turn() {
    let (mut game, city_id) = republic_disorder_scenario();
    for consecutive_turn in 1..=2 {
        game.update_city_disorder_revolution(city_id);
        assert_eq!(game.cities[0].anarchy, consecutive_turn);
        assert_eq!(game.players[HUMAN].government, Government::Republic);
        assert_eq!(game.players[HUMAN].revolution_turn, None);
    }

    game.update_city_disorder_revolution(city_id);

    assert_eq!(game.cities[0].anarchy, 3);
    assert_eq!(
        game.players[HUMAN].government,
        Government::during_revolution()
    );
    assert_eq!(
        game.players[HUMAN].revolution_target,
        Some(Government::Republic)
    );
    assert!(game.players[HUMAN].revolution_turn.unwrap() > game.turn);
}

#[test]
fn republic_content_turn_resets_disorder_revolution_counter() {
    let (mut game, city_id) = republic_disorder_scenario();
    for _ in 0..2 {
        game.update_city_disorder_revolution(city_id);
    }
    assert_eq!(game.cities[0].anarchy, 2);

    // Two Elvis specialists provide enough luxury to make the sole unhappy
    // citizen content, producing a non-disorder city turn.
    game.cities[0].specialists = [0, 0, 2];
    assert!(!game.city_forecast(&game.cities[0]).disorder);
    game.update_city_disorder_revolution(city_id);

    assert_eq!(game.cities[0].anarchy, 0);
    assert_eq!(game.players[HUMAN].government, Government::Republic);
    assert_eq!(game.players[HUMAN].revolution_turn, None);
}

#[test]
fn statue_of_liberty_makes_republic_disorder_revolution_immediate() {
    let (mut game, city_id) = republic_disorder_scenario();
    game.cities[0].buildings.push(Building::STATUE_OF_LIBERTY);
    for _ in 0..3 {
        game.update_city_disorder_revolution(city_id);
    }

    assert_eq!(game.cities[0].anarchy, 3);
    assert_eq!(
        game.players[HUMAN].government,
        Government::during_revolution()
    );
    assert_eq!(
        game.players[HUMAN].revolution_target,
        Some(Government::Republic)
    );
    assert_eq!(game.players[HUMAN].revolution_turn, Some(game.turn));

    game.finish_revolutions();

    assert_eq!(game.players[HUMAN].government, Government::Republic);
}

fn republic_disorder_scenario() -> (Simulation, u32) {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
    }
    game.create_city(HUMAN, 12, 24);
    game.cities[0].size = 5;
    game.cities[0].specialists = [0; 3];
    game.players[HUMAN].government = Government::Republic;
    game.players[HUMAN].known.push(Tech::THE_REPUBLIC);
    game.players[HUMAN].rates = [0, 100, 0];
    assert!(game.city_forecast(&game.cities[0]).disorder);
    let city_id = game.cities[0].id;
    (game, city_id)
}

#[test]
fn mobilization_requires_nationalism_and_peace_treaty_returns_to_normalcy() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.players[HUMAN].known.clear();
    assert!(!game.apply(crate::sim::Command::Mobilize));
    assert!(!game.players[HUMAN].mobilized);

    game.players[HUMAN].known.push(Tech::NATIONALISM);
    assert!(game.apply(crate::sim::Command::Mobilize));
    assert!(game.players[HUMAN].mobilized);
    assert!(!game.apply(crate::sim::Command::Mobilize));

    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    assert!(game.players[HUMAN].mobilized);

    game.sign_pact(HUMAN, 1, Pact::Peace);
    assert!(!game.players[HUMAN].mobilized);
}
