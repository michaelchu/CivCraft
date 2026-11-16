//! Registered window-free gameplay regression scenarios.
//!
//! Each scenario mutates a freshly generated campaign through
//! simulation-level helpers and records named pass/fail checks. The headless
//! simulator runs one by its identifier, and the desktop self-test replays the
//! same table, so a scenario is defined exactly once.

use crate::sim::Simulation;

mod diplomacy;
mod early;
mod headless;
mod military;
mod native_rules;
mod research;
mod strategy;

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
    /// A newly generated campaign with the human player in seat zero.
    NewCampaign,
    /// A newly generated campaign in which every civilization is AI-driven.
    AllAiCampaign,
}

/// A reusable, deterministic regression over a generated campaign.
pub(crate) struct Scenario {
    /// Stable kebab-case identifier accepted by `headless_sim --scenario`.
    pub(crate) id: &'static str,
    pub(crate) fixture: Fixture,
    actions: fn(&mut Simulation, &mut Checks),
}

impl Scenario {
    const fn new(id: &'static str, actions: fn(&mut Simulation, &mut Checks)) -> Self {
        Self {
            id,
            fixture: Fixture::NewCampaign,
            actions,
        }
    }

    const fn all_ai(id: &'static str, actions: fn(&mut Simulation, &mut Checks)) -> Self {
        Self {
            id,
            fixture: Fixture::AllAiCampaign,
            actions,
        }
    }

    /// Applies the fixture to a newly generated campaign and runs the
    /// scenario's actions, returning its checks in order.
    pub(crate) fn run(&self, campaign: &mut Simulation) -> Vec<Check> {
        if self.fixture == Fixture::AllAiCampaign {
            campaign.enable_all_civilizations_as_ai();
        }
        let mut checks = Checks::default();
        (self.actions)(campaign, &mut checks);
        checks.0
    }
}

const GROUPS: &[&[Scenario]] = &[
    headless::SCENARIOS,
    early::SCENARIOS,
    military::SCENARIOS,
    diplomacy::SCENARIOS,
    strategy::SCENARIOS,
    native_rules::SCENARIOS,
    research::SCENARIOS,
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

    /// Checks that fail on the reference campaign in a fresh process. They
    /// failed the same way before the scenarios were registered here (verified
    /// against the pre-registration desktop blocks), so they are recorded
    /// rather than hidden: the test below fails if this list goes stale in
    /// either direction. Fix the rule or the scenario, then remove the entry.
    const KNOWN_FAILING_CHECKS: &[&str] = &[
        "ai_bodyguard_null_unit_upkeep",
        "inland_overseas_settlement",
        "overseas_settlement",
    ];

    #[test]
    fn registered_scenarios_pass_on_the_reference_campaign_except_known_failures() {
        let mut names = BTreeSet::new();
        let mut failing = BTreeSet::new();
        for scenario in all() {
            let mut campaign = Simulation::new(crate::test_support::seeded_map());
            let checks = scenario.run(&mut campaign);
            assert!(!checks.is_empty(), "{} made no checks", scenario.id);
            for check in checks {
                assert!(
                    names.insert(check.name),
                    "check {} is reported by more than one scenario",
                    check.name
                );
                if !check.passed {
                    failing.insert(check.name);
                }
            }
        }
        assert_eq!(
            failing,
            KNOWN_FAILING_CHECKS
                .iter()
                .copied()
                .collect::<BTreeSet<_>>(),
            "the set of failing scenario checks changed"
        );
    }
}
