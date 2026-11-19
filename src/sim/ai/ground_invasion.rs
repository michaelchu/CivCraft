//! Implements ground invasion behavior within the ai subsystem.

use super::*;

pub(in crate::sim) type SharedInvasionSupport =
    std::sync::Arc<std::collections::BTreeMap<u32, (i32, i32)>>;

impl Simulation {
    pub(super) fn run_coordinated_ground_order(
        &mut self,
        actor: &Unit,
        objective: diplomacy::WarObjective,
    ) -> bool {
        if !Self::can_join_ground_offensive(actor.kind) || actor.transport.is_some() {
            return false;
        }
        let Some(city) = self.city_by_id(objective.city) else {
            return false;
        };
        if city.owner == actor.owner {
            return false;
        }
        let attacking = self.at_war(actor.owner, city.owner);
        let goal = if attacking {
            (city.x, city.y)
        } else {
            objective.staging
        };
        // Keep this a local front, leaving distant units to their regular
        // roles and avoiding long searches for an unreachable muster tile.
        if self.map_distance(actor.x, actor.y, goal.0, goal.1) > 10 {
            return false;
        }
        let mut routes = if attacking {
            normal_paths::NormalPaths::attack_map(self, actor)
        } else {
            normal_paths::NormalPaths::goto_map(self, actor, GotoTask::None)
        };
        let Some((path, _)) = routes.path(self, goal.0, goal.1) else {
            return false;
        };
        if attacking {
            self.ai_ground_targets.insert(actor.id, objective.city);
        } else {
            self.ai_ground_targets.remove(&actor.id);
        }
        self.unit_goto_tiles.insert(actor.id, goal);
        self.follow_ground_invasion_path(actor.id, &path);
        true
    }

    #[cfg(test)]
    pub(crate) fn ai_ground_invasion_fixture(&mut self) -> (u32, u32, u32) {
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
            tile.known_by.set(1, true);
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

    /// Production candidates share the same army and have no real attacker
    /// to exclude. Keep their city markers within the advisor pass.
    pub(in crate::sim) fn offensive_invasion_support(&self, owner: usize) -> SharedInvasionSupport {
        let cached = self.ai_advisor_cache_enabled.get();
        if cached && let Some(markers) = self.ai_spatial_index.borrow().invasion_support.get(&owner)
        {
            self.trace
                .count_operation("ai.invasion_support.cache_hits", 1);
            return markers.clone();
        }
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| self.at_war(owner, city.owner))
            .collect();
        let markers = std::sync::Arc::new(self.invasion_support_many(owner, &cities, 0));
        if cached {
            self.ai_spatial_index
                .borrow_mut()
                .invasion_support
                .insert(owner, markers.clone());
        }
        markers
    }

    /// Calculate invasion markers for a set of cities from the owner's army.
    pub(in crate::sim) fn invasion_support_many(
        &self,
        owner: usize,
        target_cities: &[&City],
        except: u32,
    ) -> std::collections::BTreeMap<u32, (i32, i32)> {
        let _profile = self.trace.profiler().scope("ai.invasion_support");
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
        let indexed = self.list_order.planning_active();
        let mut units_scanned = 0;
        let mut cargo_support = std::collections::BTreeMap::<u32, (i32, i32)>::new();
        let add_cargo = |support: &mut (i32, i32), cargo: &Unit| {
            if cargo.kind.is_military() {
                support.0 += cargo.kind.stats().moves * campaign_move_fragments();
                support.1 += i32::from(cargo.kind.can_occupy_city());
            }
        };
        let mut army = Vec::new();
        if indexed {
            army.extend(self.player_units_ordered(owner));
            units_scanned += army.len();
        } else {
            // Isolated callers may have no live player or cargo lists.
            // Collect both from a single pass over the current units.
            for unit in &self.units {
                if unit.owner == owner {
                    army.push(unit);
                }
                if let Some(transport) = unit.transport {
                    add_cargo(cargo_support.entry(transport).or_default(), unit);
                }
            }
            units_scanned += self.units.len();
        }
        self.trace
            .count_operation("ai.invasion_support.batch_calls", 1);
        for actor in army {
            if actor.id == except {
                continue;
            }
            let military = actor.kind.is_military();
            if (military && actor.transport.is_some()) || (!military && !actor.kind.is_naval()) {
                continue;
            }
            let (mut attacks, mut occupiers) = if indexed {
                let mut support = (0, 0);
                for cargo in self.cargo_units_ordered(actor.id) {
                    units_scanned += 1;
                    add_cargo(&mut support, cargo);
                }
                support
            } else {
                cargo_support.get(&actor.id).copied().unwrap_or_default()
            };
            let radius = if military {
                attacks += if actor.kind.bombard_rate() > 0 {
                    1
                } else {
                    actor.kind.stats().moves * campaign_move_fragments()
                };
                occupiers += i32::from(actor.kind.can_occupy_city());
                self.unit_move_rate(actor) / campaign_move_fragments()
            } else if (attacks, occupiers) != (0, 0) {
                2 // Nonattacking ferries still mark the contribution of their cargo.
            } else {
                continue;
            };
            if let Some(&city_id) = self.ai_ground_targets.get(&actor.id)
                && let Some(&index) = target_index.get(&city_id)
            {
                result[index].0 += attacks;
                result[index].1 += occupiers;
            }
            for (x, y, _) in self.map_wrap.civ3_radius(actor.x, actor.y, radius) {
                if let Some(index) = city_at[self.map_wrap.index(x, y)]
                    && undefended[index]
                {
                    result[index].0 += attacks;
                    result[index].1 += occupiers;
                }
            }
        }
        self.trace
            .count_operation("ai.invasion_support.units_scanned", units_scanned as u64);
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
    ) -> std::collections::HashMap<u32, super::super::offense::OffensiveTarget> {
        let mut missions = std::collections::HashMap::new();
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
        if !(0..self.players.len()).any(|other| self.at_war(owner, other)) {
            return missions;
        }
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
            if let Some((_, city)) = self.passenger_assault_order(&boat) {
                self.ai_ground_targets.insert(boat.id, city.id);
                continue;
            }
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
        let production = std::cell::OnceCell::new();

        for unit in actors {
            if reserved.contains(&unit.id) {
                self.ai_ground_targets.remove(&unit.id);
                continue;
            }
            let (outputs, average) = production.get_or_init(|| {
                let outputs: std::collections::HashMap<_, _> = self
                    .cities_owned_by(owner)
                    .map(|city| (city.id, self.city_forecast(city).shield_surplus()))
                    .collect();
                let average = outputs.values().sum::<i32>() / outputs.len().max(1) as i32;
                (outputs, average)
            });
            let output = unit
                .home_city
                .and_then(|id| outputs.get(&id))
                .copied()
                .unwrap_or(0)
                .max(*average)
                .max(1);
            let target = self.offensive_target_with_assignments(
                output,
                &unit,
                &self.assigned_attack_strength(&unit),
            );
            if let Some(city) = target.as_ref().and_then(|target| target.city) {
                self.ai_ground_targets.insert(unit.id, city);
            } else {
                self.ai_ground_targets.remove(&unit.id);
            }
            if let Some(target) = target {
                missions.insert(unit.id, target);
            }
        }
        missions
    }

    pub(in crate::sim) fn ground_attack_is_my_turn(&self, actor: &Unit, defender: &Unit) -> bool {
        // daiunit.c:is_my_turn: strongest ready attacker against this defender
        // strikes first, so a weak unit cannot sacrifice the entire stack.
        let rating = |unit: &Unit| {
            Self::attack_power(unit) * unit.hp * unit.kind.stats().firepower / (POWER_FACTOR * 3)
        };
        let own = rating(actor);
        let own_defense = self.defense_power_against(defender, defender.x, defender.y, actor.kind);
        for other in self
            .map_wrap
            .civ3_radius(defender.x, defender.y, 1)
            .into_iter()
            .flat_map(|(x, y, _)| self.tile_units_ordered(x, y))
            .filter(|u| {
                u.id != actor.id
                    && u.owner == actor.owner
                    && u.moves_left > 0
                    && u.kind.is_military()
                    && !u.kind.is_aircraft()
                    && !u.kind.is_naval()
                    && (u.transport.is_none() || u.kind.has_marine_attack())
                    && !self.treaty_blocks_move(u, defender.x, defender.y)
                    && self.best_defender_id(u, defender.x, defender.y).is_some()
            })
        {
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

    pub(super) fn run_ground_mission(
        &mut self,
        actor: &Unit,
        target: &super::super::offense::OffensiveTarget,
    ) -> bool {
        let id = actor.id;
        let goal = (target.x, target.y);
        let hostile = target
            .city
            .and_then(|city| self.city_by_id(city))
            .is_some_and(|city| self.at_war(actor.owner, city.owner))
            || self
                .tile_units_ordered(goal.0, goal.1)
                .any(|enemy| self.at_war(actor.owner, enemy.owner));
        if !hostile {
            return false;
        }
        if let Some(ferry) = &target.ferry {
            let Some(boat) = self.unit_by_id(ferry.id).cloned() else {
                return false;
            };
            if !self.can_board_ship(actor, &boat) || !self.assign_ai_ferry(boat.id, id) {
                return false;
            }
            if (boat.x, boat.y) == (ferry.x, ferry.y) {
                self.unit_goto_tiles.insert(id, goal);
                self.follow_ground_invasion_path(id, &target.path);
            } else {
                self.ai_ferry_goto_waypoint(id, (boat.x, boat.y), goal);
            }
            if self.board_specific_ship(id, boat.id)
                || self.embark_adjacent_ship(id, boat.id)
                || self
                    .unit_by_id(id)
                    .is_some_and(|unit| unit.transport == Some(boat.id))
            {
                self.run_raider_transport(&boat);
            }
            return true;
        }
        self.ai_guard_for_destination(id, goal.0, goal.1);
        self.unit_goto_tiles.insert(id, goal);
        self.follow_ground_invasion_path(id, &target.path);
        true
    }

    fn follow_ground_invasion_path(&mut self, id: u32, path: &[(i32, i32)]) {
        let profiler = self.trace.profiler();
        let length = path.len();
        let _profile = profiler.scope("ai.ground_invasion.step_execution");
        for (position, (dx, dy)) in path.iter().copied().enumerate() {
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
}
