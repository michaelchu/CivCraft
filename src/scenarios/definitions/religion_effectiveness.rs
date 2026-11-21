//! Actual culture gains and capped spread, with parallel policy controls.
use super::{Check, Scenario, ScenarioRun};
use crate::{
    rules::{Building, Tech},
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{FaithPolicy, Government, Simulation, drive::Drive, fixtures::WorldBuilder},
    world::GeneratedMap,
};
use serde_json::{Value, json};

pub(super) const CULTURE: Scenario = Scenario::new(
    "religion-ai-culture",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| setup(m, false),
    },
);
pub(super) const SATURATED: Scenario = Scenario::new(
    "religion-ai-saturated-spread",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| setup(m, true),
    },
);
fn setup(map: GeneratedMap, saturated: bool) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut g, ids) = super::religion_strategy::base(map);
    let mut w = WorldBuilder::new(&mut g);
    w.ai_control(2, false);
    w.ai_control(3, false);
    w.known_technologies(
        1,
        &Tech::all()
            .filter(|t| t.is_civ3_advance())
            .collect::<Vec<_>>(),
    );
    w.government(1, Government::DEMOCRACY);
    w.city_buildings(ids[1], &[Building::PALACE, Building::TEMPLE]);
    let sources = [
        ids[1],
        w.founding_city(1, (10, 24)),
        w.founding_city(1, (14, 24)),
    ];
    for &id in &sources[1..] {
        w.city_buildings(id, &[Building::TEMPLE]);
    }
    let mut targets = Vec::new();
    if saturated {
        for at in [(11, 22), (12, 22), (13, 22), (11, 26), (12, 26)] {
            let id = w.founding_city(1, at);
            w.city_buildings(id, &[Building::TEMPLE]);
            targets.push(id);
        }
    } else {
        w.city_history(ids[1], 6000);
    }
    w.finish();
    for target in &targets {
        let target = g.city_by_id(*target).unwrap();
        for source in sources {
            let source = g.city_by_id(source).unwrap();
            assert!(
                g.map_wrap
                    .civ3_distance(source.x, source.y, target.x, target.y)
                    <= 4,
                "every saturated target must receive all three sources"
            );
        }
    }
    assert!(g.found_religion(1, ids[1], 2));
    let mut w = WorldBuilder::new(&mut g);
    for &id in &sources {
        w.city_religion(id, &[(2, 300)], Some(2));
    }
    for &id in &targets {
        w.city_religion(id, &[(2, 90)], None);
    }
    w.finish();
    let mut control = g.clone();
    assert!(control.declare_faith(
        1,
        Some(2),
        if saturated {
            FaithPolicy::Missionary
        } else {
            FaithPolicy::Devout
        }
    ));
    let initial_culture = g.civilization_culture(1);
    (
        g,
        Box::new(Audit {
            control,
            saturated,
            targets,
            initial_culture,
            culture_gain: [0; 2],
            tolerant: true,
            equal_spread: false,
            converted: false,
            zero_marginal: false,
            decisions: 0,
            dropped: false,
            last_sequence: None,
        }),
    )
}
struct Audit {
    control: Simulation,
    saturated: bool,
    targets: Vec<u32>,
    initial_culture: i64,
    culture_gain: [i64; 2],
    tolerant: bool,
    equal_spread: bool,
    converted: bool,
    zero_marginal: bool,
    decisions: usize,
    dropped: bool,
    last_sequence: Option<u64>,
}
impl ScenarioRun for Audit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*"]
    }
    fn comparison_game(&mut self) -> Option<&mut Simulation> {
        Some(&mut self.control)
    }
    fn observe_turn(&mut self, g: &Simulation, completed: u32) -> Value {
        self.tolerant &= g.national_faith(1).policy == FaithPolicy::Tolerant;
        self.culture_gain = [
            g.civilization_culture(1) - self.initial_culture,
            self.control.civilization_culture(1) - self.initial_culture,
        ];
        let main = serde_json::to_value(g.observe_religion()).unwrap();
        let control = serde_json::to_value(self.control.observe_religion()).unwrap();
        self.equal_spread = self.targets.iter().all(|id| {
            let city = |v: &Value| {
                v["cities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["city"] == *id)
                    .unwrap()["influence"]
                    .clone()
            };
            city(&main) == city(&control)
        });
        self.converted = self
            .targets
            .iter()
            .all(|id| g.city_dominant_faith(*id) == Some(2));
        self.dropped |= g.trace.dropped() > 0;
        for e in g.trace.pending_events() {
            if self.last_sequence.is_some_and(|s| e.sequence <= s) {
                continue;
            }
            self.last_sequence = Some(e.sequence);
            if e.kind == "religion.ai_decision" && e.payload["owner"] == 1 {
                self.decisions += 1;
                if self.decisions == 1 {
                    self.zero_marginal = e.payload["missionary_opportunity"] == 0;
                }
            }
        }
        json!({"completed":completed,"saturated":self.saturated,"policy":g.national_faith(1).policy,"control_policy":self.control.national_faith(1).policy,"culture_gain":self.culture_gain,"equal_spread":self.equal_spread,"converted":self.converted,"religion":main,"control":control})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        let outcome = if self.saturated {
            completed >= 8
                && self.equal_spread
                && self.converted
                && self.zero_marginal
                && self.control.national_faith(1).policy == FaithPolicy::Missionary
        } else {
            completed >= 3 && self.control.national_faith(1).policy == FaithPolicy::Devout
        };
        vec![Check {
            name: if self.saturated {
                "capped_spread_avoids_wasted_missionary_policy"
            } else {
                "culture_goal_policy_improves_actual_history"
            },
            passed: outcome
                && self.tolerant
                && self.decisions > 0
                && !self.dropped
                && self.culture_gain[0] - self.culture_gain[1] >= 3 * i64::from(completed),
            detail: format!(
                "turns={completed}, Tolerant throughout={}, culture gain={:?}, equal spread={}, targets converted={}, zero marginal={}, decisions={}, dropped={}",
                self.tolerant,
                self.culture_gain,
                self.equal_spread,
                self.converted,
                self.zero_marginal,
                self.decisions,
                self.dropped
            ),
        }]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policies_deliver_culture_without_wasting_capped_spread() {
        let mut failures = Vec::new();
        for seed in [42, 7, 99] {
            for (scenario, turns) in [(CULTURE, 3), (SATURATED, 8)] {
                let ScenarioKind::Runtime { setup, .. } = scenario.kind else {
                    unreachable!()
                };
                let (mut g, mut audit) = setup(crate::mapgen::generate(seed));
                g.trace = crate::sim::trace::Collector::enabled(["*".into()].into(), 10000);
                for n in 0..turns {
                    g.simulate_turn();
                    audit.comparison_game().unwrap().simulate_turn();
                    audit.observe_turn(&g, n + 1);
                    g.trace.drain();
                }
                for c in audit.finish(turns) {
                    if !c.passed {
                        failures.push(format!("seed {seed}, {}: {}", scenario.id, c.detail));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
