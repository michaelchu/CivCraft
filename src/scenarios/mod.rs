//! Registered deterministic gameplay scenarios.
//!
//! Each scenario mutates a freshly generated campaign through
//! simulation-level helpers and records named pass/fail checks. The headless
//! simulator runs them by identifier, and scripted scenarios also run in unit tests.

use crate::{sim::Simulation, world::GeneratedMap};

mod catalog;
pub(crate) use catalog::{all, find};

#[cfg(any(test, feature = "scenarios"))]
mod definitions;

#[cfg(test)]
pub(crate) use definitions::{
    funding_campaign_fixture, space_victory_fixture, strategy_pair_fixture,
};

/// One run of a scenario: actions, observations and final checks.
/// Preparatory and ordinary turns are advanced by the runner.
pub(crate) trait ScenarioRun {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn has_actions(&self) -> bool {
        false
    }
    fn execute_actions(&mut self, _game: &mut Simulation) -> Vec<Check> {
        vec![]
    }
    /// Normal turns requested inside the traced action boundary.
    fn action_turns(&self) -> u32 {
        0
    }
    fn complete_actions(&mut self, _game: &mut Simulation) -> Vec<Check> {
        vec![]
    }
    fn records_turns(&self) -> bool {
        true
    }
    /// Additional campaigns advanced by the runner before observing each turn.
    /// Scenarios lend their state here; observers never advance campaign clocks.
    fn companion_campaigns(&mut self) -> &mut [Simulation] {
        &mut []
    }
    fn observe_turn(&mut self, _game: &Simulation, _completed_turns: u32) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        vec![]
    }
}

pub(crate) type ScenarioSetup = fn(GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>);

/// One named assertion made by a scenario.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct Check {
    pub(crate) name: &'static str,
    pub(crate) passed: bool,
    pub(crate) detail: String,
}

/// Collects a scenario's checks in the order they were made.
#[derive(Default)]
pub(crate) struct Checks(Vec<Check>);

/// Which seats run under AI control before a scenario's actions or turns.
#[derive(Clone, Copy)]
#[cfg_attr(not(any(test, feature = "scenarios")), allow(dead_code))]
pub(crate) enum Control {
    Human,
    AllAi,
}

/// Whether a runtime retains the reference generator's world or replaces it.
#[derive(Clone, Copy)]
#[cfg_attr(not(any(test, feature = "scenarios")), allow(dead_code))]
pub(crate) enum RuntimeWorld {
    Reference,
    Custom,
}

pub(crate) type ScenarioActions = fn(&mut Simulation, &mut Checks);

/// Valid execution forms, with their setup requirements kept together.
#[derive(Clone, Copy)]
#[cfg_attr(not(any(test, feature = "scenarios")), allow(dead_code))]
pub(crate) enum ScenarioKind {
    /// Generate the map using the caller's options and run every seat as AI.
    Generated,
    /// Defer a command script until the runner attaches tracing.
    Actions {
        control: Control,
        execute: ScenarioActions,
    },
    /// Prepare a world and a runtime that can request actions or observe turns.
    Runtime {
        control: Control,
        world: RuntimeWorld,
        setup: ScenarioSetup,
    },
}

/// A named deterministic scenario. Versions are explicit report contracts,
/// independent of the execution form or choice of starting world.
pub(crate) struct Scenario {
    pub(crate) id: &'static str,
    pub(crate) version: u16,
    kind: ScenarioKind,
}

impl Scenario {
    pub(crate) const fn new(id: &'static str, version: u16, kind: ScenarioKind) -> Self {
        Self { id, version, kind }
    }

    pub(crate) fn accepts_world_options(&self) -> bool {
        matches!(self.kind, ScenarioKind::Generated)
    }

    pub(crate) fn uses_custom_world(&self) -> bool {
        matches!(
            self.kind,
            ScenarioKind::Runtime {
                world: RuntimeWorld::Custom,
                ..
            }
        )
    }

    /// Prepare the world; actions wait until the runner attaches tracing.
    pub(crate) fn start(&self, map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
        let (mut game, runtime, control): (Simulation, Box<dyn ScenarioRun>, Control) =
            match self.kind {
                ScenarioKind::Generated => (
                    Simulation::new(map),
                    Box::new(Scripted { actions: None }),
                    Control::AllAi,
                ),
                ScenarioKind::Actions { control, execute } => (
                    Simulation::new(map),
                    Box::new(Scripted {
                        actions: Some(execute),
                    }),
                    control,
                ),
                ScenarioKind::Runtime { control, setup, .. } => {
                    let (game, runtime) = setup(map);
                    (game, runtime, control)
                }
            };
        if matches!(control, Control::AllAi) {
            game.enable_all_civilizations_as_ai();
        }
        (game, runtime)
    }
}

/// Scripted actions use the same lifecycle as scenarios that observe turns.
struct Scripted {
    actions: Option<ScenarioActions>,
}

impl ScenarioRun for Scripted {
    fn has_actions(&self) -> bool {
        self.actions.is_some()
    }
    fn execute_actions(&mut self, game: &mut Simulation) -> Vec<Check> {
        let Some(actions) = self.actions.take() else {
            return vec![];
        };
        let mut checks = Checks::default();
        actions(game, &mut checks);
        game.enable_all_civilizations_as_ai();
        checks.0
    }
    fn records_turns(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn scenario_and_check_names_are_unique() {
        let ids: Vec<_> = all().map(|scenario| scenario.id).collect();
        assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
        for id in ids {
            assert!(
                id.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
                "{id} is not kebab-case"
            );
        }
    }

    #[test]
    fn scripted_scenarios_pass_on_the_reference_campaign() {
        let mut names = BTreeSet::new();
        for scenario in all() {
            let (mut campaign, mut runtime) = scenario.start(crate::test_support::seeded_map());
            if !runtime.has_actions() {
                continue;
            }
            let mut checks = runtime.execute_actions(&mut campaign);
            for _ in 0..runtime.action_turns() {
                let before = campaign.turn();
                campaign.simulate_turn();
                if campaign.turn() != before {
                    for companion in runtime.companion_campaigns() {
                        if companion.game_over().is_none() {
                            companion.simulate_turn();
                        }
                    }
                }
            }
            checks.extend(runtime.complete_actions(&mut campaign));
            assert!(!checks.is_empty(), "{} made no checks", scenario.id);
            for check in checks {
                assert!(
                    names.insert(check.name),
                    "check {} is reported by more than one scenario",
                    check.name
                );
                assert!(
                    check.passed,
                    "{}: {}: {}",
                    scenario.id, check.name, check.detail
                );
            }
        }
    }
}
