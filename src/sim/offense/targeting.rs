use super::*;

impl super::Simulation {
    pub(super) fn offensive_beachhead(
        &self,
        map: &mut NormalPaths,
        cargo: UnitKind,
        owner: usize,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32, bool)> {
        let mut best = None;
        let mut best_cost = i32::MAX;
        if cargo.has_marine_attack() {
            for (x, y) in self.map_wrap.neighbors(x, y) {
                if let Some(cost) = map.movement_cost(self, x, y)
                    && cost < best_cost
                {
                    best_cost = cost;
                    best = Some((x, y, true));
                }
            }
        } else {
            let mut checked = std::collections::HashSet::from([(x, y)]);
            for (bx, by) in self.map_wrap.neighbors(x, y) {
                if !self.pf_native_tile(cargo, bx, by) {
                    continue;
                }
                for (sx, sy) in self.map_wrap.neighbors(bx, by) {
                    if !checked.insert((sx, sy))
                        || self
                            .tile_units_ordered(sx, sy)
                            .any(|u| !self.allied(owner, u.owner))
                    {
                        continue;
                    }
                    if let Some(cost) = map.movement_cost(self, sx, sy)
                        && cost < best_cost
                    {
                        best_cost = cost;
                        best = Some((sx, sy, (sx, sy) != (bx, by)));
                    }
                }
            }
        }
        best
    }

    pub(super) fn offensive_find_ferry(&self, passenger: &Unit) -> Option<Unit> {
        // aiferry_find_boat's combined PF and sea_move EC. The source
        // intentionally cannot cross sea back onto a second continent.
        let mut map = NormalPaths::ferry_search_map(self, passenger);
        let mut best_turns = i32::MAX;
        let mut best = None;
        for (x, y, turn, extra) in map.positions(self) {
            if turn + (extra / 65536) as i32 > best_turns {
                break;
            }
            let tiles: Vec<_> = if self.tile(x, y).terrain.is_water() {
                std::iter::once((x, y))
                    .chain(self.map_wrap.neighbors(x, y))
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
                        / (boat.kind.stats().moves * MOVE_FRAGMENTS);
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

    pub(super) fn offensive_target(&self, city: &City, seed: &Unit) -> Option<OffensiveTarget> {
        let mut map = NormalPaths::attack_map(self, seed);
        let mut best: Option<OffensiveTarget> = None;
        let mut fallback = 0.0_f32;
        let rating = Self::offensive_attack_rating(seed);
        let unhap = self.offensive_military_unhappiness(city);
        let ferry = if !seed.kind.is_naval() {
            self.offensive_find_ferry(seed).or_else(|| {
                UnitKind::ALL_NATIVE
                    .into_iter()
                    .rev()
                    .find(|kind| {
                        kind.has_role(UnitRole::Ferryboat) && self.unit_available(seed.owner, *kind)
                    })
                    .or_else(|| {
                        UnitKind::ALL_NATIVE
                            .into_iter()
                            .find(|kind| kind.has_role(UnitRole::Ferryboat))
                    })
                    .filter(|_| {
                        self.map_wrap
                            .neighbors(seed.x, seed.y)
                            .any(|(x, y)| self.tile(x, y).terrain.is_water())
                    })
                    .map(|kind| self.make_unit(0, seed.owner, kind, seed.x, seed.y))
            })
        } else {
            None
        };
        let mut ferry_map = ferry
            .as_ref()
            .map(|ferry| NormalPaths::overlap_map(self, ferry));
        for owner in 0..self.players.len() {
            if !self.ai_potentially_hostile(seed.owner, owner) {
                continue;
            }
            for enemy in self.player_cities_ordered(owner) {
                if !self.tile(enemy.x, enemy.y).known_by[seed.owner]
                    || !self.offensive_can_attack(seed.kind, enemy.x, enemy.y)
                {
                    continue;
                }
                let mut by_boat = false;
                let turns = if let Some(turns) = map.position_turn(self, enemy.x, enemy.y) {
                    turns
                } else if let Some(fmap) = ferry_map.as_mut() {
                    let Some((x, y, land)) =
                        self.offensive_beachhead(fmap, seed.kind, seed.owner, enemy.x, enemy.y)
                    else {
                        continue;
                    };
                    by_boat = true;
                    let mut turns = fmap.position_turn(self, x, y).unwrap() + i32::from(land);
                    if let Some(boat) = ferry.as_ref()
                        && boat.id != 0
                        && (boat.x, boat.y) != (seed.x, seed.y)
                    {
                        let Some(walk) = map.position_turn(self, boat.x, boat.y) else {
                            continue;
                        };
                        turns += walk;
                    }
                    turns
                } else {
                    continue;
                };
                let defender = self.anticipated_city_defender(enemy, seed, turns, true);
                let vulnerability = defender.as_ref().map_or(0, |d| {
                    self.offensive_defense_squared(d, seed.kind, enemy.x, enemy.y, true)
                });
                let mut benefit = defender
                    .as_ref()
                    .map_or(0.0, |d| d.kind.stats().cost as f32);
                let (helper, helper_cost) = self.offensive_helpers(seed, enemy);
                let (invaders, occupiers) = self.invasion_support(seed.owner, enemy, 0);
                let victims = self.tile_units_ordered(enemy.x, enemy.y).count() as i32;
                let reserves = invaders - victims
                    + if seed.kind.has_flag(UnitFlag::OneAttack) {
                        1
                    } else {
                        seed.kind.stats().moves * MOVE_FRAGMENTS
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
                want -= (turns * if unhap { 53 } else { 17 }) as f32;
                let new_ferry = by_boat && ferry.as_ref().is_some_and(|f| f.id == 0);
                let cost = Self::offensive_balanced_cost(seed.kind)
                    + if new_ferry {
                        ferry.as_ref().unwrap().kind.stats().cost
                    } else {
                        0
                    };
                want = self.offensive_amortize(city, want, turns.max(1), cost);
                let choose_fallback =
                    want <= 0.0 && best.as_ref().is_none_or(|old| old.want <= 0.0) && {
                        let bk = self.offensive_amortize(city, benefit * 17.0, turns.max(1), cost);
                        if bk > fallback {
                            fallback = bk;
                            true
                        } else {
                            false
                        }
                    };
                if choose_fallback || want > best.as_ref().map_or(0.0, |old| old.want) {
                    best = Some(OffensiveTarget {
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
            for enemy in self.player_units_ordered(owner) {
                if !self.tile(enemy.x, enemy.y).known_by[seed.owner]
                    || self.cities.iter().any(|c| (c.x, c.y) == (enemy.x, enemy.y))
                    || !enemy.kind.has_hostile_action()
                    || !self.offensive_can_attack(seed.kind, enemy.x, enemy.y)
                    || self.best_defender_id(seed, enemy.x, enemy.y) != Some(enemy.id)
                {
                    continue;
                }
                let Some(turns) = map.position_turn(self, enemy.x, enemy.y) else {
                    continue;
                };
                let vulnerability =
                    self.offensive_defense_squared(enemy, seed.kind, enemy.x, enemy.y, true);
                let mut want = if turns > 10 {
                    0.0
                } else {
                    kill_desire(
                        enemy.kind.stats().cost as f32,
                        rating * rating,
                        seed.kind.stats().cost,
                        vulnerability,
                        1,
                    ) - (turns * if unhap { 53 } else { 17 }) as f32
                };
                want = self.offensive_amortize(
                    city,
                    want,
                    turns.max(1),
                    Self::offensive_balanced_cost(seed.kind),
                );
                if want > best.as_ref().map_or(0.0, |old| old.want) {
                    best = Some(OffensiveTarget {
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
        best
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
                current.hp = current.kind.stats().hp;
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
                    unit.hp = unit.kind.stats().hp;
                    unit.state = UnitState::Idle;
                    unit
                });
            let benefit = defender
                .as_ref()
                .map_or(0.0, |d| d.kind.stats().cost as f32);
            (defender, benefit)
        }
    }

    pub(super) fn offensive_candidate_turns(
        &self,
        candidate: &Unit,
        target: &OffensiveTarget,
    ) -> Option<i32> {
        let mut pf = NormalPaths::enemy_map(self, candidate);
        let mut turns = None;
        if let Some(boat) = target.ferry.as_ref() {
            let mut fmap = NormalPaths::overlap_map(self, boat);
            if let Some((x, y, _)) = self.offensive_beachhead(
                &mut fmap,
                candidate.kind,
                candidate.owner,
                target.x,
                target.y,
            ) {
                turns = fmap.position_turn(self, x, y);
                let first = pf.positions(self).into_iter().find(|(x, y, _, _)| {
                    (*x, *y) == (boat.x, boat.y) || (*x, *y) == (target.x, target.y)
                });
                if let Some((x, y, travel, _)) = first {
                    if (x, y) == (target.x, target.y) {
                        turns = Some(travel);
                    } else {
                        turns = turns.map(|sea| sea + travel);
                    }
                }
            }
        }
        turns.or_else(|| pf.position_turn(self, target.x, target.y))
    }
}
