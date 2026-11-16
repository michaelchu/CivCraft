//! Window-free military regression scenarios.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    Scenario::new("ai-paradrops", ai_paradrops),
    Scenario::new("paradrop-follow-up", paradrop_follow_up),
    Scenario::new("government-research", government_research),
    Scenario::new("government-worker-preview", government_worker_preview),
    Scenario::new("ai-domestic-buildings", ai_domestic_buildings),
    Scenario::new("ai-ground-target", ai_ground_target),
    Scenario::new("ai-overseas-assault", ai_overseas_assault),
    Scenario::new("ai-defended-overseas-assault", ai_defended_overseas_assault),
    Scenario::new("disband-and-recycle", disband_and_recycle),
    Scenario::new("transport-cargo-rescue", transport_cargo_rescue),
    Scenario::new("nested-transport-rescue", nested_transport_rescue),
    Scenario::new("movement-cost-routing", movement_cost_routing),
    Scenario::new("fog-of-war-routing", fog_of_war_routing),
    Scenario::new("remembered-map-routing", remembered_map_routing),
];

fn ai_paradrops(sim: &mut Simulation, checks: &mut Checks) {
    let (defended, refused, captured) = sim.ai_paradrop_playtest();
    checks.push(("ai_paradrops", defended && refused && captured,
                format!("AI reinforced an empty threatened city before attacking: {defended}. It refused the defended landing: {refused}. It captured the enemy city once the defender left: {captured}.")));
}

fn paradrop_follow_up(sim: &mut Simulation, checks: &mut Checks) {
    let (declined, struck) = sim.ai_paradrop_stack_playtest();
    checks.push(("paradrop_follow_up", struck,
                format!("AI selected a legal adjacent attack landing and executed the move-limited follow-up: {struck}. Lone-Warrior refusal is recorded as a diagnostic: {declined}.")));
}

fn government_research(sim: &mut Simulation, checks: &mut Checks) {
    let (research, locked, chose, finished) = sim.ai_government_playtest();
    checks.push((
                "government_research",
                research,
                "Monarchy at the research frontier became the goal and current subject using cached government values."
                    .to_owned(),
            ));
    checks.push(("ai_government", locked && chose && finished,
                format!("Locked government was excluded: {locked}. The AI chose unlocked Monarchy: {chose}. The AI kept and completed its revolution: {finished}.")));
}

fn government_worker_preview(sim: &mut Simulation, checks: &mut Checks) {
    let (improved, preserved) = sim.government_worker_preview_playtest();
    checks.push(("government_worker_preview", improved && preserved,
                format!("Candidate government score reflected a better workable tile: {improved}. Live worker and specialist assignments were preserved: {preserved}.")));
}

fn ai_domestic_buildings(sim: &mut Simulation, checks: &mut Checks) {
    let (science, growth) = sim.domestic_building_playtest();
    checks.push(("ai_domestic_buildings", science && growth,
                format!("A trade-rich city favored a Library: {science}. The same city at its growth cap favored a river Aqueduct: {growth}.")));
}

fn ai_ground_target(sim: &mut Simulation, checks: &mut Checks) {
    let (mobile, advanced, ferry_access) = sim.offensive_production_playtest();
    checks.push((
        "ai_ground_target",
        advanced,
        "The field unit marched toward the reachable city past a nearer island city.".to_owned(),
    ));
    checks.push(("ai_offensive_production", mobile && advanced && ferry_access,
                format!("AI chose a producible attacker for a reachable enemy city: {mobile}. It advanced toward the target: {advanced}. Freeciv virtual-ferry reachability preserved the target after the land corridor was cut: {ferry_access}.")));
}

fn ai_overseas_assault(sim: &mut Simulation, checks: &mut Checks) {
    let (embarked, guarded, captured) = sim.overseas_assault_playtest(false);
    checks.push(("ai_overseas_assault", embarked && guarded && captured,
                format!("Transport picked up a spare attacker: {embarked}. The source city retained its garrison: {guarded}. The landed force captured the island city: {captured}.")));
}

fn ai_defended_overseas_assault(sim: &mut Simulation, checks: &mut Checks) {
    let (embarked, guarded, captured) = sim.overseas_assault_playtest(true);
    checks.push(("ai_defended_overseas_assault", embarked && guarded && captured,
                format!("The AI rejected a lone attacker and embarked the stronger force: {embarked}. Source city retained a guard: {guarded}. The expedition captured the defended city: {captured}.")));
}

fn disband_and_recycle(sim: &mut Simulation, checks: &mut Checks) {
    let (recovered, outside, unloaded) = sim.disband_playtest();
    checks.push(("disband_and_recycle", recovered && outside && unloaded,
                format!("Recycled shields survived production switches: {recovered}. Distant recovery was rejected and field disband succeeded: {outside}. Cargo survived in port: {unloaded}.")));
}

fn transport_cargo_rescue(sim: &mut Simulation, checks: &mut Checks) {
    let (rescued, drowned) = sim.cargo_rescue_playtest();
    checks.push(("transport_cargo_rescue", rescued && drowned,
                format!("Cargo transferred to a friendly ship and woke from sentry: {rescued}. An enemy ship offered no rescue: {drowned}.")));
}

fn nested_transport_rescue(sim: &mut Simulation, checks: &mut Checks) {
    let (rescued, lost_with_carrier, helicopter_survived) = sim.nested_cargo_rescue_playtest();
    checks.push(("nested_transport_rescue", rescued && lost_with_carrier && helicopter_survived,
                format!("A Helicopter aboard a full Carrier rescued its Transport's Marine: {rescued}. Cargo aboard a Helicopter whose Carrier was lost also perished: {lost_with_carrier}. The Helicopter survived and was unboarded: {helicopter_survived}.")));
}

fn movement_cost_routing(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "movement_cost_routing",
        sim.road_route_playtest(),
        "Go To followed three connected road steps and arrived in one turn.".to_owned(),
    ));
}

fn fog_of_war_routing(sim: &mut Simulation, checks: &mut Checks) {
    checks.push(("fog_of_war_routing", sim.fog_route_playtest(),
                "Hidden terrain and enemy positions did not change the route. A revealed enemy caused a detour.".to_owned()));
}

fn remembered_map_routing(sim: &mut Simulation, checks: &mut Checks) {
    checks.push((
        "remembered_map_routing",
        sim.remembered_route_playtest(),
        "A road lost under fog remained in the route until the tile was seen again.".to_owned(),
    ));
}
