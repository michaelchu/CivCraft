//! Faith-only counterfactuals exercise actual treaty acceptance and war planning.
use super::{Check, Scenario, ScenarioRun};
use crate::{
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{
        FaithPolicy, HUMAN, Pact, Simulation, TreatyOffer, drive::Drive, fixtures::WorldBuilder,
    },
    world::GeneratedMap,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug)]
enum Case {
    Shared,
    Different,
    Tolerant,
    War,
}
macro_rules! scenario {
    ($constant:ident, $id:literal, $case:ident) => {
        pub(super) const $constant: Scenario = Scenario::new(
            $id,
            1,
            ScenarioKind::Runtime {
                control: Control::Human,
                world: RuntimeWorld::Custom,
                setup: |m| setup(m, Case::$case),
            },
        );
    };
}
scenario!(SHARED, "religion-diplomacy-shared", Shared);
scenario!(DIFFERENT, "religion-diplomacy-different", Different);
scenario!(TOLERANT, "religion-diplomacy-tolerant", Tolerant);
scenario!(WAR, "religion-diplomacy-war", War);

fn setup(map: GeneratedMap, case: Case) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut g, ids) = super::religion_strategy::base(map);
    let trust = match case {
        Case::Shared => 980,
        Case::Different | Case::Tolerant => 1020,
        // Native peaceful love gain and decay bring this to -2,290 before
        // planning: exactly neutral's boundary, within Devout's +40 window.
        Case::War => -2540,
    };
    let mut w = WorldBuilder::new(&mut g);
    w.ai_control(2, false);
    w.ai_control(3, false);
    w.start_at(30, -2500);
    w.diplomacy(HUMAN, 1, Pact::Peace, 1, [trust, trust]);
    w.embassy(HUMAN, 1);
    w.embassy(1, HUMAN);
    w.treasury(HUMAN, 10000);
    w.finish();
    assert!(g.found_religion(HUMAN, ids[0], 1));
    assert!(g.found_religion(1, ids[1], 3));
    // Both declarations are legal in the identical city population. Only the
    // national declaration/policy changes between each pair of campaigns.
    WorldBuilder::new(&mut g).city_religion(ids[1], &[(1, 150), (3, 300)], Some(3));
    assert!(g.declare_faith(HUMAN, Some(1), FaithPolicy::Devout));
    let mut control = g.clone();
    let faith = if matches!(case, Case::Shared) { 1 } else { 3 };
    assert!(g.declare_faith(1, Some(faith), FaithPolicy::Devout));
    if matches!(case, Case::Tolerant) {
        assert!(control.declare_faith(1, Some(3), FaithPolicy::Tolerant));
    } else {
        // Lock the undeclared control's policy for the same ten-turn cooldown,
        // so its ordinary AI cannot erase the experimental condition.
        assert!(control.declare_faith(1, None, FaithPolicy::Devout));
    }
    let initial = json!({"main":g.observe_diplomacy(),"control":control.observe_diplomacy()});
    (
        g,
        Box::new(Audit {
            case,
            control,
            initial,
            accepted: [false; 2],
            result: false,
            events: 0,
            dropped: false,
            last_sequence: None,
            diagnostics: Value::Null,
        }),
    )
}
struct Audit {
    case: Case,
    control: Simulation,
    initial: Value,
    accepted: [bool; 2],
    result: bool,
    events: usize,
    dropped: bool,
    last_sequence: Option<u64>,
    diagnostics: Value,
}
impl ScenarioRun for Audit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["diplomacy.*", "religion.*"]
    }
    fn comparison_game(&mut self) -> Option<&mut Simulation> {
        Some(&mut self.control)
    }
    fn before_turn(&mut self, g: &mut Simulation, completed: u32) {
        if completed != 0 || matches!(self.case, Case::War) {
            return;
        }
        let mut offer = TreatyOffer {
            pact: Some(Pact::Alliance),
            ..Default::default()
        };
        offer.terms[0] = offer.terms[0].set_gold(5000);
        // Use the human treaty command and the AI's actual acceptance policy;
        // never force an alliance via execute_offer/sign_pact.
        self.accepted = [
            g.propose_treaty(1, offer.clone()),
            self.control.propose_treaty(1, offer),
        ];
    }
    fn observe_turn(&mut self, g: &Simulation, completed: u32) -> Value {
        self.dropped |= g.trace.dropped() > 0;
        for event in g.trace.pending_events() {
            if self.last_sequence.is_some_and(|s| event.sequence <= s) {
                continue;
            }
            self.last_sequence = Some(event.sequence);
            if event.kind == "diplomacy.transition"
                && ((event.payload["actor"] == 1 && event.payload["other"] == HUMAN)
                    || (event.payload["actor"] == HUMAN && event.payload["other"] == 1))
                && (event.payload["action"] == "war_plan_started"
                    || event.payload["current"] == "Alliance")
            {
                self.events += 1;
            }
        }
        let main = g
            .observe_diplomacy()
            .into_iter()
            .find(|p| p.players == [HUMAN, 1])
            .unwrap();
        let control = self
            .control
            .observe_diplomacy()
            .into_iter()
            .find(|p| p.players == [HUMAN, 1])
            .unwrap();
        self.result = match self.case {
            Case::Shared => {
                self.accepted == [true, false]
                    && main.pact == Some(Pact::Alliance)
                    && control.pact == Some(Pact::Peace)
                    && main.religion_attitude == [4, 4]
                    && control.religion_attitude == [0, 0]
            }
            Case::Different | Case::Tolerant => {
                self.accepted == [false, true]
                    && main.pact == Some(Pact::Peace)
                    && control.pact == Some(Pact::Alliance)
                    && main.religion_attitude == [-4, -4]
                    && control.religion_attitude
                        == if matches!(self.case, Case::Tolerant) {
                            [-4, 0]
                        } else {
                            [0, 0]
                        }
            }
            Case::War => {
                main.trust == control.trust
                    && main.war_plans[1]
                        .as_ref()
                        .is_some_and(|p| p.reason == "hatred" && p.countdown > 0)
                    && control.war_plans[1].is_none()
                    && main.pact == Some(Pact::Peace)
                    && control.pact == Some(Pact::Peace)
                    && main.religion_attitude == [-4, -4]
                    && control.religion_attitude == [0, 0]
            }
        };
        self.diagnostics = json!({"completed":completed,"case":format!("{:?}",self.case),"initial":self.initial,"accepted":self.accepted,"main":main,"control":control,"strategy":g.strategic_priorities(1),"events":self.events});
        self.diagnostics.clone()
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        vec![Check {
            name: match self.case {
                Case::Shared => "shared_faith_tips_alliance_acceptance",
                Case::Different => "different_faith_blocks_marginal_alliance",
                Case::Tolerant => "tolerant_policy_removes_directional_penalty",
                Case::War => "religious_tension_tips_preparation_without_instant_war",
            },
            passed: completed >= 1
                && self.result
                && !self.dropped
                && (!matches!(self.case, Case::Shared | Case::War) || self.events > 0),
            detail: format!("{}, dropped={}", self.diagnostics, self.dropped),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn faith_changes_diplomatic_decisions_in_controlled_campaigns() {
        let mut failures = Vec::new();
        for seed in [42, 7, 99] {
            for scenario in [SHARED, DIFFERENT, TOLERANT, WAR] {
                let ScenarioKind::Runtime { setup, .. } = scenario.kind else {
                    unreachable!()
                };
                let (mut g, mut audit) = setup(crate::mapgen::generate(seed));
                g.trace = crate::sim::trace::Collector::enabled(["*".into()].into(), 10000);
                audit.before_turn(&mut g, 0);
                g.simulate_turn();
                audit.comparison_game().unwrap().simulate_turn();
                audit.observe_turn(&g, 1);
                for check in audit.finish(1) {
                    if !check.passed {
                        failures.push(format!("seed {seed}, {}: {}", scenario.id, check.detail));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
