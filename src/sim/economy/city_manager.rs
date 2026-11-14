//! Automated city worker assignment and production management.

use super::ruleset::{CityEconomySettings, GoldUpkeepStyle};
use super::*;

// common/aicore/cm.c:estimate_fitness and compare_tile_type_by_fitness.
pub(super) fn cm_estimated_fitness(
    raw: [i32; 6],
    bonuses: [i32; 6],
    rates: [i32; 3],
    factors: [i32; 6],
) -> f64 {
    let mut outputs = raw.map(f64::from);
    let trade = (outputs[2] * f64::from(bonuses[2]) / 100.0) as i32;
    for (rate, index) in rates.into_iter().zip([5, 3, 4]) {
        outputs[index] += f64::from(rate * trade) / 100.0;
    }
    for index in 0..6 {
        outputs[index] *= f64::from(bonuses[index]) / 100.0;
    }
    (0..6)
        .map(|index| outputs[index] * f64::from(factors[index]))
        .sum::<f64>()
        + outputs[4]
}

pub(super) fn cm_compare_types(
    a: (f64, usize, [i32; 6]),
    b: (f64, usize, [i32; 6]),
) -> std::cmp::Ordering {
    let difference = b.0 - a.0;
    if difference > 0.5 {
        std::cmp::Ordering::Greater
    } else if difference < -0.5 {
        std::cmp::Ordering::Less
    } else {
        a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2))
    }
}

pub(super) const CM_INFINITY: i32 = 1_000_000_000;
pub(super) const CM_MAX_LOOP: usize = 27_500;

pub(super) struct CmLattice {
    pub(super) tiles: Vec<(i32, i32)>,
    pub(super) groups: Vec<Vec<usize>>,
    pub(super) production: Vec<[i32; 6]>,
    pub(super) capacity: Vec<i32>,
    pub(super) better: Vec<Vec<usize>>,
    pub(super) orders: [Vec<usize>; 2],
    pub(super) by_stat: [Vec<usize>; 6],
}

#[derive(Clone)]
pub(super) struct CmPartial {
    pub(super) counts: Vec<i32>,
    pub(super) production: [i32; 6],
    pub(super) idle: i32,
}

impl CmPartial {
    fn add(&mut self, lattice: &CmLattice, kind: usize, count: i32) {
        self.idle -= count;
        self.counts[kind] += count;
        for output in 0..6 {
            self.production[output] += count * lattice.production[kind][output];
        }
    }

    fn prerequisites_filled(&self, lattice: &CmLattice, kind: usize) -> bool {
        lattice.better[kind]
            .iter()
            .all(|&other| self.counts[other] == lattice.capacity[other])
    }
}

pub(super) struct CmQueryResult {
    pub(super) city: Option<City>,
    #[cfg(test)]
    pub(super) iterations: usize,
    #[cfg(test)]
    pub(super) aborted: bool,
    #[cfg(test)]
    pub(super) disorder_prunes: usize,
}

impl Simulation {
    pub(super) fn city_manager_surplus(&self, city: &City, forecast: &CityForecast) -> [i32; 6] {
        [
            forecast.food_surplus(city.size, forecast.support.food),
            if forecast.disorder {
                (forecast.shields - forecast.support.shields).min(0)
            } else {
                forecast.shields - forecast.support.shields
            },
            forecast.trade,
            forecast.gold_surplus(forecast.support.gold),
            forecast.luxury,
            forecast.science,
        ]
    }

    pub(in crate::sim) fn city_manager_result(
        &self,
        city: &City,
        occupied: &std::collections::HashSet<(i32, i32)>,
    ) -> City {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("city_manager.arrange");
        let citizens = city.size.max(0) as usize;
        let mut city_tiles: Vec<_> = self.map_city_radius(city).collect();
        // common/city.c:compare_iter_index defines reproducible city indices.
        city_tiles.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        let mut tiles: Vec<_> = city_tiles
            .into_iter()
            .map(|(_, _, x, y)| (x, y))
            .filter(|tile| {
                !occupied.contains(tile) && self.tile_can_support_worker(city, tile.0, tile.1)
            })
            .map(|tile| {
                (
                    tile,
                    self.city_tile_yields(city, tile.0, tile.1, false, None),
                )
            })
            .collect();
        tiles.sort_by_key(|&(_, (food, shields, trade))| (-food, -shields, -trade));
        // cm.c's lattice permits a weaker tile type only after every
        // strictly better type has been filled. Group equal types first;
        // identical tiles retain city-map traversal order within their type.
        let mut groups: Vec<(Vec<usize>, (i32, i32, i32))> = Vec::new();
        for (index, &(_, output)) in tiles.iter().enumerate() {
            if let Some(group) = groups.iter_mut().find(|group| group.1 == output) {
                group.0.push(index);
            } else {
                groups.push((vec![index], output));
            }
        }
        let full = city.food == Self::growth_box(city.size);
        let food_factor = if city.size == 1 {
            20
        } else if full {
            0
        } else {
            10
        };
        let mut production: Vec<[i32; 6]> = groups
            .iter()
            .map(|group| [group.1.0, group.1.1, group.1.2, 0, 0, 0])
            .collect();
        for (specialist, output_type, output_index) in [
            ("Scientist", "Science", 5),
            ("Taxman", "Gold", 3),
            ("Elvis", "Luxury", 4),
        ] {
            let mut output = [0; 6];
            output[output_index] = self.city_rule_effect(
                city,
                "Specialist_Output",
                None,
                Some(output_type),
                Some(specialist),
                false,
                self.players[city.owner].government,
                None,
            );
            production.push(output);
        }
        let depth: Vec<usize> = production
            .iter()
            .enumerate()
            .map(|(index, output)| {
                production
                    .iter()
                    .enumerate()
                    .filter(|&(other, better)| {
                        other != index
                            && better.iter().zip(output).all(|(a, b)| a >= b)
                            && (better != output || other >= groups.len() && index < groups.len())
                    })
                    .map(|(other, _)| {
                        groups
                            .get(other)
                            .map_or(citizens + 1, |group| group.0.len())
                    })
                    .sum()
            })
            .collect();
        let government = self.players[city.owner].government;
        let bonuses = [
            self.city_output_bonus_percent(city, "Food", government, None),
            self.city_output_bonus_percent(city, "Shield", government, None),
            self.city_output_bonus_percent(city, "Trade", government, None),
            self.city_output_bonus_percent(city, "Gold", government, None),
            self.city_output_bonus_percent(city, "Luxury", government, None),
            self.city_output_bonus_percent(city, "Science", government, None),
        ];
        let rates = self.players[city.owner].rates;
        let orders: [Vec<usize>; 2] = std::array::from_fn(|emergency| {
            let factors = if emergency == 0 {
                [food_factor, 5, 0, 2, 0, 2]
            } else {
                [1; 6]
            };
            let estimates: Vec<f64> = production
                .iter()
                .map(|&raw| cm_estimated_fitness(raw, bonuses, rates, factors))
                .collect();
            let mut order: Vec<_> = (0..production.len()).collect();
            order.sort_by(|&a, &b| {
                cm_compare_types(
                    (estimates[a], depth[a], production[a]),
                    (estimates[b], depth[b], production[b]),
                )
            });
            order
        });
        let capacity: Vec<i32> = (0..production.len())
            .map(|index| {
                groups
                    .get(index)
                    .map_or(CM_INFINITY, |group| group.0.len() as i32)
            })
            .collect();
        let better: Vec<Vec<usize>> = production
            .iter()
            .enumerate()
            .map(|(index, output)| {
                production
                    .iter()
                    .enumerate()
                    .filter(|&(other, candidate)| {
                        other != index
                            && candidate.iter().zip(output).all(|(a, b)| a >= b)
                            && (candidate != output
                                || other >= groups.len() && index < groups.len())
                    })
                    .map(|(other, _)| other)
                    .collect()
            })
            .collect();
        let orders = orders.map(|order| {
            order
                .into_iter()
                .filter(|&index| depth[index] < citizens)
                .collect()
        });
        let by_stat = std::array::from_fn(|output| {
            let trade_bonus = match output {
                5 => rates[0],
                3 => rates[1],
                4 => rates[2],
                _ => 0,
            } as f64
                * f64::from(bonuses[2])
                / 100.0;
            let mut order: Vec<_> = (0..production.len())
                .filter(|&index| depth[index] < citizens)
                .collect();
            order.sort_by(|&a, &b| {
                let value_a =
                    f64::from(production[a][output]) + trade_bonus * f64::from(production[a][2]);
                let value_b =
                    f64::from(production[b][output]) + trade_bonus * f64::from(production[b][2]);
                if value_a != value_b {
                    ((value_b - value_a) as i32).cmp(&0)
                } else {
                    depth[a]
                        .cmp(&depth[b])
                        .then_with(|| production[b].cmp(&production[a]))
                }
            });
            order
        });
        let lattice = CmLattice {
            tiles: tiles.into_iter().map(|entry| entry.0).collect(),
            groups: groups.into_iter().map(|group| group.0).collect(),
            production,
            capacity,
            better,
            orders,
            by_stat,
        };
        let original = self.city_manager_surplus(city, &self.city_forecast(city));
        for stage in 0..4 {
            let minima = match stage {
                0 => [i32::from(!full), 1, 0, -CM_INFINITY, 0, 0],
                1 => [0, 0, 0, -CM_INFINITY, 0, 0],
                2 => [
                    original[0].min(0),
                    original[1].min(0),
                    original[2].min(0),
                    -CM_INFINITY,
                    original[4].min(0),
                    original[5].min(0),
                ],
                _ => [-CM_INFINITY; 6],
            };
            let result = self.city_manager_query(
                city,
                &lattice,
                food_factor,
                minima,
                stage == 3,
                CM_MAX_LOOP,
            );
            // cityturn.c deliberately checks validity, not the abort flag:
            // a sufficient incumbent from a truncated search is applied.
            if let Some(city) = result.city {
                return city;
            }
        }
        city.clone()
    }

    fn city_manager_apply(&self, city: &City, lattice: &CmLattice, partial: &CmPartial) -> City {
        let mut result = city.clone();
        result.worked_tiles = lattice
            .groups
            .iter()
            .enumerate()
            .flat_map(|(kind, group)| {
                group
                    .iter()
                    .take(partial.counts[kind] as usize)
                    .map(|&tile| lattice.tiles[tile])
            })
            .collect();
        result.specialists =
            std::array::from_fn(|index| partial.counts[lattice.groups.len() + index]);
        result
    }

    pub(in crate::sim) fn city_manager_raw_production(
        &self,
        city: &City,
        raw: [i32; 6],
    ) -> [i32; 6] {
        // cm.c:compute_max_stats_heuristic combines maxima for six different
        // completions into a synthetic citizen_base, then set_city_production.
        let center = self.city_tile_yields(city, city.x, city.y, true, None);
        let (food, shield, trade) = self.c2c3_finalize_city_outputs(
            city,
            (raw[0] + center.0, raw[1] + center.1, raw[2] + center.2),
            self.players[city.owner].government,
            None,
        );
        let [science, gold, luxury] = self.split_city_trade_from_citizens(
            city,
            trade,
            self.players[city.owner].rates,
            [raw[5], raw[3], raw[4]],
        );
        [
            food,
            shield,
            trade,
            gold.max(0),
            luxury.max(0),
            science.max(0),
        ]
    }

    pub(super) fn city_manager_query(
        &self,
        city: &City,
        lattice: &CmLattice,
        food_factor: i32,
        minima: [i32; 6],
        emergency: bool,
        limit: usize,
    ) -> CmQueryResult {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("city_manager.query");
        let order = &lattice.orders[usize::from(emergency)];
        let mut rank = vec![usize::MAX; lattice.production.len()];
        for (position, &kind) in order.iter().enumerate() {
            rank[kind] = position;
        }
        let factors = if emergency {
            [1; 6]
        } else {
            [food_factor, 5, 0, 2, 0, 2]
        };
        let initial = self.city_forecast(city);
        let settings = CityEconomySettings::campaign();
        let unit_gold = if settings.gold_upkeep_style == GoldUpkeepStyle::City {
            initial.support.gold
        } else {
            0
        };
        let building_gold = if settings.gold_upkeep_style == GoldUpkeepStyle::Nation {
            0
        } else {
            initial.building_upkeep
        };
        let usage = [
            city.size * Self::citizen_food_cost() + initial.support.food,
            initial.support.shields,
            0,
            building_gold + unit_gold,
            0,
            0,
        ];
        let min_production: [i32; 6] = std::array::from_fn(|index| usage[index] + minima[index]);
        let mut current = CmPartial {
            counts: vec![0; lattice.production.len()],
            production: if emergency { [-CM_INFINITY; 6] } else { [0; 6] },
            idle: city.size.max(0),
        };
        let mut best = current.clone();
        let mut best_fitness = (false, -i64::from(CM_INFINITY));
        let mut min_luxury = -CM_INFINITY;
        let mut stack: Vec<usize> = Vec::new();
        let mut iterations = 0;
        #[cfg(test)]
        let disorder_prunes = std::cell::Cell::new(0);
        let max_content = self
            .city_base_content(city, self.players[city.owner].government)
            .max(0);
        let happy_cost = CityEconomySettings::campaign().happy_cost.max(1);
        loop {
            if current.idle == 0 {
                let candidate = self.city_manager_apply(city, lattice, &current);
                let forecast = self.city_forecast(&candidate);
                let surplus = self.city_manager_surplus(&candidate, &forecast);
                if forecast.disorder {
                    min_luxury = surplus[4]
                        + happy_cost
                            * (candidate.specialists.iter().sum::<i32>() - max_content).max(0)
                        + 1;
                }
                let sufficient = (emergency || !forecast.disorder)
                    && (0..6).all(|index| surplus[index] >= minima[index]);
                let fitness = (
                    sufficient,
                    (0..6)
                        .map(|index| i64::from(surplus[index]) * i64::from(factors[index]))
                        .sum::<i64>()
                        + i64::from(emergency && forecast.happy),
                );
                if fitness > best_fitness {
                    best = current.clone();
                    best_fitness = fitness;
                }
            }
            let promising = |partial: &CmPartial, kind: usize| {
                let mut raw = [0; 6];
                if partial.idle == 1 {
                    raw = std::array::from_fn(|output| {
                        partial.production[output] + lattice.production[kind][output]
                    });
                } else {
                    let mut completion = partial.clone();
                    for (output, raw_output) in raw.iter_mut().enumerate() {
                        completion.clone_from(partial);
                        completion.add(lattice, kind, 1);
                        let last = completion
                            .counts
                            .iter()
                            .enumerate()
                            .filter(|&(_, count)| *count > 0)
                            .map(|(kind, _)| rank[kind])
                            .max()
                            .unwrap_or(0);
                        for &other in &lattice.by_stat[output] {
                            if rank[other] < last
                                || !completion.prerequisites_filled(lattice, other)
                            {
                                continue;
                            }
                            let take = (lattice.capacity[other] - completion.counts[other])
                                .min(completion.idle);
                            completion.add(lattice, other, take);
                            if completion.idle == 0 {
                                break;
                            }
                        }
                        *raw_output = completion.production[output];
                    }
                }
                let production = self.city_manager_raw_production(city, raw);
                let meets_min = (0..6).all(|output| production[output] >= min_production[output]);
                let beats_best = (0..6).any(|output| {
                    factors[output] > 0 && production[output] > best.production[output]
                });
                let specialists = partial.counts[lattice.groups.len()..].iter().sum::<i32>();
                let luxury =
                    production[4] + happy_cost * (specialists + partial.idle - max_content).max(0);
                #[cfg(test)]
                if luxury < min_luxury {
                    disorder_prunes.set(disorder_prunes.get() + 1);
                }
                meets_min && beats_best && luxury >= min_luxury
            };
            let next = |partial: &CmPartial, start: usize| {
                (start..order.len()).find(|&position| {
                    let kind = order[position];
                    partial.counts[kind] < lattice.capacity[kind]
                        && partial.prerequisites_filled(lattice, kind)
                        && promising(partial, kind)
                })
            };
            if current.idle > 0
                && let Some(position) = next(&current, stack.last().copied().unwrap_or(0))
            {
                current.add(lattice, order[position], 1);
                stack.push(position);
            } else {
                let mut sibling = false;
                while let Some(position) = stack.pop() {
                    current.add(lattice, order[position], -1);
                    if let Some(replacement) = next(&current, position + 1) {
                        current.add(lattice, order[replacement], 1);
                        stack.push(replacement);
                        sibling = true;
                        break;
                    }
                }
                if !sibling {
                    break;
                }
            }
            iterations += 1;
            if iterations == limit + 1 {
                break;
            }
        }
        self.trace
            .count_operation("city_manager.search_iterations", iterations as u64);
        CmQueryResult {
            city: if best.idle == 0 && best_fitness.0 {
                Some(self.city_manager_apply(city, lattice, &best))
            } else {
                None
            },
            #[cfg(test)]
            iterations,
            #[cfg(test)]
            aborted: iterations > limit,
            #[cfg(test)]
            disorder_prunes: disorder_prunes.get(),
        }
    }
}

impl City {
    pub(crate) fn tile_workers(&self) -> i32 {
        (self.size - self.specialists.iter().sum::<i32>()).max(0)
    }

    pub(in crate::sim) fn remove_specialists(&mut self, mut excess: i32) {
        // The ruleset removes entertainers first, followed by scientists and taxmen.
        for index in [2, 0, 1] {
            let removed = excess.min(self.specialists[index]);
            self.specialists[index] -= removed;
            excess -= removed;
        }
    }

    pub(in crate::sim) fn remove_citizen_assignments(&mut self, loss: i32, wrap: MapWrap) {
        let before = self.specialists.iter().sum::<i32>();
        self.remove_specialists(loss);
        let remaining = loss - (before - self.specialists.iter().sum::<i32>());
        if remaining <= 0 {
            return;
        }
        // cityturn.c:city_reduce_workers removes the first worked fields in
        // native city-index order, before the whole city is rearranged.
        let mut order: Vec<_> = wrap.city_radius(self.x, self.y).collect();
        order.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        let removed: std::collections::HashSet<_> = order
            .into_iter()
            .map(|(_, _, x, y)| (x, y))
            .filter(|tile| self.worked_tiles.contains(tile))
            .take(remaining as usize)
            .collect();
        self.worked_tiles.retain(|tile| !removed.contains(tile));
    }
}

impl Simulation {
    pub(in crate::sim) fn arrange_city_event_workers(&mut self, city_id: u32) {
        let mut occupied = std::collections::HashSet::new();
        let mut displaced = Vec::new();
        // Native city_map_update_all removes invalid workers, staging each
        // displaced citizen as the default specialist until queued CM runs.
        // Do not greedily fill those vacancies before the event city chooses.
        for index in 0..self.cities.len() {
            let city = &self.cities[index];
            let workers = city.tile_workers().max(0) as usize;
            let tiles: Vec<_> = city
                .worked_tiles
                .iter()
                .copied()
                .take(workers)
                .filter(|&(x, y)| self.tile_can_support_worker(city, x, y))
                .filter(|tile| occupied.insert(*tile))
                .collect();
            let missing = workers.saturating_sub(tiles.len());
            if missing > 0 {
                displaced.push(city.id);
            }
            self.cities[index].specialists[2] += missing as i32;
            self.cities[index].worked_tiles = tiles;
        }
        // citytools founding/transfer arranges the event city before thawing
        // displaced neighboring cities. Existing valid manual allocations in
        // other cities remain untouched.
        let mut queue = vec![city_id];
        queue.extend(displaced.into_iter().filter(|&id| id != city_id));
        for id in queue {
            let Some(index) = self.city_index_by_id(id) else {
                continue;
            };
            for tile in &self.cities[index].worked_tiles {
                occupied.remove(tile);
            }
            let arranged = self.city_manager_result(&self.cities[index], &occupied);
            occupied.extend(arranged.worked_tiles.iter().copied());
            self.cities[index].worked_tiles = arranged.worked_tiles;
            self.cities[index].specialists = arranged.specialists;
        }
    }

    pub(in crate::sim) fn arrange_ai_emergency_workers(&mut self, owner: usize) {
        let indices: Vec<_> = self
            .player_cities_ordered(owner)
            .filter_map(|city| {
                self.cities
                    .iter()
                    .position(|candidate| candidate.id == city.id)
            })
            .collect();
        let mut occupied: std::collections::HashSet<_> = self
            .cities
            .iter()
            .flat_map(|city| city.worked_tiles.iter().copied())
            .collect();
        for index in indices {
            let city = self.cities[index].clone();
            let forecast = self.city_forecast(&city);
            let surplus = self.city_manager_surplus(&city, &forecast);
            // daicity.c:CITY_EMERGENCY or a full granary, not an arbitrary
            // low-food threshold or unconditional optimization every phase.
            if surplus[1] >= 0
                && !forecast.disorder
                && city.food + surplus[0] >= 0
                && city.food != Self::growth_box(city.size)
            {
                continue;
            }
            for tile in &city.worked_tiles {
                occupied.remove(tile);
            }
            // daicity.c:resolve_city_emergency first frees every eligible
            // tile in the emergency city's radius that another same-owner
            // city is working. The donor keeps its population by converting
            // each displaced worker to the default specialist.
            let reclaim: Vec<_> = self
                .map_city_radius(&city)
                .filter_map(|(_, _, x, y)| {
                    let tile = (x, y);
                    if !self.tile_can_support_worker(&city, x, y) {
                        return None;
                    }
                    self.cities
                        .iter()
                        .enumerate()
                        .find(|(other_index, other)| {
                            *other_index != index
                                && other.owner == owner
                                && other.worked_tiles.contains(&tile)
                        })
                        .map(|(other_index, _)| (tile, other_index))
                })
                .collect();
            for (tile, donor_index) in reclaim {
                if let Some(worker) = self.cities[donor_index]
                    .worked_tiles
                    .iter()
                    .position(|&worked| worked == tile)
                {
                    self.cities[donor_index].worked_tiles.remove(worker);
                    self.cities[donor_index].specialists[2] += 1;
                    occupied.remove(&tile);
                }
            }
            let arranged = self.city_manager_result(&city, &occupied);
            occupied.extend(arranged.worked_tiles.iter().copied());
            self.cities[index].worked_tiles = arranged.worked_tiles;
            self.cities[index].specialists = arranged.specialists;
        }
    }

    pub(in crate::sim) fn arrange_government_workers(&mut self, owner: usize) {
        let indices: Vec<_> = self
            .player_cities_ordered(owner)
            .filter_map(|city| {
                self.cities
                    .iter()
                    .position(|candidate| candidate.id == city.id)
            })
            .collect();
        let mut occupied: std::collections::HashSet<_> = self
            .cities
            .iter()
            .flat_map(|city| city.worked_tiles.iter().copied())
            .collect();
        for index in indices {
            for tile in &self.cities[index].worked_tiles {
                occupied.remove(tile);
            }
            let arranged = self.city_manager_result(&self.cities[index], &occupied);
            occupied.extend(arranged.worked_tiles.iter().copied());
            self.cities[index].worked_tiles = arranged.worked_tiles;
            self.cities[index].specialists = arranged.specialists;
        }
    }

    pub(in crate::sim) fn assign_specialist(
        &mut self,
        city_id: u32,
        index: usize,
        delta: i32,
    ) -> bool {
        let Some(city) = self
            .cities
            .iter_mut()
            .find(|city| city.id == city_id && city.owner == HUMAN)
        else {
            return false;
        };
        if index >= city.specialists.len()
            || !matches!(delta, -1 | 1)
            || (delta == 1 && city.tile_workers() == 0)
            || (delta == -1 && city.specialists[index] == 0)
        {
            return false;
        }
        city.specialists[index] += delta;
        self.reconcile_city_workers();
        true
    }

    pub(in crate::sim) fn tile_can_support_worker(&self, city: &City, x: i32, y: i32) -> bool {
        self.tile(x, y).owner == Some(city.owner)
            && !self.any_city_at_cached(x, y)
            && !self.any_unit_at(x, y, |unit| {
                unit.kind.is_military()
                    && unit.transport.is_none()
                    && self.at_war(unit.owner, city.owner)
            })
    }

    pub(crate) fn can_assign_city_tile(&self, city: &City, x: i32, y: i32) -> bool {
        self.map_city_radius(city)
            .any(|(_, _, tx, ty)| (x, y) == (tx, ty))
            && self.tile_can_support_worker(city, x, y)
            && !self
                .cities
                .iter()
                .any(|other| other.worked_tiles.contains(&(x, y)))
    }

    pub(in crate::sim) fn toggle_city_tile(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == id && city.owner == HUMAN)
        else {
            return false;
        };
        let city = &self.cities[index];
        if let Some(tile) = city.worked_tiles.iter().position(|&pos| pos == (x, y)) {
            self.cities[index].worked_tiles.remove(tile);
            self.cities[index].specialists[2] += 1;
        } else {
            let Some(specialist) = [2, 0, 1].into_iter().find(|&i| city.specialists[i] > 0) else {
                return false;
            };
            if !self.can_assign_city_tile(city, x, y) {
                return false;
            }
            self.cities[index].specialists[specialist] -= 1;
            self.cities[index].worked_tiles.push((x, y));
        }
        true
    }

    pub(in crate::sim) fn reconcile_city_workers(&mut self) {
        let mut occupied = std::collections::HashSet::new();
        // Preserve existing assignments before placing any newly available workers.
        for index in 0..self.cities.len() {
            let city = &self.cities[index];
            let tiles = city
                .worked_tiles
                .iter()
                .copied()
                .filter(|&(x, y)| self.tile_can_support_worker(city, x, y))
                .filter(|pos| !occupied.contains(pos))
                .take(city.tile_workers() as usize)
                .collect::<Vec<_>>();
            occupied.extend(tiles.iter().copied());
            self.cities[index].worked_tiles = tiles;
        }
        for index in 0..self.cities.len() {
            let city = &self.cities[index];
            let needed = city.tile_workers() as usize - city.worked_tiles.len();
            if needed == 0 {
                continue;
            }
            let mut candidates: Vec<_> = self
                .map_city_radius(city)
                .filter(|&(_, _, x, y)| !occupied.contains(&(x, y)))
                .filter(|&(_, _, x, y)| self.tile_can_support_worker(city, x, y))
                .map(|(_, _, x, y)| {
                    let tile = self.tile(x, y);
                    let (food, shields, trade) = tile.yields();
                    (
                        tile.contamination_output(food) * 4
                            + tile.contamination_output(shields) * 3
                            + tile.contamination_output(trade) * 2,
                        (x, y),
                    )
                })
                .collect();
            candidates.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
            let city = &mut self.cities[index];
            for (_, pos) in candidates.into_iter().take(needed) {
                city.worked_tiles.push(pos);
                occupied.insert(pos);
            }
            city.specialists[2] += city.tile_workers() - city.worked_tiles.len() as i32;
        }
    }
}
