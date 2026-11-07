use super::*;

pub(super) const WORKER_FACTOR: f32 = 1024.0;
pub(super) const ACTIVITIES: &[WorkKind] = &[
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

#[derive(Clone)]
pub(super) struct WorkerPlan {
    pub(super) destination: (i32, i32),
    pub(super) activity: WorkKind,
    pub(super) path: Vec<(i32, i32)>,
    pub(super) arrival: i32,
    pub(super) delay: i32,
    pub(super) old: f32,
    pub(super) new: f32,
    pub(super) extra: f32,
    pub(super) worked: bool,
    pub(super) request: Option<WorkerRequest>,
}

#[derive(Default)]
pub(super) struct WorkMap {
    // This is native's calloc-per-phase workermap, not save-game state.
    pub(super) reservations: BTreeMap<(i32, i32), (u32, i32)>,
    pub(super) orders: BTreeMap<u32, ((i32, i32), WorkKind)>,
}

pub(super) fn amortize(value: f32, delay: i32) -> f32 {
    (f64::from(value) * (23.0_f64 / 24.0).powi(delay)) as f32
}

fn wants_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001 * (a.abs() + b.abs())
}

pub(super) fn consider_action(best: &mut Option<WorkerPlan>, plan: WorkerPlan) {
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
    pub(super) fn worker_projection(
        &self,
        actor: &Unit,
        activity: WorkKind,
    ) -> Option<(Tile, i32)> {
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

    pub(super) fn worker_tile_value(
        &mut self,
        city: &City,
        position: (i32, i32),
        tile: Tile,
    ) -> f32 {
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

    pub(super) fn worker_road_extra(&self, position: (i32, i32), railroad: bool) -> f32 {
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

    pub(super) fn worker_can_reserve(
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
                    && self.unit_by_id(id).is_none_or(|u| {
                        self.map_distance(actor.x, actor.y, position.0, position.1)
                            < self.map_distance(u.x, u.y, position.0, position.1)
                    }))
        })
    }

    pub(super) fn worker_plan(
        &mut self,
        actor: &Unit,
        state: &WorkMap,
        requests: &[WorkerRequest],
    ) -> Option<WorkerPlan> {
        let routes = self.worker_routes(actor);
        let allowance = self.unit_move_rate(actor).max(1);
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
            let cost = routes.costs[self
                .map_wrap
                .index(request.destination.0, request.destination.1)];
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
                let cost = routes.costs[self.map_wrap.index(x, y)];
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
}
