//! Normal-turn religion choices under different strategic goals and alliances.
use super::{Check, Scenario, ScenarioRun};
use crate::{
    rules::{Building, Tech},
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{
        FaithPolicy, HUMAN, Pact, Production, Simulation, UnitKind,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
    },
    world::{GeneratedMap, Terrain},
};
use serde_json::{Value, json};
pub(super) const GOALS: Scenario = Scenario::new(
    "religion-ai-goals",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_goals,
    },
);
pub(super) const COALITION: Scenario = Scenario::new(
    "religion-ai-coalition",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_coalition,
    },
);
pub(super) fn base(map: GeneratedMap) -> (Simulation, Vec<u32>) {
    let techs = [
        Tech::MEDITATION,
        Tech::POLYTHEISM,
        Tech::MONOTHEISM,
        Tech(12),
        Tech(58),
        Tech::THEOLOGY,
        Tech::DIVINE_RIGHT,
    ];
    let mut g = WorldBuilder::ocean_campaign(
        map,
        &["rome", "korea", "egypt", "carthage"].map(|id| CivPreset {
            id,
            technologies: Some(&techs),
        }),
    );
    let mut w = WorldBuilder::new(&mut g);
    w.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    let mut ids = Vec::new();
    for owner in 0..4 {
        let at = (6 + owner as i32 * 6, 24);
        let id = w.city(owner, at, 3);
        ids.push(id);
        w.city_workers(id, 3);
        w.city_buildings(id, &[Building::PALACE]);
        w.unit(owner, UnitKind::Warrior, at);
        w.production_stock(id, Production::Building(Building::COINAGE), 0);
        w.economy(owner, 500, [40, 60, 0]);
    }
    w.finish();
    (g, ids)
}
fn setup_goals(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut g, ids) = base(map);
    let mut w = WorldBuilder::new(&mut g);
    w.city_history(ids[2], 6000);
    w.finish();
    (g, Box::new(Goals { result: Vec::new() }))
}
struct Goals {
    result: Vec<(usize, u32)>,
}
impl ScenarioRun for Goals {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*"]
    }
    fn observe_turn(&mut self, g: &Simulation, _completed: u32) -> Value {
        self.result = g
            .religion_catalog()
            .iter()
            .map(|f| (f.founder, f.id))
            .collect();
        json!({"founders":self.result,"strategies":(1..4).map(|i|g.strategic_priorities(i)).collect::<Vec<_>>(),"religion":g.observe_religion()})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        vec![Check {
            name: "religion_goal_driven_foundation",
            passed: completed >= 1
                && [(1, 4), (2, 2), (3, 3)]
                    .iter()
                    .all(|p| self.result.contains(p)),
            detail: format!(
                "Science Korea -> Confucianism, culture Egypt -> Hinduism, economy Carthage -> Judaism: {:?}",
                self.result
            ),
        }]
    }
}
fn setup_coalition(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut g, ids) = base(map);
    assert!(g.found_religion(HUMAN, ids[0], 1));
    assert!(g.declare_faith(HUMAN, Some(1), FaithPolicy::Devout));
    assert!(g.found_religion(1, ids[1], 3));
    let mut w = WorldBuilder::new(&mut g);
    w.ai_control(2, false);
    w.ai_control(3, false);
    w.city_religion(ids[1], &[(1, 100), (3, 300)], Some(3));
    w.diplomacy(HUMAN, 1, Pact::Alliance, 1, [2000; 2]);
    w.finish();
    (
        g,
        Box::new(Coalition {
            shared: false,
            alliance: false,
        }),
    )
}
struct Coalition {
    shared: bool,
    alliance: bool,
}
impl ScenarioRun for Coalition {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*"]
    }
    fn observe_turn(&mut self, g: &Simulation, _completed: u32) -> Value {
        self.shared = g.national_faith(1).declared == Some(1);
        self.alliance = g.contacted_pact(HUMAN, 1) == Some(Pact::Alliance);
        json!({"shared":self.shared,"alliance":self.alliance,"religion":g.observe_religion()})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        vec![Check {
            name: "religion_coalition_adoption",
            passed: completed >= 1 && self.shared && self.alliance,
            detail: format!(
                "Adopted an ally's present minority faith={}, alliance preserved={}",
                self.shared, self.alliance
            ),
        }]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal_turns_choose_goal_aligned_faiths_and_preserve_coalitions() {
        for setup in [
            setup_goals as fn(GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>),
            setup_coalition,
        ] {
            let (mut g, mut audit) = setup(crate::test_support::seeded_map());
            g.simulate_turn();
            audit.observe_turn(&g, 1);
            for c in audit.finish(1) {
                assert!(c.passed, "{}", c.detail);
            }
        }
    }
}
