use super::*;

pub(crate) struct CityForecast {
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) trade: i32,
    pub(crate) science: i32,
    pub(super) gross_science: i32,
    pub(super) gross_gold: i32,
    pub(crate) gold: i32,
    pub(crate) luxury: i32,
    pub(crate) support: super::upkeep::UnitUpkeep,
    pub(crate) building_upkeep: i32,
    pub(crate) disorder: bool,
    pub(super) happy: bool,
}

impl CityForecast {
    pub(crate) fn food_surplus(&self, size: i32, support_food: i32) -> i32 {
        let surplus = self.food - size * CITIZEN_FOOD_COST - support_food;
        if self.disorder {
            surplus.min(0)
        } else {
            surplus
        }
    }

    pub(crate) fn shield_surplus(&self) -> i32 {
        if self.disorder {
            0
        } else {
            (self.shields - self.support.shields).max(0)
        }
    }

    pub(crate) fn gold_surplus(&self, unit_gold: i32) -> i32 {
        self.gold - self.building_upkeep - unit_gold
    }
}

impl City {
    pub(crate) fn tile_workers(&self) -> i32 {
        self.size - self.specialists.iter().sum::<i32>()
    }

    pub(super) fn remove_specialists(&mut self, mut excess: i32) {
        // The ruleset removes entertainers first, followed by scientists and taxmen.
        for index in [2, 0, 1] {
            let removed = excess.min(self.specialists[index]);
            self.specialists[index] -= removed;
            excess -= removed;
        }
    }

    pub(super) fn remove_citizen_assignments(&mut self, loss: i32) {
        let before = self.specialists.iter().sum::<i32>();
        self.remove_specialists(loss);
        let remaining = loss - (before - self.specialists.iter().sum::<i32>());
        if remaining <= 0 {
            return;
        }
        // cityturn.c:city_reduce_workers removes the first worked fields in
        // native city-index order, before the whole city is rearranged.
        let mut order: Vec<_> = Simulation::city_radius(self).collect();
        order.sort_by_key(|&(dx, dy, _, _)| (isohex_vector_distance(dx, dy).pow(2), dx, dy));
        let removed: std::collections::HashSet<_> = order
            .into_iter()
            .map(|(_, _, x, y)| (x, y))
            .filter(|tile| self.worked_tiles.contains(tile))
            .take(remaining as usize)
            .collect();
        self.worked_tiles.retain(|tile| !removed.contains(tile));
    }

    pub(super) fn has_building(&self, building: Building) -> bool {
        self.buildings.contains(&building)
    }
}

// common/aicore/cm.c:estimate_fitness and compare_tile_type_by_fitness.
fn cm_estimated_fitness(
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

fn cm_compare_types(a: (f64, usize, [i32; 6]), b: (f64, usize, [i32; 6])) -> std::cmp::Ordering {
    let difference = b.0 - a.0;
    if difference > 0.5 {
        std::cmp::Ordering::Greater
    } else if difference < -0.5 {
        std::cmp::Ordering::Less
    } else {
        a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2))
    }
}

const CM_INFINITY: i32 = 1_000_000_000;
const CM_MAX_LOOP: usize = 27_500;

struct CmLattice {
    tiles: Vec<(i32, i32)>,
    groups: Vec<Vec<usize>>,
    production: Vec<[i32; 6]>,
    capacity: Vec<i32>,
    better: Vec<Vec<usize>>,
    orders: [Vec<usize>; 2],
    by_stat: [Vec<usize>; 6],
}

#[derive(Clone)]
struct CmPartial {
    counts: Vec<i32>,
    production: [i32; 6],
    idle: i32,
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

struct CmQueryResult {
    city: Option<City>,
    #[cfg(test)]
    iterations: usize,
    #[cfg(test)]
    aborted: bool,
    #[cfg(test)]
    disorder_prunes: usize,
}

impl Simulation {
    fn city_manager_surplus(&self, city: &City, forecast: &CityForecast) -> [i32; 6] {
        [
            forecast.food_surplus(city.size, forecast.support.food),
            if forecast.disorder {
                (forecast.shields - forecast.support.shields).min(0)
            } else {
                forecast.shields - forecast.support.shields
            },
            forecast.trade,
            forecast.gold - forecast.building_upkeep - forecast.support.gold,
            forecast.luxury,
            forecast.science,
        ]
    }

    pub(super) fn city_manager_result(
        &self,
        city: &City,
        occupied: &std::collections::HashSet<(i32, i32)>,
    ) -> City {
        let citizens = city.size.max(0) as usize;
        let mut city_tiles: Vec<_> = self.map_city_radius(city).collect();
        // common/city.c:compare_iter_index defines reproducible city indices.
        city_tiles.sort_by_key(|&(dx, dy, _, _)| (isohex_vector_distance(dx, dy).pow(2), dx, dy));
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
        let copernicus = i32::from(
            city.has_building(Building::COPERNICUS_OBSERVATORY)
                && !self.building_is_obsolete(city, Building::COPERNICUS_OBSERVATORY, None),
        );
        let mut production: Vec<[i32; 6]> = groups
            .iter()
            .map(|group| [group.1.0, group.1.1, group.1.2, 0, 0, copernicus])
            .collect();
        for (index, wonder) in [
            Building::DARWINS_VOYAGE,
            Building::A_SMITHS_TRADING_CO,
            Building::SHAKESPEARES_THEATER,
        ]
        .into_iter()
        .enumerate()
        {
            let mut output = [0; 6];
            output[[5, 3, 4][index]] = 2 + i32::from(self.completed_wonders.contains(&wonder));
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
        let commercial = self.commercial_output_bonus(city) - 100;
        let palace = city.has_building(Building::PALACE)
            || city.has_building(Building::ECCLESIASTICAL_PALACE);
        let gold_adjustment = if palace {
            match government {
                Government::Despotism => 75,
                Government::Monarchy => 50,
                _ => 0,
            }
        } else {
            0
        };
        let bonuses = [
            100,
            self.shield_output_bonus(city),
            100,
            100 + commercial + gold_adjustment,
            100 + commercial - 50 * i32::from(government == Government::Anarchy),
            100 + self.science_building_bonus(city)
                - 50 * i32::from(government == Government::Fundamentalism),
        ];
        let rates = if government == Government::Anarchy {
            [0, 0, 100]
        } else {
            self.players[city.owner].rates
        };
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

    fn city_manager_raw_production(&self, city: &City, raw: [i32; 6]) -> [i32; 6] {
        // cm.c:compute_max_stats_heuristic combines maxima for six different
        // completions into a synthetic citizen_base, then set_city_production.
        let center = self.city_tile_yields(city, city.x, city.y, true, None);
        let shield = (raw[1] + center.1)
            .wrapping_mul(self.shield_output_bonus(city))
            .wrapping_div(100)
            .max(0);
        let trade = (raw[2] + center.2 + self.route_trade(city)).max(0);
        let (shield, trade) = self.government_output_waste(city, shield, trade, None);
        let copernicus = i32::from(
            city.has_building(Building::COPERNICUS_OBSERVATORY)
                && !self.building_is_obsolete(city, Building::COPERNICUS_OBSERVATORY, None),
        );
        let [science, gold, luxury] = self.split_city_trade_from_citizens(
            city,
            trade,
            self.players[city.owner].rates,
            [raw[5] + copernicus, raw[3], raw[4]],
        );
        [
            self.government_food_output(
                city,
                (raw[0] + center.0).max(0),
                self.players[city.owner].government,
            ),
            shield,
            trade,
            gold.max(0),
            luxury.max(0),
            science.max(0),
        ]
    }

    fn city_manager_query(
        &self,
        city: &City,
        lattice: &CmLattice,
        food_factor: i32,
        minima: [i32; 6],
        emergency: bool,
        limit: usize,
    ) -> CmQueryResult {
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
        let usage = [
            city.size * CITIZEN_FOOD_COST + initial.support.food,
            initial.support.shields,
            0,
            initial.building_upkeep + initial.support.gold,
            0,
            0,
        ];
        let min_production: [i32; 6] = std::array::from_fn(|index| usage[index] + minima[index]);
        let mut current = CmPartial {
            counts: vec![0; lattice.production.len()],
            production: if emergency { [-CM_INFINITY; 6] } else { [0; 6] },
            idle: city.size,
        };
        let mut best = current.clone();
        let mut best_fitness = (false, -i64::from(CM_INFINITY));
        let mut min_luxury = -CM_INFINITY;
        let mut stack: Vec<usize> = Vec::new();
        let mut iterations = 0;
        #[cfg(test)]
        let disorder_prunes = std::cell::Cell::new(0);
        let basis = self.players[city.owner].government.empire_size();
        let cities = self
            .cities
            .iter()
            .filter(|other| other.owner == city.owner)
            .count() as i32;
        let max_content = (4 - if basis > 0 {
            (cities - 1).max(0) / basis
        } else {
            0
        })
        .max(0);
        loop {
            if current.idle == 0 {
                let candidate = self.city_manager_apply(city, lattice, &current);
                let forecast = self.city_forecast(&candidate);
                let surplus = self.city_manager_surplus(&candidate, &forecast);
                if forecast.disorder {
                    min_luxury = surplus[4]
                        + 2 * (candidate.specialists.iter().sum::<i32>() - max_content).max(0)
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
                    for output in 0..6 {
                        let mut completion = partial.clone();
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
                        raw[output] = completion.production[output];
                    }
                }
                let production = self.city_manager_raw_production(city, raw);
                let meets_min = (0..6).all(|output| production[output] >= min_production[output]);
                let beats_best = (0..6).any(|output| {
                    factors[output] > 0 && production[output] > best.production[output]
                });
                let specialists = partial.counts[lattice.groups.len()..].iter().sum::<i32>();
                let luxury = production[4] + 2 * (specialists + partial.idle - max_content).max(0);
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

    pub(super) fn arrange_city_event_workers(&mut self, city_id: u32) {
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
            let Some(index) = self.cities.iter().position(|city| city.id == id) else {
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

    pub(super) fn arrange_ai_emergency_workers(&mut self, owner: usize) {
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
            let city = &self.cities[index];
            let forecast = self.city_forecast(city);
            let surplus = self.city_manager_surplus(city, &forecast);
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
            let arranged = self.city_manager_result(city, &occupied);
            occupied.extend(arranged.worked_tiles.iter().copied());
            self.cities[index].worked_tiles = arranged.worked_tiles;
            self.cities[index].specialists = arranged.specialists;
        }
    }

    pub(super) fn arrange_government_workers(&mut self, owner: usize) {
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

    pub(crate) fn city_forecast(&self, city: &City) -> CityForecast {
        self.city_forecast_with_tech(city, None)
    }

    pub(super) fn city_forecast_with_tech(
        &self,
        city: &City,
        granted: Option<Tech>,
    ) -> CityForecast {
        let (food, shields, trade) = self.city_yields_with_tech(city, granted);
        let [science, gold, luxury] = self.city_trade(city, trade);
        let (disorder, happy) = self.city_mood(city, luxury);
        CityForecast {
            food,
            shields,
            trade,
            luxury,
            disorder,
            happy,
            gross_science: science,
            gross_gold: gold,
            science: if disorder { 0 } else { science },
            gold: if disorder { 0 } else { gold },
            support: self.city_unit_upkeep(city),
            building_upkeep: self.building_upkeep(city),
        }
    }

    pub(super) fn assign_specialist(&mut self, city_id: u32, index: usize, delta: i32) -> bool {
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

    pub(super) fn raise_rate(&mut self, owner: usize, index: usize) -> bool {
        let player = &mut self.players[owner];
        let rates = &mut player.rates;
        if player.government == Government::Anarchy
            || index >= rates.len()
            || rates[index] >= player.government.max_rate()
        {
            return false;
        }
        let donor = (0..rates.len())
            .filter(|&other| other != index)
            .max_by_key(|&other| rates[other])
            .unwrap();
        rates[donor] -= 10;
        rates[index] += 10;
        true
    }

    pub(crate) fn city_trade(&self, city: &City, trade: i32) -> [i32; 3] {
        self.split_city_trade(city, trade, self.players[city.owner].rates)
    }

    pub(super) fn science_building_bonus(&self, city: &City) -> i32 {
        [
            (Building::LIBRARY, Building::GREAT_LIBRARY),
            (Building::UNIVERSITY, Building::ISAAC_NEWTONS_COLLEGE),
            (Building::RESEARCH_LAB, Building::INTERNET),
        ]
        .into_iter()
        .filter(|&(building, _)| city.has_building(building))
        .map(|(_, wonder)| 50 + 50 * i32::from(self.completed_wonders.contains(&wonder)))
        .sum()
    }

    pub(super) fn distribute_trade(trade: i32, rates: [i32; 3]) -> [i32; 3] {
        let scaled = rates.map(|rate| trade * rate);
        let mut output = scaled.map(|value| value / 100);
        let mut order = [0, 1, 2];
        // Freeciv assigns ties to the smallest integer share, then science, tax, luxury.
        order.sort_by_key(|&i| (-(scaled[i] % 100), output[i], i));
        for &index in order
            .iter()
            .take((trade - output.iter().sum::<i32>()) as usize)
        {
            output[index] += 1;
        }
        output
    }

    pub(super) fn split_city_trade(&self, city: &City, trade: i32, rates: [i32; 3]) -> [i32; 3] {
        let mut citizens = [0; 3];
        for (index, wonder) in [
            Building::DARWINS_VOYAGE,
            Building::A_SMITHS_TRADING_CO,
            Building::SHAKESPEARES_THEATER,
        ]
        .into_iter()
        .enumerate()
        {
            citizens[index] =
                city.specialists[index] * (2 + i32::from(self.completed_wonders.contains(&wonder)));
        }
        if city.has_building(Building::COPERNICUS_OBSERVATORY)
            && !self.building_is_obsolete(city, Building::COPERNICUS_OBSERVATORY, None)
        {
            citizens[0] += 1 + city.worked_tiles.len().min(city.tile_workers() as usize) as i32;
        }
        self.split_city_trade_from_citizens(city, trade, rates, citizens)
    }

    fn split_city_trade_from_citizens(
        &self,
        city: &City,
        trade: i32,
        rates: [i32; 3],
        citizens: [i32; 3],
    ) -> [i32; 3] {
        let government = self.players[city.owner].government;
        let rates = if government == Government::Anarchy {
            [0, 0, 100]
        } else {
            rates
        };
        let mut output = Self::distribute_trade(trade, rates);

        for index in 0..3 {
            output[index] += citizens[index];
        }
        // common/city.c:get_city_tithes_bonus/set_city_production:
        // Happiness_To_Gold uses full Make_Content + Force_Content effect
        // amounts, even when no citizen needs calming. Civ2Civ3 has no
        // Force_Content rows at Normal difficulty.
        if government == Government::Fundamentalism {
            output[1] += self.building_content(city);
        }

        output[0] += 4 * i32::from(city.has_building(Building::GREAT_LIBRARY))
            + 6 * i32::from(city.has_building(Building::ISAAC_NEWTONS_COLLEGE))
            + i32::from(self.has_active_wonder(city.owner, Building::DARWINS_VOYAGE));
        output[2] += 2 * i32::from(government == Government::Federation);
        output[2] +=
            4 * i32::from(
                city.has_building(Building::HANGING_GARDENS)
                    && !self.building_is_obsolete(city, Building::HANGING_GARDENS, None),
            ) + 2 * i32::from(self.has_active_wonder(city.owner, Building::JS_BACHS_CATHEDRAL))
                + 2 * i32::from(self.has_active_wonder(city.owner, Building::CURE_FOR_CANCER))
                + i32::from(self.has_active_wonder(city.owner, Building::SHAKESPEARES_THEATER))
                + 5 * i32::from(city.has_building(Building::SHAKESPEARES_THEATER));

        let commercial_bonus = self.commercial_output_bonus(city) - 100;
        for (index, amount) in output.iter_mut().enumerate() {
            let bonus = if index == 0 {
                self.science_building_bonus(city)
            } else {
                commercial_bonus
            };
            let adjustment = match (government, index) {
                (Government::Anarchy, 2) | (Government::Fundamentalism, 0) => -50,
                (Government::Despotism, 1)
                    if city.has_building(Building::PALACE)
                        || city.has_building(Building::ECCLESIASTICAL_PALACE) =>
                {
                    75
                }
                (Government::Monarchy, 1)
                    if city.has_building(Building::PALACE)
                        || city.has_building(Building::ECCLESIASTICAL_PALACE) =>
                {
                    50
                }
                _ => 0,
            };
            *amount = amount.wrapping_mul(100 + bonus + adjustment) / 100;
        }
        output
    }

    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
        self.city_mood(city, luxury).0
    }

    pub(super) fn building_content(&self, city: &City) -> i32 {
        let has = |building| city.has_building(building);
        let owns = |wonder| self.has_active_wonder(city.owner, wonder);
        let communist = self.players[city.owner].government == Government::Communism;
        let mut content = 2 * i32::from(has(Building::POLICE_STATION))
            + i32::from(has(Building::COURTHOUSE))
            + i32::from(owns(Building::HANGING_GARDENS));
        if has(Building::TEMPLE) {
            content += 1
                + i32::from(!communist && self.has_tech(city.owner, Tech::MYSTICISM))
                + 2 * i32::from(owns(Building::TEMPLE_OF_ARTEMIS));
        }
        if has(Building::CATHEDRAL) {
            content += 3
                + i32::from(!communist && self.has_tech(city.owner, Tech::THEOLOGY))
                + 3 * i32::from(owns(Building::MICHELANGELOS_CHAPEL));
        }
        if has(Building::AMPHITHEATER) {
            content += 3 + i32::from(self.has_tech(city.owner, Tech::ELECTRICITY));
        }
        if owns(Building::MAUSOLEUM_OF_MAUSOLOS) {
            content += i32::from(has(Building::CITY_WALLS)) + i32::from(has(Building::COURTHOUSE));
        }
        content
    }

    pub(super) fn city_mood(&self, city: &City, luxury: i32) -> (bool, bool) {
        let (happy, unhappy, angry) = self.city_feelings(city, luxury);
        (
            happy < unhappy + 2 * angry,
            city.size >= 3 && unhappy == 0 && angry == 0 && happy >= (city.size + 1) / 2,
        )
    }

    pub(super) fn city_feelings(&self, city: &City, luxury: i32) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, true, true)
    }

    pub(super) fn city_feelings_before_military(
        &self,
        city: &City,
        luxury: i32,
    ) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, false, true)
    }

    pub(super) fn city_feelings_after_effects(&self, city: &City, luxury: i32) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, false, false)
    }

    fn city_feelings_stages(
        &self,
        city: &City,
        luxury: i32,
        military: bool,
        martial_enabled: bool,
    ) -> (i32, i32, i32) {
        let government = self.players[city.owner].government;
        // Apply luxury before Make_Content.
        let cities = self
            .cities
            .iter()
            .filter(|other| other.owner == city.owner)
            .count() as i32;
        let basis = government.empire_size();
        let empire_penalty = if basis > 0 {
            (cities - 1).max(0) / basis
        } else {
            0
        };
        let base = 4 - empire_penalty;
        let workers = city.tile_workers();
        let specialists = city.size - workers;
        let content = (base.clamp(0, city.size) - specialists).max(0);
        let mut angry = (-base).clamp(0, workers);
        let mut unhappy = workers - content - angry;
        let mut upgrades = luxury / 2;
        let calmed = angry.min(upgrades);
        angry -= calmed;
        unhappy += calmed;
        upgrades -= calmed;
        let happy_content = content.min(upgrades);
        upgrades -= happy_content;
        let happy_unhappy = unhappy.min(upgrades / 2);
        unhappy -= happy_unhappy;
        let mut content = content - happy_content;
        upgrades -= happy_unhappy * 2;
        if upgrades >= 1 {
            let promoted = unhappy.min(1);
            unhappy -= promoted;
            content += promoted;
        }

        let martial = match government {
            Government::Anarchy | Government::Despotism | Government::Monarchy => 1,
            Government::Communism => 2,
            _ => 0,
        };
        let garrison = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == city.owner
                    && !unit.kind.is_civilian()
                    && (unit.x, unit.y) == (city.x, city.y)
            })
            .count()
            .min(3) as i32;
        let pacification = i32::from(government == Government::Tribal)
            + if martial_enabled {
                martial * garrison
            } else {
                0
            }
            + self.building_content(city);
        let calming_angry = pacification.min(angry);
        angry -= calming_angry;
        unhappy += calming_angry;
        let promoted = unhappy.min(pacification - calming_angry);
        unhappy -= promoted;
        content += promoted;

        let mut happy = happy_content + happy_unhappy;
        if !military {
            return (happy, unhappy, angry);
        }
        let pressure = self.city_military_unhappiness(city);
        let from_content = content.min(pressure);
        unhappy += from_content;
        let pressure = pressure - from_content;
        let from_happy = happy.min(pressure / 2);
        unhappy += from_happy;
        happy = (happy - from_happy - pressure % 2).max(0);
        (happy, unhappy, angry)
    }

    /// city_unit_unhappiness summed over the native supported-unit list.
    pub(super) fn city_military_unhappiness(&self, city: &City) -> i32 {
        use super::unit_catalog::{UnitClass, UnitFlag};
        let government = self.players[city.owner].government;
        let suffrage = self.has_active_wonder(city.owner, Building::WOMENS_SUFFRAGE);
        let factor = match government {
            Government::Anarchy | Government::Tribal | Government::Despotism => 0,
            Government::Democracy => 2 - i32::from(suffrage),
            _ => 1,
        };
        if factor == 0 {
            return 0;
        }
        // The provided ruleset has Make_Content_Mil allowances, and NO
        // Make_Content_Mil_Per effects. Police relieve a fixed city total.
        let mut free = match government {
            Government::Republic => 1,
            Government::Monarchy | Government::Communism => 3,
            _ => 2,
        };
        free += i32::from(suffrage) - i32::from(suffrage && government == Government::Democracy);
        free += i32::from(city.has_building(Building::POLICE_STATION))
            * if government == Government::Democracy && !suffrage {
                2
            } else {
                1
            };
        free += i32::from(self.has_active_wonder(city.owner, Building::KING_RICHARDS_CRUSADE));
        free += i32::from(self.has_active_wonder(city.owner, Building::STATUE_OF_ZEUS));
        let mut supported: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.home_city == Some(city.id) && unit.owner == city.owner)
            .collect();
        supported.sort_by_key(|unit| std::cmp::Reverse(unit.home_order));
        let mut pressure = 0;
        for unit in supported {
            let happy_cost = unit.kind.base_upkeep().2 * factor;
            if happy_cost <= 0 {
                continue;
            }
            let class = unit.kind.unit_class();
            let friendly_extra = match self.tile(unit.x, unit.y).base {
                Some(Base::Fortress) => matches!(
                    class,
                    UnitClass::Land
                        | UnitClass::SmallLand
                        | UnitClass::BigLand
                        | UnitClass::Merchant
                ),
                Some(Base::Airbase) => matches!(
                    class,
                    UnitClass::Air | UnitClass::Helicopter | UnitClass::Missile
                ),
                _ => false,
            } && self.cities.iter().any(|nearby| {
                self.allied(nearby.owner, unit.owner)
                    && self.map_distance(unit.x, unit.y, nearby.x, nearby.y) <= 3
            });
            let aggressive = unit.kind.stats().attack > 0
                && !self
                    .cities
                    .iter()
                    .any(|other| (other.x, other.y) == (unit.x, unit.y))
                && self.tile(unit.x, unit.y).owner != Some(unit.owner)
                && !friendly_extra;
            if !aggressive && !unit.kind.has_flag(UnitFlag::FieldUnit) {
                continue;
            }
            let relieved = free.min(happy_cost);
            free -= relieved;
            pressure += happy_cost - relieved;
        }
        pressure
    }

    pub(super) fn offensive_military_unhappiness(&self, city: &City) -> bool {
        self.city_military_unhappiness(city) > 0
    }

    pub(super) fn balance_building_upkeep(&mut self, owner: usize, city_id: u32) {
        while self.players[owner].gold < 0 {
            let candidates: Vec<_> = self
                .cities
                .iter()
                .enumerate()
                .filter(|(_, city)| city.id == city_id)
                .flat_map(|(index, city)| {
                    let free = self.free_building_upkeep(city);
                    city.buildings
                        .iter()
                        .copied()
                        .filter(move |building| {
                            building.data().genus == crate::rules::Genus::Improvement
                                && building.data().upkeep > free
                        })
                        .map(move |building| (index, building))
                })
                .collect();
            if candidates.is_empty() {
                break;
            }
            let (index, building) = candidates[self.rand_below(candidates.len() as i32) as usize];
            let city = &mut self.cities[index];
            city.buildings.retain(|&built| built != building);
            self.players[owner].gold += building.data().cost;
            if owner == HUMAN {
                let message = format!(
                    "{} sold its {} to cover upkeep.",
                    city.name,
                    building.data().name
                );
                self.add_event(message);
            }
        }
    }

    fn free_building_upkeep(&self, city: &City) -> i32 {
        match self.players[city.owner].government {
            Government::Anarchy => 99,
            Government::Communism => 1,
            _ => i32::from(
                city.has_building(Building::STOCK_EXCHANGE)
                    && self.has_active_wonder(city.owner, Building::A_SMITHS_TRADING_CO),
            ),
        }
    }

    pub(crate) fn building_upkeep(&self, city: &City) -> i32 {
        city.buildings
            .iter()
            .map(|building| (building.data().upkeep - self.free_building_upkeep(city)).max(0))
            .sum()
    }

    pub(crate) fn city_size_limit(city: &City) -> i32 {
        if ![
            Building::AQUEDUCT,
            Building::AQUEDUCT_RIVER,
            Building::AQUEDUCT_LAKE,
        ]
        .iter()
        .any(|name| city.has_building(*name))
        {
            8
        } else if city.has_building(Building::SEWER_SYSTEM) {
            i32::MAX
        } else {
            16
        }
    }

    pub(super) fn food_retained(city: &City) -> i32 {
        match city.size {
            ..=3 => 50,
            _ if !city.has_building(Building::GRANARY) => 0,
            4 => 50,
            5..=6 => 34,
            _ => 25,
        }
    }
}

impl Simulation {
    pub(crate) fn military_unhappiness_rules_playtest(&self) -> bool {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known.clear();
        game.players[HUMAN].government = Government::Democracy;
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 10, 24);
        let home = game.cities[0].id;
        for _ in 0..5 {
            let id = game.spawn_unit(HUMAN, UnitKind::Bomber, 10, 24);
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(home);
            unit.home_order = id;
        }
        let raw = game.city_military_unhappiness(&game.cities[0]) == 8
            && game.offensive_military_unhappiness(&game.cities[0])
            && game.city_feelings(&game.cities[0], 0).1 <= game.cities[0].size;
        game.cities[0].buildings.push(Building::POLICE_STATION);
        let police = game.city_military_unhappiness(&game.cities[0]) == 6;
        game.cities[0].buildings.push(Building::WOMENS_SUFFRAGE);
        let suffrage = game.city_military_unhappiness(&game.cities[0]) == 2;
        for unit in &mut game.units {
            unit.kind = UnitKind::Awacs;
        }
        raw && police && suffrage && !game.offensive_military_unhappiness(&game.cities[0])
    }

    pub(crate) fn ai_city_manager_playtest(&self) -> bool {
        let mut scratch = self.clone();
        scratch.units.clear();
        scratch.cities.clear();
        for tile in &mut scratch.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.road = false;
            tile.river = false;
        }
        scratch.create_city(1, 12, 24);
        let mut city = scratch.cities[0].clone();
        city.size = 6;
        city.buildings.push(Building::TEMPLE);
        scratch.players[1].known.push(Tech::MYSTICISM);
        city.worked_tiles.clear();
        city.specialists = [6, 0, 0];
        let arranged = scratch.city_manager_result(&city, &Default::default());
        let output = scratch.city_forecast(&arranged);
        let mut native_order: Vec<_> = Self::city_radius(&city)
            .filter(|&(_, _, x, y)| scratch.tile_can_support_worker(&city, x, y))
            .collect();
        native_order.sort_by_key(|&(dx, dy, _, _)| (isohex_vector_distance(dx, dy).pow(2), dx, dy));
        let expected: Vec<_> = native_order
            .into_iter()
            .take(arranged.worked_tiles.len())
            .map(|(_, _, x, y)| (x, y))
            .collect();
        arranged.worked_tiles.len() + arranged.specialists.iter().sum::<i32>() as usize == 6
            && !output.disorder
            && output.food_surplus(6, output.support.food) >= 0
            && arranged.worked_tiles != city.worked_tiles
            && arranged.worked_tiles == expected
    }
}

impl Simulation {
    pub(crate) fn fundamentalism_tithes_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.players[1].known = vec![Tech::MYSTICISM];
        self.players[1].government = Government::Fundamentalism;
        self.players[1].rates = [100, 0, 0];
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        self.create_city(1, 12, 24);
        self.create_city(1, 25, 24);
        self.cities[1]
            .buildings
            .extend([Building::TEMPLE_OF_ARTEMIS, Building::HANGING_GARDENS]);
        self.cities[0]
            .buildings
            .extend([Building::TEMPLE, Building::MARKETPLACE]);
        let city = self.cities[0].clone();
        let no_unhappy = self.city_feelings(&city, 0).1 == 0;
        // Temple1+Mysticism1+Artemis2+HangingGardens1=5 rawgold;
        // Marketplace raises this to7, regardless of citysize1 needs.
        let tithe = self.city_trade(&city, 0)[1] == 7
            && self.city_manager_raw_production(&city, [0; 6])[3] == 7;
        self.players[1].government = Government::Monarchy;
        let other_government = self.city_trade(&city, 0)[1] == 0;
        no_unhappy && tithe && other_government
    }
}

impl Simulation {
    pub(crate) fn ai_field_unit_unrest_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.players[1].government = Government::Republic;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        self.create_city(1, 12, 24);
        self.cities[0].size = 4;
        let home = self.cities[0].id;
        for _ in 0..2 {
            let id = self.spawn_unit(1, UnitKind::Bomber, 12, 24);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .home_city = Some(home);
        }
        let city = self.cities[0].clone();
        let in_city = self.city_feelings_before_military(&city, 0).1 == 0
            && self.city_feelings(&city, 0).1 == 1;
        let base = offset_native_by_map(12, 24, 1, 0);
        self.tile_mut(base.0, base.1).base = Some(Base::Airbase);
        self.tile_mut(base.0, base.1).owner = Some(1);
        for unit in &mut self.units {
            (unit.x, unit.y) = base;
        }
        let friendly_base = self.city_feelings(&city, 0).1 == 1;
        for unit in &mut self.units {
            unit.kind = UnitKind::Fighter;
        }
        let fighter = self.city_feelings(&city, 0).1 == 0;
        in_city && friendly_base && fighter
    }
}

#[cfg(test)]
mod city_manager_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn military_unhappiness_uses_source_government_allowances_and_fixed_police_relief() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.military_unhappiness_rules_playtest());
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known.clear();
        game.create_city(HUMAN, 10, 24);
        let home = game.cities[0].id;
        for _ in 0..5 {
            let id = game.spawn_unit(HUMAN, UnitKind::Bomber, 10, 24);
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(home);
            unit.home_order = id;
        }
        for (government, expected) in [
            (Government::Anarchy, 0),
            (Government::Tribal, 0),
            (Government::Despotism, 0),
            (Government::Monarchy, 2),
            (Government::Communism, 2),
            (Government::Fundamentalism, 3),
            (Government::Federation, 3),
            (Government::Republic, 4),
            (Government::Democracy, 8),
        ] {
            game.players[HUMAN].government = government;
            assert_eq!(
                game.city_military_unhappiness(&game.cities[0]),
                expected,
                "{government:?}"
            );
        }
        game.cities[0].buildings.push(Building::POLICE_STATION);
        assert_eq!(
            game.city_military_unhappiness(&game.cities[0]),
            6,
            "Democracy police free two city-wide points, not two per unit"
        );
        game.cities[0].buildings.push(Building::WOMENS_SUFFRAGE);
        assert_eq!(game.city_military_unhappiness(&game.cities[0]), 2);
        game.players[HUMAN].government = Government::Republic;
        assert_eq!(game.city_military_unhappiness(&game.cities[0]), 2);
        game.cities[0]
            .buildings
            .extend([Building::KING_RICHARDS_CRUSADE, Building::STATUE_OF_ZEUS]);
        assert_eq!(game.city_military_unhappiness(&game.cities[0]), 0);
        assert!(!game.offensive_military_unhappiness(&game.cities[0]));
    }

    #[test]
    fn raw_military_pressure_survives_feelings_saturation_and_respects_base_native_classes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known.clear();
        game.players[HUMAN].government = Government::Republic;
        game.create_city(HUMAN, 10, 24);
        let home = game.cities[0].id;
        let site = offset_native_by_map(10, 24, 1, 0);
        game.tile_mut(site.0, site.1).owner = None;
        game.tile_mut(site.0, site.1).base = Some(Base::Fortress);
        for _ in 0..3 {
            let id = game.spawn_unit(HUMAN, UnitKind::Fighter, site.0, site.1);
            game.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .home_city = Some(home);
        }
        assert_eq!(
            game.city_military_unhappiness(&game.cities[0]),
            2,
            "Fortress is not a native nonaggressive extra for Air class"
        );
        assert!(game.offensive_military_unhappiness(&game.cities[0]));
        game.tile_mut(site.0, site.1).base = Some(Base::Airbase);
        assert_eq!(game.city_military_unhappiness(&game.cities[0]), 0);
        for unit in &mut game.units {
            unit.kind = UnitKind::Bomber;
        }
        assert_eq!(
            game.city_military_unhappiness(&game.cities[0]),
            2,
            "FieldUnit still pays in friendly Airbase"
        );
    }

    #[test]
    fn field_units_pay_happy_upkeep_even_in_cities_and_friendly_bases() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_field_unit_unrest_playtest());
        let city = game.cities[0].clone();
        for kind in [UnitKind::Bomber, UnitKind::StealthBomber, UnitKind::Nuclear] {
            for unit in &mut game.units {
                unit.kind = kind;
                (unit.x, unit.y) = (city.x, city.y);
            }
            assert_eq!(game.city_feelings(&city, 0).1, 1, "{kind:?}");
        }
        let mut police = city.clone();
        police.buildings.push(Building::POLICE_STATION);
        assert_eq!(game.city_feelings(&police, 0).1, 0);
    }

    #[test]
    fn carrier_can_apply_martial_law_without_causing_overseas_military_unhappiness() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.players[1].government = Government::Despotism;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        let sea = offset_native_by_map(12, 24, 1, 0);
        game.tile_mut(sea.0, sea.1).terrain = Terrain::Ocean;
        game.create_city(1, 12, 24);
        let mut city = game.cities[0].clone();
        city.size = 6;
        city.specialists = [0; 3];
        assert_eq!(game.city_feelings_after_effects(&city, 0).1, 2);
        let carrier = game.spawn_unit(1, UnitKind::Carrier, 12, 24);
        assert!(!UnitKind::Carrier.is_civilian() && !UnitKind::Carrier.is_military());
        assert_eq!(game.city_feelings(&city, 0).1, 1);
        let unit = game
            .units
            .iter_mut()
            .find(|unit| unit.id == carrier)
            .unwrap();
        (unit.x, unit.y) = sea;
        unit.home_city = Some(city.id);
        game.players[1].government = Government::Democracy;
        assert_eq!(
            game.city_feelings(&city, 0),
            game.city_feelings_before_military(&city, 0)
        );
    }

    #[test]
    fn disorder_preserves_shields_for_support_and_only_removes_positive_surplus() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.players[1].government = Government::Tribal;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Forest;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        let mut city = game.cities[0].clone();
        city.size = 8;
        city.specialists = [0; 3];
        city.worked_tiles = Simulation::city_radius(&city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| game.tile_can_support_worker(&city, x, y))
            .take(8)
            .collect();
        let home = city.id;
        for _ in 0..6 {
            let id = game.spawn_unit(1, UnitKind::Warrior, 25, 24);
            game.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .home_city = Some(home);
        }
        let forecast = game.city_forecast(&city);
        assert!(
            forecast.disorder
                && forecast.support.shields > 0
                && forecast.shields > forecast.support.shields
        );
        // common/city.c:output_types + unhappy_city_check: SURPLUS, not
        // ALL_PRODUCTION, so usage remains available to supported units.
        assert_eq!(game.city_manager_surplus(&city, &forecast)[1], 0);
    }

    #[test]
    fn tithes_apply_owner_wonders_and_marketplace_without_unhappy_citizens() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.fundamentalism_tithes_playtest());
    }

    #[test]
    fn tithes_are_raw_gold_before_commercial_bonus_and_require_fundamentalism() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.players[1].known = vec![Tech::MYSTICISM];
        game.players[1].government = Government::Fundamentalism;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        let mut city = game.cities[0].clone();
        city.specialists = [0, 0, 0];
        city.worked_tiles.clear();
        assert_eq!(game.city_trade(&city, 0)[1], 0);
        city.buildings.push(Building::TEMPLE);
        assert_eq!(game.city_trade(&city, 0)[1], 2);
        city.buildings.push(Building::MARKETPLACE);
        assert_eq!(game.city_trade(&city, 0)[1], 3);
        game.players[1].government = Government::Democracy;
        assert_eq!(game.city_trade(&city, 0)[1], 0);
    }

    #[test]
    fn city_manager_desktop_fixture() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_city_manager_playtest());
    }

    #[test]
    fn city_manager_matches_small_city_lattice_oracle_across_rates_and_governments() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        let mut city = game.cities[0].clone();
        city.size = 3;
        let all: Vec<_> = Simulation::city_radius(&city)
            .map(|(_, _, x, y)| (x, y))
            .collect();
        let allowed = &all[..6];
        for (index, &(x, y)) in allowed.iter().enumerate() {
            let tile = game.tile_mut(x, y);
            tile.terrain = [
                Terrain::Grassland,
                Terrain::Plains,
                Terrain::Hills,
                Terrain::Forest,
                Terrain::Grassland,
                Terrain::Plains,
            ][index];
            tile.road = index % 2 == 0;
        }
        let occupied: HashSet<_> = all[6..].iter().copied().collect();
        for government in [
            Government::Tribal,
            Government::Republic,
            Government::Communism,
        ] {
            game.players[1].government = government;
            for rates in [[60, 30, 10], [30, 60, 10], [20, 20, 60]] {
                game.players[1].rates = rates;
                city.specialists = [3, 0, 0];
                city.worked_tiles.clear();
                let original = game.city_manager_surplus(&city, &game.city_forecast(&city));
                let grade = |candidate: &City| {
                    let forecast = game.city_forecast(candidate);
                    let output = game.city_manager_surplus(candidate, &forecast);
                    let stage = if !forecast.disorder && output[0] >= 1 && output[1] >= 1 {
                        0
                    } else if !forecast.disorder && output[0] >= 0 && output[1] >= 0 {
                        1
                    } else if !forecast.disorder
                        && [0, 1, 2, 4, 5]
                            .into_iter()
                            .all(|i| output[i] >= original[i].min(0))
                    {
                        2
                    } else {
                        3
                    };
                    let fitness = if stage == 3 {
                        output.iter().map(|&x| i64::from(x)).sum::<i64>()
                            + i64::from(forecast.happy)
                    } else {
                        i64::from(output[0]) * 10
                            + i64::from(output[1]) * 5
                            + i64::from(output[3] + output[5]) * 2
                    };
                    (-(stage as i32), fitness)
                };
                let yields: Vec<_> = allowed
                    .iter()
                    .map(|&(x, y)| game.city_tile_yields(&city, x, y, false, None))
                    .collect();
                let mut best = (i32::MIN, i64::MIN);
                for mask in 0_u32..1 << allowed.len() {
                    let count = mask.count_ones() as i32;
                    if count > city.size {
                        continue;
                    }
                    // Independently apply CM's literal prerequisite rule to
                    // physical subsets, rather than reusing the DP states.
                    let admissible =
                        (0..allowed.len())
                            .filter(|&i| mask & (1 << i) != 0)
                            .all(|i| {
                                let a = yields[i];
                                a != (0, 0, 0)
                                    && (0..allowed.len()).all(|j| {
                                        let b = yields[j];
                                        !(b != a && b.0 >= a.0 && b.1 >= a.1 && b.2 >= a.2)
                                            || mask & (1 << j) != 0
                                    })
                            });
                    if !admissible {
                        continue;
                    }
                    let mut candidate = city.clone();
                    candidate.worked_tiles = allowed
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| mask & (1 << i) != 0)
                        .map(|(_, &tile)| tile)
                        .collect();
                    let specialists = city.size - count;
                    for scientist in 0..=specialists {
                        for entertainer in 0..=specialists - scientist {
                            candidate.specialists = [
                                scientist,
                                specialists - scientist - entertainer,
                                entertainer,
                            ];
                            best = best.max(grade(&candidate));
                        }
                    }
                }
                assert_eq!(
                    grade(&game.city_manager_result(&city, &occupied)),
                    best,
                    "{government:?}, rates={rates:?}"
                );
            }
        }
    }

    #[test]
    fn city_manager_equal_tiles_follow_native_city_index_not_row_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.road = false;
            tile.river = false;
        }
        game.create_city(1, 12, 24);
        let mut city = game.cities[0].clone();
        city.size = 1;
        city.worked_tiles.clear();
        city.specialists = [1, 0, 0];
        let result = game.city_manager_result(&city, &Default::default());
        // city.c:compare_iter_index sorts distance, dx, dy. The first
        // neighboring city index is map-vector(-1,-1) on isohex topology.
        let expected = Simulation::city_radius(&city)
            .filter(|&(_, _, x, y)| game.tile_can_support_worker(&city, x, y))
            .min_by_key(|&(dx, dy, _, _)| (isohex_vector_distance(dx, dy).pow(2), dx, dy))
            .map(|(_, _, x, y)| (x, y))
            .unwrap();
        assert_eq!(result.worked_tiles, vec![expected]);
    }

    #[test]
    fn city_manager_equal_specialist_fitness_uses_native_output_lexicographic_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        game.players[1].government = Government::Communism;
        let mut city = game.cities[0].clone();
        city.size = 3;
        city.specialists = [3, 0, 0];
        city.worked_tiles.clear();
        city.buildings
            .extend([Building::LIBRARY, Building::MARKETPLACE]);
        let occupied = Simulation::city_radius(&city)
            .map(|(_, _, x, y)| (x, y))
            .collect();
        let result = game.city_manager_result(&city, &occupied);
        // Both specialists yield3 after50% bonuses, with estimated fitness6.
        // Equal depth then output order Food,Shield,Trade,Gold,Luxury,Science
        // places taxmen before scientists; DFS retains the first equal leaf.
        assert_eq!(result.specialists, [0, 3, 0]);
    }

    #[test]
    fn native_cm_estimate_truncates_trade_and_weights_luxury_for_happiness() {
        let result = cm_estimated_fitness(
            [1, 2, 3, 0, 0, 0],
            [100, 150, 125, 150, 100, 100],
            [60, 30, 10],
            [10, 5, 0, 2, 0, 2],
        );
        //10food+15shields+2.7gold+3.6science+.3luxury; adjusted trade3.75 truncates3.
        assert!((result - 31.6).abs() < 1e-10);
    }

    #[test]
    fn native_cm_comparator_uses_half_point_tolerance_then_depth_and_output_order() {
        use std::cmp::Ordering::*;
        let tax = [0, 0, 0, 2, 0, 0];
        let science = [0, 0, 0, 0, 0, 2];
        assert_eq!(cm_compare_types((4.0, 0, tax), (4.5, 0, science)), Less);
        assert_eq!(
            cm_compare_types((4.0, 0, tax), (4.5001, 0, science)),
            Greater
        );
        assert_eq!(cm_compare_types((4.0, 2, tax), (4.0, 0, science)), Greater);
    }

    fn specialists_only_lattice() -> CmLattice {
        let production = vec![[0, 0, 0, 0, 0, 2], [0, 0, 0, 2, 0, 0], [0, 0, 0, 0, 2, 0]];
        let by_stat = std::array::from_fn(|output| {
            let mut order = vec![0, 1, 2];
            order.sort_by(|&a, &b| {
                production[b][output]
                    .cmp(&production[a][output])
                    .then_with(|| production[b].cmp(&production[a]))
            });
            order
        });
        CmLattice {
            tiles: vec![],
            groups: vec![],
            production,
            capacity: vec![CM_INFINITY; 3],
            better: vec![vec![]; 3],
            orders: [vec![1, 0, 2], vec![2, 1, 0]],
            by_stat,
        }
    }

    #[test]
    fn native_cm_budget_counts_nonterminal_stack_steps_and_returns_incumbent() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(1, 12, 24);
        game.players[1].government = Government::Communism;
        let mut city = game.cities[0].clone();
        city.size = 3;
        city.worked_tiles.clear();
        city.specialists = [3, 0, 0];
        let lattice = specialists_only_lattice();
        let early = game.city_manager_query(&city, &lattice, 10, [-CM_INFINITY; 6], true, 1);
        assert!(early.aborted);
        assert_eq!(early.iterations, 2);
        assert!(early.city.is_none());
        let complete =
            game.city_manager_query(&city, &lattice, 10, [-CM_INFINITY; 6], true, CM_MAX_LOOP);
        assert!(!complete.aborted);
        assert_eq!(complete.iterations, 19);
        assert_eq!(complete.city.unwrap().specialists, [0, 0, 3]);
        city.size = 255;
        city.specialists = [255, 0, 0];
        let truncated =
            game.city_manager_query(&city, &lattice, 10, [-CM_INFINITY; 6], true, CM_MAX_LOOP);
        assert!(truncated.aborted);
        assert_eq!(truncated.iterations, 27_501);
        assert_eq!(truncated.city.unwrap().specialists.iter().sum::<i32>(), 255);
    }

    #[test]
    fn native_cm_learns_minimum_luxury_from_disorder_and_prunes_later_choices() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.road = false;
            tile.river = false;
        }
        game.create_city(1, 12, 24);
        game.players[1].government = Government::Communism;
        game.players[1].rates = [100, 0, 0];
        let mut city = game.cities[0].clone();
        city.size = 8;
        city.worked_tiles.clear();
        city.specialists = [8, 0, 0];
        let tiles: Vec<_> = Simulation::city_radius(&city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| game.tile_can_support_worker(&city, x, y))
            .take(8)
            .collect();
        let production = vec![
            [2, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 2],
            [0, 0, 0, 2, 0, 0],
            [0, 0, 0, 0, 2, 0],
        ];
        let by_stat = std::array::from_fn(|output| {
            let mut order = vec![0, 1, 2, 3];
            order.sort_by(|&a, &b| {
                production[b][output]
                    .cmp(&production[a][output])
                    .then_with(|| production[b].cmp(&production[a]))
            });
            order
        });
        let lattice = CmLattice {
            tiles,
            groups: vec![(0..8).collect()],
            production,
            capacity: vec![8, CM_INFINITY, CM_INFINITY, CM_INFINITY],
            better: vec![vec![]; 4],
            orders: [vec![0, 2, 1, 3], vec![3, 0, 2, 1]],
            by_stat,
        };
        let result = game.city_manager_query(
            &city,
            &lattice,
            10,
            [0, 0, 0, -CM_INFINITY, 0, 0],
            false,
            CM_MAX_LOOP,
        );
        assert!(result.disorder_prunes > 0);
        assert!(!result.aborted);
    }

    #[test]
    fn native_cm_emergency_sentinel_preserves_local_c_int_production_arithmetic() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        game.players[1].government = Government::Communism;
        let mut city = game.cities[0].clone();
        city.buildings.extend([
            Building::FACTORY,
            Building::MFG_PLANT,
            Building::POWER_PLANT,
        ]);
        let center = game.city_tile_yields(&city, city.x, city.y, true, None);
        let output = game.city_manager_raw_production(&city, [-CM_INFINITY; 6]);
        // Native C -O2 oracle: max(0,(-1000000000+centerShield)*200/100)
        // Signed int multiplication wraps on the bundled target platform.
        let expected = (-CM_INFINITY + center.1).wrapping_mul(200) / 100;
        assert_eq!(output[1], expected.max(0));
    }

    #[test]
    fn city_manager_large_city_preserves_citizen_count_under_native_search_budget() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for (index, tile) in game.tiles.iter_mut().enumerate() {
            tile.terrain = [
                Terrain::Grassland,
                Terrain::Plains,
                Terrain::Hills,
                Terrain::Forest,
            ][index % 4];
            tile.resource = None;
            tile.road = index % 3 == 0;
            tile.river = index % 5 == 0;
        }
        game.create_city(1, 12, 24);
        let mut city = game.cities[0].clone();
        city.size = 30;
        city.specialists = [30, 0, 0];
        city.worked_tiles.clear();
        let start = std::time::Instant::now();
        let arranged = game.city_manager_result(&city, &Default::default());
        eprintln!("CM size30 elapsed {:?}", start.elapsed());
        assert_eq!(
            arranged.worked_tiles.len() as i32 + arranged.specialists.iter().sum::<i32>(),
            30
        );
    }
}
