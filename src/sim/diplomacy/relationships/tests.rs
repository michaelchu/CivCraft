use super::*;

#[test]
fn plans_and_senate_refusals_do_not_earn_ally_support() {
    let mut game = war_planning_game();
    game.set_scenario_diplomacy(1, 2, Pact::Alliance, 1, [6000; 2]);
    game.set_scenario_diplomacy(HUMAN, 2, Pact::War, 1, [0; 2]);
    game.begin_ai_war_plan(1, HUMAN, 3, WarReason::Alliance);
    assert!(game.ai_relationship_memory.is_empty());
    game.players[1].government = Government::Democracy;
    assert!(!game.cancel_pact(1, HUMAN));
    assert!(game.ai_relationship_memory.is_empty());
}

#[test]
fn relationship_credit_is_bounded_directional_and_decays_without_resetting_clock() {
    let mut game = war_planning_game();
    game.remember_relationship(1, HUMAN, "pact_breached");
    game.remember_relationship(1, HUMAN, "pact_breached");
    assert_eq!(game.relationship_memory(1, HUMAN).grievance, 50);
    assert_eq!(game.relationship_memory(HUMAN, 1).grievance, 0);
    game.remember_relationship(1, HUMAN, "completed_trade");
    game.turn += 5;
    game.remember_relationship(1, HUMAN, "completed_trade");
    assert_eq!(game.relationship_memory(1, HUMAN).commerce, 8);
    game.turn += 1;
    game.remember_relationship(1, HUMAN, "completed_trade");
    assert_eq!(game.relationship_memory(1, HUMAN).commerce, 16);
    game.turn += 6;
    assert_eq!(game.relationship_memory(1, HUMAN).commerce, 6);
    assert_eq!(game.relationship_memory(1, HUMAN).grievance, 37);
    for _ in 0..80 {
        game.turn += 6;
        game.remember_relationship(1, HUMAN, "completed_trade");
        game.remember_relationship(1, HUMAN, "ally_supported");
    }
    let memory = game.relationship_memory(1, HUMAN);
    assert!((0..=100).contains(&memory.commerce));
    assert!((0..=100).contains(&memory.loyalty));
    assert_eq!(memory.grievance, 0);
    assert!(game.relationship_trade_adjustment(1, HUMAN) <= 10);
    game.turn += 500;
    let memory = game.relationship_memory(1, HUMAN);
    assert_eq!(
        (memory.commerce, memory.loyalty, memory.grievance),
        (0, 0, 0)
    );
}

#[test]
fn remembered_betrayal_changes_policy_but_preserves_human_choice() {
    let mut game = war_planning_game();
    game.players[1].ai = true;
    game.remember_relationship(1, HUMAN, "pact_breached");
    assert!(game.relationship_caution(1, HUMAN));
    assert!(game.relationship_attitude_bonus(1, HUMAN) < 0);
    assert!(game.relationship_trade_adjustment(1, HUMAN) < 0);
    assert!(game.relationship_war_adjustment(1, HUMAN) > 0);
    assert!(
        game.pact_refusal(1, HUMAN, Pact::Alliance)
            .unwrap()
            .contains("rebuild trust")
    );
    game.players[1]
        .known
        .extend([Tech::BRONZE_WORKING, Tech::IRON_WORKING]);
    game.players[HUMAN].known.push(Tech::BRONZE_WORKING);
    game.set_scenario_diplomacy(1, HUMAN, Pact::Alliance, 1, [6000; 2]);
    assert_eq!(
        game.diplomatic_technology_value(1, HUMAN, Tech::IRON_WORKING, true),
        Err("military_security")
    );
    game.players[1].ai = false;
    assert!(!game.relationship_caution(1, HUMAN));
    assert_eq!(game.relationship_trade_adjustment(1, HUMAN), 0);
}

#[test]
fn offers_and_gifts_do_not_create_completed_exchange_memory() {
    let mut game = war_planning_game();
    let mut offer = TreatyOffer {
        pact: None,
        terms: Default::default(),
    };
    offer.terms[0].gold = 10;
    game.remember_completed_trade(HUMAN, 1, &offer);
    offer.terms[1].gold = 10;
    game.remember_completed_trade(HUMAN, 1, &offer);
    assert!(game.ai_relationship_memory.is_empty());
    offer.terms[1].gold = 0;
    offer.terms[1].technology = Some(Tech::IRON_WORKING);
    assert!(!game.execute_offer(HUMAN, 1, &offer));
    assert!(game.ai_relationship_memory.is_empty());
    offer.terms[0].gold = 0;
    game.remember_completed_trade(HUMAN, 1, &offer);
    assert!(game.ai_relationship_memory.is_empty());
    game.begin_ai_war_plan(1, HUMAN, 5, WarReason::Alliance);
    assert!(game.ai_relationship_memory.is_empty());
}

#[test]
fn reliable_partner_receives_better_terms_and_optional_war_protection() {
    let mut game = war_planning_game();
    game.players[1].ai = true;
    let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
    game.players[1].known.extend([Tech::ALPHABET, writing]);
    game.players[HUMAN].known = vec![Tech::ALPHABET];
    game.set_scenario_diplomacy(1, HUMAN, Pact::Alliance, 1, [6000; 2]);
    let ordinary = game
        .diplomatic_technology_value(1, HUMAN, writing, true)
        .unwrap();
    for _ in 0..20 {
        game.remember_relationship(1, HUMAN, "completed_trade");
        game.turn += 6;
    }
    let friendly = game
        .diplomatic_technology_value(1, HUMAN, writing, true)
        .unwrap();
    assert!(
        friendly < ordinary,
        "friendly={friendly}, ordinary={ordinary}, commerce={}",
        game.relationship_memory(1, HUMAN).commerce
    );
    assert!(game.relationship_attitude_bonus(1, HUMAN) > 0);
    assert!(game.relationship_war_adjustment(1, HUMAN) < 0);
}

#[test]
fn relationship_memory_survives_saves_and_rejects_corrupt_history() {
    let mut game = war_planning_game();
    game.remember_relationship(1, HUMAN, "completed_trade");
    game.remember_relationship(1, HUMAN, "pact_breached");
    let path = crate::sim::save::tests::temporary_save("relationships");
    game.save_campaign(&path).unwrap();
    let loaded = Simulation::load_campaign(&path).unwrap();
    assert_eq!(
        game.encode_campaign().unwrap(),
        loaded.encode_campaign().unwrap()
    );
    let memory = game.ai_relationship_memory.get_mut(&(1, HUMAN)).unwrap();
    memory.last_trade = Some(game.turn + 1);
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    let memory = game.ai_relationship_memory.get_mut(&(1, HUMAN)).unwrap();
    memory.last_trade = None;
    memory.commerce = 101;
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    std::fs::remove_file(path).unwrap();
}
