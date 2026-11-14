//! Advisor selection and valuation of research goals.

use super::super::action_rules::SimulationRequirementContext;
use super::super::content_adapter::{FreecivTechBinding, bind_freeciv_technology};
use super::*;
use crate::rules::freeciv::{
    EffectRecord, Requirement as FreecivRequirement, RequirementContext, RequirementRange,
    RequirementValue, RulesetData,
};

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
        if !self.has_tech(owner, Tech::MAP_MAKING) {
            self.add_ai_tech_want(
                owner,
                Tech::MAP_MAKING,
                self.ferry_tech_want(owner, Tech::MAP_MAKING) as f32,
            );
        }
        // Apply every contribution to the existing phase accumulator; grouping
        // cities first would change f32 rounding after government feedback.
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        for city in cities {
            let baseline = self.advisor_city_output(&city, None);
            let (food, gross_shields, _) = self.city_yields(&city);
            let upkeep = self.city_unit_upkeep(&city);
            let food_surplus = food - city.size * Self::citizen_food_cost() - upkeep.food;
            let shields = (gross_shields - upkeep.shields).max(0);
            for &tech in improvement_unlock_techs() {
                if !self.has_tech(owner, tech) {
                    self.add_ai_tech_want(
                        owner,
                        tech,
                        self.city_building_tech_want_with_baseline(
                            &city,
                            tech,
                            baseline,
                            food_surplus,
                            shields,
                        ) as f32,
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
        for &tech in defender_techs() {
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

    #[cfg(test)]
    pub(super) fn city_tech_effect_want(&self, city: &City, tech: Tech) -> i32 {
        let obsoletes_building = city.buildings.iter().any(|building| {
            building
                .data()
                .obsolete
                .contains(&(Requirement::Tech(tech.0), true))
        });
        if !city_effect_techs().contains(&tech) && !obsoletes_building {
            return 0;
        }
        let baseline = self.advisor_city_output(city, None);
        self.city_tech_effect_want_with_baseline(city, tech, baseline)
    }

    fn city_tech_effect_want_with_baseline(&self, city: &City, tech: Tech, baseline: i32) -> i32 {
        let direct = match tech {
            Tech::RAILROAD | Tech::TRADE | Tech::THE_CORPORATION => {
                self.advisor_city_output(city, Some(tech)) - baseline
            }
            _ => 0,
        };
        // daitech.c adds each supported effect's raw value to the per-city
        // accumulator before applying its common 14/8 conversion. The direct
        // output preview above remains the base-want path for output effects.
        let effect_value = self.freeciv_city_tech_effect_value(city, tech);
        let mut want = (direct + effect_value) * 14 / 8;
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

    /// Source-backed subset of Freeciv daieffects.c:dai_effect_value used by
    /// daitech.c:dai_tech_effect_values. This covers the selected C2C3
    /// vision, movement, upgrade-price, health, and Make_Content effects.
    /// Output-waste/penalty effects are covered by the city-output preview;
    /// Railroad/Trade remain explicit output-preview adapters because their
    /// requirements have no direct Civ III advance binding. Freeciv itself
    /// gives no value to the selected Trade_Revenue_Bonus, Inspire_Partisans,
    /// Has_Senate, or timeline effects.
    fn freeciv_city_tech_effect_value(&self, city: &City, tech: Tech) -> i32 {
        if !tech.is_civ3_advance() {
            return 0;
        }

        let ruleset = RulesetData::campaign_default();
        // dai_tech_effect_values temporarily marks the candidate advance as
        // known while checking whether the effect's other requirements remain
        // possible, including whether it makes a building obsolete.
        let context = SimulationRequirementContext::for_city_at(self, city, None)
            .with_granted_tech(Some(tech));
        let mut total = 0_i32;

        for effect in &ruleset.effects {
            if effect.multiplier.is_some()
                || !matches!(
                    effect.kind.to_ascii_lowercase().as_str(),
                    "city_vision_radius_sq"
                        | "unit_vision_radius_sq"
                        | "move_bonus"
                        | "upgrade_price_pct"
                        | "make_content"
                        | "health_pct"
                )
            {
                continue;
            }

            let candidate_techs: Vec<_> = effect
                .requirements
                .iter()
                .filter_map(|requirement| bound_player_tech(ruleset, requirement))
                .filter(|(required, _)| *required == tech)
                .collect();
            if candidate_techs.is_empty() {
                continue;
            }

            // Freeciv evaluates the effect with the candidate advance known,
            // then subtracts it when that advance removes the effect.
            let active_after = candidate_techs.iter().all(|(_, present)| *present);
            let active_before = candidate_techs.iter().all(|(_, present)| !*present);
            let direction = match (active_before, active_after) {
                (true, false) => -1,
                (false, true) => 1,
                _ => continue,
            };

            let other_requirements_active = effect.requirements.iter().all(|requirement| {
                if bound_player_tech(ruleset, requirement)
                    .is_some_and(|(required, _)| required == tech)
                {
                    return true;
                }
                // dai_tech_effect_values passes only player and city to
                // is_req_active(..., RPT_POSSIBLE). In that context Local and
                // Tile requirements are TRI_MAYBE and therefore possible,
                // independent of their present flag.
                if matches!(
                    requirement.range,
                    RequirementRange::Local | RequirementRange::Tile
                ) {
                    return true;
                }
                context
                    .is_present(requirement)
                    .is_ok_and(|present| present == requirement.present)
            });
            if !other_requirements_active {
                continue;
            }

            let amount = match i32::try_from(effect.value) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let value = match effect.kind.to_ascii_lowercase().as_str() {
                // Freeciv's WAG: one unit of value per city per radius amount.
                "city_vision_radius_sq" | "unit_vision_radius_sq" => amount,
                // C2C3 sets illness_on=TRUE. daieffects.c uses the city's
                // current illness before the candidate tech's health bonus.
                "health_pct" => {
                    // The selected C2C3 game.ruleset sets illness_on=TRUE.
                    let illness = self.city_illness(city, self.city_forecast(city).shields);
                    5 + (amount / 5) * illness
                }
                "make_content" => {
                    let luxury = self.city_forecast(city).luxury;
                    let max_converted = self.city_min_unhappy_after_effect_stage(city, luxury);
                    let entertainers = self.city_entertainer_count(city);
                    amount.min(max_converted + entertainers) * 35
                }
                // Preserve daieffects.c's expression exactly: v starts at 0,
                // so EFT_MOVE_BONUS contributes num_affected_units(). Its
                // class matcher ignores UnitType requirements, as Freeciv's
                // unit_class_found() marks those requirements not applicable.
                "move_bonus" => match self.tech_effect_affected_units(city.owner, effect) {
                    Some(count) => count,
                    None => continue,
                },
                // Freeciv estimates 50 shields as the average base upgrade
                // price and applies -upgradeable * amount / 2.
                "upgrade_price_pct" => {
                    let upgradeable = self
                        .units
                        .iter()
                        .filter(|unit| {
                            unit.owner == city.owner && self.upgrade_target(unit).is_some()
                        })
                        .count() as i32;
                    -(upgradeable * amount / 2)
                }
                _ => continue,
            };
            total += direction * value;
        }

        total
    }

    /// Mirror daieffects.c:num_affected_units for the selected Move_Bonus
    /// effects. UnitClass requirements filter C2C3 class bindings; UnitType
    /// requirements are not applicable to a Freeciv unit class and therefore
    /// do not narrow this count.
    fn tech_effect_affected_units(&self, owner: usize, effect: &EffectRecord) -> Option<i32> {
        let class_requirements: Vec<_> = effect
            .requirements
            .iter()
            .filter(|requirement| {
                requirement.kind.eq_ignore_ascii_case("UnitClass")
                    && requirement.range == RequirementRange::Local
            })
            .collect();
        if class_requirements.is_empty() {
            return Some(self.units.iter().filter(|unit| unit.owner == owner).count() as i32);
        }

        let mut count = 0;
        for unit in self.units.iter().filter(|unit| unit.owner == owner) {
            let class = super::super::action_rules::freeciv_unit_class_rule_name(unit.kind)?;
            let matches = class_requirements.iter().all(|requirement| {
                let RequirementValue::String(name) = &requirement.name else {
                    return false;
                };
                class.eq_ignore_ascii_case(name) == requirement.present
            });
            count += i32::from(matches);
        }
        Some(count)
    }

    pub(in crate::sim) fn record_ai_tech_effect_wants(&mut self, owner: usize) {
        let cities: Vec<_> = self.player_cities_ordered(owner).cloned().collect();
        let baselines = cities
            .iter()
            .map(|city| self.advisor_city_output(city, None))
            .collect::<Vec<_>>();
        // daitech.c: effect feedback is the final contribution before selection.
        for tech in Tech::all().filter(|&tech| {
            city_effect_techs().contains(&tech)
                || cities.iter().any(|city| {
                    city.buildings.iter().any(|building| {
                        building
                            .data()
                            .obsolete
                            .contains(&(Requirement::Tech(tech.0), true))
                    })
                })
        }) {
            if self.has_tech(owner, tech) {
                continue;
            }
            for (city, &baseline) in cities.iter().zip(&baselines) {
                self.add_ai_tech_want(
                    owner,
                    tech,
                    self.city_tech_effect_want_with_baseline(city, tech, baseline) as f32,
                );
            }
        }
    }

    fn city_building_tech_want_with_baseline(
        &self,
        city: &City,
        tech: Tech,
        baseline: i32,
        food_surplus: i32,
        shields: i32,
    ) -> i32 {
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
        let limit = self.city_size_limit(city);
        let new_limit = self.city_size_limit(improved);
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
        let retention = self.growth_food_retained(improved) - self.growth_food_retained(city);
        if retention > 0 {
            benefit +=
                8 + retention / 10 * food_surplus.max(0) + retention / 15 * (-food_surplus).max(1);
        }
        benefit
    }
}

fn improvement_unlock_techs() -> &'static [Tech] {
    static TECHS: std::sync::OnceLock<Vec<Tech>> = std::sync::OnceLock::new();
    TECHS.get_or_init(|| {
        Tech::all()
            .filter(|&tech| {
                Building::all().any(|building| {
                    building.data().genus == Genus::Improvement
                        && building
                            .data()
                            .requirements
                            .contains(&(Requirement::Tech(tech.0), true))
                })
            })
            .collect()
    })
}

fn defender_techs() -> &'static [Tech] {
    static TECHS: std::sync::OnceLock<Vec<Tech>> = std::sync::OnceLock::new();
    TECHS.get_or_init(|| {
        Tech::all()
            .filter(|&tech| {
                Simulation::production_defender_types()
                    .any(|kind| kind.required_tech() == Some(tech))
            })
            .collect()
    })
}

fn city_effect_techs() -> &'static [Tech] {
    static TECHS: std::sync::OnceLock<Vec<Tech>> = std::sync::OnceLock::new();
    TECHS.get_or_init(|| {
        let ruleset = RulesetData::campaign_default();
        Tech::all()
            .filter(|&tech| {
                matches!(tech, Tech::RAILROAD | Tech::TRADE | Tech::THE_CORPORATION)
                    || tech.is_civ3_advance()
                        && ruleset.effects.iter().any(|effect| {
                            effect.multiplier.is_none()
                                && matches!(
                                    effect.kind.to_ascii_lowercase().as_str(),
                                    "city_vision_radius_sq"
                                        | "unit_vision_radius_sq"
                                        | "move_bonus"
                                        | "upgrade_price_pct"
                                        | "make_content"
                                        | "health_pct"
                                )
                                && effect
                                    .requirements
                                    .iter()
                                    .filter_map(|requirement| {
                                        bound_player_tech(ruleset, requirement)
                                    })
                                    .any(|(required, _)| required == tech)
                        })
            })
            .collect()
    })
}

/// Resolve only the player-scoped, non-surviving Tech universals that
/// daitech.c treats as the candidate advance itself.
fn bound_player_tech(
    ruleset: &RulesetData,
    requirement: &FreecivRequirement,
) -> Option<(Tech, bool)> {
    if !requirement.kind.eq_ignore_ascii_case("Tech")
        || requirement.range != RequirementRange::Player
        || requirement.survives
    {
        return None;
    }
    let RequirementValue::String(name) = &requirement.name else {
        return None;
    };
    match bind_freeciv_technology(ruleset, name).ok()? {
        FreecivTechBinding::Bound(tech) if tech.is_civ3_advance() => {
            Some((tech, requirement.present))
        }
        FreecivTechBinding::Bound(_) | FreecivTechBinding::MissingFromCiv3Catalog => None,
    }
}
