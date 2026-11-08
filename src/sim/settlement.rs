//! City founding and settlement-site selection.

use super::pathfinding::reconstruct_path_to_root;
use super::*;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct SettlementReservation {
    pub(super) owner: usize,
    pub(super) destination: (i32, i32),
    pub(super) worker: Option<(i32, i32)>,
}

pub(super) struct CitySite {
    pub(super) value: f64,
    pub(super) worker: Option<(i32, i32)>,
}

type ScoredCitySite = ((i32, i32), Vec<(i32, i32)>, f64);

impl Simulation {
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
        let government = self.ai_goal_government(founder.owner);
        let reservations = self.settlement_citymap(founder.owner, Some(founder.id));
        let start = self.map_wrap.index(founder.x, founder.y);
        let mut costs = vec![i32::MAX; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        costs[start] = 0;
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        let mut best = None;
        let mut best_score = 0.0;
        let mut best_turn = 0;
        let mut traveler = founder.clone();
        let allowance = self.unit_move_rate(founder).max(1);
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost != costs[index] {
                continue;
            }
            let (x, y) = self.map_wrap.position(index);
            let turns = cost / allowance;
            if let Some(site) =
                self.city_site_result(founder.owner, x, y, government, &reservations)
            {
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
            for (nx, ny, step, direction) in self.settler_land_steps(&traveler) {
                let next = self.map_wrap.index(nx, ny);
                let next_cost = cost + step.min(remaining);
                if next_cost < costs[next] {
                    costs[next] = next_cost;
                    previous[next] = Some((index, direction));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }

        let destination = best?;
        let (_, path) = reconstruct_path_to_root(&previous, destination)?;
        Some((self.map_wrap.position(destination), path, best_score))
    }

    pub(super) fn settler_land_steps<'a>(
        &'a self,
        traveler: &'a Unit,
    ) -> impl Iterator<Item = (i32, i32, i32, (i32, i32))> + 'a {
        CIV3_DIRS.into_iter().filter_map(move |direction| {
            let (dx, dy) = direction;
            let (nx, ny) = self.map_wrap.civ3_offset(traveler.x, traveler.y, dx, dy)?;
            if self.treaty_blocks_move(traveler, nx, ny)
                || self.zone_blocks_step(traveler, (traveler.x, traveler.y), (nx, ny))
                || self.units.iter().any(|other| {
                    (other.x, other.y) == (nx, ny) && !self.allied(other.owner, traveler.owner)
                })
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (nx, ny) && !self.allied(city.owner, traveler.owner)
                })
            {
                return None;
            }
            self.movement_cost_for(traveler, nx, ny)
                .map(|step| (nx, ny, step, direction))
        })
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
            || self.units.iter().any(|unit| {
                unit.kind.is_military()
                    && self.at_war(unit.owner, owner)
                    && self.map_wrap.civ3_distance(x, y, unit.x, unit.y) <= 1
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
        let mut city_tiles: Vec<_> = self.map_city_radius_at(x, y).collect();
        // daisettler.c:cityresult_fill uses native city indices and only
        // replaces best_other on a strictly greater want. Equal output
        // workers therefore use city.c:compare_iter_index, not row order.
        city_tiles.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        for (_, _, tx, ty) in city_tiles {
            if (tx, ty) == (x, y) || reserved(tx, ty) < 0 {
                continue;
            }
            let output =
                self.city_tile_yields_under_government(&city, tx, ty, false, None, government);
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
        let worker = best.map(|(_, _, tile)| tile);
        let (best_value, best_output, _) = best.unwrap_or((0, (0, 0, 0), (x, y)));
        if center.0 + best_output.0 <= CITIZEN_FOOD_COST || center.1 + best_output.1 == 0 {
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
