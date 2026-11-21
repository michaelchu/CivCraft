//! Implements targeting behavior within the offense subsystem.

use super::*;

impl super::Simulation {
    pub(in crate::sim) fn offensive_beachhead(
        &self,
        map: &mut NormalPaths,
        cargo: UnitKind,
        owner: usize,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32, (i32, i32))> {
        self.offensive_beachhead_within(map, cargo, owner, x, y, None)
    }

    fn offensive_beachhead_within(
        &self,
        map: &mut NormalPaths,
        cargo: UnitKind,
        owner: usize,
        x: i32,
        y: i32,
        max_cost: Option<i32>,
    ) -> Option<(i32, i32, (i32, i32))> {
        let mut best = None;
        let mut best_cost = max_cost.unwrap_or(i32::MAX);
        if cargo.has_marine_attack() {
            let beach = (x, y);
            for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                let cost = if best_cost < i32::MAX {
                    map.movement_cost_within(self, nx, ny, best_cost.saturating_sub(1))
                } else {
                    map.movement_cost(self, nx, ny)
                };
                if let Some(cost) = cost
                    && cost < best_cost
                {
                    best_cost = cost;
                    best = Some((nx, ny, beach));
                }
            }
        } else {
            let mut checked = [(x, y); 32];
            let mut checked_len = 1_usize;
            for (bx, by) in self.map_wrap.civ3_neighbors(x, y) {
                if !self.pf_native_tile(cargo, bx, by) {
                    continue;
                }
                for (sx, sy) in self.map_wrap.civ3_neighbors(bx, by) {
                    if checked[..checked_len].contains(&(sx, sy)) {
                        continue;
                    }
                    if checked_len < checked.len() {
                        checked[checked_len] = (sx, sy);
                        checked_len += 1;
                    }
                    if self.any_unit_at(sx, sy, |unit| !self.allied(owner, unit.owner)) {
                        continue;
                    }
                    let cost = if best_cost < i32::MAX {
                        map.movement_cost_within(self, sx, sy, best_cost.saturating_sub(1))
                    } else {
                        map.movement_cost(self, sx, sy)
                    };
                    if let Some(cost) = cost
                        && cost < best_cost
                    {
                        best_cost = cost;
                        best = Some((sx, sy, (bx, by)));
                    }
                }
            }
        }
        best
    }

    pub(super) fn offensive_find_ferry(&self, passenger: &Unit) -> Option<Unit> {
        // Fast-exit when the owner has no eligible transport with free capacity.
        if !self.units.iter().any(|boat| {
            boat.owner == passenger.owner
                && self.ferry_can_take_unit(boat, passenger)
                && self.cargo_count(boat.id) < boat.kind.transport_capacity()
                && boat.kind.fuel_capacity() == 0
                && boat.kind.hp_loss_percent() == 0
        }) {
            return None;
        }
        // aiferry_find_boat's combined PF and sea_move EC. The source
        // intentionally cannot cross sea back onto a second continent.
        let mut map = NormalPaths::ferry_search_map(self, passenger);
        let mut best_turns = i32::MAX;
        let mut best = None;
        while let Some((x, y, turn, extra)) = map.next_position(self) {
            if turn + (extra / 65536) as i32 > best_turns {
                break;
            }
            let tiles: Vec<_> = if self.tile(x, y).terrain.is_water() {
                std::iter::once((x, y))
                    .chain(self.map_wrap.civ3_neighbors(x, y))
                    .collect()
            } else {
                vec![(x, y)]
            };
            for (x, y) in tiles {
                for boat in self.tile_units_ordered(x, y) {
                    if !self.ferry_can_take_unit(boat, passenger)
                        || self.cargo_count(boat.id) >= boat.kind.transport_capacity()
                        || boat.kind.fuel_capacity() > 0
                        || boat.kind.hp_loss_percent() > 0
                    {
                        continue;
                    }
                    let ferry_turns = (((extra / 65536) as i32 * 16) - boat.moves_left)
                        / (boat.kind.stats().moves * campaign_move_fragments());
                    let turns = turn.max(ferry_turns);
                    if turns < best_turns {
                        best_turns = turns;
                        best = Some(boat.clone());
                    }
                }
            }
        }
        best
    }

    fn offensive_ferry_candidate(&self, seed: &Unit) -> Option<Unit> {
        self.offensive_find_ferry(seed).or_else(|| {
            UnitKind::ALL_NATIVE
                .into_iter()
                .rev()
                .find(|kind| {
                    kind.has_ai_strategy(UnitAiStrategy::NavalTransport)
                        && self.unit_available(seed.owner, *kind)
                })
                .or_else(|| {
                    UnitKind::ALL_NATIVE
                        .into_iter()
                        .find(|kind| kind.has_ai_strategy(UnitAiStrategy::NavalTransport))
                })
                .filter(|_| {
                    self.map_wrap
                        .civ3_neighbors(seed.x, seed.y)
                        .any(|(x, y)| self.tile(x, y).terrain.is_water())
                })
                .map(|kind| self.make_unit(0, seed.owner, kind, seed.x, seed.y))
        })
    }

    #[cfg(test)]
    pub(super) fn offensive_target(&self, city: &City, seed: &Unit) -> Option<OffensiveTarget> {
        self.offensive_target_with_assignments(
            self.cached_offensive_output(city),
            seed,
            &self.assigned_attack_strength(seed),
        )
    }
    pub(in crate::sim) fn offensive_target_with_assignments(
        &self,
        output: i32,
        seed: &Unit,
        assigned: &std::collections::HashMap<u32, (i32, i32)>,
    ) -> Option<OffensiveTarget> {
        self.offensive_target_plan(output, seed, assigned)
            .map(|plan| plan.target)
    }

    pub(super) fn offensive_target_plan(
        &self,
        output: i32,
        seed: &Unit,
        assigned: &std::collections::HashMap<u32, (i32, i32)>,
    ) -> Option<OffensivePlan> {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("offense.target_search");
        self.trace.count_operation("offensive.target_searches", 1);
        let virtual_unit = seed.id == 0;
        let rating = Self::offensive_attack_rating(seed);
        let hostile = |owner| {
            if virtual_unit {
                self.ai_potentially_hostile(seed.owner, owner)
            } else {
                self.at_war(seed.owner, owner)
            }
        };
        if rating == 0 || !(0..self.players.len()).any(hostile) {
            return None;
        }
        let mut map = NormalPaths::attack_map(self, seed);
        let connectivity = self.movement_connectivity(seed.owner);
        let cached;
        let live;
        let invasion_support = if virtual_unit {
            cached = self.offensive_invasion_support(seed.owner);
            cached.as_ref()
        } else {
            let target_cities: Vec<_> = (0..self.players.len())
                .filter(|&owner| hostile(owner))
                .flat_map(|owner| self.player_cities_ordered(owner))
                .collect();
            live = self.invasion_support_many(seed.owner, &target_cities, seed.id);
            &live
        };
        let mut best: Option<OffensiveTarget> = None;
        let mut fallback = 0.0_f32;
        let mut ferry_initialized = seed.kind.is_naval();
        let mut ferry = None;
        let mut ferry_map = None;
        let mut deferred_over_horizon: Vec<&City> = Vec::new();
        for pass in 0..2 {
            if pass == 1
                && (!virtual_unit
                    || best.as_ref().is_some_and(|target| target.want > 0.0)
                    || deferred_over_horizon.is_empty())
            {
                break;
            }
            let bounded = pass == 0;
            let pass_cities: Vec<&City> = if bounded {
                (0..self.players.len())
                    .filter(|&owner| hostile(owner))
                    .flat_map(|owner| self.player_cities_ordered(owner))
                    .filter(|enemy| {
                        self.tile(enemy.x, enemy.y).known_by[seed.owner]
                            && self.offensive_can_attack(seed.kind, enemy.x, enemy.y)
                    })
                    .collect()
            } else {
                std::mem::take(&mut deferred_over_horizon)
            };
            for enemy in pass_cities {
                let mut by_boat = false;
                let direct_possible = connectivity.can_directly_reach_tile(
                    self,
                    seed.kind,
                    (seed.x, seed.y),
                    (enemy.x, enemy.y),
                    false,
                );
                let mut direct = if !direct_possible {
                    None
                } else if bounded {
                    map.position_turn_within(self, enemy.x, enemy.y, 10)
                } else {
                    map.position_turn(self, enemy.x, enemy.y)
                };
                if direct.is_none() && !ferry_initialized {
                    ferry = if virtual_unit {
                        self.offensive_ferry_candidate(seed)
                    } else {
                        self.offensive_find_ferry(seed)
                    };
                    ferry_map = ferry
                        .as_ref()
                        .map(|boat| NormalPaths::overlap_map(self, boat));
                    ferry_initialized = true;
                }
                if direct.is_none() && direct_possible && bounded && ferry_map.is_some() {
                    // Only use a ferry when the enemy city is truly unreachable
                    // by land, not merely farther than 10 turns on the same landmass.
                    direct = map.position_turn(self, enemy.x, enemy.y);
                }
                let turns = if let Some(turns) = direct {
                    turns
                } else if let Some(fmap) = ferry_map.as_mut() {
                    let ferry_max_cost = bounded.then(|| {
                        let boat_rate = ferry
                            .as_ref()
                            .map_or(campaign_move_fragments(), |boat| self.unit_move_rate(boat));
                        11_i32.saturating_mul(boat_rate)
                    });
                    let Some((x, y, beach)) = self.offensive_beachhead_within(
                        fmap,
                        seed.kind,
                        seed.owner,
                        enemy.x,
                        enemy.y,
                        ferry_max_cost,
                    ) else {
                        if bounded && virtual_unit {
                            deferred_over_horizon.push(enemy);
                        }
                        continue;
                    };
                    by_boat = true;
                    let mut turns =
                        fmap.position_turn(self, x, y).unwrap() + i32::from((x, y) != beach);
                    if let Some(boat) = ferry.as_ref()
                        && boat.id != 0
                        && (boat.x, boat.y) != (seed.x, seed.y)
                    {
                        let walk = if bounded {
                            map.position_turn_within(self, boat.x, boat.y, (10 - turns).max(0))
                        } else {
                            map.position_turn(self, boat.x, boat.y)
                        };
                        let Some(walk) = walk else {
                            if bounded && virtual_unit {
                                deferred_over_horizon.push(enemy);
                            }
                            continue;
                        };
                        turns += walk;
                    }
                    turns
                } else {
                    if bounded && virtual_unit {
                        deferred_over_horizon.push(enemy);
                    }
                    continue;
                };
                // Beyond the attack horizon a city can only serve as the
                // production advisor's fallback until it finds a positive target.
                if turns > 10 {
                    if bounded && virtual_unit {
                        deferred_over_horizon.push(enemy);
                        continue;
                    }
                    if !virtual_unit || best.as_ref().is_some_and(|target| target.want > 0.0) {
                        continue;
                    }
                }
                let defender = self.anticipated_city_defender(enemy, seed, turns, virtual_unit);
                let vulnerability = defender.as_ref().map_or(0, |d| {
                    self.offensive_defense_squared(d, seed.kind, enemy.x, enemy.y, virtual_unit)
                });
                let mut benefit = defender
                    .as_ref()
                    .map_or(0.0, |d| d.kind.stats().cost as f32);
                let (helper, helper_cost) = self.offensive_helpers(seed, enemy, assigned);
                let (invaders, occupiers) =
                    invasion_support.get(&enemy.id).copied().unwrap_or_default();
                let victims = self.tile_units_ordered(enemy.x, enemy.y).count() as i32;
                let reserves = invaders - victims
                    + if !virtual_unit {
                        0
                    } else if seed.kind.has_flag(UnitFlag::OneAttack) {
                        1
                    } else {
                        seed.kind.stats().moves * campaign_move_fragments()
                    };
                if reserves > 0 && (seed.kind.can_occupy_city() || occupiers > 0) {
                    benefit += (self.advisor_city_output(enemy, None) * reserves / 5) as f32;
                }
                let power = (rating + helper).pow(2);
                let mut want = if !seed.kind.can_occupy_city() && defender.is_none() || turns > 10 {
                    0.0
                } else if seed.kind.can_occupy_city()
                    && occupiers == 0
                    && (invaders > 0 || victims == 0)
                {
                    (seed.kind.stats().cost * 17) as f32
                } else {
                    kill_desire(
                        benefit,
                        power,
                        seed.kind.stats().cost + helper_cost,
                        vulnerability,
                        victims + 1,
                    )
                };
                want -= (turns * 17) as f32;
                let new_ferry = by_boat && ferry.as_ref().is_some_and(|f| f.id == 0);
                let cost = Self::offensive_balanced_cost(seed.kind)
                    + if new_ferry {
                        ferry.as_ref().unwrap().kind.stats().cost
                    } else {
                        0
                    };
                want = Self::amortize_attack(want, turns.max(1), cost, output);
                let choose_fallback = virtual_unit
                    && want <= 0.0
                    && best.as_ref().is_none_or(|old| old.want <= 0.0)
                    && {
                        let bk = Self::amortize_attack(benefit * 17.0, turns.max(1), cost, output);
                        if bk > fallback {
                            fallback = bk;
                            true
                        } else {
                            false
                        }
                    };
                if choose_fallback || want > best.as_ref().map_or(0.0, |old| old.want) {
                    best = Some(OffensiveTarget {
                        path: Vec::new(),
                        x: enemy.x,
                        y: enemy.y,
                        city: Some(enemy.id),
                        turns,
                        want,
                        ferry: by_boat.then(|| ferry.clone().unwrap()),
                        new_ferry,
                    });
                }
            }
            if bounded {
                for owner in 0..self.players.len() {
                    if !hostile(owner) {
                        continue;
                    }
                    for enemy in self.player_units_ordered(owner) {
                        if !self.tile(enemy.x, enemy.y).known_by[seed.owner]
                            || self.any_city_at_cached(enemy.x, enemy.y)
                            || virtual_unit && !enemy.kind.has_hostile_action()
                            || !self.offensive_can_attack(seed.kind, enemy.x, enemy.y)
                            || !connectivity.can_directly_reach_tile(
                                self,
                                seed.kind,
                                (seed.x, seed.y),
                                (enemy.x, enemy.y),
                                true,
                            )
                            || self.best_defender_id(seed, enemy.x, enemy.y) != Some(enemy.id)
                        {
                            continue;
                        }
                        let Some(turns) = map.position_turn_within(self, enemy.x, enemy.y, 10)
                        else {
                            continue;
                        };
                        if turns > 10 {
                            continue;
                        }
                        let vulnerability = self.offensive_defense_squared(
                            enemy,
                            seed.kind,
                            enemy.x,
                            enemy.y,
                            virtual_unit,
                        );
                        let want = kill_desire(
                            enemy.kind.stats().cost as f32,
                            rating * rating,
                            seed.kind.stats().cost,
                            vulnerability,
                            1,
                        ) - (turns * 17) as f32;
                        let want = Self::amortize_attack(
                            want,
                            turns.max(1),
                            Self::offensive_balanced_cost(seed.kind),
                            output,
                        );
                        if want > best.as_ref().map_or(0.0, |old| old.want) {
                            best = Some(OffensiveTarget {
                                path: Vec::new(),
                                x: enemy.x,
                                y: enemy.y,
                                city: None,
                                turns,
                                want,
                                ferry: None,
                                new_ferry: false,
                            });
                        }
                    }
                }
            }
        }
        if !virtual_unit && let Some(target) = best.as_mut() {
            let goal = target
                .ferry
                .as_ref()
                .map_or((target.x, target.y), |boat| (boat.x, boat.y));
            target.path = map.path(self, goal.0, goal.1)?.0;
        }
        best.map(|target| {
            let invasion = target
                .city
                .and_then(|id| invasion_support.get(&id).copied())
                .unwrap_or_default();
            // Freeciv's pferrymap output preserves the search for production advice.
            let ferry_map = ferry_map.filter(|_| virtual_unit && target.ferry.is_some());
            OffensivePlan {
                target,
                ferry_map,
                invasion,
            }
        })
    }

    pub(super) fn offensive_victim_count(&self, x: i32, y: i32) -> i32 {
        if self.stack_vulnerable(x, y) {
            1
        } else {
            1 + self.tile_units_ordered(x, y).count() as i32
        }
    }

    pub(super) fn offensive_victim(
        &self,
        seed: &Unit,
        target: &OffensiveTarget,
    ) -> (Option<Unit>, f32) {
        if let Some(city) = target.city.and_then(|id| self.city_by_id(id)) {
            // kill_something_with deliberately retains def_type on the
            // <=1-turn branch while clearing benefit and def_vet. The later
            // type loop therefore still rates an unbuilt rank-zero defender.
            let mut defender = self.defender_versus(city, seed);
            if target.turns <= 1
                && let Some(unit) = defender.as_mut()
            {
                unit.veteran = 0;
                unit.hp = unit.max_hit_points();
            }
            let vulnerability = if target.turns > 1 {
                defender.as_ref().map_or(0, |d| {
                    self.offensive_defense_squared(d, seed.kind, target.x, target.y, true)
                })
            } else {
                0
            };
            let mut benefit = if target.turns > 1 {
                defender
                    .as_ref()
                    .map_or(0.0, |d| d.kind.stats().cost as f32)
            } else {
                0.0
            };
            if let Some(current) = self
                .best_defender_id(seed, target.x, target.y)
                .and_then(|id| self.unit_by_id(id))
            {
                let mut current = current.clone();
                current.hp = current.max_hit_points();
                current.state = UnitState::Idle;
                let power =
                    self.offensive_defense_squared(&current, seed.kind, target.x, target.y, true);
                if power > vulnerability {
                    benefit = current.kind.stats().cost as f32;
                    defender = Some(current);
                }
            }
            (defender, benefit)
        } else {
            let defender = self
                .best_defender_id(seed, target.x, target.y)
                .and_then(|id| self.unit_by_id(id))
                .map(|unit| {
                    let mut unit = unit.clone();
                    unit.hp = unit.max_hit_points();
                    unit.state = UnitState::Idle;
                    unit
                });
            let benefit = defender
                .as_ref()
                .map_or(0.0, |d| d.kind.stats().cost as f32);
            (defender, benefit)
        }
    }

    #[cfg(test)]
    pub(super) fn offensive_candidate_turns(
        &self,
        candidate: &Unit,
        target: &OffensiveTarget,
        ferry_map: Option<&mut NormalPaths>,
    ) -> Option<i32> {
        let connectivity = self.movement_connectivity(candidate.owner);
        self.offensive_candidate_turns_with_connectivity(
            candidate,
            target,
            ferry_map,
            &connectivity,
        )
    }

    pub(super) fn offensive_candidate_turns_with_connectivity(
        &self,
        candidate: &Unit,
        target: &OffensiveTarget,
        ferry_map: Option<&mut NormalPaths>,
        connectivity: &crate::sim::ai::MovementConnectivity,
    ) -> Option<i32> {
        let _profile = self.trace.profiler().scope("offense.candidate_route");
        self.trace.count_operation("offensive.candidate_routes", 1);
        let direct_possible = connectivity.can_directly_reach_tile(
            self,
            candidate.kind,
            (candidate.x, candidate.y),
            (target.x, target.y),
            target.city.is_none(),
        );
        if (target.ferry.is_none() || ferry_map.is_none()) && !direct_possible {
            return None;
        }
        let _map_profile = self
            .trace
            .profiler()
            .scope("offense.candidate_route.map_build");
        let mut pf = NormalPaths::enemy_map(self, candidate);
        drop(_map_profile);
        let mut turns = None;
        if let (Some(boat), Some(fmap)) = (target.ferry.as_ref(), ferry_map) {
            let beachhead = {
                let _profile = self
                    .trace
                    .profiler()
                    .scope("offense.candidate_route.beachhead");
                self.offensive_beachhead(fmap, candidate.kind, candidate.owner, target.x, target.y)
            };
            if let Some((x, y, _)) = beachhead {
                {
                    let _profile = self
                        .trace
                        .profiler()
                        .scope("offense.candidate_route.ferry_sea_reach");
                    turns = fmap.position_turn(self, x, y);
                }
                let first = {
                    let _profile = self
                        .trace
                        .profiler()
                        .scope("offense.candidate_route.ferry_land_scan");
                    let mut first = None;
                    while let Some(position) = pf.next_position(self) {
                        if (position.0, position.1) == (boat.x, boat.y)
                            || (position.0, position.1) == (target.x, target.y)
                        {
                            first = Some(position);
                            break;
                        }
                    }
                    first
                };
                if let Some((x, y, travel, _)) = first {
                    if (x, y) == (target.x, target.y) {
                        turns = Some(travel);
                    } else {
                        turns = turns.map(|sea| sea + travel);
                    }
                }
            }
        }
        if let Some(turns) = turns {
            Some(turns)
        } else if !direct_possible {
            None
        } else {
            let _profile = self
                .trace
                .profiler()
                .scope("offense.candidate_route.target_reach");
            pf.position_turn_astar(self, target.x, target.y)
        }
    }
}
