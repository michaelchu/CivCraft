use super::*;

fn peaceful_fixture() -> Simulation {
    let mut game = war_planning_game();
    for player in &mut game.players {
        player.ai = false;
    }
    game
}

#[test]
fn audit_freeciv_armistice_preserves_civilian_units() {
    let mut game = peaceful_fixture();
    game.tile_mut(12, 23).terrain = Terrain::Grassland;
    let worker = game.spawn_unit(HUMAN, UnitKind::Worker, 12, 23);
    let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, 12, 23);
    assert!(game.unit_by_id(worker).unwrap().kind.is_civilian());
    game.sign_pact(HUMAN, 1, Pact::Peace);
    game.tile_mut(12, 23).owner = Some(1);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().until = game.turn;
    game.advance_diplomacy();
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert!(game.unit_by_id(warrior).is_none());
    assert!(
        game.unit_by_id(worker).is_some(),
        "Freeciv srv_main.c exempts NonMil/Civilian units"
    );
}

#[test]
fn audit_freeciv_seamap_uses_recipients_city_count() {
    let mut game = peaceful_fixture();
    game.create_city(HUMAN, 4, 20);
    game.create_city(HUMAN, 4, 24);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 10000;
    let give = Concessions::default().set_map(Some(TreatyMap::Sea));
    let actual = game
        .ai_treaty_clause_balance(1, HUMAN, &give, &Concessions::default(), Pact::Peace)
        .unwrap()
        .0;
    let recipient_cities = game.cities.iter().filter(|c| c.owner == HUMAN).count() as i64;
    assert_eq!(
        actual,
        -15 * recipient_cities,
        "Freeciv CLAUSE_SEAMAP prices the recipient's cities"
    );
}

#[test]
fn audit_freeciv_worldmap_cost_doubles_during_ceasefire() {
    let mut game = peaceful_fixture();
    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 10000;
    let give = Concessions::default().set_map(Some(TreatyMap::World));
    let actual = game
        .ai_treaty_clause_balance(1, HUMAN, &give, &Concessions::default(), Pact::Ceasefire)
        .unwrap()
        .0;
    let giver_cities = game.cities.iter().filter(|c| c.owner == 1).count().max(1) as i64;
    assert_eq!(
        actual,
        -80 * giver_cities,
        "Freeciv pplayers_in_peace excludes ceasefire"
    );
}

#[test]
fn audit_freeciv_effect_embassy_requires_prior_contact() {
    let mut game = peaceful_fixture();
    game.diplomacy.remove(&(HUMAN, 2));
    game.cities
        .iter_mut()
        .find(|c| c.owner == HUMAN)
        .unwrap()
        .buildings
        .push(Building::MARCO_POLOS_EMBASSY);
    assert!(
        !game.has_embassy(HUMAN, 2),
        "Freeciv player_has_embassy_from_effect excludes DS_NO_CONTACT until Have_Contacts executes"
    );
}

#[test]
fn audit_freeciv_city_offer_can_reveal_an_unknown_owned_city() {
    let mut game = peaceful_fixture();
    game.create_city(HUMAN, 4, 20);
    let city = game
        .cities
        .iter()
        .find(|c| c.owner == HUMAN && c.x == 4)
        .unwrap()
        .clone();
    assert!(!city.has_building(Building::PALACE));
    game.tile_mut(city.x, city.y).known_by.set(1, false);
    assert!(
        game.treaty_trade_cities(HUMAN, 1).contains(&city.id),
        "Freeciv permits offering an unknown owned city and shares its city map"
    );
}

#[test]
fn audit_freeciv_visual_recognition_does_not_override_first_contact_defaults() {
    let mut game = peaceful_fixture();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.sign_pact(1, 2, Pact::Alliance);
    game.diplomacy.remove(&(HUMAN, 2));
    let city = game.cities.iter().find(|c| c.owner == 2).unwrap().clone();
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().vision[1] = true;
    game.spawn_unit(1, UnitKind::Warrior, city.x, city.y);
    game.refresh_visibility();
    assert!(game.tile(city.x, city.y).visible);
    assert!(game.contacted_pact(HUMAN, 2).is_none());
    game.make_diplomatic_contact(HUMAN, 2);
    assert_eq!(
        game.pact(HUMAN, 2),
        Pact::Peace,
        "Freeciv get_default_diplstate gives Peace with a common ally"
    );
}

#[test]
fn audit_matching_first_contact_without_visual_preemption() {
    let mut game = peaceful_fixture();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.sign_pact(1, 2, Pact::Alliance);
    game.diplomacy.remove(&(HUMAN, 2));
    game.make_diplomatic_contact(HUMAN, 2);
    assert_eq!(game.pact(HUMAN, 2), Pact::Peace);
}

#[test]
fn audit_matching_ceasefire_expires_to_war() {
    let mut game = peaceful_fixture();
    game.sign_pact(HUMAN, 1, Pact::Ceasefire);
    assert_eq!(game.pact_turns(HUMAN, 1), 16);
    game.turn += 16;
    game.advance_diplomacy();
    assert_eq!(game.pact(HUMAN, 1), Pact::War);
}

#[test]
fn audit_matching_alliance_cancels_to_armistice() {
    let mut game = peaceful_fixture();
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    assert!(game.cancel_pact(HUMAN, 1));
    assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
    assert_eq!(game.pact_turns(HUMAN, 1), 16);
}

#[test]
fn audit_matching_shared_vision_is_directional_and_transitive() {
    let mut game = peaceful_fixture();
    game.diplomacy.entry((HUMAN, 1)).or_default().vision[0] = true;
    game.diplomacy.entry((1, 2)).or_default().vision[0] = true;
    assert_eq!(game.vision_recipients(HUMAN), [HUMAN, 1, 2]);
    assert_eq!(game.vision_recipients(2), [2]);
}

#[test]
fn technology_clauses_append_remove_and_execute_atomically() {
    let mut game = peaceful_fixture();
    game.players[HUMAN].known.clear();
    game.players[1].known = vec![Tech::ALPHABET, Tech::POTTERY];
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    let terms = Concessions::default()
        .add_technology(Tech::ALPHABET)
        .add_technology(Tech::POTTERY)
        .add_technology(Tech::ALPHABET);
    assert_eq!(
        terms.technologies().collect::<Vec<_>>(),
        [Tech::ALPHABET, Tech::POTTERY]
    );
    assert_eq!(
        terms
            .remove_technology(Tech::ALPHABET)
            .technologies()
            .collect::<Vec<_>>(),
        [Tech::POTTERY]
    );
    assert_eq!(
        terms
            .remove_technology(Tech::POTTERY)
            .technologies()
            .collect::<Vec<_>>(),
        [Tech::ALPHABET]
    );
    let mut offer = TreatyOffer::default();
    offer.terms[1] = terms.clone().add_technology(Tech::MONARCHY);
    let before = game.players[HUMAN].known.clone();
    assert!(!game.execute_offer(HUMAN, 1, &offer));
    assert_eq!(game.players[HUMAN].known, before);
    offer.terms[1] = terms;
    assert!(game.execute_offer(HUMAN, 1, &offer));
    assert!(game.has_tech(HUMAN, Tech::ALPHABET));
    assert!(game.has_tech(HUMAN, Tech::POTTERY));
    let json = serde_json::to_value(&offer).unwrap();
    assert_eq!(serde_json::from_value::<TreatyOffer>(json).unwrap(), offer);
}

#[test]
fn treaty_expiry_and_ai_diplomacy_run_at_separate_boundaries() {
    let mut game = war_planning_game();
    game.ai_war_plans.insert(
        (1, HUMAN),
        WarPlan {
            countdown: 0,
            reason: WarReason::Hatred,
        },
    );
    game.advance_diplomacy();
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
    game.trace = trace::Collector::enabled(["*".to_owned()].into(), 10000);
    let ending_turn = game.turn;
    game.simulate_turn();
    let events = game.trace.drain();
    let declaration = events
        .iter()
        .find(|event| event.payload["action"] == "war_declared")
        .unwrap();
    assert_eq!(declaration.turn, ending_turn + 1);
    assert_eq!(declaration.phase, trace::Phase::AiPlanning);
}

#[test]
fn each_ai_negotiates_before_the_next_ai_declares_war() {
    let mut game = peaceful_fixture();
    game.players[2].ai = true;
    game.players[HUMAN].ai = true;
    game.players[1].ai = true;
    game.sign_pact(HUMAN, 1, Pact::Alliance);
    game.sign_pact(HUMAN, 2, Pact::Peace);
    game.sign_pact(1, 2, Pact::Peace);
    for pair in [(HUMAN, 1), (HUMAN, 2), (1, 2)] {
        let relation = game.diplomacy.get_mut(&pair).unwrap();
        relation.first_contact = Some(game.turn);
        relation.pact = if pair == (HUMAN, 1) {
            Pact::Alliance
        } else {
            Pact::Peace
        };
        relation.until = 0;
        relation.trust = [3000; 2];
    }
    game.ai_diplomatic_memory
        .entry((HUMAN, 2))
        .or_default()
        .spam = 0;
    game.make_diplomatic_contact(HUMAN, 2);
    game.make_diplomatic_contact(1, 2);
    game.ai_war_plans.insert(
        (1, 2),
        WarPlan {
            countdown: 0,
            reason: WarReason::Hatred,
        },
    );
    game.refresh_diplomatic_intelligence();
    game.trace = trace::Collector::enabled(["*".to_owned()].into(), 10000);
    game.run_begin_phase_diplomacy();
    let events = game.trace.drain();
    let agreement = events
        .iter()
        .find(|event| {
            event.payload["action"] == "offer_accepted"
                && event.payload["actor"] == HUMAN
                && event.payload["other"] == 2
        })
        .unwrap();
    let war = events
        .iter()
        .find(|event| {
            event.payload["action"] == "war_declared"
                && event.payload["actor"] == 1
                && event.payload["other"] == 2
        })
        .unwrap();
    assert!(agreement.sequence < war.sequence);
    assert_eq!(game.pact(HUMAN, 2), Pact::Alliance);
    assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
}
