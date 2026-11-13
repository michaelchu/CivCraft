//! AI planning for aircraft missions and refueling.

use super::*;

type AircraftAttackPath = (Vec<(i32, i32)>, (i32, i32), f64);
type AircraftBaseRoute = (Vec<(i32, i32)>, (i32, i32));

impl Simulation {
    pub(in crate::sim) fn aircraft_target_value(&self, plane: &Unit, x: i32, y: i32) -> f64 {
        if !self.players[plane.owner].ai && !self.tile(x, y).known_by[plane.owner]
            || self
                .units
                .iter()
                .any(|u| (u.x, u.y) == (x, y) && !self.at_war(plane.owner, u.owner))
            || self
                .cities
                .iter()
                .any(|c| (c.x, c.y) == (x, y) && !self.at_war(plane.owner, c.owner))
        {
            return 0.0;
        }
        if plane.id != 0
            && self
                .cities
                .iter()
                .find(|c| (c.x, c.y) == (x, y))
                .is_some_and(|city| self.invasion_support(plane.owner, city, plane.id).1 == 0)
        {
            return 0.0;
        }
        let Some(defender) = self
            .best_defender_id(plane, x, y)
            .and_then(|id| self.unit_by_id(id))
        else {
            return 0.0;
        };
        let stats = plane.kind.stats();
        let (firepower, retaliation) = self.combat_firepower(plane, defender);
        let win = (100.0
            * combat_win_chance(
                Self::attack_power(plane),
                plane.hp,
                firepower,
                self.defense_power_against(defender, x, y, plane.kind),
                defender.hp,
                retaliation,
            ))
        .floor()
            / 100.0;
        let mut benefit: i32 = self
            .combat_stack_losses(defender, plane)
            .map(|u| u.kind.stats().cost)
            .sum();
        if benefit == 0 {
            return 0.0;
        }
        if plane.kind.is_missile() {
            benefit -= stats.cost;
        }
        let risk = f64::from(stats.cost * stats.hp) / f64::from(plane.hp.max(1));
        let profit = 17.0 * (f64::from(benefit) * win - risk * (1.0 - win)) - 17.0 + 36.0;
        let outputs: Vec<_> = self
            .cities
            .iter()
            .filter(|c| c.owner == plane.owner)
            .map(|c| self.city_forecast(c).shield_surplus())
            .collect();
        let average = outputs.iter().sum::<i32>() / outputs.len().max(1) as i32;
        let home = self
            .cities
            .iter()
            .find(|c| Some(c.id) == plane.home_city)
            .map(|c| self.city_forecast(c).shield_surplus())
            .unwrap_or(1);
        let balanced = 2 * stats.cost * stats.attack / (stats.attack + stats.defense).max(1);
        profit.max(0.0) * (23.0_f64 / 24.0).powi(1 + balanced / home.max(average).max(1))
    }

    pub(in crate::sim) fn aircraft_production_choice(
        &self,
        city: &City,
    ) -> Option<(UnitKind, f64)> {
        let mut best = None;
        let mut best_value = 0.0;
        for kind in UnitKind::ALL_NATIVE
            .into_iter()
            .filter(|kind| kind.fuel_capacity() > 0)
        {
            // daiair.c rejects unit_type_is_losing_hp even for fuelled Air.
            // This ruleset supplies no global Unit_Recover effect; Medicine
            // is Health_Pct, so the Air/Helicopter class loss remains active.
            if !self.unit_buildable(city, kind) || kind.hp_loss_percent() > 0 {
                continue;
            }
            let mut plane = self.make_unit(0, city.owner, kind, city.x, city.y);
            plane.home_city = Some(city.id);
            plane.veteran = self.trained_rank(city, kind);
            plane.hp = plane.max_hit_points();
            if let Some((_, _, value)) = self.aircraft_best_target(&plane)
                && value > best_value
            {
                best_value = value;
                best = Some((kind, value));
            }
        }
        best
    }

    pub(super) fn aircraft_attack_path(&self, plane: &Unit) -> Option<AircraftAttackPath> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best = None;
        let mut best_value = 0.0;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, true, false) {
            if !self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == (x, y) && self.at_war(plane.owner, unit.owner))
            {
                continue;
            }
            let value = self.aircraft_target_value(plane, x, y);
            if value > best_value {
                best_value = value;
                best = Some((x, y));
            }
        }
        let target = best?;
        Some((map.path(self, target.0, target.1)?, target, best_value))
    }

    pub(super) fn aircraft_best_target(&self, plane: &Unit) -> Option<(i32, i32, f64)> {
        if plane.kind.fuel_capacity() > 0 {
            return self
                .aircraft_attack_path(plane)
                .map(|(_, (x, y), value)| (x, y, value));
        }
        self.units
            .iter()
            .filter(|u| self.at_war(plane.owner, u.owner) && u.transport.is_none())
            .filter_map(|u| {
                self.route_cost_within(plane, u.x, u.y, plane.moves_left)?;
                let value = self.aircraft_target_value(plane, u.x, u.y);
                (value > 0.0).then_some((u.x, u.y, value))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
    }

    fn source_unit_can_strike(&self, attacker: &Unit, defender: &Unit) -> bool {
        if (attacker.x, attacker.y) == (defender.x, defender.y) {
            return false;
        }
        let cost = if let Some(mut map) = super::fuel_paths::FuelPaths::new(self, attacker, true) {
            map.movement_cost(self, defender.x, defender.y)
        } else {
            super::normal_paths::NormalPaths::enemy_map(self, attacker)
                .movement_cost(self, defender.x, defender.y)
        };
        cost.is_some_and(|cost| cost <= attacker.moves_left)
    }

    pub(in crate::sim) fn source_hunter_launch(&mut self, hunter_id: u32, primary: u32) {
        let Some(hunter) = self.unit_by_id(hunter_id).cloned() else {
            return;
        };
        if !self.units.iter().any(|unit| unit.id == primary) {
            return;
        }
        let missiles: Vec<_> = self
            .cargo_units_ordered(hunter_id)
            .filter(|unit| unit.owner == hunter.owner && unit.kind.is_missile())
            .map(|unit| unit.id)
            .collect();
        for id in missiles {
            let Some(missile) = self.unit_by_id(id).cloned() else {
                continue;
            };
            // Literal daihunter.c: the map describes the HUNTER and its raw
            // move cost is compared with missile MP divided by SINGLE_MOVE.
            let mut map = super::normal_paths::NormalPaths::enemy_map(self, &hunter);
            let mut target = None;
            for (x, y) in map.reachable_within(self, missile.moves_left / MOVE_FRAGMENTS) {
                if self.cities.iter().any(|city| (city.x, city.y) == (x, y))
                    || self.best_defender_id(&hunter, x, y).is_none()
                {
                    continue;
                }
                for victim in self.tile_units_ordered(x, y) {
                    if !self.at_war(hunter.owner, victim.owner) {
                        continue;
                    }
                    if victim.id == primary
                        || victim.kind.stats().attack > hunter.kind.stats().defense
                            && self.source_unit_can_strike(victim, &hunter)
                    {
                        target = Some((x, y));
                        break;
                    }
                }
                if target.is_some() {
                    break;
                }
            }
            if let Some(target) = target {
                // Native deboard passes the hunter's id as actor, targeting
                // itself. It is illegal for the supported naval hunters and
                // does not unload the missile; normal GOTO performs departure.
                self.unit_goto_tiles.insert(id, target);
                let result = self.ai_goto_unit(id, target, super::normal_paths::GotoTask::None);
                if result.alive {
                    let live = self
                        .units
                        .iter()
                        .find(|unit| unit.id == id)
                        .unwrap()
                        .clone();
                    if let Some(primary) = self.unit_by_id(primary)
                        && self
                            .map_wrap
                            .civ3_distance(live.x, live.y, primary.x, primary.y)
                            == 1
                    {
                        if let Some(direction) = CIV3_DIRS.iter().position(|&(dx, dy)| {
                            self.map_wrap.civ3_offset(live.x, live.y, dx, dy)
                                == Some((primary.x, primary.y))
                        }) {
                            self.try_move_unit_civ3(id, direction);
                        }
                    }
                }
                if !self.units.iter().any(|unit| unit.id == primary) {
                    return;
                }
            }
        }
    }

    pub(super) fn aircraft_repair_gain(&self, plane: &Unit, x: i32, y: i32) -> i32 {
        self.stationary_unit_hp_recovery_at(plane, x, y)
    }

    pub(super) fn aircraft_regen_turns(&self, plane: &Unit, x: i32, y: i32) -> i32 {
        let lost = plane.max_hit_points() - plane.hp;
        if lost <= 0 {
            return 0;
        }
        let gain = self.aircraft_repair_gain(plane, x, y);
        if gain <= 0 { i32::MAX } else { 1 + lost / gain }
    }

    pub(super) fn aircraft_city_needs_defender(
        &self,
        plane: &Unit,
        x: i32,
        y: i32,
        current: bool,
    ) -> bool {
        let Some(city) = self.city_at(x, y) else {
            return false;
        };
        // daimilitary.c:grave_danger counts occupiers arriving within one
        // turn (including transports carrying occupation-capable units).
        let danger = self.city_grave_danger(city);
        let defenders = self
            .units
            .iter()
            .filter(|u| (u.x, u.y) == (x, y) && (!current || u.id != plane.id))
            .count();
        danger > defenders * 2
    }

    pub(super) fn aircraft_refuel_route(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<AircraftBaseRoute> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best = None;
        let mut best_regen = i32::MAX;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, false, true) {
            if !bases.contains(&(x, y)) {
                continue;
            }
            let turns = self.aircraft_regen_turns(plane, x, y);
            if plane.hp >= plane.max_hit_points() || turns <= 0 {
                best = Some((x, y));
                break;
            }
            if turns < best_regen {
                best_regen = turns;
                best = Some((x, y));
            }
        }
        let destination = best?;
        Some((map.path(self, destination.0, destination.1)?, destination))
    }

    pub(super) fn strategic_airbase(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<AircraftBaseRoute> {
        let lost = plane.max_hit_points() - plane.hp;
        let gain = self.aircraft_repair_gain(plane, plane.x, plane.y);
        let mut minimum_regen = i32::MAX;
        if lost > 0 && gain > 0 {
            let remaining = lost - if plane.moved { 0 } else { gain };
            if remaining <= 0 {
                return None;
            }
            minimum_regen = 1 + remaining / gain;
        }
        let mut defend = self.aircraft_city_needs_defender(plane, plane.x, plane.y, true);
        if defend && (lost <= 0 || minimum_regen <= 1) {
            return None;
        }
        let mut virtual_plane = plane.clone();
        virtual_plane.id = 0;
        virtual_plane.moves_left = self.unit_move_rate(plane);
        let mut worth = if defend {
            0.0
        } else {
            self.aircraft_best_target(&virtual_plane)
                .map_or(0.0, |t| t.2)
        };
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let routes: Vec<_> = map
            .reachable_within(self, plane.moves_left, true, false)
            .into_iter()
            .filter(|&(x, y, _)| bases.contains(&(x, y)))
            .filter_map(|(x, y, cost)| Some((cost, x, y, map.path(self, x, y)?)))
            .collect();
        let mut best = None;
        for (_, x, y, path) in routes {
            let mut faster = false;
            if lost > 0 {
                let turns = self.aircraft_regen_turns(plane, x, y);
                if turns > minimum_regen {
                    continue;
                }
                if turns < minimum_regen {
                    minimum_regen = turns;
                    best = Some((path.clone(), (x, y)));
                    worth = 0.0;
                    faster = true;
                }
            }
            if self.aircraft_city_needs_defender(plane, x, y, false) {
                if lost <= 0 {
                    return Some((path, (x, y)));
                }
                if !defend {
                    best = Some((path, (x, y)));
                    defend = true;
                }
                continue;
            } else if defend {
                if faster {
                    defend = false;
                } else {
                    continue;
                }
            }
            (virtual_plane.x, virtual_plane.y) = (x, y);
            let value = self
                .aircraft_best_target(&virtual_plane)
                .map_or(0.0, |t| t.2);
            if value > worth {
                worth = value;
                best = Some((path, (x, y)));
            }
        }
        best
    }

    pub(in crate::sim) fn run_ai_strike_aircraft(&mut self, aircraft: &Unit) {
        let position = (aircraft.x, aircraft.y);
        let mut bases = Vec::new();
        for (index, _) in self.tiles.iter().enumerate() {
            let (x, y) = self.map_wrap.position(index);
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
                ship.kind.can_carry(aircraft.kind)
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
        if on_base && aircraft.fuel < aircraft.kind.fuel_capacity() {
            return;
        }
        let before = aircraft.moves_left;
        let route = if on_base {
            self.aircraft_attack_path(aircraft)
                .map(|(path, target, _)| (path, target))
                .or_else(|| self.strategic_airbase(aircraft, &bases))
        } else {
            let old_goal = self
                .unit_goto_tiles
                .get(&aircraft.id)
                .copied()
                .filter(|goal| *goal != position && bases.contains(goal));
            old_goal
                .and_then(|goal| {
                    let mut map = super::fuel_paths::FuelPaths::new(self, aircraft, true)?;
                    Some((map.path(self, goal.0, goal.1)?, goal))
                })
                .or_else(|| self.aircraft_refuel_route(aircraft, &bases))
        };
        if let Some((path, _destination)) = route {
            self.follow_source_ai_path(aircraft.id, path);
            if let Some(live) = self
                .units
                .iter()
                .find(|unit| unit.id == aircraft.id)
                .cloned()
                && live.moves_left > 0
                && live.moves_left != before
            {
                self.run_ai_strike_aircraft(&live);
            }
        }
    }
}
