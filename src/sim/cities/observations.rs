//! Observations of actual city processing; these do not define expected rules.
use crate::sim::trace::Payload;
use serde::Serialize;
#[derive(Serialize)]
pub(crate) struct GrowthApplied {
    pub food_before: i32,
    pub food_after: i32,
    pub population_before: i32,
    pub population_after: i32,
    pub applied_surplus: i32,
    pub size_limit: i32,
    pub outcome: &'static str,
}
impl Payload for GrowthApplied {
    const KIND: &'static str = "cities.growth_applied";
}
#[derive(Serialize)]
pub(crate) struct ProductionCompleted {
    pub item: String,
    pub shields_before: i32,
    pub shields_after: i32,
}
impl Payload for ProductionCompleted {
    const KIND: &'static str = "production.completed";
}
