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
        terms: [terms; 2],
    };
    sim.diplomacy
        .get_mut(&Simulation::relation_key(HUMAN, 1))
        .unwrap()
        .offer = Some(offer);
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
