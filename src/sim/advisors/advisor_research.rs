//! Advisor selection and valuation of research goals.

use super::*;

impl Simulation {
    pub(in crate::sim) fn advisor_tech_want(&self, owner: usize, tech: Tech) -> f32 {
        if !self.players[owner].ai {
            return 0.0;
        }
        self.ai_tech_wants
            .get(&owner)
            .and_then(|wants| wants.get(tech.0))
            .copied()
            .unwrap_or(0.0)
    }

    pub(in crate::sim) fn clear_ai_tech_wants(&mut self, owner: usize) {
        self.ai_tech_wants
            .insert(owner, vec![0.0; Tech::all().count()]);
    }

    pub(in crate::sim) fn add_ai_tech_want(&mut self, owner: usize, tech: Tech, want: f32) {
        let wants = self
            .ai_tech_wants
            .entry(owner)
            .or_insert_with(|| vec![0.0; Tech::all().count()]);
        if let Some(value) = wants.get_mut(tech.0) {
            *value += want;
        }
    }

    pub(in crate::sim) fn record_ai_domestic_tech_wants(&mut self, owner: usize) {
        for tech in Tech::all() {
            if self.has_tech(owner, tech) {
                continue;
            }
            self.add_ai_tech_want(owner, tech, self.ferry_tech_want(owner, tech) as f32);
        }
        // Apply every contribution to the existing phase accumulator; grouping
        // cities first would change f32 rounding after government feedback.
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            for tech in Tech::all() {
                if !self.has_tech(owner, tech) {
                    self.add_ai_tech_want(
                        owner,
                        tech,
                        self.city_building_tech_want(&city, tech) as f32,
                    );
                }
            }
        }
    }

    pub(in crate::sim) fn record_ai_first_activity_tech_wants(&mut self, owner: usize) {
        if !self.players[owner].ai {
            return;
        }
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            for (tech, want) in self.counter_defender_tech_feedback(&city) {
                self.add_ai_tech_want(owner, tech, want);
            }
        }
    }

    pub(super) fn record_ai_military_tech_wants(&mut self, city: &City) {
        for (tech, want) in self.counter_defender_tech_feedback(city) {
            self.add_ai_tech_want(city.owner, tech, want);
        }
        for tech in Tech::all() {
            self.add_ai_tech_want(
                city.owner,
                tech,
                self.production_defender_tech_want(city, tech),
            );
        }
        for (tech, want) in self.offensive_advice(city).1 {
            self.add_ai_tech_want(city.owner, tech, want);
        }
    }

    /// Explicit preview for isolated advisor scenarios. Runtime research and
    /// diplomacy read the retained phase vector instead of recomputing advice.
    pub(in crate::sim) fn advisor_tech_wants(&self, owner: usize) -> Vec<f32> {
        let mut preview = self.clone();
        preview.clear_ai_tech_wants(owner);
        if let Some((tech, want)) = preview.government_research_want(owner) {
            preview.add_ai_tech_want(owner, tech, want as f32);
        }
        preview.record_ai_domestic_tech_wants(owner);
        let cities: Vec<_> = preview.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            preview.record_ai_military_tech_wants(&city);
        }
        preview.record_ai_tech_effect_wants(owner);
        preview.ai_tech_wants.remove(&owner).unwrap()
    }

    pub(in crate::sim) fn advisor_city_output(&self, city: &City, granted: Option<Tech>) -> i32 {
        self.advisor_city_output_exact(city, granted) as i32
    }

    pub(in crate::sim) fn advisor_city_output_exact(
        &self,
        city: &City,
        granted: Option<Tech>,
    ) -> f64 {
        let forecast = self.city_forecast_with_tech(city, granted);
        // advdata.h stores priorities as int: 18*1.2 truncates to21.
        let (science_weight, luxury_weight) =
            if self.players[city.owner].researching != Tech::FUTURE {
                (21, 1)
            } else {
                (1, TRADE_WEIGHT)
            };
        // daicity.c:dai_city_want stores adv_want after each compound
        // assignment, so retain source f32 rounding at those boundaries.
        let mut value = 0.0_f32;
        value += (FOOD_WEIGHT * forecast.food) as f32;
        if forecast.shields != 0 {
            value += (SHIELD_WEIGHT * forecast.shields) as f32;
            value -= (self.pollution_risk(city, forecast.shields) * 20) as f32;
        }
        value += (luxury_weight * forecast.luxury) as f32;
        value += (science_weight * forecast.gross_science) as f32;
        let gold = forecast.gross_gold - forecast.building_upkeep;
        let tax = self.players[city.owner].rates[1];
        if tax > 50 {
            value = (f64::from(value) + f64::from(gold * TRADE_WEIGHT * (tax - 40)) / 14.0) as f32;
        } else {
            value += (gold * TRADE_WEIGHT) as f32;
        }
        f64::from(value)
    }

    pub(super) fn city_tech_effect_want(&self, city: &City, tech: Tech) -> i32 {
        let baseline = self.advisor_city_output(city, None);
        let owner = city.owner;
        let direct = match tech {
            Tech::RAILROAD | Tech::TRADE | Tech::THE_CORPORATION => {
                self.advisor_city_output(city, Some(tech)) - baseline
            }
            Tech::MYSTICISM
                if city.has_building(Building::TEMPLE)
                    && self.players[owner].government != Government::Communism =>
            {
                TRADE_WEIGHT
            }
            _ => 0,
        };
        let mut want = direct * 14 / 8;
        for &building in &city.buildings {
            if building
                .data()
                .obsolete
                .contains(&(Requirement::Tech(tech.0), true))
            {
                let mut obsolete = city.clone();
                obsolete.buildings.retain(|&built| built != building);
                want += (self.advisor_city_output(&obsolete, Some(tech))
                    - self.advisor_city_output(city, Some(tech)))
                    * 14
                    / 8;
            }
        }
        want
    }

    pub(in crate::sim) fn record_ai_tech_effect_wants(&mut self, owner: usize) {
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        // daitech.c: effect feedback is the final contribution before selection.
        for tech in Tech::all() {
            if self.has_tech(owner, tech) {
                continue;
            }
            for city in &cities {
                self.add_ai_tech_want(owner, tech, self.city_tech_effect_want(city, tech) as f32);
            }
        }
    }

    pub(super) fn city_building_tech_want(&self, city: &City, tech: Tech) -> i32 {
        let baseline = self.advisor_city_output(city, None);
        let (food, gross_shields, _) = self.city_yields(city);
        let upkeep = self.city_unit_upkeep(city);
        let food_surplus = food - city.size * CITIZEN_FOOD_COST - upkeep.food;
        let shields = (gross_shields - upkeep.shields).max(0);
        let mut want = 0;
        for building in
            Building::all().filter(|building| building.data().genus == Genus::Improvement)
        {
            let data = building.data();
            if !data
                .requirements
                .contains(&(Requirement::Tech(tech.0), true))
                || self
                    .building_blocker_with_tech(city, building, Some(tech))
                    .is_some()
            {
                continue;
            }
            let mut improved = city.clone();
            improved.buildings.push(building);
            let mut benefit = self.advisor_city_output(&improved, None) - baseline;
            benefit += self.building_growth_value(city, &improved, food_surplus);
            benefit += self.defensive_building_effect_want(city, building) as i32;
            let cost = data.cost / (shields * 10 + 1);
            want += (benefit - cost).max(0) * 14 / 8;
        }
        want
    }

    pub(super) fn building_growth_value(
        &self,
        city: &City,
        improved: &City,
        food_surplus: i32,
    ) -> i32 {
        let mut benefit = 0;
        // Growth headroom matters when a city is about to reach its current limit.
        let limit = Self::city_size_limit(city);
        let new_limit = Self::city_size_limit(improved);
        if new_limit > limit && limit != i32::MAX {
            let increase = (new_limit - limit).min(8);
            benefit += increase * 4 / limit;
            if city.size >= limit {
                let stored = if city.food == Self::growth_box(city.size) {
                    city.food - Self::growth_box(city.size - 1)
                } else {
                    0
                };
                benefit += (food_surplus + stored) * (FOOD_WEIGHT * increase + 30);
            }
        }
        let retention = Self::food_retained(improved) - Self::food_retained(city);
        if retention > 0 {
            benefit +=
                8 + retention / 10 * food_surplus.max(0) + retention / 15 * (-food_surplus).max(1);
        }
        benefit
    }
}
