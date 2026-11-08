//! Regression tests for the behavior of the parent module.

use super::*;

#[test]
fn advisor_danger_distinguishes_love_pacts_excuses_and_allocated_enemy_allies() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.river = false;
        tile.road = false;
        tile.railroad = false;
    }
    game.create_city(1, 10, 24);
    let nearby = offset_native_by_map(10, 24, 1, 0);
    let threat = game.spawn_unit(HUMAN, UnitKind::Warrior, nearby.0, nearby.1);
    let assessed = |sim: &Simulation| {
        sim.city_threat_arrivals(&sim.cities[0])
            .any(|(unit, _, _)| unit.id == threat)
    };
    game.sign_pact(1, HUMAN, Pact::Peace);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 999;
    assert!(game.advisor_player_dangerous(1, HUMAN));
    assert!(assessed(&game));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
    assert!(!game.advisor_player_dangerous(1, HUMAN));
    assert!(!assessed(&game));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Ceasefire;
    assert!(game.advisor_player_dangerous(1, HUMAN));
    assert!(assessed(&game));
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::Peace;
    relation.cause_until[1] = game.turn + 1;
    assert!(game.advisor_player_dangerous(1, HUMAN));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().cause_until[1] = game.turn;
    assert!(!game.advisor_player_dangerous(1, HUMAN));
    game.ai_war_plans.insert(
        (1, HUMAN),
        WarPlan {
            countdown: 0,
            reason: WarReason::Spoils,
        },
    );
    assert!(game.advisor_player_dangerous(1, HUMAN));
    assert!(assessed(&game));
    game.ai_war_plans.clear();
    game.spawn_unit(2, UnitKind::Warrior, 24, 40);
    game.make_diplomatic_contact(1, 2);
    game.diplomacy.get_mut(&(1, 2)).unwrap().pact = Pact::War;
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.units.retain(|unit| unit.owner != 2);
    game.cities.retain(|city| city.owner != 2);
    assert!(!game.player_alive(2));
    assert!(game.advisor_player_dangerous(1, HUMAN));
    assert!(!game.advisor_player_dangerous(1, 1));
}

#[test]
fn defense_building_baseline_respects_native_peace_love_and_ceasefire_danger() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.cities.clear();
    for tile in &mut game.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.river = false;
        tile.base = None;
    }
    game.create_city(1, 10, 24);
    game.cities[0].buildings.clear();
    game.create_city(HUMAN, 22, 40);
    game.sign_pact(1, HUMAN, Pact::Peace);
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::Peace;
    relation.trust[1] = 999;
    assert_eq!(
        game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
        83.0
    );
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
    assert_eq!(
        game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
        3.0
    );
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Ceasefire;
    assert_eq!(
        game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
        83.0
    );
}

#[test]
fn embassy_access_extends_the_legacy_wire_without_aliasing_city_ids() {
    let legacy_access = (u64::from(u32::MAX) << 3) | 3;
    let legacy = Concessions {
        gold: 77,
        technology: Some(Tech::POTTERY),
        access: legacy_access,
    };
    let bytes = bincode::serialize(&(77i32, Some(Tech::POTTERY), legacy_access)).unwrap();
    assert_eq!(bincode::serialize(&legacy).unwrap(), bytes);
    let decoded: Concessions = bincode::deserialize(&bytes).unwrap();
    assert_eq!(decoded.city(), Some(u32::MAX));
    assert!(!decoded.embassy());
    let extended = decoded.with_embassy();
    assert!(extended.valid_access());
    assert_eq!(extended.city(), Some(u32::MAX));
    assert!(extended.shared_vision() && extended.embassy());
    assert_eq!(extended.map(), Some(TreatyMap::World));
    let cleared = extended.with_city(0);
    assert_eq!(cleared.city(), None);
    assert!(cleared.embassy() && cleared.shared_vision());
    for access in [6, 1u64 << 36, u64::MAX] {
        assert!(
            !Concessions {
                access,
                ..Concessions::default()
            }
            .valid_access()
        );
    }
}

#[test]
fn embassy_clause_gold_values_follow_pact_love_and_embassy_leakage_rules() {
    let game = war_planning_game();
    let embassy = Concessions::default().with_embassy();
    let empty = Concessions::default();
    assert_eq!(
        game.ai_treaty_clause_balance(1, HUMAN, embassy, empty, Pact::Alliance),
        Some((0, true))
    );
    assert_eq!(
        game.ai_treaty_clause_balance(1, HUMAN, embassy, empty, Pact::Peace),
        Some((-150, false))
    );
    assert_eq!(
        game.ai_treaty_clause_balance(1, HUMAN, embassy, empty, Pact::Ceasefire),
        Some((-2000, false))
    );
    assert_eq!(
        game.ai_treaty_clause_balance(1, HUMAN, empty, embassy, Pact::Peace),
        Some((10, true))
    );
    assert!(
        game.ai_treaty_clause_balance(1, HUMAN, empty, embassy, Pact::War)
            .is_none()
    );
}

#[test]
fn embassy_clauses_are_directional_permanent_and_reject_stale_duplicates() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    let mut offer = TreatyOffer::default();
    offer.terms[1] = Concessions::default().with_embassy();
    assert!(game.execute_offer(HUMAN, 1, offer));
    assert!(game.has_real_embassy(HUMAN, 1));
    assert!(!game.has_real_embassy(1, HUMAN));
    game.diplomatic_contact_until.clear();
    assert!(game.diplomatic_meeting_possible(HUMAN, 1));
    assert!(!game.execute_offer(HUMAN, 1, offer));
    offer.terms[1] = Concessions::default();
    offer.terms[0] = Concessions::default().with_embassy();
    assert!(game.execute_offer(HUMAN, 1, offer));
    assert!(game.has_real_embassy(1, HUMAN));
}

#[test]
fn an_existing_offer_can_be_accepted_after_physical_contact_expires() {
    let mut game = war_planning_game();
    let mut offer = TreatyOffer::default();
    offer.terms[0].gold = 11;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
    game.diplomatic_contact_until.clear();
    assert!(!game.can_negotiate(1));
    assert!(game.accept_offer(1, offer));
    assert_eq!(game.players[HUMAN].gold, 39);
}

#[test]
fn historical_alliance_increases_betrayal_loss_for_every_ai() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    assert!(game.cancel_pact(HUMAN, 1));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
    assert!(game.cancel_pact(HUMAN, 1));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -4660);
    assert_eq!(game.diplomacy[&(HUMAN, 2)].trust[1], -1330);
}

#[test]
fn friendly_ui_fixture_has_a_real_alternate_target_for_peace_then_alliance() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    game.diplomacy_playtest_friendship();
    game.diplomacy_playtest_tick(12);
    assert_eq!(game.pact_offer(1), Some(Pact::Peace));
    assert!(game.negotiate(1, Pact::Peace));
    game.diplomacy_playtest_tick(16);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert_eq!(game.pact_offer(1), None);
    game.diplomacy_playtest_unit(1).unwrap();
    game.diplomacy_playtest_tick(1);
    assert_eq!(game.pact_offer(1), Some(Pact::Alliance));
}

#[test]
fn native_pair_spam_delays_first_ai_offer_without_delaying_human_offers() {
    let mut game = war_planning_game();
    game.sign_pact(1, 2, Pact::Ceasefire);
    game.make_diplomatic_contact(1, 2);
    let relation = game.diplomacy.get_mut(&(1, 2)).unwrap();
    relation.until = game.turn + 4;
    relation.trust = [2000; 2];
    assert_eq!(game.ai_diplomatic_memory[&(1, 2)].spam, 3);
    for left in [2, 1] {
        game.advance_diplomatic_offers();
        assert_eq!(game.ai_diplomatic_memory[&(1, 2)].spam, left);
        assert_eq!(game.pact(1, 2), Pact::Ceasefire);
    }
    game.advance_diplomatic_offers();
    assert_eq!(game.pact(1, 2), Pact::Armistice);
}

#[test]
fn cached_diplomatic_intelligence_changes_real_negotiation_commands() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_diplomatic_intelligence_playtest());
}

#[test]
fn new_phase_love_uses_previous_intelligence_before_refresh() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1100);
    game.refresh_diplomatic_intelligence();
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1050);
    game.sign_pact(HUMAN, 2, Pact::Peace);
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1000);
    game.refresh_diplomatic_intelligence();
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1100);
}

#[test]
fn cached_enemy_alliance_holds_a_refusal_until_refresh_and_reloads_live() {
    let mut game = war_planning_game();
    game.tile_mut(24, 40).terrain = Terrain::Grassland;
    game.create_city(3, 24, 40);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 4000;
    for third in [2, 3] {
        game.sign_pact(HUMAN, third, Pact::Alliance);
        game.diplomacy.entry((1, third)).or_default().first_contact = Some(1);
    }
    game.refresh_diplomatic_intelligence();
    // The source retains the last matching example, not the first.
    assert_eq!(game.diplomatic_intel(1, HUMAN).allied_with_enemy, Some(3));
    for third in [2, 3] {
        game.sign_pact(HUMAN, third, Pact::Peace);
    }
    assert_eq!(
        game.pact_refusal(1, HUMAN, Pact::Alliance),
        Some("Keep your distance from our enemy for a while first.")
    );
    let rng = game.random_snapshot();
    let trust = game.diplomacy[&(HUMAN, 1)].trust;
    let path = std::env::temp_dir().join(format!("civcraft-intel-{}.save", std::process::id()));
    game.save_campaign(&path).unwrap();
    let resumed = Simulation::load_campaign(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(resumed.random_snapshot(), rng);
    assert_eq!(resumed.diplomacy[&(HUMAN, 1)].trust, trust);
    assert_eq!(resumed.diplomatic_intel(1, HUMAN).allied_with_enemy, None);
    assert!(resumed.pact_refusal(1, HUMAN, Pact::Alliance).is_none());
    // Cached pointers are intentionally transient, as native save_relations
    // does not serialize them and phase_begin(FALSE) recomputes on load.
}

#[test]
fn shared_ally_greed_discount_is_a_phase_snapshot() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -2250);
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.sign_pact(1, 2, Pact::Alliance);
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -2250);
    game.refresh_diplomatic_intelligence();
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -1125);
    assert!(game.cancel_pact(1, 2));
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -1125);
    game.refresh_diplomatic_intelligence();
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -2250);
}

#[test]
fn explicit_map_and_city_deal_fixtures_establish_finite_contact() {
    let fresh = || Simulation::new(crate::mapgen::generate(42));
    assert!(fresh().treaty_map_playtest());
    assert!(fresh().city_trade_playtest());
    assert!(fresh().city_buyback_playtest());
}

#[test]
fn allied_exchange_ui_fixture_consumes_a_retained_snapshot_and_accepts_the_real_offer() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.players[1].gold = 300;
    game.diplomacy_playtest_technologies(true);
    game.diplomacy_playtest_allied_sharing();
    let sharing = game
        .treaty_offer(1)
        .expect("actual established-alliance sharing offer");
    assert_eq!(sharing.terms[0].technology, Some(Tech::POTTERY));
    assert_eq!(sharing.terms[1].technology.unwrap().name(), "Writing");
    assert!(game.accept_offer(1, sharing));
    game.diplomacy_playtest_allied_exchange();
    let currency = Tech::all().find(|tech| tech.name() == "Currency").unwrap();
    let offer = game
        .treaty_offer(1)
        .expect("actual subsequent allied trade offer");
    let baseline = game.diplomacy_playtest_allied_trade_baseline().unwrap();
    assert_eq!(offer.pact, None);
    assert_eq!(offer.terms[0].technology, Some(currency));
    assert_eq!(offer.terms[1].technology, Some(Tech::WARRIOR_CODE));
    assert_eq!(offer.terms[1].gold, baseline + 1);
    let retained = game.ai_tech_wants.clone();
    let quoted = game.tech_sell_price(HUMAN, 1, currency).unwrap();
    let mut cold = game.clone();
    cold.ai_tech_wants.clear();
    assert_eq!(
        cold.tech_sell_price(HUMAN, 1, currency).unwrap() + 1,
        quoted
    );
    // Live economic conditions change, but no last-activities refresh
    // occurred. Quote evaluation must consume the same retained vector.
    game.players[1].rates = [0, 100, 0];
    for city in game.cities.iter_mut().filter(|city| city.owner == 1) {
        city.size = 6;
        city.specialists = [6, 0, 0];
    }
    assert_eq!(game.tech_sell_price(HUMAN, 1, currency), Some(quoted));
    assert_eq!(game.ai_tech_wants, retained);
    let human_gold = game.players[HUMAN].gold;
    assert!(game.accept_offer(1, offer));
    assert_eq!(game.players[HUMAN].gold, human_gold + baseline + 1);
    assert!(game.has_tech(HUMAN, Tech::WARRIOR_CODE));
    assert!(game.has_tech(1, currency));
    assert_eq!(game.ai_tech_wants, retained);
}

#[test]
fn allied_trade_baseline_recomputes_embassy_leakage_before_advisor_premium() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    let currency = Tech::all().find(|tech| tech.name() == "Currency").unwrap();
    game.players[HUMAN].known = vec![Tech::ALPHABET, Tech::BRONZE_WORKING, currency];
    game.players[1].known = vec![Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::WARRIOR_CODE];
    let before = game.diplomacy_playtest_allied_trade_baseline().unwrap();
    let embassy = TreatyOffer {
        terms: [Concessions::default().with_embassy(); 2],
        ..Default::default()
    };
    assert!(game.execute_offer(HUMAN, 1, embassy));
    let after = game.diplomacy_playtest_allied_trade_baseline().unwrap();
    assert!(after < before);
    let expected = (3 * game.target_cost(1, currency) / 2) / 2
        - (3 * game.target_cost(HUMAN, Tech::WARRIOR_CODE) / 2) / 2;
    assert_eq!(after, expected);
}

#[test]
fn established_sharing_fixture_keeps_inventory_for_the_real_exchange() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.diplomacy_playtest_technologies(true);
    let before = [
        game.players[HUMAN].known.clone(),
        game.players[1].known.clone(),
    ];
    game.diplomacy_playtest_allied_sharing();
    assert_eq!(game.players[HUMAN].known, before[0]);
    assert_eq!(game.players[1].known, before[1]);
    let offer = game.treaty_offer(1).unwrap();
    assert_eq!(offer.pact, None);
    assert_eq!(offer.terms[0].technology, Some(Tech::POTTERY));
    assert_eq!(offer.terms[1].technology.unwrap().name(), "Writing");
    assert!(offer.terms[0].gold > 0);
    assert!(offer.terms.iter().all(|terms| terms.shared_vision()));
}

#[test]
fn automatic_alliance_origination_is_a_pact_then_trusted_allied_sharing() {
    let mut game = war_planning_game();
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.first_contact = Some(game.turn);
    relation.trust[1] = 3000;
    game.advance_diplomatic_offers();
    let offer = game.treaty_offer(1).unwrap();
    assert_eq!(offer.pact, Some(Pact::Alliance));
    assert_eq!(offer.terms, [Concessions::default(); 2]);
    assert!(game.execute_offer(HUMAN, 1, offer));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
    game.advance_diplomatic_offers();
    assert!(game.treaty_offer(1).is_none());
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5001;
    game.advance_diplomatic_offers();
    let sharing = game.treaty_offer(1).unwrap();
    assert_eq!(sharing.pact, None);
    assert!(sharing.terms[1].shared_vision());
    assert!(
        sharing
            .terms
            .iter()
            .all(|terms| terms.map().is_none() && terms.city().is_none())
    );
}

#[test]
fn unsafe_vision_is_revoked_even_when_target_spam_processing_is_skipped() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().vision[1] = true;
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.diplomacy.get_mut(&(HUMAN, 2)).unwrap().vision[0] = true;
    game.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
    assert!(!game.vision_safe(1, HUMAN));
    game.advance_diplomatic_offers();
    assert!(!game.diplomacy[&(HUMAN, 1)].vision[1]);
}

#[test]
fn positive_most_hated_target_still_suppresses_source_offer_cadence() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
    let spam = game.ai_diplomatic_memory[&(1, HUMAN)].spam;
    game.advance_diplomatic_offers();
    assert_eq!(game.pact_offer(1), None);
    assert_eq!(game.ai_diplomatic_memory[&(1, HUMAN)].spam, spam);
}

#[test]
fn phase_love_and_gift_response_match_reference_integer_order() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_diplomatic_love_playtest());
}

#[test]
fn visible_trespass_penalty_is_capped_in_native_love_units() {
    let mut game = war_planning_game();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
    game.tile_mut(12, 24).owner = Some(1);
    for _ in 0..9 {
        game.spawn_unit(HUMAN, UnitKind::Warrior, 12, 24);
    }
    game.advance_ai_love();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -140);
}

#[test]
fn gold_can_compensate_for_missing_love_but_not_a_hard_alliance_refusal() {
    let mut game = war_planning_game();
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::Ceasefire;
    relation.until = game.turn + 4;
    relation.trust[1] = 1000;
    game.players[HUMAN].gold = 1000;
    let mut offer = TreatyOffer {
        pact: Some(Pact::Peace),
        ..TreatyOffer::default()
    };
    assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Peace), -62);
    assert!(!game.execute_offer(HUMAN, 1, offer));
    offer.terms[0].gold = 62;
    assert!(game.execute_offer(HUMAN, 1, offer));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 990;
    offer.pact = Some(Pact::Alliance);
    offer.terms[0].gold = 900;
    assert!(!game.execute_offer(HUMAN, 1, offer));
}

#[test]
fn human_counteroffer_pays_gold_to_sign_peace_through_command() {
    let mut game = war_planning_game();
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::Ceasefire;
    relation.until = game.turn + 4;
    relation.trust[1] = 1000;
    game.players[HUMAN].gold = 250;
    let mut offer = TreatyOffer {
        pact: Some(Pact::Peace),
        ..TreatyOffer::default()
    };
    assert!(!game.apply(Command::ProposeTreaty(1, offer)));
    assert!(
        game.events()
            .last()
            .unwrap()
            .contains("requires at least 62"),
        "event: {:?}",
        game.events().last()
    );
    assert_eq!(game.pact(HUMAN, 1), Pact::Ceasefire);
    assert_eq!(game.players[HUMAN].gold, 250);

    offer.terms[HUMAN].gold = 75;

    assert!(game.apply(Command::ProposeTreaty(1, offer)));
    assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
    assert_eq!(game.players[HUMAN].gold, 175);
}

#[test]
fn profitable_war_target_loses_love_while_expansion_suppresses_war_desire() {
    let mut game = war_planning_game();
    game.tile_mut(14, 26).terrain = Terrain::Grassland;
    game.create_city(1, 14, 26);
    for city in &mut game.cities {
        city.production = Production::Unit(UnitKind::Warrior);
    }
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
    let desire = game.ai_war_desire(1, HUMAN);
    assert!(desire > 0);
    game.advance_ai_love();
    assert_eq!(
        game.diplomacy[&(HUMAN, 1)].trust[1],
        ((-f64::from(desire.min(50)) * 0.96) as i32) * 10
    );
    game.spawn_unit(1, UnitKind::Settlers, 12, 24);
    game.spawn_unit(1, UnitKind::Settlers, 14, 26);
    assert!(game.ai_war_desire(1, HUMAN) < desire);
}

#[test]
fn an_ai_city_concession_must_leave_more_than_two_cities() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    for (x, y) in [(14, 24), (16, 24)] {
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(1, x, y);
    }
    let city = game.cities.last().unwrap();
    let mut give = Concessions::default().with_city(city.id);
    let receive = Concessions {
        gold: game.city_sale_value(city, HUMAN),
        ..Default::default()
    };
    assert!(
        game.ai_treaty_clause_balance(1, HUMAN, give, receive, Pact::Alliance)
            .is_none()
    );
    game.tile_mut(20, 24).terrain = Terrain::Grassland;
    game.create_city(1, 20, 24);
    assert!(
        game.ai_treaty_clause_balance(1, HUMAN, give, receive, Pact::Alliance)
            .is_some()
    );
    give.gold = 0;
    assert!(
        game.ai_treaty_clause_balance(1, HUMAN, give, receive, Pact::War)
            .is_none()
    );
}

#[test]
fn balanced_trades_do_not_count_as_all_beneficial_gifts() {
    let mut game = war_planning_game();
    game.players[1].gold = 1000;
    game.players[HUMAN].gold = 1000;
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
    let mut terms = TreatyOffer::default();
    terms.terms[0].gold = 110;
    terms.terms[1].gold = 110;
    assert!(game.execute_offer(HUMAN, 1, terms));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1000);
}

#[test]
fn treaty_giver_policies_alliance_jump_and_senate_callback_match_source() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_treaty_policy_playtest());
}

#[test]
fn dangerous_technology_is_an_ai_giver_veto_not_a_receiving_veto() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.diplomacy.entry((1, 2)).or_default().pact = Pact::War;
    game.players[1].known = vec![Tech::POTTERY];
    game.players[HUMAN].known.clear();
    game.players[2].known.clear();
    let mut gift = TreatyOffer::default();
    gift.terms[1].technology = Some(Tech::POTTERY);
    assert!(
        game.tech_trade_quote(1, HUMAN, Tech::POTTERY, true)
            .unwrap()
            .1
    );
    assert!(!game.execute_offer(HUMAN, 1, gift));
    assert!(!game.has_tech(HUMAN, Tech::POTTERY));
}

#[test]
fn only_ai_givers_reject_unsafe_vision_and_alliance_cancellation_preserves_grants() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.diplomacy.get_mut(&(HUMAN, 2)).unwrap().vision[0] = true;
    game.diplomacy.entry((1, 2)).or_default().pact = Pact::Peace;
    let mut gift = TreatyOffer::default();
    gift.terms[1].access = 1;
    assert!(!game.vision_safe(1, HUMAN));
    assert!(!game.execute_offer(HUMAN, 1, gift));
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().vision = [true; 2];
    assert!(game.cancel_pact(HUMAN, 1));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].vision, [true; 2]);
    game.advance_diplomatic_offers();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].vision, [true, false]);
}

#[test]
fn directional_embassy_consumes_explorer_and_reduces_research_cost() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    let (x, y) = (10, 20);
    game.create_city(1, x, y);
    let city = game.cities[0].id;
    let (nx, ny) = offset_native_by_map(x, y, 1, 0);
    let explorer = game.spawn_unit(HUMAN, UnitKind::Explorer, nx, ny);
    let diplomat = game.spawn_unit(HUMAN, UnitKind::Diplomat, nx, ny);
    game.players[1].known.push(Tech::POTTERY);
    assert_eq!(game.target_cost(HUMAN, Tech::POTTERY), Tech::POTTERY.cost());
    assert!(game.apply(Command::EstablishEmbassy(explorer, city)));
    assert!(game.has_embassy(HUMAN, 1));
    assert!(!game.has_embassy(1, HUMAN));
    assert!(!game.apply(Command::EstablishEmbassy(diplomat, city)));
    assert!(game.units.iter().all(|unit| unit.id != explorer));
    assert_eq!(
        game.target_cost(HUMAN, Tech::POTTERY),
        Tech::POTTERY.cost() / 2
    );
}

#[test]
fn technology_price_discounts_allies_before_adding_leakage() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.players[1].known = vec![Tech::POTTERY];
    game.players[HUMAN].known.clear();
    game.players[2].known.clear();
    assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(45));
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(22));
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    let other = game.diplomacy.entry((1, 2)).or_default();
    other.pact = Pact::Peace;
    other.trust[0] = 1000;
    assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(33));
    game.sign_pact(1, 2, Pact::Ceasefire);
    assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), None);
}

#[test]
fn low_science_can_make_a_balanced_offer_unacceptable_to_the_ai() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.diplomacy_playtest_technologies(true);
    game.sign_pact(HUMAN, 1, Pact::Peace);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
    let offer = game.build_offer(HUMAN, 1, None);
    assert!(offer.terms.iter().all(|term| term.technology.is_some()));
    assert!(game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
    game.players[1].last_science = 0;
    assert!(!game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
    game.players[1].research = -1;
    assert!(!game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
}

#[test]
fn stale_bundle_applies_no_gold_technology_pact_or_vision() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    game.create_city(1, 20, 20);
    game.diplomacy_playtest_technologies(true);
    let key = Simulation::relation_key(HUMAN, 1);
    let relation = game.diplomacy.entry(key).or_default();
    relation.pact = Pact::Peace;
    relation.trust = [3000; 2];
    let offer = game.build_offer(HUMAN, 1, Some(Pact::Alliance));
    assert!(offer.terms[0].gold > 0 && offer.terms[1].technology.is_some());
    game.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
    game.players[HUMAN].gold = offer.terms[0].gold - 1;
    let balances = [game.players[HUMAN].gold, game.players[1].gold];
    assert!(!game.apply(Command::AcceptOffer(1, offer)));
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert_eq!([game.players[HUMAN].gold, game.players[1].gold], balances);
    assert!(!game.has_tech(HUMAN, offer.terms[1].technology.unwrap()));
    assert_eq!(game.vision_recipients(1), vec![1]);
}

#[test]
fn an_armistice_disbands_units_that_fail_to_withdraw() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    game.create_city(1, 20, 20);
    let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
    game.sign_pact(HUMAN, 1, Pact::Peace);
    game.diplomacy_playtest_tick(15);
    assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
    assert!(game.units.iter().any(|unit| unit.id == soldier));
    game.diplomacy_playtest_tick(1);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert!(!game.units.iter().any(|unit| unit.id == soldier));
}

#[test]
fn breaking_an_alliance_removes_visitors_from_former_allied_cities() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    game.create_city(1, 20, 20);
    let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    assert!(game.apply(Command::CancelPact(1)));
    assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
    assert!(
        game.units
            .iter()
            .any(|unit| unit.id == soldier && (unit.x, unit.y) != (20, 20))
    );
}

#[test]
fn senate_enforces_treaties_unless_provoked_or_waived() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 10);
    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    game.players[HUMAN].government = Government::Federation;
    assert!(!game.apply(Command::CancelPact(1)));
    game.record_provocation(HUMAN, 1);
    assert!(game.apply(Command::CancelPact(1)));
    assert!(game.at_war(HUMAN, 1));

    let find = |name| {
        Building::all()
            .find(|building| building.data().name == name)
            .unwrap()
    };
    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    game.cities[0].buildings.push(find("Statue of Liberty"));
    assert!(game.apply(Command::CancelPact(1)));

    game.cities[0]
        .buildings
        .retain(|building| building.data().name != "Statue of Liberty");
    game.players[HUMAN].government = Government::Tribal;
    game.cities[0].buildings.push(find("United Nations"));
    game.completed_wonders.push(find("Manhattan Project"));
    let fission = Tech::all()
        .find(|tech| *tech == Tech::NUCLEAR_FISSION)
        .unwrap();
    game.players[HUMAN].known.push(fission);
    assert!(game.has_senate(HUMAN));
    game.apollo_completed = true;
    assert!(!game.has_senate(HUMAN));
}
