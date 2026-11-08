//! Implements hunter behavior within the ai subsystem.

use super::*;

impl Simulation {
    pub(super) fn hunter_juiciness(&self, target: &Unit) -> (i32, i32) {
        use crate::sim::unit_catalog::UnitFlag;
        let mut cost = 0;
        let mut threat = 0;
        for unit in self.tile_units_ordered(target.x, target.y) {
            let stats = unit.kind.stats();
            threat += stats.attack * stats.hp * stats.firepower;
            if unit.kind.has_flag(UnitFlag::GameLoss) {
                cost += 1000;
                threat += 5000;
            }
            if unit.kind.has_hostile_action() {
                threat += 500;
            }
            cost += stats.cost;
        }
        (threat * 9 + cost, cost)
    }

    pub(super) fn hunter_candidate_value(
        &self,
        hunter: &Unit,
        target: &Unit,
        distance: i32,
    ) -> Option<i32> {
        // Chase comparisons use the unit TYPE's unmodified speed. Damage,
        // veterancy and positional effects affect PF, but not this decision.
        if hunter.kind.stats().moves < target.kind.stats().moves
            && !self
                .previous_naval_positions
                .get(&target.id)
                .is_some_and(|&(x, y)| {
                    self.map_distance(hunter.x, hunter.y, target.x, target.y)
                        < self.map_distance(hunter.x, hunter.y, x, y)
                })
        {
            return None;
        }
        let (threat, cost) = self.hunter_juiciness(target);
        let cost = if let Some(defender) = self
            .best_defender_id(hunter, target.x, target.y)
            .and_then(|id| self.unit_by_id(id))
        {
            let (attack, defense) = self.combat_firepower(hunter, defender);
            let win = combat_win_chance(
                Self::attack_power(hunter),
                hunter.hp,
                attack,
                self.defense_power_against(defender, target.x, target.y, hunter.kind),
                defender.hp,
                defense,
            );
            (f64::from(cost) * win) as i32
        } else {
            cost
        };
        let value = threat / (distance + 1);
        (cost >= hunter.kind.stats().cost && value >= hunter.kind.stats().cost).then_some(value)
    }

    pub(super) fn hunt_value(&self, hunter: &Unit, target: &Unit) -> Option<(i32, i32)> {
        let limit = 6 * self.unit_move_rate(hunter);
        let distance = crate::sim::normal_paths::NormalPaths::enemy_map(self, hunter)
            .movement_cost(self, target.x, target.y)?;
        if distance == 0 || distance > limit {
            return None;
        }
        self.hunter_candidate_value(hunter, target, distance)
            .map(|value| (value, distance))
    }

    pub(in crate::sim) fn hunter_production_choice(&self, _city: &City) -> Option<(UnitKind, f64)> {
        // difficulty.c's Normal profile has H_TARGETS. daihunter.c's
        // dai_hunter_choice returns before both missile and hunter advice when
        // that handicap is set. Existing Hunter-role units are still managed.
        // Hard-profile guess_best/hunter production is not implemented.
        None
    }

    pub(crate) fn normal_hunter_production_playtest(&self) -> (bool, bool, bool) {
        let mut game = self.clone();
        let (port, target) = game.hunter_production_playtest_setup();
        let city = game.city_by_id(port).unwrap().clone();
        let mut hunter = game.make_unit(0, city.owner, UnitKind::Submarine, city.x, city.y);
        hunter.home_city = Some(port);
        let target = game.unit_by_id(target).unwrap().clone();
        let worthwhile = game.hunt_value(&hunter, &target).is_some();
        let hull_suppressed = game.hunter_production_choice(&city).is_none();
        game.spawn_unit(city.owner, UnitKind::Submarine, city.x, city.y);
        let ammo_available = game.can_produce(&city, Production::Unit(UnitKind::CruiseMissile));
        (
            worthwhile,
            hull_suppressed,
            ammo_available && game.hunter_production_choice(&city).is_none(),
        )
    }

    /// Source tactical hunter policies, isolated from the dedicated production
    /// gate (which Normal AI's handicap disables).
    pub(crate) fn native_hunter_tactics_playtest(&self) -> (bool, bool, bool, bool) {
        use crate::sim::unit_catalog::UnitFlag;
        let mut sim = self.clone();
        sim.units.clear();
        sim.cities.clear();
        sim.hunter_targets.clear();
        sim.previous_naval_positions.clear();
        sim.ai_ground_targets.clear();
        sim.sign_pact(1, HUMAN, Pact::War);
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            tile.known_by.fill(false);
        }
        let start = (20, 30);
        let near = offset_native_by_map(start.0, start.1, 2, 0);
        let far = offset_native_by_map(start.0, start.1, 5, 0);
        let hunter_id = sim.spawn_unit(1, UnitKind::Horseman, start.0, start.1);
        let near_id = sim.spawn_unit(HUMAN, UnitKind::Diplomat, near.0, near.1);
        let far_id = sim.spawn_unit(HUMAN, UnitKind::Leader, far.0, far.1);
        let hunter = sim
            .units
            .iter()
            .find(|u| u.id == hunter_id)
            .unwrap()
            .clone();
        // A much richer GameLoss target must not replace an already worthwhile
        // earlier PF tile. Both enemy squares are unknown to this Normal AI.
        let first = sim.hunter_target(&hunter, &Default::default()).map(|v| v.0);
        let far_unit = sim.unit_by_id(far_id).unwrap();
        let richer = sim.hunt_value(&hunter, far_unit).is_some_and(|(v, _)| {
            sim.hunt_value(&hunter, sim.unit_by_id(near_id).unwrap())
                .is_some_and(|(near_value, _)| v > near_value)
        });
        let normal_omni = first == Some(near_id)
            && !sim.tile(near.0, near.1).known_by[1]
            && !sim.tile(far.0, far.1).known_by[1];
        sim.run_ai_player(1);
        let land = first == Some(near_id)
            && richer
            && sim
                .units
                .iter()
                .any(|u| u.id == hunter_id && (u.x, u.y) != start);

        let mut stack = self.clone();
        stack.units.clear();
        stack.cities.clear();
        stack.sign_pact(1, HUMAN, Pact::War);
        let leader = stack.spawn_unit(HUMAN, UnitKind::Leader, near.0, near.1);
        let target = stack.units.iter().find(|u| u.id == leader).unwrap();
        let stats = target.kind.stats();
        let (threat, cost) = stack.hunter_juiciness(target);
        let gameloss = target.kind.has_flag(UnitFlag::GameLoss)
            && cost == stats.cost + 1000
            && threat
                == (stats.attack * stats.hp * stats.firepower
                    + 5000
                    + 500 * i32::from(target.kind.has_hostile_action()))
                    * 9
                    + cost;

        let mut speed = sim.clone();
        let target = speed.spawn_unit(HUMAN, UnitKind::Cavalry, far.0, far.1);
        let mut fast = speed.units.iter().find(|u| u.id == target).unwrap().clone();
        fast.hp = 1;
        speed.previous_naval_positions.insert(target, far);
        let base_speed = speed.hunter_candidate_value(&hunter, &fast, 6).is_none()
            && speed.unit_move_rate(&fast) <= speed.unit_move_rate(&hunter);
        (land, normal_omni, gameloss, base_speed)
    }

    pub(super) fn hunter_target(
        &self,
        hunter: &Unit,
        claimed: &std::collections::HashSet<u32>,
    ) -> Option<(u32, i32, i32)> {
        use crate::sim::unit_catalog::UnitFlag;
        let original = self
            .hunter_targets
            .get(&hunter.id)
            .and_then(|id| self.unit_by_id(*id));
        let original_threat = original.map_or(0, |unit| self.hunter_juiciness(unit).0);
        let limit = 6 * self.unit_move_rate(hunter);
        let mut map = crate::sim::normal_paths::NormalPaths::enemy_map(self, hunter);
        // Native PF order, then native tile-list order; this is deliberately
        // the FIRST worthwhile stack, rather than the globally richest stack.
        for (x, y) in map.reachable_within(self, limit) {
            if self.cities.iter().any(|city| (city.x, city.y) == (x, y))
                || self
                    .tile_units_ordered(x, y)
                    .any(|unit| !self.at_war(hunter.owner, unit.owner))
            {
                continue;
            }
            let distance = map.movement_cost(self, x, y)?;
            for target in self.tile_units_ordered(x, y) {
                if !self.advisor_player_dangerous(hunter.owner, target.owner)
                    || claimed.contains(&target.id)
                    || (!target.kind.has_hostile_action()
                        && target.kind.transport_capacity() == 0
                        && !target.kind.has_flag(UnitFlag::GameLoss))
                    || !self.pf_physical_attack(hunter, target.kind, x, y)
                {
                    continue;
                }
                let Some(value) = self.hunter_candidate_value(hunter, target, distance) else {
                    continue;
                };
                if original.is_some_and(|unit| unit.id != target.id) && original_threat > value {
                    continue;
                }
                return Some((target.id, x, y));
            }
        }
        None
    }

    // Return -1 for a completed kill, 0 for no task/death, positive for an
    // unfinished hunt. dai_manage_military retries a successful hunt once.
    pub(super) fn manage_hunter(
        &mut self,
        hunter: &Unit,
        claimed: &mut std::collections::HashSet<u32>,
    ) -> i32 {
        let Some((target, x, y)) = self.hunter_target(hunter, claimed) else {
            if self.hunter_targets.remove(&hunter.id).is_some() {
                self.unit_goto_tiles.remove(&hunter.id);
            }
            return 0;
        };
        self.hunter_targets.insert(hunter.id, target);
        claimed.insert(target);
        self.clear_ai_charge(hunter.id);
        self.unit_goto_tiles.insert(hunter.id, (x, y));
        // dai_unit_new_task(AIUNIT_HUNTER) grabs eligible missiles in native
        // tile-list order wherever the hunt begins, not only in a home port.
        let missiles: Vec<_> = self
            .tile_units_ordered(hunter.x, hunter.y)
            .filter(|missile| {
                missile.owner == hunter.owner
                    && missile.kind.is_missile()
                    && missile.transport.is_none()
                    && !self.ai_guard_targets.contains_key(&missile.id)
            })
            .map(|missile| missile.id)
            .collect();
        for missile in missiles {
            self.board_specific_ship(missile, hunter.id);
        }
        self.launch_hunter_missiles(hunter.id, target);
        if self.units.iter().any(|unit| unit.id == target) {
            let path =
                crate::sim::normal_paths::NormalPaths::enemy_map(self, hunter).path(self, x, y);
            if let Some((path, _)) = path
                && !self.follow_source_ai_path(hunter.id, path).alive
            {
                return 0;
            }
            if self.units.iter().any(|unit| unit.id == target) {
                self.launch_hunter_missiles(hunter.id, target);
            }
        }
        if !self.units.iter().any(|unit| unit.id == target) {
            self.hunter_targets.remove(&hunter.id);
            self.unit_goto_tiles.remove(&hunter.id);
            claimed.remove(&target);
            return -1;
        }
        1
    }

    pub(super) fn run_existing_hunter(
        &mut self,
        hunter: &Unit,
        claimed: &mut std::collections::HashSet<u32>,
    ) -> bool {
        let result = self.manage_hunter(hunter, claimed);
        if result == -1
            && let Some(actor) = self.unit_by_id(hunter.id).cloned()
        {
            self.manage_hunter(&actor, claimed);
        }
        result != 0 || !self.units.iter().any(|unit| unit.id == hunter.id)
    }

    fn launch_hunter_missiles(&mut self, hunter_id: u32, primary: u32) {
        self.source_hunter_launch(hunter_id, primary);
    }
}
