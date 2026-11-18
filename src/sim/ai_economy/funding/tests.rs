use super::*;

#[test]
fn registered_wartime_funding_checks_pass() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    for (name, passed, detail) in game.wartime_funding_scenario() {
        assert!(passed, "{name}: {detail}");
    }
}

#[test]
fn forced_upkeep_losses_are_visible_without_changing_accounting() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.units.clear();
    game.cities.clear();
    game.create_city(1, 10, 24);
    game.cities[0].buildings.push(Building::LIBRARY);
    game.players[1].gold = -1;
    let mut observed = game.clone();
    observed.trace = trace::Collector::enabled(["*".to_owned()].into(), 1000);
    game.balance_national_building_upkeep(1);
    observed.balance_national_building_upkeep(1);
    assert_eq!(
        game.encode_campaign().unwrap(),
        observed.encode_campaign().unwrap()
    );
    let events = observed.trace.drain();
    assert!(
        events
            .iter()
            .any(|event| event.kind == "economy.upkeep_shortfall"
                && event.payload["action"] == "building_sale")
    );
    let unit = observed.spawn_unit(1, UnitKind::Warrior, 10, 24);
    observed.players[1].gold = -1;
    observed.balance_unit_gold_upkeep(1, &[(unit, 1)]);
    assert!(observed.unit_by_id(unit).is_none());
    assert!(
        observed
            .trace
            .drain()
            .iter()
            .any(|event| event.kind == "economy.upkeep_shortfall"
                && event.payload["action"] == "unit_upkeep_action")
    );
}

#[test]
fn forecast_at_proposed_rates_matches_an_actual_rate_change_and_preserves_workers() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.wartime_funding_scenario();
    let before = game.encode_campaign().unwrap();
    for rates in [[70, 30, 0], [40, 40, 20], [0, 70, 30]] {
        let projected = game.ai_projected_income_at_rates(1, rates);
        let mut changed = game.clone();
        changed.players[1].rates = rates;
        assert_eq!(projected, changed.ai_projected_income(1));
    }
    assert_eq!(game.encode_campaign().unwrap(), before);
}

#[test]
fn suggested_defense_is_not_a_purchase_commitment_in_a_guarded_city() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.wartime_funding_scenario();
    game.players[1].known.push(Tech::MASONRY);
    game.cities[0].production = Production::Building(Building::LIBRARY);
    game.cities[1].production = Production::Building(Building::LIBRARY);
    game.spawn_unit(1, UnitKind::Spearman, 10, 24);
    game.spawn_unit(1, UnitKind::Spearman, 18, 30);
    let threat = offset_native_by_map(10, 24, 1, 0);
    for _ in 0..3 {
        game.spawn_unit(2, UnitKind::Warrior, threat.0, threat.1);
    }
    game.refresh_visibility();
    assert!(game.advisor_guard_city_assessment(&game.cities[0]).urgency > 1);
    assert!(game.native_city_defense_choice(&game.cities[0]).0.is_some());
    let need = game.ai_military_funding_need(1, false);
    assert_eq!(
        need.purchase(),
        0,
        "Unselected advice must not starve research"
    );
}

#[test]
fn an_unaffordable_rush_does_not_consume_all_discretionary_research() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.wartime_funding_scenario();
    game.players[1].gold = 0;
    game.cities[0].production = Production::Unit(UnitKind::Warrior);
    game.cities[0].shields = 0;
    let price = game.rush_cost(&game.cities[0]).unwrap();
    let strategy = game.strategic_priorities(1);
    let decision = game.ai_funding_decision(
        1,
        [70, 30, 0],
        &strategy,
        MilitaryNeed {
            routine_purchase: 0,
            urgent_purchase: price,
        },
    );
    assert!(!decision.urgent_purchase_affordable);
    assert!(decision.rates[0] >= decision.refill_science_floor);
    assert!(decision.rates[0] > 0);
    game.players[1].gold = price + game.strategic_upkeep_buffer(1) + strategy.gold_reserve;
    let decision = game.ai_funding_decision(
        1,
        [70, 30, 0],
        &game.strategic_priorities(1),
        MilitaryNeed {
            routine_purchase: 0,
            urgent_purchase: price,
        },
    );
    assert_eq!(decision.rates, [70, 30, 0]);
}

#[test]
fn a_funded_urgent_purchase_does_not_waive_research_for_other_preparation_costs() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.wartime_funding_scenario();
    game.players[1].gold = 100;
    let strategy = game.strategic_priorities(1);
    let decision = game.ai_funding_decision(
        1,
        [70, 30, 0],
        &strategy,
        MilitaryNeed {
            routine_purchase: 1000,
            urgent_purchase: 10,
        },
    );
    assert!(decision.urgent_purchase_affordable);
    assert!(decision.rates[0] >= decision.refill_science_floor);
}

#[test]
fn scientific_traits_preserve_a_larger_share_during_discretionary_military_refills() {
    let mut game = Simulation::new(crate::test_support::seeded_map());
    game.wartime_funding_scenario();
    game.players[1].gold = 0;
    let mut choices = Vec::new();
    for id in ["rome", "korea"] {
        game.players[1].civilization_id = Some(id.into());
        let decision = game.ai_funding_decision(
            1,
            [70, 30, 0],
            &game.strategic_priorities(1),
            MilitaryNeed {
                routine_purchase: 1000,
                urgent_purchase: 0,
            },
        );
        assert!(decision.projected_income >= 0);
        choices.push(decision.rates[0]);
    }
    assert!(choices[1] > choices[0]);
}
