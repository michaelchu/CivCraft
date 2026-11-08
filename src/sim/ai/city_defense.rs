//! Implements city defense behavior within the ai subsystem.

use super::*;

impl Simulation {
    pub(in crate::sim) fn choose_ai_rates(&mut self, owner: usize) {
        self.manage_ai_taxes(owner);
    }

    pub(super) fn counterattack_from_ai_cities(
        &mut self,
        owner: usize,
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
                    for (tx, ty) in self.map_wrap.neighbors(x, y) {
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
                            best = Some((attacker.id, tx, ty));
                            best_gain = gain;
                        }
                    }
                }
                let Some((id, tx, ty)) = best else {
                    break;
                };
                let (dx, dy) = self.map_wrap.vector(x, y, tx, ty);
                committed.insert(id);
                self.try_move_unit(id, dx, dy);
            }
        }
        committed
    }

    pub(in crate::sim) fn reinforce_ai_cities(
        &mut self,
        owner: usize,
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
        self.players[HUMAN].gold = 0;
        let guard = self.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        self.units.last_mut().unwrap().hp = 1;
        let id = self.spawn_unit(1, UnitKind::Marines, origin.0, origin.1);
        let attacker = self.unit_by_id(id).unwrap().clone();
        let city = &self.cities[0];
        let near = self.anticipated_city_defender(city, &attacker, 1, false);
        let far = self.anticipated_city_defender(city, &attacker, 2, false);
        let immediate = near.is_some_and(|u| u.id == guard && u.hp == 1);
        let prepared = far.is_some_and(|u| {
            u.kind == UnitKind::Musketeers && u.veteran > 0 && u.hp == u.kind.stats().hp
        });
        let avoided = self.ground_city_attack_choice(&attacker).is_none();
        self.players[HUMAN].known.clear();
        self.cities[0].buildings.clear();
        immediate && prepared && avoided && self.ground_city_attack_choice(&attacker).is_some()
    }
}
