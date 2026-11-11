//! Unit support and upkeep calculations.

use super::*;

pub(crate) struct UnitUpkeep {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) gold: i32,
}

impl Simulation {
    pub(crate) fn city_unit_upkeep(&self, city: &City) -> UnitUpkeep {
        UnitUpkeep {
            food: 0,
            shields: 0,
            gold: self
                .city_gold_upkeep(city)
                .iter()
                .map(|(_, cost)| cost)
                .sum(),
        }
    }

    pub(super) fn city_gold_upkeep(&self, city: &City) -> Vec<(u32, i32)> {
        let Some(policy) = self.players[city.owner].government.unit_support() else {
            return Vec::new();
        };
        let free_units = self
            .cities
            .iter()
            .filter(|candidate| candidate.owner == city.owner)
            .map(|candidate| match candidate.size {
                1..=6 => policy.town,
                7..=12 => policy.city,
                _ => policy.metropolis,
            })
            .sum::<i32>();
        let mut units: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == city.owner && unit.kind.requires_support())
            .collect();
        units.sort_by_key(|unit| unit.id);
        units
            .into_iter()
            .enumerate()
            .filter_map(|(index, unit)| {
                (unit.home_city == Some(city.id) && index as i32 >= free_units)
                    .then_some((unit.id, policy.cost_per_unit))
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
        if unit.kind.has_no_home() || unit.home_city.is_none() {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn despotism_support_is_treasury_paid_after_four_free_units_per_town() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.tile_mut(x, y).terrain = Terrain::Desert;
        game.create_city(HUMAN, x, y);
        let city_id = game.cities[0].id;
        game.units.clear();
        let mut units = Vec::new();
        for kind in [
            UnitKind::Settler,
            UnitKind::Worker,
            UnitKind::Warrior,
            UnitKind::Scout,
            UnitKind::Archer,
        ] {
            let id = game.spawn_unit(HUMAN, kind, x + 2, y);
            let unit = game.units.last_mut().unwrap();
            unit.home_city = Some(city_id);
            units.push(id);
        }
        assert_eq!(game.city_gold_upkeep(&game.cities[0]), vec![(units[4], 1)]);
        let upkeep = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!((upkeep.food, upkeep.shields, upkeep.gold), (0, 0, 1));
    }

    #[test]
    fn republic_uses_city_size_allowance_and_charges_two_gold_including_workers() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 10);
        game.create_city(HUMAN, x, y);
        game.players[HUMAN].government = Government::Republic;
        let city_id = game.cities[0].id;
        game.units.clear();
        let mut units = Vec::new();
        for kind in [UnitKind::Settler, UnitKind::Worker, UnitKind::Warrior] {
            let id = game.spawn_unit(HUMAN, kind, x + 2, y);
            game.units.last_mut().unwrap().home_city = Some(city_id);
            units.push(id);
        }
        assert_eq!(
            game.city_gold_upkeep(&game.cities[0]),
            vec![(units[1], 2), (units[2], 2)]
        );
    }
}
