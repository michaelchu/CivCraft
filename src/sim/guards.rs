//! Implements guards behavior within the sim subsystem.

use super::normal_paths::GotoTask;
use super::unit_catalog::UnitClass;
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
                            && self.unit_by_id(id).is_some_and(|u| u.owner == owner)
                            && self.ai_guard_state(id) == GuardState::Assigned(guard)
                    }
                    GuardTarget::City(id) => self.city_by_id(id).is_some_and(|c| c.owner == owner),
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
        super::action_rules::unit_has_role(kind, "DefendGood")
    }

    fn guard_native_sets(kind: UnitKind) -> (u16, u16) {
        // Literal terrain.native_to and extra.native_to class sets used by
        // common/unittype.c:set_unit_class_caches (including Maglev/River).
        const LAND: u16 = 0b11_1111_1111_1000;
        const WATER: u16 = 0b111;
        if kind.is_aircraft() || kind.is_missile() {
            return (LAND | WATER, 0b100);
        }
        if kind == UnitKind::Galley {
            return (0b101, 0b10000);
        }
        if kind.is_naval() {
            return (WATER, 0);
        }
        if kind.unit_class() == UnitClass::BigLand {
            // Terrain enumeration indices: Jungle4, Mountains9, Swamp11.
            return (LAND & !(1 << 4 | 1 << 9 | 1 << 11), 0b0011);
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
        // A zero-attack destination is safe even for a zero-defense civilian
        // (for example, a Settler). Equality is meaningful only after some
        // hostile force contributes danger.
        if danger > 0 && danger >= defense {
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
        if guard.moves_left <= 0
            || self
                .map_wrap
                .step_distance(guard.x, guard.y, actor.x, actor.y)
                != 1
        {
            return;
        }
        let direction = CIV3_DIRS.iter().position(|&(dx, dy)| {
            self.map_wrap.eight_way_offset(guard.x, guard.y, dx, dy) == Some((actor.x, actor.y))
        });
        if let Some(unit) = self.units.iter_mut().find(|u| u.id == id) {
            unit.state = UnitState::Idle;
        }
        if let Some(direction) = direction {
            self.try_move_unit(id, direction);
        }
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
        let tile_count = self.tiles.len();
        let mut costs = vec![i32::MAX; tile_count];
        let start = self.map_wrap.index(guard.x, guard.y);
        costs[start] = 0;
        let mut queue = exploration::IndexedExplorerQueue::new(tile_count);
        queue.put(0, start);
        let profile = self.step_movement_profile(guard.kind);
        let (allied_mask, _, _) = self.pact_masks(guard.owner);
        let ignores_zoc = Self::unit_ignores_zoc(guard.kind);
        let mut enemy_zoc = if ignores_zoc {
            Vec::new()
        } else {
            vec![None; tile_count]
        };
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
            let from_no_zoc = ignores_zoc
                || self.city_at_cached(actor.x, actor.y).is_some()
                || Self::terrain_has_no_zoc(self.tiles[index].terrain);
            for (x, y) in self.map_wrap.eight_way_neighbors(actor.x, actor.y) {
                let next = self.map_wrap.index(x, y);
                if costs[next] <= spent {
                    continue;
                }
                let Some(step) = self.movement_cost_with_profile(&actor, x, y, &profile) else {
                    continue;
                };
                let candidate = spent + step.min(remaining);
                if candidate > limit || candidate >= costs[next] {
                    continue;
                }
                let dest_city = self.city_at_cached(x, y);
                if dest_city
                    .is_some_and(|city| city.owner >= 64 || ((allied_mask >> city.owner) & 1) == 0)
                    || self.any_unit_at(x, y, |u| {
                        u.owner >= 64 || ((allied_mask >> u.owner) & 1) == 0
                    })
                    || self.treaty_blocks_move(&actor, x, y)
                {
                    continue;
                }
                if !from_no_zoc
                    && dest_city.is_none()
                    && !Self::terrain_has_no_zoc(self.tiles[next].terrain)
                    && !self.any_unit_at(x, y, |_| true)
                {
                    let from_controlled = *enemy_zoc[index].get_or_insert_with(|| {
                        self.enemy_zoc_controls_tile(guard.owner, actor.x, actor.y, false)
                    });
                    if from_controlled {
                        let to_controlled = *enemy_zoc[next].get_or_insert_with(|| {
                            self.enemy_zoc_controls_tile(guard.owner, x, y, false)
                        });
                        if to_controlled {
                            continue;
                        }
                    }
                }
                let new_entry = costs[next] == i32::MAX;
                self.trace.count_operation(
                    if new_entry {
                        "guard_route.queue_inserts"
                    } else {
                        "guard_route.queue_decrease_keys"
                    },
                    1,
                );
                if !new_entry {
                    self.trace.count_operation(
                        "guard_route.queue_frontier_entries_at_decrease_key",
                        queue.frontier_len() as u64,
                    );
                }
                costs[next] = candidate;
                queue.put(i64::from(candidate), next);
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
            if !self
                .list_order
                .unit_ids_at(x, y)
                .is_some_and(|ids| ids.is_empty())
            {
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
            }
            if !guard.kind.is_naval()
                && let Some(city) = self
                    .city_at_cached(x, y)
                    .filter(|city| city.owner == guard.owner)
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

    pub(super) fn ai_military_rampage(
        &mut self,
        id: u32,
        adjacent_threshold: i32,
        movement_threshold: i32,
    ) -> bool {
        let Some(initial) = self.unit_by_id(id) else {
            return false;
        };
        let mut attempts = initial.moves_left + 1;
        while attempts > 0 {
            attempts -= 1;
            let Some(actor) = self.unit_by_id(id).cloned() else {
                return false;
            };
            if actor.moves_left <= 0 || actor.kind.stats().attack <= 0 {
                return true;
            }
            let (_, war_mask, _) = self.pact_masks(actor.owner);
            let mut best = None;
            let mut best_want = 0;
            let enemies: Vec<_> = self
                .units
                .iter()
                .filter(|enemy| {
                    ((enemy.owner < 64 && ((war_mask >> enemy.owner) & 1) != 0)
                        || (enemy.owner >= 64 && self.at_war(actor.owner, enemy.owner)))
                        && (actor.x, actor.y) != (enemy.x, enemy.y)
                })
                .collect();
            if enemies.is_empty() {
                return true;
            }
            let goals: Vec<_> = enemies
                .iter()
                .map(|enemy| self.map_wrap.index(enemy.x, enemy.y))
                .collect();
            let (_, previous, expanded) =
                self.search_route_graph_map(&actor, &goals, actor.moves_left, false, true);
            let previous = previous.expect("rampage search requests paths");
            self.trace.count_operation("rampage.searches", 1);
            self.trace
                .count_operation("rampage.tiles_expanded", expanded);
            for (enemy, goal) in enemies.into_iter().zip(goals) {
                let target = (enemy.x, enemy.y);
                if previous[goal].is_none()
                    || self.best_defender_id(&actor, target.0, target.1) != Some(enemy.id)
                    || !self.ground_attack_is_my_turn(&actor, enemy)
                {
                    continue;
                }
                let vulnerable = self.city_at_cached(target.0, target.1).is_none()
                    && !matches!(
                        self.tile(target.0, target.1).base,
                        Some(Base::Fort | Base::Fortress)
                    );
                let mut benefit = if vulnerable {
                    self.tile_units_ordered(target.0, target.1)
                        .filter(|u| self.at_war(actor.owner, u.owner))
                        .map(|u| u.kind.stats().cost)
                        .sum()
                } else {
                    enemy.kind.stats().cost
                };
                if !vulnerable && self.tile_units_ordered(target.0, target.1).count() > 1 {
                    benefit = benefit * actor.hp / actor.max_hit_points();
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
                let threshold = if self
                    .map_wrap
                    .step_distance(actor.x, actor.y, target.0, target.1)
                    == 1
                {
                    adjacent_threshold
                } else {
                    movement_threshold
                };
                if desire > threshold && desire > best_want {
                    best = Some(goal);
                    best_want = desire;
                }
            }
            let start = self.map_wrap.index(actor.x, actor.y);
            let Some(path) =
                best.and_then(|goal| super::pathfinding::reconstruct_path(&previous, start, goal))
            else {
                return true;
            };
            self.follow_grid_path(id, path);
            if self.unit_by_id(id).is_none() {
                return false;
            }
        }
        self.unit_by_id(id).is_some()
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
                    .city_by_id(city)
                    .filter(|c| c.owner == actor.owner)
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
            && actor.hp * 4 >= actor.max_hit_points()
            && Self::guard_defender_role(actor.kind)
            && let Some((target, _)) = self.look_for_ai_charge(&actor)
        {
            self.assign_ai_guard(id, target);
            self.ai_ground_targets.remove(&id);
            let goal = match target {
                GuardTarget::Unit(charge) => self.unit_by_id(charge).map(|u| (u.x, u.y)),
                GuardTarget::City(city) => self.city_by_id(city).map(|c| (c.x, c.y)),
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
                let result = self.ai_goto_unit_checked(id, destination, GotoTask::Escort, true);
                // dai_gothere also returns false for a living guard which
                // has not reached the rendezvous or an adjacent tile yet.
                if !result.alive
                    || self.unit_by_id(id).is_none_or(|u| {
                        self.map_wrap
                            .step_distance(u.x, u.y, destination.0, destination.1)
                            > 1
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
        // Freeciv bodyguards attack worthwhile adjacent threats but move only
        // to a free city. Keep those thresholds distinct from paratrooper
        // rampage, which attacks any worthwhile target after landing.
        self.ai_military_rampage(id, 68, 99999);
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
