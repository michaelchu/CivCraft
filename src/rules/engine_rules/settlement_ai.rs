//! CivCraft's ruleset-backed tuning for the Freeciv settler advisor.
//!
//! These include ported Freeciv AI values and explicit CivCraft balance tuning.
//! They live in the selected CivCraft ruleset so scenario or balance variants
//! can tune them without editing the algorithm.

use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettlementAiSettings {
    pub(crate) food_priority: i32,
    pub(crate) shield_priority: i32,
    pub(crate) trade_priority: i32,
    pub(crate) research_priority_percent: i32,
    pub(crate) no_research_priority: i32,
    pub(crate) growth_priority: i32,
    pub(crate) growth_potential_deemphasis: i32,
    pub(crate) preferred_land_tiles_per_city: i32,
    pub(crate) expansion_cost_priority: i32,
    pub(crate) free_expansion_cities: i32,
    pub(crate) growth_food_threshold: i32,
    pub(crate) perfection: i32,
    pub(crate) good_enough: i32,
    pub(crate) discount_period: i32,
    pub(crate) defense_emphasis: i32,
    pub(crate) naval_emphasis: i32,
    pub(crate) wonder_city_min_cities: i32,
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
            let native = &crate::rules::native::settlements().ai;
            assert!(native.growth_potential_deemphasis > 0);
            assert!(native.preferred_land_tiles_per_city > 0);
            assert!(native.expansion_cost_priority >= 0);
            assert!(native.free_expansion_cities >= 0);
            assert!(native.discount_period > 0);
            assert!(native.wonder_city_min_cities >= 0);
            assert!(native.wonder_city_pop_founder_divisor > 0);
            assert!(native.wonder_city_other_founder_divisor > 0);
            assert!(native.expansionist_trait_baseline > 0);
            assert!(native.ferry_cost_divisor > 0);
            Self {
                food_priority: native.food_priority,
                shield_priority: native.shield_priority,
                trade_priority: native.trade_priority,
                research_priority_percent: native.research_priority_percent,
                no_research_priority: native.no_research_priority,
                growth_priority: native.growth_priority,
                growth_potential_deemphasis: native.growth_potential_deemphasis,
                preferred_land_tiles_per_city: native.preferred_land_tiles_per_city,
                expansion_cost_priority: native.expansion_cost_priority,
                free_expansion_cities: native.free_expansion_cities,
                growth_food_threshold: native.growth_food_threshold,
                perfection: native.perfection,
                good_enough: native.good_enough,
                discount_period: native.discount_period,
                defense_emphasis: native.defense_emphasis,
                naval_emphasis: native.naval_emphasis,
                wonder_city_min_cities: native.wonder_city_min_cities,
                wonder_city_pop_founder_divisor: native.wonder_city_pop_founder_divisor,
                wonder_city_other_founder_divisor: native.wonder_city_other_founder_divisor,
                expansionist_trait_baseline: native.expansionist_trait_baseline,
                ferry_cost_divisor: native.ferry_cost_divisor,
                ferry_tech_want: native.ferry_tech_want,
            }
        })
    }
}
