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
        self.create_city(1, home.0, home.1);
        self.create_city(HUMAN, front.0, front.1);
        let city = self.cities.last().unwrap().id;
        self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        // daiair's strict MC < currentMP target bound excludes OneAttack
        // bombers, whose native PF attack endpoint consumes a whole turn.
        // A Fighter exercises the actual supported combined-air mission.
        let bomber = self.spawn_unit(1, UnitKind::Fighter, home.0, home.1);
        let departure = offset_native_by_map(home.0, home.1, 1, 0);
        let infantry = self.spawn_unit(1, UnitKind::Marines, departure.0, departure.1);
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
        if !self.at_war(owner, city.owner) {
            return (0, 0);
        }
        let undefended = !self.units.iter().any(|u| {
            (u.x, u.y) == (city.x, city.y)
                && u.kind.stats().defense > 0
                && !self.allied(owner, u.owner)
        });
        let mut attacks = 0;
        let mut occupiers = 0;
        for actor in self.units.iter().filter(|u| {
            u.owner == owner && u.id != except && u.kind.is_military() && u.transport.is_none()
        }) {
            let assigned = self.ai_ground_targets.get(&actor.id) == Some(&city.id);
            let radius = self.unit_move_rate(actor) / MOVE_FRAGMENTS;
            let local = undefended && self.map_distance(actor.x, actor.y, city.x, city.y) <= radius;
            let marks = i32::from(assigned) + i32::from(local);
            if marks == 0 {
                continue;
            }
            attacks += marks
                * if actor.kind.bombard_rate() > 0 {
                    1
                } else {
                    actor.kind.stats().moves * MOVE_FRAGMENTS
                };
            occupiers += marks * i32::from(actor.kind.can_occupy_city());
            for cargo in self
                .units
                .iter()
                .filter(|u| u.transport == Some(actor.id) && u.kind.is_military())
            {
                attacks += marks * cargo.kind.stats().moves * MOVE_FRAGMENTS;
                occupiers += marks * i32::from(cargo.kind.can_occupy_city());
            }
        }
        // Nonattacking ferries still carry occupation-capable reinforcements.
        for boat in self.units.iter().filter(|u| {
            u.owner == owner && u.id != except && !u.kind.is_military() && u.kind.is_naval()
        }) {
            let assigned = self.ai_ground_targets.get(&boat.id) == Some(&city.id);
            let local = undefended && self.map_distance(boat.x, boat.y, city.x, city.y) <= 2;
            if assigned || local {
                for cargo in self
                    .units
                    .iter()
                    .filter(|u| u.transport == Some(boat.id) && u.kind.is_military())
                {
                    let marks = i32::from(assigned) + i32::from(local);
                    attacks += marks * cargo.kind.stats().moves * MOVE_FRAGMENTS;
                    occupiers += marks * i32::from(cargo.kind.can_occupy_city());
                }
            }
        }
        (attacks, occupiers)
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
                    let shore = self.map_wrap.offset(x, y, dx, dy)?;
                    self.cities
                        .iter()
                        .filter(|c| {
                            self.at_war(owner, c.owner)
                                && self.map_distance(shore.0, shore.1, c.x, c.y) <= 1
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
            if let Some((target, _)) = self.ground_city_attack_choice(&unit) {
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
                && self.map_distance(u.x, u.y, defender.x, defender.y) <= 1
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
        let Some(actor) = self.unit_by_id(id).cloned() else {
            return;
        };
        let goal = path.iter().fold((actor.x, actor.y), |(x, y), &(dx, dy)| {
            self.map_wrap
                .offset(x, y, dx, dy)
                .expect("planned step is real")
        });
        if goal == (actor.x, actor.y) {
            return;
        }
        self.ai_guard_for_destination(id, goal.0, goal.1);
        self.unit_goto_tiles.insert(id, goal);
        let Some((path, _)) = self.ai_goto_path(&actor, goal, GotoTask::Attack) else {
            return;
        };
        let length = path.len();
        for (position, (dx, dy)) in path.into_iter().enumerate() {
            let Some(actor) = self.unit_by_id(id).cloned() else {
                return;
            };
            let Some(target) = self.map_wrap.offset(actor.x, actor.y, dx, dy) else {
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
            let result = self.try_move_unit(id, dx, dy);
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

    pub(super) fn ground_city_attack_choice(&self, unit: &Unit) -> Option<(u32, Vec<(i32, i32)>)> {
        let mut best = None;
        let mut best_value = 0.0;
        for city in self.cities.iter().filter(|city| {
            self.at_war(unit.owner, city.owner) && self.tile(city.x, city.y).known_by[unit.owner]
        }) {
            let Some((path, cost)) = self.search_route(unit, city.x, city.y, false, i32::MAX)
            else {
                continue;
            };
            if path.is_empty() {
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
                self.invasion_support(unit.owner, city, unit.id);
            let allowance = self.unit_move_rate(unit).max(1);
            let turns = (cost + allowance - unit.moves_left.max(0)) / allowance;
            if turns > 10 {
                continue;
            }
            let rating = |actor: &Unit| {
                Self::attack_power(actor) * actor.hp * actor.kind.stats().firepower
                    / (POWER_FACTOR * 3)
            };
            let helpers: Vec<_> = self
                .units
                .iter()
                .filter_map(|u| {
                    if u.id == unit.id || !self.allied(unit.owner, u.owner) || rating(u) == 0 {
                        return None;
                    }
                    let nearby = i32::from(self.map_distance(u.x, u.y, city.x, city.y) <= 1);
                    let assigned = i32::from(self.ai_ground_targets.get(&u.id) == Some(&city.id));
                    (nearby + assigned > 0).then_some((u, nearby + assigned))
                })
                .collect();
            let attack = rating(unit)
                + helpers
                    .iter()
                    .map(|(u, count)| rating(u) * count)
                    .sum::<i32>();
            let attack = f64::from(attack).powi(2);
            let loss = unit.kind.stats().cost
                + helpers
                    .iter()
                    .map(|(u, count)| u.kind.stats().cost * count)
                    .sum::<i32>();
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
            let production: Vec<_> = self
                .cities
                .iter()
                .filter(|c| c.owner == unit.owner)
                .map(|c| self.city_forecast(c).shield_surplus())
                .collect();
            let average = production.iter().sum::<i32>() / production.len().max(1) as i32;
            let output = self
                .cities
                .iter()
                .find(|c| Some(c.id) == unit.home_city)
                .map_or(1, |c| self.city_forecast(c).shield_surplus())
                .max(average)
                .max(1);
            let stats = unit.kind.stats();
            let balanced = 2 * stats.cost * stats.attack / (stats.attack + stats.defense).max(1);
            let value = gain * (23.0_f64 / 24.0).powi(turns.max(1) + balanced / output);
            if value > best_value {
                best_value = value;
                best = Some((city.id, path));
            }
        }
        best
    }
}
