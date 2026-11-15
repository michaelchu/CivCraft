//! Implements ground invasion behavior within the ai subsystem.

use super::*;

impl Simulation {
    pub(crate) fn ai_ground_invasion_playtest_setup(&mut self) -> (u32, u32, u32) {
        self.units.clear();
        self.cities.clear();
        self.ai_ground_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.hut = false;
            tile.known_by[1] = true;
        }
        self.sign_pact(1, HUMAN, Pact::War);
        let home = (10, 24);
        let front = offset_native_by_map(home.0, home.1, 6, 0);
        self.players[1]
            .known
            .extend([Tech::AMPHIBIOUS_WARFARE, Tech::REPLACEABLE_PARTS]);
        self.tile_mut(home.0, home.1).terrain = Terrain::Forest;
        self.tile_mut(home.0, home.1).resource = Some(Resource::Rubber);
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, front.0, front.1);
        let city = self.cities.last().unwrap().id;
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        // daiair's strict MC < currentMP target bound excludes OneAttack
        // bombers, whose native PF attack endpoint consumes a whole turn.
        // A Fighter exercises the actual supported combined-air mission.
        let bomber = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let departure = offset_native_by_map(home.0, home.1, 1, 0);
        let infantry = self.spawn_unit(1, UnitKind::Marine, departure.0, departure.1);
        self.spawn_unit(HUMAN, UnitKind::Warrior, front.0, front.1);
        self.units.last_mut().unwrap().hp = 2;
        self.set_random_seed(1);
        self.refresh_ground_invasions(1, &Default::default());
        (infantry, bomber, city)
    }

    // daiunit.c:single_invader/invasion_funct. A real GOTO marks its
    // destination even when distant; mere proximity marks undefended cities.
    pub(in crate::sim) fn invasion_support(
        &self,
        owner: usize,
        city: &City,
        except: u32,
    ) -> (i32, i32) {
        self.invasion_support_many(owner, &[city], except)
            .get(&city.id)
            .copied()
            .unwrap_or_default()
    }

    /// Calculate invasion markers for a set of cities while scanning the
    /// army once, matching Freeciv's per-attacker initialization pass.
    pub(in crate::sim) fn invasion_support_many(
        &self,
        owner: usize,
        target_cities: &[&City],
        except: u32,
    ) -> std::collections::BTreeMap<u32, (i32, i32)> {
        let targets: Vec<_> = target_cities
            .iter()
            .copied()
            .filter(|city| self.at_war(owner, city.owner))
            .collect();
        if targets.is_empty() {
            return std::collections::BTreeMap::new();
        }
        let mut result = vec![(0_i32, 0_i32); targets.len()];
        let mut undefended = Vec::with_capacity(targets.len());
        let mut city_at = vec![None; self.tiles.len()];
        let target_index: std::collections::HashMap<_, _> = targets
            .iter()
            .enumerate()
            .map(|(index, city)| (city.id, index))
            .collect();
        for (index, city) in targets.iter().enumerate() {
            city_at[self.map_wrap.index(city.x, city.y)] = Some(index);
            undefended.push(!self.any_unit_at(city.x, city.y, |u| {
                u.kind.stats().defense > 0 && !self.allied(owner, u.owner)
            }));
        }
        let mut cargo_support = std::collections::BTreeMap::<u32, (i32, i32)>::new();
        for cargo in self.units.iter().filter(|unit| unit.kind.is_military()) {
            let Some(transport) = cargo.transport else {
                continue;
            };
            let support = cargo_support.entry(transport).or_default();
            support.0 += cargo.kind.stats().moves * MOVE_FRAGMENTS;
            support.1 += i32::from(cargo.kind.can_occupy_city());
        }
        self.trace
            .count_operation("ai.invasion_support.batch_calls", 1);
        self.trace.count_operation(
            "ai.invasion_support.units_scanned",
            (self.units.len() * 3) as u64,
        );
        let add_actor_marker =
            |result: &mut [(i32, i32)], index: usize, marks: i32, actor: &Unit| {
                result[index].0 += marks
                    * if actor.kind.bombard_rate() > 0 {
                        1
                    } else {
                        actor.kind.stats().moves * MOVE_FRAGMENTS
                    };
                result[index].1 += marks * i32::from(actor.kind.can_occupy_city());
                if let Some(&(cargo_attacks, cargo_occupiers)) = cargo_support.get(&actor.id) {
                    result[index].0 += marks * cargo_attacks;
                    result[index].1 += marks * cargo_occupiers;
                }
            };
        for actor in self.units.iter().filter(|u| {
            u.owner == owner && u.id != except && u.kind.is_military() && u.transport.is_none()
        }) {
            if let Some(&city_id) = self.ai_ground_targets.get(&actor.id)
                && let Some(&index) = target_index.get(&city_id)
            {
                add_actor_marker(&mut result, index, 1, actor);
            }
            let radius = self.unit_move_rate(actor) / MOVE_FRAGMENTS;
            for (x, y, _) in self.map_wrap.civ3_radius(actor.x, actor.y, radius) {
                if let Some(index) = city_at[self.map_wrap.index(x, y)]
                    && undefended[index]
                {
                    add_actor_marker(&mut result, index, 1, actor);
                }
            }
        }
        // Nonattacking ferries still carry occupation-capable reinforcements.
        for boat in self.units.iter().filter(|u| {
            u.owner == owner && u.id != except && !u.kind.is_military() && u.kind.is_naval()
        }) {
            let Some(&(cargo_attacks, cargo_occupiers)) = cargo_support.get(&boat.id) else {
                continue;
            };
            if let Some(&city_id) = self.ai_ground_targets.get(&boat.id)
                && let Some(&index) = target_index.get(&city_id)
            {
                result[index].0 += cargo_attacks;
                result[index].1 += cargo_occupiers;
            }
            for (x, y, _) in self.map_wrap.civ3_radius(boat.x, boat.y, 2) {
                if let Some(index) = city_at[self.map_wrap.index(x, y)]
                    && undefended[index]
                {
                    result[index].0 += cargo_attacks;
                    result[index].1 += cargo_occupiers;
                }
            }
        }
        targets
            .into_iter()
            .zip(result)
            .map(|(city, support)| (city.id, support))
            .collect()
    }

    pub(super) fn refresh_ground_invasions(
        &mut self,
        owner: usize,
        reserved: &std::collections::HashSet<u32>,
    ) {
        let valid: std::collections::HashSet<_> = self
            .ai_ground_targets
            .iter()
            .filter_map(|(&id, &target)| {
                let actor = self.unit_by_id(id)?;
                let city = self.city_by_id(target)?;
                let invader =
                    actor.kind.is_military() && !actor.kind.is_aircraft() && !actor.kind.is_naval();
                let ferry = actor.kind.is_naval()
                    && self
                        .units
                        .iter()
                        .any(|cargo| cargo.transport == Some(id) && cargo.kind.can_occupy_city());
                (self.at_war(actor.owner, city.owner) && (invader || ferry)).then_some(id)
            })
            .collect();
        self.ai_ground_targets.retain(|id, _| valid.contains(id));
        let boats: Vec<_> =
            self.units
                .iter()
                .filter(|u| {
                    u.owner == owner
                        && u.kind.is_naval()
                        && self.units.iter().any(|cargo| {
                            cargo.transport == Some(u.id) && cargo.kind.can_occupy_city()
                        })
                })
                .cloned()
                .collect();
        for boat in boats {
            let troops: Vec<_> = self
                .units
                .iter()
                .filter(|u| u.transport == Some(boat.id) && u.kind.is_military())
                .collect();
            let target = self
                .raider_landing(&boat, None, Some(&troops), &[])
                .and_then(|(_, x, y, (dx, dy))| {
                    let shore = self.map_wrap.civ3_offset(x, y, dx, dy)?;
                    self.cities
                        .iter()
                        .filter(|c| {
                            self.at_war(owner, c.owner)
                                && self.map_wrap.civ3_distance(shore.0, shore.1, c.x, c.y) <= 1
                                && self.assault_capture_chance(c, &troops) >= 0.5
                        })
                        .min_by_key(|c| c.id)
                        .map(|c| c.id)
                });
            if let Some(target) = target {
                self.ai_ground_targets.insert(boat.id, target);
            } else {
                self.ai_ground_targets.remove(&boat.id);
            }
        }
        let actors: Vec<_> = self
            .units
            .iter()
            .filter(|u| {
                u.owner == owner
                    && u.kind.is_military()
                    && !u.kind.is_aircraft()
                    && !u.kind.is_naval()
                    && u.transport.is_none()
            })
            .cloned()
            .collect();
        for unit in actors {
            let guarding = self.cities.iter().any(|c| {
                c.owner == owner
                    && (c.x, c.y) == (unit.x, unit.y)
                    && (self
                        .units
                        .iter()
                        .filter(|u| {
                            u.owner == owner && u.kind.is_military() && (u.x, u.y) == (c.x, c.y)
                        })
                        .count()
                        <= 1
                        || self.city_threats(c).next().is_some())
            });
            if reserved.contains(&unit.id) || guarding {
                self.ai_ground_targets.remove(&unit.id);
                continue;
            }
            if let Some(target) = self.ground_city_attack_choice(&unit) {
                self.ai_ground_targets.insert(unit.id, target);
            } else {
                self.ai_ground_targets.remove(&unit.id);
            }
        }
    }

    pub(in crate::sim) fn ground_attack_is_my_turn(&self, actor: &Unit, defender: &Unit) -> bool {
        // daiunit.c:is_my_turn: strongest ready attacker against this defender
        // strikes first, so a weak unit cannot sacrifice the entire stack.
        let rating = |unit: &Unit| {
            Self::attack_power(unit) * unit.hp * unit.kind.stats().firepower / (POWER_FACTOR * 3)
        };
        let own = rating(actor);
        let own_defense = self.defense_power_against(defender, defender.x, defender.y, actor.kind);
        for other in self.units.iter().filter(|u| {
            u.id != actor.id
                && u.owner == actor.owner
                && u.moves_left > 0
                && u.kind.is_military()
                && !u.kind.is_aircraft()
                && !u.kind.is_naval()
                && (u.transport.is_none() || u.kind.has_marine_attack())
                && self
                    .map_wrap
                    .civ3_distance(u.x, u.y, defender.x, defender.y)
                    <= 1
                && !self.treaty_blocks_move(u, defender.x, defender.y)
                && self.best_defender_id(u, defender.x, defender.y).is_some()
        }) {
            let defense = self.defense_power_against(defender, defender.x, defender.y, other.kind);
            if defense == 0 {
                return true;
            }
            if rating(other) * own_defense / defense > own {
                return false;
            }
        }
        true
    }

    pub(super) fn follow_ground_invasion_path(&mut self, id: u32, path: Vec<(i32, i32)>) {
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("ai.ground_invasion.follow_path");
        let Some(actor) = self.unit_by_id(id).cloned() else {
            return;
        };
        let goal = path.iter().fold((actor.x, actor.y), |(x, y), &(dx, dy)| {
            self.map_wrap
                .civ3_offset(x, y, dx, dy)
                .expect("planned step is real")
        });
        if goal == (actor.x, actor.y) {
            return;
        }
        let _profile = profiler.scope("ai.ground_invasion.destination_guard");
        self.ai_guard_for_destination(id, goal.0, goal.1);
        drop(_profile);
        self.unit_goto_tiles.insert(id, goal);
        let Some((path, _)) = self.ai_goto_path(&actor, goal, GotoTask::Attack) else {
            return;
        };
        let length = path.len();
        let _profile = profiler.scope("ai.ground_invasion.step_execution");
        for (position, (dx, dy)) in path.into_iter().enumerate() {
            let Some(actor) = self.unit_by_id(id).cloned() else {
                return;
            };
            let Some(target) = self.map_wrap.civ3_offset(actor.x, actor.y, dx, dy) else {
                break;
            };
            let enemy =
                self.units
                    .iter()
                    .any(|u| (u.x, u.y) == target && self.at_war(actor.owner, u.owner))
                    || self.cities.iter().any(|city| {
                        (city.x, city.y) == target && self.at_war(actor.owner, city.owner)
                    });
            if actor.moves_left <= 0 || enemy && position + 1 != length {
                return;
            }
            // adv_follow_path postpones a peaceful end-of-turn step into
            // danger. Keep this beside the specialized attacker ordering
            // below; routing through the generic executor would lose it.
            let cost = self.raw_move_cost_for(&actor, target.0, target.1);
            if !enemy
                && actor.moves_left <= cost
                && self.unit_move_rate(&actor) > cost
                && self.explorer_danger_at(&actor, target.0, target.1)
                && !self.explorer_danger_at(&actor, actor.x, actor.y)
            {
                return;
            }
            if self
                .best_defender_id(&actor, target.0, target.1)
                .and_then(|id| self.unit_by_id(id))
                .is_some_and(|defender| !self.ground_attack_is_my_turn(&actor, defender))
            {
                return;
            }
            let result = CIV3_DIRS
                .iter()
                .position(|&step| step == (dx, dy))
                .map_or(MoveResult::Blocked, |direction| {
                    self.try_move_unit_civ3(id, direction)
                });
            if result == MoveResult::Blocked
                || result == MoveResult::Fought
                || self
                    .units
                    .iter()
                    .find(|u| u.id == id)
                    .is_none_or(|u| u.moves_left <= 0)
            {
                return;
            }
        }
    }

    pub(super) fn ground_city_attack_choice(&self, unit: &Unit) -> Option<u32> {
        let mut best = None;
        let mut best_value = 0.0;
        let production: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == unit.owner)
            .map(|city| self.city_forecast(city).shield_surplus())
            .collect();
        let average = production.iter().sum::<i32>() / production.len().max(1) as i32;
        let output = self
            .cities
            .iter()
            .find(|city| Some(city.id) == unit.home_city)
            .map_or(1, |city| self.city_forecast(city).shield_surplus())
            .max(average)
            .max(1);
        let rating = |actor: &Unit| {
            Self::attack_power(actor) * actor.hp * actor.kind.stats().firepower / (POWER_FACTOR * 3)
        };
        // Freeciv initializes all city invasion markers and builds one PF map
        // per unit, then asks it for each enemy city.
        let targets: Vec<_> = self
            .cities
            .iter()
            .filter(|city| {
                self.at_war(unit.owner, city.owner)
                    && self.tile(city.x, city.y).known_by[unit.owner]
            })
            .collect();
        let invasion = self.invasion_support_many(unit.owner, &targets, unit.id);
        let destinations: Vec<_> = targets.iter().map(|city| (city.x, city.y)).collect();
        let route_costs = self.route_costs_to_many(unit, &destinations);
        for (city, route_cost) in targets.into_iter().zip(route_costs) {
            let Some(cost) = route_cost else {
                continue;
            };
            if cost == 0 {
                continue;
            }
            let defenders = self
                .units
                .iter()
                .filter(|defender| {
                    (defender.x, defender.y) == (city.x, city.y)
                        && !self.allied(unit.owner, defender.owner)
                })
                .count();
            let (invasion_attacks, invasion_occupiers) =
                invasion.get(&city.id).copied().unwrap_or_default();
            let allowance = self.unit_move_rate(unit).max(1);
            let turns = (cost + allowance - unit.moves_left.max(0)) / allowance;
            if turns > 10 {
                continue;
            }
            let (helper_attack, helper_cost) = self.offensive_helpers(unit, city);
            let attack = rating(unit) + helper_attack;
            let attack = f64::from(attack).powi(2);
            let loss = unit.kind.stats().cost + helper_cost;
            let anticipated = self.anticipated_city_defender(city, unit, turns, false);
            let defender = anticipated.as_ref();
            let mut benefit = defender.map_or(0.0, |d| f64::from(d.kind.stats().cost));
            let reserves = invasion_attacks - defenders as i32;
            if reserves > 0 && (unit.kind.can_occupy_city() || invasion_occupiers > 0) {
                benefit += f64::from(self.advisor_city_output(city, None).max(0) * reserves) / 5.0;
            }
            let vulnerability = defender.map_or(0.0, |d| {
                let rating = self.defense_power_against(d, city.x, city.y, unit.kind)
                    * d.hp
                    * d.kind.stats().firepower
                    / (POWER_FACTOR * 3);
                f64::from(rating).powi(2)
            });
            let victims = defenders as f64 + 1.0;
            let gain = if unit.kind.can_occupy_city()
                && invasion_occupiers == 0
                && (invasion_attacks > 0 || defenders == 0)
            {
                f64::from(unit.kind.stats().cost * 17)
            } else if !unit.kind.can_occupy_city() && defender.is_none() {
                0.0
            } else if attack + vulnerability * victims > 0.0 {
                (benefit * attack - f64::from(loss) * vulnerability) * victims * 17.0
                    / (attack + vulnerability * victims)
            } else {
                0.0
            };
            let gain = gain - f64::from(turns * 17);
            let stats = unit.kind.stats();
            let balanced = 2 * stats.cost * stats.attack / (stats.attack + stats.defense).max(1);
            let value = gain * (23.0_f64 / 24.0).powi(turns.max(1) + balanced / output);
            if value > best_value {
                best_value = value;
                best = Some(city.id);
            }
        }
        best
    }
}
