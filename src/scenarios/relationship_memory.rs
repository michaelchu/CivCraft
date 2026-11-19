//! Executed exchanges, justified ally support and unjustified breaches.
use super::{Checks, Scenario};
use crate::sim::{HUMAN, Pact, Simulation, fixtures::WorldBuilder};
pub(super) const SCENARIO: Scenario = Scenario::new("relationship-memory", execute);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    super::proactive_trade::execute(game, &mut Checks::default());
    let (support_gain, breach_grievance) = Simulation::relationship_credit_rules();
    let commerce = game.observe_relationship(1, 2).commerce;
    checks.push((
        "completed_trade_memory",
        commerce > 0 && game.observe_relationship(2, 1).commerce > 0,
        "An executed AI technology purchase builds directional cooperation".into(),
    ));
    let before = game.encode_campaign().unwrap();
    let _ = game.observe_relationship(1, 2);
    checks.push((
        "readonly_observation",
        before == game.encode_campaign().unwrap(),
        "Inspecting relationship memory does not mutate the campaign".into(),
    ));
    // A third nation is already fighting nation 2; nation 1 joins its ally.
    let turn = game.turn();
    let mut world = WorldBuilder::new(game);
    world.founding_city(HUMAN, (8, 20));
    world.ai_control(HUMAN, true);
    world.diplomacy(HUMAN, 1, Pact::Alliance, turn, [6000; 2]);
    world.diplomacy(HUMAN, 2, Pact::War, turn, [0; 2]);
    world.diplomacy(1, 2, Pact::Ceasefire, turn, [0; 2]);
    world.treaty_expiry(1, 2, turn + 16);
    let trust = game
        .observe_diplomacy()
        .into_iter()
        .find(|pair| pair.players == [HUMAN, 1])
        .unwrap()
        .trust[0];
    game.declare_ai_war_now(1, 2);
    checks.push((
        "actual_ally_support",
        game.diplomatic_at_war(1, 2) && game.observe_relationship(HUMAN, 1).loyalty == support_gain,
        "Joining an ally's existing war earns loyalty only after war starts".into(),
    ));
    checks.push((
        "ally_support_context",
        game.observe_relationship(2, 1).grievance == 0
            && game
                .observe_diplomacy()
                .into_iter()
                .find(|pair| pair.players == [HUMAN, 1])
                .unwrap()
                .trust[0]
                == trust,
        "Helping an ally is not classified as an unjustified breach or punished by that ally"
            .into(),
    ));
    // A separate branch removes the ally-war justification.
    let mut breach = game.clone();
    WorldBuilder::new(&mut breach).diplomacy(HUMAN, 2, Pact::Peace, turn, [0; 2]);
    WorldBuilder::new(&mut breach).diplomacy(1, 2, Pact::Alliance, turn, [6000; 2]);
    breach.declare_ai_war_now(1, 2);
    checks.push((
        "betrayal_counted_once",
        breach.observe_relationship(2, 1).grievance == breach_grievance,
        "Alliance-to-armistice-to-war is one unjustified betrayal".into(),
    ));
}
