//! Worker automation, tile assignment, and terrain improvement planning.

use super::pathfinding::reconstruct_path;
use super::*;

mod planning;
mod playtest;
use planning::{WORKER_FACTOR, WorkMap, amortize};
#[cfg(test)]
use planning::{WorkerPlan, consider_action};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashSet};

/// Native city requests are real input orders. The default AI produces none;
/// callers with a request producer can pass them to the same planner.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct WorkerRequest {
    pub(super) city: u32,
    pub(super) destination: (i32, i32),
    pub(super) activity: WorkKind,
    pub(super) want: i32,
}

struct WorkerRoutes {
    map_wrap: MapWrap,
    costs: Vec<i32>,
    previous: Vec<Option<(usize, (i32, i32))>>,
    start: usize,
}

impl WorkerRoutes {
    fn path(&self, destination: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        let cursor = self.map_wrap.index(destination.0, destination.1);
        if self.costs[cursor] == i32::MAX {
            return None;
        }
        reconstruct_path(&self.previous, self.start, cursor)
    }
}

impl Simulation {
    fn worker_guard(kind: UnitKind) -> bool {
        // Native is_guard_unit is precisely !UTYF_CIVILIAN (NonMil).
        !kind.is_civilian()
    }

    fn worker_enemy_native(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        if kind.is_aircraft() || kind.is_missile() {
            return true;
        }
        if kind.is_naval() {
            return tile.terrain.is_water();
        }
        if tile.terrain.is_water() {
            return false;
        }
        !kind.is_big_land()
            || !matches!(
                tile.terrain,
                Terrain::Mountains | Terrain::Marsh | Terrain::Jungle
            )
            || tile.road
            || matches!(tile.base, Some(Base::Fort | Base::Fortress))
    }

    fn worker_safe_tile(&self, owner: usize, x: i32, y: i32) -> bool {
        // Native adv_worker_safe_tile accepts any guard on the worksite.
        if self.any_unit_at(x, y, |u| Self::worker_guard(u.kind)) {
            return true;
        }
        !self.units.iter().any(|u| {
            self.at_war(owner, u.owner)
                && (self.players[owner].ai || self.unit_visible_to(owner, u))
                && u.kind.stats().attack > 0
                && self.map_wrap.civ3_distance(x, y, u.x, u.y) <= 2
                && (self.worker_enemy_native(u.kind, x, y)
                    || (u.kind.is_naval()
                        && u.kind != UnitKind::Submarine
                        && self
                            .map_wrap
                            .civ3_neighbors(x, y)
                            .any(|(tx, ty)| self.tile(tx, ty).terrain.is_water())))
        })
    }

    fn worker_routes(&self, unit: &Unit) -> WorkerRoutes {
        let start = self.map_wrap.index(unit.x, unit.y);
        let mut result = WorkerRoutes {
            map_wrap: self.map_wrap,
            costs: vec![i32::MAX; self.tiles.len()],
            previous: vec![None; self.tiles.len()],
            start,
        };
        result.costs[start] = 0;
        let mut queue = BinaryHeap::from([Reverse((0, 0, start))]);
        let allowance = self.unit_move_rate(unit).max(1);
        let mut sequence = 0;
        let mut actor = unit.clone();
        while let Some(Reverse((spent, _, index))) = queue.pop() {
            if result.costs[index] != spent {
                continue;
            }
            (actor.x, actor.y) = self.map_wrap.position(index);
            let remaining = if spent < unit.moves_left {
                unit.moves_left - spent
            } else {
                allowance - (spent - unit.moves_left.max(0)) % allowance
            };
            for (direction, (dx, dy)) in CIV3_DIRS.into_iter().enumerate() {
                let Some((x, y)) = self.map_wrap.civ3_neighbor(actor.x, actor.y, direction) else {
                    continue;
                };
                if self
                    .tile(x, y)
                    .owner
                    .is_some_and(|other| !self.allied(unit.owner, other))
                    || self.treaty_blocks_move(&actor, x, y)
                    || self.zone_blocks_step(&actor, (actor.x, actor.y), (x, y))
                    || self.any_unit_at(x, y, |u| !self.allied(unit.owner, u.owner))
                    || self.any_city_at(x, y, |c| !self.allied(unit.owner, c.owner))
                {
                    continue;
                }
                let cost = if !self.players[unit.owner].ai && !self.tile(x, y).known_by[unit.owner]
                {
                    Some(
                        super::movement_rules::MovementRules::campaign()
                            .unknown_tile_move_cost(unit.kind)
                            .expect("selected C2C3 unknown-tile movement cost"),
                    )
                } else {
                    self.movement_cost_for(&actor, x, y)
                };
                let Some(cost) = cost else {
                    continue;
                };
                let next = self.map_wrap.index(x, y);
                let candidate = spent + cost.min(remaining);
                if candidate < result.costs[next] {
                    result.costs[next] = candidate;
                    result.previous[next] = Some((index, (dx, dy)));
                    sequence += 1;
                    queue.push(Reverse((candidate, sequence, next)));
                }
            }
        }
        result
    }

    fn worker_shelter(&mut self, actor: &Unit) {
        let routes = self.worker_routes(actor);
        let target = self
            .cities
            .iter()
            .filter(|c| self.allied(actor.owner, c.owner))
            .filter_map(|c| {
                routes
                    .path((c.x, c.y))
                    .map(|path| (routes.costs[self.map_wrap.index(c.x, c.y)], path))
            })
            .min_by_key(|(cost, _)| *cost);
        if let Some((_, path)) = target {
            if !path.is_empty() && self.ai_worker_tasks.insert(actor.id) {
                let destination = path.iter().fold((actor.x, actor.y), |(x, y), &(dx, dy)| {
                    self.map_wrap
                        .civ3_offset(x, y, dx, dy)
                        .expect("planned step is real")
                });
                self.unit_goto_tiles.insert(actor.id, destination);
            }
            self.follow_civ3_path(actor.id, path);
        }
    }

    fn run_worker(
        &mut self,
        id: u32,
        state: &mut WorkMap,
        requests: &mut Vec<WorkerRequest>,
        depth: usize,
    ) {
        if depth > 5 && (depth as f64) > self.units.len() as f64 * 1.5 {
            state.orders.remove(&id);
            state.reservations.retain(|_, (worker, _)| *worker != id);
            self.ai_worker_tasks.remove(&id);
            self.unit_goto_tiles.remove(&id);
            return;
        }
        let Some(mut actor) = self.unit_by_id(id).cloned() else {
            return;
        };
        if actor.transport.is_some() {
            return;
        }
        if actor.work_left > 0 {
            if self.worker_safe_tile(actor.owner, actor.x, actor.y) {
                return;
            }
            self.units
                .iter_mut()
                .find(|u| u.id == id)
                .unwrap()
                .clear_work();
            actor.clear_work();
        }
        if actor.state == UnitState::Sentry {
            self.units.iter_mut().find(|u| u.id == id).unwrap().state = UnitState::Idle;
        }
        if matches!(actor.state, UnitState::Fortifying | UnitState::Fortified) {
            if self.worker_safe_tile(actor.owner, actor.x, actor.y) {
                return;
            }
            self.units.iter_mut().find(|u| u.id == id).unwrap().state = UnitState::Idle;
        }
        let Some(plan) = self.worker_plan(&actor, state, requests) else {
            if self.players[actor.owner].ai && depth == 0 {
                // Default AI's settler_run differs from the generic Human /
                // displaced FindWork callback: no seek-shelter here.
                if actor.kind.stats().can_found {
                    self.ai_worker_tasks.remove(&id);
                    self.unit_goto_tiles.remove(&id);
                    if matches!(self.ai_guard_state(id), guards::GuardState::Assigned(_)) {
                        self.clear_ai_guard(id);
                    }
                } else if self.ai_worker_tasks.insert(id) {
                    self.unit_goto_tiles.remove(&id); // first AUTO task, NULL target
                }
            } else {
                self.worker_shelter(&actor);
            }
            return;
        };
        if self.ai_worker_tasks.insert(id) {
            if actor.owner == HUMAN && plan.request.is_some() {
                self.unit_goto_tiles.remove(&id); // native generic request's best_tile is NULL
            } else {
                self.unit_goto_tiles.insert(id, plan.destination);
            }
        }
        state.reservations.retain(|_, (worker, _)| *worker != id);
        let displaced = state
            .reservations
            .insert(plan.destination, (id, plan.arrival))
            .map(|r| r.0)
            .filter(|other| *other != id);
        state.orders.insert(id, (plan.destination, plan.activity));
        if let Some(displaced) = displaced {
            state.orders.remove(&displaced);
            self.unit_goto_tiles.remove(&displaced); // native clears goto before FindWork recursion
            self.run_worker(displaced, state, requests, depth + 1);
            if state.orders.get(&id) != Some(&(plan.destination, plan.activity))
                || self
                    .units
                    .iter()
                    .find(|u| u.id == id)
                    .is_none_or(|u| (u.x, u.y) != (actor.x, actor.y) || u.work_left > 0)
            {
                return;
            }
        }
        self.follow_civ3_path(id, plan.path);
        if self
            .units
            .iter()
            .any(|u| u.id == id && (u.x, u.y) == plan.destination && u.moves_left > 0)
            && self.begin_work(id, plan.activity)
            && let Some(request) = plan.request
            && let Some(index) = requests.iter().position(|candidate| *candidate == request)
        {
            requests.remove(index);
        }
    }

    pub(super) fn run_worker_phase_with_requests(
        &mut self,
        owner: usize,
        requests: &mut Vec<WorkerRequest>,
    ) -> HashSet<u32> {
        let workers: Vec<_> = self
            .player_units_ordered(owner)
            .filter(|u| {
                u.moves_left > 0
                    && if self.players[owner].ai {
                        u.kind.is_worker()
                    } else {
                        u.automated && u.kind.is_worker()
                    }
            })
            .map(|u| u.id)
            .collect();
        let mut state = WorkMap::default();
        for &id in &workers {
            if self.players[owner].ai
                && let Some(actor) = self
                    .units
                    .iter()
                    .find(|u| u.id == id && u.kind == UnitKind::Settler)
                    .cloned()
            {
                if self.ai_settlement_targets.contains_key(&id) {
                    self.run_ai_founder_with_improvement(&actor, 0.0);
                    continue;
                }
                let want = self
                    .worker_plan(&actor, &state, requests)
                    .map_or(0.0, |plan| {
                        if plan.request.is_some() {
                            1.0
                        } else {
                            amortize(
                                (plan.new - plan.old + plan.extra) * WORKER_FACTOR,
                                plan.delay,
                            ) / WORKER_FACTOR
                        }
                    });
                if self.run_ai_founder_with_improvement(&actor, want) {
                    continue;
                }
            }
            self.run_worker(id, &mut state, requests, 0);
        }
        workers.into_iter().collect()
    }

    pub(super) fn run_ai_worker_phase(&mut self, owner: usize) -> HashSet<u32> {
        self.run_worker_phase_with_requests(owner, &mut Vec::new())
    }

    pub(super) fn worker_improvement_want(&mut self, actor: &Unit) -> f32 {
        self.worker_plan(actor, &WorkMap::default(), &[])
            .map_or(0.0, |plan| {
                amortize(
                    (plan.new - plan.old + plan.extra) * WORKER_FACTOR,
                    plan.delay,
                ) / WORKER_FACTOR
            })
    }

    pub(super) fn run_auto_worker(&mut self, id: u32) {
        self.run_worker(id, &mut WorkMap::default(), &mut Vec::new(), 0);
    }
}

#[cfg(test)]
#[path = "workers/tests.rs"]
mod tests;
