//! Peaceful contact, cautious trust, explicit war and barbarian hostility.
use super::{Checks, Scenario};
use crate::scenarios::{Control, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    sim::{
        BARBARIANS, HUMAN, MoveResult, Pact, Simulation, TreatyOffer, UnitKind,
        fixtures::WorldBuilder,
    },
    world::topology::offset_native_by_map,
};
pub(super) const SCENARIO: Scenario = Scenario::new(
    "peaceful-first-contact",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);
fn execute(sim: &mut Simulation, checks: &mut Checks) {
    let mut world = WorldBuilder::new(sim);
    world.clear_campaign_entities();
    world.ai_control(1, true);
    for (owner, x, y) in [(HUMAN, 8, 20), (1, 12, 24)] {
        world.founding_city(owner, (x, y));
    }
    let origin = offset_native_by_map(12, 24, -1, 0);
    world.grassland(origin);
    let actor = world.unit(HUMAN, UnitKind::Warrior, origin);
    world.unit(1, UnitKind::Spearman, (12, 24));
    sim.refresh_diplomatic_contact_at(HUMAN, origin.0, origin.1);
    let peaceful = sim.contacted_pact(HUMAN, 1) == Some(Pact::Peace);
    let relation = sim
        .observe_diplomacy()
        .into_iter()
        .find(|pair| pair.players == [HUMAN, 1])
        .expect("physical contact creates a relationship");
    let cautious = (-20..=20).contains(&relation.trust[1])
        && relation.trust[0] == 0
        && relation.shared_vision == [false; 2]
        && relation.embassy == [false; 2];
    let trust = relation.trust[1];
    let direction = sim
        .direction_for_unit_offset(actor, 1, 0)
        .expect("the defender is adjacent to the attacker");
    let attack_blocked = sim.try_move_unit(actor, direction) == MoveResult::Blocked;

    WorldBuilder::new(sim).treasury(HUMAN, 100);
    let mut offer = TreatyOffer::default();
    offer.terms[0].gold = 100;
    let trade_builds_trust = sim.propose_treaty(1, offer)
        && sim
            .observe_diplomacy()
            .into_iter()
            .find(|pair| pair.players == [HUMAN, 1])
            .unwrap()
            .trust[1]
            > trust
        && sim.pact(HUMAN, 1) == Pact::Peace;
    let declared = sim.cancel_pact(HUMAN, 1)
        && sim.at_war(HUMAN, 1)
        && sim.try_move_unit(actor, direction) == MoveResult::Fought;
    sim.make_diplomatic_contact(1, HUMAN);
    let war_preserved = sim.at_war(HUMAN, 1);

    WorldBuilder::new(sim).grassland((28, 24));
    WorldBuilder::new(sim).unit(BARBARIANS, UnitKind::Warrior, (28, 24));
    sim.make_diplomatic_contact(HUMAN, BARBARIANS);
    let barbarians_hostile = sim.contacted_pact(HUMAN, BARBARIANS) == Some(Pact::War);
    checks.extend([
        (
            "peaceful_contact",
            peaceful,
            "Physical first contact starts at Peace".into(),
        ),
        (
            "cautious_contact",
            cautious,
            "Initial trust stays low without diplomatic privileges".into(),
        ),
        (
            "attack_requires_war",
            attack_blocked && declared,
            "Attacks require an explicit declaration".into(),
        ),
        (
            "trade_builds_trust",
            trade_builds_trust,
            "A gold gift increases AI trust while preserving Peace".into(),
        ),
        (
            "contact_preserves_war",
            war_preserved,
            "Refreshing contact does not end an established war".into(),
        ),
        (
            "barbarians_hostile",
            barbarians_hostile,
            "Barbarians remain hostile on contact".into(),
        ),
    ]);
}
