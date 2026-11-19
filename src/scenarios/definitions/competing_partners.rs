//! Comparing military and ordinary trade after cooperation or a breach.

use super::*;
use crate::{
    rules::Tech,
    sim::{Pact, Simulation, TreatyOffer, drive::Drive, fixtures::WorldBuilder},
};

const SELLER: usize = 1;
const RELIABLE: usize = 2;
const BREACHER: usize = 3;
const TRUST: [i32; 2] = [6000; 2];

pub(super) const SCENARIO: Scenario = Scenario::new(
    "competing-trade-partners",
    1,
    ScenarioKind::Actions {
        control: Control::Human,
        execute,
    },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
    let mut world = WorldBuilder::new(game);
    world.clear_campaign_entities();
    for (owner, position) in [
        (SELLER, (10, 22)),
        (RELIABLE, (17, 25)),
        (BREACHER, (24, 30)),
    ] {
        world.founding_city(owner, position);
        world.ai_control(owner, true);
        world.known_technologies(owner, &[Tech::ALPHABET, Tech::BRONZE_WORKING]);
        world.treasury(owner, 2000);
    }
    world.add_technology(RELIABLE, writing);
    world.research_goal(SELLER, writing);
    world.last_science(SELLER, 20);
    world.diplomacy(SELLER, RELIABLE, Pact::Alliance, 0, TRUST);
    world.embassy(SELLER, RELIABLE);
    world.finish();

    game.advance_diplomatic_offers_for(SELLER);
    let mut world = WorldBuilder::new(game);
    world.diplomacy(SELLER, BREACHER, Pact::Alliance, 0, TRUST);
    world.embassy(SELLER, BREACHER);
    game.declare_ai_war_now(BREACHER, SELLER);
    let cooperation = game.observe_relationship(SELLER, RELIABLE).commerce;
    let grievance = game.observe_relationship(SELLER, BREACHER).grievance;
    let mut world = WorldBuilder::new(game);
    // Equalize current pact, trust, knowledge and funds while retaining history.
    for partner in [RELIABLE, BREACHER] {
        world.diplomatic_state(SELLER, partner, Pact::Peace, TRUST);
        world.known_technologies(partner, &[Tech::ALPHABET, Tech::BRONZE_WORKING, writing]);
        world.treasury(partner, 2000);
        // These bidders submit the prescribed offer. The seller's AI alone
        // decides whether to accept it.
        world.ai_control(partner, false);
    }
    world.known_technologies(
        SELLER,
        &[
            Tech::ALPHABET,
            Tech::BRONZE_WORKING,
            writing,
            Tech::IRON_WORKING,
            Tech::POTTERY,
        ],
    );
    world.treasury(SELLER, 2000);
    let military = Tech::IRON_WORKING;
    let ordinary = Tech::POTTERY;
    let military_value = [RELIABLE, BREACHER]
        .map(|partner| game.diplomatic_technology_value(SELLER, partner, military, true));
    let ordinary_value = [RELIABLE, BREACHER]
        .map(|partner| game.diplomatic_technology_value(SELLER, partner, ordinary, true));

    // Compare equal military bids against cloned states. Then try a low-risk
    // purchase from the same partner whose military bid was evaluated.
    let military_price = military_value[0].unwrap_or(1).saturating_add(1) as i32;
    let mut comparison = game.clone();
    let sold_reliable = submit_bid(game, RELIABLE, military, military_price);
    let sold_breacher = submit_bid(&mut comparison, BREACHER, military, military_price);
    let low_risk_price = ordinary_value[1].unwrap_or(1).saturating_add(1) as i32;
    let sold_ordinary = submit_bid(game, BREACHER, ordinary, low_risk_price);
    checks.extend([
        ("competing_partners_have_distinct_history", cooperation > 0 && grievance > 0,
            format!("Completed trade cooperation {cooperation}, pact breach grievance {grievance}")),
        ("sensitive_trade_favors_reliable_partner",
            military_value[0].is_ok() && military_value[1] == Err("military_security") && sold_reliable && !sold_breacher,
            format!("Equal Iron Working bids of {military_price}, reliable valuation {:?} and sale {sold_reliable}, breacher valuation {:?} and sale {sold_breacher}", military_value[0], military_value[1])),
        ("low_risk_trade_survives_grievance", ordinary_value[1].is_ok() && sold_ordinary && game.has_tech(BREACHER, ordinary),
            format!("Pottery bid of {low_risk_price}, breacher valuation {:?} and sale {sold_ordinary}", ordinary_value[1])),
    ]);
}

fn submit_bid(game: &mut Simulation, buyer: usize, technology: Tech, gold: i32) -> bool {
    let mut offer = TreatyOffer::default();
    offer.terms[0].technology = Some(technology);
    offer.terms[1].gold = gold;
    game.execute_offer(buyer, SELLER, &offer)
}
