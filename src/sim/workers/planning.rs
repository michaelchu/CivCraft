//! Candidate scoring and plan construction for the parent automation system.

use super::*;

pub(super) const WORKER_FACTOR: f32 = 1024.0;
pub(super) const ACTIVITIES: &[WorkKind] = &[
    WorkKind::Cultivate,
    WorkKind::Plant,
    WorkKind::Irrigation,
    WorkKind::Mine,
    WorkKind::OilWell,
    WorkKind::OilPlatform,
    WorkKind::Clean(Contamination::Pollution),
    WorkKind::Farmland,
    WorkKind::Clean(Contamination::Fallout),
    WorkKind::Clean(Contamination::Craters),
    WorkKind::Road,
    WorkKind::Railroad,
];

// Freeciv autoworkers.c's square-topology neighborhood, expressed in map
// coordinates. The first eight positions surround the candidate tile; the
// remaining four extend the network two steps cardinally.
const ROAD_BONUS_OFFSETS: [(i32, i32); 12] = [
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

type WorkerActivityTiles = Vec<bool>;
type CachedWorkerProjection = Option<Option<(Tile, i32)>>;
type WorkerActivityProjections = [CachedWorkerProjection; ACTIVITIES.len()];
type WorkerProjectionCache = BTreeMap<(usize, usize, usize), WorkerActivityProjections>;
type RoadRailProjectionCache = BTreeMap<(usize, usize, usize), Option<(Tile, i32)>>;

pub(super) struct RoadBonusCache {
    roads: WorkerActivityTiles,
    railroads: WorkerActivityTiles,
    values: Vec<[Option<f32>; 2]>,
}

fn freeciv_square_road_connector_bonus(
    real_road: &[bool; ROAD_BONUS_OFFSETS.len()],
    potential_road: &[bool; ROAD_BONUS_OFFSETS.len()],
    is_slow: &[bool; ROAD_BONUS_OFFSETS.len()],
) -> i32 {
    // Transcribed from autoworkers.c::adv_workers_road_bonus's non-hex
    // topology branch. Each pattern recognizes a useful road continuation.
    let mut bonus = 0;
    if potential_road[0]
        && !real_road[1]
        && !real_road[3]
        && (!real_road[2] || !real_road[8])
        && (!is_slow[2] || !is_slow[4] || !is_slow[7] || !is_slow[6] || !is_slow[5])
    {
        bonus += 1;
    }
    if potential_road[2]
        && !real_road[1]
        && !real_road[4]
        && (!real_road[7] || !real_road[10])
        && (!is_slow[0] || !is_slow[3] || !is_slow[7] || !is_slow[6] || !is_slow[5])
    {
        bonus += 1;
    }
    if potential_road[5]
        && !real_road[6]
        && !real_road[3]
        && (!real_road[5] || !real_road[11])
        && (!is_slow[2] || !is_slow[4] || !is_slow[7] || !is_slow[1] || !is_slow[0])
    {
        bonus += 1;
    }
    if potential_road[7]
        && !real_road[6]
        && !real_road[4]
        && (!real_road[0] || !real_road[9])
        && (!is_slow[2] || !is_slow[3] || !is_slow[0] || !is_slow[1] || !is_slow[5])
    {
        bonus += 1;
    }

    if potential_road[1]
        && !real_road[4]
        && !real_road[3]
        && (!is_slow[5] || !is_slow[6] || !is_slow[7])
    {
        bonus += 1;
    }
    if potential_road[3]
        && !real_road[1]
        && !real_road[6]
        && (!is_slow[2] || !is_slow[4] || !is_slow[7])
    {
        bonus += 1;
    }
    if potential_road[4]
        && !real_road[1]
        && !real_road[6]
        && (!is_slow[0] || !is_slow[3] || !is_slow[5])
    {
        bonus += 1;
    }
    if potential_road[6]
        && !real_road[4]
        && !real_road[3]
        && (!is_slow[0] || !is_slow[1] || !is_slow[2])
    {
        bonus += 1;
    }
    bonus
}

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

#[derive(Clone, Default)]
struct WorkerTileValueCache {
    old: Option<f32>,
    projected: [Option<f32>; ACTIVITIES.len()],
    road_then_rail: Option<f32>,
}

type WorkerDemographics = (
    std::sync::Arc<[usize]>,
    BTreeMap<(usize, usize), (i32, i32)>,
);

#[derive(Default)]
pub(in crate::sim) struct WorkMap {
    // This is native's calloc-per-phase workermap, not save-game state.
    pub(super) reservations: BTreeMap<(i32, i32), (u32, i32)>,
    pub(super) orders: BTreeMap<u32, ((i32, i32), WorkKind)>,
    // Freeciv's infrastructure cache computes city/tile improvement values
    // once for an auto-worker pass. Routing, arrival and reservations remain
    // worker-specific, while these yield projections are shared by every real
    // or virtual worker considered in the phase.
    pub(in crate::sim) worker_demographics: Option<WorkerDemographics>,
    infrastructure: BTreeMap<u32, Vec<WorkerTileValueCache>>,
    // Work action eligibility and terrain projection depend on player, unit
    // type, tile, and activity. This cache is phase-local and is invalidated
    // when a founder creates a city during the worker phase.
    projections: WorkerProjectionCache,
    road_rail_projections: RoadRailProjectionCache,
    pub(super) road_bonuses: Option<RoadBonusCache>,
    safety_owner: Option<usize>,
    safety: Vec<Option<bool>>,
}

impl WorkMap {
    pub(super) fn clear_projections(&mut self) {
        self.projections.clear();
        self.road_rail_projections.clear();
        self.road_bonuses = None;
    }

    pub(super) fn prepare_road_bonuses(
        &mut self,
        roads: &WorkerActivityTiles,
        railroads: &WorkerActivityTiles,
    ) {
        // infracache.c deliberately leaves road bonuses dynamic because
        // workers can start building connectors during the same phase.
        // Reuse them only while that activity map is unchanged. Terrain
        // changes from founding a city call clear_projections as well.
        if self
            .road_bonuses
            .as_ref()
            .is_none_or(|cache| cache.roads != *roads || cache.railroads != *railroads)
        {
            self.road_bonuses = Some(RoadBonusCache {
                roads: roads.clone(),
                railroads: railroads.clone(),
                values: vec![[None; 2]; roads.len()],
            });
        }
    }
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
    fn cached_worker_safe_tile(
        &self,
        state: &mut WorkMap,
        owner: usize,
        position: (i32, i32),
    ) -> bool {
        if state.safety_owner != Some(owner) || state.safety.len() != self.tiles.len() {
            state.safety_owner = Some(owner);
            state.safety.clear();
            state.safety.resize(self.tiles.len(), None);
        }
        let index = self.map_wrap.index(position.0, position.1);
        if let Some(safe) = state.safety[index] {
            self.trace.count_operation("worker.safety_cache_hits", 1);
            return safe;
        }
        self.trace.count_operation("worker.safety_cache_misses", 1);
        let safe = self.worker_safe_tile(owner, position.0, position.1);
        state.safety[index] = Some(safe);
        safe
    }

    fn cached_worker_old_value(
        &mut self,
        values: &mut WorkerTileValueCache,
        city: &City,
        position: (i32, i32),
    ) -> f32 {
        if let Some(value) = values.old {
            self.trace.count_operation("worker.value_cache_hits", 1);
            self.trace.count_operation("worker.projections_avoided", 1);
            return value;
        }
        self.trace.count_operation("worker.value_cache_misses", 1);
        let value =
            self.worker_tile_value(city, position, self.tile(position.0, position.1).clone());
        values.old = Some(value);
        value
    }

    fn cached_worker_projected_value(
        &mut self,
        values: &mut WorkerTileValueCache,
        city: &City,
        position: (i32, i32),
        activity_index: usize,
        tile: Tile,
    ) -> f32 {
        if let Some(value) = values.projected[activity_index] {
            self.trace.count_operation("worker.value_cache_hits", 1);
            self.trace.count_operation("worker.projections_avoided", 1);
            return value;
        }
        self.trace.count_operation("worker.value_cache_misses", 1);
        let value = self.worker_tile_value(city, position, tile);
        values.projected[activity_index] = Some(value);
        value
    }

    fn cached_worker_road_then_rail_value(
        &mut self,
        values: &mut WorkerTileValueCache,
        city: &City,
        position: (i32, i32),
        road: Tile,
    ) -> f32 {
        if let Some(value) = values.road_then_rail {
            self.trace.count_operation("worker.value_cache_hits", 2);
            self.trace.count_operation("worker.projections_avoided", 2);
            return value;
        }
        self.trace.count_operation("worker.value_cache_misses", 2);
        let road_value = self.worker_tile_value(city, position, road.clone());
        let mut rail = road;
        rail.railroad = true;
        let value = self.worker_tile_value(city, position, rail) + road_value;
        values.road_then_rail = Some(value);
        value
    }

    pub(super) fn worker_projection(
        &self,
        actor: &Unit,
        activity: WorkKind,
    ) -> Option<(Tile, i32)> {
        if !actor.kind.is_worker()
            || actor.transport.is_some()
            || !self.work_action_available_for_unit_fail_closed(actor, activity, (actor.x, actor.y))
        {
            return None;
        }
        let mut tile = self.tile(actor.x, actor.y).clone();
        let time = match activity {
            WorkKind::Road if !tile.road => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::Railroad if !tile.railroad => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::Irrigation if !tile.irrigation => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::Mine if !tile.mine => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::Farmland if !tile.farmland => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::OilWell if !tile.oil_well => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::OilPlatform if !tile.oil_platform => {
                if !super::super::action_rules::apply_freeciv_work_extra(&mut tile, activity) {
                    return None;
                }
                self.work_activity_time_fail_closed(activity, tile.terrain)
            }
            WorkKind::Clean(target) if tile.has_contamination(target) => {
                match target {
                    Contamination::Pollution => tile.pollution = false,
                    Contamination::Fallout => tile.fallout = false,
                    Contamination::Craters => tile.craters = false,
                };
                match self.work_activity_time(activity, tile.terrain) {
                    Ok(time) => time,
                    Err(_) => return None,
                }
            }
            WorkKind::Cultivate | WorkKind::Plant => {
                let (terrain, _) = self.terrain_work(actor, activity)?;
                let time = self.work_activity_time_fail_closed(activity, tile.terrain);
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
                if terrain != Terrain::Ocean {
                    tile.oil_platform = false;
                }
                if matches!(terrain, Terrain::Lake | Terrain::Ocean) {
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

    fn cached_worker_projection(
        &self,
        projections: &mut WorkerProjectionCache,
        actor: &Unit,
        activity: WorkKind,
    ) -> Option<(Tile, i32)> {
        let Some(activity_index) = ACTIVITIES
            .iter()
            .position(|&candidate| candidate == activity)
        else {
            return self.worker_projection(actor, activity);
        };
        let tile = self.map_wrap.index(actor.x, actor.y);
        let slots = projections
            .entry((actor.owner, actor.kind.index(), tile))
            .or_insert_with(|| std::array::from_fn(|_| None));
        let slot = &mut slots[activity_index];
        if let Some(result) = slot {
            self.trace
                .count_operation("worker.projection_cache_hits", 1);
            return result.clone();
        }
        self.trace
            .count_operation("worker.projection_cache_misses", 1);
        let result = self.worker_projection(actor, activity);
        *slot = Some(result.clone());
        result
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

    fn cached_worker_road_rail_projection(
        &self,
        projections: &mut RoadRailProjectionCache,
        actor: &Unit,
        road: Tile,
        road_time: i32,
    ) -> Option<(Tile, i32)> {
        let key = (
            actor.owner,
            actor.kind.index(),
            self.map_wrap.index(actor.x, actor.y),
        );
        if let Some(result) = projections.get(&key) {
            self.trace.count_operation("worker.road_rail_cache_hits", 1);
            return result.clone();
        }
        self.trace
            .count_operation("worker.road_rail_cache_misses", 1);
        let result = self
            .work_action_available_for_unit_with_tile_fail_closed(
                actor,
                WorkKind::Railroad,
                (actor.x, actor.y),
                Some(&road),
            )
            .then(|| {
                let rail_time =
                    self.work_activity_time_fail_closed(WorkKind::Railroad, road.terrain);
                (road, road_time + rail_time)
            });
        projections.insert(key, result.clone());
        result
    }

    pub(super) fn cached_worker_road_extra(
        &self,
        bonuses: &mut Option<RoadBonusCache>,
        position: (i32, i32),
        railroad: bool,
        roads: &WorkerActivityTiles,
        railroads: &WorkerActivityTiles,
    ) -> f32 {
        let index = self.map_wrap.index(position.0, position.1);
        let slot = usize::from(railroad);
        let cache = bonuses.as_mut().expect("road activities prepared");
        if let Some(value) = cache.values[index][slot] {
            self.trace
                .count_operation("worker.road_bonus_cache_hits", 1);
            return value;
        }
        self.trace
            .count_operation("worker.road_bonus_cache_misses", 1);
        let value = self.worker_road_extra_with_activities(position, railroad, roads, railroads);
        cache.values[index][slot] = Some(value);
        value
    }

    #[cfg(test)]
    pub(super) fn worker_road_extra(&self, position: (i32, i32), railroad: bool) -> f32 {
        let (road_activity, railroad_activity) = self.worker_activity_tiles();
        self.worker_road_extra_with_activities(
            position,
            railroad,
            &road_activity,
            &railroad_activity,
        )
    }

    fn worker_activity_tiles(&self) -> (WorkerActivityTiles, WorkerActivityTiles) {
        let mut roads = vec![false; self.tiles.len()];
        let mut railroads = vec![false; self.tiles.len()];
        for unit in &self.units {
            let index = self.map_wrap.index(unit.x, unit.y);
            match unit.work_kind {
                Some(WorkKind::Road) => {
                    roads[index] = true;
                }
                Some(WorkKind::Railroad) => {
                    railroads[index] = true;
                }
                _ => {}
            }
        }
        (roads, railroads)
    }

    fn worker_road_extra_with_activities(
        &self,
        position: (i32, i32),
        railroad: bool,
        road_activity: &WorkerActivityTiles,
        railroad_activity: &WorkerActivityTiles,
    ) -> f32 {
        let (map_x, map_y) = native_to_map(position.0, position.1);
        let mut real_road = [false; ROAD_BONUS_OFFSETS.len()];
        let mut potential_road = [false; ROAD_BONUS_OFFSETS.len()];
        let mut is_slow = [false; ROAD_BONUS_OFFSETS.len()];
        let target_activity = if railroad {
            WorkKind::Railroad
        } else {
            WorkKind::Road
        };
        for (index, &(dx, dy)) in ROAD_BONUS_OFFSETS.iter().enumerate() {
            let (x, y) = map_to_native(map_x + dx, map_y + dy);
            if !(0..self.map_wrap.height()).contains(&y) {
                // Freeciv's bounded-map edge has no tile here. Its road bonus
                // treats such cells as neither roads nor slow destinations.
                continue;
            }
            let position = (x.rem_euclid(self.map_wrap.width()), y);
            let tile = self.tile(position.0, position.1);
            let exact_extra = if railroad { tile.railroad } else { tile.road };
            let dependency_extra = railroad && tile.road;
            let tile_index = self.map_wrap.index(position.0, position.1);
            let dependency_activity = railroad && road_activity[tile_index];
            let activity_in_progress = if railroad {
                railroad_activity[tile_index]
            } else {
                road_activity[tile_index]
            };

            real_road[index] = exact_extra || activity_in_progress;
            // Railroad has Road as a C2C3 extra dependency. A neighboring
            // road is therefore a potential connector even before upgrade.
            potential_road[index] = real_road[index] || dependency_extra || dependency_activity;
            let build_time = self
                .work_activity_time(target_activity, tile.terrain)
                .unwrap_or(0);
            is_slow[index] = build_time == 0 || build_time > 5;
        }

        let bonus = freeciv_square_road_connector_bonus(&real_road, &potential_road, &is_slow);
        if bonus == 0 {
            return 0.0;
        }
        let tile = self.tile(position.0, position.1);
        let rules = super::super::movement_rules::MovementRules::campaign();
        let current = super::super::movement_rules::MovementTile::new(
            tile.terrain,
            tile.road,
            tile.railroad,
            tile.river,
            tile.base,
        );
        let mut improved = current;
        improved.road = true;
        improved.railroad = railroad || tile.railroad;
        let old = rules.step_cost(
            current,
            current,
            super::super::unit_catalog::UnitClass::Land,
            true,
            false,
            false,
        );
        let new = rules.step_cost(
            improved,
            improved,
            super::super::unit_catalog::UnitClass::Land,
            true,
            false,
            false,
        );
        let (Ok(old), Ok(new)) = (old, new) else {
            debug_assert!(false, "could not evaluate C2C3 road movement benefit");
            return 0.0;
        };
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
        state: &mut WorkMap,
        requests: &[WorkerRequest],
    ) -> Option<WorkerPlan> {
        self.evaluate_worker_plan(actor, state, requests, true)
    }

    pub(super) fn evaluate_worker_plan(
        &mut self,
        actor: &Unit,
        state: &mut WorkMap,
        requests: &[WorkerRequest],
        need_path: bool,
    ) -> Option<WorkerPlan> {
        // daisettler.c:settler_run gates worker_evaluate_improvements on
        // UTYF_WORKERS. Units without that flag cannot perform any of these
        // actions; do not build routes and scan every city just to get None.
        if !actor.kind.is_worker() || actor.transport.is_some() {
            return None;
        }
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("worker.plan");
        let allowance = self.unit_move_rate(actor).max(1);
        let occupied = |sim: &Self, position: (i32, i32)| {
            sim.any_unit_at(position.0, position.1, |u| {
                u.id != actor.id && u.owner == actor.owner && u.kind.is_worker()
            })
        };
        let _discovery_profile = profiler.scope("worker.plan.worksite_discovery");
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == actor.owner)
            .cloned()
            .collect();
        let mut worksite_candidates = Vec::new();
        let mut route_targets = std::collections::HashSet::new();
        for (city_index, city) in cities.iter().enumerate() {
            for (radius_index, (_, _, x, y)) in self.map_city_radius(city).enumerate() {
                let position = (x, y);
                let worked = (city.x, city.y) == position || city.worked_tiles.contains(&position);
                if (!worked && !self.can_assign_city_tile(city, x, y))
                    || !self.cached_worker_safe_tile(state, actor.owner, position)
                    || occupied(self, position)
                {
                    continue;
                }
                route_targets.insert(self.map_wrap.index(x, y));
                worksite_candidates.push((city_index, radius_index, position, worked));
            }
        }
        for request in requests {
            if cities.iter().any(|city| city.id == request.city)
                && !occupied(self, request.destination)
            {
                route_targets.insert(
                    self.map_wrap
                        .index(request.destination.0, request.destination.1),
                );
            }
        }
        self.trace.count_operation(
            "worker.worksite_candidates",
            worksite_candidates.len() as u64,
        );
        drop(_discovery_profile);
        let routes = self.worker_routes_to(actor, Some(route_targets));
        let _request_profile = profiler.scope("worker.plan.request_selection");
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
            if self
                .cached_worker_projection(&mut state.projections, &at, request.activity)
                .is_none()
            {
                continue;
            }
            let value = (request.want + 1) * 10 / (arrival + 1);
            if value > request_value && arrival < request_distance {
                request_value = value;
                request_distance = arrival;
                best_request = Some(WorkerPlan {
                    destination: request.destination,
                    activity: request.activity,
                    path: Vec::new(),
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
        if let Some(mut plan) = best_request {
            if need_path {
                self.trace.count_operation("worker.paths_reconstructed", 1);
                plan.path = routes.path(plan.destination)?;
            }
            return Some(plan);
        }
        drop(_request_profile);
        let _activity_profile = profiler.scope("worker.plan.activity_context");
        let (road_activity, railroad_activity) = self.worker_activity_tiles();
        state.prepare_road_bonuses(&road_activity, &railroad_activity);
        let rate = self.activity_rate(actor);
        drop(_activity_profile);
        let _scoring_profile = profiler.scope("worker.plan.candidate_scoring");
        let mut best = None;
        let mut reachable_worksites = 0_u64;
        for (city_index, radius_index, position, worked) in worksite_candidates {
            let city = &cities[city_index];
            let (x, y) = position;
            let cost = routes.costs[self.map_wrap.index(x, y)];
            if cost == i32::MAX {
                continue;
            }
            reachable_worksites += 1;
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
            let tiles = state.infrastructure.entry(city.id).or_default();
            if tiles.len() <= radius_index {
                tiles.resize_with(radius_index + 1, WorkerTileValueCache::default);
            }
            let values = &mut tiles[radius_index];
            let old = {
                let _profile = profiler.scope("worker.plan.old_value");
                self.cached_worker_old_value(values, city, position)
            };
            for (activity_index, &activity) in ACTIVITIES.iter().enumerate() {
                let projection = {
                    let _profile = profiler.scope("worker.plan.projection");
                    self.cached_worker_projection(&mut state.projections, &at, activity)
                };
                let Some((tile, time)) = projection else {
                    continue;
                };
                self.trace
                    .count_operation("worker.candidates_considered", 1);
                let new = {
                    let _profile = profiler.scope("worker.plan.projected_value");
                    self.cached_worker_projected_value(values, city, position, activity_index, tile)
                };
                let _extra_profile = profiler.scope("worker.plan.activity_extra");
                let extra = match activity {
                    WorkKind::Road => self.cached_worker_road_extra(
                        &mut state.road_bonuses,
                        position,
                        false,
                        &road_activity,
                        &railroad_activity,
                    ),
                    WorkKind::Railroad => self.cached_worker_road_extra(
                        &mut state.road_bonuses,
                        position,
                        true,
                        &road_activity,
                        &railroad_activity,
                    ),
                    WorkKind::Clean(target) if target != Contamination::Craters => {
                        self.worker_climate_urgency(target) as f32
                    }
                    WorkKind::Clean(_) => 0.0,
                    _ => 0.0,
                };
                let work = (time * ACTIVITY_FACTOR + rate - 1) / rate;
                let delay = arrival + work + i32::from(remaining == 0);
                drop(_extra_profile);
                let _compare_profile = profiler.scope("worker.plan.compare_action");
                consider_action(
                    &mut best,
                    WorkerPlan {
                        destination: position,
                        activity,
                        path: Vec::new(),
                        arrival,
                        delay,
                        old,
                        new,
                        extra,
                        worked,
                        request: None,
                    },
                );
                drop(_compare_profile);
            }
            // A missing road can be worth building for its ruleset-enabled
            // railroad successor; source adds both cached total values.
            if !self.tile(x, y).road
                && let Some((road, time)) =
                    self.cached_worker_projection(&mut state.projections, &at, WorkKind::Road)
                && let Some((road, time)) = self.cached_worker_road_rail_projection(
                    &mut state.road_rail_projections,
                    &at,
                    road,
                    time,
                )
            {
                let new = self.cached_worker_road_then_rail_value(values, city, position, road);
                let work = (time * ACTIVITY_FACTOR + rate - 1) / rate;
                consider_action(
                    &mut best,
                    WorkerPlan {
                        destination: position,
                        activity: WorkKind::Road,
                        path: Vec::new(),
                        arrival,
                        delay: arrival + work + i32::from(remaining == 0),
                        old,
                        new,
                        extra: self.cached_worker_road_extra(
                            &mut state.road_bonuses,
                            position,
                            true,
                            &road_activity,
                            &railroad_activity,
                        ),
                        worked,
                        request: None,
                    },
                );
            }
        }
        self.trace
            .count_operation("worker.reachable_worksites", reachable_worksites);
        let mut best = best.filter(|plan| {
            amortize(
                (plan.new - plan.old + plan.extra) * WORKER_FACTOR,
                plan.delay,
            ) > 0.0
        })?;
        if need_path {
            self.trace.count_operation("worker.paths_reconstructed", 1);
            best.path = routes.path(best.destination)?;
        }
        Some(best)
    }
}
