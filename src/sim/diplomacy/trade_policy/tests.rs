use super::*;

fn writing() -> Tech {
    Tech::all().find(|tech| tech.name() == "Writing").unwrap()
}

fn fixture() -> Simulation {
    let mut game = war_planning_game();
    for owner in [HUMAN, 1] {
        game.players[owner].ai = true;
        game.players[owner].known = vec![Tech::ALPHABET];
        game.players[owner].researching = Tech::POTTERY;
        game.players[owner].research_goal = Tech::UNSET;
        game.players[owner].research = 0;
        game.players[owner].last_science = 20;
    }
    game.players[1].known.push(writing());
    game.players[HUMAN].gold = 5000;
    game.players[1].gold = 0;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust = [6000; 2];
    game
}

#[test]
fn an_ally_excluded_by_the_war_target_filter_can_sell_for_cash() {
    let mut game = fixture();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    let before = [game.players[HUMAN].gold, game.players[1].gold];
    game.advance_diplomatic_offers_for(HUMAN);
    assert!(game.has_tech(HUMAN, writing()));
    assert!(game.players[HUMAN].gold < before[0]);
    assert!(game.players[1].gold > before[1]);
    assert_eq!(
        game.players[HUMAN].gold + game.players[1].gold,
        before[0] + before[1]
    );
    let after = game.players[HUMAN].gold;
    game.advance_proactive_trades_for(HUMAN);
    assert_eq!(game.players[HUMAN].gold, after);
}

#[test]
fn treasury_reserve_and_nearly_finished_research_prevent_bad_purchases() {
    let mut game = fixture();
    game.players[HUMAN].gold = game.diplomatic_trade_reserve(HUMAN);
    assert!(game.choose_technology_trade(HUMAN, 1).selected.is_none());
    game.players[HUMAN].gold = 5000;
    game.players[HUMAN].researching = writing();
    game.players[HUMAN].research = game.target_cost(HUMAN, writing()) - 1;
    assert!(game.choose_technology_trade(HUMAN, 1).selected.is_none());
}

#[test]
fn scientific_traits_and_active_research_goals_change_willingness_to_pay() {
    let mut game = fixture();
    game.players[HUMAN].civilization_id = Some("rome".into());
    let ordinary = game
        .diplomatic_technology_value(HUMAN, 1, writing(), false)
        .unwrap();
    game.players[HUMAN].civilization_id = Some("korea".into());
    let scientific = game
        .diplomatic_technology_value(HUMAN, 1, writing(), false)
        .unwrap();
    assert!(scientific > ordinary);
    game.players[HUMAN].research_goal = writing();
    assert!(
        game.diplomatic_technology_value(HUMAN, 1, writing(), false)
            .unwrap()
            > scientific
    );
}

#[test]
fn military_exports_and_committed_hostility_are_rejected() {
    let mut game = fixture();
    game.players[HUMAN].known.push(Tech::BRONZE_WORKING);
    game.players[1]
        .known
        .extend([Tech::BRONZE_WORKING, Tech::IRON_WORKING]);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
    assert_eq!(
        game.diplomatic_technology_value(1, HUMAN, Tech::IRON_WORKING, true),
        Err("military_security")
    );
    game.begin_ai_war_plan(1, HUMAN, 3, WarReason::Hatred);
    assert!(game.choose_technology_trade(HUMAN, 1).selected.is_none());
}

#[test]
fn pending_human_offers_and_player_treasury_are_not_overwritten() {
    let mut game = fixture();
    game.players[HUMAN].ai = false;
    let offer = TreatyOffer {
        pact: Some(Pact::Alliance),
        ..Default::default()
    };
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer.clone());
    let gold = game.players[HUMAN].gold;
    game.advance_diplomatic_offers_for(1);
    assert_eq!(game.treaty_offer(1), Some(offer));
    assert_eq!(game.players[HUMAN].gold, gold);
}

#[test]
fn selection_is_deterministic_read_only_and_respects_acceptance() {
    let game = fixture();
    let random = game.random_snapshot();
    let first = game.choose_technology_trade(HUMAN, 1);
    let second = game.choose_technology_trade(HUMAN, 1);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert_eq!(game.random_snapshot(), random);
    let choice = first.selected.unwrap();
    assert!(game.ai_accepts_treaty_exchange(HUMAN, 1, &choice.offer.terms, Pact::Peace));
}

#[test]
fn incoming_technology_offers_cannot_drain_ai_reserves_or_partially_execute() {
    let mut game = fixture();
    let offer = game
        .choose_technology_trade(HUMAN, 1)
        .selected
        .unwrap()
        .offer;
    assert!(offer.terms[0].gold > 0);
    game.players[HUMAN].gold = game.diplomatic_trade_reserve(HUMAN) + offer.terms[0].gold - 1;
    let known = game.players[HUMAN].known.clone();
    let gold = [game.players[HUMAN].gold, game.players[1].gold];
    assert!(!game.execute_offer(HUMAN, 1, &offer));
    assert_eq!(game.players[HUMAN].known, known);
    assert_eq!([game.players[HUMAN].gold, game.players[1].gold], gold);
}

#[test]
fn proactive_human_trade_waits_for_explicit_acceptance() {
    let mut game = fixture();
    game.players[HUMAN].ai = false;
    let gold = game.players[HUMAN].gold;
    game.advance_proactive_trades_for(1);
    assert!(!game.has_tech(HUMAN, writing()));
    assert_eq!(game.players[HUMAN].gold, gold);
    let offer = game.treaty_offer(1).expect("a pending technology offer");
    assert!(game.accept_offer(1, offer));
    assert!(game.has_tech(HUMAN, writing()));
    assert!(game.players[HUMAN].gold < gold);
}

#[test]
fn human_private_research_does_not_change_ai_trade_proposals() {
    let mut game = fixture();
    game.players[HUMAN].ai = false;
    let before = serde_json::to_value(game.choose_technology_trade(1, HUMAN)).unwrap();
    game.players[HUMAN].research_goal = writing();
    game.players[HUMAN].researching = writing();
    game.players[HUMAN].research = game.target_cost(HUMAN, writing()) - 1;
    game.players[HUMAN].last_science = 0;
    let after = serde_json::to_value(game.choose_technology_trade(1, HUMAN)).unwrap();
    assert_eq!(before, after);
}
