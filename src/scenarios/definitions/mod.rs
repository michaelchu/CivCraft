//! Optional registered catalog, definitions and construction helpers.

use super::*;

mod ai_paradrops;
mod border_offensive;
mod campaign_readiness;
mod city_governor;
mod combat_matrix;
mod culture_history;
mod diplomacy_parity;
mod diplomatic_needs;
mod expansion_balance;
mod great_leader;
mod harbor_trade;
mod lifecycle_history;
mod naval;
mod peaceful_first_contact;
mod proactive_trade;
mod relationship_memory;
mod research_history;
mod settlement_ferry;
mod shared_strategy;
mod state_presets;
mod unit_recovery;
mod wartime_funding;

#[cfg(test)]
pub(crate) use shared_strategy::setup as strategy_pair_fixture;
#[cfg(test)]
pub(crate) use wartime_funding::setup as funding_campaign_fixture;

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

pub(super) const SCENARIOS: &[Scenario] = &[
    unit_recovery::SCENARIO,
    settlement_ferry::SCENARIO,
    state_presets::SCENARIO,
    campaign_readiness::SCENARIO,
    border_offensive::SCENARIO,
    city_governor::GOVERNED,
    city_governor::CONTROL,
    city_governor::FOOD_SHOCK,
    city_governor::FOOD_SHOCK_CONTROL,
    city_governor::UNREST_SHOCK,
    city_governor::UNREST_SHOCK_CONTROL,
    diplomatic_needs::AID,
    diplomatic_needs::MAPS,
    diplomatic_needs::RECOVERY,
    harbor_trade::SCENARIO,
    naval::ai_assault::SCENARIO,
    naval::ship_combat::SCENARIO,
    expansion_balance::SCENARIO,
    lifecycle_history::SCENARIO,
    great_leader::SCENARIO,
    research_history::SCENARIO,
    combat_matrix::SCENARIO,
    ai_paradrops::SCENARIO,
    diplomacy_parity::SCENARIO,
    peaceful_first_contact::SCENARIO,
    proactive_trade::SCENARIO,
    shared_strategy::SCENARIO,
    wartime_funding::SCENARIO,
    relationship_memory::SCENARIO,
    culture_history::SCENARIO,
];
