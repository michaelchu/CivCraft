use super::exploration::ExplorerQueue;
use super::pathfinding::reconstruct_path;
use super::pf_actions::PfAction;
use super::*;

mod playtest;

struct LeaderPaths {
    costs: Vec<i32>,
    previous: Vec<Option<(usize, (i32, i32))>>,
    order: Vec<usize>,
    start: usize,
}

enum EnemyDistances {
    Normal(super::normal_paths::NormalPaths),
    Fuel(super::fuel_paths::FuelPaths),
}

impl EnemyDistances {
    fn cost(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        match self {
            Self::Normal(paths) => paths.movement_cost(sim, x, y),
            Self::Fuel(paths) => paths.movement_cost(sim, x, y),
        }
    }
}

impl LeaderPaths {
    fn path(&self, target: usize) -> Option<Vec<(i32, i32)>> {
        if self.costs[target] == i32::MAX {
            return None;
        }
        reconstruct_path(&self.previous, self.start, target)
    }
}

impl Simulation {
    fn leader_nonallied_at(&self, owner: usize, x: i32, y: i32) -> bool {
        self.units
            .iter()
            .any(|unit| (unit.x, unit.y) == (x, y) && !self.allied(owner, unit.owner))
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner))
    }

    // daiunit.c's reverse query is deliberately NOT an ordinary enemy route:
    // its owner is the leader; it has no ZOC/occupant callback, assumes full
    // enemy movement, and considers the target attackable even for transports.
    fn leader_paths(&self, unit: &Unit, reverse: Option<(usize, usize)>) -> LeaderPaths {
        self.leader_paths_mode(unit, reverse, false)
    }

    fn leader_paths_mode(
        &self,
        unit: &Unit,
        reverse: Option<(usize, usize)>,
        observed: bool,
    ) -> LeaderPaths {
        let rate = self.unit_move_rate(unit);
        let mut actor = unit.clone();
        if let Some((_, owner)) = reverse {
            actor.owner = owner;
        }
        let initial = if reverse.is_some() {
            rate
        } else {
            unit.moves_left.max(0)
        };
        let start = self.map_wrap.index(unit.x, unit.y);
        let mut result = LeaderPaths {
            costs: vec![i32::MAX; self.tiles.len()],
            previous: vec![None; self.tiles.len()],
            order: Vec::new(),
            start,
        };
        // Reverse PF rejects rawcost >= rate*(3+1), including its own
        // initial zero-cost position when the candidate's rate is zero.
        if reverse.is_some() && rate == 0 {
            return result;
        }
        result.costs[start] = 0;
        let mut queue = ExplorerQueue::default();
        queue.put(0, start);
        while let Some((spent, current)) = queue.pop() {
            let spent = spent as i32;
            if result.costs[current] != spent {
                continue;
            }
            if reverse.is_some() && spent >= 4 * rate {
                break;
            }
            result.order.push(current);
            if reverse.is_some_and(|(target, _)| current == target) {
                break;
            }
            let (x, y) = self.map_wrap.position(current);
            let known_here = !observed || self.tile(x, y).known_by[actor.owner];
            let action_here = if reverse.is_some_and(|(target, _)| current == target) {
                PfAction::Attack
            } else if reverse.is_some() {
                PfAction::None
            } else {
                self.pf_default_action(
                    actor.kind,
                    actor.owner,
                    (x, y),
                    known_here,
                    !observed || self.pf_main_sight(actor.owner, x, y),
                    !observed,
                )
            };
            if !matches!(action_here, PfAction::None | PfAction::Impossible) {
                continue;
            }
            if rate == 0 && spent >= initial {
                continue;
            }
            (actor.x, actor.y) = (x, y);
            let remaining = if spent < initial {
                initial - spent
            } else {
                rate - (spent - initial) % rate
            };
            for (dx, dy) in ISOHEX_DIRS {
                let Some((nx, ny)) = self.map_wrap.offset(x, y, dx, dy) else {
                    continue;
                };
                let next = self.map_wrap.index(nx, ny);
                let target = reverse.is_some_and(|(target, _)| next == target);
                let known = !observed || self.tile(nx, ny).known_by[actor.owner];
                if known
                    && !actor.kind.is_civilian()
                    && self
                        .tile(nx, ny)
                        .owner
                        .is_some_and(|owner| self.pact(actor.owner, owner) == Pact::Peace)
                    || (reverse.is_none()
                        && known
                        && (!observed || self.tile(x, y).known_by[actor.owner])
                        && self.zone_blocks_step(&actor, (x, y), (nx, ny)))
                {
                    continue;
                }
                let action = if target {
                    PfAction::Attack
                } else if reverse.is_some() {
                    PfAction::None
                } else {
                    self.pf_default_action(
                        actor.kind,
                        actor.owner,
                        (nx, ny),
                        known,
                        !observed || self.pf_main_sight(actor.owner, nx, ny),
                        !observed,
                    )
                };
                if action == PfAction::Impossible {
                    continue;
                }
                let cost = if action != PfAction::None {
                    if reverse.is_none()
                        && known_here
                        && !self.pf_action_step_possible(&actor, action)
                    {
                        continue;
                    }
                    Self::pf_action_move_cost(actor.kind, action, rate)
                } else if !known {
                    if unit.kind.is_naval() {
                        2 * MOVE_FRAGMENTS
                    } else if unit.kind.stats().ignores_terrain {
                        4
                    } else {
                        6 * MOVE_FRAGMENTS
                    }
                } else {
                    let Some(cost) = self
                        .movement_cost_for(&actor, nx, ny)
                        .or_else(|| self.boarding_target(&actor, nx, ny).map(|_| MOVE_FRAGMENTS))
                    else {
                        continue;
                    };
                    cost
                };
                let candidate = spent + cost.min(remaining);
                if candidate < result.costs[next] && (reverse.is_none() || candidate < 4 * rate) {
                    result.costs[next] = candidate;
                    result.previous[next] = Some((current, (dx, dy)));
                    queue.put(i64::from(candidate), next);
                }
            }
        }
        result
    }

    pub(super) fn ai_movement_cost_order(&self, unit: &Unit) -> Vec<(i32, i32, i32)> {
        let paths = self.leader_paths(unit, None);
        paths
            .order
            .into_iter()
            .map(|index| {
                let (x, y) = self.map_wrap.position(index);
                (paths.costs[index], x, y)
            })
            .collect()
    }

    fn leader_follow_path(&mut self, id: u32, path: Vec<(i32, i32)>) {
        let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) else {
            return;
        };
        let activity = unit.state;
        unit.state = UnitState::Idle;
        for (dx, dy) in path {
            let Some(unit) = self.unit_by_id(id).cloned() else {
                break;
            };
            if unit.moves_left <= 0 {
                break;
            }
            let Some((nx, ny)) = self.map_wrap.offset(unit.x, unit.y, dx, dy) else {
                continue;
            };
            if self.leader_nonallied_at(unit.owner, nx, ny)
                || (unit.owner == BARBARIANS && self.tile(nx, ny).hut)
            {
                break;
            }
            if let Some(cost) = self.movement_cost_for(&unit, nx, ny)
                && unit.moves_left <= cost
                && self.unit_move_rate(&unit) > cost
                && self.explorer_danger_at(&unit, nx, ny)
                && !self.explorer_danger_at(&unit, unit.x, unit.y)
            {
                break;
            }
            if self.try_move_unit(id, dx, dy) == MoveResult::Blocked {
                break;
            }
        }
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
            unit.state = activity;
        }
    }

    pub(super) fn run_barbarian_leader(&mut self, id: u32) {
        let Some(leader) = self.unit_by_id(id).cloned() else {
            return;
        };
        let position = (leader.x, leader.y);
        if leader.moves_left == 0
            || (!self.tile(position.0, position.1).terrain.is_water()
                && self
                    .units
                    .iter()
                    .filter(|unit| (unit.x, unit.y) == position)
                    .count()
                    > 1)
        {
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .state = UnitState::Sentry;
            return;
        }
        let boat = self
            .ai_ferries
            .boats
            .iter()
            .find_map(|(&boat, _)| (self.ferry_boss(boat) == Some(id)).then_some(boat));
        if let Some(boat) = boat {
            self.release_ai_ferry(id);
            let warrior = self
                .tile_units_ordered(position.0, position.1)
                .find(|unit| {
                    (unit.x, unit.y) == position
                        && unit.kind != UnitKind::BarbarianLeader
                        && unit.kind.transport_capacity() == 0
                        && unit.moves_left > 0
                })
                .map(|unit| unit.id);
            if let Some(warrior) = warrior {
                self.assign_ai_ferry(boat, warrior);
                self.run_ai_ferry_after_handoff(boat);
                return;
            }
        }

        let guards: Vec<_> = self
            .player_units_ordered(leader.owner)
            .filter(|unit| unit.owner == leader.owner && unit.kind != UnitKind::BarbarianLeader)
            .filter(|unit| {
                let paths = self.leader_paths_mode(unit, None, true);
                paths.costs[self.map_wrap.index(leader.x, leader.y)] != i32::MAX
            })
            .map(|unit| unit.id)
            .collect();
        if !guards.is_empty() {
            let paths = self.leader_paths(&leader, None);
            for &tile in paths.order.iter().filter(|&&tile| tile != paths.start) {
                let point = self.map_wrap.position(tile);
                if self
                    .units
                    .iter()
                    .any(|unit| guards.contains(&unit.id) && (unit.x, unit.y) == point)
                {
                    if let Some(path) = paths.path(tile) {
                        self.leader_follow_path(id, path);
                    }
                    return;
                }
            }
        }

        let mut worst = None;
        let mut minimum = i32::MAX;
        for owner in 0..self.players.len() {
            for enemy in self
                .player_units_ordered(owner)
                .filter(|unit| unit.owner == owner && unit.owner != leader.owner)
            {
                let index = self.map_wrap.index(leader.x, leader.y);
                let cost = self.leader_paths(enemy, Some((index, leader.owner))).costs[index];
                if cost < minimum {
                    minimum = cost;
                    worst = Some(enemy.clone());
                }
            }
        }
        let Some(enemy) = worst else {
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .state = UnitState::Idle;
            return;
        };
        // The initial reverse query deliberately stayed PF_NORMAL; only
        // the selected enemy's ordinary map uses native fuel constraints.
        let mut enemy_paths =
            if let Some(paths) = super::fuel_paths::FuelPaths::new(self, &enemy, true) {
                EnemyDistances::Fuel(paths)
            } else {
                EnemyDistances::Normal(super::normal_paths::NormalPaths::enemy_map(self, &enemy))
            };
        let mut safest_cost = enemy_paths.cost(self, leader.x, leader.y).unwrap_or(-1);
        loop {
            let Some(actor) = self.unit_by_id(id).cloned() else {
                return;
            };
            if actor.moves_left <= 0 {
                return;
            }
            let mut safest = (actor.x, actor.y);
            for (dx, dy) in ISOHEX_DIRS {
                let Some((x, y)) = self.map_wrap.offset(actor.x, actor.y, dx, dy) else {
                    continue;
                };
                if self.treaty_blocks_move(&actor, x, y)
                    || self.leader_nonallied_at(actor.owner, x, y)
                    || self.zone_blocks_step(&actor, (actor.x, actor.y), (x, y))
                    || self.movement_cost_for(&actor, x, y).is_none()
                {
                    continue;
                }
                if let Some(cost) = enemy_paths.cost(self, x, y)
                    && cost > safest_cost
                {
                    safest_cost = cost;
                    safest = (x, y);
                }
            }
            if safest == (actor.x, actor.y) {
                self.units
                    .iter_mut()
                    .find(|unit| unit.id == id)
                    .unwrap()
                    .state = UnitState::Idle;
                return;
            }
            let paths = self.leader_paths(&actor, None);
            if let Some(path) = paths.path(self.map_wrap.index(safest.0, safest.1)) {
                self.leader_follow_path(id, path);
            }
            if self
                .units
                .iter()
                .find(|unit| unit.id == id)
                .is_none_or(|unit| (unit.x, unit.y) == (actor.x, actor.y))
            {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "leaders/tests.rs"]
mod tests;
