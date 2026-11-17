//! Registered window-free gameplay regression scenarios.
//!
//! Each scenario mutates a freshly generated campaign through
//! simulation-level helpers and records named pass/fail checks. The headless
//! simulator runs one by its identifier, and a unit test runs them all.

use crate::sim::Simulation;

#[cfg(any(test, feature = "scenarios"))]
mod headless;

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

/// A reusable, deterministic regression over a generated campaign.
pub(crate) struct Scenario {
    /// Stable kebab-case identifier accepted by `headless_sim --scenario`.
    pub(crate) id: &'static str,
    pub(crate) fixture: Fixture,
    actions: Option<fn(&mut Simulation, &mut Checks)>,
}

impl Scenario {
    #[cfg(any(test, feature = "scenarios"))]
    const fn new(id: &'static str, actions: fn(&mut Simulation, &mut Checks)) -> Self {
        Self {
            id,
            fixture: Fixture::NewCampaign,
            actions: Some(actions),
        }
    }

    #[cfg(any(test, feature = "scenarios"))]
    const fn all_ai(id: &'static str, actions: fn(&mut Simulation, &mut Checks)) -> Self {
        Self {
            id,
            fixture: Fixture::AllAiCampaign,
            actions: Some(actions),
        }
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

const GENERATED: &[Scenario] = &[Scenario {
    id: "generated-ai",
    fixture: Fixture::Generated,
    actions: None,
}];

const GROUPS: &[&[Scenario]] = &[
    GENERATED,
    #[cfg(any(test, feature = "scenarios"))]
    headless::SCENARIOS,
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
    fn registered_scenarios_pass_on_the_reference_campaign() {
        let mut names = BTreeSet::new();
        for scenario in all().filter(|scenario| scenario.has_actions()) {
            let mut campaign = Simulation::new(crate::test_support::seeded_map());
            scenario.prepare(&mut campaign);
            let checks = scenario.run(&mut campaign);
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
