//! Implements city defense behavior within the ai subsystem.

use super::*;

impl Simulation {
    pub(in crate::sim) fn choose_ai_rates(&mut self, owner: usize) {
        self.manage_ai_taxes(owner);
    }

    /// Reserve the best local units as city defenders before any other
    /// military jobs are assigned, following Freeciv's `dai_set_defenders`.
    /// The returned set is turn-local because CivCraft has no persistent
    /// generic unit-task field; the planning pass uses it to keep these units
    /// out of attacks, escorts, and reinforcement orders for this turn.
    pub(super) fn assign_ai_city_defenders(
        &mut self,
        owner: usize,
    ) -> std::collections::HashSet<u32> {
        let mut reserved = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return reserved;
        }

        let city_ids: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| city.id)
            .collect();
        for city_id in city_ids {
            let Some(city) = self.city_by_id(city_id).cloned() else {
                continue;
            };
            let mut defense = 0_i64;
            let attack = self.advisor_guard_city_assessment(&city).danger;
            let wall_value = if attack > 0 { 90 } else { 5 };
            let mut walls = 0;
            while walls * walls < wall_value * 10 {
                walls += 1;
            }
            let has_defense = self.advisor_city_has_defense_effect(&city);
            let government = self.players[owner].government;
            let martial_max = self
                .city_rule_effect(
                    &city,
                    "Martial_Law_Max",
                    None,
                    None,
                    None,
                    false,
                    government,
                    None,
                )
                .max(0);
            let martial_each = self
                .city_rule_effect(
                    &city,
                    "Martial_Law_By_Unit",
                    None,
                    None,
                    None,
                    false,
                    government,
                    None,
                )
                .max(0);
            let (_, unhappy, angry) = self.city_feelings(&city, self.city_forecast(&city).luxury);
            // The native AI counts unhappy and angry citizens by nationality,
            // then adds entertainers because it prefers a police unit to the
            // luxury specialist. CivCraft's city model has aggregate moods.
            let martless_unhappy = unhappy + angry + city.specialists[2].max(0);
            let candidates: Vec<_> = self
                .tile_units_ordered(city.x, city.y)
                .filter(|unit| unit.owner == owner && unit.transport.is_none())
                .map(|unit| unit.id)
                .collect();
            let mut count = 0_i32;
            let mut emergency = false;

            loop {
                let defense_needed = defense <= i64::from(attack);
                let martial_needed = count < martial_max
                    && martial_each > 0
                    && martless_unhappy > martial_each * count;
                if !defense_needed && !martial_needed {
                    break;
                }

                let mut best: Option<(u32, i32)> = None;
                for id in candidates.iter().copied() {
                    if reserved.contains(&id) {
                        continue;
                    }
                    let Some(unit) = self.unit_by_id(id) else {
                        continue;
                    };
                    let assigned_to_this_city = matches!(
                        self.ai_guard_targets.get(&id),
                        Some(super::super::guards::GuardTarget::City(target)) if *target == city.id
                    );
                    let has_other_job = self.ai_guard_targets.contains_key(&id)
                        && !assigned_to_this_city
                        || self.ai_ground_targets.contains_key(&id)
                        || self.ai_settlement_targets.contains_key(&id)
                        || self.hunter_targets.contains_key(&id)
                        || self.ai_ferries.passengers.contains_key(&id)
                        || self.ai_ferries.boats.contains_key(&id)
                        || self
                            .unit_goto_tiles
                            .get(&id)
                            .is_some_and(|target| *target != (city.x, city.y));
                    if !emergency && has_other_job {
                        continue;
                    }
                    let mut want = self.advisor_garrison_defense_rating(&city, unit);
                    if has_defense {
                        want = want * walls / 10;
                    }
                    // assess_defense_unit returns a squared rating, applying
                    // the same wall value used by assess_defense_quadratic.
                    want = want.saturating_mul(want);
                    if want > best.map_or(0, |(_, best_want)| best_want) {
                        best = Some((id, want));
                    }
                }

                let Some((id, want)) = best else {
                    if defense_needed && !emergency {
                        // Native retries once with all roles eligible if no
                        // ordinary defender can cover an actual threat.
                        emergency = true;
                        continue;
                    }
                    break;
                };
                let unit = self.unit_by_id(id).unwrap().clone();
                if (martless_unhappy < martial_each * count
                    || count >= martial_max
                    || martial_each <= 0)
                    && ((count >= 2 && unit.kind.stats().attack > unit.kind.stats().defense)
                        || (count >= 4 && unit.kind.stats().attack == unit.kind.stats().defense))
                {
                    break;
                }

                if emergency {
                    // The emergency pass may take over an existing guard
                    // assignment, as Freeciv does when danger is uncovered.
                    self.clear_ai_charge(id);
                    self.clear_ai_guard(id);
                    self.ai_ground_targets.remove(&id);
                    self.ai_settlement_targets.remove(&id);
                    self.hunter_targets.remove(&id);
                    self.ai_worker_tasks.remove(&id);
                    self.unit_goto_tiles.remove(&id);
                    self.release_ai_ferry(id);
                    let passenger =
                        self.ai_ferries
                            .boats
                            .get(&id)
                            .and_then(|boat| match boat.state {
                                super::super::ferries::FerryState::Available => None,
                                super::super::ferries::FerryState::Assigned(passenger) => {
                                    Some(passenger)
                                }
                            });
                    if let Some(boat) = self.ai_ferries.boats.get_mut(&id) {
                        boat.state = super::super::ferries::FerryState::Available;
                    }
                    if let Some(passenger) = passenger
                        && let Some(request) = self.ai_ferries.passengers.get_mut(&passenger)
                    {
                        request.request = super::super::ferries::FerryRequest::Wanted;
                    }
                }
                reserved.insert(id);
                defense += i64::from(want);
                count += 1;
            }
        }
        reserved
    }

    pub(super) fn counterattack_from_ai_cities(
        &mut self,
        owner: usize,
        reserved_defenders: &std::collections::HashSet<u32>,
    ) -> std::collections::HashSet<u32> {
        let mut committed = std::collections::HashSet::new();
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| (city.x, city.y))
            .collect();
        for (x, y) in cities {
            loop {
                let garrison: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == owner
                            && (unit.x, unit.y) == (x, y)
                            && unit.kind.is_military()
                            && !unit.kind.is_naval()
                            && !unit.kind.is_aircraft()
                            && unit.transport.is_none()
                            && !reserved_defenders.contains(&unit.id)
                    })
                    .collect();
                if garrison.len() <= 1 {
                    break;
                }
                let mut best = None;
                let mut best_gain = 0.0;
                for attacker in garrison {
                    if attacker.moves_left <= 0
                        || attacker.work_left > 0
                        || attacker.state == UnitState::Sentry
                        || committed.contains(&attacker.id)
                        || self.ai_guard_targets.contains_key(&attacker.id)
                    {
                        continue;
                    }
                    for (direction, (dx, dy)) in CIV3_DIRS.into_iter().enumerate() {
                        let Some((tx, ty)) = self.map_wrap.civ3_offset(x, y, dx, dy) else {
                            continue;
                        };
                        if !self.tile(tx, ty).known_by[owner]
                            || self.treaty_blocks_move(attacker, tx, ty)
                            || self.movement_cost_for(attacker, tx, ty).is_none()
                            || self.cities.iter().any(|city| (city.x, city.y) == (tx, ty))
                        {
                            continue;
                        }
                        let Some(defender) = self
                            .best_defender_id(attacker, tx, ty)
                            .and_then(|id| self.unit_by_id(id))
                            .filter(|unit| unit.kind.is_military())
                        else {
                            continue;
                        };
                        let (attack_fp, defense_fp) = self.combat_firepower(attacker, defender);
                        let chance = combat_win_chance(
                            Self::attack_power(attacker),
                            attacker.hp,
                            attack_fp,
                            self.defense_power_against(defender, tx, ty, attacker.kind),
                            defender.hp,
                            defense_fp,
                        );
                        let gain = chance * f64::from(defender.kind.stats().cost)
                            - (1.0 - chance) * f64::from(attacker.kind.stats().cost);
                        if chance >= 0.65 && gain > best_gain {
                            best = Some((attacker.id, direction));
                            best_gain = gain;
                        }
                    }
                }
                let Some((id, direction)) = best else {
                    break;
                };
                committed.insert(id);
                self.try_move_unit_civ3(id, direction);
            }
        }
        committed
    }

    pub(in crate::sim) fn reinforce_ai_cities(
        &mut self,
        owner: usize,
        reserved_defenders: &std::collections::HashSet<u32>,
    ) -> std::collections::HashSet<u32> {
        let mut dispatched = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return dispatched;
        }
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| {
                (
                    city.id,
                    self.city_threats(city)
                        .map(|(_, danger)| danger)
                        .sum::<i32>(),
                )
            })
            .filter(|&(_, danger)| danger > 0)
            .collect();
        let mut cities = cities;
        cities.sort_by_key(|&(id, danger)| (std::cmp::Reverse(danger), id));
        let is_defender = |unit: &Unit| {
            unit.owner == owner
                && unit.kind.is_military()
                && !unit.kind.is_naval()
                && !unit.kind.is_aircraft()
                && unit.transport.is_none()
        };
        for &(city_id, _) in &cities {
            let city = self.city_by_id(city_id).unwrap();
            let target = (city.x, city.y);
            let garrison: Vec<_> = self
                .units
                .iter()
                .filter(|unit| is_defender(unit) && (unit.x, unit.y) == target)
                .collect();
            let threatened: Vec<_> = self
                .city_threats(city)
                .filter_map(|(enemy, danger)| {
                    let odds = garrison
                        .iter()
                        .map(|unit| self.city_attacker_odds(city, enemy, unit, None))
                        .fold(1.0_f64, f64::min);
                    (odds > 0.5).then_some((enemy, danger, odds))
                })
                .collect();
            if threatened.is_empty() {
                continue;
            }
            let mut best = None;
            for unit in self.units.iter().filter(|unit| {
                is_defender(unit)
                    && unit.moves_left > 0
                    && unit.state != UnitState::Sentry
                    && unit.work_left == 0
                    && (unit.x, unit.y) != target
                    && !dispatched.contains(&unit.id)
                    && !reserved_defenders.contains(&unit.id)
                    && !self.ai_guard_targets.contains_key(&unit.id)
            }) {
                if let Some(home) = self
                    .cities
                    .iter()
                    .find(|home| home.owner == owner && (home.x, home.y) == (unit.x, unit.y))
                    && ((cities.iter().any(|&(id, _)| id == home.id)
                        && (self.city_grave_danger(home) > 0 || self.city_grave_danger(city) == 0))
                        || !self.units.iter().any(|guard| {
                            is_defender(guard)
                                && guard.id != unit.id
                                && (guard.x, guard.y) == (home.x, home.y)
                        }))
                {
                    continue;
                }
                let budget = unit.moves_left + self.unit_move_rate(unit);
                if self.map_distance(unit.x, unit.y, city.x, city.y) > budget {
                    continue;
                }
                let Some(cost) = self.route_cost_within(unit, city.x, city.y, budget) else {
                    continue;
                };
                let mut stationed = unit.clone();
                (stationed.x, stationed.y) = target;
                stationed.state = UnitState::Idle;
                let gain = threatened
                    .iter()
                    .map(|&(enemy, danger, odds)| {
                        (odds - self.city_attacker_odds(city, enemy, &stationed, None)).max(0.0)
                            * f64::from(danger)
                    })
                    .sum::<f64>()
                    / f64::from(cost.max(1));
                if gain > 0.0 && best.as_ref().is_none_or(|&(_, previous)| gain > previous) {
                    best = Some((unit.id, gain));
                }
            }
            if let Some((id, _)) = best {
                self.unit_goto_tiles.insert(id, target);
                self.ai_guard_for_destination(id, target.0, target.1);
                if self.ai_goto_unit(id, target, GotoTask::Defend).moved {
                    dispatched.insert(id);
                }
            }
        }
        dispatched
    }

    pub(crate) fn ai_defender_forecast_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_ground_targets.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.resource = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.known_by[1] = true;
        }
        self.sign_pact(1, HUMAN, Pact::War);
        let origin = (10, 24);
        let target = offset_native_by_map(origin.0, origin.1, 5, 0);
        self.create_city(HUMAN, target.0, target.1);
        self.cities[0].buildings.push(Building::BARRACKS_II);
        self.players[HUMAN].known = vec![Tech::GUNPOWDER];
        // Musketman requires connected Saltpeter in the Conquests roster.
        let center = self.tile_mut(target.0, target.1);
        center.terrain = Terrain::Hills;
        center.resource = Some(Resource::Saltpeter);
        center.owner = Some(HUMAN);
        center.road = true;
        self.players[HUMAN].gold = 0;
        let guard = self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        let id = self.spawn_unit(1, UnitKind::Marine, origin.0, origin.1);
        let attacker = self.unit_by_id(id).unwrap().clone();
        let city = &self.cities[0];
        let near = self.anticipated_city_defender(city, &attacker, 1, false);
        let far = self.anticipated_city_defender(city, &attacker, 2, false);
        let immediate = near.is_some_and(|u| u.id == guard && u.hp == 1);
        let prepared = far.is_some_and(|u| {
            u.kind == UnitKind::Musketman && u.veteran > 0 && u.hp == u.kind.stats().hp
        });
        let avoided = self.ground_city_attack_choice(&attacker).is_none();
        self.players[HUMAN].known.clear();
        self.cities[0].buildings.clear();
        immediate && prepared && avoided && self.ground_city_attack_choice(&attacker).is_some()
    }
}

#[cfg(test)]
#[path = "city_defense_tests.rs"]
mod tests;
