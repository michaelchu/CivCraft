//! AI planning for aircraft missions and refueling.

use super::*;

type AircraftAttackPath = (Vec<(i32, i32)>, (i32, i32), f64);
type AircraftBaseRoute = (Vec<(i32, i32)>, (i32, i32));

impl Simulation {
    fn aircraft_production_denominator(&self, plane: &Unit) -> i32 {
        let mut total = 0;
        let mut count = 0;
        for city in &self.cities {
            if city.owner == plane.owner {
                total += self.city_forecast(city).shield_surplus();
                count += 1;
            }
        }
        let average = total / count.max(1);
        let home = plane
            .home_city
            .and_then(|id| self.city_by_id(id))
            .map(|c| self.city_forecast(c).shield_surplus())
            .unwrap_or(1);
        home.max(average).max(1)
    }

    pub(in crate::sim) fn aircraft_target_value(&self, plane: &Unit, x: i32, y: i32) -> f64 {
        let max_output = self.aircraft_production_denominator(plane);
        self.aircraft_target_value_with_output(plane, x, y, max_output)
    }

    fn aircraft_target_value_with_output(
        &self,
        plane: &Unit,
        x: i32,
        y: i32,
        max_output: i32,
    ) -> f64 {
        let (_, war_mask, _) = self.pact_masks(plane.owner);
        let at_war_owner = |other: usize| {
            (other < 64 && ((war_mask >> other) & 1) != 0)
                || (other >= 64 && self.at_war(plane.owner, other))
        };
        if !self.players[plane.owner].ai && !self.tile(x, y).known_by[plane.owner]
            || self.any_unit_at(x, y, |u| !at_war_owner(u.owner))
            || self
                .city_at_cached(x, y)
                .is_some_and(|c| !at_war_owner(c.owner))
        {
            return 0.0;
        }
        if plane.id != 0
            && self
                .city_at_cached(x, y)
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
        let (win, risk) = if stats.attack == 0 && plane.kind.bombard_strength() > 0 {
            let bombard = f64::from(plane.kind.bombard_strength()) * 10.0;
            let defense = f64::from(
                self.defense_power_against(defender, x, y, plane.kind)
                    .max(1),
            );
            let hit_chance = bombard / (bombard + defense);
            // Bombardment deals non-lethal damage without direct ground retaliation.
            let expected_fraction = (hit_chance * f64::from(plane.kind.bombard_rate().max(1))
                / f64::from(defender.hp.max(1)))
            .min(0.75);
            (expected_fraction, 0.0)
        } else {
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
            let risk = f64::from(stats.cost * stats.hp) / f64::from(plane.hp.max(1));
            (win, risk)
        };
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
        let profit = 17.0 * (f64::from(benefit) * win - risk * (1.0 - win)) - 17.0 + 36.0;
        if profit <= 0.0 {
            return 0.0;
        }
        let eff_attack = stats.attack.max(plane.kind.bombard_strength());
        let balanced = 2 * stats.cost * eff_attack / (eff_attack + stats.defense).max(1);
        profit * (23.0_f64 / 24.0).powi(1 + balanced / max_output)
    }

    pub(in crate::sim) fn aircraft_production_choice(
        &self,
        city: &City,
    ) -> Option<(UnitKind, f64)> {
        let (_, war_mask, _) = self.pact_masks(city.owner);
        if war_mask == 0
            && !(0..self.players.len())
                .any(|other| other != city.owner && self.at_war(city.owner, other))
        {
            return None;
        }
        let mut best = None;
        let mut best_value = 0.0;
        let mut max_output: Option<i32> = None;
        for kind in UnitKind::all().filter(|kind| kind.fuel_capacity() > 0) {
            // Fuelled aircraft refuel at cities/airbases every turn, so do not
            // reject them for the Air class's field hp_loss_percent.
            if !self.unit_buildable(city, kind) {
                continue;
            }
            let mut plane = self.make_unit(0, city.owner, kind, city.x, city.y);
            plane.home_city = Some(city.id);
            plane.veteran = self.trained_rank(city, kind);
            plane.hp = plane.max_hit_points();
            let output =
                *max_output.get_or_insert_with(|| self.aircraft_production_denominator(&plane));
            if let Some((_, _, value)) = self.aircraft_attack_path_with_output(&plane, output)
                && value > best_value
            {
                best_value = value;
                best = Some((kind, value));
            }
        }
        best
    }

    pub(super) fn aircraft_attack_path(&self, plane: &Unit) -> Option<AircraftAttackPath> {
        let max_output = self.aircraft_production_denominator(plane);
        self.aircraft_attack_path_with_output(plane, max_output)
    }

    fn aircraft_attack_path_with_output(
        &self,
        plane: &Unit,
        max_output: i32,
    ) -> Option<AircraftAttackPath> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let (_, war_mask, _) = self.pact_masks(plane.owner);
        let mut best = None;
        let mut best_value = 0.0;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, true, false) {
            if !self.any_unit_at(x, y, |unit| {
                (unit.owner < 64 && ((war_mask >> unit.owner) & 1) != 0)
                    || (unit.owner >= 64 && self.at_war(plane.owner, unit.owner))
            }) {
                continue;
            }
            let value = self.aircraft_target_value_with_output(plane, x, y, max_output);
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
        let targets: Vec<_> = self
            .units
            .iter()
            .filter(|u| self.at_war(plane.owner, u.owner) && u.transport.is_none())
            .collect();
        let destinations: Vec<_> = targets.iter().map(|u| (u.x, u.y)).collect();
        let route_costs = self.route_costs_to_many_within(plane, &destinations, plane.moves_left);
        targets
            .into_iter()
            .zip(route_costs)
            .filter_map(|(u, cost)| {
                cost?;
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
            // NormalPaths stores costs in movement fragments. Freeciv's
            // `missile->moves_left / SINGLE_MOVE` is therefore already the
            // corresponding fragment budget in CivCraft; do not divide a
            // second time before passing it to reachable_within().
            for (x, y, _) in map.reachable_within(self, missile.moves_left) {
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
                        && let Some(direction) = CIV3_DIRS.iter().position(|&(dx, dy)| {
                            self.map_wrap.civ3_offset(live.x, live.y, dx, dy)
                                == Some((primary.x, primary.y))
                        })
                    {
                        self.try_move_unit_civ3(id, direction);
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

    /// Rebase a strike aircraft to a refueling site from which it can bombard
    /// a valuable hostile target, then use the normal bombard selector there.
    pub(super) fn aircraft_bombard_base_route(
        &self,
        plane: &Unit,
        bases: &[(i32, i32)],
    ) -> Option<AircraftBaseRoute> {
        let mut map = super::fuel_paths::FuelPaths::new(self, plane, true)?;
        let mut best: Option<(f64, i32, (i32, i32))> = None;
        for &(x, y) in bases {
            let Some(cost) = map.movement_cost(self, x, y) else {
                continue;
            };
            let carrier = self.units.iter().find(|ship| {
                (ship.x, ship.y) == (x, y)
                    && self.allied(plane.owner, ship.owner)
                    && self.ruleset_transport_pair_allowed_fail_closed(ship.kind, plane.kind)
            });
            let mut staged = plane.clone();
            staged.x = x;
            staged.y = y;
            staged.transport = carrier.map(|ship| ship.id);
            staged.moves_left = self.unit_move_rate(plane);
            let value = self
                .map_wrap
                .civ3_radius(x, y, Self::civ3_bombard_range(plane.kind))
                .into_iter()
                .filter(|&(tx, ty, _)| self.can_bombard_tile(&staged, tx, ty))
                .map(|(tx, ty, _)| self.ai_bombard_tile_value(&staged, tx, ty))
                .fold(0.0_f64, f64::max);
            if value <= 0.0 {
                continue;
            }
            let score = value / (1.0 + f64::from(cost) / f64::from(self.unit_move_rate(plane)));
            if best
                .is_none_or(|(old, old_cost, _)| score > old || (score == old && cost < old_cost))
            {
                best = Some((score, cost, (x, y)));
            }
        }
        let (_, _, destination) = best?;
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
        let mut best = None;
        for (x, y, _) in map.reachable_within(self, plane.moves_left, true, false) {
            if !bases.contains(&(x, y)) {
                continue;
            }
            let mut faster = false;
            if lost > 0 {
                let turns = self.aircraft_regen_turns(plane, x, y);
                if turns > minimum_regen {
                    continue;
                }
                if turns < minimum_regen {
                    minimum_regen = turns;
                    best = Some((x, y));
                    worth = 0.0;
                    faster = true;
                }
            }
            if self.aircraft_city_needs_defender(plane, x, y, false) {
                if lost <= 0 {
                    best = Some((x, y));
                    break;
                }
                if !defend {
                    best = Some((x, y));
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
                best = Some((x, y));
            }
        }
        let (x, y) = best?;
        Some((map.path(self, x, y)?, (x, y)))
    }

    pub(in crate::sim) fn run_ai_strike_aircraft(&mut self, aircraft: &Unit) {
        if matches!(aircraft.order, Some(UnitOrder::AirSuperiority { .. })) {
            return;
        }
        let position = (aircraft.x, aircraft.y);
        let on_base = self.refuel_site(aircraft.owner, position.0, position.1)
            || self.aboard_refueling_transport(aircraft);
        if on_base && self.run_ai_bombard(aircraft) {
            return;
        }
        if on_base && aircraft.fuel < aircraft.kind.fuel_capacity() {
            return;
        }
        let attack = on_base
            .then(|| self.aircraft_attack_path(aircraft))
            .flatten();
        if on_base && attack.is_none() && self.ai_assign_air_superiority(aircraft) {
            return;
        }
        let static_sites = self.planning.airfields(&self.tiles, self.map_wrap);
        let mut bases: Vec<_> = self
            .cities
            .iter()
            .filter(|c| self.allied(aircraft.owner, c.owner))
            .map(|c| (c.x, c.y))
            .chain(static_sites.iter().copied().filter(|&(x, y)| {
                let tile = self.tile(x, y);
                tile.known_by[aircraft.owner]
                    && matches!(tile.base, Some(Base::Airstrip | Base::Airbase))
            }))
            .filter(|&(x, y)| !self.any_unit_at(x, y, |u| !self.allied(aircraft.owner, u.owner)))
            .collect();
        drop(static_sites);
        bases.sort_unstable_by_key(|&(x, y)| self.map_wrap.index(x, y));
        bases.dedup();
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
        let before = aircraft.moves_left;
        let route = if on_base {
            attack
                .map(|(path, target, _)| (path, target))
                .or_else(|| self.aircraft_bombard_base_route(aircraft, &bases))
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
        if let Some((path, destination)) = route {
            if bases.contains(&destination) {
                self.unit_goto_tiles.insert(aircraft.id, destination);
            }
            let movement = self.follow_source_ai_path(aircraft.id, path);
            if movement.at_goal {
                self.unit_goto_tiles.remove(&aircraft.id);
            }
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

    fn ai_assign_air_superiority(&mut self, aircraft: &Unit) -> bool {
        if !self.players[aircraft.owner].ai || !self.can_set_air_superiority(aircraft) {
            return false;
        }
        let radius = aircraft.kind.operational_range() / 2;
        let threatened = self.units.iter().any(|enemy| {
            enemy.kind.is_aircraft()
                && self.at_war(aircraft.owner, enemy.owner)
                && self.unit_visible_to(aircraft.owner, enemy)
                && self
                    .map_wrap
                    .civ3_distance(aircraft.x, aircraft.y, enemy.x, enemy.y)
                    <= radius
        });
        threatened && self.start_air_superiority(aircraft.id)
    }
}
