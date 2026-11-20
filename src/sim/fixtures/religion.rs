//! Explicit influence setup for short normal-turn religion scenarios.
use super::*;
use crate::sim::religion::{CityFaith, ReligionId};
use std::collections::BTreeMap;
impl WorldBuilder<'_> {
    pub(crate) fn city_religion(
        &mut self,
        city: u32,
        influence: &[(ReligionId, i32)],
        dominant: Option<ReligionId>,
    ) {
        assert!(self.game.city_by_id(city).is_some());
        let values: BTreeMap<_, _> = influence.iter().copied().collect();
        self.game
            .religion
            .cities
            .insert(city, CityFaith::fixture(values, dominant));
    }
}
