//! City founding and settlement-site selection.

use super::movement::{WorkerRouteMovementProfile, WorkerRouteZoneCache};
use super::pathfinding::reconstruct_path_to_root;
pub(super) mod observations;

use super::*;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct SettlementReservation {
    pub(super) owner: usize,
    pub(super) destination: (i32, i32),
    pub(super) worker: Option<(i32, i32)>,
}

#[derive(Clone)]
pub(super) struct CitySite {
    pub(super) value: f64,
    pub(super) worker: Option<(i32, i32)>,
}

type ScoredCitySite = ((i32, i32), Vec<(i32, i32)>, f64);

impl Simulation {
    pub(super) fn advisor_city_site_result_with_threat_map(
        &self,
        founder: &Unit,
        x: i32,
        y: i32,
        government: Government,
        reservations: &[i32],
        hostile_military: &[bool],
    ) -> Option<CitySite> {
        self.advisor_city_site_result_impl(
            founder,
            x,
            y,
            government,
            reservations,
            Some(hostile_military),
        )
    }

    fn advisor_city_site_result_impl(
        &self,
        founder: &Unit,
        x: i32,
        y: i32,
        government: Government,
        reservations: &[i32],
        hostile_military: Option<&[bool]>,
    ) -> Option<CitySite> {
        let cacheable = self.ai_advisor_cache_enabled.get()
            && matches!(founder.id, 0 | u32::MAX)
            && !self.units.iter().any(|unit| unit.id == founder.id);
        if !cacheable {
            return self.city_site_result_impl(
                founder.owner,
                x,
                y,
                government,
                reservations,
                hostile_military,
            );
        }
        let index = self.map_wrap.index(x, y);
        if let Some(cached) = self
            .ai_spatial_index
            .borrow()
            .virtual_city_sites
            .get(&founder.owner)
            .and_then(|sites| sites[index].as_ref())
        {
            self.trace.count_operation("settlement.site_cache_hits", 1);
            return cached.clone();
        }
        self.trace
            .count_operation("settlement.site_cache_misses", 1);
        let result = self.city_site_result_impl(
            founder.owner,
            x,
            y,
            government,
            reservations,
            hostile_military,
        );
        let mut spatial = self.ai_spatial_index.borrow_mut();
        let sites = spatial
            .virtual_city_sites
            .entry(founder.owner)
            .or_insert_with(|| vec![None; self.tiles.len()]);
        sites[index] = Some(result.clone());
        result
    }

    fn advisor_virtual_city_tile_yield(
        &self,
        city: &City,
        x: i32,
        y: i32,
        government: Government,
    ) -> (i32, i32, i32) {
        if !self.ai_advisor_cache_enabled.get() {
            return self.city_tile_yields_under_government(city, x, y, false, None, government);
        }
        let owner = city.owner;
        let index = self.map_wrap.index(x, y);
        if let Some((cached_government, yields)) = self
            .ai_spatial_index
            .borrow()
            .virtual_city_tile_yields
            .get(&owner)
            && *cached_government == government
            && let Some(Some(output)) = yields.get(index)
        {
            self.trace
                .count_operation("settlement.tile_yield_cache_hits", 1);
            return *output;
        }
        self.trace
            .count_operation("settlement.tile_yield_cache_misses", 1);
        let output = self.city_tile_yields_under_government(city, x, y, false, None, government);
        let mut spatial = self.ai_spatial_index.borrow_mut();
        let entry = spatial
            .virtual_city_tile_yields
            .entry(owner)
            .or_insert_with(|| (government, vec![None; self.tiles.len()]));
        if entry.0 != government {
            *entry = (government, vec![None; self.tiles.len()]);
        }
        entry.1[index] = Some(output);
        output
    }

    pub(super) fn settlement_citymap(&self, owner: usize, except: Option<u32>) -> Vec<i32> {
        let mut map = vec![0; self.tiles.len()];
        for city in &self.cities {
            for (_, _, x, y) in self.map_city_radius_at(city.x, city.y) {
                let index = self.map_wrap.index(x, y);
                if map[index] >= 0 {
                    map[index] += 1;
                }
            }
            for &(x, y) in &city.worked_tiles {
                map[self.map_wrap.index(x, y)] = -1;
            }
            map[self.map_wrap.index(city.x, city.y)] = -1;
        }
        for (&id, reservation) in &self.ai_settlement_targets {
            if reservation.owner != owner
                || Some(id) == except
                || !self
                    .units
                    .iter()
                    .any(|unit| unit.id == id && unit.owner == owner && unit.kind.stats().can_found)
            {
                continue;
            }
            for (_, _, x, y) in
                self.map_city_radius_at(reservation.destination.0, reservation.destination.1)
            {
                let index = self.map_wrap.index(x, y);
                if map[index] >= 0 {
                    map[index] += 1;
                }
            }
            map[self
                .map_wrap
                .index(reservation.destination.0, reservation.destination.1)] = -1;
            if let Some((x, y)) = reservation.worker {
                map[self.map_wrap.index(x, y)] = -1;
            }
        }
        map
    }

    pub(super) fn prune_settlement_reservations(&mut self, owner: usize) {
        let invalid: Vec<_> = self
            .ai_settlement_targets
            .iter()
            .filter_map(|(&id, reservation)| {
                let valid_unit = self.units.iter().any(|unit| {
                    unit.id == id && unit.owner == reservation.owner && unit.kind.stats().can_found
                });
                (!valid_unit).then_some(id)
            })
            .collect();
        for id in invalid {
            self.ai_settlement_targets.remove(&id);
            self.unit_goto_tiles.remove(&id);
            self.ai_worker_tasks.remove(&id);
        }
        // citymap_turn_init rebuilds future centers and crowding, but the
        // extra first-worker reservation exists only in the assignment turn.
        for reservation in self
            .ai_settlement_targets
            .values_mut()
            .filter(|entry| entry.owner == owner)
        {
            reservation.worker = None;
        }
    }

    pub(super) fn reserve_settlement_site(&mut self, founder: &Unit, destination: (i32, i32)) {
        let government = self.ai_goal_government(founder.owner);
        let map = self.settlement_citymap(founder.owner, Some(founder.id));
        if let Some(site) = self.city_site_result(
            founder.owner,
            destination.0,
            destination.1,
            government,
            &map,
        ) {
            use std::collections::HashSet;
            let nearest_city = self
                .cities
                .iter()
                .map(|city| {
                    (
                        self.map_distance(destination.0, destination.1, city.x, city.y),
                        city.id,
                    )
                })
                .min();
            let candidate_radius: HashSet<_> = self
                .map_city_radius_at(destination.0, destination.1)
                .map(|(_, _, x, y)| self.map_wrap.index(x, y))
                .collect();
            let city_radius_overlap_tiles = self
                .cities
                .iter()
                .map(|city| {
                    self.map_city_radius_at(city.x, city.y)
                        .filter(|(_, _, x, y)| {
                            candidate_radius.contains(&self.map_wrap.index(*x, *y))
                        })
                        .count()
                })
                .sum();
            let travel_distance =
                self.map_distance(founder.x, founder.y, destination.0, destination.1);
            self.trace.emit(
                self.turn,
                || {
                    vec![
                        trace::Entity::Unit(founder.id),
                        trace::Entity::Civilization(founder.owner),
                    ]
                },
                || observations::SiteSelected {
                    destination,
                    score: site.value,
                    travel_distance,
                    nearest_city_id: nearest_city.map(|(_, id)| id),
                    nearest_city_distance: nearest_city.map(|(distance, _)| distance),
                    city_radius_overlap_tiles,
                    first_worker_tile: site.worker,
                },
            );
            self.ai_worker_tasks.remove(&founder.id);
            self.unit_goto_tiles.insert(founder.id, destination);
            self.ai_settlement_targets.insert(
                founder.id,
                SettlementReservation {
                    owner: founder.owner,
                    destination,
                    worker: site.worker,
                },
            );
        }
    }

    pub(super) fn run_ai_founder(&mut self, founder: &Unit) {
        let improvement = if founder.kind.stats().can_work {
            self.worker_improvement_want(founder)
        } else {
            0.0
        };
        if !self.run_ai_founder_with_improvement(founder, improvement) {
            self.run_auto_worker(founder.id);
        }
    }

    pub(super) fn run_ai_founder_with_improvement(
        &mut self,
        founder: &Unit,
        improvement: f32,
    ) -> bool {
        if let Some(reservation) = self.ai_settlement_targets.get(&founder.id).copied() {
            if reservation.owner != founder.owner
                || !self.can_found_at(reservation.destination.0, reservation.destination.1)
            {
                self.ai_settlement_targets.remove(&founder.id);
                self.unit_goto_tiles.remove(&founder.id);
                self.ai_worker_tasks.remove(&founder.id);
                self.release_ai_ferry(founder.id);
                return true; // daisettler.c BUILD_CITY cancels without same-turn recursion.
            }
            let path = self.path_for_unit(
                founder,
                reservation.destination.0,
                reservation.destination.1,
            );
            if founder.transport.is_none()
                && (!path.is_empty() || (founder.x, founder.y) == reservation.destination)
            {
                self.release_ai_ferry(founder.id);
            }
            // Temporary route obstruction does not abandon an assigned city
            // mission; Freeciv waits for its ferry/route to become available.
            self.ai_guard_for_destination(
                founder.id,
                reservation.destination.0,
                reservation.destination.1,
            );
            self.follow_civ3_path(founder.id, path);
            if self.units.iter().any(|unit| {
                unit.id == founder.id
                    && unit.moves_left > 0
                    && (unit.x, unit.y) == reservation.destination
            }) {
                self.found_city(founder.id);
                self.ai_settlement_targets.remove(&founder.id);
            }
            return true;
        }
        if founder.work_left > 0 {
            return false;
        }
        let city = self.best_city_site(founder);
        // daisettler.c:dai_auto_settler_run chooses a city only when its
        // adv_want is STRICTLY greater; equal wants keep terrain work.
        if let Some((destination, path, _)) = city.filter(|(_, _, want)| *want as f32 > improvement)
        {
            if founder.transport.is_none() {
                self.release_ai_ferry(founder.id);
            }
            self.reserve_settlement_site(founder, destination);
            self.ai_guard_for_destination(founder.id, destination.0, destination.1);
            self.follow_civ3_path(founder.id, path);
            if self.units.iter().any(|unit| {
                unit.id == founder.id && unit.moves_left > 0 && (unit.x, unit.y) == destination
            }) {
                self.found_city(founder.id);
                self.ai_settlement_targets.remove(&founder.id);
            }
            true
        } else {
            if improvement > 0.0 {
                self.release_ai_ferry(founder.id);
            }
            false
        }
    }

    pub(super) fn best_city_site(&self, founder: &Unit) -> Option<ScoredCitySite> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("settlement.best_city_site");
        self.trace.count_operation("settlement.site_searches", 1);
        let government = self.ai_goal_government(founder.owner);
        let reservations = self.settlement_citymap(founder.owner, Some(founder.id));
        let start = self.map_wrap.index(founder.x, founder.y);
        let (_, _, blocked_land, hostile_military) =
            self.settlement_route_blocked_tiles(founder.owner);
        let mut route_zone_cache = WorkerRouteZoneCache::new(self);
        let movement_profile = self.worker_route_movement_profile(founder.kind);
        let mut costs = vec![i32::MAX; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        costs[start] = 0;
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        let mut best = None;
        let mut best_score = 0.0;
        let mut best_turn = 0;
        let mut traveler = founder.clone();
        let allowance = self.unit_move_rate(founder).max(1);
        let mut expanded = 0_u64;
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost != costs[index] {
                continue;
            }
            expanded += 1;
            let (x, y) = self.map_wrap.position(index);
            let turns = cost / allowance;
            if let Some(site) = self.advisor_city_site_result_with_threat_map(
                founder,
                x,
                y,
                government,
                &reservations,
                &hostile_military,
            ) {
                let score = site.value * (23.0_f64 / 24.0).powi(3 * turns)
                    - f64::from(founder.kind.stats().cost);
                if score > best_score {
                    best = Some(index);
                    best_score = score;
                    best_turn = turns;
                }
                if best_score > 250.0 && turns > allowance && best_turn < turns {
                    break;
                }
            }
            (traveler.x, traveler.y) = (x, y);
            let remaining = if cost < founder.moves_left {
                founder.moves_left - cost
            } else {
                allowance - (cost - founder.moves_left.max(0)) % allowance
            };
            traveler.moves_left = remaining;
            for (nx, ny, step, direction) in self.settler_land_steps(
                &traveler,
                &blocked_land,
                &mut route_zone_cache,
                &movement_profile,
            ) {
                let next = self.map_wrap.index(nx, ny);
                let next_cost = cost + step.min(remaining);
                if next_cost < costs[next] {
                    costs[next] = next_cost;
                    previous[next] = Some((index, direction));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        self.trace
            .count_operation("settlement.site_search_expansions", expanded);
        let (zoc_cache_hits, allied_cache_hits) = route_zone_cache.cache_hits();
        self.trace
            .count_operation("settlement.site_route_zoc_cache_hits", zoc_cache_hits);
        self.trace
            .count_operation("settlement.site_route_allied_cache_hits", allied_cache_hits);

        let destination = best?;
        let (_, path) = reconstruct_path_to_root(&previous, destination)?;
        Some((self.map_wrap.position(destination), path, best_score))
    }

    pub(super) fn settler_land_steps<'a>(
        &'a self,
        traveler: &'a Unit,
        blocked_land: &'a [bool],
        route_zone_cache: &'a mut WorkerRouteZoneCache,
        movement_profile: &'a WorkerRouteMovementProfile,
    ) -> impl Iterator<Item = (i32, i32, i32, (i32, i32))> + 'a {
        // All callers search for a founder. The civilian treaty check only
        // rejects occupied tiles, which blocked_land already covers; its
        // non-civilian peaceful-border rule does not apply here.
        debug_assert!(traveler.kind.is_civilian());
        CIV3_DIRS.into_iter().filter_map(move |direction| {
            let (dx, dy) = direction;
            let (nx, ny) = self.map_wrap.civ3_offset(traveler.x, traveler.y, dx, dy)?;
            if blocked_land[self.map_wrap.index(nx, ny)]
                || self.zone_blocks_worker_route_step_with_profile(
                    traveler,
                    (traveler.x, traveler.y),
                    (nx, ny),
                    route_zone_cache,
                    movement_profile,
                )
            {
                return None;
            }
            self.movement_cost_for(traveler, nx, ny)
                .map(|step| (nx, ny, step, direction))
        })
    }

    pub(super) fn settlement_route_blocked_tiles(
        &self,
        owner: usize,
    ) -> (Vec<bool>, Vec<bool>, Vec<bool>, Vec<bool>) {
        let mut blocked_units = vec![false; self.tiles.len()];
        let mut hostile_military = vec![false; self.tiles.len()];
        for unit in &self.units {
            if !self.allied(unit.owner, owner) {
                blocked_units[self.map_wrap.index(unit.x, unit.y)] = true;
            }
            if unit.kind.is_military() && self.at_war(unit.owner, owner) {
                hostile_military[self.map_wrap.index(unit.x, unit.y)] = true;
            }
        }
        let mut blocked_cities = vec![false; self.tiles.len()];
        for city in &self.cities {
            if !self.allied(city.owner, owner) {
                blocked_cities[self.map_wrap.index(city.x, city.y)] = true;
            }
        }
        let mut blocked_land = blocked_units.clone();
        for (land, &city) in blocked_land.iter_mut().zip(&blocked_cities) {
            *land |= city;
        }
        (
            blocked_units,
            blocked_cities,
            blocked_land,
            hostile_military,
        )
    }

    pub(super) fn city_site_value(&self, owner: usize, x: i32, y: i32) -> Option<f64> {
        let government = self.ai_goal_government(owner);
        let reservations = self.settlement_citymap(owner, None);
        self.city_site_result(owner, x, y, government, &reservations)
            .map(|site| site.value)
    }

    pub(super) fn city_site_result(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        government: Government,
        reservations: &[i32],
    ) -> Option<CitySite> {
        self.city_site_result_impl(owner, x, y, government, reservations, None)
    }

    fn city_site_result_impl(
        &self,
        owner: usize,
        x: i32,
        y: i32,
        government: Government,
        reservations: &[i32],
        hostile_military: Option<&[bool]>,
    ) -> Option<CitySite> {
        self.trace
            .count_operation("settlement.site_candidate_evaluations", 1);
        let tile = self.tile(x, y);
        if !self.can_found_at(x, y)
            || self
                .map_city_radius_at(x, y)
                .any(|(_, _, tx, ty)| reservations[self.map_wrap.index(tx, ty)] < 0)
            || tile.owner.is_some_and(|other| {
                other != owner
                    && matches!(
                        self.pact(owner, other),
                        Pact::Armistice | Pact::Peace | Pact::Alliance
                    )
            })
            || std::iter::once((x, y))
                .chain(self.map_wrap.civ3_neighbors(x, y))
                .any(|(tx, ty)| {
                    hostile_military.map_or_else(
                        || {
                            self.any_unit_at(tx, ty, |unit| {
                                unit.kind.is_military() && self.at_war(unit.owner, owner)
                            })
                        },
                        |tiles| tiles[self.map_wrap.index(tx, ty)],
                    )
                })
        {
            return None;
        }
        let city = City::new(u32::MAX, owner, x, y, String::new(), self.turn);
        let science = if self.players[owner].researching == Tech::FUTURE {
            1.0
        } else {
            21.0
        };
        let weighted = |(food, shields, trade): (i32, i32, i32), reservation: i32| {
            let mut value =
                (f64::from(food * 30 + shields * 17) + f64::from(trade) * science) as i32;
            if food >= 2 {
                value *= 2;
            }
            if value > 0 {
                value -= (reservation * 15).min(value - 1);
            }
            value
        };
        let reserved = |tx, ty| reservations[self.map_wrap.index(tx, ty)];
        let center = self.city_tile_yields_under_government(&city, x, y, true, None, government);
        let mut best = None;
        let mut remaining = 0;
        let mut radius_tiles_scored = 0_u64;
        // The Civ III city radius has 21 entries. Keep this short-lived list
        // on the stack; site searches score hundreds of candidate cities and
        // allocating a Vec for every candidate adds avoidable churn.
        let mut city_tiles = [(0, 0, 0, 0); 21];
        let mut city_tile_count = 0;
        for tile in self.map_city_radius_at(x, y) {
            city_tiles[city_tile_count] = tile;
            city_tile_count += 1;
        }
        debug_assert!(city_tile_count <= city_tiles.len());
        // daisettler.c:cityresult_fill uses native city indices and only
        // replaces best_other on a strictly greater want. Equal output
        // workers therefore use city.c:compare_iter_index, not row order.
        city_tiles[..city_tile_count].sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        let _radius_profile = self
            .trace
            .profiler()
            .scope("settlement.site_scoring.radius");
        for &(_, _, tx, ty) in &city_tiles[..city_tile_count] {
            if (tx, ty) == (x, y) || reserved(tx, ty) < 0 {
                continue;
            }
            let output = self.advisor_virtual_city_tile_yield(&city, tx, ty, government);
            radius_tiles_scored += 1;
            let value = weighted(output, reserved(tx, ty));
            if best.is_none_or(|(score, _, _)| value > score) {
                if let Some((score, _, _)) = best {
                    remaining += score.max(0) / 8;
                }
                best = Some((value, output, (tx, ty)));
            } else {
                remaining += value.max(0) / 8;
            }
        }
        self.trace
            .count_operation("settlement.site_radius_tiles_scored", radius_tiles_scored);
        let worker = best.map(|(_, _, tile)| tile);
        let (best_value, best_output, _) = best.unwrap_or((0, (0, 0, 0), (x, y)));
        if center.0 + best_output.0 <= Self::citizen_food_cost() || center.1 + best_output.1 == 0 {
            return None;
        }
        let gross_shields = center.1 + best_output.1;
        let gross_trade = center.2 + best_output.2;
        let (net_shields, net_trade) =
            self.government_output_waste_under(&city, gross_shields, gross_trade, None, government);
        let total = (weighted(center, reserved(x, y)) + best_value
            - 17 * (gross_shields - net_shields)
            - (science * f64::from(gross_trade - net_trade)) as i32)
            .max(0);
        let mut total = total as f32;
        // daisettler.c preserves integer division order, including its float
        // defense tie-breaker and integer per-tile growth rounding.
        let mut defense = 10 + tile.terrain.defense_bonus() / 10;
        defense += defense * tile.base.map_or(0, |base| base.defense_bonus()) / 100;
        total += 100.0 / (total + 1.0) * (100 / defense * 20) as f32;
        if self
            .map_wrap
            .civ3_neighbors(x, y)
            .any(|(nx, ny)| self.tile(nx, ny).terrain.is_water())
        {
            total += total * 20.0 / 100.0;
        }
        Some(CitySite {
            value: f64::from(total + remaining as f32),
            worker,
        })
    }
}

#[cfg(test)]
#[path = "settlement/tests.rs"]
mod tests;
