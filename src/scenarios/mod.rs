//! Registered deterministic gameplay scenarios.
//!
//! Each scenario mutates a freshly generated campaign through
//! simulation-level helpers and records named pass/fail checks. The headless
//! simulator runs them by identifier, and scripted scenarios also run in unit tests.

use crate::{sim::Simulation, world::GeneratedMap};

#[cfg(any(test, feature = "scenarios"))]
mod ai_paradrops;
#[cfg(any(test, feature = "scenarios"))]
mod campaign_readiness;
#[cfg(any(test, feature = "scenarios"))]
mod catalog;
#[cfg(any(test, feature = "scenarios"))]
mod city_governor;
#[cfg(any(test, feature = "scenarios"))]
mod combat_matrix;
#[cfg(any(test, feature = "scenarios"))]
mod culture_history;
#[cfg(any(test, feature = "scenarios"))]
mod diplomacy_parity;
#[cfg(any(test, feature = "scenarios"))]
mod diplomatic_needs;
#[cfg(any(test, feature = "scenarios"))]
mod expansion_balance;
#[cfg(any(test, feature = "scenarios"))]
mod great_leader;
#[cfg(any(test, feature = "scenarios"))]
mod harbor_trade;
#[cfg(any(test, feature = "scenarios"))]
mod lifecycle_history;
#[cfg(any(test, feature = "scenarios"))]
mod naval;
#[cfg(any(test, feature = "scenarios"))]
mod peaceful_first_contact;
#[cfg(any(test, feature = "scenarios"))]
mod proactive_trade;
#[cfg(any(test, feature = "scenarios"))]
mod relationship_memory;
#[cfg(any(test, feature = "scenarios"))]
mod research_history;
#[cfg(any(test, feature = "scenarios"))]
mod settlement_ferry;
#[cfg(any(test, feature = "scenarios"))]
mod shared_strategy;
#[cfg(any(test, feature = "scenarios"))]
mod state_presets;
#[cfg(any(test, feature = "scenarios"))]
mod unit_recovery;
#[cfg(any(test, feature = "scenarios"))]
mod wartime_funding;

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

#[cfg(any(test, feature = "scenarios"))]
impl Checks {
    pub(crate) fn push(&mut self, (name, passed, detail): (&'static str, bool, String)) {
        self.0.push(Check {
            name,
            passed,
            detail,
        });
    }

    pub(crate) fn extend<I>(&mut self, checks: I)
    where
        I: IntoIterator<Item = (&'static str, bool, String)>,
    {
        for check in checks {
            self.push(check);
        }
    }
}

/// The campaign state a scenario expects before its actions run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fixture {
    /// A campaign generated with the caller's options, with all seats AI-driven.
    Generated,
    /// A newly generated campaign with the human player in seat zero.
    #[cfg_attr(not(any(test, feature = "scenarios")), allow(dead_code))]
    NewCampaign,
    /// A newly generated campaign in which every civilization is AI-driven.
    #[cfg_attr(not(any(test, feature = "scenarios")), allow(dead_code))]
    AllAiCampaign,
}

/// A named deterministic scenario over a generated campaign.
pub(crate) struct Scenario {
    /// Stable kebab-case identifier accepted by `headless_sim --scenario`.
    pub(crate) id: &'static str,
    pub(crate) fixture: Fixture,
    actions: Option<fn(&mut Simulation, &mut Checks)>,
    pub(crate) campaign: Option<ScenarioSetup>,
    pub(crate) version: u16,
    pub(crate) custom_world: bool,
}

impl Scenario {
    #[cfg(any(test, feature = "scenarios"))]
    const fn new(id: &'static str, actions: fn(&mut Simulation, &mut Checks)) -> Self {
        Self {
            id,
            fixture: Fixture::NewCampaign,
            actions: Some(actions),
            campaign: None,
            version: 1,
            custom_world: false,
        }
    }

    #[cfg(any(test, feature = "scenarios"))]
    const fn all_ai(id: &'static str, actions: fn(&mut Simulation, &mut Checks)) -> Self {
        Self {
            id,
            fixture: Fixture::AllAiCampaign,
            actions: Some(actions),
            campaign: None,
            version: 1,
            custom_world: false,
        }
    }

    #[cfg(any(test, feature = "scenarios"))]
    pub(crate) const fn campaign(id: &'static str, setup: ScenarioSetup) -> Self {
        Self {
            id,
            fixture: Fixture::AllAiCampaign,
            actions: None,
            campaign: Some(setup),
            version: 2,
            custom_world: true,
        }
    }

    #[cfg(any(test, feature = "scenarios"))]
    pub(crate) const fn human_campaign(id: &'static str, setup: ScenarioSetup) -> Self {
        Self {
            fixture: Fixture::NewCampaign,
            ..Self::campaign(id, setup)
        }
    }

    /// Scripted actions may request normal turns from the runner before checks finish.
    #[cfg(any(test, feature = "scenarios"))]
    const fn scripted_runtime(id: &'static str, setup: ScenarioSetup) -> Self {
        Self {
            fixture: Fixture::NewCampaign,
            version: 1,
            custom_world: false,
            ..Self::campaign(id, setup)
        }
    }

    /// Prepare a fresh scenario runtime; actions wait until tracing is attached.
    pub(crate) fn start(&'static self, map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
        let (mut game, runtime) = if let Some(setup) = self.campaign {
            setup(map)
        } else {
            (
                Simulation::new(map),
                Box::new(Scripted {
                    definition: self,
                    pending: self.has_actions(),
                }) as Box<dyn ScenarioRun>,
            )
        };
        self.prepare(&mut game);
        (game, runtime)
    }

    pub(crate) fn prepare(&self, campaign: &mut Simulation) {
        if matches!(self.fixture, Fixture::Generated | Fixture::AllAiCampaign) {
            campaign.enable_all_civilizations_as_ai();
        }
    }

    pub(crate) fn has_actions(&self) -> bool {
        self.actions.is_some()
    }

    /// Runs scenario actions after the campaign and its observers are ready.
    pub(crate) fn run(&self, campaign: &mut Simulation) -> Vec<Check> {
        let mut checks = Checks::default();
        if let Some(actions) = self.actions {
            actions(campaign, &mut checks);
        }
        checks.0
    }
}

/// Scripted actions use the same lifecycle as scenarios that observe turns.
struct Scripted {
    definition: &'static Scenario,
    pending: bool,
}

impl ScenarioRun for Scripted {
    fn has_actions(&self) -> bool {
        self.pending
    }
    fn execute_actions(&mut self, game: &mut Simulation) -> Vec<Check> {
        if !self.pending {
            return vec![];
        }
        self.pending = false;
        let checks = self.definition.run(game);
        game.enable_all_civilizations_as_ai();
        checks
    }
    fn records_turns(&self) -> bool {
        false
    }
}

const GENERATED: &[Scenario] = &[Scenario {
    id: "generated-ai",
    fixture: Fixture::Generated,
    actions: None,
    campaign: None,
    version: 1,
    custom_world: false,
}];

const GROUPS: &[&[Scenario]] = &[
    GENERATED,
    #[cfg(any(test, feature = "scenarios"))]
    catalog::SCENARIOS,
];

/// Every registered scenario, in registration order.
pub(crate) fn all() -> impl Iterator<Item = &'static Scenario> {
    GROUPS.iter().flat_map(|group| group.iter())
}

/// Looks up a scenario by its identifier.
pub(crate) fn find(id: &str) -> Option<&'static Scenario> {
    all().find(|scenario| scenario.id == id)
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
                campaign.simulate_turn();
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
