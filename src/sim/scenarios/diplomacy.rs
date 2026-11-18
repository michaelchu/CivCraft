//! Diplomacy fixtures shared by screenshot review scenarios.

use super::super::diplomacy::Concessions;
use super::super::*;

/// Controlled war-ending scenario for the diplomacy interaction gallery.
pub(crate) fn ui_review_compensation_negotiation(sim: &mut Simulation, other: usize) {
    sim.players[HUMAN].gold = 250;
    let relation = sim
        .diplomacy
        .get_mut(&Simulation::relation_key(HUMAN, other))
        .expect("the diplomacy fixture establishes contact");
    relation.pact = Pact::Ceasefire;
    relation.until = sim.turn + 4;
    relation.trust[usize::from(other > HUMAN)] = 1000;
}

/// Worst-case concessions for the isolated, opt-in screenshot gallery.
pub(crate) fn ui_review_offer(sim: &mut Simulation) -> TreatyOffer {
    let city = sim.cities_owned_by(1).next().unwrap().id;
    let terms = Concessions::default()
        .set_gold(12345)
        .set_technology(Some(Tech::AMPHIBIOUS_WARFARE))
        .set_shared_vision(true)
        .set_map(Some(TreatyMap::World))
        .set_embassy(true)
        .set_city(Some(city));
    let offer = TreatyOffer {
        pact: Some(Pact::Alliance),
        terms: [terms.clone(), terms],
    };
    sim.diplomacy
        .get_mut(&Simulation::relation_key(HUMAN, 1))
        .unwrap()
        .offer = Some(offer.clone());
    offer
}

pub(crate) fn ui_review_contacts(sim: &mut Simulation) {
    for other in 1..PLAYER_COUNT {
        sim.make_diplomatic_contact(HUMAN, other);
        // Established peace lets the gallery exercise city purchase controls.
        let relation = sim
            .diplomacy
            .get_mut(&Simulation::relation_key(HUMAN, other))
            .unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
    }
}

/// Reusable headless coverage for the diplomacy parity repairs. The caller
/// attaches tracing before this helper so treaty/war transitions remain visible.
pub(crate) fn diplomacy_parity(sim: &mut Simulation) -> Vec<(&'static str, bool, String)> {
    sim.units.clear();
    sim.cities.clear();
    sim.diplomacy.clear();
    sim.ai_war_plans.clear();
    for player in &mut sim.players {
        player.ai = false;
    }
    for (owner, x, y) in [(HUMAN, 8, 20), (1, 12, 24), (2, 18, 30)] {
        sim.tile_mut(x, y).terrain = Terrain::Grassland;
        sim.create_city(owner, x, y);
    }
    sim.tile_mut(12, 23).terrain = Terrain::Grassland;
    let worker = sim.spawn_unit(HUMAN, UnitKind::Worker, 12, 23);
    let warrior = sim.spawn_unit(HUMAN, UnitKind::Warrior, 12, 23);
    sim.sign_pact(HUMAN, 1, Pact::Peace);
    sim.tile_mut(12, 23).owner = Some(1);
    sim.diplomacy.get_mut(&(HUMAN, 1)).unwrap().until = sim.turn;
    sim.advance_diplomacy();
    let civilian_preserved = sim.unit_by_id(worker).is_some();
    let military_removed = sim.unit_by_id(warrior).is_none();

    sim.players[HUMAN].known.clear();
    sim.players[1].known = vec![Tech::ALPHABET, Tech::POTTERY];
    let mut offer = TreatyOffer::default();
    offer.terms[1] = Concessions::default()
        .add_technology(Tech::ALPHABET)
        .add_technology(Tech::POTTERY);
    let exchanged = sim.propose_treaty(1, offer)
        && sim.has_tech(HUMAN, Tech::ALPHABET)
        && sim.has_tech(HUMAN, Tech::POTTERY);

    sim.players[1].ai = true;
    sim.ai_war_plans.insert(
        (1, HUMAN),
        crate::sim::diplomacy::WarPlan {
            countdown: 0,
            reason: crate::sim::diplomacy::WarReason::Hatred,
        },
    );
    let ending_turn = sim.turn;
    sim.simulate_turn();
    let declared = sim.turn == ending_turn + 1 && sim.pact(HUMAN, 1) == Pact::War;
    vec![
        (
            "civilian_preserved",
            civilian_preserved,
            "Armistice expiry preserves a Worker on foreign territory".into(),
        ),
        (
            "military_removed",
            military_removed,
            "Armistice expiry removes the Warrior on that tile".into(),
        ),
        (
            "multiple_technologies",
            exchanged,
            "One treaty transfers Alphabet and Pottery".into(),
        ),
        (
            "begin_phase_war",
            declared,
            "A prepared AI declaration executes in the following begin phase".into(),
        ),
    ]
}
