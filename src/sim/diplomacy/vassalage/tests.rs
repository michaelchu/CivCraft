//! Rule-level invariants; scenarios cover the actual command and turn paths.
use super::*;
use crate::sim::commands::Command;

fn fixture() -> Simulation {
    let mut game = war_planning_game();
    game.players[HUMAN].known.push(Tech::FEUDALISM);
    game.players[HUMAN].gold = 1000;
    game.players[1].ai = true;
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        game.set_scenario_diplomacy(a, b, Pact::Peace, 1, [6000, 6000]);
        game.make_diplomatic_contact(a, b);
    }
    game.spawn_unit(1, UnitKind::Warrior, 12, 24);
    for _ in 0..3 {
        game.spawn_unit(HUMAN, UnitKind::ModernArmor, 12, 23);
    }
    game.refresh_visibility();
    game
}
fn bind(game: &mut Simulation, master: usize, vassal: usize, kind: VassalKind) {
    if kind == VassalKind::Capitulation {
        game.sign_pact(master, vassal, Pact::War);
    }
    let terms = VassalTerms { master, kind };
    let changes = game.plan_vassalage(master, vassal, terms).unwrap();
    game.establish_vassalage(master, vassal, terms, changes);
}
#[test]
fn capitulation_uses_the_player_command_and_preserves_ownership() {
    let mut game = fixture();
    game.sign_pact(HUMAN, 1, Pact::War);
    let owners: Vec<_> = game.cities.iter().map(|c| (c.id, c.owner)).collect();
    let offer = game.vassal_offer_view(1).unwrap();
    assert!(game.apply(Command::ProposeTreaty(1, offer)));
    assert_eq!(game.vassal_master(1), Some(HUMAN));
    assert_eq!(
        owners,
        game.cities
            .iter()
            .map(|c| (c.id, c.owner))
            .collect::<Vec<_>>()
    );
    assert!(game.valid_vassalage());
    assert!(game.military_access(HUMAN, 1));
    assert!(!game.allied(HUMAN, 1));
    assert!(!game.apply(Command::CancelPact(1)));
    assert!(!game.cancel_pact(1, 2));
    let peace = TreatyOffer {
        pact: Some(Pact::Alliance),
        ..Default::default()
    };
    assert!(!game.execute_offer(1, 2, &peace));
}
#[test]
fn attacks_and_peace_expand_to_both_groups() {
    let mut game = fixture();
    game.tile_mut(24, 24).terrain = Terrain::Grassland;
    game.create_city(3, 24, 24);
    game.players[2].known.push(Tech::FEUDALISM);
    game.players[3].ai = true;
    game.set_scenario_diplomacy(2, 3, Pact::Peace, 1, [6000, 6000]);
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    bind(&mut game, 2, 3, VassalKind::Voluntary);
    assert!(game.cancel_pact(2, 1));
    for a in [0, 1] {
        for b in [2, 3] {
            assert!(game.at_war(a, b));
        }
    }
    game.sign_pact(0, 2, Pact::Ceasefire);
    for a in [0, 1] {
        for b in [2, 3] {
            assert_eq!(game.pact(a, b), Pact::Ceasefire);
        }
    }
    assert!(game.valid_vassalage());
}
#[test]
fn ceasefire_expiry_propagates_and_internal_peace_never_expires() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    game.sign_pact(0, 2, Pact::Ceasefire);
    let until = game.diplomacy[&(0, 2)].until;
    // Exercise an asymmetric timer; the shared planner must bring the subject along.
    game.diplomacy.get_mut(&(1, 2)).unwrap().until = until + 10;
    game.turn = until;
    game.advance_diplomacy();
    assert!(game.at_war(0, 2) && game.at_war(1, 2));
    assert_eq!(game.pact(0, 1), Pact::Peace);
    assert_eq!(game.diplomacy[&(0, 1)].until, 0);
}
#[test]
fn capitulation_ends_private_wars_but_retains_the_masters_enemies() {
    let mut game = fixture();
    game.sign_pact(1, 2, Pact::War);
    bind(&mut game, 0, 1, VassalKind::Capitulation);
    assert!(!game.at_war(1, 2));
    assert!(game.valid_vassalage());
    let mut game = fixture();
    game.sign_pact(0, 2, Pact::War);
    bind(&mut game, 0, 1, VassalKind::Capitulation);
    assert!(game.at_war(1, 2));
}
#[test]
fn voluntary_submission_inherits_wars_and_conflicts_fail_without_mutation() {
    let mut game = fixture();
    game.sign_pact(1, 2, Pact::War);
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    assert!(game.at_war(0, 2));
    let mut game = fixture();
    game.sign_pact(0, 2, Pact::Alliance);
    game.sign_pact(1, 2, Pact::War);
    let before = game.encode_campaign().unwrap();
    assert!(
        game.plan_vassalage(
            0,
            1,
            VassalTerms {
                master: 0,
                kind: VassalKind::Voluntary
            }
        )
        .is_err()
    );
    assert_eq!(before, game.encode_campaign().unwrap());
}
#[test]
fn cycles_nested_subjects_humans_and_missing_technology_are_rejected() {
    let mut game = fixture();
    game.players[0].known.retain(|&t| t != Tech::FEUDALISM);
    let terms = VassalTerms {
        master: 0,
        kind: VassalKind::Voluntary,
    };
    assert!(game.plan_vassalage(0, 1, terms).is_err());
    game.players[0].known.push(Tech::FEUDALISM);
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    assert!(
        game.plan_vassalage(
            1,
            2,
            VassalTerms {
                master: 1,
                kind: VassalKind::Voluntary
            }
        )
        .is_err()
    );
    assert!(
        game.plan_vassalage(
            1,
            0,
            VassalTerms {
                master: 1,
                kind: VassalKind::Voluntary
            }
        )
        .is_err()
    );
    assert!(
        game.plan_vassalage(
            2,
            0,
            VassalTerms {
                master: 2,
                kind: VassalKind::Voluntary
            }
        )
        .is_err()
    );
}
#[test]
fn independence_boundaries_use_land_and_population_together() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Capitulation);
    for tile in &mut game.tiles {
        tile.owner = None;
    }
    for t in &mut game.tiles[..20] {
        t.terrain = Terrain::Grassland;
        t.owner = Some(0);
    }
    for t in &mut game.tiles[20..30] {
        t.terrain = Terrain::Grassland;
        t.owner = Some(1);
    }
    game.vassalage.agreements.get_mut(&1).unwrap().starting_land = 10;
    for c in &mut game.cities {
        c.size = if c.owner == 0 { 10 } else { 5 };
    }
    assert_eq!(game.vassal_independence_reason(1), None);
    game.tiles[30].terrain = Terrain::Grassland;
    game.tiles[30].owner = Some(1);
    assert_eq!(game.vassal_independence_reason(1), None);
    game.cities.iter_mut().find(|c| c.owner == 1).unwrap().size = 6;
    assert_eq!(game.vassal_independence_reason(1), Some("relative_size"));
    game.cities.iter_mut().find(|c| c.owner == 1).unwrap().size = 1;
    for t in &mut game.tiles[25..31] {
        t.owner = None;
    }
    assert_eq!(game.vassal_independence_reason(1), None); // Exactly half retained.
    game.tiles[24].owner = None;
    assert_eq!(
        game.vassal_independence_reason(1),
        Some("protection_failed")
    );
}
#[test]
fn voluntary_eligibility_and_release_restore_permissions_without_erasing_knowledge() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    let start = game.turn;
    game.turn = start + 9;
    assert_eq!(game.vassal_independence_reason(1), None);
    game.turn += 1;
    assert_eq!(
        game.vassal_independence_reason(1),
        Some("voluntary_term_completed")
    );
    assert!(game.gives_shared_vision(1, 0));
    assert!(!game.cancel_shared_vision(1, 0));
    let known = game.tiles.iter().filter(|t| t.known_by[0]).count();
    assert!(game.apply(Command::ReleaseVassal(1)));
    assert!(!game.military_access(0, 1));
    assert!(!game.gives_shared_vision(1, 0));
    assert!(game.tiles.iter().filter(|t| t.known_by[0]).count() >= known);
}
#[test]
fn master_elimination_releases_subjects_before_victory_evaluation() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    game.cities.retain(|c| c.owner != 0);
    game.units.retain(|u| u.owner != 0);
    game.advance_vassalage();
    assert_eq!(game.vassal_master(1), None);
}
#[test]
fn incoming_offer_requires_human_acceptance_and_revalidates_strength() {
    let mut game = fixture();
    game.sign_pact(0, 1, Pact::War);
    game.advance_vassal_offers_for(1);
    let offer = game.treaty_offer(1).expect("AI surrender offer");
    assert_eq!(game.vassal_master(1), None);
    game.units.retain(|u| u.owner != 0);
    assert!(!game.apply(Command::AcceptOffer(1, offer)));
    assert_eq!(game.vassal_master(1), None);
}
#[test]
fn treaty_clause_cannot_bypass_regular_trade_valuation() {
    let mut game = fixture();
    game.sign_pact(0, 1, Pact::War);
    let mut offer = game.vassal_offer_view(1).unwrap();
    offer.terms[1].gold = 100;
    let before = game.encode_campaign().unwrap();
    assert!(!game.execute_offer(0, 1, &offer));
    assert_eq!(before, game.encode_campaign().unwrap());
}
#[test]
fn vassal_military_can_cross_protected_borders_without_transport_rights() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    let id = game.spawn_unit(0, UnitKind::Warrior, 11, 23);
    game.tile_mut(11, 24).terrain = Terrain::Grassland;
    game.tile_mut(11, 24).owner = Some(1);
    let unit = game.unit_by_id(id).unwrap();
    assert!(!game.treaty_blocks_move(unit, 11, 24));
    assert!(!game.allied(0, 1));
}

#[test]
fn conquest_includes_surviving_subjects_but_does_not_award_allied_victory() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Capitulation);
    game.cities.retain(|c| c.owner != 2);
    game.units.retain(|u| u.owner != 2);
    assert!(!game.check_allied_victory());
    game.check_game_over();
    assert!(game.game_over.is_some());
    assert!(game.player_alive(1));
}

#[test]
fn protected_city_garrisons_are_visible_and_accessible() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    let id = game.units.iter().find(|u| u.owner == 1).unwrap().id;
    assert!(game.unit_visible_to(HUMAN, game.unit_by_id(id).unwrap()));
    let own = game.spawn_unit(HUMAN, UnitKind::Warrior, 12, 23);
    assert!(!game.treaty_blocks_move(game.unit_by_id(own).unwrap(), 12, 24));
    assert!(
        game.movement_cost_for(game.unit_by_id(own).unwrap(), 12, 24)
            .is_some()
    );
}

#[test]
fn senate_veto_is_checked_for_the_initiator_and_not_each_subject() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    game.players[1].government = Government::Democracy;
    assert!(game.senate_blocks_cancellation(1, 2));
    assert!(game.cancel_pact(0, 2));
    assert!(game.at_war(0, 2) && game.at_war(1, 2));
}

#[test]
fn protected_vision_does_not_erase_separately_negotiated_grants() {
    let mut game = fixture();
    game.diplomacy.get_mut(&(0, 1)).unwrap().vision[1] = true;
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    assert!(game.release_vassal(0, 1));
    assert!(game.gives_shared_vision(1, 0));
}

#[test]
fn armistice_expiry_uses_the_final_peace_state_without_renewing_the_timer() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    game.sign_pact(0, 2, Pact::Peace);
    let until = game.diplomacy[&(0, 2)].until;
    game.diplomacy.get_mut(&(1, 2)).unwrap().until = until + 5;
    game.turn = until;
    game.advance_diplomacy();
    for pair in [(0, 2), (1, 2)] {
        assert_eq!(game.diplomacy[&pair].pact, Pact::Peace);
        assert_eq!(game.diplomacy[&pair].until, 0);
    }
}

#[test]
fn independent_ai_master_can_accept_the_same_capitulation_terms() {
    let mut game = fixture();
    game.players[0].ai = true;
    game.sign_pact(0, 1, Pact::War);
    game.advance_vassal_offers_for(1);
    assert_eq!(game.vassal_master(1), Some(0));
    assert!(!game.advisor_player_dangerous(1, 0));
    assert!(!game.ai_potentially_hostile(1, 0));
}

#[test]
fn independence_gives_every_former_group_member_the_same_withdrawal_period() {
    let mut game = fixture();
    game.tile_mut(24, 24).terrain = Terrain::Grassland;
    game.create_city(3, 24, 24);
    game.players[3].ai = true;
    game.set_scenario_diplomacy(0, 3, Pact::Peace, 1, [6000, 6000]);
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    bind(&mut game, 0, 3, VassalKind::Voluntary);
    assert!(game.release_vassal(0, 1));
    for pair in [(0, 1), (1, 3)] {
        assert_eq!(game.diplomacy[&pair].pact, Pact::Armistice);
        assert_eq!(game.diplomacy[&pair].until, game.turn + 16);
    }
    assert!(game.valid_vassalage());
}

#[test]
fn ai_protector_refuses_a_voluntary_offer_with_overwhelming_inherited_pressure() {
    let mut game = fixture();
    game.players[0].ai = true;
    game.sign_pact(1, 2, Pact::War);
    game.tile_mut(13, 24).terrain = Terrain::Grassland;
    for _ in 0..20 {
        game.spawn_unit(2, UnitKind::ModernArmor, 13, 24);
    }
    game.refresh_visibility();
    let terms = VassalTerms {
        master: 0,
        kind: VassalKind::Voluntary,
    };
    assert!(game.plan_vassalage(0, 1, terms).is_ok());
    assert!(!game.ai_accepts_vassalage(0, 1, terms));
}

#[test]
fn subjects_remain_available_for_diplomacy_after_physical_contact_lapses() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    game.diplomatic_contact_until.clear();
    game.diplomacy.get_mut(&(0, 1)).unwrap().embassy = [false, false];
    assert!(game.can_negotiate(1));
    assert!(
        game.diplomacy_overview_view()
            .iter()
            .any(|r| r.owner == 1 && r.can_release)
    );
}

#[test]
fn propagated_armistice_expiry_also_expels_subject_garrisons() {
    let mut game = fixture();
    bind(&mut game, 0, 1, VassalKind::Voluntary);
    game.sign_pact(0, 2, Pact::Peace);
    let unit = game.spawn_unit(1, UnitKind::Warrior, 18, 30);
    game.tile_mut(18, 30).owner = Some(2);
    let until = game.diplomacy[&(0, 2)].until;
    game.diplomacy.get_mut(&(1, 2)).unwrap().until = until + 5;
    game.turn = until;
    game.advance_diplomacy();
    assert!(game.unit_by_id(unit).is_none());
}
