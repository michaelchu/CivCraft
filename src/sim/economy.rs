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
        let surplus = self.food - size * CITIZEN_FOOD_COST - support_food;
        if self.disorder {
            surplus.min(0)
        } else {
            surplus
        }
    }

    pub(crate) fn shield_surplus(&self) -> i32 {
        if self.disorder {
            0
        } else {
            (self.shields - self.support.shields).max(0)
        }
    }

    pub(crate) fn gold_surplus(&self, unit_gold: i32) -> i32 {
        self.gold - self.building_upkeep - unit_gold
    }
}

impl City {
    pub(super) fn has_building(&self, building: Building) -> bool {
        self.buildings.contains(&building)
    }
}

impl Simulation {
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
        if player.government == Government::Anarchy
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
        [
            (Building::LIBRARY, Building::GREAT_LIBRARY),
            (Building::UNIVERSITY, Building::ISAAC_NEWTONS_COLLEGE),
            (Building::RESEARCH_LAB, Building::INTERNET),
        ]
        .into_iter()
        .filter(|&(building, _)| city.has_building(building))
        .map(|(_, wonder)| 50 + 50 * i32::from(self.completed_wonders.contains(&wonder)))
        .sum::<i32>()
            + 100 * i32::from(city.has_building(Building::SETI_PROGRAM))
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
        let mut citizens = [0; 3];
        for (index, wonder) in [
            Building::DARWINS_VOYAGE,
            Building::A_SMITHS_TRADING_CO,
            Building::SHAKESPEARES_THEATER,
        ]
        .into_iter()
        .enumerate()
        {
            citizens[index] =
                city.specialists[index] * (2 + i32::from(self.completed_wonders.contains(&wonder)));
        }
        if city.has_building(Building::COPERNICUS_OBSERVATORY)
            && !self.building_is_obsolete(city, Building::COPERNICUS_OBSERVATORY, None)
        {
            citizens[0] += 1 + city.worked_tiles.len().min(city.tile_workers() as usize) as i32;
        }
        self.split_city_trade_from_citizens(city, trade, rates, citizens)
    }

    fn split_city_trade_from_citizens(
        &self,
        city: &City,
        trade: i32,
        rates: [i32; 3],
        citizens: [i32; 3],
    ) -> [i32; 3] {
        let government = self.players[city.owner].government;
        let rates = if government == Government::Anarchy {
            [0, 0, 100]
        } else {
            rates
        };
        let mut output = Self::distribute_trade(trade, rates);

        for index in 0..3 {
            output[index] += citizens[index];
        }
        // common/city.c:get_city_tithes_bonus/set_city_production:
        // Happiness_To_Gold uses full Make_Content + Force_Content effect
        // amounts, even when no citizen needs calming. Civ2Civ3 has no
        // Force_Content rows at Normal difficulty.
        if government == Government::Fundamentalism {
            output[1] += self.building_content(city);
        }

        output[0] += 4 * i32::from(city.has_building(Building::GREAT_LIBRARY))
            + 6 * i32::from(city.has_building(Building::ISAAC_NEWTONS_COLLEGE))
            + i32::from(self.has_active_wonder(city.owner, Building::DARWINS_VOYAGE));
        output[2] += 2 * i32::from(government == Government::Federation);
        output[2] +=
            4 * i32::from(
                city.has_building(Building::HANGING_GARDENS)
                    && !self.building_is_obsolete(city, Building::HANGING_GARDENS, None),
            ) + 2 * i32::from(self.has_active_wonder(city.owner, Building::JS_BACHS_CATHEDRAL))
                + 2 * i32::from(self.has_active_wonder(city.owner, Building::CURE_FOR_CANCER))
                + i32::from(self.has_active_wonder(city.owner, Building::SHAKESPEARES_THEATER))
                + 5 * i32::from(city.has_building(Building::SHAKESPEARES_THEATER));

        let commercial_bonus = self.commercial_output_bonus(city) - 100;
        for (index, amount) in output.iter_mut().enumerate() {
            let bonus = if index == 0 {
                self.science_building_bonus(city)
            } else {
                commercial_bonus
            };
            let adjustment = match (government, index) {
                (Government::Anarchy, 2) | (Government::Fundamentalism, 0) => -50,
                (Government::Despotism, 1)
                    if city.has_building(Building::PALACE)
                        || city.has_building(Building::ECCLESIASTICAL_PALACE) =>
                {
                    75
                }
                (Government::Monarchy, 1)
                    if city.has_building(Building::PALACE)
                        || city.has_building(Building::ECCLESIASTICAL_PALACE) =>
                {
                    50
                }
                _ => 0,
            };
            *amount = amount.wrapping_mul(100 + bonus + adjustment) / 100;
        }
        output
    }

    pub(super) fn balance_building_upkeep(&mut self, owner: usize, city_id: u32) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .cities
                .iter()
                .enumerate()
                .filter(|(_, city)| city.id == city_id)
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
        match self.players[city.owner].government {
            Government::Anarchy => 99,
            Government::Communism => 1,
            _ => i32::from(
                city.has_building(Building::STOCK_EXCHANGE)
                    && self.has_active_wonder(city.owner, Building::A_SMITHS_TRADING_CO),
            ),
        }
    }

    pub(crate) fn building_upkeep(&self, city: &City) -> i32 {
        city.buildings
            .iter()
            .map(|building| (building.data().upkeep - self.free_building_upkeep(city)).max(0))
            .sum()
    }

    pub(crate) fn city_size_limit(city: &City) -> i32 {
        let has_aqueduct = [
            Building::AQUEDUCT,
            Building::AQUEDUCT_RIVER,
            Building::AQUEDUCT_LAKE,
        ]
        .iter()
        .any(|name| city.has_building(*name));
        if city.has_building(Building::HOSPITAL) || city.has_building(Building::SEWER_SYSTEM) {
            i32::MAX
        } else if has_aqueduct {
            12
        } else {
            6
        }
    }

    pub(super) fn food_retained(city: &City) -> i32 {
        match city.size {
            ..=3 => 50,
            _ if !city.has_building(Building::GRANARY) => 0,
            4 => 50,
            5..=6 => 34,
            _ => 25,
        }
    }
}

#[cfg(test)]
#[path = "economy/city_manager_tests.rs"]
mod city_manager_tests;
