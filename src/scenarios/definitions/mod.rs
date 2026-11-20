//! Optional registered catalog, definitions and construction helpers.

use super::*;

mod ai_paradrops;
mod border_offensive;
mod campaign_readiness;
mod city_governor;
mod combat_matrix;
mod competing_partners;
mod culture_border_cases;
mod culture_border_growth;
mod culture_borders;
mod culture_empire_victory;
mod culture_history;
mod culture_wonders;
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
mod religion;
mod religion_campaigns;
mod religion_diplomacy;
mod religion_effectiveness;
mod religion_strategy;
mod research_history;
mod settlement_ferry;
mod shared_strategy;
mod space_victory;
mod state_presets;
mod trait_campaign;
mod unit_recovery;
mod upkeep_shock;
mod vassalage;
mod vassalage_ai;
mod wartime_funding;

#[cfg(test)]
pub(crate) use shared_strategy::setup as strategy_pair_fixture;
#[cfg(test)]
pub(crate) use space_victory::setup as space_victory_fixture;
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
    religion_effectiveness::CULTURE,
    religion_effectiveness::SATURATED,
    religion_diplomacy::SHARED,
    religion_diplomacy::DIFFERENT,
    religion_diplomacy::TOLERANT,
    religion_diplomacy::WAR,
    religion_campaigns::RACE,
    religion_campaigns::CLAIMED,
    religion_campaigns::RECOVERY,
    religion_campaigns::SPREAD,
    religion_strategy::GOALS,
    religion_strategy::COALITION,
    religion::UNLOCKS,
    religion::SPREAD,
    religion::COMPETITION,
    religion::AI,
    vassalage::CAPITULATION,
    vassalage::VOLUNTARY,
    vassalage_ai::CAPITULATION,
    vassalage_ai::VOLUNTARY,
    vassalage_ai::WAR_PEACE,
    vassalage_ai::INDEPENDENCE,
    unit_recovery::SCENARIO,
    settlement_ferry::SCENARIO,
    state_presets::SCENARIO,
    campaign_readiness::SCENARIO,
    border_offensive::SCENARIO,
    border_offensive::REINFORCED,
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
    naval::ai_assault::SECOND_WAVE,
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
    trait_campaign::SCENARIO,
    space_victory::SCENARIO,
    space_victory::INDUSTRIAL_LOSS,
    competing_partners::SCENARIO,
    upkeep_shock::SCENARIO,
    wartime_funding::SCENARIO,
    relationship_memory::SCENARIO,
    culture_history::SCENARIO,
    culture_borders::SCENARIO,
    culture_border_growth::SCENARIO,
    culture_border_cases::CONTESTS,
    culture_border_cases::LAND_CLAIMS,
    culture_border_cases::CITY_LOSS,
    culture_wonders::SCENARIO,
    culture_empire_victory::SCENARIO,
];
