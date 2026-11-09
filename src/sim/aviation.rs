//! Aircraft, fuel, airlift, and paradrop rules.

use super::*;

mod aircraft_ai;
mod paradrop;

impl Simulation {
    pub(super) fn refuel_site(&self, owner: usize, x: i32, y: i32) -> bool {
        self.cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && self.allied(owner, city.owner))
            || (matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
                && self.tile(x, y).known_by[owner])
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

    pub(super) fn run_source_air_ferry(&mut self, carrier: &Unit) -> (bool, Vec<u32>) {
        let mut done = Vec::new();
        if carrier.moves_left <= 0 {
            return (true, done);
        }
        if carrier.hp < carrier.kind.stats().hp
            && self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (carrier.x, carrier.y))
        {
            return (true, done);
        }
        self.refresh_ferry_boss(carrier.id);
        loop {
            let Some(boss) = self
                .ferry_boss(carrier.id)
                .and_then(|id| self.unit_by_id(id))
                .cloned()
            else {
                return (false, done);
            };
            if self
                .map_wrap
                .civ3_distance(carrier.x, carrier.y, boss.x, boss.y)
                > 1
            {
                // Native temporarily abandons a distant boss and falls back
                // to pickup/exploration if no cargo replaces it. The restored
                // reservation must not activate that remote passenger.
                return (false, done);
            }
            // Land passengers retain their ordinary founder/worker/military
            // dispatch; fueled cargo uses the same manager as a land airbase.
            if boss.kind.fuel_capacity() == 0 {
                return (false, done);
            }
            if boss.moves_left > 0 && !self.ai_guard_targets.contains_key(&boss.id) {
                self.run_ai_strike_aircraft(&boss);
            }
            let Some(ship) = self
                .units
                .iter()
                .find(|unit| unit.id == carrier.id)
                .cloned()
            else {
                return (true, done);
            };
            if self
                .units
                .iter()
                .any(|unit| unit.id == boss.id && (unit.x, unit.y) == (ship.x, ship.y))
            {
                // daiferry.c: the boss remains on the boat, so hold station
                // and mark it done for the owner's remaining unit loop.
                done.push(boss.id);
                return (true, done);
            }
            if ship.moves_left <= 0 {
                return (true, done);
            }
            if self.cargo_count(ship.id) == 0 {
                return (false, done);
            }
            self.release_ai_ferry(boss.id);
            self.refresh_ferry_boss(ship.id);
        }
    }

    // daiair.c: find_something_to_bomb, dai_evaluate_tile_for_air_attack.
    // All legally attackable enemy stacks compete, not just aircraft for fighters.
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
            if self.visual_events.enabled {
                let hp_steps = (hp..=target.hp).rev().map(|hp| (attacker.hp, hp)).collect();
                self.publish_visual_event(VisualEvent::Combat(Box::new(
                    presentation::CombatVisual {
                        attacker: attacker.clone(),
                        defender: target.clone(),
                        hp: hp_steps,
                        city: self.city_at(x, y).cloned(),
                        territory: (0..self.map_wrap.width() * self.map_wrap.height())
                            .map(|i| {
                                self.displayed_tile_owner(
                                    i % self.map_wrap.width(),
                                    i / self.map_wrap.width(),
                                )
                            })
                            .collect(),
                    },
                )));
            }
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
        for owner in 0..self.players.len() {
            let ids: Vec<_> = self
                .player_units_ordered(owner)
                .filter(|unit| unit.kind.is_aircraft())
                .map(|unit| unit.id)
                .collect();
            for id in ids {
                self.restore_aircraft_unit(id);
            }
            self.finish_aircraft_fuel_deaths(owner);
        }
    }

    pub(super) fn finish_aircraft_fuel_deaths(&mut self, owner: usize) {
        let dead: Vec<_> = self
            .player_units_ordered(owner)
            .filter(|unit| unit.kind.fuel_capacity() > 0 && unit.fuel == 0)
            .map(|unit| (unit.id, unit.kind))
            .collect();
        for (id, kind) in dead {
            if !self.units.iter().any(|unit| unit.id == id) {
                continue;
            }
            self.remove_units_with_cargo(vec![id]);
            if owner == HUMAN {
                self.add_event(format!("Your {} crashed.", kind.name()));
            }
        }
    }

    pub(super) fn restore_aircraft_unit(&mut self, id: u32) {
        let unit = self.unit_by_id(id).cloned();
        let Some(mut unit) = unit else { return };
        let max_fuel = unit.kind.fuel_capacity();
        let max_hp = unit.kind.stats().hp;
        let airborne_loss = if unit.kind.is_missile() {
            0
        } else {
            percent_of(max_hp, 10)
        };
        let site = self.refuel_site(unit.owner, unit.x, unit.y);
        if site || self.aboard_refueling_transport(&unit) {
            let gain = self.aircraft_repair_gain(&unit, unit.x, unit.y);
            let bomber = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
            bomber.fuel = max_fuel;
            bomber.moved = false;
            if !unit.moved {
                bomber.hp = (bomber.hp + gain).min(max_hp);
            }
            return;
        }
        // player_restore_units restores hit points (including airborne
        // loss), kills dead actors, then attempts last-fuel rescue.
        unit.hp -= airborne_loss;
        self.units
            .iter_mut()
            .find(|actor| actor.id == id)
            .unwrap()
            .hp = unit.hp;
        self.units
            .iter_mut()
            .find(|actor| actor.id == id)
            .unwrap()
            .moved = false;
        if unit.hp <= 0 {
            self.remove_units_with_cargo(vec![id]);
            if unit.owner == HUMAN {
                self.add_event(format!("Your {} crashed.", unit.kind.name()));
            }
            return;
        }
        if max_fuel > 0 && unit.fuel <= 1 && self.boarding_target(&unit, unit.x, unit.y).is_some() {
            self.board_or_deboard(id);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .fuel = max_fuel;
            return;
        }
        if max_fuel > 0
            && unit.fuel <= 1
            && unit.moves_left > 0
            && let Some(mut map) =
                super::fuel_paths::FuelPaths::new(self, &unit, self.players[unit.owner].ai)
        {
            let destination = map
                .reachable_within(self, unit.moves_left, false, true)
                .into_iter()
                .find(|&(x, y, _)| {
                    !self
                        .tile_units_ordered(x, y)
                        .any(|other| !self.allied(unit.owner, other.owner))
                        && (self.refuel_site(unit.owner, x, y)
                            || self.boarding_target(&unit, x, y).is_some())
                });
            if let Some((x, y, _)) = destination
                && let Some(path) = map.path(self, x, y)
            {
                self.follow_source_ai_path(id, path);
                if let Some(live) = self.unit_by_id(id).cloned() {
                    if (live.x, live.y) == (x, y)
                        && self.boarding_target(&live, x, y).is_some()
                        && !self.refuel_site(live.owner, x, y)
                    {
                        self.board_or_deboard(id);
                    }
                    if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
                        actor.state = UnitState::Idle;
                    }
                }
            }
        }

        let Some(index) = self.unit_index_by_id(id) else {
            return;
        };
        let unit = &self.units[index];
        let refueled =
            self.refuel_site(unit.owner, unit.x, unit.y) || self.aboard_refueling_transport(unit);
        let unit = &mut self.units[index];
        unit.fuel = if refueled {
            max_fuel
        } else {
            unit.fuel.saturating_sub(1)
        };
    }
}

#[cfg(test)]
#[path = "aviation/tests.rs"]
mod tests;
