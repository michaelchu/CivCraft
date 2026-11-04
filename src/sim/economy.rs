use super::*;

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

    pub(super) fn has_building(&self, name: &str) -> bool {
        self.buildings
            .iter()
            .any(|building| building.data().name == name)
    }
}

impl Simulation {
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
            "Darwin's Voyage",
            "A.Smith's Trading Co.",
            "Shakespeare's Theater",
        ]
        .into_iter()
        .enumerate()
        {
            let improved = self
                .completed_wonders
                .iter()
                .any(|building| building.data().name == wonder);
            output[index] += city.specialists[index] * (2 + i32::from(improved));
        }

        let bonuses = [
            ["Library", "University", "Research Lab"].as_slice(),
            ["Marketplace", "Bank", "Stock Exchange"].as_slice(),
            ["Marketplace", "Bank", "Stock Exchange"].as_slice(),
        ];
        for (index, (amount, buildings)) in output.iter_mut().zip(bonuses).enumerate() {
            let bonus = buildings
                .iter()
                .filter(|name| city.has_building(name))
                .count() as i32;
            let adjustment = match (government, index) {
                (Government::Anarchy, 2) | (Government::Fundamentalism, 0) => -50,
                (Government::Despotism, 1)
                    if city.has_building("Palace")
                        || city.has_building("Ecclesiastical Palace") =>
                {
                    75
                }
                (Government::Monarchy, 1)
                    if city.has_building("Palace")
                        || city.has_building("Ecclesiastical Palace") =>
                {
                    50
                }
                _ => 0,
            };
            *amount = percent_of(*amount, 100 + 50 * bonus + adjustment);
        }
        output
    }

    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
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

        let mysticism = self.players[city.owner]
            .known
            .iter()
            .any(|tech| tech.name() == "Mysticism");
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
            + 2 * i32::from(city.has_building("Police Station"))
            + i32::from(city.has_building("Courthouse"))
            + if city.has_building("Temple") {
                1 + i32::from(mysticism && government != Government::Communism)
            } else {
                0
            };
        let calming_angry = pacification.min(angry);
        angry -= calming_angry;
        unhappy += calming_angry;
        let promoted = unhappy.min(pacification - calming_angry);
        unhappy -= promoted;
        content += promoted;

        let suffrage = self.owns_wonder(city.owner, "Women's Suffrage");
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
                        && !self
                            .cities
                            .iter()
                            .any(|other| (other.x, other.y) == (unit.x, unit.y))
                })
                .count() as i32;
            let police = i32::from(city.has_building("Police Station"))
                * if government == Government::Democracy && !suffrage {
                    2
                } else {
                    1
                };
            let local_suffrage = i32::from(city.has_building("Women's Suffrage"));
            let pressure =
                (deployed * (factor - police - local_suffrage).max(0) - allowance).max(0);
            let from_content = content.min(pressure);
            unhappy += from_content;
            let pressure = pressure - from_content;
            let from_happy = happy.min(pressure / 2);
            unhappy += from_happy;
            happy = (happy - from_happy - pressure % 2).max(0);
        }
        happy < unhappy + 2 * angry
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
                city.has_building("Stock Exchange")
                    && self.owns_wonder(city.owner, "A.Smith's Trading Co."),
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
        if !["Aqueduct", "Aqueduct, River", "Aqueduct, Lake"]
            .iter()
            .any(|name| city.has_building(name))
        {
            8
        } else if city.has_building("Sewer System") {
            i32::MAX
        } else {
            16
        }
    }

    pub(super) fn food_retained(city: &City) -> i32 {
        match city.size {
            ..=3 => 50,
            _ if !city.has_building("Granary") => 0,
            4 => 50,
            5..=6 => 34,
            _ => 25,
        }
    }
}
