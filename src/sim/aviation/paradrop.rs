//! Paratrooper launch and landing rules.

use super::*;

impl Simulation {
    fn airlift_origin(&self, unit: &Unit) -> Option<&City> {
        if unit.kind.is_naval() || unit.moves_left <= 0 || self.cargo_count(unit.id) != 0 {
            return None;
        }
        self.cities.iter().find(|city| {
            (city.x, city.y) == (unit.x, unit.y)
                && city.owner == unit.owner
                && city.has_building(Building::AIRPORT)
                && !city.airlift_used
        })
    }

    pub(crate) fn can_airlift(&self, unit: &Unit, destination: &City) -> bool {
        destination.owner == unit.owner
            && self
                .airlift_origin(unit)
                .is_some_and(|source| source.id != destination.id)
    }

    pub(in crate::sim) fn airlift(&mut self, id: u32, destination: u32) -> bool {
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        let unit = &self.units[index];
        let Some(source) = self.airlift_origin(unit).map(|city| city.id) else {
            return false;
        };
        let Some(city) = self
            .cities
            .iter()
            .find(|city| city.id == destination && city.owner == unit.owner && city.id != source)
        else {
            return false;
        };
        let (x, y, name) = (city.x, city.y, city.name.clone());
        let unit = &mut self.units[index];
        let owner = unit.owner;
        (unit.x, unit.y) = (x, y);
        unit.transport = None;
        unit.moves_left = 0;
        unit.moved = true;
        unit.state = UnitState::Idle;
        unit.clear_work();
        self.list_order.unit(unit);
        self.record_transport_change(id, None);
        for city in &mut self.cities {
            if city.id == source || city.id == destination {
                city.airlift_used = true;
            }
        }
        self.refresh_diplomatic_contact_at(owner, x, y);
        self.refresh_visibility();
        if owner == HUMAN {
            self.add_event(format!("A unit was airlifted to {name}."));
        }
        true
    }

    pub(crate) fn can_begin_paradrop(&self, unit: &Unit) -> bool {
        unit.kind == UnitKind::Paratroopers
            && unit.moves_left >= MOVE_FRAGMENTS
            && !unit.moved
            && unit.work_left == 0
            && unit.transport.is_none()
            && self.refuel_site(unit.owner, unit.x, unit.y)
    }

    fn paradrop_tile_legal(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !(0..self.map_wrap.height()).contains(&y)
            || !(1..=10).contains(&self.map_distance(unit.x, unit.y, x, y))
        {
            return false;
        }
        let target = self.tile(x, y);
        target.known_by[unit.owner]
            && !target.terrain.is_water()
            && !target.owner.is_some_and(|owner| {
                owner != unit.owner && self.pact(unit.owner, owner) == Pact::Peace
            })
            && !self.cities.iter().any(|city| {
                (city.x, city.y) == (x, y)
                    && !self.allied(unit.owner, city.owner)
                    && !self.at_war(unit.owner, city.owner)
            })
    }

    pub(in crate::sim) fn run_ai_paratrooper(&mut self, unit: &Unit) -> bool {
        if !self.can_begin_paradrop(unit) || unit.hp * 2 < unit.kind.stats().hp {
            return false;
        }
        let owner = unit.owner;
        if self
            .cities
            .iter()
            .any(|city| city.owner == owner && (city.x, city.y) == (unit.x, unit.y))
            && !self.units.iter().any(|guard| {
                guard.id != unit.id
                    && guard.owner == owner
                    && guard.kind.is_military()
                    && guard.transport.is_none()
                    && (guard.x, guard.y) == (unit.x, unit.y)
            })
        {
            return false;
        }

        let empty = |x, y| self.units.iter().all(|other| (other.x, other.y) != (x, y));
        let cities = self
            .cities
            .iter()
            .filter(|city| self.paradrop_tile_legal(unit, city.x, city.y) && empty(city.x, city.y));
        let threatened = cities
            .clone()
            .filter(|city| city.owner == owner)
            .filter_map(|city| {
                let danger = self
                    .city_threats(city)
                    .map(|(_, danger)| danger)
                    .sum::<i32>();
                (danger > 0).then_some((city.size * danger, city))
            })
            .max_by_key(|&(score, city)| (score, std::cmp::Reverse(city.id)));
        if let Some((_, city)) = threatened {
            return self.paradrop(unit.id, city.x, city.y);
        }
        let labels = self.continent_labels();
        let origin = labels[self.map_wrap.index(unit.x, unit.y)];
        let conquest = cities
            .filter(|city| self.at_war(owner, city.owner))
            .max_by_key(|city| {
                let overseas = labels[self.map_wrap.index(city.x, city.y)] != origin;
                (city.size + i32::from(overseas), std::cmp::Reverse(city.id))
            });
        if let Some(city) = conquest {
            return self.paradrop(unit.id, city.x, city.y);
        }

        let mut best = None;
        let mut best_value = 0.0;
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = self.map_wrap.position(index);
            if !empty(x, y)
                || !self.paradrop_tile_legal(unit, x, y)
                || self
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner))
            {
                continue;
            }
            let mut staged = unit.clone();
            (staged.x, staged.y) = (x, y);
            for (tx, ty) in self.map_wrap.neighbors(x, y) {
                if !self.tile(tx, ty).known_by[owner]
                    || self.tile(tx, ty).terrain.is_water()
                    || self.treaty_blocks_move(&staged, tx, ty)
                {
                    continue;
                }
                let Some(defender) = self
                    .best_defender_id(&staged, tx, ty)
                    .and_then(|id| self.unit_by_id(id))
                else {
                    continue;
                };
                let (attack_fp, defense_fp) = self.combat_firepower(&staged, defender);
                let chance = combat_win_chance(
                    Self::attack_power(&staged),
                    staged.hp,
                    attack_fp,
                    self.defense_power_against(defender, tx, ty, staged.kind),
                    defender.hp,
                    defense_fp,
                );
                let threatened_hp: i32 = self
                    .combat_stack_losses(defender, &staged)
                    .filter(|unit| unit.transport.is_none())
                    .map(|unit| unit.hp)
                    .sum();
                let value = chance * f64::from(threatened_hp * 100) - f64::from(staged.hp * 100)
                    + f64::from(self.tile(x, y).terrain.defense_bonus() / 10);
                if value > best_value {
                    best_value = value;
                    best = Some((x, y, self.map_wrap.vector(x, y, tx, ty)));
                }
            }
        }
        if let Some((x, y, (dx, dy))) = best
            && self.paradrop(unit.id, x, y)
        {
            self.try_move_unit(unit.id, dx, dy);
            return true;
        }
        false
    }

    pub(in crate::sim) fn paradrop(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_begin_paradrop(&unit) {
            return false;
        }
        let x = x.rem_euclid(self.map_wrap.width());
        if !self.paradrop_tile_legal(&unit, x, y) {
            if unit.owner == HUMAN {
                self.add_event("Paratroopers need a known land tile and permission to enter.");
            }
            return false;
        }
        let occupiers: Vec<_> = self
            .units
            .iter()
            .filter(|enemy| (enemy.x, enemy.y) == (x, y) && !self.allied(unit.owner, enemy.owner))
            .collect();
        if !occupiers.is_empty() {
            if unit.owner == HUMAN
                && occupiers
                    .iter()
                    .any(|enemy| self.unit_visible_to_human(enemy))
            {
                self.add_event("Visible enemy units block the landing.");
                return false;
            }
            self.refresh_diplomatic_contact_at(unit.owner, x, y);
            if unit.owner == HUMAN {
                if let Some(paratroopers) = self.units.iter_mut().find(|unit| unit.id == id) {
                    (paratroopers.x, paratroopers.y) = (x, y);
                    self.list_order.unit(paratroopers);
                }
                self.refresh_visibility();
                self.add_event("Paratroopers were lost landing on enemy units.");
            }
            self.remove_units_with_cargo(vec![id]);
        } else {
            // The drop itself costs no movement, but enter_tile records the action.
            self.enter_tile(id, x, y, 0, None);
        }
        self.refresh_visibility();
        self.check_game_over();
        true
    }
}
