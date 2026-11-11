//! Aircraft, fuel, airlift, and paradrop rules.

use super::*;

mod aircraft_ai;
mod paradrop;
mod rebase;

impl Simulation {
    pub(crate) fn can_assign_air_superiority(&self, unit: &Unit) -> bool {
        unit.owner == HUMAN
            && unit.kind.is_aircraft()
            && unit.kind.can_attack_aircraft()
            && unit.kind.fuel_capacity() > 0
            && unit.moves_left > 0
            && !unit.moved
            && (self.refuel_site(unit.owner, unit.x, unit.y)
                || self.aboard_refueling_transport(unit))
    }

    pub(crate) fn assign_air_superiority(&mut self, id: u32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_assign_air_superiority(&unit) {
            return false;
        }
        let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) else {
            return false;
        };
        actor.state = UnitState::AirSuperiority;
        actor.moves_left = 0;
        actor.moved = true;
        self.add_event(format!("{} assigned to Air Superiority.", unit.kind.name()));
        true
    }

    pub(super) fn assign_ai_air_superiority(&mut self, id: u32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if unit.kind.is_missile()
            || !unit.kind.is_aircraft()
            || !unit.kind.can_attack_aircraft()
            || unit.kind.fuel_capacity() == 0
            || unit.moves_left <= 0
            || unit.moved
            || !(self.refuel_site(unit.owner, unit.x, unit.y)
                || self.aboard_refueling_transport(&unit))
        {
            return false;
        }
        let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) else {
            return false;
        };
        actor.state = UnitState::AirSuperiority;
        actor.moves_left = 0;
        actor.moved = true;
        true
    }

    fn air_superiority_interceptors(&self, enemy_owner: usize, x: i32, y: i32) -> Vec<Unit> {
        self.units
            .iter()
            .filter(|unit| {
                unit.state == UnitState::AirSuperiority
                    && unit.kind.can_attack_aircraft()
                    && unit.kind.is_aircraft()
                    && unit.owner != enemy_owner
                    && self.at_war(unit.owner, enemy_owner)
                    && (self.refuel_site(unit.owner, unit.x, unit.y)
                        || self.aboard_refueling_transport(unit))
                    && self.map_wrap.civ3_distance(unit.x, unit.y, x, y)
                        <= i32::from(unit.kind.fuel_capacity()) / 2
            })
            .cloned()
            .collect()
    }

    fn intercept_air_mission(&mut self, aircraft: &Unit, x: i32, y: i32) -> bool {
        if !aircraft.kind.is_aircraft() && !aircraft.kind.is_missile() {
            return false;
        }
        let chance = Self::air_mission_interception_chance(aircraft.kind);
        for interceptor in self.air_superiority_interceptors(aircraft.owner, x, y) {
            if self.rand_below(100) >= chance {
                continue;
            }
            if !self.units.iter().any(|unit| unit.id == aircraft.id) {
                return true;
            }
            if let Some(actor) = self.units.iter_mut().find(|unit| unit.id == interceptor.id) {
                // Air Superiority remains the assigned mission after a sortie;
                // provide a full attack roll while the aircraft is on patrol.
                actor.moves_left = MOVE_FRAGMENTS;
            } else {
                continue;
            }
            self.resolve_combat(interceptor.id, aircraft.id, x, y, None);
            return true;
        }
        false
    }

    fn air_mission_interception_chance(kind: UnitKind) -> i32 {
        let rules = *super::air_mission_rules::air_mission_rules();
        if kind.uses_stealth_vision() {
            rules.chance_to_intercept_stealth_missions
        } else {
            rules.chance_to_intercept_air_missions
        }
    }

    pub(crate) fn can_begin_bombard(&self, unit: &Unit) -> bool {
        unit.owner == HUMAN
            && unit.kind.bombard_range() > 0
            && !unit.kind.is_aircraft()
            && !unit.kind.is_nuclear_weapon()
            && unit.transport.is_none()
            && unit.moves_left > 0
            && unit.work_left == 0
            && unit.state != UnitState::Sentry
    }

    pub(crate) fn can_bombard_tile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_bombard(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !self.tile(x, y).visible
            || (x, y) == (unit.x, unit.y)
            || self.map_wrap.civ3_distance(unit.x, unit.y, x, y) > unit.kind.bombard_range()
        {
            return false;
        }
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && !self.at_war(unit.owner, city.owner))
            || self.units.iter().any(|target| {
                (target.x, target.y) == (x, y) && !self.at_war(unit.owner, target.owner)
            })
        {
            return false;
        }
        self.units.iter().any(|target| {
            (target.x, target.y) == (x, y)
                && target.transport.is_none()
                && !target.kind.is_aircraft()
                && self.at_war(unit.owner, target.owner)
        })
    }

    pub(super) fn try_ranged_bombard(&mut self, id: u32, x: i32, y: i32) -> MoveResult {
        let Some(attacker) = self.unit_by_id(id).cloned() else {
            return MoveResult::Blocked;
        };
        if !self.can_bombard_tile(&attacker, x, y) {
            return MoveResult::Blocked;
        }
        self.try_bombard(&attacker, x, y)
    }

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
        if carrier.hp < carrier.max_hit_points()
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

    fn air_defense_stops_bomber(&mut self, bomber: &Unit, x: i32, y: i32) -> bool {
        // The Conquests editor's Air Defense strength acts as an attack value
        // against the bombarding aircraft's base defense. Community test
        // results report this value at one tenth of its editor number, so in
        // the sim's POWER_FACTOR scale the attack is the raw BIQ value while
        // the aircraft defense is multiplied by POWER_FACTOR. No terrain,
        // veteran, or fortification modifiers apply to this interception.
        // (CivFanatics editor/combat references linked from the rules README.)
        if !bomber.kind.is_aircraft() && !bomber.kind.is_missile() {
            return false;
        }
        let aircraft_defense = bomber.kind.stats().defense * POWER_FACTOR;
        if aircraft_defense <= 0 {
            return false;
        }
        let defenses: Vec<_> = self
            .tile_units_ordered(x, y)
            .filter(|unit| unit.transport.is_none() && self.at_war(unit.owner, bomber.owner))
            .filter_map(|unit| {
                let rule = unit
                    .kind
                    .conquests_id()
                    .and_then(crate::sim::civ3_unit_rules::get)?;
                (rule.air_defense > 0).then_some((unit.owner, unit.kind, rule.air_defense))
            })
            .take(4)
            .collect();

        for (owner, kind, attack) in defenses {
            if self.rand_below(attack + aircraft_defense) < attack {
                self.remove_units_with_cargo(vec![bomber.id]);
                self.nation_scores[owner].units_killed += 1;
                self.refresh_visibility();
                if bomber.owner == HUMAN {
                    self.add_event(format!(
                        "Your {} was shot down by a {}.",
                        bomber.kind.name(),
                        kind.name()
                    ));
                } else if owner == HUMAN {
                    self.add_event(format!(
                        "An enemy {} was shot down by your {}.",
                        bomber.kind.name(),
                        kind.name()
                    ));
                }
                return true;
            }
        }
        false
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
        if self.intercept_air_mission(attacker, x, y) {
            return MoveResult::Fought;
        }
        if self.air_defense_stops_bomber(attacker, x, y) {
            return MoveResult::Fought;
        }
        let target = targets[self.rand_below(targets.len() as i32) as usize].clone();
        let bombard_strength = attacker.kind.bombard_strength();
        let attack = if bombard_strength > 0 {
            bombard_strength * POWER_FACTOR
        } else {
            Self::attack_power(attacker)
        };
        let defense = self.defense_power_against(&target, x, y, attacker.kind);
        let hits = (0..attacker.kind.bombard_rate())
            .filter(|_| attack > 0 && self.rand_below(attack + defense) < attack)
            .count() as i32;
        let (firepower, _) = self.combat_firepower(attacker, &target);
        let lethal = attacker.kind.has_lethal_bombard_against(target.kind);
        let defender = self
            .units
            .iter_mut()
            .find(|unit| unit.id == target.id)
            .unwrap();
        let hp = (defender.hp - hits * firepower).max(i32::from(!lethal));
        let damage = defender.hp - hp;
        defender.hp = hp;
        if self.visual_events.enabled {
            let hp_steps = (hp..=target.hp).rev().map(|hp| (attacker.hp, hp)).collect();
            self.publish_visual_event(VisualEvent::Combat(Box::new(presentation::CombatVisual {
                attacker: attacker.clone(),
                defender: target.clone(),
                hp: hp_steps,
                city: self.city_at(x, y).cloned(),
                bombard_effects: attacker.kind.conquests_rule().bombard_effects > 0,
                territory: (0..self.map_wrap.width() * self.map_wrap.height())
                    .map(|i| {
                        self.displayed_tile_owner(
                            i % self.map_wrap.width(),
                            i / self.map_wrap.width(),
                        )
                    })
                    .collect(),
            })));
        }
        let destroyed = hp == 0;
        if destroyed {
            self.remove_units_with_cargo(vec![target.id]);
            // GCON_Craters: a unit with Create Craters may leave damage when
            // bombardment clears the last unit from a non-city tile. This
            // pipeline does not yet destroy terrain improvements, so preserve
            // them as an eligibility blocker instead of pretending they were
            // removed by the attack.
            let crater_rule = attacker.kind.conquests_rule().create_craters > 0;
            let tile = self.tile(x, y);
            let has_terrain_improvement = tile.road
                || tile.railroad
                || tile.irrigation
                || tile.farmland
                || tile.base.is_some()
                || tile.mine
                || tile.oil_well
                || tile.oil_platform;
            let has_city = self.city_at(x, y).is_some();
            let has_units = self.tile_units_ordered(x, y).next().is_some();
            if crater_rule && !has_city && !has_units && !has_terrain_improvement {
                self.tile_mut(x, y).craters = true;
            }
            self.refresh_visibility();
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
                "A {} dealt {damage} damage to a {}.",
                attacker.kind.name(),
                target.kind.name()
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
        let max_hp = unit.max_hit_points();
        let airborne_loss = if unit.kind.is_missile() {
            0
        } else {
            percent_of(max_hp, 10).max(1)
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
