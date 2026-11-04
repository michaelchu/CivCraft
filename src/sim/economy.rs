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
        let rates = &mut self.players[owner].rates;
        if index >= rates.len() || rates[index] >= 60 {
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
        for (amount, buildings) in output.iter_mut().zip(bonuses) {
            let bonus = buildings
                .iter()
                .filter(|name| city.has_building(name))
                .count();
            *amount = apply_percent_bonus(*amount, 50 * bonus as i32);
        }
        output
    }

    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
        // Opening government: Tribal. Apply luxury before Make_Content.
        let cities = self
            .cities
            .iter()
            .filter(|other| other.owner == city.owner)
            .count() as i32;
        let empire_penalty = (cities - 1).max(0) / 12;
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
        upgrades -= happy_unhappy * 2;
        if upgrades >= 1 {
            unhappy = (unhappy - 1).max(0);
        }

        let mysticism = self.players[city.owner]
            .known
            .iter()
            .any(|tech| tech.name() == "Mysticism");
        let pacification = 1 + if city.has_building("Temple") {
            1 + i32::from(mysticism)
        } else {
            0
        };
        let calming_angry = pacification.min(angry);
        angry -= calming_angry;
        unhappy = (unhappy + calming_angry - (pacification - calming_angry)).max(0);
        happy_content + happy_unhappy < unhappy + 2 * angry
    }

    pub(super) fn balance_building_upkeep(&mut self, owner: usize) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .cities
                .iter()
                .enumerate()
                .filter(|(_, city)| city.owner == owner)
                .flat_map(|(index, city)| {
                    city.buildings
                        .iter()
                        .copied()
                        .filter(|building| {
                            building.data().genus == crate::rules::Genus::Improvement
                                && building.data().upkeep > 0
                        })
                        .map(move |building| (index, building))
                })
                .collect();
            if candidates.is_empty() {
                self.players[owner].gold = 0;
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

    pub(crate) fn building_upkeep(city: &City) -> i32 {
        city.buildings
            .iter()
            .map(|building| building.data().upkeep)
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
