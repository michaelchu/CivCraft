//! CivCraft's ruleset-backed tuning for the Freeciv settler advisor.
//!
//! These are ported Freeciv AI algorithm values, not Civ III gameplay rules.
//! They live in the selected CivCraft ruleset so scenario or balance variants
//! can tune them without editing the algorithm.

use std::sync::OnceLock;

use super::{LoadError, RulesetData, required_parameter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettlementAiSettings {
    pub(crate) food_priority: i32,
    pub(crate) shield_priority: i32,
    pub(crate) trade_priority: i32,
    pub(crate) research_priority_percent: i32,
    pub(crate) no_research_priority: i32,
    pub(crate) growth_priority: i32,
    pub(crate) growth_potential_deemphasis: i32,
    pub(crate) growth_food_threshold: i32,
    pub(crate) perfection: i32,
    pub(crate) good_enough: i32,
    pub(crate) discount_period: i32,
    pub(crate) defense_emphasis: i32,
    pub(crate) naval_emphasis: i32,
    pub(crate) wonder_city_pop_founder_divisor: i32,
    pub(crate) wonder_city_other_founder_divisor: i32,
    pub(crate) expansionist_trait_baseline: i32,
    pub(crate) ferry_cost_divisor: i32,
    pub(crate) ferry_tech_want: i32,
}

impl SettlementAiSettings {
    pub(crate) fn campaign() -> &'static Self {
        static SETTINGS: OnceLock<SettlementAiSettings> = OnceLock::new();
        SETTINGS.get_or_init(|| {
            Self::from_ruleset(RulesetData::campaign_default())
                .unwrap_or_else(|error| panic!("invalid settlement AI settings: {error}"))
        })
    }

    fn from_ruleset(rules: &RulesetData) -> Result<Self, LoadError> {
        let integer = |key: &str| -> Result<i32, LoadError> {
            let parameter =
                required_parameter(rules, "game.ruleset", "civcraft_settlement_ai", key)?;
            let value = parameter
                .integer_value()
                .map_err(|error| LoadError::new(format!("invalid settlement AI setting: {error}")))?
                .ok_or_else(|| {
                    LoadError::new(format!(
                        "game.ruleset [civcraft_settlement_ai].{key} has no integer value"
                    ))
                })?;
            i32::try_from(value).map_err(|_| {
                LoadError::new(format!(
                    "game.ruleset [civcraft_settlement_ai].{key} is outside the supported range"
                ))
            })
        };
        let settings = Self {
            food_priority: integer("food_priority")?,
            shield_priority: integer("shield_priority")?,
            trade_priority: integer("trade_priority")?,
            research_priority_percent: integer("research_priority_percent")?,
            no_research_priority: integer("no_research_priority")?,
            growth_priority: integer("growth_priority")?,
            growth_potential_deemphasis: integer("growth_potential_deemphasis")?,
            growth_food_threshold: integer("growth_food_threshold")?,
            perfection: integer("perfection")?,
            good_enough: integer("good_enough")?,
            discount_period: integer("discount_period")?,
            defense_emphasis: integer("defense_emphasis")?,
            naval_emphasis: integer("naval_emphasis")?,
            wonder_city_pop_founder_divisor: integer("wonder_city_pop_founder_divisor")?,
            wonder_city_other_founder_divisor: integer("wonder_city_other_founder_divisor")?,
            expansionist_trait_baseline: integer("expansionist_trait_baseline")?,
            ferry_cost_divisor: integer("ferry_cost_divisor")?,
            ferry_tech_want: integer("ferry_tech_want")?,
        };
        if settings.growth_potential_deemphasis <= 0
            || settings.discount_period <= 0
            || settings.wonder_city_pop_founder_divisor <= 0
            || settings.wonder_city_other_founder_divisor <= 0
            || settings.expansionist_trait_baseline <= 0
            || settings.ferry_cost_divisor <= 0
        {
            return Err(LoadError::new(
                "settlement AI divisors must be greater than zero",
            ));
        }
        Ok(settings)
    }
}
