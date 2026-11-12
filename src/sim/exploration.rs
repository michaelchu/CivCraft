//! Unit exploration and frontier target selection.

use super::pathfinding::reconstruct_path;
use super::*;

const BEST_NORMAL: i32 = 60_900;
const HUT_SCORE: i32 = BEST_NORMAL + 2;
const BEST_POSSIBLE: i32 = HUT_SCORE + BEST_NORMAL;
const PF_TURN_FACTOR: i64 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExploreResult {
    Okay,
    Paused,
    Done,
    Dead,
}

// utility/specpq.h: equal priorities stay in heap order, rather than FIFO.
// This also replaces a queued node in place when its cost improves.
#[derive(Default)]
pub(super) struct ExplorerQueue(Vec<(i64, usize)>);

impl ExplorerQueue {
    pub(super) fn put(&mut self, cost: i64, tile: usize) {
        if self.0.is_empty() {
            self.0.push((0, 0));
        }
        let mut index = (1..self.0.len())
            .rev()
            .find(|&i| self.0[i].1 == tile)
            .unwrap_or_else(|| {
                self.0.push((i64::MAX, tile));
                self.0.len() - 1
            });
        if cost >= self.0[index].0 {
            return;
        }
        while index > 1 && self.0[index / 2].0 > cost {
            self.0[index] = self.0[index / 2];
            index /= 2;
        }
        self.0[index] = (cost, tile);
    }

    pub(super) fn insert(&mut self, cost: i64, tile: usize) {
        if self.0.is_empty() {
            self.0.push((0, 0));
        }
        let mut index = self.0.len();
        self.0.push((cost, tile));
        while index > 1 && self.0[index / 2].0 > cost {
            self.0[index] = self.0[index / 2];
            index /= 2;
        }
        self.0[index] = (cost, tile);
    }

    pub(super) fn peek(&self) -> Option<(i64, usize)> {
        self.0.get(1).copied()
    }

    pub(super) fn pop(&mut self) -> Option<(i64, usize)> {
        if self.0.len() <= 1 {
            return None;
        }
        let first = self.0[1];
        let last = self.0.pop().unwrap();
        if self.0.len() > 1 {
            let mut index = 1;
            while index * 2 < self.0.len() {
                let mut child = index * 2;
                if child + 1 < self.0.len() && self.0[child + 1].0 < self.0[child].0 {
                    child += 1;
                }
                if self.0[child].0 >= last.0 {
                    break;
                }
                self.0[index] = self.0[child];
                index = child;
            }
            self.0[index] = last;
        }
        Some(first)
    }
}

struct ExplorerRoutes {
    // Native PF chooses by combined movement and unsigned extra costs.
    costs: Vec<(i32, i64)>,
    previous: Vec<Option<(usize, (i32, i32))>>,
}

impl Simulation {
    fn explorer_native(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        if kind.is_aircraft() {
            true
        } else if kind == UnitKind::Galley {
            matches!(tile.terrain, Terrain::Coast | Terrain::Sea | Terrain::Lake) || tile.river
        } else {
            tile.terrain.is_water() == kind.is_naval()
        }
    }

    fn explorer_likely_native(&self, owner: usize, kind: UnitKind, x: i32, y: i32) -> i32 {
        if self.tile(x, y).known_by[owner] {
            return if self.explorer_native(kind, x, y) {
                100
            } else {
                0
            };
        }
        // autoexplorer.c deliberately preserved: the native test uses the
        // unknown CENTER, not the known adjacent tile being iterated.
        let known = self
            .map_wrap
            .civ3_neighbors(x, y)
            .filter(|&(nx, ny)| self.tile(nx, ny).known_by[owner])
            .count() as i32;
        50 + (50 / CIV3_DIRS.len() as i32)
            * known
            * if self.explorer_native(kind, x, y) {
                1
            } else {
                -1
            }
    }

    fn explorer_may_visit(&self, unit: &Unit, x: i32, y: i32) -> bool {
        !self.treaty_blocks_move(unit, x, y)
            && !self
                .units
                .iter()
                .any(|other| (other.x, other.y) == (x, y) && !self.allied(unit.owner, other.owner))
            && !self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && !self.allied(unit.owner, city.owner))
    }

    fn explorer_desirable(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        if (unit.owner == BARBARIANS && self.tile(x, y).hut) || !self.explorer_may_visit(unit, x, y)
        {
            return 0;
        }
        let radius_sq = self.unit_vision_radius_sq_for(unit.owner, unit.kind, x, y);
        let radius = (radius_sq as f64).sqrt().ceil() as i32;
        let mut unknown = 0;
        let mut score = 0;
        for (nx, ny, distance) in self.map_wrap.civ3_radius(x, y, radius) {
            if !self.within_vision_distance_sq(distance, radius_sq) {
                continue;
            }
            let native = self.explorer_likely_native(unit.owner, unit.kind, nx, ny);
            if !self.tile(nx, ny).known_by[unit.owner] {
                unknown += 1;
                score += native * 21 + (100 - native) * 81;
            } else if distance == 1 {
                score += (100 - native) * 51;
            }
        }
        if unknown == 0 {
            score = 0;
        }
        // Normal AI has no H_HUTS handicap. The own-city bonus remains the
        // unimplemented TODO in the reference, rather than a local addition.
        if self.tile(x, y).known_by[unit.owner] && self.tile(x, y).hut && !unit.kind.is_aircraft() {
            score += HUT_SCORE;
        }
        score
    }

    pub(super) fn explorer_stack_risk(&self, unit: &Unit, x: i32, y: i32) -> i64 {
        let tile = self.tile(x, y);
        let stack = if self.cities.iter().any(|city| (city.x, city.y) == (x, y)) {
            0
        } else {
            self.units
                .iter()
                .filter(|other| other.owner == unit.owner && (other.x, other.y) == (x, y))
                .map(|other| other.kind.stats().cost)
                .sum::<i32>()
        };
        let db = 10 + tile.terrain.defense_bonus() / 10
            // chance_killed_at uses this literal integer division, unlike
            // adv_danger_at's multiplicative extras adjustment.
            + self.pf_extra_class_defense(unit.kind, x, y) / 100;
        let probability = if tile.terrain.is_water() { 0.05 } else { 0.15 };
        let allowance = self.unit_move_rate(unit).max(1);
        let fearfulness =
            PF_TURN_FACTOR as f64 / 36.0 * f64::from(MOVE_FRAGMENTS) / f64::from(allowance);
        let mut risk = f64::from(unit.kind.stats().cost + stack) * probability * 10.0
            / f64::from(db)
            * fearfulness;
        // Applied even to units ignoring ZOC: it estimates being delayed,
        // and is not the PF callback that tests legal movement.
        if self.under_foreign_control(unit.owner, x, y, false) {
            risk += (PF_TURN_FACTOR * 20) as f64;
        }
        risk as i64
    }

    fn explorer_step_cost(&self, unit: &Unit, x: i32, y: i32, known_only: bool) -> Option<i32> {
        if !self.explorer_may_visit(unit, x, y)
            || (known_only && !self.tile(x, y).known_by[unit.owner])
            || self.zone_blocks_step(unit, (unit.x, unit.y), (x, y))
        {
            return None;
        }
        if !known_only && !self.players[unit.owner].ai && !self.tile(x, y).known_by[unit.owner] {
            // common/movement.c: highest native terrain cost, doubled when
            // the class cannot enter every terrain type.
            return Some(if unit.kind.is_naval() {
                2 * MOVE_FRAGMENTS
            } else if unit.kind.stats().ignores_terrain {
                4
            } else {
                6 * MOVE_FRAGMENTS
            });
        }
        self.movement_cost_for(unit, x, y)
            .or_else(|| self.boarding_target(unit, x, y).map(|_| MOVE_FRAGMENTS))
    }

    fn explorer_search(
        &self,
        unit: &Unit,
        goal: Option<(i32, i32)>,
    ) -> (Option<(i32, i32)>, ExplorerRoutes) {
        let start = self.map_wrap.index(unit.x, unit.y);
        let rate = self.unit_move_rate(unit).max(1);
        let mut routes = ExplorerRoutes {
            costs: vec![(i32::MAX, i64::MAX); self.tiles.len()],
            previous: vec![None; self.tiles.len()],
        };
        routes.costs[start] = (0, 0);
        let mut queue = ExplorerQueue::default();
        queue.put(0, start);
        let mut best = None;
        let mut best_log = f64::NEG_INFINITY;
        let mut max_dist = i32::MAX;
        let log_df = 0.6_f64.ln();
        let mut actor = unit.clone();
        while let Some((combined, current)) = queue.pop() {
            let (spent, extra) = routes.costs[current];
            if combined != i64::from(spent) * PF_TURN_FACTOR + i64::from(rate) * extra {
                continue;
            }
            let (x, y) = self.map_wrap.position(current);
            if let Some(goal) = goal {
                if (x, y) == goal {
                    return (Some(goal), routes);
                }
            } else if current != start {
                // pf_map_move_costs_iterate(..., FALSE) excludes the start.
                let score = self.explorer_desirable(unit, x, y);
                if score > 0 {
                    let log_score = f64::from(score).ln() + f64::from(spent) * log_df;
                    if log_score > best_log {
                        best_log = log_score;
                        best = Some((x, y));
                        max_dist = (f64::from(spent)
                            + (f64::from(score).ln() - f64::from(BEST_POSSIBLE).ln()) / log_df)
                            as i32;
                    }
                    if spent > max_dist {
                        break;
                    }
                }
            }
            (actor.x, actor.y) = (x, y);
            let remaining = if spent < unit.moves_left {
                unit.moves_left - spent
            } else {
                rate - (spent - unit.moves_left.max(0)) % rate
            };
            for (dx, dy) in CIV3_DIRS {
                let Some((nx, ny)) = self.map_wrap.civ3_offset(x, y, dx, dy) else {
                    continue;
                };
                let Some(cost) = self.explorer_step_cost(&actor, nx, ny, goal.is_none()) else {
                    continue;
                };
                let index = self.map_wrap.index(nx, ny);
                let movement = spent + cost.min(remaining);
                let extra = extra
                    + if goal.is_some() {
                        // Native pf_normal_node.extra_tile caches EC in an
                        // unsigned short, including the literal truncation.
                        i64::from(self.explorer_stack_risk(unit, nx, ny) as u16)
                    } else {
                        0
                    };
                let next = i64::from(movement) * PF_TURN_FACTOR + i64::from(rate) * extra;
                let old = routes.costs[index];
                if old.0 == i32::MAX
                    || next < i64::from(old.0) * PF_TURN_FACTOR + i64::from(rate) * old.1
                {
                    routes.costs[index] = (movement, extra);
                    routes.previous[index] = Some((current, (dx, dy)));
                    queue.put(next, index);
                }
            }
        }
        (if goal.is_none() { best } else { None }, routes)
    }

    pub(super) fn explorer_danger_at(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && self.allied(unit.owner, city.owner))
            && !self
                .units
                .iter()
                .any(|other| (other.x, other.y) == (x, y) && !self.allied(unit.owner, other.owner))
        {
            return false;
        }
        let stats = unit.kind.stats();
        let defense = percent_of(stats.defense * POWER_FACTOR, unit.veteran_power())
            * unit.hp
            * stats.firepower
            / (POWER_FACTOR * 3);
        let mut db = 10 + self.tile(x, y).terrain.defense_bonus() / 10;
        db += db * self.pf_extra_class_defense(unit.kind, x, y) / 100;
        let mut attack = 0_i64;
        for enemy in &self.units {
            if self.map_distance(x, y, enemy.x, enemy.y) != 1
                || !self.at_war(unit.owner, enemy.owner)
                || enemy.kind.stats().attack <= 0
                || (!self.players[unit.owner].ai && !self.tile(enemy.x, enemy.y).visible)
                || !self.pf_physical_attack(enemy, unit.kind, x, y)
                || (self
                    .units
                    .iter()
                    .any(|target| (target.x, target.y) == (x, y) && target.transport.is_none())
                    && !self.units.iter().any(|target| {
                        (target.x, target.y) == (x, y)
                            && target.transport.is_none()
                            && self.pf_physical_attack(enemy, target.kind, x, y)
                    }))
            {
                continue;
            }
            let rating = percent_of(
                enemy.kind.stats().attack * POWER_FACTOR,
                enemy.veteran_power(),
            ) * enemy.hp
                * enemy.kind.stats().firepower
                / (POWER_FACTOR * 3);
            attack += i64::from(rating);
            if attack * attack * 10 >= i64::from(defense) * i64::from(defense) * i64::from(db) {
                return true;
            }
        }
        false
    }

    fn follow_explorer_goal(&mut self, unit: &Unit, goal: (i32, i32)) {
        let id = unit.id;
        let (reachable, routes) = self.explorer_search(unit, Some(goal));
        if reachable.is_none() {
            return;
        }
        let start = self.map_wrap.index(unit.x, unit.y);
        let cursor = self.map_wrap.index(goal.0, goal.1);
        let Some(path) = reconstruct_path(&routes.previous, start, cursor) else {
            return;
        };
        if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
            actor.state = UnitState::Idle;
        }
        for (dx, dy) in path {
            let Some(actor) = self.unit_by_id(id).cloned() else {
                return;
            };
            let Some((nx, ny)) = self.map_wrap.civ3_offset(actor.x, actor.y, dx, dy) else {
                continue;
            };
            if actor.moves_left <= 0
                || !self.explorer_may_visit(&actor, nx, ny)
                || (actor.owner == BARBARIANS && self.tile(nx, ny).hut)
            {
                break;
            }
            if let Some(cost) = self.movement_cost_for(&actor, nx, ny)
                && actor.moves_left <= cost
                && self.unit_move_rate(&actor) > cost
                && self.explorer_danger_at(&actor, nx, ny)
                && !self.explorer_danger_at(&actor, actor.x, actor.y)
            {
                break;
            }
            let Some(direction) = CIV3_DIRS.iter().position(|&step| step == (dx, dy)) else {
                break;
            };
            if self.try_move_unit_civ3(id, direction) == MoveResult::Blocked {
                break;
            }
        }
        // adv_follow_path restores the caller's activity after temporary GOTO.
        if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
            actor.state = unit.state;
        }
    }

    pub(super) fn run_idle_ai_ferry(&mut self, id: u32) {
        if self.run_auto_explorer(id) == ExploreResult::Dead {
            return;
        }
        let Some(unit) = self
            .units
            .iter()
            .find(|unit| unit.id == id && unit.moves_left > 0)
            .cloned()
        else {
            return;
        };
        let mut best = i32::MAX;
        let mut home = None;
        for (cost, x, y) in self.ai_movement_cost_order(&unit) {
            if cost > best {
                break;
            }
            let Some(city) = self
                .cities
                .iter()
                .find(|city| (city.x, city.y) == (x, y) && self.allied(unit.owner, city.owner))
            else {
                continue;
            };
            // find_nearest_safe_city is a name, not a threat filter: safety
            // remains native's TODO. Sea/Trireme HP_Regen comes from the port.
            let regeneration = if unit.kind.is_naval() {
                city.has_building(Building::PORT_FACILITY)
            } else if unit.kind.is_aircraft() {
                city.has_building(Building::AIRPORT)
            } else {
                self.active_barracks(city) || self.tile(x, y).base == Some(Base::Fortress)
            };
            let score = cost * if regeneration { 1 } else { 3 };
            if score < best {
                best = score;
                home = Some((x, y));
            }
        }
        if let Some((x, y)) = home {
            // The ferry advisor gives up its job before returning home.
            self.release_ai_ferry(id);
            self.follow_explorer_goal(&unit, (x, y));
        }
    }

    pub(super) fn run_auto_explorer(&mut self, id: u32) -> ExploreResult {
        // Native recursion is repeated only after actual displacement. Every
        // movement costs a positive fragment, so this iterative form has the
        // same stopping condition without consuming the Rust call stack.
        loop {
            let Some(unit) = self.unit_by_id(id).cloned() else {
                return ExploreResult::Dead;
            };
            if unit.moves_left <= 0 {
                return ExploreResult::Okay;
            }
            let Some(goal) = self.explorer_search(&unit, None).0 else {
                return ExploreResult::Done;
            };
            // autoexplorer.c:explorer_goto sets this before the temporary
            // adv_follow_path activity, which restores this same destination.
            self.unit_goto_tiles.insert(id, goal);
            self.follow_explorer_goal(&unit, goal);
            let Some(actor) = self.unit_by_id(id) else {
                return ExploreResult::Dead;
            };
            if actor.moves_left <= 0 {
                return ExploreResult::Okay;
            }
            if (actor.x, actor.y) == (unit.x, unit.y) {
                return ExploreResult::Paused;
            }
        }
    }
}

#[cfg(test)]
#[path = "exploration/tests.rs"]
mod tests;
