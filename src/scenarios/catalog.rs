//! One catalog for scripted actions and normal-turn gameplay scenarios.
//! Existing identifiers retain their registration order.

use super::*;

pub(super) const SCENARIOS: &[Scenario] = &[
    super::unit_recovery::SCENARIO,
    super::settlement_ferry::SCENARIO,
    super::state_presets::SCENARIO,
    super::campaign_readiness::SCENARIO,
    super::city_governor::GOVERNED,
    super::city_governor::CONTROL,
    super::city_governor::FOOD_SHOCK,
    super::city_governor::FOOD_SHOCK_CONTROL,
    super::city_governor::UNREST_SHOCK,
    super::city_governor::UNREST_SHOCK_CONTROL,
    super::diplomatic_needs::AID,
    super::diplomatic_needs::MAPS,
    super::diplomatic_needs::RECOVERY,
    super::harbor_trade::SCENARIO,
    super::naval::ai_assault::SCENARIO,
    super::naval::ship_combat::SCENARIO,
    super::expansion_balance::SCENARIO,
    super::lifecycle_history::SCENARIO,
    super::great_leader::SCENARIO,
    super::research_history::SCENARIO,
    super::combat_matrix::SCENARIO,
    super::ai_paradrops::SCENARIO,
    super::diplomacy_parity::SCENARIO,
    super::peaceful_first_contact::SCENARIO,
    super::proactive_trade::SCENARIO,
    super::shared_strategy::SCENARIO,
    super::wartime_funding::SCENARIO,
    super::relationship_memory::SCENARIO,
    super::culture_history::SCENARIO,
];
