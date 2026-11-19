//! Observe real diplomatic advisor turns over independent controlled needs.
use super::{
    builder::{CivPreset, WorldBuilder},
    regression::{Check, Playtest, Scenario},
    *,
};
use crate::world::GeneratedMap;
use serde_json::{Value, json};

pub(super) const AID: Scenario =
    Scenario::playtest("diplomatic-aid", |map| setup(map, "ally_funding"));
pub(super) const MAPS: Scenario =
    Scenario::playtest("diplomatic-maps", |map| setup(map, "useful_map"));
pub(super) const RECOVERY: Scenario =
    Scenario::playtest("diplomatic-recovery", |map| setup(map, "war_recovery"));

fn setup(map: GeneratedMap, reason: &'static str) -> (Simulation, Box<dyn Playtest>) {
    let technologies: Vec<_> = Tech::all()
        .filter(|tech| {
            tech.era() == crate::rules::TechEra::Ancient
                && !matches!(tech.name(), "Monarchy" | "The Republic")
        })
        .collect();
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: Some(&technologies),
            },
            CivPreset {
                id: "korea",
                technologies: Some(&technologies),
            },
            CivPreset {
                id: "scandinavia",
                technologies: Some(&technologies),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(30, -2000);
    let mut cities = Vec::new();
    for (owner, x) in [(0, 8), (1, 24)] {
        for dy in -2..=2 {
            for dx in -2..=2 {
                world.grassland((x + dx, 12 + dy));
            }
        }
        let id = world.city(owner, (x, 12), 1);
        cities.push(id);
        if reason == "war_recovery" || (reason == "ally_funding" && owner == 1) {
            for building in [
                Building::LIBRARY,
                Building::MARKETPLACE,
                Building::BARRACKS,
                Building::COLOSSEUM,
            ] {
                world.city_improvement(id, building);
            }
        }
    }
    // A third living nation prevents an allied-victory exit before diplomacy.
    world.grassland((16, 34));
    world.city(2, (16, 34), 1);
    // Established relations need a continuing channel, rather than an expired
    // first-contact timer between distant islands.
    if reason == "useful_map" {
        for (owner, x) in [(0, 8), (1, 24)] {
            for dy in -2..=2 {
                for dx in -2..=2 {
                    world.grassland((x + dx, 26 + dy));
                }
            }
            world.unit(owner, UnitKind::Warrior, (x, 26));
        }
    }
    let pact = match reason {
        "ally_funding" => Pact::Alliance,
        "war_recovery" => Pact::War,
        _ => Pact::Peace,
    };
    world.diplomacy(
        0,
        1,
        pact,
        1,
        if pact == Pact::War {
            [-6000; 2]
        } else {
            [6000; 2]
        },
    );
    world.embassy(0, 1);
    world.finish();
    game.players[0].gold = if reason == "war_recovery" { 20 } else { 2000 };
    game.players[1].gold = if reason == "useful_map" { 2000 } else { 20 };
    let audit = Audit {
        reason,
        cities,
        executed: 0,
        proposed: 0,
        dropped: false,
        safe_donor: true,
        accepted_pact: false,
    };
    (game, Box::new(audit))
}

struct Audit {
    reason: &'static str,
    cities: Vec<u32>,
    executed: usize,
    proposed: usize,
    dropped: bool,
    safe_donor: bool,
    accepted_pact: bool,
}
impl Playtest for Audit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["diplomacy.negotiation_decision", "diplomacy.transition"]
    }
    fn observe_turn(&mut self, game: &Simulation, _completed_turns: u32) -> Value {
        self.dropped |= game.trace.dropped() > 0;
        let mut selected = Vec::new();
        for event in game.trace.pending_events() {
            if event.kind == "diplomacy.negotiation_decision"
                && (event.payload["reason"] == self.reason
                    || (self.reason == "useful_map" && event.payload["reason"] == "map_exchange"))
                && !event.payload["selected"].is_null()
            {
                self.proposed += 1;
                selected.push(event.payload["selected"].clone());
            }
            if event.kind == "diplomacy.transition"
                && event.payload["action"] == "offer_accepted"
                && selected.contains(&event.payload["offer"])
            {
                self.executed += 1;
            }
        }
        // The report retains selected terms as well as the actual post-turn
        // state; the observer never drives advisors or advances a turn itself.
        self.safe_donor &= self.reason != "ally_funding"
            || game.players[0].gold >= game.diplomatic_trade_reserve(0);
        self.accepted_pact |= game.pact(0, 1) == Pact::Ceasefire;
        json!({"reason":self.reason,"selected":selected,"executions":self.executed,
            "cities":self.cities,"gold":[game.players[0].gold,game.players[1].gold],"pact":game.pact(0,1)})
    }
    fn finish(self: Box<Self>, completed_turns: u32) -> Vec<Check> {
        let name = match self.reason {
            "ally_funding" => "diplomatic_aid_during_campaign",
            "useful_map" => "diplomatic_map_during_campaign",
            _ => "diplomatic_recovery_during_campaign",
        };
        vec![Check {
            name,
            passed: completed_turns > 0
                && !self.dropped
                && self.proposed > 0
                && self.executed > 0
                && self.safe_donor
                && (self.reason != "war_recovery" || self.accepted_pact),
            detail: format!(
                "{}: {} selected needs-based proposals, {} completed treaties over {completed_turns} normal turns; donor reserve safe: {}.",
                self.reason, self.proposed, self.executed, self.safe_donor
            ),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diplomacy_playtests_reach_their_outcomes_in_one_normal_turn() {
        for reason in ["ally_funding", "useful_map", "war_recovery"] {
            let (mut game, mut audit) = setup(crate::test_support::seeded_map(), reason);
            game.enable_all_civilizations_as_ai();
            game.trace = trace::Collector::enabled(["*".to_owned()].into(), 10000);
            game.simulate_turn();
            let state = audit.observe_turn(&game, 1);
            for check in audit.finish(1) {
                assert!(
                    check.passed,
                    "{}: {}. State: {state}",
                    check.name, check.detail
                );
            }
        }
    }
}
