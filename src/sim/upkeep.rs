use super::*;

pub(crate) struct UnitUpkeep {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) gold: i32,
    pub(super) food_unit: Option<u32>,
}

impl Simulation {
    pub(crate) fn city_unit_upkeep(&self, city: &City) -> UnitUpkeep {
        let mut food_units: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.home_city == Some(city.id)
                    && unit.owner == city.owner
                    && unit.kind.base_upkeep().0 > 0
                    && !self.fanatic_upkeep_exempt(unit)
            })
            .collect();
        food_units.sort_by_key(|unit| std::cmp::Reverse(unit.home_order));
        let free_food = city.size.clamp(4, 20) as usize;
        let shield_units = self
            .units
            .iter()
            .filter(|unit| {
                unit.home_city == Some(city.id)
                    && unit.owner == city.owner
                    && self.unit_shield_cost(unit) > 0
            })
            .count() as i32;
        let (_, free_shields, _) = self.players[city.owner].government.unit_support();
        let zeus = self.has_active_wonder(city.owner, Building::STATUE_OF_ZEUS);
        UnitUpkeep {
            food: food_units.len().saturating_sub(free_food) as i32,
            shields: (shield_units - free_shields - i32::from(zeus)).max(0),
            gold: self
                .city_gold_upkeep(city)
                .iter()
                .map(|(_, cost)| cost)
                .sum(),
            food_unit: food_units.get(free_food).map(|unit| unit.id),
        }
    }

    fn fanatic_upkeep_exempt(&self, unit: &Unit) -> bool {
        unit.kind == UnitKind::Fanatics
            && self.players[unit.owner].government == Government::Fundamentalism
    }

    fn unit_shield_cost(&self, unit: &Unit) -> i32 {
        if self.fanatic_upkeep_exempt(unit)
            || !matches!(
                self.players[unit.owner].government,
                Government::Anarchy
                    | Government::Tribal
                    | Government::Communism
                    | Government::Republic
            )
            || self.unit_gold_cost(unit) > 0
        {
            0
        } else {
            unit.kind.base_upkeep().1
        }
    }

    fn unit_gold_cost(&self, unit: &Unit) -> i32 {
        if self.fanatic_upkeep_exempt(unit) || !unit.kind.has_shield_to_gold() {
            0
        } else {
            unit.kind.base_upkeep().1 * self.players[unit.owner].government.unit_support().0
        }
    }

    pub(super) fn city_gold_upkeep(&self, city: &City) -> Vec<(u32, i32)> {
        let (conversion, _, free_gold) = self.players[city.owner].government.unit_support();
        if conversion == 0 {
            return Vec::new();
        }
        let mut free = free_gold
            + 2 * i32::from(self.has_active_wonder(city.owner, Building::KING_RICHARDS_CRUSADE));
        let mut units: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == city.owner && unit.home_city == Some(city.id))
            .collect();
        units.sort_by_key(|unit| std::cmp::Reverse(unit.home_order));
        units
            .into_iter()
            .filter_map(|unit| {
                let cost = self.unit_gold_cost(unit);
                let covered = free.min(cost);
                free -= covered;
                (cost > covered).then_some((unit.id, cost - covered))
            })
            .collect()
    }

    pub(super) fn balance_unit_gold_upkeep(&mut self, owner: usize, paid: &[(u32, i32)]) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| paid.iter().any(|&(id, _)| id == unit.id))
                .map(|unit| unit.id)
                .collect();
            if candidates.is_empty() {
                self.players[owner].gold = 0;
                break;
            }
            let count = candidates.len();
            let mut id = candidates[self.rand_below(count as i32) as usize];
            // The reference sells upkeep-paying cargo before its transport.
            while let Some(cargo) = self.units.iter().find(|unit| {
                unit.transport == Some(id) && paid.iter().any(|&(paid_id, _)| paid_id == unit.id)
            }) {
                id = cargo.id;
            }
            let unit = self.unit_by_id(id).unwrap();
            if owner == HUMAN {
                let message = format!(
                    "Not enough gold to support {}. The unit was disbanded.",
                    unit.kind.name()
                );
                self.add_event(message);
            }
            self.remove_units_with_cargo(vec![id]);
            self.players[owner].gold += paid.iter().find(|&&(paid_id, _)| paid_id == id).unwrap().1;
        }
    }

    pub(crate) fn home_city_target(&self, unit: &Unit) -> Option<u32> {
        if unit.kind.has_no_home() || (unit.home_city.is_none() && !unit.kind.is_diplomatic()) {
            return None;
        }
        self.cities
            .iter()
            .find(|city| {
                city.owner == unit.owner
                    && city.x == unit.x
                    && city.y == unit.y
                    && unit.home_city != Some(city.id)
            })
            .map(|city| city.id)
    }

    pub(super) fn change_home_city(&mut self, unit_index: usize) -> bool {
        let Some(city_id) = self.home_city_target(&self.units[unit_index]) else {
            return false;
        };
        let unit = &mut self.units[unit_index];
        unit.home_city = Some(city_id);
        unit.home_order = self.next_unit_id;
        self.next_unit_id += 1;
        true
    }

    pub(super) fn release_city_support(&mut self, city_id: u32) {
        self.release_city_support_except(city_id, None);
    }

    pub(super) fn release_city_support_except(&mut self, city_id: u32, keep_owner: Option<usize>) {
        let mut lost = Vec::new();
        for unit in self
            .units
            .iter_mut()
            .filter(|unit| unit.home_city == Some(city_id) && Some(unit.owner) != keep_owner)
        {
            if let Some(city) = self.cities.iter().find(|city| {
                city.id != city_id
                    && city.owner == unit.owner
                    && (city.x, city.y) == (unit.x, unit.y)
            }) {
                unit.home_city = Some(city.id);
                unit.home_order = self.next_unit_id;
                self.next_unit_id += 1;
            } else {
                lost.push(unit.id);
            }
        }
        self.units.retain(|unit| {
            !lost.contains(&unit.id) && unit.transport.is_none_or(|id| !lost.contains(&id))
        });
        self.prune_unit_ai_state();
    }

    pub(super) fn pay_shield_support(
        &mut self,
        city: &City,
        gross: i32,
        upkeep: i32,
    ) -> (i32, i32) {
        let deficit = (upkeep - gross).max(0);
        let mut recovered = 0;
        if deficit > 0 {
            let mut supported: Vec<_> = self
                .units
                .iter()
                .filter(|unit| {
                    unit.owner == city.owner
                        && unit.home_city == Some(city.id)
                        && self.unit_shield_cost(unit) > 0
                })
                .cloned()
                .collect();
            supported.sort_by_key(|unit| std::cmp::Reverse(unit.home_order));
            for unit in supported {
                if self.city_unit_upkeep(city).shields <= gross {
                    break;
                }
                if !self.units.iter().any(|existing| existing.id == unit.id) {
                    continue;
                }
                // Automatic recovery targets the friendly city on the unit's tile.
                if let Some(target) = self.cities.iter_mut().find(|target| {
                    target.owner == unit.owner && (target.x, target.y) == (unit.x, unit.y)
                }) {
                    let cost = unit.kind.stats().cost;
                    if target.id == city.id {
                        recovered += cost - cost / 2;
                    } else {
                        target.shields += cost - cost / 2;
                    }
                }
                self.remove_unsupported_unit(unit.id, city, "shields");
            }
        }
        let remaining = if deficit > 0 {
            self.city_unit_upkeep(city).shields
        } else {
            upkeep
        };
        ((gross - remaining).max(0), recovered)
    }

    pub(super) fn remove_unsupported_unit(&mut self, id: u32, city: &City, resource: &str) {
        let Some(unit) = self.unit_by_id(id) else {
            return;
        };
        if city.owner == HUMAN {
            let message = format!(
                "{} could not supply {} for {}. The unit was lost.",
                city.name,
                resource,
                unit.kind.name()
            );
            self.add_event(message);
        }
        self.remove_units_with_cargo(vec![id]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn famine_takes_a_unit_charged_food_before_a_citizen() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.tile_mut(x, y).terrain = Terrain::Desert;
        game.create_city(HUMAN, x, y);
        let city_id = game.cities[0].id;
        assert!(game.apply(Command::Specialist(city_id, 0, 1)));

        let mut units = Vec::new();
        for _ in 0..5 {
            let id = game.spawn_unit(HUMAN, UnitKind::Settlers, x + 2, y);
            let unit = game.units.last_mut().unwrap();
            unit.home_city = Some(city_id);
            unit.home_order = id;
            units.push(id);
        }
        let support = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!((support.food, support.shields), (1, 0));
        assert_eq!(support.food_unit, Some(units[0]));
        game.process_cities();
        assert_eq!(game.cities[0].size, 1);
        assert!(!game.units.iter().any(|unit| unit.id == units[0]));
        assert_eq!(game.city_unit_upkeep(&game.cities[0]).food, 0);
    }
}
