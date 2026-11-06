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
        } else if kind == UnitKind::Trireme {
            matches!(tile.terrain, Terrain::Ocean | Terrain::Lake) || tile.river
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
            .neighbors(x, y)
            .filter(|&(nx, ny)| self.tile(nx, ny).known_by[owner])
            .count() as i32;
        50 + (50 / ISOHEX_DIRS.len() as i32)
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
        let radius_sq = unit.kind.stats().vision_sq;
        let radius = (radius_sq as f64).sqrt().ceil() as i32;
        let mut unknown = 0;
        let mut score = 0;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let distance = isohex_vector_distance(dx, dy);
                if distance * distance > radius_sq {
                    continue;
                }
                let Some((nx, ny)) = self.map_wrap.offset(x, y, dx, dy) else {
                    continue;
                };
                let native = self.explorer_likely_native(unit.owner, unit.kind, nx, ny);
                if !self.tile(nx, ny).known_by[unit.owner] {
                    unknown += 1;
                    score += native * 21 + (100 - native) * 81;
                } else if distance == 1 {
                    score += (100 - native) * 51;
                }
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
            for (dx, dy) in ISOHEX_DIRS {
                let Some((nx, ny)) = self.map_wrap.offset(x, y, dx, dy) else {
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
        let mut cursor = self.map_wrap.index(goal.0, goal.1);
        let mut path = Vec::new();
        while cursor != start {
            let Some((parent, step)) = routes.previous[cursor] else {
                return;
            };
            path.push(step);
            cursor = parent;
        }
        path.reverse();
        if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
            actor.state = UnitState::Idle;
        }
        for (dx, dy) in path {
            let Some(actor) = self.units.iter().find(|unit| unit.id == id).cloned() else {
                return;
            };
            let Some((nx, ny)) = self.map_wrap.offset(actor.x, actor.y, dx, dy) else {
                continue;
            };
            if actor.moves_left <= 0
                || !self.explorer_may_visit(&actor, nx, ny)
                || (actor.owner == BARBARIANS && self.tile(nx, ny).hut)
            {
                break;
            }
            if let Some(cost) = self.movement_cost_for(&actor, nx, ny) {
                if actor.moves_left <= cost
                    && self.unit_move_rate(&actor) > cost
                    && self.explorer_danger_at(&actor, nx, ny)
                    && !self.explorer_danger_at(&actor, actor.x, actor.y)
                {
                    break;
                }
            }
            if self.try_move_unit(id, dx, dy) == MoveResult::Blocked {
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

    pub(crate) fn ai_idle_exploration_playtest(&mut self) -> bool {
        let mut explored = true;
        for kind in [UnitKind::Warrior, UnitKind::Caravel] {
            self.units.clear();
            self.cities.clear();
            self.prune_unit_ai_state();
            for tile in &mut self.tiles {
                tile.terrain = if kind.is_naval() {
                    Terrain::Ocean
                } else {
                    Terrain::Grassland
                };
                tile.hut = false;
                tile.owner = None;
                tile.road = false;
                tile.railroad = false;
                tile.river = false;
                tile.known_by.fill(false);
            }
            let id = self.spawn_unit(1, kind, 10, 20);
            self.refresh_visibility();
            let before = self.tiles.iter().filter(|tile| tile.known_by[1]).count();
            self.run_ai_player(1);
            explored &= self
                .units
                .iter()
                .any(|unit| unit.id == id && (unit.x, unit.y) != (10, 20))
                && self.tiles.iter().filter(|tile| tile.known_by[1]).count() > before;
        }
        explored
    }

    pub(crate) fn ai_exploration_legality_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(false);
        }
        let mut rejected = true;
        for kind in [
            UnitKind::Fighter,
            UnitKind::Bomber,
            UnitKind::StealthFighter,
            UnitKind::StealthBomber,
            UnitKind::CruiseMissile,
            UnitKind::Nuclear,
            UnitKind::Helicopter,
        ] {
            let id = self.spawn_unit(HUMAN, kind, 10, 20);
            rejected &= !self.apply(Command::Automate(id));
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .automated = true;
            rejected &= self.apply(Command::Automate(id));
        }
        self.units.clear();
        self.prune_unit_ai_state();
        let mut explored = true;
        for kind in [UnitKind::Warrior, UnitKind::Caravel] {
            for tile in &mut self.tiles {
                tile.terrain = if kind.is_naval() {
                    Terrain::Ocean
                } else {
                    Terrain::Grassland
                };
                tile.known_by[HUMAN] = false;
            }
            let id = self.spawn_unit(HUMAN, kind, 10, 20);
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .state = UnitState::Sentry;
            explored &= self.apply(Command::Automate(id));
            self.refresh_visibility();
            let before = self
                .tiles
                .iter()
                .filter(|tile| tile.known_by[HUMAN])
                .count();
            self.run_automated_unit(id);
            explored &= self
                .units
                .iter()
                .any(|unit| unit.id == id && (unit.x, unit.y) != (10, 20))
                && self
                    .tiles
                    .iter()
                    .filter(|tile| tile.known_by[HUMAN])
                    .count()
                    > before;
            self.units.clear();
            self.prune_unit_ai_state();
        }
        rejected && explored
    }

    pub(crate) fn ai_exploration_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(false);
        }
        let ai = self.spawn_unit(1, UnitKind::Explorer, 10, 20);
        self.tile_mut(10, 20).known_by[1] = true;
        let constrained = self.run_auto_explorer(ai) == ExploreResult::Done;
        self.units.clear();
        let human = self.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        self.refresh_visibility();
        let before = self
            .tiles
            .iter()
            .filter(|tile| tile.known_by[HUMAN])
            .count();
        let automated = self.apply(Command::Automate(human));
        self.run_automated_unit(human);
        let moved = self.units.iter().any(|unit| {
            unit.id == human
                && unit.automated
                && unit.moves_left == 0
                && (unit.x, unit.y) != (10, 20)
        });
        let discovered = self
            .tiles
            .iter()
            .filter(|tile| tile.known_by[HUMAN])
            .count()
            > before;
        for tile in &mut self.tiles {
            tile.known_by[HUMAN] = true;
        }
        self.units
            .iter_mut()
            .find(|unit| unit.id == human)
            .unwrap()
            .moves_left = MOVE_FRAGMENTS;
        self.run_automated_unit(human);
        constrained
            && automated
            && moved
            && discovered
            && self
                .units
                .iter()
                .any(|unit| unit.id == human && !unit.automated)
    }

    pub(super) fn run_auto_explorer(&mut self, id: u32) -> ExploreResult {
        // Native recursion is repeated only after actual displacement. Every
        // movement costs a positive fragment, so this iterative form has the
        // same stopping condition without consuming the Rust call stack.
        loop {
            let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
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
            let Some(actor) = self.units.iter().find(|unit| unit.id == id) else {
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
mod tests {
    use super::*;

    fn world() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.known_by.fill(false);
        }
        game
    }

    #[test]
    fn human_warriors_and_ships_explore_but_fuelled_and_losing_hp_types_cannot_activate_it() {
        assert!(world().ai_exploration_legality_playtest());
    }

    #[test]
    fn idle_ai_military_and_ferries_use_frontier_exploration() {
        assert!(world().ai_idle_exploration_playtest());
    }

    #[test]
    fn nonworker_autoexploration_does_not_start_unsolicited_cleanup() {
        let mut game = world();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
        }
        let ship = game.spawn_unit(HUMAN, UnitKind::Caravel, 10, 20);
        game.tile_mut(10, 20).pollution = true;
        game.refresh_visibility();
        assert!(game.apply(Command::Automate(ship)));
        game.run_automated_unit(ship);
        let unit = game.units.iter().find(|unit| unit.id == ship).unwrap();
        assert!(unit.work_kind.is_none());
        assert_ne!((unit.x, unit.y), (10, 20));
        assert!(game.tile(10, 20).pollution);
    }

    #[test]
    fn idle_ferry_recovery_uses_allied_ports_and_native_regeneration_penalty() {
        let mut game = world();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.known_by[1] = true;
        }
        let ship = game.spawn_unit(1, UnitKind::Caravel, 10, 20);
        let near = offset_native_by_map(10, 20, 1, 0);
        let far = offset_native_by_map(10, 20, 0, 2);
        game.sign_pact(1, 2, Pact::Alliance);
        for (owner, point) in [(1, near), (2, far)] {
            game.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            game.create_city(owner, point.0, point.1);
        }
        game.cities
            .iter_mut()
            .find(|city| (city.x, city.y) == far)
            .unwrap()
            .buildings
            .push(Building::PORT_FACILITY);
        game.run_idle_ai_ferry(ship);
        let unit = game.units.iter().find(|unit| unit.id == ship).unwrap();
        assert_eq!((unit.x, unit.y), far);
    }

    #[test]
    fn explorer_stack_risk_uses_current_damaged_unit_speed() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Engineer, 10, 20);
        let mut actor = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let target = offset_native_by_map(10, 20, 1, 0);
        let healthy = game.explorer_stack_risk(&actor, target.0, target.1);
        actor.hp = actor.kind.stats().hp / 2;
        assert_eq!(game.unit_move_rate(&actor), MOVE_FRAGMENTS);
        assert_eq!(
            game.explorer_stack_risk(&actor, target.0, target.1),
            healthy * 2
        );
    }

    #[test]
    fn native_start_ignore_can_leave_a_forbidden_start_without_entering_forbidden_destinations() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy_playtest_tick(16);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        game.tile_mut(10, 20).owner = Some(1);
        let next = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(10, 20).known_by[HUMAN] = true;
        game.tile_mut(next.0, next.1).known_by[HUMAN] = true;
        let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert!(!game.explorer_may_visit(unit, 10, 20));
        assert_eq!(game.explorer_search(unit, None).0, Some(next));
        assert_eq!(game.run_auto_explorer(id), ExploreResult::Okay);
        let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!((unit.x, unit.y), next);
    }

    #[test]
    fn civilian_explorers_spies_and_ferries_cross_peace_borders_but_warriors_and_carriers_do_not() {
        let mut game = world();
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy_playtest_tick(16);
        game.tile_mut(10, 20).owner = Some(1);
        for kind in [
            UnitKind::Explorer,
            UnitKind::Spy,
            UnitKind::Caravel,
            UnitKind::Warrior,
            UnitKind::Carrier,
        ] {
            let id = game.spawn_unit(HUMAN, kind, 8, 18);
            let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
            assert_eq!(game.explorer_may_visit(actor, 10, 20), kind.is_civilian());
        }
    }

    #[test]
    fn native_heap_ties_and_unsigned_short_extra_cost_are_preserved() {
        let mut queue = ExplorerQueue::default();
        for tile in 0..4 {
            queue.put(10, tile);
        }
        assert_eq!(
            (0..4).map(|_| queue.pop().unwrap().1).collect::<Vec<_>>(),
            vec![0, 3, 2, 1]
        );
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        let actor = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let tile = offset_native_by_map(10, 20, 1, 0);
        let baseline = game.explorer_stack_risk(&actor, tile.0, tile.1);
        let adjacent = offset_native_by_map(tile.0, tile.1, 1, 0);
        game.spawn_unit(1, UnitKind::Warrior, adjacent.0, adjacent.1);
        let controlled = game.explorer_stack_risk(&actor, tile.0, tile.1);
        assert_eq!(controlled - baseline, PF_TURN_FACTOR * 20);
        assert_eq!(controlled as u16, baseline as u16);
    }

    #[test]
    fn likely_native_keeps_reference_unknown_center_quirk() {
        let mut game = world();
        let point = (10, 20);
        for (x, y) in terrain_work::neighbors(point.0, point.1) {
            game.tile_mut(x, y).known_by[HUMAN] = true;
        }
        assert_eq!(
            game.explorer_likely_native(HUMAN, UnitKind::Explorer, point.0, point.1),
            98
        );
        game.tile_mut(point.0, point.1).terrain = Terrain::Ocean;
        assert_eq!(
            game.explorer_likely_native(HUMAN, UnitKind::Explorer, point.0, point.1),
            2
        );
    }

    #[test]
    fn scoring_requires_unknown_sight_but_known_huts_override_it() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let unit = game.units.iter().find(|u| u.id == id).unwrap().clone();
        for tile in &mut game.tiles {
            tile.known_by[HUMAN] = true;
        }
        assert_eq!(game.explorer_desirable(&unit, 12, 24), 0);
        game.tile_mut(12, 24).hut = true;
        assert_eq!(game.explorer_desirable(&unit, 12, 24), 60_902);
        game.tile_mut(12, 24).known_by[HUMAN] = false;
        assert!(game.explorer_desirable(&unit, 12, 24) < HUT_SCORE);
    }

    #[test]
    fn first_search_never_targets_unknown_even_for_omniscient_ai() {
        let mut game = world();
        let id = game.spawn_unit(1, UnitKind::Explorer, 10, 20);
        let unit = game.units.iter().find(|u| u.id == id).unwrap().clone();
        for tile in &mut game.tiles {
            tile.known_by[1] = false;
        }
        game.tile_mut(10, 20).known_by[1] = true;
        assert!(game.explorer_search(&unit, None).0.is_none());
        let next = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(next.0, next.1).known_by[1] = true;
        assert_eq!(game.explorer_search(&unit, None).0, Some(next));
    }

    #[test]
    fn exploration_never_attacks_or_enters_nonallied_cities_or_peace_borders() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let next = offset_native_by_map(10, 20, 1, 0);
        let enemy = game.spawn_unit(1, UnitKind::Warrior, next.0, next.1);
        let unit = game.units.iter().find(|u| u.id == id).unwrap().clone();
        assert!(!game.explorer_may_visit(&unit, next.0, next.1));
        game.units.retain(|u| u.id != enemy);
        game.create_city(1, next.0, next.1);
        assert!(!game.explorer_may_visit(&unit, next.0, next.1));
        game.cities.clear();
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy_playtest_tick(16);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        game.tile_mut(next.0, next.1).owner = Some(1);
        assert!(!game.explorer_may_visit(&unit, next.0, next.1));
    }

    #[test]
    fn second_search_avoids_an_expensive_vulnerable_stack() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        let unit = game.units.iter().find(|u| u.id == id).unwrap().clone();
        for tile in &mut game.tiles {
            tile.known_by[HUMAN] = true;
        }
        let stacked = offset_native_by_map(10, 20, 1, 0);
        let goal = offset_native_by_map(10, 20, 2, 0);
        game.spawn_unit(HUMAN, UnitKind::Battleship, stacked.0, stacked.1);
        let (_, routes) = game.explorer_search(&unit, Some(goal));
        let mut cursor = Simulation::idx(goal.0, goal.1);
        while cursor != Simulation::idx(10, 20) {
            assert_ne!(cursor, Simulation::idx(stacked.0, stacked.1));
            cursor = routes.previous[cursor].unwrap().0;
        }
        assert!(
            game.explorer_stack_risk(&unit, stacked.0, stacked.1)
                > game.explorer_stack_risk(&unit, goal.0, goal.1)
        );
    }

    #[test]
    fn paused_exploration_clears_human_automation_without_a_dangerous_last_step() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        let next = offset_native_by_map(10, 20, 1, 0);
        let enemy = offset_native_by_map(10, 20, 2, 0);
        game.spawn_unit(1, UnitKind::Warrior, enemy.0, enemy.1);
        game.tile_mut(10, 20).known_by[HUMAN] = true;
        game.tile_mut(next.0, next.1).known_by[HUMAN] = true;
        game.tile_mut(enemy.0, enemy.1).visible = true;
        let actor = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        actor.moves_left = 2;
        actor.automated = true;
        game.run_automated_unit(id);
        let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!((actor.x, actor.y), (10, 20));
        assert_eq!(actor.moves_left, 2);
        assert!(!actor.automated);
    }

    #[test]
    fn end_of_turn_danger_uses_native_combined_attack_and_terrain_defense() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        let next = offset_native_by_map(10, 20, 1, 0);
        let enemy_tile = offset_native_by_map(10, 20, 2, 0);
        game.spawn_unit(1, UnitKind::Warrior, enemy_tile.0, enemy_tile.1);
        game.refresh_visibility();
        let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert!(!game.explorer_danger_at(unit, 10, 20));
        assert!(game.explorer_danger_at(unit, next.0, next.1));
        // A city is a shelter regardless of the adjacent enemy's strength.
        game.create_city(HUMAN, next.0, next.1);
        let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert!(!game.explorer_danger_at(unit, next.0, next.1));
    }

    #[test]
    fn desktop_exploration_fixture_uses_real_automation_and_movement() {
        let mut game = world();
        assert!(game.ai_exploration_playtest());
    }

    #[test]
    fn reaching_a_frontier_recurses_and_reveals_more_without_random_wandering() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        game.refresh_visibility();
        let before = game
            .tiles
            .iter()
            .filter(|tile| tile.known_by[HUMAN])
            .count();
        let result = game.run_auto_explorer(id);
        assert_eq!(result, ExploreResult::Okay);
        let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_ne!((actor.x, actor.y), (10, 20));
        assert_eq!(actor.moves_left, 0);
        assert!(
            game.tiles
                .iter()
                .filter(|tile| tile.known_by[HUMAN])
                .count()
                > before
        );
    }
}
