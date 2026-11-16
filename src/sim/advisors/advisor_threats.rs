//! Advisor assessment of military threats and urgency.

use super::{ADVISOR_POWER_DIVIDER, GuardCityAssessment};
use crate::rules::freeciv::EffectKind;
use crate::sim::pathfinding::{Frontier, moves_left_after};
use crate::sim::unit_catalog::{UnitClass, UnitFlag};
use crate::sim::{
    Base, Building, CITY_DEFEND_BONUS, City, FORTIFY_DEFENSE_BONUS, LARGE_CITY_DEFEND_BONUS,
    LARGE_CITY_SIZE, MOVE_FRAGMENTS, POWER_FACTOR, Simulation, Unit, UnitKind, apply_percent_bonus,
    percent_of,
};

impl Simulation {
    pub(in crate::sim) fn begin_ai_advisor_cache(&self) {
        self.caches.clear_advisor_threats();
        let tile_count = self.tiles.len();
        let mut tile_unit_indices = vec![Vec::new(); tile_count];
        for (unit_index, unit) in self.units.iter().enumerate() {
            tile_unit_indices[self.map_wrap.index(unit.x, unit.y)].push(unit_index);
        }
        for (tile_index, indices) in tile_unit_indices.iter_mut().enumerate() {
            let position = self.map_wrap.position(tile_index);
            indices.sort_by_key(|&unit_index| {
                std::cmp::Reverse(
                    self.list_order
                        .tile_unit_order_key(&self.units[unit_index], position),
                )
            });
        }
        let tile_unit_revision = self.list_order.tile_revision();
        let mut spatial = self.caches.spatial.borrow_mut();
        spatial.tile_unit_indices = Some(tile_unit_indices);
        spatial.tile_unit_revision = tile_unit_revision;
        spatial.unit_head.clear();
        spatial.unit_head.resize(tile_count, None);
        spatial.unit_next.clear();
        spatial.unit_next.resize(self.units.len(), None);
        for (unit_index, unit) in self.units.iter().enumerate() {
            let tile_index = self.map_wrap.index(unit.x, unit.y);
            spatial.unit_next[unit_index] = spatial.unit_head[tile_index];
            spatial.unit_head[tile_index] = Some(unit_index);
        }
        spatial.city_at.clear();
        spatial.city_at.resize(tile_count, None);
        for (city_index, city) in self.cities.iter().enumerate() {
            spatial.city_at[self.map_wrap.index(city.x, city.y)] = Some(city_index);
        }
        spatial.continent_labels = self.continent_labels();
        self.caches.advisor_enabled.set(true);
        self.caches.threat_routes_enabled.set(true);
    }

    pub(in crate::sim) fn end_ai_advisor_cache(&self) {
        self.caches.end_advisor_cache();
    }

    pub(in crate::sim) fn end_ai_advisor_cache_preserving_threat_routes(&self) {
        self.caches.end_advisor_cache_preserving_threat_routes();
    }

    pub(in crate::sim) fn clear_ai_threat_route_cache(&self) {
        self.caches.clear_ai_threat_route_cache();
    }

    pub(in crate::sim) fn any_unit_at(
        &self,
        x: i32,
        y: i32,
        predicate: impl Fn(&Unit) -> bool,
    ) -> bool {
        if !self.caches.advisor_cache_enabled() {
            return self
                .units
                .iter()
                .any(|unit| (unit.x, unit.y) == (x, y) && predicate(unit));
        }
        let spatial = self.caches.spatial.borrow();
        let mut unit_index = spatial.unit_head[self.map_wrap.index(x, y)];
        while let Some(index) = unit_index {
            if predicate(&self.units[index]) {
                return true;
            }
            unit_index = spatial.unit_next[index];
        }
        false
    }

    pub(in crate::sim) fn any_city_at_cached(&self, x: i32, y: i32) -> bool {
        self.city_at_cached(x, y).is_some()
    }

    pub(in crate::sim) fn city_at_cached(&self, x: i32, y: i32) -> Option<&City> {
        if self.caches.advisor_cache_enabled() {
            let index = self.caches.spatial.borrow().city_at[self.map_wrap.index(x, y)]?;
            self.cities.get(index)
        } else {
            self.cities.iter().find(|city| (city.x, city.y) == (x, y))
        }
    }

    pub(in crate::sim) fn any_city_at(
        &self,
        x: i32,
        y: i32,
        predicate: impl Fn(&City) -> bool,
    ) -> bool {
        if !self.caches.advisor_cache_enabled() {
            return self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && predicate(city));
        }
        self.caches.spatial.borrow().city_at[self.map_wrap.index(x, y)]
            .is_some_and(|index| predicate(&self.cities[index]))
    }

    pub(in crate::sim) fn advisor_continent_labels(&self) -> Vec<usize> {
        if self.caches.advisor_cache_enabled() {
            self.caches.spatial.borrow().continent_labels.clone()
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
        if self.caches.threat_route_cache_enabled() && !unit.kind.is_naval() {
            let goal = self.map_wrap.index(city.x, city.y);
            if let Some(costs) = self.caches.danger_route(unit.id) {
                let spent = costs[goal];
                return (spent != i32::MAX).then_some(spent / self.unit_move_rate(unit));
            }
            let costs = self.danger_route_costs(unit)?;
            let spent = costs[goal];
            self.caches.store_danger_route(unit.id, costs);
            return (spent != i32::MAX).then_some(spent / self.unit_move_rate(unit));
        }
        self.danger_route_turns_uncached(unit, city)
    }

    fn danger_route_costs(&self, unit: &Unit) -> Option<Vec<i32>> {
        let allowance = self.unit_move_rate(unit);
        if allowance <= 0 {
            return None;
        }
        let limit = 4 * allowance - 1; // normal assess_danger horizon: turns0..3
        let start = self.map_wrap.index(unit.x, unit.y);
        let mut frontier = Frontier::from_start(self.tiles.len(), i32::MAX, start, 0);
        let mut actor = unit.clone();
        actor.moves_left = allowance;
        while let Some((spent, index, ())) = frontier.pop() {
            (actor.x, actor.y) = self.map_wrap.position(index);
            let remaining = moves_left_after(spent, allowance, allowance);
            for (x, y) in self.map_wrap.civ3_neighbors(actor.x, actor.y) {
                let index = self.map_wrap.index(x, y);
                let cost = self.movement_cost_for(&actor, x, y);
                let Some(cost) = cost else {
                    continue;
                };
                let candidate = spent + cost.min(remaining);
                if candidate <= limit {
                    frontier.relax(index, candidate);
                }
            }
        }
        Some(frontier.into_costs())
    }

    fn danger_route_turns_uncached(&self, unit: &Unit, city: &City) -> Option<i32> {
        // pft_fill_reverse_parameter uses a terrain-only reverse map: no
        // current movement, diplomacy, ZOC, fog or intervening-unit blockers.
        let allowance = self.unit_move_rate(unit);
        if allowance <= 0 {
            return None;
        }
        let limit = 4 * allowance - 1;
        let start = self.map_wrap.index(unit.x, unit.y);
        let goal = self.map_wrap.index(city.x, city.y);
        let mut frontier = Frontier::from_start(self.tiles.len(), i32::MAX, start, 0);
        let mut actor = unit.clone();
        actor.moves_left = allowance;
        while let Some((spent, index, ())) = frontier.pop() {
            if index == goal {
                return Some(spent / allowance);
            }
            (actor.x, actor.y) = self.map_wrap.position(index);
            let remaining = moves_left_after(spent, allowance, allowance);
            for (x, y) in self.map_wrap.civ3_neighbors(actor.x, actor.y) {
                let index = self.map_wrap.index(x, y);
                let cost = self.movement_cost_for(&actor, x, y).or_else(|| {
                    // A ship's reverse attack map overlaps the coastal city,
                    // even when the ship cannot enter its land tile normally.
                    (index == goal && unit.kind.is_naval()).then_some(MOVE_FRAGMENTS)
                });
                let Some(cost) = cost else {
                    continue;
                };
                let candidate = spent + cost.min(remaining);
                if candidate <= limit {
                    frontier.relax(index, candidate);
                }
            }
        }
        None
    }

    pub(super) fn city_danger_arrival(&self, unit: &Unit, city: &City) -> Option<i32> {
        if self.caches.threat_route_cache_enabled() {
            let key = (unit.id, city.id);
            if let Some(turns) = self.caches.threat_arrival(key) {
                return turns;
            }
            let turns = self.city_danger_arrival_uncached(unit, city);
            self.caches.store_threat_arrival(key, turns);
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
        let cache_enabled = self.caches.advisor_cache_enabled();
        if cache_enabled
            && let Some(assessment) = self
                .caches
                .spatial
                .borrow()
                .guard_assessments
                .get(&city.id)
                .copied()
        {
            self.trace.count_operation("guard_assessment.cache_hit", 1);
            return assessment;
        }
        if cache_enabled {
            self.trace.count_operation("guard_assessment.cache_miss", 1);
        }
        let mut result = GuardCityAssessment {
            danger: 0,
            urgency: 0,
            grave: 0,
            defense: 0,
            defense_igwall: 0,
        };
        for (enemy, _, turns) in self.city_threat_arrivals(city) {
            let occupies = enemy.kind.can_occupy_city() || enemy.kind.can_carry(UnitKind::Warrior);
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
        if self.caches.advisor_cache_enabled() {
            self.caches
                .spatial
                .borrow_mut()
                .guard_assessments
                .insert(city.id, result);
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
        self.units.iter().filter_map(move |unit| {
            let stats = unit.kind.stats();
            let carries_occupiers = unit.kind.can_carry(UnitKind::Warrior);
            if (!unit.kind.is_military() && !carries_occupiers)
                || !self.advisor_player_dangerous(city.owner, unit.owner)
            {
                return None;
            }
            let turns = self.city_danger_arrival(unit, city)?;
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
