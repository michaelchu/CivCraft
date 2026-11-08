//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn civ2civ3_default_government_and_initial_support_are_despotism() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    assert_eq!(support.gold, 1); // Despotism converts shields to gold; two free.
}

#[test]
fn barbarians_remove_distance_waste_but_retain_government_base_and_building_relief() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.barbarian_output_waste_playtest());
    let mut city = game.cities[0].clone();
    assert_eq!(
        game.government_output_waste_under(&city, 100, 100, None, Government::Tribal),
        (100, 70)
    );
    game.players[BARBARIANS]
        .known
        .extend([Tech::TRADE, Tech::THE_CORPORATION]);
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
fn fundamentalism_tithes_feed_actual_forecasts_and_candidate_valuation() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[1].government = Government::Fundamentalism;
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
        2
    );
    // Even a content size1 city receives both Temple/Mysticism tithes.
    assert_eq!(game.city_feelings(&temple, 0).1, 0);
    assert!(
        game.government_advisor_city_value(&temple) > game.government_advisor_city_value(&baseline)
    );
}

#[test]
fn cached_government_scores_follow_reference_cadence_but_unlocks_apply_immediately() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_government_cache_playtest());
}

#[test]
fn government_cache_interval_uses_city_count_with_five_and_twenty_turn_bounds() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
fn government_player_bonus_uses_player_context_and_gates_partisan_techs() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known.clear();
    assert_eq!(game.government_player_bonus(1, Government::Communism), 0);
    assert_eq!(game.government_player_bonus(1, Government::Federation), 0);
    assert_eq!(game.government_player_bonus(1, Government::Democracy), 0);
    assert_eq!(
        game.government_player_bonus(1, Government::Fundamentalism),
        3
    );
    game.players[1]
        .known
        .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
    game.players[HUMAN].known.push(Tech::GUERILLA_WARFARE);
    assert_eq!(game.government_player_bonus(1, Government::Communism), 3);
    assert_eq!(game.government_player_bonus(1, Government::Democracy), 3);
    assert_eq!(
        game.government_player_bonus(BARBARIANS, Government::Communism),
        0
    );
}

#[test]
fn government_source_effects_include_negative_requirements_and_veteran_class_counts() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
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
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    game.players[HUMAN].known = Tech::all().collect();
    let upkeep = game.technology_upkeep(HUMAN);
    assert!(upkeep > 0);
    let known = game.players[HUMAN].known.len();
    game.apply_research_upkeep(HUMAN, 0);
    assert_eq!(game.players[HUMAN].known.len(), known - 1);
    game.players[HUMAN].government = Government::Anarchy;
    assert_eq!(game.technology_upkeep(HUMAN), 0);
}

#[test]
fn government_upkeep_applies_output_effects_and_gold_allowances() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    let city_id = game.cities[0].id;
    for kind in [UnitKind::Warrior; 5]
        .into_iter()
        .chain([UnitKind::Caravel])
    {
        let id = game.spawn_unit(HUMAN, kind, 10, 10);
        let unit = game.units.last_mut().unwrap();
        unit.home_city = Some(city_id);
        unit.home_order = id;
    }
    // Caravel lacks ShieldToGold, but does not bypass the government's
    // shield Upkeep_Pct effect (zero in the gold-support governments).
    for (government, shields, gold) in [
        (Government::Anarchy, 4, 0),
        (Government::Tribal, 4, 0),
        (Government::Despotism, 0, 3),
        (Government::Monarchy, 0, 2),
        (Government::Communism, 3, 0),
        (Government::Fundamentalism, 0, 6),
        (Government::Federation, 0, 6),
        (Government::Republic, 5, 0),
        (Government::Democracy, 0, 8),
    ] {
        game.players[HUMAN].government = government;
        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!(
            (upkeep.food, upkeep.shields, upkeep.gold),
            (2, shields, gold),
            "{government:?}"
        );
    }
    let paid = game.city_gold_upkeep(&game.cities[0]);
    let first = paid[0].0;
    game.players[HUMAN].gold = -8;
    game.balance_unit_gold_upkeep(HUMAN, &paid);
    assert_eq!(game.players[HUMAN].gold, 0);
    assert!(!game.units.iter().any(|unit| unit.id == first));
    assert!(game.units.iter().any(|unit| unit.kind == UnitKind::Caravel));
}

#[test]
fn revolution_respects_unlocks_anarchy_and_rate_caps() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].government = Government::Tribal;
    assert!(!game.apply(Command::Government(Government::Monarchy)));
    assert!(game.apply(Command::Government(Government::Despotism)));
    let finish = game.players[HUMAN].revolution_turn.unwrap();
    assert!((1..=5).contains(&(finish - game.turn)));
    assert_eq!(game.players[HUMAN].government, Government::Anarchy);
    assert!(!game.apply(Command::RaiseRate(1)));
    game.turn = finish;
    game.finish_revolutions();
    assert_eq!(game.players[HUMAN].government, Government::Despotism);
    assert!(!game.apply(Command::Government(Government::Tribal)));
}
