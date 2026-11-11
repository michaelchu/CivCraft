//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn default_government_is_despotism_with_civ3_treasury_support() {
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
    assert_eq!(support.food, 0);
    assert_eq!(support.gold, 0); // Three supported units fit within four free.
}

#[test]
fn barbarians_keep_the_current_government_output_waste_rules() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.barbarian_output_waste_playtest());
    let mut city = game.cities[0].clone();
    assert_eq!(
        game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
        (90, 80)
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
fn conquests_governments_have_no_fundamentalism_tithe_effect() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    game.players[1].government = Government::Fascism;
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
fn fascism_has_no_inherited_fanatic_bonus_and_partisan_bonus_is_player_scoped() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known.clear();
    assert_eq!(game.government_player_bonus(1, Government::Communism), 0);
    assert_eq!(game.government_player_bonus(1, Government::Feudalism), 0);
    assert_eq!(game.government_player_bonus(1, Government::Democracy), 0);
    assert_eq!(game.government_player_bonus(1, Government::Fascism), 0);
    game.players[1]
        .known
        .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
    game.players[HUMAN].known.push(Tech::NATIONALISM);
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
fn conquests_governments_have_their_source_free_slots_and_gold_rates() {
    for (government, slots, cost) in [
        (Government::Anarchy, [0, 0, 0], 1),
        (Government::Despotism, [4, 4, 4], 1),
        (Government::Monarchy, [2, 4, 8], 1),
        (Government::Communism, [6, 6, 6], 1),
        (Government::Fascism, [4, 7, 10], 1),
        (Government::Feudalism, [5, 2, 1], 3),
        (Government::Republic, [1, 3, 4], 2),
        (Government::Democracy, [0, 0, 0], 1),
    ] {
        let policy = government.unit_support().unwrap();
        assert_eq!(
            [policy.town, policy.city, policy.metropolis],
            slots,
            "{government:?}"
        );
        assert_eq!(policy.cost_per_unit, cost, "{government:?}");
    }
}

#[test]
fn revolution_respects_unlocks_anarchy_and_rate_caps() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[HUMAN].government = Government::Despotism;
    assert!(!game.apply(Command::Government(Government::Monarchy)));
    game.players[HUMAN].known.push(Tech::MONARCHY);
    assert!(game.apply(Command::Government(Government::Monarchy)));
    let finish = game.players[HUMAN].revolution_turn.unwrap();
    assert!((1..=5).contains(&(finish - game.turn)));
    assert_eq!(game.players[HUMAN].government, Government::Anarchy);
    assert!(!game.apply(Command::RaiseRate(1)));
    game.turn = finish;
    game.finish_revolutions();
    assert_eq!(game.players[HUMAN].government, Government::Monarchy);
    assert!(!game.apply(Command::Government(Government::Monarchy)));
}
