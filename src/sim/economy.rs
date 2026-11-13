//! City output, citizen mood, tax allocation, upkeep, and economy advice inputs.
//!
//! Economy calculations connect worked tiles and specialists to food,
//! production, trade, and science, then apply government, building, and other
//! effects. `CityForecast` provides a common view of expected output for UI and
//! advisor code. The child modules handle worker assignment and mood rules;
//! authoritative turn-time collection remains coordinated by the simulation.
//!
//! Keep forecast and actual-resolution paths aligned when changing effects.
//! Integer rounding, effect order, and worker selection can change growth,
//! production completion, or research even when displayed totals look close.

use super::*;

mod city_manager;
mod city_mood;
pub(in crate::sim) mod ruleset;
#[cfg(test)]
use city_manager::*;

pub(crate) struct CityForecast {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) trade: i32,
    pub(crate) science: i32,
    pub(super) gross_science: i32,
    pub(super) gross_gold: i32,
    pub(crate) gold: i32,
    pub(crate) luxury: i32,
    pub(crate) support: super::upkeep::UnitUpkeep,
    pub(crate) building_upkeep: i32,
    pub(crate) disorder: bool,
    pub(super) happy: bool,
}

impl CityForecast {
    pub(crate) fn food_surplus(&self, size: i32, support_food: i32) -> i32 {
        let surplus =
            self.food - size * ruleset::CityEconomySettings::campaign().food_cost - support_food;
        if self.disorder {
            surplus.min(0)
        } else {
            surplus
        }
    }

    pub(crate) fn shield_surplus(&self) -> i32 {
        let output = if self.disorder { 0 } else { self.shields };
        output - self.support.shields
    }

    pub(crate) fn gold_surplus(&self, unit_gold: i32) -> i32 {
        let settings = ruleset::CityEconomySettings::campaign();
        let building_upkeep = if settings.gold_upkeep_style == ruleset::GoldUpkeepStyle::Nation {
            0
        } else {
            self.building_upkeep
        };
        let unit_upkeep = if settings.gold_upkeep_style == ruleset::GoldUpkeepStyle::City {
            unit_gold
        } else {
            0
        };
        self.gold - building_upkeep - unit_upkeep
    }
}

impl City {
    pub(super) fn has_building(&self, building: Building) -> bool {
        self.buildings.contains(&building)
    }
}

impl Simulation {
    pub(crate) fn citizen_food_cost() -> i32 {
        ruleset::CityEconomySettings::campaign().food_cost
    }

    pub(crate) fn city_forecast(&self, city: &City) -> CityForecast {
        self.city_forecast_with_tech(city, None)
    }

    pub(super) fn city_forecast_with_tech(
        &self,
        city: &City,
        granted: Option<Tech>,
    ) -> CityForecast {
        let (food, shields, trade) = self.city_yields_with_tech(city, granted);
        let [science, gold, luxury] = self.city_trade(city, trade);
        let (disorder, happy) = self.city_mood(city, luxury);
        CityForecast {
            food,
            shields,
            trade,
            luxury,
            disorder,
            happy,
            gross_science: science,
            gross_gold: gold,
            science: if disorder { 0 } else { science },
            gold: if disorder { 0 } else { gold },
            support: self.city_unit_upkeep(city),
            building_upkeep: self.building_upkeep(city),
        }
    }

    pub(super) fn raise_rate(&mut self, owner: usize, index: usize) -> bool {
        let player = &mut self.players[owner];
        let rates = &mut player.rates;
        if player.government == Government::during_revolution()
            || index >= rates.len()
            || rates[index] >= player.government.max_rate()
        {
            return false;
        }
        let donor = (0..rates.len())
            .filter(|&other| other != index)
            .max_by_key(|&other| rates[other])
            .unwrap();
        rates[donor] -= 10;
        rates[index] += 10;
        true
    }

    pub(crate) fn city_trade(&self, city: &City, trade: i32) -> [i32; 3] {
        self.split_city_trade(city, trade, self.players[city.owner].rates)
    }

    pub(super) fn science_building_bonus(&self, city: &City) -> i32 {
        self.city_output_bonus_percent(city, "Science", self.players[city.owner].government, None)
            - 100
    }

    pub(super) fn distribute_trade(trade: i32, rates: [i32; 3]) -> [i32; 3] {
        let scaled = rates.map(|rate| trade * rate);
        let mut output = scaled.map(|value| value / 100);
        let mut order = [0, 1, 2];
        // Freeciv assigns ties to the smallest integer share, then science, tax, luxury.
        order.sort_by_key(|&i| (-(scaled[i] % 100), output[i], i));
        for &index in order
            .iter()
            .take((trade - output.iter().sum::<i32>()) as usize)
        {
            output[index] += 1;
        }
        output
    }

    pub(super) fn split_city_trade(&self, city: &City, trade: i32, rates: [i32; 3]) -> [i32; 3] {
        let government = self.players[city.owner].government;
        let specialist_output = |index, specialist, output_type| {
            city.specialists[index]
                * self.city_rule_effect(
                    city,
                    "Specialist_Output",
                    None,
                    Some(output_type),
                    Some(specialist),
                    false,
                    government,
                    None,
                )
        };
        let citizens = [
            specialist_output(0, "Scientist", "Science"),
            specialist_output(1, "Taxman", "Gold"),
            specialist_output(2, "Elvis", "Luxury"),
        ];
        self.split_city_trade_from_citizens(city, trade, rates, citizens)
    }

    fn split_city_trade_from_citizens(
        &self,
        city: &City,
        trade: i32,
        mut rates: [i32; 3],
        citizens: [i32; 3],
    ) -> [i32; 3] {
        let government = self.players[city.owner].government;
        if government == Government::during_revolution() {
            // common/city.c::add_tax_income forces all trade to luxury during
            // the ruleset's temporary government without changing saved rates.
            rates = [0, 0, 100];
        }
        let mut output = Self::distribute_trade(trade, rates);
        for index in 0..3 {
            output[index] += citizens[index];
        }

        if self.city_rule_effect(
            city,
            "Happiness_To_Gold",
            None,
            None,
            None,
            false,
            government,
            None,
        ) > 0
        {
            output[1] += self.city_rule_effect(
                city,
                "Make_Content",
                None,
                None,
                None,
                false,
                government,
                None,
            ) + self.city_rule_effect(
                city,
                "Force_Content",
                None,
                None,
                None,
                false,
                government,
                None,
            );
        }

        for (index, output_type) in ["Science", "Gold", "Luxury"].into_iter().enumerate() {
            let bonus = self.city_output_bonus_percent(city, output_type, government, None);
            output[index] = (i64::from(output[index]) * i64::from(bonus) / 100)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        }
        output
    }

    pub(super) fn balance_building_upkeep(&mut self, owner: usize, city_id: u32) {
        self.balance_building_upkeep_for_cities(owner, Some(city_id));
    }

    pub(super) fn balance_national_building_upkeep(&mut self, owner: usize) {
        let total = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| self.building_upkeep(city))
            .sum::<i32>();
        self.players[owner].gold -= total;
        self.balance_building_upkeep_for_cities(owner, None);
    }

    fn balance_building_upkeep_for_cities(&mut self, owner: usize, city_id: Option<u32>) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .cities
                .iter()
                .enumerate()
                .filter(|(_, city)| city.owner == owner && city_id.is_none_or(|id| city.id == id))
                .flat_map(|(index, city)| {
                    let free = self.free_building_upkeep(city);
                    city.buildings
                        .iter()
                        .copied()
                        .filter(move |building| {
                            building.data().genus == crate::rules::Genus::Improvement
                                && building.data().upkeep > free
                        })
                        .map(move |building| (index, building))
                })
                .collect();
            if candidates.is_empty() {
                break;
            }
            let (index, building) = candidates[self.rand_below(candidates.len() as i32) as usize];
            let city = &mut self.cities[index];
            city.buildings.retain(|&built| built != building);
            self.players[owner].gold += building.data().cost;
            if owner == HUMAN {
                let message = format!(
                    "{} sold its {} to cover upkeep.",
                    city.name,
                    building.data().name
                );
                self.add_event(message);
            }
        }
    }

    fn free_building_upkeep(&self, city: &City) -> i32 {
        self.building_upkeep_free_limit(city)
    }

    pub(crate) fn building_upkeep(&self, city: &City) -> i32 {
        let free = self.free_building_upkeep(city);
        city.buildings
            .iter()
            .map(|building| {
                if building.data().upkeep <= free {
                    0
                } else {
                    building.data().upkeep
                }
            })
            .sum()
    }

    pub(crate) fn city_size_limit(&self, city: &City) -> i32 {
        self.city_size_limit_from_ruleset(city)
    }

    pub(super) fn growth_food_retained(&self, city: &City) -> i32 {
        self.city_growth_retention(city, false)
    }

    pub(super) fn shrink_food_retained(&self, city: &City) -> i32 {
        self.city_growth_retention(city, true)
    }
}

#[cfg(test)]
#[path = "economy/city_manager_tests.rs"]
mod city_manager_tests;
