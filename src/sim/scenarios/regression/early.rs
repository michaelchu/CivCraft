//! Window-free early regression scenarios.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::new("barbarian-uprisings", barbarian_uprisings),
    Scenario::new("ai-expansion", ai_expansion),
    Scenario::new("overseas-settlement", overseas_settlement),
    Scenario::new("settlement-ferry-production", settlement_ferry_production),
    Scenario::new("inland-overseas-settlement", inland_overseas_settlement),
    Scenario::new("ferry-research", ferry_research),
    Scenario::new("offshore-settlement-pickup", offshore_settlement_pickup),
    Scenario::new("inland-ferry-pickup", inland_ferry_pickup),
    Scenario::new("rendezvous-route-selection", rendezvous_route_selection),
    Scenario::new("city-border-claims", city_border_claims),
    Scenario::new("culture-accumulation", culture_accumulation),
    Scenario::new("fogged-borders", fogged_borders),
    Scenario::new("ai-city-defense", ai_city_defense),
    Scenario::new("ai-fortifications", ai_fortifications),
    Scenario::new("ai-reinforcements", ai_reinforcements),
    Scenario::new("ai-counterattacks", ai_counterattacks),
    Scenario::new("ai-fighter-strike", ai_fighter_strike),
    Scenario::new("campaign-save-load", campaign_save_load),
];

fn barbarian_uprisings(sim: &mut Simulation, checks: &mut Checks) {
    let mut land = sim.clone();
    let mut sea = sim.clone();

    let (land, sea) = (
        land.uprising_playtest(false, false),
        sea.uprising_playtest(true, false),
    );
    checks.push(("barbarian_uprisings", land && sea,
                format!("Recurring uprising spawned land raiders: {land}. Sea uprising loaded raiders aboard a barbarian ship: {sea}. Both respected the turn-120 onset.")));
}

fn ai_expansion(sim: &mut Simulation, checks: &mut Checks) {
    let (production, site, founded) = sim.ai_expansion_playtest();
    checks.push(("ai_expansion", production && site && founded,
                format!("An AI with three cities chose to build a Settler: {production}. Its plan stayed on reachable land: {site}. It completed the trip before founding: {founded}.")));
}

fn overseas_settlement(sim: &mut Simulation, checks: &mut Checks) {
    let (production, embarked, founded) = sim.overseas_settlement_playtest(false);
    checks.push(("overseas_settlement", production && embarked && founded,
                format!("A viable ferry route justified Settler production: {production}. The ferry picked up the Settler and sailed: {embarked}. The Settler founded a city on the island: {founded}.")));
}

fn settlement_ferry_production(sim: &mut Simulation, checks: &mut Checks) {
    let (boat, founder, settled) = sim.settlement_ferry_production_playtest();
    checks.push(("settlement_ferry_production", boat && founder && settled,
                format!("Map Making let the AI queue a settlement Trireme: {boat}. Completing the ship queued a Settler: {founder}. The pair founded a city overseas: {settled}.")));
}

fn inland_overseas_settlement(sim: &mut Simulation, checks: &mut Checks) {
    let (production, embarked, founded) = sim.overseas_settlement_playtest(true);
    checks.push(("inland_overseas_settlement", production && embarked && founded,
                format!("An inland site justified production: {production}. The ferry sailed: {embarked}. The Settler landed before founding inland: {founded}.")));
}

fn ferry_research(sim: &mut Simulation, checks: &mut Checks) {
    let (inland, coastal, resumed) = sim.ferry_research_playtest();
    checks.push(("ferry_research", inland && coastal && resumed,
                format!("Inland city kept its research goal: {inland}. Port prioritized Map Making and its prerequisites: {coastal}. The AI selected an available advance toward its newly scored goal after unlocking ferries: {resumed}.")));
}

fn offshore_settlement_pickup(sim: &mut Simulation, checks: &mut Checks) {
    let (waiting, embarked, founded) = sim.offshore_pickup_playtest(false);
    checks.push(("offshore_settlement_pickup", waiting && embarked && founded,
                format!("Settler waited on open shore as the ferry approached: {waiting}. It boarded the offshore ferry: {embarked}. It founded on the destination island: {founded}.")));
}

fn inland_ferry_pickup(sim: &mut Simulation, checks: &mut Checks) {
    let (walked, embarked, founded) = sim.offshore_pickup_playtest(true);
    checks.push(("inland_ferry_pickup", walked && embarked && founded,
                format!("Settler and ferry traveled toward a rendezvous: {walked}. Settler boarded: {embarked}. It founded a city overseas: {founded}.")));
}

fn rendezvous_route_selection(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "rendezvous_route_selection",
        sim.rendezvous_route_playtest(),
        "A later pickup won because it shortened the full route to the island.".to_owned(),
    ));
}

fn city_border_claims(sim: &mut Simulation, checks: &mut Checks) {
    let (tie, history, size_reach) = sim.city_borders_playtest();
    checks.push(("city_border_claims", tie && history && size_reach,
                format!("Equal Freeciv border claims kept the earlier source: {tie}. City culture history did not alter border strength: {history}. C2C3 city size expanded border reach: {size_reach}.")));
}

fn culture_accumulation(sim: &mut Simulation, checks: &mut Checks) {
    let (before_boundary, at_boundary, turn_refresh) = sim.culture_accumulation_playtest();
    checks.push((
                "culture_accumulation",
                before_boundary && at_boundary && turn_refresh,
                format!(
                    "City history accrued from selected ruleset effects and contributed to culture score: {before_boundary}. National culture reflected the city's Freeciv score: {at_boundary}. C2C3 culture victory remained disabled by default: {turn_refresh}."
                ),
            ));
}

fn fogged_borders(sim: &mut Simulation, checks: &mut Checks) {
    let (water, fog) = sim.fogged_border_playtest();
    checks.push(("fogged_borders", water && fog,
                format!("City claimed adjacent coast and the enclosed lake but left open sea unclaimed: {water}. Ownership stayed last-seen under fog and updated on return: {fog}.")));
}

fn ai_city_defense(sim: &mut Simulation, checks: &mut Checks) {
    let (budget, quiet, threatened) = sim.ai_city_defense_playtest();
    checks.push(("ai_city_defense", budget && quiet && threatened,
                format!("Native defender priorities held across shield stocks: {budget}. A safe city kept a non-defender choice: {quiet}. A weak garrison facing an enemy attacker switched to a defender: {threatened}.")));
}

fn ai_fortifications(sim: &mut Simulation, checks: &mut Checks) {
    let (empty, walls, naval) = sim.ai_fortifications_playtest();
    checks.push(("ai_fortifications", empty && walls && naval,
                format!("Empty city chose a defender: {empty}. Garrisoned city chose walls against a Cannon: {walls}. Native generic naval-defense choice and actual land/sea effect distinctions matched: {naval}.")));
}

fn ai_reinforcements(sim: &mut Simulation, checks: &mut Checks) {
    let (sent, held, reserve) = sim.ai_reinforcements_playtest();
    checks.push(("ai_reinforcements", sent && held && reserve,
                format!("A reserve Marine marched toward the threatened city: {sent}. The Marine arrived and held: {held}. The source city kept its Warrior: {reserve}.")));
}

fn ai_counterattacks(sim: &mut Simulation, checks: &mut Checks) {
    let (held, attack, _diagnostic) = sim.ai_counterattack_playtest();
    checks.push(("ai_counterattacks", held && attack,
                format!("Lone defender stayed: {held}. Spare Marine destroyed the adjacent Cannon while a guard stayed: {attack}. A separate Warrior matchup is diagnostic because Freeciv rampage uses scored choices, not a categorical refusal rule.")));
}

fn ai_fighter_strike(sim: &mut Simulation, checks: &mut Checks) {
    let (intercepted, skipped, refueled) = sim.ai_fighter_playtest();
    checks.push(("ai_fighter_strike", intercepted && skipped && refueled,
                format!("Fighter used its explicit strike path against a nearby bomber: {intercepted}. The fighter waited for full fuel before another sortie: {skipped}. Fighter returned and refueled: {refueled}.")));
}

fn campaign_save_load(sim: &mut Simulation, checks: &mut Checks) {
    let (passed, detail) = match sim.save_campaign_playtest() {
        Ok((restored, next, rejected)) => (
            restored && next && rejected,
            format!(
                "Campaign restored preserved state with native task resets: {restored}. Two loaded replays continued identically: {next}. Save replacement, legacy migrations, and corrupt-file rejection worked: {rejected}."
            ),
        ),
        Err(error) => (false, error.to_string()),
    };
    checks.push(("campaign_save_load", passed, detail));
}
