use super::*;
use crate::sim::fixtures::WorldBuilder;

fn campaign() -> (Simulation, u32, u32) {
    let mut g = Simulation::new(crate::test_support::seeded_map());
    let mut w = WorldBuilder::new(&mut g);
    w.clear_units_and_cities();
    let a = w.founding_city(HUMAN, (10, 24));
    let b = w.founding_city(HUMAN, (12, 24));
    w.finish();
    g.players[HUMAN].known.push(Tech::MEDITATION);
    (g, a, b)
}
#[test]
fn founding_legality_declaration_and_cooldown_are_atomic() {
    let (mut g, a, _) = campaign();
    assert!(!g.found_religion(HUMAN, a, 99));
    assert!(g.found_religion(HUMAN, a, 1));
    assert!(!g.found_religion(HUMAN, a, 2));
    assert!(g.declare_faith(HUMAN, Some(1), FaithPolicy::Devout));
    assert!(!g.declare_faith(HUMAN, None, FaithPolicy::Tolerant));
    assert_eq!(g.religion_content(g.city_by_id(a).unwrap()), 2);
    g.turn += 10;
    assert!(g.declare_faith(HUMAN, None, FaithPolicy::Tolerant));
    assert_eq!(g.religion_content(g.city_by_id(a).unwrap()), 1);
}
#[test]
fn spread_is_gradual_and_resumes_identically_without_notification_replay() {
    let (mut g, a, b) = campaign();
    g.found_religion(HUMAN, a, 1);
    g.advance_religion();
    assert_eq!(g.city_dominant_faith(b), None);
    let path = std::env::temp_dir().join(format!("religion-{}.save", std::process::id()));
    g.save_campaign(&path).unwrap();
    let mut resumed = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    for _ in 0..100 {
        g.advance_religion();
        resumed.advance_religion();
    }
    assert_eq!(g.city_dominant_faith(b), Some(1));
    assert_eq!(
        g.encode_campaign().unwrap(),
        resumed.encode_campaign().unwrap()
    );
    assert_eq!(
        g.notifications
            .iter()
            .filter(|n| n.related_city == Some(b) && n.text.contains("has reached"))
            .count(),
        1
    );
    assert_eq!(
        g.notifications
            .iter()
            .filter(|n| n.related_city == Some(b) && n.text.contains("has adopted"))
            .count(),
        1
    );
}
#[test]
fn hysteresis_and_ties_preserve_a_stable_incumbent() {
    let mut s = CityFaith::default();
    s.influence.insert(1, 100);
    assert_eq!(dominant(&s), Some(1));
    s.dominant = Some(1);
    s.influence.insert(1, 80);
    assert_eq!(dominant(&s), Some(1));
    s.influence.insert(1, 60);
    assert_eq!(dominant(&s), None);
    s.influence.insert(1, 500);
    s.influence.insert(2, 500);
    s.dominant = None;
    assert_eq!(dominant(&s), None);
}
#[test]
fn corrupt_religion_reference_is_rejected() {
    let (mut g, a, _) = campaign();
    let path = std::env::temp_dir().join(format!("religion-legacy-{}.save", std::process::id()));
    g.found_religion(HUMAN, a, 1);
    g.religion
        .cities
        .get_mut(&a)
        .unwrap()
        .influence
        .insert(99, 30);
    g.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn religion_attitude_is_directional_and_neutral_without_a_declaration() {
    let (mut g, a, _) = campaign();
    g.found_religion(HUMAN, a, 1);
    g.religion.nations.insert(
        HUMAN,
        NationalFaith {
            declared: Some(1),
            policy: FaithPolicy::Devout,
            changed: None,
        },
    );
    assert_eq!(g.religion_attitude(HUMAN, 1), 0);
    g.religion.nations.insert(
        1,
        NationalFaith {
            declared: Some(2),
            ..Default::default()
        },
    );
    assert_eq!(g.religion_attitude(HUMAN, 1), -4);
    assert_eq!(g.religion_attitude(1, HUMAN), 0);
    g.religion.nations.get_mut(&1).unwrap().declared = Some(1);
    assert_eq!(g.religion_attitude(HUMAN, 1), 4);
}

#[test]
fn snapshot_prevents_conversion_chains_and_city_order_does_not_change_results() {
    let (mut g, a, b) = campaign();
    let third = WorldBuilder::new(&mut g).founding_city(HUMAN, (18, 24));
    g.found_religion(HUMAN, a, 1);
    g.religion
        .cities
        .insert(b, CityFaith::fixture([(1, 99)].into(), None));
    let path = std::env::temp_dir().join(format!("religion-order-{}.save", std::process::id()));
    g.save_campaign(&path).unwrap();
    // Reload also exercises the serialized source/destination ordering.
    let mut reversed = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    reversed.cities.reverse();
    g.advance_religion();
    reversed.advance_religion();
    assert_eq!(g.city_dominant_faith(b), Some(1));
    assert!(!g.religion.cities[&third].influence.contains_key(&1));
    assert_eq!(
        serde_json::to_value(&g.religion).unwrap(),
        serde_json::to_value(&reversed.religion).unwrap()
    );
}

#[test]
fn trade_deduplicates_caps_pressure_and_stops_at_war() {
    let (mut g, a, _) = campaign();
    let b = WorldBuilder::new(&mut g).founding_city(1, (28, 24));
    g.sign_pact(HUMAN, 1, Pact::Peace);
    g.found_religion(HUMAN, a, 1);
    // Religion consumes established route records; route eligibility has its own tests.
    let edge = trade_routes::TradeRoute {
        from_city_id: a,
        to_city_id: b,
        goods_rule_name: "Goods".into(),
    };
    g.trade_routes = vec![
        edge.clone(),
        edge.clone(),
        trade_routes::TradeRoute {
            from_city_id: b,
            to_city_id: a,
            ..edge
        },
    ];
    g.advance_religion();
    assert_eq!(g.religion.cities[&b].influence[&1], rules().trade_pressure);
    g.religion.cities.remove(&b);
    g.sign_pact(HUMAN, 1, Pact::War);
    g.advance_religion();
    assert!(!g.religion.cities[&b].influence.contains_key(&1));
}

#[test]
fn conquest_retains_beliefs_and_razing_removes_only_city_state() {
    let (mut g, a, _) = campaign();
    g.found_religion(HUMAN, a, 1);
    g.cities.iter_mut().find(|c| c.id == a).unwrap().owner = 1;
    assert_eq!(g.city_dominant_faith(a), Some(1));
    g.remove_city_because(a, lifecycle::CityRemoval::Disaster, None);
    assert!(!g.religion.cities.contains_key(&a));
    assert_eq!(g.religion_catalog().len(), 1);
    assert!(g.valid_religion());
}

#[test]
fn policy_benefits_apply_once_to_city_history_and_mood() {
    let (mut g, a, _) = campaign();
    let index = g.city_index_by_id(a).unwrap();
    let base = g.cities[index].history;
    g.record_city_history(index);
    let ordinary = g.cities[index].history - base;
    g.found_religion(HUMAN, a, 1);
    let before = g.cities[index].history;
    g.record_city_history(index);
    assert_eq!(g.cities[index].history - before, ordinary + 1);
    g.cities[index].size = 6;
    let original = g.city_feelings(&g.cities[index], 0);
    g.declare_faith(HUMAN, Some(1), FaithPolicy::Devout);
    let devout = g.city_feelings(&g.cities[index], 0);
    assert!(original.1 > 0);
    assert_eq!(devout.1, original.1 - 1);
    assert_eq!(devout.0, original.0);
    assert_eq!(devout.2, original.2);
    assert_eq!(g.religion_culture(&g.cities[index]), 0);
}

#[test]
fn passive_observation_does_not_change_campaign_or_random_stream() {
    let (mut g, a, _) = campaign();
    g.found_religion(HUMAN, a, 1);
    let before = g.encode_campaign().unwrap();
    let random = g.native_random.clone();
    g.observe_religion();
    g.observe_diplomacy();
    assert_eq!(before, g.encode_campaign().unwrap());
    assert_eq!(random, g.native_random);
}

#[test]
fn incoming_pressure_is_capped_and_foreign_source_names_stay_hidden() {
    let (mut g, a, b) = campaign();
    g.found_religion(HUMAN, a, 1);
    for position in [(20, 24), (24, 24), (28, 24)] {
        let city = WorldBuilder::new(&mut g).founding_city(1, position);
        g.religion
            .cities
            .insert(city, CityFaith::fixture([(1, 300)].into(), Some(1)));
        g.trade_routes.push(trade_routes::TradeRoute {
            from_city_id: city,
            to_city_id: b,
            goods_rule_name: "Goods".into(),
        });
        g.tile_mut(position.0, position.1).visible = false;
    }
    g.sign_pact(HUMAN, 1, Pact::Peace);
    g.religion
        .cities
        .insert(b, CityFaith::fixture([(1, 24)].into(), None));
    g.advance_religion();
    assert_eq!(
        g.religion.cities[&b].influence[&1],
        24 - 1 + rules().incoming_cap
    );
    let notice = g
        .notifications
        .iter()
        .find(|n| n.related_city == Some(b) && n.text.contains("has reached"))
        .unwrap();
    assert!(!notice.text.contains("with influence from"));
    assert_eq!(notice.priority, NotificationPriority::Routine);
}

#[test]
fn catalog_unlocks_are_exact_and_each_faith_can_only_be_claimed_once() {
    for f in &rules().faiths {
        let (mut g, a, _) = campaign();
        g.players[HUMAN].known.clear();
        assert!(!g.found_religion(HUMAN, a, f.id));
        g.players[HUMAN].known.push(f.tech());
        assert!(g.found_religion(HUMAN, a, f.id));
        let foreign = WorldBuilder::new(&mut g).founding_city(1, (24, 24));
        g.players[1].known.push(f.tech());
        assert!(!g.found_religion(1, foreign, f.id));
        assert_eq!(g.religion_name(f.id), f.name);
        assert!(g.valid_religion());
    }
}

#[test]
fn predefined_bonuses_match_city_output_and_spread() {
    for id in 1..=7 {
        let (mut g, a, b) = campaign();
        let city = g.city_by_id(a).unwrap().clone();
        let baseline = g.split_city_trade(&city, 10, [50, 50, 0]);
        let f = definition(id).unwrap();
        g.players[HUMAN].known.push(f.tech());
        assert!(g.found_religion(HUMAN, a, id));
        let output = g.split_city_trade(&city, 10, [50, 50, 0]);
        assert_eq!(output[0] - baseline[0], f.science);
        assert_eq!(output[1] - baseline[1], f.gold);
        assert_eq!(
            g.religion_culture(&city),
            rules().tolerant_culture + f.culture
        );
        assert_eq!(g.religion_content(&city), f.content);
        let before = g.religion.cities[&a].influence[&id];
        g.advance_religion();
        assert_eq!(
            g.religion.cities[&a].influence[&id],
            before - (before + 99) / 100
                + rules().holy_pressure
                + rules().community_pressure
                + f.local_pressure
        );
        assert_eq!(
            g.religion.cities[&b].influence[&id],
            rules().local_pressure + f.outgoing_pressure
        );
    }
}

#[test]
fn islam_trade_bonus_and_fixed_identity_survive_save_resume() {
    let (mut g, a, _) = campaign();
    g.players[HUMAN].known.push(Tech::DIVINE_RIGHT);
    let b = WorldBuilder::new(&mut g).founding_city(1, (28, 24));
    g.sign_pact(HUMAN, 1, Pact::Peace);
    assert!(g.found_religion(HUMAN, a, 7));
    g.trade_routes.push(trade_routes::TradeRoute {
        from_city_id: a,
        to_city_id: b,
        goods_rule_name: "Goods".into(),
    });
    g.advance_religion();
    assert_eq!(
        g.religion.cities[&b].influence[&7],
        rules().trade_pressure + 2
    );
    let path =
        std::env::temp_dir().join(format!("religion-fixed-islam-{}.save", std::process::id()));
    g.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(loaded.has_tech(HUMAN, Tech::DIVINE_RIGHT));
    assert!(loaded.has_tech(HUMAN, Tech::MEDITATION));
    assert_eq!(loaded.religion_name(7), "Islam");
    assert_eq!(
        g.encode_campaign().unwrap(),
        loaded.encode_campaign().unwrap()
    );
}
