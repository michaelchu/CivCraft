use super::*;

fn fixture() -> Simulation {
    let mut game = war_planning_game();
    game.players[HUMAN].ai = true;
    game.players[1].ai = true;
    game.players[HUMAN].gold = 2000;
    game.players[1].gold = 0;
    game.set_scenario_diplomacy(HUMAN, 1, Pact::Alliance, 1, [6000; 2]);
    game.cities
        .iter_mut()
        .find(|city| city.owner == 1)
        .unwrap()
        .buildings
        .extend(std::iter::repeat_n(Building::LIBRARY, 8));
    game
}

#[test]
fn concrete_ally_aid_executes_atomically_without_spending_donor_reserves() {
    let mut game = fixture();
    let allowance = game.diplomatic_aid_allowance(HUMAN, 1);
    assert!(allowance > 0);
    let decision = game.recovery_negotiation(HUMAN, 1).unwrap();
    assert_eq!(decision.reason, "ally_funding");
    let offer = decision.selected.unwrap();
    let gold = game.players[HUMAN].gold + game.players[1].gold;
    assert_eq!(offer.terms[0].gold, allowance);
    assert!(game.execute_offer(HUMAN, 1, &offer));
    assert_eq!(gold, game.players[HUMAN].gold + game.players[1].gold);
    assert!(game.players[HUMAN].gold >= game.diplomatic_trade_reserve(HUMAN));
    assert_eq!(game.diplomatic_aid_allowance(HUMAN, 1), 0);
    assert!(game.ai_relationship_memory.is_empty());
}

#[test]
fn aid_requires_need_trust_surplus_and_canonical_acceptance() {
    let mut game = fixture();
    let allowance = game.diplomatic_aid_allowance(HUMAN, 1);
    let mut offer = TreatyOffer::default();
    offer.terms[0].gold = allowance + 1;
    assert!(!game.execute_offer(HUMAN, 1, &offer));
    game.players[HUMAN].gold = game.diplomatic_trade_reserve(HUMAN);
    assert_eq!(game.diplomatic_aid_allowance(HUMAN, 1), 0);
    game.players[HUMAN].gold = 2000;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[0] = 1000;
    assert_eq!(game.diplomatic_aid_allowance(HUMAN, 1), 0);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust = [6000; 2];
    game.cities
        .iter_mut()
        .find(|city| city.owner == 1)
        .unwrap()
        .buildings
        .clear();
    assert_eq!(game.diplomatic_aid_allowance(HUMAN, 1), 0);
}

#[test]
fn ai_requests_from_human_wait_for_approval_and_keep_pair_cooldown() {
    let mut game = fixture();
    game.players[HUMAN].ai = false;
    // Requests use the AI's trust, not an invented human preference score.
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[0] = 0;
    let gold = game.players[HUMAN].gold;
    game.advance_proactive_trades_for(1);
    let offer = game.treaty_offer(1).unwrap();
    assert!(offer.terms[0].gold > 0);
    assert_eq!(game.players[HUMAN].gold, gold);
    game.advance_proactive_trades_for(1);
    assert_eq!(game.treaty_offer(1), Some(offer));
    game.decline_offer(1);
    game.advance_proactive_trades_for(1);
    assert!(game.treaty_offer(1).is_none());
}

#[test]
fn economic_recovery_uses_the_same_peace_value_in_proposals_and_acceptance() {
    let mut game = fixture();
    game.players[HUMAN].gold = 0;
    game.cities
        .iter_mut()
        .find(|city| city.owner == HUMAN)
        .unwrap()
        .buildings
        .extend(std::iter::repeat_n(Building::LIBRARY, 8));
    game.set_scenario_diplomacy(HUMAN, 1, Pact::War, 1, [-6000; 2]);
    assert!(game.diplomatic_peace_value(HUMAN, 1) > 0);
    assert!(game.diplomatic_peace_value(1, HUMAN) > 0);
    let decision = game.recovery_negotiation(1, HUMAN).unwrap();
    assert_eq!(decision.reason, "war_recovery");
    assert!(game.execute_offer(HUMAN, 1, &decision.selected.unwrap()));
    assert_eq!(game.pact(HUMAN, 1), Pact::Ceasefire);
}

#[test]
fn hidden_armies_do_not_create_military_peace_pressure() {
    let mut game = war_planning_game();
    game.players[HUMAN].ai = true;
    game.players[HUMAN].gold = 5000;
    game.set_scenario_diplomacy(HUMAN, 1, Pact::War, 1, [-6000; 2]);
    for _ in 0..8 {
        game.spawn_unit(1, UnitKind::Warrior, 30, 40);
    }
    assert_eq!(game.diplomatic_peace_value(HUMAN, 1), 0);
}

#[test]
fn useful_maps_have_shared_values_and_repeated_maps_have_none() {
    let mut game = war_planning_game();
    game.players[HUMAN].ai = true;
    game.players[1].ai = true;
    game.players[HUMAN].gold = 5000;
    game.players[1].gold = 5000;
    game.set_scenario_diplomacy(HUMAN, 1, Pact::Peace, 1, [6000; 2]);
    for x in 0..12 {
        game.spawn_unit(1, UnitKind::Warrior, x, 40);
    }
    game.refresh_visibility();
    assert!(game.new_map_tiles(1, HUMAN, TreatyMap::World) >= policy().map_minimum_tiles);
    let decision = game.information_negotiation(HUMAN, 1).unwrap();
    assert!(matches!(decision.reason, "useful_map" | "map_exchange"));
    let offer = decision.selected.unwrap();
    let before = game.new_map_tiles(1, HUMAN, TreatyMap::World);
    assert!(game.execute_offer(HUMAN, 1, &offer));
    assert!(game.new_map_tiles(1, HUMAN, TreatyMap::World) < before);
    assert_eq!(
        game.diplomatic_map_value(HUMAN, 1, TreatyMap::World, false, Pact::Peace),
        Some(0)
    );
}
