use super::*;

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
    pub(crate) fn tile_workers(&self) -> i32 {
        self.size - self.specialists.iter().sum::<i32>()
    }

    pub(super) fn remove_specialists(&mut self, mut excess: i32) {
        // The ruleset removes entertainers first, followed by scientists and taxmen.
        for index in [2, 0, 1] {
            let removed = excess.min(self.specialists[index]);
            self.specialists[index] -= removed;
            excess -= removed;
        }
    }

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

    pub(super) fn assign_specialist(&mut self, city_id: u32, index: usize, delta: i32) -> bool {
        let Some(city) = self
            .cities
            .iter_mut()
            .find(|city| city.id == city_id && city.owner == HUMAN)
        else {
            return false;
        };
        if index >= city.specialists.len()
            || !matches!(delta, -1 | 1)
            || (delta == 1 && city.tile_workers() == 0)
            || (delta == -1 && city.specialists[index] == 0)
        {
            return false;
        }
        city.specialists[index] += delta;
        self.reconcile_city_workers();
        true
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
        .sum()
    }

    pub(super) fn split_city_trade(&self, city: &City, trade: i32, rates: [i32; 3]) -> [i32; 3] {
        let government = self.players[city.owner].government;
        let rates = if government == Government::Anarchy {
            [0, 0, 100]
        } else {
            rates
        };
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

        for (index, wonder) in [
            Building::DARWINS_VOYAGE,
            Building::A_SMITHS_TRADING_CO,
            Building::SHAKESPEARES_THEATER,
        ]
        .into_iter()
        .enumerate()
        {
            let improved = self
                .completed_wonders
                .iter()
                .any(|&building| building == wonder);
            output[index] += city.specialists[index] * (2 + i32::from(improved));
        }

        output[0] += 4 * i32::from(city.has_building(Building::GREAT_LIBRARY))
            + 6 * i32::from(city.has_building(Building::ISAAC_NEWTONS_COLLEGE))
            + i32::from(self.has_active_wonder(city.owner, Building::DARWINS_VOYAGE));
        if city.has_building(Building::COPERNICUS_OBSERVATORY)
            && !self.building_is_obsolete(city, Building::COPERNICUS_OBSERVATORY, None)
        {
            output[0] += 1 + city.worked_tiles.len().min(city.tile_workers() as usize) as i32;
        }

        output[2] +=
            4 * i32::from(
                city.has_building(Building::HANGING_GARDENS)
                    && !self.building_is_obsolete(city, Building::HANGING_GARDENS, None),
            ) + 2 * i32::from(self.has_active_wonder(city.owner, Building::JS_BACHS_CATHEDRAL))
                + 2 * i32::from(self.has_active_wonder(city.owner, Building::CURE_FOR_CANCER))
                + i32::from(self.has_active_wonder(city.owner, Building::SHAKESPEARES_THEATER))
                + 5 * i32::from(city.has_building(Building::SHAKESPEARES_THEATER));

        let commercial_bonus = 50
            * [
                Building::MARKETPLACE,
                Building::BANK,
                Building::STOCK_EXCHANGE,
            ]
            .into_iter()
            .filter(|&building| city.has_building(building))
            .count() as i32;
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
            *amount = percent_of(*amount, 100 + bonus + adjustment);
        }
        output
    }

    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
        self.city_mood(city, luxury).0
    }

    fn building_content(&self, city: &City) -> i32 {
        let has = |building| city.has_building(building);
        let owns = |wonder| self.has_active_wonder(city.owner, wonder);
        let communist = self.players[city.owner].government == Government::Communism;
        let mut content = 2 * i32::from(has(Building::POLICE_STATION))
            + i32::from(has(Building::COURTHOUSE))
            + i32::from(owns(Building::HANGING_GARDENS));
        if has(Building::TEMPLE) {
            content += 1
                + i32::from(!communist && self.has_tech(city.owner, Tech::MYSTICISM))
                + 2 * i32::from(owns(Building::TEMPLE_OF_ARTEMIS));
        }
        if has(Building::CATHEDRAL) {
            content += 3
                + i32::from(!communist && self.has_tech(city.owner, Tech::THEOLOGY))
                + 3 * i32::from(owns(Building::MICHELANGELOS_CHAPEL));
        }
        if has(Building::AMPHITHEATER) {
            content += 3 + i32::from(self.has_tech(city.owner, Tech::ELECTRICITY));
        }
        if owns(Building::MAUSOLEUM_OF_MAUSOLOS) {
            content += i32::from(has(Building::CITY_WALLS)) + i32::from(has(Building::COURTHOUSE));
        }
        content
    }

    pub(super) fn city_mood(&self, city: &City, luxury: i32) -> (bool, bool) {
        let (happy, unhappy, angry) = self.city_feelings(city, luxury);
        (
            happy < unhappy + 2 * angry,
            city.size >= 3 && unhappy == 0 && angry == 0 && happy >= (city.size + 1) / 2,
        )
    }

    pub(super) fn city_feelings(&self, city: &City, luxury: i32) -> (i32, i32, i32) {
        let government = self.players[city.owner].government;
        // Apply luxury before Make_Content.
        let cities = self
            .cities
            .iter()
            .filter(|other| other.owner == city.owner)
            .count() as i32;
        let basis = government.empire_size();
        let empire_penalty = if basis > 0 {
            (cities - 1).max(0) / basis
        } else {
            0
        };
        let base = 4 - empire_penalty;
        let workers = city.tile_workers();
        let specialists = city.size - workers;
        let content = (base.clamp(0, city.size) - specialists).max(0);
        let mut angry = (-base).clamp(0, workers);
        let mut unhappy = workers - content - angry;
        let mut upgrades = luxury / 2;
        let calmed = angry.min(upgrades);
        angry -= calmed;
        unhappy += calmed;
        upgrades -= calmed;
        let happy_content = content.min(upgrades);
        upgrades -= happy_content;
        let happy_unhappy = unhappy.min(upgrades / 2);
        unhappy -= happy_unhappy;
        let mut content = content - happy_content;
        upgrades -= happy_unhappy * 2;
        if upgrades >= 1 {
            let promoted = unhappy.min(1);
            unhappy -= promoted;
            content += promoted;
        }

        let martial = match government {
            Government::Anarchy | Government::Despotism | Government::Monarchy => 1,
            Government::Communism => 2,
            _ => 0,
        };
        let garrison = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == city.owner
                    && unit.kind.is_military()
                    && (unit.x, unit.y) == (city.x, city.y)
            })
            .count()
            .min(3) as i32;
        let pacification = i32::from(government == Government::Tribal)
            + martial * garrison
            + self.building_content(city);
        let calming_angry = pacification.min(angry);
        angry -= calming_angry;
        unhappy += calming_angry;
        let promoted = unhappy.min(pacification - calming_angry);
        unhappy -= promoted;
        content += promoted;

        let suffrage = self.has_active_wonder(city.owner, Building::WOMENS_SUFFRAGE);
        let (factor, allowance) = match government {
            Government::Republic => (1, 1),
            Government::Democracy => (2 - i32::from(suffrage), 3 - i32::from(suffrage)),
            Government::Communism | Government::Fundamentalism | Government::Federation => (1, 2),
            _ => (0, 0),
        };
        let mut happy = happy_content + happy_unhappy;
        if factor > 0 {
            let deployed = self
                .units
                .iter()
                .filter(|unit| {
                    unit.home_city == Some(city.id)
                        && unit.owner == city.owner
                        && unit.kind.is_military()
                        && self.tile(unit.x, unit.y).owner != Some(city.owner)
                        && !(matches!(
                            self.tile(unit.x, unit.y).base,
                            Some(Base::Fortress | Base::Airbase)
                        ) && self.cities.iter().any(|nearby| {
                            nearby.owner == unit.owner
                                && self.map_distance(unit.x, unit.y, nearby.x, nearby.y) <= 3
                        }))
                        && !self
                            .cities
                            .iter()
                            .any(|other| (other.x, other.y) == (unit.x, unit.y))
                })
                .count() as i32;
            let police = i32::from(city.has_building(Building::POLICE_STATION))
                * if government == Government::Democracy && !suffrage {
                    2
                } else {
                    1
                };
            let local_suffrage = i32::from(city.has_building(Building::WOMENS_SUFFRAGE));
            let crusade =
                i32::from(self.has_active_wonder(city.owner, Building::KING_RICHARDS_CRUSADE));
            let pressure =
                (deployed * (factor - police - local_suffrage).max(0) - allowance - crusade).max(0);
            let from_content = content.min(pressure);
            unhappy += from_content;
            let pressure = pressure - from_content;
            let from_happy = happy.min(pressure / 2);
            unhappy += from_happy;
            happy = (happy - from_happy - pressure % 2).max(0);
        }
        (happy, unhappy, angry)
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
        if ![
            Building::AQUEDUCT,
            Building::AQUEDUCT_RIVER,
            Building::AQUEDUCT_LAKE,
        ]
        .iter()
        .any(|name| city.has_building(*name))
        {
            8
        } else if city.has_building(Building::SEWER_SYSTEM) {
            i32::MAX
        } else {
            16
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
