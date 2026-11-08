//! Regression tests for diplomacy behavior, including edge cases and interactions with the simulation.

use super::*;

#[test]
fn physical_contact_and_treaty_embassy_work_through_commands() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.embassy_contact_playtest());
}

#[test]
fn finite_contact_expires_and_only_physical_events_refresh_it() {
    let mut game = war_planning_game();
    let first = game.diplomacy[&(HUMAN, 1)].first_contact;
    assert!(game.can_negotiate(1));
    game.diplomacy_playtest_tick(20);
    assert!(!game.can_negotiate(1));
    assert!(!game.negotiate(1, Pact::Alliance));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].first_contact, first);
    let city = game.cities.iter().find(|city| city.owner == 1).unwrap();
    let (x, y) = (city.x, city.y);
    game.spawn_unit(HUMAN, UnitKind::Diplomat, x, y);
    game.refresh_diplomatic_contact_at(HUMAN, x, y);
    assert!(game.can_negotiate(1));
    assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], game.turn + 20);
}

#[test]
fn legacy_contact_migration_gives_one_bounded_window_without_callbacks() {
    let mut game = war_planning_game();
    game.diplomacy.entry((HUMAN, 2)).or_default();
    game.diplomatic_contact_until.clear();
    let rng = game.random_snapshot();
    let trust = game.diplomacy[&(HUMAN, 1)].trust;
    game.seed_legacy_diplomatic_contacts();
    assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], game.turn + 20);
    assert!(!game.diplomatic_contact_until.contains_key(&(HUMAN, 2)));
    assert_eq!(game.random_snapshot(), rng);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust, trust);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].first_contact, Some(1));
    game.turn += 20;
    game.seed_legacy_diplomatic_contacts();
    assert!(!game.can_negotiate(1));
}

#[test]
fn ordinary_combat_and_city_conquest_do_not_fabricate_incidents() {
    let mut game = war_planning_game();
    let city = game.cities.iter_mut().find(|city| city.owner == 1).unwrap();
    city.size = 3;
    city.buildings
        .retain(|&building| building != Building::PALACE);
    let target = (city.x, city.y);
    let city_id = city.id;
    let origin = offset_native_by_map(target.0, target.1, -1, 0);
    let neighbor = offset_native_by_map(target.0, target.1, 1, 0);
    game.tile_mut(origin.0, origin.1).terrain = Terrain::Grassland;
    game.tile_mut(neighbor.0, neighbor.1).terrain = Terrain::Grassland;
    let defender = game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == defender)
        .unwrap()
        .hp = 1;
    game.spawn_unit(2, UnitKind::Warrior, neighbor.0, neighbor.1);
    let actor = game.spawn_unit(HUMAN, UnitKind::Legion, origin.0, origin.1);
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::War;
    relation.trust[1] = 0;
    relation.cause_until[1] = 0;
    game.diplomatic_contact_until.clear();
    assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
    game.set_random_seed(1);
    assert!(game.apply(Command::Move(actor, 1, 0)));
    assert_eq!(
        game.cities
            .iter()
            .find(|city| city.id == city_id)
            .unwrap()
            .owner,
        HUMAN
    );
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 0);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].cause_until[1], 0);
    // Actual occupation still makes contact with a neighboring third party.
    assert!(game.diplomatic_contact_until[&(HUMAN, 2)] > game.turn);
}

#[test]
fn a_lost_attack_does_not_refresh_contact_without_actual_movement() {
    let mut game = war_planning_game();
    let city = game.cities.iter().find(|city| city.owner == 1).unwrap();
    let target = (city.x, city.y);
    let origin = offset_native_by_map(target.0, target.1, -1, 0);
    game.tile_mut(origin.0, origin.1).terrain = Terrain::Grassland;
    game.spawn_unit(1, UnitKind::Phalanx, target.0, target.1);
    let actor = game.spawn_unit(HUMAN, UnitKind::Warrior, origin.0, origin.1);
    game.units
        .iter_mut()
        .find(|unit| unit.id == actor)
        .unwrap()
        .hp = 1;
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::War;
    relation.trust[1] = 0;
    relation.cause_until[1] = 0;
    game.diplomatic_contact_until.clear();
    game.set_random_seed(1);
    assert!(game.apply(Command::Move(actor, 1, 0)));
    assert!(game.units.iter().all(|unit| unit.id != actor));
    assert!(!game.diplomatic_contact_until.contains_key(&(HUMAN, 1)));
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 0);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].cause_until[1], 0);
}

#[test]
fn capturing_a_merchant_renews_ownership_contact_without_an_incident() {
    let mut game = war_planning_game();
    let (actor, merchant, _) = game.merchant_capture_playtest_setup();
    let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
    relation.pact = Pact::War;
    relation.trust[1] = 0;
    relation.cause_until[1] = 0;
    game.diplomatic_contact_until.clear();
    assert!(game.apply(Command::Move(actor, 1, 0)));
    assert!(game.units.iter().all(|unit| unit.id != merchant));
    assert!(
        game.units
            .iter()
            .any(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Freight)
    );
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 0);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].cause_until[1], 0);
    assert!(game.diplomatic_contact_until[&(HUMAN, 1)] > game.turn);
}

#[test]
fn intercepted_nuclear_action_has_no_incident_but_a_successful_strike_does() {
    for intercepted in [true, false] {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (missile, _, city_id) = game.nuclear_playtest_setup(intercepted);
        let city = game.city_by_id(city_id).unwrap();
        let target = (city.x, city.y);
        let relation = game.diplomacy.entry((HUMAN, 1)).or_default();
        relation.trust[1] = 0;
        relation.cause_until[1] = 0;
        let weapon = game
            .units
            .iter()
            .find(|unit| unit.id == missile)
            .unwrap()
            .clone();
        game.nuclear_strike(&weapon, target.0, target.1);
        let relation = &game.diplomacy[&(HUMAN, 1)];
        if intercepted {
            assert_eq!(relation.trust[1], 0);
            assert_eq!(relation.cause_until[1], 0);
            assert_eq!(
                game.cities
                    .iter()
                    .find(|city| city.id == city_id)
                    .unwrap()
                    .size,
                4
            );
        } else {
            assert_eq!(relation.trust[1], -3000);
            assert_eq!(relation.cause_until[1], game.turn + 2);
            assert_eq!(
                game.cities
                    .iter()
                    .find(|city| city.id == city_id)
                    .unwrap()
                    .size,
                2
            );
        }
        assert!(game.units.iter().all(|unit| unit.id != missile));
    }
}

#[test]
fn marco_polo_refreshes_global_contacts_before_turn_advance_and_leaves_grace() {
    let mut game = war_planning_game();
    let home = game
        .cities
        .iter()
        .position(|city| city.owner == HUMAN)
        .unwrap();
    game.cities[home]
        .buildings
        .push(Building::MARCO_POLOS_EMBASSY);
    game.turn = 50;
    assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
    game.refresh_diplomatic_contact_effects();
    assert_eq!(game.diplomacy[&(HUMAN, 1)].first_contact, Some(1));
    assert_eq!(game.diplomacy[&(HUMAN, 2)].first_contact, Some(50));
    assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], 70);
    assert_eq!(game.diplomatic_contact_until[&(HUMAN, 2)], 71);
    game.cities[home]
        .buildings
        .retain(|&building| building != Building::MARCO_POLOS_EMBASSY);
    game.turn = 69;
    assert!(game.can_negotiate(1));
    game.turn = 70;
    assert!(!game.can_negotiate(1));
    assert!(game.can_negotiate(2));
    game.turn = 71;
    assert!(!game.can_negotiate(2));
}

#[test]
fn virtual_embassies_allow_meetings_without_manufacturing_physical_contact() {
    let mut game = war_planning_game();
    game.cities
        .iter_mut()
        .find(|city| city.owner == HUMAN)
        .unwrap()
        .buildings
        .push(Building::MARCO_POLOS_EMBASSY);
    assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
    assert!(game.can_negotiate(2));
    assert!(game.negotiate(2, Pact::Ceasefire));
    assert_eq!(game.pact(HUMAN, 2), Pact::Ceasefire);
    assert!(!game.diplomatic_contact_until.contains_key(&(HUMAN, 2)));
    assert_eq!(game.diplomacy[&(HUMAN, 2)].first_contact, Some(0));
}

#[test]
fn first_contact_skips_the_first_decrement_and_common_allies_start_at_peace() {
    let mut game = war_planning_game();
    game.sign_pact(HUMAN, 2, Pact::Alliance);
    game.sign_pact(1, 2, Pact::Alliance);
    game.diplomacy.remove(&(HUMAN, 1));
    game.diplomatic_contact_until.remove(&(HUMAN, 1));
    game.make_diplomatic_contact(HUMAN, 1);
    assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], game.turn + 21);
}

#[test]
fn a_successful_bribe_applies_its_incident_after_validation() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_incident_action_playtest());
}

#[test]
fn observer_incidents_and_offer_cadence_work_together() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    assert!(game.ai_observer_incident_cadence_playtest());
}

#[test]
fn incident_categories_and_scope_use_native_integer_love() {
    let mut game = war_planning_game();
    for (incident, badness) in [
        (DiplomaticIncident::Snooping, 2),
        (DiplomaticIncident::Theft, 5),
        (DiplomaticIncident::UnitWeakening, 3),
        (DiplomaticIncident::UnitLoss, 5),
        (DiplomaticIncident::CityLoss, 10),
        (DiplomaticIncident::BuildingLoss, 5),
        (DiplomaticIncident::PopulationLoss, 5),
        (DiplomaticIncident::TerrainLoss, 4),
        (DiplomaticIncident::ExtraLoss, 2),
        (DiplomaticIncident::Misc, 1),
    ] {
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        game.record_diplomatic_incident(HUMAN, Some(1), incident, false);
        assert_eq!(
            game.diplomacy[&(HUMAN, 1)].trust[1],
            -(badness * 1000 / 35) * 10
        );
    }
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
    game.record_diplomatic_incident(HUMAN, Some(1), DiplomaticIncident::Theft, true);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -2850);
    assert_eq!(game.diplomacy[&(HUMAN, 2)].trust[1], -100);
    game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
    game.record_diplomatic_incident(HUMAN, Some(1), DiplomaticIncident::Nuclear, false);
    assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -3000);
}

#[test]
fn first_contact_randomizes_each_ai_once_without_changing_human_attitude() {
    let mut game = war_planning_game();
    game.diplomacy.remove(&(1, 2));
    game.make_diplomatic_contact(1, 2);
    let relation = &game.diplomacy[&(1, 2)];
    assert_eq!(relation.first_contact, Some(game.turn));
    assert!(
        relation
            .trust
            .iter()
            .all(|&trust| (-20..=20).contains(&trust) && trust % 10 == 0)
    );
    let before = relation.trust;
    let rng = game.random_snapshot();
    game.make_diplomatic_contact(2, 1);
    assert_eq!(game.diplomacy[&(1, 2)].trust, before);
    assert_eq!(game.random_snapshot(), rng);
}

#[test]
fn first_contact_notifies_the_human_once_and_diplomacy_exposes_war_status() {
    let mut game = war_planning_game();
    game.events.clear();
    game.diplomacy.remove(&(HUMAN, 2));

    game.make_diplomatic_contact(HUMAN, 2);

    assert_eq!(game.pact(HUMAN, 2), Pact::War);
    assert_eq!(game.events.len(), 1);
    assert!(game.events[0].contains("You have made contact with the"));
    assert!(game.events[0].contains(&game.players[2].name));
    assert!(game.events[0].contains(game.players[2].leader_name()));

    game.make_diplomatic_contact(2, HUMAN);
    assert_eq!(game.events.len(), 1);
}

#[test]
fn a_capital_attack_does_not_guarantee_free_peace_after_twelve_turns() {
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.units.clear();
    game.tile_mut(8, 24).terrain = Terrain::Grassland;
    game.create_city(HUMAN, 8, 24);
    let id = game.civil_war_playtest().unwrap();
    assert!(game.negotiate(1, Pact::Ceasefire));
    game.players[HUMAN].government = Government::Tribal;
    assert!(game.cancel_pact(HUMAN, 1));
    assert!(game.apply(Command::Move(id, 1, 0)));
    game.end_turn();
    assert!(game.negotiate(1, Pact::Ceasefire));
    game.diplomacy_playtest_technologies(false);
    // Exercise an established rival ready for another war. Native
    // dai_war_desire deliberately removes its expansion-era appetite
    // when every surviving city builds Settlers (as quiet cities now do).
    // A capital incident alone must not override that source policy.
    let garrisons: Vec<_> = game
        .cities
        .iter_mut()
        .filter(|city| city.owner == 1)
        .map(|city| {
            city.production = Production::Unit(UnitKind::Warrior);
            (city.x, city.y)
        })
        .collect();
    for (x, y) in garrisons {
        game.spawn_unit(1, UnitKind::Warrior, x, y);
    }
    assert!(game.ai_war_desire(1, HUMAN) > 0);
    game.diplomacy_playtest_tick(12);
    let key = (HUMAN, 1);
    assert_eq!(game.pact(HUMAN, 1), Pact::Ceasefire);
    assert_eq!(game.pact_offer(1), None);
    assert!(game.diplomacy[&key].trust[1] < 1250);
    assert!(game.ai_war_desire(1, HUMAN) > 0);
}
