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

    pub(super) fn airlift(&mut self, id: u32, destination: u32) -> bool {
        let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
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
        for city in &mut self.cities {
            if city.id == source || city.id == destination {
                city.airlift_used = true;
            }
        }
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
        if !(0..MAP_H).contains(&y) || !(1..=10).contains(&self.map_distance(unit.x, unit.y, x, y))
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

    pub(super) fn run_ai_paratrooper(&mut self, unit: &Unit) -> bool {
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
        let origin = labels[Self::idx(unit.x, unit.y)];
        let conquest = cities
            .filter(|city| self.at_war(owner, city.owner))
            .max_by_key(|city| {
                let overseas = labels[Self::idx(city.x, city.y)] != origin;
                (city.size + i32::from(overseas), std::cmp::Reverse(city.id))
            });
        if let Some(city) = conquest {
            return self.paradrop(unit.id, city.x, city.y);
        }

        let mut best = None;
        let mut best_value = 0.0;
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = Self::xy(index);
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
            for (tx, ty) in super::terrain_work::neighbors(x, y) {
                if !self.tile(tx, ty).known_by[owner]
                    || self.tile(tx, ty).terrain.is_water()
                    || self.treaty_blocks_move(&staged, tx, ty)
                {
                    continue;
                }
                let Some(defender) = self
                    .best_defender_id(&staged, tx, ty)
                    .and_then(|id| self.units.iter().find(|enemy| enemy.id == id))
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
                    best = Some((x, y, wrapped_map_vector(x, y, tx, ty)));
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

    pub(super) fn paradrop(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return false;
        };
        if !self.can_begin_paradrop(&unit) {
            return false;
        }
        let x = x.rem_euclid(MAP_W);
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
            if unit.owner == HUMAN {
                if let Some(paratroopers) = self.units.iter_mut().find(|unit| unit.id == id) {
                    (paratroopers.x, paratroopers.y) = (x, y);
                }
                self.refresh_visibility();
                self.add_event("Paratroopers were lost landing on enemy units.");
            }
            self.remove_units_with_cargo(vec![id]);
        } else {
            // The drop itself costs no movement, but enter_tile records the action.
            self.enter_tile(id, x, y, 0);
        }
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    pub(super) fn refuel_site(&self, owner: usize, x: i32, y: i32) -> bool {
        self.cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && self.allied(owner, city.owner))
            || (matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
                && self
                    .tile(x, y)
                    .owner
                    .is_none_or(|other| self.allied(owner, other)))
    }

    fn aboard_refueling_transport(&self, unit: &Unit) -> bool {
        unit.transport.is_some_and(|id| {
            self.units.iter().any(|ship| {
                ship.id == id
                    && ship.kind.can_carry(unit.kind)
                    && self.allied(unit.owner, ship.owner)
            })
        })
    }

    pub(super) fn position_ai_carrier(&mut self, carrier: &Unit) -> bool {
        let owner = carrier.owner;
        let deck = (carrier.x, carrier.y);
        let strike_aircraft = |unit: &Unit| {
            unit.owner == owner
                && matches!(
                    unit.kind,
                    UnitKind::Bomber
                        | UnitKind::StealthBomber
                        | UnitKind::Fighter
                        | UnitKind::StealthFighter
                )
        };
        if self.cargo_count(carrier.id) < carrier.kind.transport_capacity()
            && self
                .units
                .iter()
                .filter(|unit| strike_aircraft(unit))
                .any(|plane| {
                    plane.transport.is_none()
                        && !self.refuel_site(owner, plane.x, plane.y)
                        && self.map_distance(plane.x, plane.y, deck.0, deck.1) * MOVE_FRAGMENTS
                            <= self.unit_move_allowance(owner, plane.kind)
                })
        {
            return true;
        }

        let wing: Vec<_> = self
            .units
            .iter()
            .filter(|unit| strike_aircraft(unit) && unit.transport == Some(carrier.id))
            .collect();
        if wing.is_empty() {
            return false;
        }
        let range = wing
            .iter()
            .filter(|plane| plane.kind.bombard_rate() > 0 && plane.hp * 2 >= plane.kind.stats().hp)
            .map(|plane| self.unit_move_allowance(owner, plane.kind) / MOVE_FRAGMENTS)
            .max()
            .unwrap_or(0);
        if range == 0 || carrier.moves_left <= 0 {
            return true;
        }

        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|enemy| {
                self.at_war(owner, enemy.owner)
                    && !enemy.kind.is_aircraft()
                    && enemy.transport.is_none()
                    && enemy.hp > 1
                    && !self.tile(enemy.x, enemy.y).terrain.is_water()
                    && self.tile(enemy.x, enemy.y).known_by[owner]
            })
            .collect();
        if targets
            .iter()
            .any(|enemy| self.map_distance(deck.0, deck.1, enemy.x, enemy.y) <= range)
        {
            return true;
        }

        let mut best = None;
        let mut best_value = 0.0;
        for (index, tile) in self.tiles.iter().enumerate() {
            if !tile.terrain.is_water() {
                continue;
            }
            let (x, y) = Self::xy(index);
            let target_value = targets
                .iter()
                .filter_map(|enemy| {
                    let distance = self.map_distance(x, y, enemy.x, enemy.y);
                    (2..=range).contains(&distance).then(|| {
                        f64::from(enemy.kind.stats().cost) / (1.0 + 0.1 * f64::from(distance))
                    })
                })
                .fold(0.0_f64, f64::max);
            if target_value == 0.0
                || self.units.iter().any(|unit| {
                    self.at_war(owner, unit.owner)
                        && self.map_distance(x, y, unit.x, unit.y)
                            <= i32::from(unit.kind.is_naval())
                })
            {
                continue;
            }
            let min_steps = self.map_distance(deck.0, deck.1, x, y);
            if target_value / (1.0 + f64::from(min_steps)) <= best_value {
                continue;
            }
            let path = self.path_for_unit(carrier, x, y);
            if !path.is_empty() {
                let value = target_value / (1.0 + path.len() as f64);
                if value > best_value {
                    best_value = value;
                    best = Some(path);
                }
            }
        }
        if let Some(path) = best {
            self.follow_path(carrier.id, path);
        }
        true
    }

    pub(super) fn run_ai_strike_aircraft(&mut self, aircraft: &Unit) {
        let fighter = aircraft.kind.can_attack_aircraft();
        let position = (aircraft.x, aircraft.y);
        let mut bases = Vec::new();
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = Self::xy(index);
            if self.refuel_site(aircraft.owner, x, y)
                && !self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (x, y) && !self.allied(aircraft.owner, unit.owner)
                })
            {
                bases.push((x, y));
            }
        }
        let decks: Vec<_> = self
            .units
            .iter()
            .filter(|ship| {
                ship.kind == UnitKind::Carrier
                    && self.allied(aircraft.owner, ship.owner)
                    && (aircraft.transport == Some(ship.id)
                        || self.cargo_count(ship.id) < ship.kind.transport_capacity())
            })
            .map(|ship| ((ship.x, ship.y), ship.id))
            .collect();
        for &(tile, _) in &decks {
            if !bases.contains(&tile) {
                bases.push(tile);
            }
        }
        let on_base = self.refuel_site(aircraft.owner, position.0, position.1)
            || self.aboard_refueling_transport(aircraft);
        if on_base && aircraft.hp * 2 < aircraft.kind.stats().hp {
            return;
        }
        let allowance = self.unit_move_allowance(aircraft.owner, aircraft.kind);
        let mut best = None;
        let mut best_value = 0.0;
        for enemy in self.units.iter().filter(|unit| {
            self.at_war(aircraft.owner, unit.owner)
                && unit.transport.is_none()
                && (if fighter {
                    unit.kind.is_aircraft()
                        && self.best_defender_id(aircraft, unit.x, unit.y) == Some(unit.id)
                } else {
                    !unit.kind.is_aircraft() && !self.tile(unit.x, unit.y).terrain.is_water()
                })
                && self.tile(unit.x, unit.y).known_by[aircraft.owner]
                && self.map_distance(aircraft.x, aircraft.y, unit.x, unit.y) * MOVE_FRAGMENTS
                    <= aircraft.moves_left + MOVE_FRAGMENTS
        }) {
            if self.units.iter().any(|unit| {
                (unit.x, unit.y) == (enemy.x, enemy.y) && !self.at_war(aircraft.owner, unit.owner)
            }) || self.cities.iter().any(|city| {
                (city.x, city.y) == (enemy.x, enemy.y) && !self.at_war(aircraft.owner, city.owner)
            }) {
                continue;
            }
            for (x, y) in super::terrain_work::neighbors(enemy.x, enemy.y) {
                let at = (x, y);
                if self.map_distance(aircraft.x, aircraft.y, x, y) * MOVE_FRAGMENTS
                    >= aircraft.moves_left
                    || self.units.iter().any(|unit| {
                        (unit.x, unit.y) == at && !self.allied(aircraft.owner, unit.owner)
                    })
                    || self.cities.iter().any(|city| {
                        (city.x, city.y) == at && !self.allied(aircraft.owner, city.owner)
                    })
                {
                    continue;
                }
                let path = self.path_for_unit(aircraft, x, y);
                let remaining = aircraft.moves_left - path.len() as i32 * MOVE_FRAGMENTS;
                if (at != position && path.is_empty()) || remaining <= 0 {
                    continue;
                }
                let mut staged = aircraft.clone();
                (staged.x, staged.y) = at;
                staged.moves_left = remaining;
                let after_attack = if fighter {
                    (remaining - MOVE_FRAGMENTS).max(0)
                } else {
                    0
                };
                let return_budget = if aircraft.fuel <= 1 {
                    after_attack
                } else {
                    allowance.max(after_attack)
                };
                let recovery = bases
                    .iter()
                    .filter_map(|&(bx, by)| {
                        if (bx, by) == at {
                            return Some((Vec::new(), (bx, by)));
                        }
                        if self.map_distance(x, y, bx, by) * MOVE_FRAGMENTS > return_budget {
                            return None;
                        }
                        let path = self.path_for_unit(&staged, bx, by);
                        (!path.is_empty() && path.len() as i32 * MOVE_FRAGMENTS <= return_budget)
                            .then_some((path, (bx, by)))
                    })
                    .min_by_key(|(path, _)| path.len());
                let Some((return_path, recovery)) = recovery else {
                    continue;
                };
                let attack = Self::attack_power(&staged);
                let defense = self.defense_power_against(enemy, enemy.x, enemy.y, aircraft.kind);
                let (firepower, retaliation) = self.combat_firepower(&staged, enemy);
                let profit = if fighter {
                    let chance = combat_win_chance(
                        attack,
                        aircraft.hp,
                        firepower,
                        defense,
                        enemy.hp,
                        retaliation,
                    );
                    let cost = f64::from(aircraft.kind.stats().cost * aircraft.kind.stats().hp)
                        / f64::from(aircraft.hp.max(1));
                    chance * f64::from(enemy.kind.stats().cost) - (1.0 - chance) * cost
                } else {
                    let damage = (f64::from(aircraft.kind.bombard_rate() * firepower * attack)
                        / f64::from((attack + defense).max(1)))
                    .min(f64::from((enemy.hp - 1).max(0)));
                    damage * f64::from(enemy.kind.stats().cost) / f64::from(enemy.kind.stats().hp)
                };
                let value = profit / (1.0 + 0.1 * (path.len() + return_path.len()) as f64);
                if value > best_value {
                    best_value = value;
                    best = Some((at, path, (enemy.x, enemy.y), return_path, recovery));
                }
            }
        }
        if let Some((at, path, target, return_path, recovery)) = best {
            self.follow_path(aircraft.id, path);
            if self
                .units
                .iter()
                .any(|unit| unit.id == aircraft.id && (unit.x, unit.y) == at)
            {
                let (dx, dy) = wrapped_map_vector(at.0, at.1, target.0, target.1);
                if self
                    .units
                    .iter()
                    .any(|unit| unit.id == aircraft.id && unit.transport.is_some())
                    && !self.board_or_deboard(aircraft.id)
                {
                    return;
                }
                self.try_move_unit(aircraft.id, dx, dy);
                if fighter {
                    self.follow_path(aircraft.id, return_path);
                }
                if let Some(&(_, carrier)) = decks.iter().find(|&&(tile, _)| tile == recovery) {
                    self.board_specific_ship(aircraft.id, carrier);
                }
            }
        } else if !on_base {
            let route = bases
                .into_iter()
                .filter_map(|(x, y)| {
                    let path = self.path_for_unit(aircraft, x, y);
                    ((x, y) == position || !path.is_empty()).then_some((path, (x, y)))
                })
                .min_by_key(|(path, _)| path.len());
            if let Some((path, destination)) = route {
                self.follow_path(aircraft.id, path);
                if let Some(&(_, carrier)) = decks.iter().find(|&&(tile, _)| tile == destination) {
                    self.board_specific_ship(aircraft.id, carrier);
                }
            }
        }
    }

    pub(super) fn try_bombard(&mut self, attacker: &Unit, x: i32, y: i32) -> MoveResult {
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && !self.at_war(attacker.owner, city.owner))
            || self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == (x, y) && !self.at_war(attacker.owner, unit.owner))
        {
            return MoveResult::Blocked;
        }
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                (unit.x, unit.y) == (x, y) && !unit.kind.is_aircraft() && unit.transport.is_none()
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            return MoveResult::Blocked;
        }
        let attack = Self::attack_power(attacker);
        let mut damage = 0;
        for target in &targets {
            let defense = self.defense_power_against(target, x, y, attacker.kind);
            let hits = (0..attacker.kind.bombard_rate())
                .filter(|_| self.rand_below(attack + defense) >= defense)
                .count() as i32;
            let (firepower, _) = self.combat_firepower(attacker, target);
            let defender = self
                .units
                .iter_mut()
                .find(|unit| unit.id == target.id)
                .unwrap();
            let hp = (defender.hp - hits * firepower).max(1);
            damage += defender.hp - hp;
            defender.hp = hp;
        }
        let bomber = self
            .units
            .iter_mut()
            .find(|unit| unit.id == attacker.id)
            .unwrap();
        bomber.moves_left = 0;
        bomber.moved = true;
        bomber.state = UnitState::Idle;
        if attacker.owner == HUMAN || targets.iter().any(|unit| unit.owner == HUMAN) {
            self.add_event(format!(
                "A {} dealt {damage} damage to {} units.",
                attacker.kind.name(),
                targets.len()
            ));
        }
        MoveResult::Fought
    }

    pub(super) fn restore_aircraft(&mut self) {
        let aircraft: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.kind.is_aircraft())
            .map(|unit| unit.id)
            .collect();
        for id in aircraft {
            let unit = self.units.iter().find(|unit| unit.id == id).cloned();
            let Some(unit) = unit else { continue };
            let max_fuel = unit.kind.fuel_capacity();
            let max_hp = unit.kind.stats().hp;
            let airborne_loss = if unit.kind.is_missile() {
                0
            } else {
                percent_of(max_hp, 10)
            };
            let site = self.refuel_site(unit.owner, unit.x, unit.y);
            if site || self.aboard_refueling_transport(&unit) {
                let city = self
                    .cities
                    .iter()
                    .find(|city| (city.x, city.y) == (unit.x, unit.y));
                let percent = if let Some(city) = city {
                    if city.has_building(Building::AIRPORT) {
                        100
                    } else {
                        33
                    }
                } else if self.tile(unit.x, unit.y).base == Some(Base::Airbase) {
                    34
                } else if site {
                    10
                } else {
                    0
                };
                let bomber = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                bomber.fuel = max_fuel;
                if !unit.moved {
                    bomber.hp = (bomber.hp + percent_of(max_hp, percent)).min(max_hp);
                }
                continue;
            }
            if unit.fuel == 1
                && unit.hp > airborne_loss
                && self.boarding_target(&unit, unit.x, unit.y).is_some()
            {
                self.board_or_deboard(id);
                self.units
                    .iter_mut()
                    .find(|unit| unit.id == id)
                    .unwrap()
                    .fuel = max_fuel;
                continue;
            }
            if unit.fuel == 1 && unit.hp > airborne_loss && unit.moves_left > 0 {
                let sites: Vec<_> = self
                    .tiles
                    .iter()
                    .enumerate()
                    .filter_map(|(index, _)| {
                        let (x, y) = Self::xy(index);
                        let distance = self.map_distance(unit.x, unit.y, x, y);
                        (distance * MOVE_FRAGMENTS <= unit.moves_left
                            && (self.refuel_site(unit.owner, x, y)
                                || self.boarding_target(&unit, x, y).is_some())
                            && self.best_defender_id(&unit, x, y).is_none())
                        .then_some((distance, x, y))
                    })
                    .collect();
                let nearest = sites
                    .into_iter()
                    .filter_map(|(_, x, y)| {
                        let path = self.path_to(id, x, y);
                        (!path.is_empty() && path.len() as i32 * MOVE_FRAGMENTS <= unit.moves_left)
                            .then_some((path.len(), x, y))
                    })
                    .min();
                if let Some((_, x, y)) = nearest {
                    self.goto_unit(id, x, y);
                }
            }
            let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
                continue;
            };
            let unit = &self.units[index];
            let refueled = self.refuel_site(unit.owner, unit.x, unit.y)
                || self.aboard_refueling_transport(unit);
            let unit = &mut self.units[index];
            unit.hp -= airborne_loss;
            unit.fuel = if refueled {
                max_fuel
            } else {
                unit.fuel.saturating_sub(1)
            };
            if (max_fuel > 0 && unit.fuel == 0) || unit.hp <= 0 {
                let unit = unit.clone();
                self.remove_units_with_cargo(vec![id]);
                if unit.owner == HUMAN {
                    self.add_event(format!("Your {} crashed.", unit.kind.name()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submarines_refuel_missiles_and_exclude_other_cargo() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (submarine, missile, _, doubled) = game.submarine_playtest_setup();
        assert!(doubled);
        assert_eq!(
            UnitKind::Submarine.required_tech().unwrap().name(),
            "Combustion"
        );
        assert!(!UnitKind::Submarine.can_carry(UnitKind::Fighter));
        assert!(!UnitKind::Submarine.can_carry(UnitKind::Warrior));
        assert!(UnitKind::Submarine.can_carry(UnitKind::Nuclear));
        assert!(game.board_or_deboard(missile));
        game.restore_aircraft();
        assert!(
            game.units.iter().any(|unit| unit.id == missile
                && unit.transport == Some(submarine)
                && unit.fuel == 1)
        );
    }

    #[test]
    fn airlift_uses_airport_capacity_and_resets_next_turn() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (first, second, destination) = game.airlift_playtest_setup();
        assert!(game.apply(Command::Airlift(first, destination)));
        assert!(!game.apply(Command::Airlift(second, destination)));
        assert!(game.apply(Command::EndTurn));
        assert!(game.apply(Command::Airlift(second, destination)));
    }

    #[test]
    fn cruise_missiles_get_city_firepower_and_face_sdi_instead_of_sam() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (bomber, defender) = game.sam_battery_playtest_setup(true);
        let attacker = game
            .units
            .iter_mut()
            .find(|unit| unit.id == bomber)
            .unwrap();
        attacker.kind = UnitKind::CruiseMissile;
        let attacker = attacker.clone();
        let target = game
            .units
            .iter()
            .find(|unit| unit.id == defender)
            .unwrap()
            .clone();
        let defense = |game: &Simulation| {
            game.defense_power_against(&target, target.x, target.y, attacker.kind)
        };
        assert_eq!(defense(&game), 45);
        game.cities
            .last_mut()
            .unwrap()
            .buildings
            .push(Building::SDI_DEFENSE);
        assert_eq!(defense(&game), 90);
        assert_eq!(game.combat_firepower(&attacker, &target), (2, 1));
        assert_eq!(attacker.kind.fuel_capacity(), 1);
    }

    #[test]
    fn aegis_cruisers_defend_against_air_attackers_and_sail_without_cargo() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (_, id) = game.aegis_playtest_setup();
        let cruiser = game.units.iter().find(|unit| unit.id == id).unwrap();
        let (x, y) = (cruiser.x, cruiser.y);
        let defense = |attacker| game.defense_power_against(cruiser, x, y, attacker);
        assert_eq!(defense(UnitKind::Caravel), 80);
        assert_eq!(defense(UnitKind::Fighter), 400);
        assert_eq!(defense(UnitKind::Helicopter), 400);
        assert_eq!(cruiser.kind.transport_capacity(), 0);
        assert_eq!(cruiser.kind.required_tech(), Some(Tech::ROCKETRY));
        assert!(!UnitKind::Transport.can_carry(cruiser.kind));
        assert_eq!(game.movement_cost_for(cruiser, x, y), Some(MOVE_FRAGMENTS));
        let (land_x, land_y) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(land_x, land_y).terrain = Terrain::Grassland;
        let cruiser = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!(game.movement_cost_for(cruiser, land_x, land_y), None);
    }

    #[test]
    fn city_defenses_use_the_attacking_units_class() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (_, defender_id) = game.sam_battery_playtest_setup(true);
        let defender = game
            .units
            .iter()
            .find(|unit| unit.id == defender_id)
            .unwrap()
            .clone();
        let city = game.cities.last_mut().unwrap();
        city.buildings.extend([
            Building::COASTAL_DEFENSE,
            Building::CITY_WALLS,
            Building::GREAT_WALL,
        ]);

        let defense = |game: &Simulation, attacker| {
            game.defense_power_against(&defender, defender.x, defender.y, attacker)
        };
        assert_eq!(defense(&game, UnitKind::Bomber), 90);
        assert_eq!(defense(&game, UnitKind::Helicopter), 90);
        assert_eq!(defense(&game, UnitKind::Caravel), 90);
        assert_eq!(defense(&game, UnitKind::Musketeers), 135);
        game.players[defender.owner].known.push(Tech::MACHINE_TOOLS);
        assert_eq!(defense(&game, UnitKind::Musketeers), 112);
    }

    #[test]
    fn helicopters_carry_field_capable_troops_and_crash_only_when_health_runs_out() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        for (tx, ty) in [(x, y), (nx, ny)] {
            let tile = game.tile_mut(tx, ty);
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.hut = false;
        }
        let helicopter = game.spawn_unit(HUMAN, UnitKind::Helicopter, x, y);
        let musketeer = game.spawn_unit(HUMAN, UnitKind::Musketeers, x, y);
        let partisan = game.spawn_unit(HUMAN, UnitKind::Partisan, x, y);
        assert!(!game.board_or_deboard(musketeer));
        assert!(game.board_or_deboard(partisan));
        assert!(game.apply(Command::Move(helicopter, 1, 0)));
        assert!(game.board_or_deboard(partisan));
        assert!(game.units.iter().any(|unit| unit.id == partisan
            && (unit.x, unit.y) == (nx, ny)
            && unit.transport.is_none()));
        assert!(game.board_or_deboard(partisan));
        for _ in 0..9 {
            game.restore_aircraft();
        }
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == helicopter && unit.hp == 3)
        );
        game.restore_aircraft();
        assert!(!game.units.iter().any(|unit| unit.id == helicopter));
        assert!(game.units.iter().any(|unit| unit.id == partisan
            && (unit.x, unit.y) == (nx, ny)
            && unit.transport.is_none()));
    }

    #[test]
    fn bombardment_is_nonlethal_and_does_not_move_the_bomber() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (bomber, targets) = game.bombard_playtest_setup();
        let first = game
            .units
            .iter_mut()
            .find(|unit| unit.id == targets[0])
            .unwrap();
        first.hp = 2;
        assert!(game.apply(Command::Move(bomber, 1, 0)));
        assert_eq!(
            game.units
                .iter()
                .find(|unit| unit.id == targets[0])
                .unwrap()
                .hp,
            1
        );
        let aircraft = game.units.iter().find(|unit| unit.id == bomber).unwrap();
        assert_eq!(
            (aircraft.x, aircraft.y, aircraft.hp, aircraft.moves_left),
            (10, 20, 30, 0)
        );
    }

    #[test]
    fn aircraft_can_return_to_airstrips_or_crash_when_out_of_fuel() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, -1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        game.tile_mut(nx, ny).base = Some(Base::Airstrip);
        let first = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        let second = game.spawn_unit(HUMAN, UnitKind::Bomber, x, y);
        for unit in game
            .units
            .iter_mut()
            .filter(|unit| [first, second].contains(&unit.id))
        {
            unit.fuel = 1;
            if unit.id == second {
                unit.moves_left = 0;
            }
        }
        game.restore_aircraft();
        let saved = game.units.iter().find(|unit| unit.id == first).unwrap();
        assert_eq!((saved.x, saved.y, saved.fuel), (nx, ny, 3));
        assert!(!game.units.iter().any(|unit| unit.id == second));
    }
}

#[cfg(test)]
mod fighter_tests {
    use super::*;

    #[test]
    fn fighter_can_attack_an_airborne_bomber_and_refuels_to_two_turns() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (fighter, bomber) = game.fighter_playtest_setup();
        let warrior = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        assert!(!game.apply(Command::Move(warrior, 1, 0)));
        assert!(game.apply(Command::Move(fighter, 1, 0)));
        assert!(!game.units.iter().any(|unit| unit.id == bomber));
        let aircraft = game
            .units
            .iter_mut()
            .find(|unit| unit.id == fighter)
            .unwrap();
        assert_eq!(aircraft.fuel, 2);
        aircraft.fuel = 1;
        game.tile_mut(10, 20).base = Some(Base::Airbase);
        game.restore_aircraft();
        let aircraft = game.units.iter().find(|unit| unit.id == fighter).unwrap();
        assert_eq!(aircraft.fuel, 2);
    }
}

#[cfg(test)]
mod carrier_tests {
    use super::*;

    #[test]
    fn carrier_production_and_emergency_landing_follow_aircraft_cargo_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (carrier, fighter) = game.carrier_playtest_setup();
        let port = Production::Unit(UnitKind::Carrier);
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Grassland;
        game.create_city(HUMAN, x, y);
        assert!(!game.can_produce(&game.cities[0], port));
        game.players[HUMAN].known.push(Tech::ADVANCED_FLIGHT);
        assert!(game.can_produce(&game.cities[0], port));
        game.cities.clear();
        assert!(UnitKind::Carrier.can_carry(UnitKind::Warrior));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Cannon));
        assert!(!UnitKind::Carrier.can_carry(UnitKind::Worker));
        assert!(!UnitKind::Transport.can_carry(UnitKind::Fighter));
        game.restore_aircraft();
        let saved = game.units.iter().find(|unit| unit.id == fighter).unwrap();
        assert_eq!((saved.transport, saved.fuel), (Some(carrier), 2));
    }
}

#[cfg(test)]
mod stealth_tests {
    use super::*;

    #[test]
    fn stealth_aircraft_replace_older_models_and_need_close_detection() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (explorer, hidden) = game.stealth_playtest_setup();
        game.units.last_mut().unwrap().kind = UnitKind::StealthFighter;
        assert!(!game.unit_visible_to_human(game.units.last().unwrap()));
        assert!(game.apply(Command::Move(explorer, 1, 0)));
        let aircraft = game.units.iter().find(|unit| unit.id == hidden).unwrap();
        assert!(game.unit_visible_to_human(aircraft));

        game.create_city(HUMAN, 10, 20);
        let city = &game.cities[0];
        game.players[HUMAN]
            .known
            .extend([Tech::FLIGHT, Tech::ADVANCED_FLIGHT, Tech::STEALTH]);
        for (old, new) in [
            (UnitKind::Fighter, UnitKind::StealthFighter),
            (UnitKind::Bomber, UnitKind::StealthBomber),
        ] {
            assert!(!game.can_produce(city, Production::Unit(old)));
            assert!(game.can_produce(city, Production::Unit(new)));
            assert_eq!(game.production_successor(city, old), Some(new));
            assert_eq!(new.fuel_capacity(), old.fuel_capacity());
        }
    }
}
