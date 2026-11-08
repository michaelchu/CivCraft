//! Implements guards behavior within the sim subsystem.

use super::normal_paths::GotoTask;
use super::unit_catalog::{UnitClass, UnitRole};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum GuardTarget {
    Unit(u32),
    City(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum GuardState {
    #[default]
    None,
    Wanted,
    Assigned(u32),
}

impl Simulation {
    pub(super) fn ai_guard_state(&self, charge: u32) -> GuardState {
        self.ai_unit_guards
            .get(&charge)
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn clear_ai_guard(&mut self, charge: u32) {
        if let GuardState::Assigned(guard) = self.ai_guard_state(charge)
            && self.ai_guard_targets.get(&guard) == Some(&GuardTarget::Unit(charge))
        {
            self.ai_guard_targets.remove(&guard);
        }
        self.ai_unit_guards.remove(&charge);
    }

    pub(super) fn clear_ai_charge(&mut self, guard: u32) {
        if let Some(GuardTarget::Unit(charge)) = self.ai_guard_targets.remove(&guard)
            && self.ai_guard_state(charge) == GuardState::Assigned(guard)
        {
            self.ai_unit_guards.remove(&charge);
        }
    }

    pub(super) fn request_ai_guard(&mut self, charge: u32) {
        self.clear_ai_guard(charge);
        if self.units.iter().any(|u| u.id == charge) {
            self.ai_unit_guards.insert(charge, GuardState::Wanted);
        }
    }

    pub(super) fn assign_ai_guard(&mut self, guard: u32, target: GuardTarget) -> bool {
        let Some(actor) = self.unit_by_id(guard) else {
            return false;
        };
        let valid = match target {
            GuardTarget::Unit(id) => {
                id != guard
                    && self
                        .units
                        .iter()
                        .any(|u| u.id == id && u.owner == actor.owner)
            }
            GuardTarget::City(id) => self
                .cities
                .iter()
                .any(|c| c.id == id && c.owner == actor.owner),
        };
        if !valid {
            return false;
        }
        self.clear_ai_charge(guard);
        if let GuardTarget::Unit(charge) = target {
            self.clear_ai_guard(charge);
            self.ai_unit_guards
                .insert(charge, GuardState::Assigned(guard));
        }
        self.ai_guard_targets.insert(guard, target);
        true
    }

    pub(super) fn forget_ai_guard_unit(&mut self, id: u32) {
        self.clear_ai_guard(id);
        self.clear_ai_charge(id);
    }

    pub(super) fn validate_ai_guards(&self) -> bool {
        self.ai_guard_targets.iter().all(|(&guard, &target)| {
            let Some(actor) = self.unit_by_id(guard) else {
                return false;
            };
            match target {
                GuardTarget::Unit(id) => {
                    id != guard
                        && self
                            .units
                            .iter()
                            .any(|u| u.id == id && u.owner == actor.owner)
                        && self.ai_guard_state(id) == GuardState::Assigned(guard)
                }
                GuardTarget::City(id) => self
                    .cities
                    .iter()
                    .any(|c| c.id == id && c.owner == actor.owner),
            }
        }) && self.ai_unit_guards.iter().all(|(&charge, &state)| {
            self.units.iter().any(|u| u.id == charge)
                && match state {
                    GuardState::Assigned(id) => {
                        self.ai_guard_targets.get(&id) == Some(&GuardTarget::Unit(charge))
                    }
                    _ => true,
                }
        })
    }

    pub(super) fn refresh_ai_guards(&mut self) {
        let invalid: Vec<_> = self
            .ai_guard_targets
            .iter()
            .filter_map(|(&guard, &target)| {
                let owner = self.unit_by_id(guard).map(|u| u.owner);
                let valid = owner.is_some_and(|owner| match target {
                    GuardTarget::Unit(id) => {
                        id != guard
                            && self.units.iter().any(|u| u.id == id && u.owner == owner)
                            && self.ai_guard_state(id) == GuardState::Assigned(guard)
                    }
                    GuardTarget::City(id) => {
                        self.cities.iter().any(|c| c.id == id && c.owner == owner)
                    }
                });
                (!valid).then_some(guard)
            })
            .collect();
        for guard in invalid {
            self.clear_ai_charge(guard);
        }
        self.ai_unit_guards.retain(|&charge, state| {
            self.units.iter().any(|u| u.id == charge)
                && match *state {
                    GuardState::None => false,
                    GuardState::Wanted => true,
                    GuardState::Assigned(guard) => {
                        self.ai_guard_targets.get(&guard) == Some(&GuardTarget::Unit(charge))
                    }
                }
        });
    }

    pub(super) fn guard_basic_defense(unit: &Unit) -> i32 {
        percent_of(
            unit.kind.stats().defense * POWER_FACTOR,
            unit.veteran_power(),
        ) * unit.hp
            * unit.kind.stats().firepower
            / (POWER_FACTOR * 3)
    }

    fn guard_attack_rating(unit: &Unit) -> i32 {
        percent_of(
            unit.kind.stats().attack * POWER_FACTOR,
            unit.veteran_power(),
        ) * unit.hp
            * unit.kind.stats().firepower
            / (POWER_FACTOR * 3)
    }

    pub(super) fn guard_defender_role(kind: UnitKind) -> bool {
        kind.has_role(UnitRole::DefendGood)
    }

    fn guard_native_sets(kind: UnitKind) -> (u16, u16) {
        // Literal terrain.native_to and extra.native_to class sets used by
        // common/unittype.c:set_unit_class_caches (including Maglev/River).
        const LAND: u16 = 0b11_1111_1111_1000;
        const WATER: u16 = 0b111;
        if kind.is_aircraft() || kind.is_missile() {
            return (LAND | WATER, 0b100);
        }
        if kind == UnitKind::Trireme {
            return (0b101, 0b10000);
        }
        if kind.is_naval() {
            return (WATER, 0);
        }
        if kind.unit_class() == UnitClass::BigLand {
            // Terrain enumeration indices: Jungle4, Mountains9, Swamp11.
            return (LAND & !(1 << 4 | 1 << 9 | 1 << 11), 0b0011);
        }
        if matches!(kind, UnitKind::Caravan | UnitKind::Freight) {
            return (LAND, 0b10011);
        }
        (LAND, 0b11011)
    }

    pub(super) fn guard_can_follow(guard: UnitKind, charge: UnitKind) -> bool {
        if guard.fuel_capacity() > 0
            && (charge.fuel_capacity() == 0 || charge.fuel_capacity() > guard.fuel_capacity())
        {
            return false;
        }
        let (terrain, extra) = Self::guard_native_sets(guard);
        let (other_terrain, other_extra) = Self::guard_native_sets(charge);
        other_terrain & !terrain == 0 && other_extra & !extra == 0
    }

    /// dai_gothere_bodyguard: requests are based on the final destination,
    /// including a city's potential freshly trained counter-defender.
    pub(super) fn ai_guard_for_destination(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(actor) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if actor.owner == BARBARIANS {
            self.clear_ai_guard(id);
            return false;
        }
        let existing = match self.ai_guard_state(id) {
            GuardState::Assigned(guard) => self.unit_by_id(guard),
            _ => None,
        };
        if existing.is_some_and(|u| (u.x, u.y) == (actor.x, actor.y)) {
            return true;
        }
        let mut danger = self
            .units
            .iter()
            .filter(|u| (u.x, u.y) == (x, y) && self.ai_potentially_hostile(actor.owner, u.owner))
            .map(Self::guard_attack_rating)
            .sum::<i32>();
        if let Some(city) = self
            .cities
            .iter()
            .find(|c| (c.x, c.y) == (x, y) && self.ai_potentially_hostile(actor.owner, c.owner))
            && let Some(defender) = self.defender_versus(city, &actor)
        {
            danger += Self::guard_attack_rating(&defender);
        }
        danger *= POWER_FACTOR * 3;
        danger /= actor.kind.stats().moves.max(1);
        if actor.kind.stats().ignores_terrain {
            danger = (f64::from(danger) / 1.5) as i32;
        }
        let defense = percent_of(
            actor.kind.stats().defense * POWER_FACTOR,
            actor.veteran_power(),
        ) * actor.hp;
        if danger >= defense {
            self.request_ai_guard(id);
            true
        } else {
            self.clear_ai_guard(id);
            false
        }
    }

    pub(super) fn ai_guard_allows_step(&self, actor: &Unit, _target: (i32, i32)) -> bool {
        if !self.players[actor.owner].ai {
            return true;
        }
        match self.ai_guard_state(actor.id) {
            GuardState::Assigned(guard) => !self
                .units
                .iter()
                .any(|u| u.id == guard && (u.x, u.y) == (actor.x, actor.y) && u.moves_left == 0),
            _ => true,
        }
    }

    pub(super) fn ai_guard_follow_step(&mut self, charge: u32, old_position: (i32, i32)) {
        let Some(actor) = self.unit_by_id(charge).cloned() else {
            return;
        };
        if !self.players[actor.owner].ai || (actor.x, actor.y) == old_position {
            return;
        }
        let GuardState::Assigned(id) = self.ai_guard_state(charge) else {
            return;
        };
        let Some(guard) = self.unit_by_id(id).cloned() else {
            return;
        };
        if guard.moves_left <= 0 || self.map_distance(guard.x, guard.y, actor.x, actor.y) != 1 {
            return;
        }
        let (dx, dy) = self.map_wrap.vector(guard.x, guard.y, actor.x, actor.y);
        if let Some(unit) = self.units.iter_mut().find(|u| u.id == id) {
            unit.state = UnitState::Idle;
        }
        self.try_move_unit(id, dx, dy);
    }

    fn ai_charge_goal(&self, charge: &Unit) -> Option<(i32, i32)> {
        self.unit_goto_tiles.get(&charge.id).copied().or_else(|| {
            self.ai_ground_targets
                .get(&charge.id)
                .and_then(|id| self.city_by_id(*id))
                .map(|c| (c.x, c.y))
                .or_else(|| {
                    self.ai_settlement_targets
                        .get(&charge.id)
                        .map(|t| t.destination)
                })
                .or_else(|| {
                    self.ai_merchant_missions
                        .get(&charge.id)
                        .and_then(|mission| self.city_by_id(mission.city))
                        .map(|city| (city.x, city.y))
                })
        })
    }

    fn ai_guard_rendezvous(&self, guard: &Unit, target: GuardTarget) -> Option<(i32, i32)> {
        match target {
            GuardTarget::City(id) => self.city_by_id(id).map(|c| (c.x, c.y)),
            GuardTarget::Unit(id) => {
                let charge = self.unit_by_id(id)?;
                let Some(goal) = self.ai_charge_goal(charge) else {
                    return Some((charge.x, charge.y));
                };
                let me_them = self.map_distance(guard.x, guard.y, charge.x, charge.y);
                let me_goal = self.map_distance(guard.x, guard.y, goal.0, goal.1);
                let them_goal = self.map_distance(charge.x, charge.y, goal.0, goal.1);
                let mine = self.unit_move_rate(guard).max(1);
                let theirs = self.unit_move_rate(charge).max(1);
                Some(
                    if me_goal < me_them
                        || (me_goal / mine < them_goal / theirs
                            && me_goal / mine < me_them / mine
                            && mine > theirs)
                    {
                        goal
                    } else {
                        (charge.x, charge.y)
                    },
                )
            }
        }
    }

    fn guard_reachable_sites(&self, guard: &Unit, limit: i32) -> Vec<(i32, i32, i32)> {
        let allowance = self.unit_move_rate(guard).max(1);
        let mut costs = vec![i32::MAX; self.tiles.len()];
        let start = self.map_wrap.index(guard.x, guard.y);
        costs[start] = 0;
        let mut queue = exploration::ExplorerQueue::default();
        queue.put(0, start);
        let mut sites = Vec::new();
        let mut actor = guard.clone();
        while let Some((cost, index)) = queue.pop() {
            let spent = cost as i32;
            if spent != costs[index] {
                continue;
            }
            if spent > limit {
                break;
            }
            (actor.x, actor.y) = self.map_wrap.position(index);
            sites.push((spent, actor.x, actor.y));
            let remaining = if spent < guard.moves_left {
                guard.moves_left - spent
            } else {
                allowance - (spent - guard.moves_left.max(0)) % allowance
            };
            for (x, y) in self.map_wrap.neighbors(actor.x, actor.y) {
                if self.treaty_blocks_move(&actor, x, y)
                    || self.zone_blocks_step(&actor, (actor.x, actor.y), (x, y))
                    || self
                        .units
                        .iter()
                        .any(|u| (u.x, u.y) == (x, y) && !self.allied(guard.owner, u.owner))
                    || self
                        .cities
                        .iter()
                        .any(|c| (c.x, c.y) == (x, y) && !self.allied(guard.owner, c.owner))
                {
                    continue;
                }
                let Some(step) = self.movement_cost_for(&actor, x, y) else {
                    continue;
                };
                let next = self.map_wrap.index(x, y);
                let candidate = spent + step.min(remaining);
                if candidate <= limit && candidate < costs[next] {
                    costs[next] = candidate;
                    queue.put(i64::from(candidate), next);
                }
            }
        }
        sites
    }

    pub(super) fn look_for_ai_charge(&self, guard: &Unit) -> Option<(GuardTarget, f32)> {
        let toughness = Self::guard_basic_defense(guard).pow(2);
        if toughness == 0 {
            return None;
        }
        let allowance = self.unit_move_rate(guard).max(1);
        let limit = 3 * allowance;
        let reachable = self.guard_reachable_sites(guard, limit);
        let mut best = None;
        let mut best_def = -1;
        let mut best_city = None;
        for (cost, x, y) in reachable {
            for buddy in self.tile_units_ordered(x, y) {
                if buddy.owner != guard.owner
                    || self.ai_guard_state(buddy.id) != GuardState::Wanted
                    || !Self::guard_can_follow(guard.kind, buddy.kind)
                    || self.unit_move_rate(buddy) > allowance
                    || buddy.kind.stats().defense >= guard.kind.stats().defense
                    || (buddy.kind.stats().attack > 0
                        && buddy.kind.transport_capacity() == 0
                        && buddy.kind.stats().attack <= guard.kind.stats().attack)
                {
                    continue;
                }
                let mut desire = toughness - Self::guard_basic_defense(buddy).pow(2);
                if desire <= 0 {
                    continue;
                }
                if buddy.kind.transport_capacity() == 0 {
                    desire >>= cost / (2 * allowance);
                }
                if desire > best_def {
                    best = Some(GuardTarget::Unit(buddy.id));
                    best_def = desire;
                }
            }
            if let Some(city) = self
                .cities
                .iter()
                .find(|c| c.owner == guard.owner && (c.x, c.y) == (x, y))
            {
                let data = self.advisor_guard_city_assessment(city);
                if data.urgency <= 0 {
                    continue;
                }
                if let Some(previous) = best_city {
                    let previous: advisors::GuardCityAssessment = previous;
                    if previous.grave > 0
                        || previous.urgency > data.urgency
                        || (previous.danger > data.danger && data.grave == 0)
                    {
                        continue;
                    }
                }
                let desire = (data.danger - data.defense).max(0) >> (cost / (2 * allowance));
                if desire > best_def && desire > 0 {
                    best = Some(GuardTarget::City(city.id));
                    best_def = desire;
                    best_city = Some(data);
                }
            }
        }
        best.map(|target| (target, (best_def * 100 / toughness) as f32))
    }

    fn ai_guard_rampage(&mut self, id: u32) {
        // dai_military_rampage(BODYGUARD_RAMPAGE_THRESHOLD=17*4,
        // RAMPAGE_FREE_CITY_OR_BETTER=99999), default occupychance=0.
        let Some(initial) = self.unit_by_id(id) else {
            return;
        };
        let mut attempts = initial.moves_left + 1;
        while attempts > 0 {
            attempts -= 1;
            let Some(actor) = self.unit_by_id(id).cloned() else {
                return;
            };
            if actor.moves_left <= 0 || actor.kind.stats().attack <= 0 {
                return;
            }
            let mut best = None;
            let mut best_want = 0;
            for enemy in self
                .units
                .iter()
                .filter(|u| self.at_war(actor.owner, u.owner))
            {
                let target = (enemy.x, enemy.y);
                if self.best_defender_id(&actor, target.0, target.1) != Some(enemy.id)
                    || !self.ground_attack_is_my_turn(&actor, enemy)
                {
                    continue;
                }
                let Some((path, _)) =
                    self.search_route(&actor, target.0, target.1, false, actor.moves_left)
                else {
                    continue;
                };
                if path.is_empty() {
                    continue;
                }
                let vulnerable = !self.cities.iter().any(|c| (c.x, c.y) == target)
                    && !matches!(
                        self.tile(target.0, target.1).base,
                        Some(Base::Fort | Base::Fortress)
                    );
                let mut benefit = if vulnerable {
                    self.units
                        .iter()
                        .filter(|u| (u.x, u.y) == target && self.at_war(actor.owner, u.owner))
                        .map(|u| u.kind.stats().cost)
                        .sum()
                } else {
                    enemy.kind.stats().cost
                };
                if !vulnerable && self.units.iter().filter(|u| (u.x, u.y) == target).count() > 1 {
                    benefit = benefit * actor.hp / actor.kind.stats().hp;
                }
                let (attack_fp, defense_fp) = self.combat_firepower(&actor, enemy);
                let chance = combat_win_chance(
                    Self::attack_power(&actor),
                    actor.hp,
                    attack_fp,
                    self.defense_power_against(enemy, target.0, target.1, actor.kind),
                    enemy.hp,
                    defense_fp,
                );
                let loss = actor.kind.stats().cost;
                let desire = ((f64::from(benefit + loss) * chance - f64::from(loss)) * 17.0) as i32;
                let threshold = if self.map_distance(actor.x, actor.y, target.0, target.1) == 1 {
                    68
                } else {
                    99999
                };
                if desire > threshold && desire > best_want {
                    best = Some(path);
                    best_want = desire;
                }
            }
            let Some(path) = best else {
                return;
            };
            self.follow_civ3_path(id, path);
        }
    }

    /// The native manager retains an escort only while its charge still
    /// needs it. Fuelled escorts move only in their charge's movement hook.
    pub(super) fn run_ai_guard(&mut self, id: u32) -> bool {
        let Some(actor) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if actor.owner == BARBARIANS {
            return false;
        }
        if actor.transport.is_some() {
            return self.ai_guard_targets.contains_key(&id);
        }
        if let Some(target) = self.ai_guard_targets.get(&id).copied() {
            let keep = match target {
                GuardTarget::Unit(charge) => self.unit_by_id(charge).is_some_and(|u| {
                    u.owner == actor.owner
                        && Self::guard_basic_defense(&actor) > Self::guard_basic_defense(u)
                }),
                GuardTarget::City(city) => self
                    .cities
                    .iter()
                    .find(|c| c.id == city && c.owner == actor.owner)
                    .is_some_and(|c| {
                        let d = self.advisor_guard_city_assessment(c);
                        d.urgency > 0 && d.danger > d.defense
                    }),
            };
            if !keep {
                self.clear_ai_charge(id);
                self.unit_goto_tiles.remove(&id);
            }
        }
        if !self.ai_guard_targets.contains_key(&id)
            && actor.hp * 4 >= actor.kind.stats().hp
            && Self::guard_defender_role(actor.kind)
            && let Some((target, _)) = self.look_for_ai_charge(&actor)
        {
            self.assign_ai_guard(id, target);
            self.ai_ground_targets.remove(&id);
            let goal = match target {
                GuardTarget::Unit(charge) => self
                    .units
                    .iter()
                    .find(|u| u.id == charge)
                    .map(|u| (u.x, u.y)),
                GuardTarget::City(city) => self
                    .cities
                    .iter()
                    .find(|c| c.id == city)
                    .map(|c| (c.x, c.y)),
            };
            if let Some(goal) = goal {
                self.unit_goto_tiles.insert(id, goal);
            }
        }
        let Some(target) = self.ai_guard_targets.get(&id).copied() else {
            return false;
        };
        if actor.kind.fuel_capacity() > 0 {
            return true;
        }
        let Some(destination) = self.ai_guard_rendezvous(&actor, target) else {
            self.clear_ai_charge(id);
            return false;
        };
        if (actor.x, actor.y) != destination {
            if self.ai_goto_is_sane(&actor, destination) {
                self.ai_guard_for_destination(id, destination.0, destination.1);
                self.unit_goto_tiles.insert(id, destination);
                let result = self.ai_goto_unit(id, destination, GotoTask::Escort);
                // dai_gothere also returns false for a living guard which
                // has not reached the rendezvous or an adjacent tile yet.
                if !result.alive
                    || self.unit_by_id(id).is_none_or(|u| {
                        self.map_distance(u.x, u.y, destination.0, destination.1) > 1
                    })
                {
                    return true;
                }
            } else {
                self.clear_ai_charge(id);
                self.unit_goto_tiles.remove(&id);
                return false;
            }
        }
        self.ai_guard_rampage(id);
        true
    }

    /// daiferry.c:aiferry_gobyboat gathers the boss's guard, waits if it
    /// exhausted movement trying to catch up, and loads it before sailing.
    pub(super) fn ai_guard_prepare_ferry(&mut self, boss: u32, boat: u32) -> bool {
        let GuardState::Assigned(id) = self.ai_guard_state(boss) else {
            return true;
        };
        let Some(passenger) = self.unit_by_id(boss).cloned() else {
            return true;
        };
        let Some(guard) = self.unit_by_id(id).cloned() else {
            self.request_ai_guard(boss);
            return true;
        };
        if (guard.x, guard.y) != (passenger.x, passenger.y) {
            if !self
                .ai_goto_unit(id, (passenger.x, passenger.y), GotoTask::Escort)
                .alive
            {
                self.request_ai_guard(boss);
                return true;
            }
            let Some(current) = self.unit_by_id(id).cloned() else {
                self.request_ai_guard(boss);
                return true;
            };
            if current.moves_left <= 0 {
                return false;
            }
            // Literal source ditches a guard which still has movement after
            // the separate-location goto branch, then asks for a replacement.
            self.request_ai_guard(boss);
            return true;
        }
        if guard.transport != Some(boat) {
            self.board_specific_ship(id, boat);
        }
        true
    }
}

#[cfg(test)]
#[path = "guards/tests.rs"]
mod tests;
