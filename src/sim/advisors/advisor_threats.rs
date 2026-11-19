//! Advisor assessment of military threats and urgency.

use super::*;
use crate::sim::movement_rules::MovementRules;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DangerRouteKey {
    owner: usize,
    kind: usize,
    start: usize,
    allowance: i32,
}

const UNSAMPLED_STEP: i32 = -2;
const BLOCKED_STEP: i32 = -1;

#[derive(Clone, Default)]
struct DangerRouteCache {
    maps: std::collections::BTreeMap<DangerRouteKey, Vec<i32>>,
    steps: std::collections::BTreeMap<(usize, usize), DangerStepCosts>,
}

type DangerStepCosts = std::collections::HashMap<usize, [i32; CIV3_DIRS.len()]>;

#[derive(Clone, Default)]
pub(super) struct ThreatCaches {
    enabled: std::cell::Cell<bool>,
    arrivals: std::cell::RefCell<std::collections::BTreeMap<(u32, u32), Option<i32>>>,
    routes: std::cell::RefCell<DangerRouteCache>,
}

impl ThreatCaches {
    pub(super) fn enabled(&self) -> bool {
        self.enabled.get()
    }
    pub(super) fn begin(&self) {
        self.arrivals.borrow_mut().clear();
        self.routes.borrow_mut().clear();
        self.enabled.set(true);
    }
    pub(super) fn clear(&self) {
        self.enabled.set(false);
        self.arrivals.borrow_mut().clear();
        self.routes.borrow_mut().clear();
    }
    pub(super) fn preserve(&self) {
        self.enabled.set(true);
    }
    #[cfg(test)]
    pub(super) fn route_sizes(&self) -> (usize, usize) {
        self.routes.borrow().sizes()
    }
    #[cfg(test)]
    pub(super) fn sizes(&self) -> (usize, usize, usize) {
        let (maps, steps) = self.routes.borrow().sizes();
        (self.arrivals.borrow().len(), maps, steps)
    }
}

impl DangerRouteCache {
    #[cfg(test)]
    pub(super) fn sizes(&self) -> (usize, usize) {
        (self.maps.len(), self.steps.len())
    }

    fn clear(&mut self) {
        self.maps.clear();
        self.steps.clear();
    }
}

impl Simulation {
    pub(in crate::sim) fn any_unit_at(
        &self,
        x: i32,
        y: i32,
        predicate: impl Fn(&Unit) -> bool,
    ) -> bool {
        if let Some(ids) = self.list_order.unit_ids_at(x, y) {
            return ids
                .iter()
                .filter_map(|id| self.unit_by_id(*id))
                .any(predicate);
        }
        if !self.advisor_cache.active() {
            return self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == (x, y) && predicate(unit));
        }
        self.advisor_cache
            .any_unit_at(self.map_wrap.index(x, y), &self.units, predicate)
    }

    pub(in crate::sim) fn any_city_at_cached(&self, x: i32, y: i32) -> bool {
        self.city_at_cached(x, y).is_some()
    }

    pub(in crate::sim) fn city_at_cached(&self, x: i32, y: i32) -> Option<&City> {
        if self.advisor_cache.active() {
            let index = self
                .advisor_cache
                .city_index_at(self.map_wrap.index(x, y))?;
            self.cities.get(index)
        } else {
            self.city_at(x, y)
        }
    }

    pub(in crate::sim) fn any_city_at(
        &self,
        x: i32,
        y: i32,
        predicate: impl Fn(&City) -> bool,
    ) -> bool {
        if !self.advisor_cache.active() {
            return self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && predicate(city));
        }
        self.advisor_cache
            .city_index_at(self.map_wrap.index(x, y))
            .is_some_and(|index| predicate(&self.cities[index]))
    }

    pub(in crate::sim) fn advisor_continent_labels(&self) -> std::sync::Arc<[usize]> {
        if self.advisor_cache.active() {
            self.advisor_cache.continent_labels()
        } else {
            self.continent_labels()
        }
    }

    pub(in crate::sim) fn city_threats<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = (&'a Unit, i32)> + 'a {
        self.city_threat_arrivals(city)
            .map(|(unit, rating, _)| (unit, rating))
    }

    pub(super) fn danger_route_turns(&self, unit: &Unit, city: &City) -> Option<i32> {
        let allowance = self.unit_move_rate(unit);
        if allowance <= 0 {
            return None;
        }
        let limit = 4 * allowance - 1; // normal assess_danger horizon: turns 0..3
        let positive_steps = MovementRules::campaign_steps_cost_at_least_one();
        let lower_bound = |x, y| {
            if positive_steps {
                self.map_distance(x, y, city.x, city.y)
            } else {
                0
            }
        };
        let first_bound = lower_bound(unit.x, unit.y);
        if first_bound > limit {
            return None;
        }
        if self.advisor_cache.threats().enabled() {
            let goal = self.map_wrap.index(city.x, city.y);
            let key = DangerRouteKey {
                owner: unit.owner,
                kind: unit.kind as usize,
                start: self.map_wrap.index(unit.x, unit.y),
                allowance,
            };
            let mut cache = self.advisor_cache.threats().routes.borrow_mut();
            if let Some(costs) = cache.maps.get(&key) {
                self.trace.count_operation("ai.danger_route.cache_hits", 1);
                let spent = costs[goal];
                return (spent != i32::MAX).then_some(spent / allowance);
            }
            self.trace.count_operation("ai.danger_route.maps_built", 1);
            // Step costs depend on the owner, unit type and the current map.
            // They can be shared across starting positions and allowances.
            let steps = cache.steps.entry((key.owner, key.kind)).or_default();
            let costs = self.danger_route_costs(unit, key.start, allowance, limit, steps);
            let spent = costs[goal];
            cache.maps.insert(key, costs);
            return (spent != i32::MAX).then_some(spent / allowance);
        }
        self.danger_route_turns_uncached(unit, city, allowance, limit, first_bound, lower_bound)
    }

    fn danger_route_costs(
        &self,
        unit: &Unit,
        start: usize,
        allowance: i32,
        limit: i32,
        steps: &mut DangerStepCosts,
    ) -> Vec<i32> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let mut distance = vec![i32::MAX; self.tiles.len()];
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        distance[start] = 0;
        let mut actor = unit.clone();
        actor.moves_left = allowance;
        let movement = self.step_movement_profile(unit.kind);
        let mut naval_attack_endpoints = Vec::new();
        let naval = unit.kind.is_naval();
        let mut looked_up = 0;
        let mut computed = 0;
        while let Some(Reverse((spent, index))) = queue.pop() {
            if distance[index] != spent {
                continue;
            }
            (actor.x, actor.y) = self.map_wrap.position(index);
            let remaining = allowance - spent % allowance;
            let steps = steps
                .entry(index)
                .or_insert([UNSAMPLED_STEP; CIV3_DIRS.len()]);
            for (neighbor, (x, y)) in self.map_wrap.civ3_neighbors(actor.x, actor.y).enumerate() {
                looked_up += 1;
                let step = &mut steps[neighbor];
                if *step == UNSAMPLED_STEP {
                    computed += 1;
                    *step = self
                        .movement_cost_with_profile(&actor, x, y, &movement)
                        .unwrap_or(BLOCKED_STEP);
                }
                if *step == BLOCKED_STEP {
                    if naval && self.any_city_at_cached(x, y) {
                        let arrival = spent + campaign_move_fragments().min(remaining);
                        if arrival <= limit {
                            naval_attack_endpoints.push((self.map_wrap.index(x, y), arrival));
                        }
                    }
                    continue;
                }
                let index = self.map_wrap.index(x, y);
                let candidate = spent + (*step).min(remaining);
                if candidate <= limit && candidate < distance[index] {
                    distance[index] = candidate;
                    queue.push(Reverse((candidate, index)));
                }
            }
        }
        // A ship can attack an otherwise inaccessible city as its final step.
        // Merge those arrivals after traversal so a city cannot open a sea route through land.
        for (city, arrival) in naval_attack_endpoints {
            distance[city] = distance[city].min(arrival);
        }
        self.trace
            .count_operation("ai.danger_route.steps_computed", computed);
        self.trace
            .count_operation("ai.danger_route.step_cache_hits", looked_up - computed);
        distance
    }

    fn danger_route_turns_uncached(
        &self,
        unit: &Unit,
        city: &City,
        allowance: i32,
        limit: i32,
        first_bound: i32,
        lower_bound: impl Fn(i32, i32) -> i32,
    ) -> Option<i32> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        // pft_fill_reverse_parameter uses a terrain-only reverse map: no
        // current movement, diplomacy, ZOC, fog or intervening-unit blockers.
        let start = self.map_wrap.index(unit.x, unit.y);
        let goal = self.map_wrap.index(city.x, city.y);
        let mut distance = vec![i32::MAX; self.tiles.len()];
        // Keep Freeciv's goal-directed query. The unobstructed step count is
        // admissible when every move spends at least one movement fragment.
        let mut queue = BinaryHeap::from([Reverse((first_bound, 0, start))]);
        distance[start] = 0;
        let mut actor = unit.clone();
        actor.moves_left = allowance;
        let movement = self.step_movement_profile(unit.kind);
        let naval = unit.kind.is_naval();
        while let Some(Reverse((_, spent, index))) = queue.pop() {
            if distance[index] != spent {
                continue;
            }
            if index == goal {
                return Some(spent / allowance);
            }
            (actor.x, actor.y) = self.map_wrap.position(index);
            let remaining = allowance - spent % allowance;
            for (x, y) in self.map_wrap.civ3_neighbors(actor.x, actor.y) {
                let index = self.map_wrap.index(x, y);
                let cost = self
                    .movement_cost_with_profile(&actor, x, y, &movement)
                    .or_else(|| {
                        // A ship's reverse attack map can reach the city even
                        // when the ship cannot enter its land tile normally.
                        (index == goal && naval).then(campaign_move_fragments)
                    });
                let Some(cost) = cost else {
                    continue;
                };
                let candidate = spent + cost.min(remaining);
                if candidate < distance[index] {
                    let estimate = candidate + lower_bound(x, y);
                    if estimate <= limit {
                        distance[index] = candidate;
                        queue.push(Reverse((estimate, candidate, index)));
                    }
                }
            }
        }
        None
    }

    pub(super) fn city_danger_arrival(&self, unit: &Unit, city: &City) -> Option<i32> {
        if self.advisor_cache.threats().enabled() {
            let key = (unit.id, city.id);
            if let Some(turns) = self.advisor_cache.threats().arrivals.borrow().get(&key) {
                return *turns;
            }
            let turns = self.city_danger_arrival_uncached(unit, city);
            self.advisor_cache
                .threats()
                .arrivals
                .borrow_mut()
                .insert(key, turns);
            return turns;
        }
        self.city_danger_arrival_uncached(unit, city)
    }

    fn city_danger_arrival_uncached(&self, unit: &Unit, city: &City) -> Option<i32> {
        // assess_danger_unit initializes with paradrop range, then compares
        // ordinary travel and the current ferry. Paradrop capability is a
        // future action estimate, not a promise that a drop is legal now.
        let mut turns = (unit.kind == UnitKind::Paratrooper)
            .then(|| self.map_distance(unit.x, unit.y, city.x, city.y) / 10);
        if let Some(walking) = self.danger_route_turns(unit, city)
            && turns.is_none_or(|current| walking < current)
        {
            turns = Some(walking);
        }
        if let Some(ferry) = unit.transport.and_then(|id| self.unit_by_id(id))
            && let Some(sailing) = self.danger_route_turns(ferry, city)
            && turns.is_none_or(|current| sailing < current)
        {
            turns = Some(sailing + i32::from(!unit.kind.has_marine_attack()));
        }
        turns
    }

    pub(in crate::sim) fn advisor_city_defend_bonus(&self, city: &City, attacker: UnitKind) -> i32 {
        let aerial = attacker.is_aircraft();
        let naval = attacker.is_naval();
        let mut bonus = if aerial {
            match self.tile(city.x, city.y).base {
                Some(Base::Airstrip | Base::Fortress) => 50,
                Some(Base::Airbase) => 100,
                _ => 0,
            }
        } else {
            self.tile(city.x, city.y)
                .base
                .map_or(0, Base::defense_bonus)
        };
        if !aerial && !naval {
            bonus += CITY_DEFEND_BONUS;
            if city.has_building(Building::CITY_WALLS) {
                bonus += 100;
            }
            if self.has_active_wonder(city.owner, Building::GREAT_WALL) {
                bonus += 50;
            }
        }
        if !aerial && city.size >= LARGE_CITY_SIZE {
            bonus += LARGE_CITY_DEFEND_BONUS;
        }
        if naval && city.has_building(Building::COASTAL_DEFENSE) {
            bonus += 100;
        }
        // SDI supplies Nuke_Proof, not Defend_Bonus. Missiles also have
        // their own class, outside both SAM Air/Helicopter effects.
        if aerial && !attacker.is_missile() && city.has_building(Building::SAM_BATTERY) {
            bonus += 100;
        }
        bonus
    }

    /// Source assess_danger and assess_defense_quadratic, with normal AI's
    /// three-turn horizon. Ratings remain integer at native assignment points.
    pub(in crate::sim) fn advisor_guard_city_assessment(&self, city: &City) -> GuardCityAssessment {
        self.trace.count_operation("guard_assessment.calls", 1);
        let cache_enabled = self.advisor_cache.active();
        if cache_enabled
            && let Some(assessment) = self.advisor_cache.memos().guard_assessment(city.id)
        {
            self.trace.count_operation("guard_assessment.cache_hit", 1);
            return assessment;
        }
        if cache_enabled {
            self.trace.count_operation("guard_assessment.cache_miss", 1);
        }
        let danger = self.planning.guard_danger().get_or_init(city.id, || {
            let mut result = super::planning::GuardDanger {
                danger: 0,
                urgency: 0,
                grave: 0,
            };
            for (enemy, _, turns) in self.city_threat_arrivals(city) {
                let occupies =
                    enemy.kind.can_occupy_city() || enemy.kind.can_carry(UnitKind::Warrior);
                let danger = self.advisor_threat_vulnerability(city, enemy);
                if occupies && turns <= 3 {
                    result.urgency += 1;
                    if turns <= 1 {
                        result.grave += 1;
                    }
                }
                result.danger += danger * danger / turns.max(1);
            }
            result.urgency += 10 * result.grave;
            result
        });
        let mut result = GuardCityAssessment {
            danger: danger.danger,
            urgency: danger.urgency,
            grave: danger.grave,
            defense: 0,
            defense_igwall: 0,
        };
        // Native chooses 90 whenever danger is nonzero, 5 otherwise, then
        // finds the integer ceiling square root of wallvalue*10.
        let wall_value = if result.danger > 0 { 90 } else { 5 };
        let mut walls = 0;
        while walls * walls < wall_value * 10 {
            walls += 1;
        }
        // city_got_defense_effect(NULL) calls get_city_bonus without a
        // unittype. All class-specific effects are MAYBE and fail CERTAIN.
        // Only these unconditional extra effects survive in this ruleset.
        let has_defense = self.advisor_city_has_defense_effect(city);
        let mut defense = 0;
        for unit in self.tile_units_ordered(city.x, city.y) {
            let mut rating = self.advisor_garrison_defense_rating(city, unit);
            result.defense_igwall += rating * rating;
            if has_defense {
                rating = rating * walls / 10;
            }
            defense += rating;
        }
        result.defense = defense.min(1 << 15).pow(2);
        if self.advisor_cache.active() {
            self.advisor_cache
                .memos()
                .remember_guard_assessment(city.id, result);
        }
        result
    }

    pub(in crate::sim) fn advisor_threat_vulnerability(&self, city: &City, enemy: &Unit) -> i32 {
        let occupies = enemy.kind.can_occupy_city() || enemy.kind.can_carry(UnitKind::Warrior);
        let stats = enemy.kind.stats();
        let mut danger = percent_of(stats.attack * POWER_FACTOR, enemy.veteran_power())
            * enemy.hp
            * stats.firepower
            / ADVISOR_POWER_DIVIDER;
        // Only_Native_Attack cannot attack a land city. Zero-attack
        // occupation carriers still require any protector.
        if enemy.kind == UnitKind::Submarine {
            danger = 0;
        }
        danger = danger * 100 / (100 + self.advisor_city_defend_bonus(city, enemy.kind)).max(1);
        if occupies {
            danger = danger.max(1);
        }
        let mut specific = 0;
        for defender in self.tile_units_ordered(city.x, city.y) {
            if defender.kind == UnitKind::AegisCruiser && enemy.kind.has_flag(UnitFlag::AirAttacker)
            {
                specific = specific.max(400);
            }
            if defender.kind == UnitKind::Destroyer && enemy.kind == UnitKind::Submarine {
                specific = specific.max(100);
            }
        }
        if specific > 100 {
            specific = (specific + 100) / 2;
        }
        danger = danger * 100 / (specific + 100);
        danger
    }

    pub(in crate::sim) fn advisor_garrison_defense_rating(&self, city: &City, unit: &Unit) -> i32 {
        if !Self::worker_guard_for_advisor(unit.kind) {
            return 0;
        }
        let stats = unit.kind.stats();
        let mut power = percent_of(stats.defense * POWER_FACTOR, unit.veteran_power());
        if !unit.kind.is_aircraft()
            && !unit.kind.is_naval()
            && unit.kind.unit_class() != UnitClass::BigLand
        {
            power = apply_percent_bonus(power, self.tile(city.x, city.y).terrain.defense_bonus());
        }
        // With attacker==NULL native deliberately skips Defend_Bonus
        // effects. Natural River defense remains an extra bonus, only
        // for native Land/Small Land classes (not Big Land or Sea).
        if self.tile(city.x, city.y).river
            && !unit.kind.is_aircraft()
            && !unit.kind.is_naval()
            && unit.kind.unit_class() != UnitClass::BigLand
        {
            power = apply_percent_bonus(power, 25);
        }
        if unit.kind.can_fortify() {
            power = apply_percent_bonus(power, FORTIFY_DEFENSE_BONUS);
        }
        let mut rating = power * unit.hp;
        if unit.kind.has_flag(UnitFlag::BadCityDefender) {
            rating = rating * stats.firepower.min(1) / 2;
        } else {
            rating *= stats.firepower;
        }
        rating /= ADVISOR_POWER_DIVIDER;
        rating
    }

    /// Match Freeciv's `city_got_defense_effect(city, nullptr)`, which asks
    /// the selected ruleset whether the city has any certain defense bonus.
    pub(in crate::sim) fn advisor_city_has_defense_effect(&self, city: &City) -> bool {
        self.city_rule_effect(
            city,
            EffectKind::DefendBonus,
            Some((city.x, city.y)),
            None,
            None,
            false,
            self.players[city.owner].government,
            None,
        ) > 0
    }

    fn worker_guard_for_advisor(kind: UnitKind) -> bool {
        !kind.is_civilian()
    }

    pub(in crate::sim) fn city_grave_danger(&self, city: &City) -> usize {
        self.city_threat_arrivals(city)
            .filter(|(unit, _, turns)| {
                *turns <= 1
                    && (unit.kind.can_occupy_city() || unit.kind.can_carry(UnitKind::Warrior))
            })
            .count()
    }

    pub(in crate::sim) fn city_threat_arrivals<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = (&'a Unit, i32, i32)> + 'a {
        // Threat consumers aggregate values or only test whether any threat
        // exists; player-list order cannot affect the result. Scan the unit
        // vector once instead of rebuilding and sorting one owner list per
        // player for every city assessment.
        let mut arrivals = std::collections::HashMap::new();
        self.units.iter().filter_map(move |unit| {
            let stats = unit.kind.stats();
            let carries_occupiers = unit.kind.can_carry(UnitKind::Warrior);
            if (!unit.kind.is_military() && !carries_occupiers)
                || !self.advisor_player_dangerous(city.owner, unit.owner)
            {
                return None;
            }
            let turns = if unit.transport.is_none() && !self.advisor_cache.threats().enabled() {
                let profile = (
                    unit.owner,
                    unit.kind as usize,
                    unit.x,
                    unit.y,
                    self.unit_move_rate(unit),
                );
                *arrivals
                    .entry(profile)
                    .or_insert_with(|| self.city_danger_arrival(unit, city))
            } else {
                self.city_danger_arrival(unit, city)
            }?;
            let rating = percent_of(POWER_FACTOR * stats.attack, unit.veteran_power())
                * unit.hp
                * stats.firepower
                / ADVISOR_POWER_DIVIDER;
            // Even an empty occupation transport requires some protection;
            // source assess_danger clamps its zero attack rating to one.
            Some((unit, rating.max(1) / turns.max(1), turns))
        })
    }
}
