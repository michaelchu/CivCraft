use super::*;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashSet};

const WORKER_FACTOR: f32 = 1024.0;
const ACTIVITIES: &[WorkKind] = &[
    WorkKind::Cultivate,
    WorkKind::Plant,
    WorkKind::Transform,
    WorkKind::Irrigation,
    WorkKind::Mine,
    WorkKind::OilWell,
    WorkKind::OilPlatform,
    WorkKind::Clean(Contamination::Pollution),
    WorkKind::Farmland,
    WorkKind::Clean(Contamination::Fallout),
    WorkKind::Road,
    WorkKind::Railroad,
];

/// Native city requests are real input orders. The default AI produces none;
/// callers with a request producer can pass them to the same planner.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct WorkerRequest {
    pub(super) city: u32,
    pub(super) destination: (i32, i32),
    pub(super) activity: WorkKind,
    pub(super) want: i32,
}

#[derive(Clone)]
struct WorkerPlan {
    destination: (i32, i32),
    activity: WorkKind,
    path: Vec<(i32, i32)>,
    arrival: i32,
    delay: i32,
    old: f32,
    new: f32,
    extra: f32,
    worked: bool,
    request: Option<WorkerRequest>,
}

#[derive(Default)]
struct WorkMap {
    // This is native's calloc-per-phase workermap, not save-game state.
    reservations: BTreeMap<(i32, i32), (u32, i32)>,
    orders: BTreeMap<u32, ((i32, i32), WorkKind)>,
}

struct WorkerRoutes {
    costs: Vec<i32>,
    previous: Vec<Option<(usize, (i32, i32))>>,
    start: usize,
}

impl WorkerRoutes {
    fn path(&self, destination: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        let mut cursor = Simulation::idx(destination.0, destination.1);
        if self.costs[cursor] == i32::MAX {
            return None;
        }
        let mut path = Vec::new();
        while cursor != self.start {
            let (parent, step) = self.previous[cursor]?;
            path.push(step);
            cursor = parent;
        }
        path.reverse();
        Some(path)
    }
}

fn amortize(value: f32, delay: i32) -> f32 {
    (f64::from(value) * (23.0_f64 / 24.0).powi(delay)) as f32
}

fn wants_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001 * (a.abs() + b.abs())
}

fn consider_action(best: &mut Option<WorkerPlan>, plan: WorkerPlan) {
    // autoworkers.c:consider_worker_action. Until a worked tile competes,
    // unworked tiles compete on future total, rather than marginal gain.
    let improves = plan.new > plan.old || (wants_equal(plan.new, plan.old) && plan.extra > 0.0);
    if !improves && plan.extra <= 0.0 {
        return;
    }
    let Some(current) = best.as_ref() else {
        if plan.worked || improves {
            *best = Some(plan);
        }
        return;
    };
    if !current.worked && !plan.worked {
        if improves
            && (plan.new > current.new
                || (wants_equal(plan.new, current.new) && plan.old < current.old))
        {
            *best = Some(plan);
        }
        return;
    }
    let value = |candidate: &WorkerPlan| {
        amortize(
            ((candidate.new - candidate.old) / if candidate.worked { 1.0 } else { 2.0 }
                + candidate.extra)
                * WORKER_FACTOR,
            candidate.delay,
        )
    };
    let incoming = value(&plan);
    // Native stores this incumbent in an int, truncating the float want.
    let old = (if current.worked {
        value(current)
    } else {
        amortize(
            (current.new - current.old) * WORKER_FACTOR / 2.0,
            current.delay,
        )
    }) as i32 as f32;
    if incoming > old || (wants_equal(incoming, old) && plan.old > current.old) {
        *best = Some(plan);
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
                Terrain::Mountains | Terrain::Swamp | Terrain::Jungle
            )
            || tile.road
            || matches!(tile.base, Some(Base::Fort | Base::Fortress))
    }

    fn worker_safe_tile(&self, owner: usize, x: i32, y: i32) -> bool {
        // Native adv_worker_safe_tile accepts any guard on the worksite.
        if self
            .units
            .iter()
            .any(|u| (u.x, u.y) == (x, y) && Self::worker_guard(u.kind))
        {
            return true;
        }
        !self.units.iter().any(|u| {
            self.at_war(owner, u.owner)
                && (self.players[owner].ai || self.unit_visible_to(owner, u))
                && (u.kind.stats().attack > 0 || u.kind.is_diplomatic())
                // square_iterate uses real map distance, also on ISOHEX.
                && self.map_distance(x, y, u.x, u.y) <= 2
                && (self.worker_enemy_native(u.kind, x, y)
                    || (u.kind.is_naval() && u.kind != UnitKind::Submarine
                        && self.map_wrap.neighbors(x, y)
                            .any(|(tx, ty)| self.tile(tx, ty).terrain.is_water())))
        })
    }

    fn worker_routes(&self, unit: &Unit) -> WorkerRoutes {
        let start = Self::idx(unit.x, unit.y);
        let mut result = WorkerRoutes {
            costs: vec![i32::MAX; self.tiles.len()],
            previous: vec![None; self.tiles.len()],
            start,
        };
        result.costs[start] = 0;
        let mut queue = BinaryHeap::from([Reverse((0, 0, start))]);
        let allowance = self.unit_move_rate(&unit).max(1);
        let mut sequence = 0;
        let mut actor = unit.clone();
        while let Some(Reverse((spent, _, index))) = queue.pop() {
            if result.costs[index] != spent {
                continue;
            }
            (actor.x, actor.y) = Self::xy(index);
            let remaining = if spent < unit.moves_left {
                unit.moves_left - spent
            } else {
                allowance - (spent - unit.moves_left.max(0)) % allowance
            };
            for (x, y) in self.map_wrap.neighbors(actor.x, actor.y) {
                if self
                    .tile(x, y)
                    .owner
                    .is_some_and(|other| !self.allied(unit.owner, other))
                    || self.treaty_blocks_move(&actor, x, y)
                    || self.zone_blocks_step(&actor, (actor.x, actor.y), (x, y))
                    || self
                        .units
                        .iter()
                        .any(|u| (u.x, u.y) == (x, y) && !self.allied(unit.owner, u.owner))
                    || self
                        .cities
                        .iter()
                        .any(|c| (c.x, c.y) == (x, y) && !self.allied(unit.owner, c.owner))
                {
                    continue;
                }
                let cost = if !self.players[unit.owner].ai && !self.tile(x, y).known_by[unit.owner]
                {
                    // Native unit unknown-terrain cost is twice the highest
                    // native terrain cost for this land class.
                    Some(if unit.kind.is_naval() { 2 } else { 6 } * MOVE_FRAGMENTS)
                } else {
                    self.movement_cost_for(&actor, x, y)
                };
                let Some(cost) = cost else {
                    continue;
                };
                let next = Self::idx(x, y);
                let candidate = spent + cost.min(remaining);
                if candidate < result.costs[next] {
                    result.costs[next] = candidate;
                    result.previous[next] =
                        Some((index, self.map_wrap.vector(actor.x, actor.y, x, y)));
                    sequence += 1;
                    queue.push(Reverse((candidate, sequence, next)));
                }
            }
        }
        result
    }

    fn worker_projection(&self, actor: &Unit, activity: WorkKind) -> Option<(Tile, i32)> {
        if !actor.kind.is_worker() || actor.transport.is_some() {
            return None;
        }
        let mut tile = self.tile(actor.x, actor.y).clone();
        let time = match activity {
            WorkKind::Road
                if !tile.road
                    && (!tile.river || self.has_tech(actor.owner, Tech::BRIDGE_BUILDING)) =>
            {
                tile.road = true;
                tile.terrain.road_time()
            }
            WorkKind::Railroad if self.can_railroad_at(actor.owner, actor.x, actor.y) => {
                tile.railroad = true;
                3
            }
            WorkKind::Irrigation
                if actor.kind.unit_class() != super::unit_catalog::UnitClass::Sea
                    && self.can_irrigate_at(actor.owner, actor.x, actor.y) =>
            {
                tile.irrigation = true;
                tile.mine = false;
                tile.oil_well = false;
                tile.terrain.irrigation_time()
            }
            WorkKind::Mine
                if !tile.mine && actor.kind.unit_class() != super::unit_catalog::UnitClass::Sea =>
            {
                tile.mine = true;
                tile.irrigation = false;
                tile.farmland = false;
                tile.terrain.mine_time()
            }
            WorkKind::Farmland
                if actor.kind.unit_class() != super::unit_catalog::UnitClass::Sea
                    && self.can_farm_at(actor.owner, actor.x, actor.y) =>
            {
                tile.farmland = true;
                tile.terrain.irrigation_time()
            }
            WorkKind::OilWell if self.oil_work(actor) == Some(activity) => {
                tile.oil_well = true;
                tile.terrain.mine_time()
            }
            WorkKind::OilPlatform if self.oil_work(actor) == Some(activity) => {
                tile.oil_platform = true;
                10
            }
            WorkKind::Clean(target) if actor.kind.can_clean() && tile.has_contamination(target) => {
                match target {
                    Contamination::Pollution => tile.pollution = false,
                    Contamination::Fallout => tile.fallout = false,
                };
                3
            }
            WorkKind::Cultivate | WorkKind::Plant | WorkKind::Transform => {
                let (terrain, time) = self.terrain_work(actor, activity)?;
                tile.terrain = terrain;
                if terrain.irrigation_time() == 0 {
                    tile.irrigation = false;
                    tile.farmland = false;
                }
                if terrain.mine_time() == 0 {
                    tile.mine = false;
                }
                if !matches!(terrain, Terrain::Desert | Terrain::Glacier) {
                    tile.oil_well = false;
                }
                if terrain != Terrain::DeepOcean {
                    tile.oil_platform = false;
                }
                if matches!(terrain, Terrain::Lake | Terrain::DeepOcean) {
                    tile.pollution = false;
                    tile.fallout = false;
                }
                if terrain.is_water() {
                    tile.road = false;
                    tile.railroad = false;
                    tile.river = false;
                    tile.hut = false;
                    tile.base = None;
                }
                time
            }
            _ => return None,
        };
        (time > 0).then_some((tile, time))
    }

    fn worker_tile_value(&mut self, city: &City, position: (i32, i32), tile: Tile) -> f32 {
        // Infrastructure caches evaluate a virtual tile. Swapping without
        // callbacks likewise leaves ownership, units, activity and RNG intact.
        let original = std::mem::replace(self.tile_mut(position.0, position.1), tile);
        let (food, shield, trade) = self.city_tile_yields(
            city,
            position.0,
            position.1,
            (city.x, city.y) == position,
            None,
        );
        *self.tile_mut(position.0, position.1) = original;
        let mut value = (food * 30) as f32;
        if food > 0 {
            value += 15.0;
        }
        value += (shield * 17) as f32;
        if shield > 0 {
            value += 8.5;
        }
        value += (trade * 18) as f32;
        if trade > 0 {
            value += 9.0;
        }
        value
    }

    fn worker_road_extra(&self, position: (i32, i32), railroad: bool) -> f32 {
        // The provided ruleset uses hex topology: native's hex road connector
        // heuristic deliberately samples these twelve map-coordinate offsets.
        let offsets = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
            (0, -2),
            (-2, 0),
            (2, 0),
            (0, 2),
        ];
        let mut absent = 0;
        for (dx, dy) in offsets {
            let Some((x, y)) = self.map_wrap.offset(position.0, position.1, dx, dy) else {
                continue;
            };
            let real = {
                let tile = self.tile(x, y);
                (if railroad { tile.railroad } else { tile.road })
                    || self.units.iter().any(|u| {
                        (u.x, u.y) == (x, y)
                            && u.work_kind
                                == Some(if railroad {
                                    WorkKind::Railroad
                                } else {
                                    WorkKind::Road
                                })
                    })
            };
            if !real {
                absent += 1;
            }
        }
        let bonus = (20 + 3 * absent) / 20;
        let tile = self.tile(position.0, position.1);
        let old = if tile.road {
            2
        } else {
            tile.terrain.movement_cost() * MOVE_FRAGMENTS
        };
        let new = if railroad { 1 } else { 2 };
        (bonus * if new < old { 1 - new + old } else { 1 }) as f32
    }

    fn worker_can_reserve(
        &self,
        actor: &Unit,
        position: (i32, i32),
        arrival: i32,
        state: &WorkMap,
    ) -> bool {
        state.reservations.get(&position).is_none_or(|&(id, eta)| {
            id == actor.id
                || arrival < eta
                || (arrival == eta
                    && self.units.iter().find(|u| u.id == id).is_none_or(|u| {
                        self.map_distance(actor.x, actor.y, position.0, position.1)
                            < self.map_distance(u.x, u.y, position.0, position.1)
                    }))
        })
    }

    fn worker_plan(
        &mut self,
        actor: &Unit,
        state: &WorkMap,
        requests: &[WorkerRequest],
    ) -> Option<WorkerPlan> {
        let routes = self.worker_routes(actor);
        let allowance = self.unit_move_rate(&actor).max(1);
        let occupied = |sim: &Self, position| {
            sim.units.iter().any(|u| {
                u.id != actor.id
                    && u.owner == actor.owner
                    && u.kind.is_worker()
                    && (u.x, u.y) == position
            })
        };
        let mut best_request = None;
        let mut request_value = -1;
        let mut request_distance = i32::MAX;
        for request in requests.iter() {
            if !self
                .cities
                .iter()
                .any(|c| c.id == request.city && c.owner == actor.owner)
                || occupied(self, request.destination)
            {
                continue;
            }
            let cost = routes.costs[Self::idx(request.destination.0, request.destination.1)];
            if cost == i32::MAX {
                continue;
            }
            let arrival = (cost + allowance - actor.moves_left.max(0)) / allowance;
            if !self.worker_can_reserve(actor, request.destination, arrival, state) {
                continue;
            }
            let mut at = actor.clone();
            (at.x, at.y) = request.destination;
            if self.worker_projection(&at, request.activity).is_none() {
                continue;
            }
            let value = (request.want + 1) * 10 / (arrival + 1);
            if value > request_value && arrival < request_distance {
                request_value = value;
                request_distance = arrival;
                best_request = Some(WorkerPlan {
                    destination: request.destination,
                    activity: request.activity,
                    path: routes.path(request.destination)?,
                    arrival,
                    delay: arrival,
                    old: 0.0,
                    new: 0.0,
                    extra: 0.0,
                    worked: true,
                    request: Some(request.clone()),
                });
            }
        }
        if best_request.is_some() {
            return best_request;
        }
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|c| c.owner == actor.owner)
            .cloned()
            .collect();
        let mut best = None;
        for city in cities {
            for (_, _, x, y) in self.map_city_radius(&city) {
                let position = (x, y);
                let worked = (city.x, city.y) == position || city.worked_tiles.contains(&position);
                if (!worked && !self.can_assign_city_tile(&city, x, y))
                    || !self.worker_safe_tile(actor.owner, x, y)
                    || occupied(self, position)
                {
                    continue;
                }
                let cost = routes.costs[Self::idx(x, y)];
                if cost == i32::MAX {
                    continue;
                }
                let arrival = (cost + allowance - actor.moves_left.max(0)) / allowance;
                if !self.worker_can_reserve(actor, position, arrival, state) {
                    continue;
                }
                let remaining = if cost < actor.moves_left {
                    actor.moves_left - cost
                } else {
                    let remainder = (cost - actor.moves_left.max(0)) % allowance;
                    if remainder == 0 {
                        0
                    } else {
                        allowance - remainder
                    }
                };
                let mut at = actor.clone();
                (at.x, at.y) = position;
                let old = self.worker_tile_value(&city, position, self.tile(x, y).clone());
                for &activity in ACTIVITIES {
                    let Some((tile, time)) = self.worker_projection(&at, activity) else {
                        continue;
                    };
                    let new = self.worker_tile_value(&city, position, tile);
                    let extra = match activity {
                        WorkKind::Road => self.worker_road_extra(position, false),
                        WorkKind::Railroad => self.worker_road_extra(position, true),
                        WorkKind::Clean(target) => self.worker_climate_urgency(target) as f32,
                        _ => 0.0,
                    };
                    let work = (time * ACTIVITY_FACTOR + actor.activity_rate() - 1)
                        / actor.activity_rate();
                    let delay = arrival + work + i32::from(remaining == 0);
                    consider_action(
                        &mut best,
                        WorkerPlan {
                            destination: position,
                            activity,
                            path: routes.path(position)?,
                            arrival,
                            delay,
                            old,
                            new,
                            extra,
                            worked,
                            request: None,
                        },
                    );
                }
                // A missing road can be worth building for its researched
                // railroad successor; source adds both cached total values.
                if !self.tile(x, y).road
                    && self.has_tech(actor.owner, Tech::RAILROAD)
                    && let Some((road, time)) = self.worker_projection(&at, WorkKind::Road)
                {
                    let road_value = self.worker_tile_value(&city, position, road.clone());
                    let mut rail = road;
                    rail.railroad = true;
                    let new = self.worker_tile_value(&city, position, rail) + road_value;
                    let work = ((time + 3) * ACTIVITY_FACTOR + actor.activity_rate() - 1)
                        / actor.activity_rate();
                    consider_action(
                        &mut best,
                        WorkerPlan {
                            destination: position,
                            activity: WorkKind::Road,
                            path: routes.path(position)?,
                            arrival,
                            delay: arrival + work + i32::from(remaining == 0),
                            old,
                            new,
                            extra: self.worker_road_extra(position, true),
                            worked,
                            request: None,
                        },
                    );
                }
            }
        }
        best.filter(|plan| {
            amortize(
                (plan.new - plan.old + plan.extra) * WORKER_FACTOR,
                plan.delay,
            ) > 0.0
        })
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
                    .map(|path| (routes.costs[Self::idx(c.x, c.y)], path))
            })
            .min_by_key(|(cost, _)| *cost);
        if let Some((_, path)) = target {
            if !path.is_empty() && self.ai_worker_tasks.insert(actor.id) {
                let destination = path.iter().fold((actor.x, actor.y), |(x, y), &(dx, dy)| {
                    self.map_wrap
                        .offset(x, y, dx, dy)
                        .expect("planned step is real")
                });
                self.unit_goto_tiles.insert(actor.id, destination);
            }
            self.follow_path(actor.id, path);
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
        let Some(mut actor) = self.units.iter().find(|u| u.id == id).cloned() else {
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
        self.follow_path(id, plan.path);
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
                    && if owner == HUMAN {
                        u.automated && u.kind.is_worker()
                    } else {
                        u.kind.is_worker()
                    }
            })
            .map(|u| u.id)
            .collect();
        let mut state = WorkMap::default();
        for &id in &workers {
            if owner != HUMAN
                && let Some(actor) = self
                    .units
                    .iter()
                    .find(|u| u.id == id && u.kind == UnitKind::Settlers)
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

    /// Desktop regression: remote cleanup selection, a closer worker taking
    /// the reservation, and deterministic shelter once no jobs remain.
    pub(crate) fn ai_worker_planning_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.road = true;
            tile.railroad = true;
            tile.irrigation = true;
            tile.farmland = true;
            tile.mine = false;
            tile.oil_well = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.base = None;
        }
        self.create_city(1, 20, 20);
        self.tile_mut(20, 20).pollution = true;
        self.climate_current[0] = 1000;
        let far = offset_native_by_map(20, 20, -4, 0);
        let near = offset_native_by_map(20, 20, 1, 0);
        // Road-only approach makes the far worker arrive next phase.
        for tile in &mut self.tiles {
            tile.railroad = false;
        }
        let near_id = self.spawn_unit(1, UnitKind::Engineer, near.0, near.1);
        // Native player lists prepend: create the far worker last so it
        // reserves first and the nearer worker must actually displace it.
        let far_id = self.spawn_unit(1, UnitKind::Settlers, far.0, far.1);
        let actor = self.units.iter().find(|u| u.id == far_id).unwrap().clone();
        let remote = self
            .worker_plan(&actor, &WorkMap::default(), &[])
            .is_some_and(|p| {
                p.destination == (20, 20) && p.activity == WorkKind::Clean(Contamination::Pollution)
            });
        self.run_ai_worker_phase(1);
        let reserved = self.units.iter().any(|u| {
            u.id == near_id
                && (u.x, u.y) == (20, 20)
                && u.work_kind == Some(WorkKind::Clean(Contamination::Pollution))
        }) && self
            .units
            .iter()
            .find(|u| u.id == far_id)
            .is_some_and(|u| u.work_kind != Some(WorkKind::Clean(Contamination::Pollution)));
        self.units.clear();
        self.tile_mut(20, 20).pollution = false;
        self.climate_current[0] = 0;
        self.cities[0].owner = HUMAN;
        self.players[HUMAN].known.clear();
        for tile in &mut self.tiles {
            tile.owner = None;
        }
        let shelter = self.spawn_unit(HUMAN, UnitKind::Worker, near.0, near.1);
        self.units.last_mut().unwrap().automated = true;
        // Generic Human automation (also the displaced-worker callback)
        // seeks shelter. Default AI phase-entry workers deliberately do not.
        self.run_ai_worker_phase(HUMAN);
        let sheltered = self
            .units
            .iter()
            .any(|u| u.id == shelter && (u.x, u.y) == (20, 20));
        (remote, reserved, sheltered)
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

impl Simulation {
    pub(crate) fn transport_auto_worker_playtest(&self) -> bool {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.players[1].known.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.owner = None;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
            tile.known_by[1] = true;
        }
        let center = (10, 24);
        let target = offset_native_by_map(center.0, center.1, 1, 0);
        game.tile_mut(center.0, center.1).terrain = Terrain::Grassland;
        game.create_city(1, center.0, center.1);
        game.cities[0].worked_tiles = vec![target];
        game.cities[0].specialists = [0; 3];
        game.tile_mut(target.0, target.1).pollution = true;
        let ship = game.spawn_unit(1, UnitKind::Transport, center.0, center.1);
        game.run_worker_phase_with_requests(1, &mut Vec::new());
        let assigned = game.units.iter().any(|unit| {
            unit.id == ship
                && (unit.x, unit.y) == target
                && unit.work_kind == Some(WorkKind::Clean(Contamination::Pollution))
        });
        game.progress_player_unit_activities(1);
        assigned && !game.tile(target.0, target.1).pollution
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.river = false;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.hut = false;
            tile.base = None;
        }
        game
    }

    fn plan(old: f32, new: f32, worked: bool, delay: i32) -> WorkerPlan {
        WorkerPlan {
            destination: (10, 20),
            activity: WorkKind::Irrigation,
            path: vec![],
            arrival: 0,
            delay,
            old,
            new,
            extra: 0.0,
            worked,
            request: None,
        }
    }

    #[test]
    fn a_sea_worker_uses_the_actual_sea_pathfinder_and_ocean_cleanup_requests() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.transport_auto_worker_playtest());
    }

    #[test]
    fn sea_worker_routes_reject_known_land_and_use_native_unknown_terrain_cost() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.known_by[HUMAN] = true;
        }
        let start = (10, 24);
        let sea = offset_native_by_map(start.0, start.1, 1, 0);
        let land = offset_native_by_map(start.0, start.1, 0, 1);
        game.tile_mut(start.0, start.1).terrain = Terrain::Ocean;
        game.tile_mut(sea.0, sea.1).terrain = Terrain::Ocean;
        game.tile_mut(sea.0, sea.1).known_by[HUMAN] = false;
        game.spawn_unit(HUMAN, UnitKind::Transport, start.0, start.1);
        let routes = game.worker_routes(game.units.last().unwrap());
        assert_eq!(routes.costs[Simulation::idx(land.0, land.1)], i32::MAX);
        assert_eq!(
            routes.costs[Simulation::idx(sea.0, sea.1)],
            2 * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn default_ai_plans_remote_work_displaces_reservations_and_shelters() {
        let mut game = game();
        assert_eq!(game.ai_worker_planning_playtest(), (true, true, true));
    }

    #[test]
    fn unworked_candidates_compare_future_total_until_worked_tiles_compete() {
        let mut best = None;
        consider_action(&mut best, plan(20.0, 100.0, false, 1));
        consider_action(&mut best, plan(90.0, 110.0, false, 20));
        assert_eq!(best.as_ref().unwrap().new, 110.0);
        consider_action(&mut best, plan(80.0, 105.0, true, 1));
        assert!(best.as_ref().unwrap().worked);
        consider_action(&mut best, plan(50.0, 90.0, false, 1));
        assert!(
            best.as_ref().unwrap().worked,
            "unworked gain is halved against worked gain"
        );
        consider_action(&mut best, plan(50.0, 110.0, false, 1));
        assert!(!best.as_ref().unwrap().worked);
    }

    #[test]
    fn worker_safety_uses_hostile_actions_native_terrain_and_real_isohex_radius() {
        let mut game = game();
        let position = (10, 20);
        let far = offset_native_by_map(10, 20, 2, -2);
        let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, far.0, far.1);
        assert!(
            game.worker_safe_tile(1, 10, 20),
            "ISOHEX forbidden corner is distance four"
        );
        let near = offset_native_by_map(10, 20, 1, -1);
        let actor = game.units.iter_mut().find(|u| u.id == enemy).unwrap();
        (actor.x, actor.y) = near;
        actor.kind = UnitKind::Diplomat;
        assert!(
            !game.worker_safe_tile(1, 10, 20),
            "zero-attack hostile diplomatic actions count"
        );
        game.units.iter_mut().find(|u| u.id == enemy).unwrap().kind = UnitKind::Cannon;
        game.tile_mut(position.0, position.1).terrain = Terrain::Mountains;
        assert!(
            game.worker_safe_tile(1, 10, 20),
            "BigLand cannot threaten a non-native mountain"
        );
        game.tile_mut(10, 20).road = true;
        assert!(!game.worker_safe_tile(1, 10, 20));
        let guard = game.spawn_unit(1, UnitKind::Transport, 10, 20);
        assert!(
            !game.worker_safe_tile(1, 10, 20),
            "NonMil transport is not a guard"
        );
        game.units.iter_mut().find(|u| u.id == guard).unwrap().kind = UnitKind::Carrier;
        assert!(
            game.worker_safe_tile(1, 10, 20),
            "zero-attack military carrier is a guard"
        );
    }

    #[test]
    fn worker_routes_do_not_cross_foreign_territory_and_clip_last_move() {
        let mut game = game();
        let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
        let actor = game.units.iter().find(|u| u.id == id).unwrap().clone();
        let neighbor = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(neighbor.0, neighbor.1).terrain = Terrain::Mountains;
        let routes = game.worker_routes(&actor);
        assert_eq!(
            routes.costs[Simulation::idx(neighbor.0, neighbor.1)],
            actor.moves_left
        );
        game.tile_mut(neighbor.0, neighbor.1).owner = Some(HUMAN);
        assert!(game.worker_routes(&actor).path(neighbor).is_none());
    }

    #[test]
    fn same_worker_task_preserves_goal_and_displaced_null_goal_is_not_recreated() {
        let mut game = game();
        game.create_city(1, 10, 20);
        game.tile_mut(10, 20).road = false;
        let city = game.cities.last().unwrap().id;
        let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
        let mut requests = vec![WorkerRequest {
            city,
            destination: (10, 20),
            activity: WorkKind::Road,
            want: 1,
        }];
        game.run_worker_phase_with_requests(1, &mut requests);
        assert!(game.ai_worker_tasks.contains(&id));
        assert_eq!(game.unit_goto_tiles.get(&id), Some(&(10, 20)));
        game.units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap()
            .clear_work();
        let near = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(10, 20).road = true;
        requests.push(WorkerRequest {
            city,
            destination: near,
            activity: WorkKind::Road,
            want: 1,
        });
        game.run_worker_phase_with_requests(1, &mut requests);
        assert_eq!(
            game.unit_goto_tiles.get(&id),
            Some(&(10, 20)),
            "same AUT_AUTO_WORKER role does not rewrite its goto"
        );
        game.unit_goto_tiles.remove(&id); // native displacement clears goto, not role
        game.units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap()
            .clear_work();
        game.units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap()
            .moves_left = 6;
        game.tile_mut(near.0, near.1).road = false;
        requests.clear();
        requests.push(WorkerRequest {
            city,
            destination: near,
            activity: WorkKind::Road,
            want: 1,
        });
        game.run_worker_phase_with_requests(1, &mut requests);
        assert!(!game.unit_goto_tiles.contains_key(&id));
    }

    #[test]
    fn idle_default_ai_worker_stays_put_while_human_automation_seeks_shelter() {
        let mut game = game();
        assert_eq!(game.ai_worker_planning_playtest(), (true, true, true));
        game.units.clear();
        game.cities[0].owner = 1;
        let near = offset_native_by_map(20, 20, 1, 0);
        let id = game.spawn_unit(1, UnitKind::Worker, near.0, near.1);
        game.players[1].known.clear();
        game.run_ai_worker_phase(1);
        let actor = game.units.iter().find(|u| u.id == id).unwrap();
        assert_eq!(
            (actor.x, actor.y),
            near,
            "default AI callback has no generic seek-shelter fallback"
        );
    }

    #[test]
    fn explicit_city_request_starts_work_and_removes_only_fulfilled_request() {
        let mut game = game();
        game.create_city(1, 10, 20);
        let city = game.cities.last().unwrap().id;
        game.tile_mut(10, 20).road = false;
        let id = game.spawn_unit(1, UnitKind::Migrants, 10, 20);
        let mut requests = vec![WorkerRequest {
            city,
            destination: (10, 20),
            activity: WorkKind::Road,
            want: 1,
        }];
        let processed = game.run_worker_phase_with_requests(1, &mut requests);
        assert!(
            processed.contains(&id),
            "Migrants carry the Workers ruleset flag"
        );
        assert!(requests.is_empty());
        let worker = game.units.iter().find(|u| u.id == id).unwrap();
        assert_eq!(worker.work_kind, Some(WorkKind::Road));
        assert!(worker.work_left > 0);
    }

    #[test]
    fn closer_worker_can_displace_an_arrival_reservation_but_not_active_work() {
        let mut game = game();
        let far = game.spawn_unit(1, UnitKind::Worker, 10, 20);
        let near = game.spawn_unit(1, UnitKind::Worker, 11, 20);
        let actor = game.units.iter().find(|u| u.id == near).unwrap().clone();
        let mut state = WorkMap::default();
        state.reservations.insert((12, 20), (far, 2));
        assert!(game.worker_can_reserve(&actor, (12, 20), 1, &state));
        assert!(!game.worker_can_reserve(&actor, (12, 20), 3, &state));
        game.tile_mut(11, 20).pollution = true;
        assert!(game.begin_work(near, WorkKind::Clean(Contamination::Pollution)));
        let before = bincode::serialize(game.units.iter().find(|u| u.id == near).unwrap()).unwrap();
        game.run_auto_worker(near);
        assert_eq!(
            before,
            bincode::serialize(game.units.iter().find(|u| u.id == near).unwrap()).unwrap()
        );
    }

    #[test]
    fn virtual_evaluation_restores_tiles_and_shelter_does_not_randomly_wander() {
        let mut game = game();
        game.create_city(1, 10, 20);
        let city = game.cities.last().unwrap().clone();
        let id = game.spawn_unit(1, UnitKind::Worker, 10, 20);
        let actor = game.units.iter().find(|u| u.id == id).unwrap().clone();
        let before = bincode::serialize(&game.tiles).unwrap();
        let mut irrigated = game.tile(10, 20).clone();
        irrigated.irrigation = true;
        game.worker_tile_value(&city, (10, 20), irrigated);
        assert_eq!(before, bincode::serialize(&game.tiles).unwrap());
        game.cities.clear();
        let before = bincode::serialize(&game).unwrap();
        game.run_auto_worker(id);
        assert_eq!(
            before,
            bincode::serialize(&game).unwrap(),
            "no useful work or shelter preserves RNG and position"
        );
        assert_eq!(actor.id, id);
    }
}
