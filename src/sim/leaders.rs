use super::exploration::ExplorerQueue;
use super::pf_actions::PfAction;
use super::*;

struct LeaderPaths {
    costs: Vec<i32>,
    previous: Vec<Option<(usize, (i32, i32))>>,
    order: Vec<usize>,
    start: usize,
}

enum EnemyDistances {
    Normal(super::normal_paths::NormalPaths),
    Fuel(super::fuel_paths::FuelPaths),
}

impl EnemyDistances {
    fn cost(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        match self {
            Self::Normal(paths) => paths.movement_cost(sim, x, y),
            Self::Fuel(paths) => paths.movement_cost(sim, x, y),
        }
    }
}

impl LeaderPaths {
    fn path(&self, target: usize) -> Option<Vec<(i32, i32)>> {
        if self.costs[target] == i32::MAX {
            return None;
        }
        let mut cursor = target;
        let mut path = Vec::new();
        while cursor != self.start {
            let (parent, step) = self.previous[cursor]?;
            path.push(step);
            cursor = parent;
        }
        path.reverse();
        Some(path)
    }
}

impl Simulation {
    fn leader_nonallied_at(&self, owner: usize, x: i32, y: i32) -> bool {
        self.units
            .iter()
            .any(|unit| (unit.x, unit.y) == (x, y) && !self.allied(owner, unit.owner))
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner))
    }

    // daiunit.c's reverse query is deliberately NOT an ordinary enemy route:
    // its owner is the leader; it has no ZOC/occupant callback, assumes full
    // enemy movement, and considers the target attackable even for transports.
    fn leader_paths(&self, unit: &Unit, reverse: Option<(usize, usize)>) -> LeaderPaths {
        self.leader_paths_mode(unit, reverse, false)
    }

    fn leader_paths_mode(
        &self,
        unit: &Unit,
        reverse: Option<(usize, usize)>,
        observed: bool,
    ) -> LeaderPaths {
        let rate = self.unit_move_rate(unit);
        let mut actor = unit.clone();
        if let Some((_, owner)) = reverse {
            actor.owner = owner;
        }
        let initial = if reverse.is_some() {
            rate
        } else {
            unit.moves_left.max(0)
        };
        let start = Self::idx(unit.x, unit.y);
        let mut result = LeaderPaths {
            costs: vec![i32::MAX; self.tiles.len()],
            previous: vec![None; self.tiles.len()],
            order: Vec::new(),
            start,
        };
        // Reverse PF rejects rawcost >= rate*(3+1), including its own
        // initial zero-cost position when the candidate's rate is zero.
        if reverse.is_some() && rate == 0 {
            return result;
        }
        result.costs[start] = 0;
        let mut queue = ExplorerQueue::default();
        queue.put(0, start);
        while let Some((spent, current)) = queue.pop() {
            let spent = spent as i32;
            if result.costs[current] != spent {
                continue;
            }
            if reverse.is_some() && spent >= 4 * rate {
                break;
            }
            result.order.push(current);
            if reverse.is_some_and(|(target, _)| current == target) {
                break;
            }
            let (x, y) = Self::xy(current);
            let known_here = !observed || self.tile(x, y).known_by[actor.owner];
            let action_here = if reverse.is_some_and(|(target, _)| current == target) {
                PfAction::Attack
            } else if reverse.is_some() {
                PfAction::None
            } else {
                self.pf_default_action(
                    actor.kind,
                    actor.owner,
                    x,
                    y,
                    known_here,
                    !observed || self.pf_main_sight(actor.owner, x, y),
                    !observed,
                )
            };
            if !matches!(action_here, PfAction::None | PfAction::Impossible) {
                continue;
            }
            if rate == 0 && spent >= initial {
                continue;
            }
            (actor.x, actor.y) = (x, y);
            let remaining = if spent < initial {
                initial - spent
            } else {
                rate - (spent - initial) % rate
            };
            for (dx, dy) in ISOHEX_DIRS {
                let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                let next = Self::idx(nx, ny);
                let target = reverse.is_some_and(|(target, _)| next == target);
                let known = !observed || self.tile(nx, ny).known_by[actor.owner];
                if known
                    && !actor.kind.is_civilian()
                    && self
                        .tile(nx, ny)
                        .owner
                        .is_some_and(|owner| self.pact(actor.owner, owner) == Pact::Peace)
                    || (reverse.is_none()
                        && known
                        && (!observed || self.tile(x, y).known_by[actor.owner])
                        && self.zone_blocks_step(&actor, (x, y), (nx, ny)))
                {
                    continue;
                }
                let action = if target {
                    PfAction::Attack
                } else if reverse.is_some() {
                    PfAction::None
                } else {
                    self.pf_default_action(
                        actor.kind,
                        actor.owner,
                        nx,
                        ny,
                        known,
                        !observed || self.pf_main_sight(actor.owner, nx, ny),
                        !observed,
                    )
                };
                if action == PfAction::Impossible {
                    continue;
                }
                let cost = if action != PfAction::None {
                    if reverse.is_none()
                        && known_here
                        && !self.pf_action_step_possible(&actor, action)
                    {
                        continue;
                    }
                    Self::pf_action_move_cost(actor.kind, action, rate)
                } else if !known {
                    if unit.kind.is_naval() {
                        2 * MOVE_FRAGMENTS
                    } else if unit.kind.stats().ignores_terrain {
                        4
                    } else {
                        6 * MOVE_FRAGMENTS
                    }
                } else {
                    let Some(cost) = self
                        .movement_cost_for(&actor, nx, ny)
                        .or_else(|| self.boarding_target(&actor, nx, ny).map(|_| MOVE_FRAGMENTS))
                    else {
                        continue;
                    };
                    cost
                };
                let candidate = spent + cost.min(remaining);
                if candidate < result.costs[next] && (reverse.is_none() || candidate < 4 * rate) {
                    result.costs[next] = candidate;
                    result.previous[next] = Some((current, (dx, dy)));
                    queue.put(i64::from(candidate), next);
                }
            }
        }
        result
    }

    pub(super) fn ai_movement_cost_order(&self, unit: &Unit) -> Vec<(i32, i32, i32)> {
        let paths = self.leader_paths(unit, None);
        paths
            .order
            .into_iter()
            .map(|index| {
                let (x, y) = Self::xy(index);
                (paths.costs[index], x, y)
            })
            .collect()
    }

    fn leader_follow_path(&mut self, id: u32, path: Vec<(i32, i32)>) {
        let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) else {
            return;
        };
        let activity = unit.state;
        unit.state = UnitState::Idle;
        for (dx, dy) in path {
            let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
                break;
            };
            if unit.moves_left <= 0 {
                break;
            }
            let (nx, ny) = offset_native_by_map(unit.x, unit.y, dx, dy);
            if self.leader_nonallied_at(unit.owner, nx, ny)
                || (unit.owner == BARBARIANS && self.tile(nx, ny).hut)
            {
                break;
            }
            if let Some(cost) = self.movement_cost_for(&unit, nx, ny) {
                if unit.moves_left <= cost
                    && self.unit_move_rate(&unit) > cost
                    && self.explorer_danger_at(&unit, nx, ny)
                    && !self.explorer_danger_at(&unit, unit.x, unit.y)
                {
                    break;
                }
            }
            if self.try_move_unit(id, dx, dy) == MoveResult::Blocked {
                break;
            }
        }
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
            unit.state = activity;
        }
    }

    pub(crate) fn ai_normal_path_actions_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.known_by.fill(true);
        }
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let blocker = offset_native_by_map(10, 20, 1, 0);
        let refuge = offset_native_by_map(10, 20, 2, 0);
        self.spawn_unit(HUMAN, UnitKind::Warrior, blocker.0, blocker.1);
        self.spawn_unit(BARBARIANS, UnitKind::Worker, refuge.0, refuge.1);
        let actor = self.units.iter().find(|unit| unit.id == leader).unwrap();
        let optimistic = self.leader_paths(actor, None).costs[Self::idx(refuge.0, refuge.1)]
            == 2 * MOVE_FRAGMENTS;
        self.run_ai_player(BARBARIANS);
        let stopped = self.units.iter().any(|unit| {
            unit.id == leader
                && (unit.x, unit.y) == (10, 20)
                && unit.moves_left == 2 * MOVE_FRAGMENTS
        });
        self.units.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
        self.spawn_unit(BARBARIANS, UnitKind::Caravel, blocker.0, blocker.1);
        let actor = self.units.iter().find(|unit| unit.id == carrier).unwrap();
        let paths = self.leader_paths(actor, None);
        let endpoint = paths.costs[Self::idx(blocker.0, blocker.1)] == MOVE_FRAGMENTS
            && paths.costs[Self::idx(refuge.0, refuge.1)] == 3 * MOVE_FRAGMENTS;
        let execution = !self.apply(Command::Move(carrier, 1, 0))
            && self
                .units
                .iter()
                .any(|unit| unit.id == carrier && (unit.x, unit.y) == (10, 20));
        optimistic && stopped && endpoint && execution
    }

    pub(crate) fn ai_barbarian_leader_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let refuge = offset_native_by_map(10, 20, 2, 0);
        self.spawn_unit(BARBARIANS, UnitKind::Worker, refuge.0, refuge.1);
        self.run_ai_player(BARBARIANS);
        let sheltered = self
            .units
            .iter()
            .any(|unit| unit.id == leader && (unit.x, unit.y) == refuge);
        self.units.clear();
        self.prune_unit_ai_state();
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let enemy = offset_native_by_map(10, 20, -1, 0);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, enemy.0, enemy.1);
        self.run_ai_player(BARBARIANS);
        sheltered
            && self.units.iter().any(|unit| {
                unit.id == leader
                    && unit.moves_left == 0
                    && self.map_distance(unit.x, unit.y, enemy.0, enemy.1) > 1
            })
    }

    pub(super) fn run_barbarian_leader(&mut self, id: u32) {
        let Some(leader) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return;
        };
        let position = (leader.x, leader.y);
        if leader.moves_left == 0
            || (!self.tile(position.0, position.1).terrain.is_water()
                && self
                    .units
                    .iter()
                    .filter(|unit| (unit.x, unit.y) == position)
                    .count()
                    > 1)
        {
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .state = UnitState::Sentry;
            return;
        }
        let boat = self
            .ai_ferries
            .boats
            .iter()
            .find_map(|(&boat, _)| (self.ferry_boss(boat) == Some(id)).then_some(boat));
        if let Some(boat) = boat {
            self.release_ai_ferry(id);
            let warrior = self
                .tile_units_ordered(position.0, position.1)
                .find(|unit| {
                    (unit.x, unit.y) == position
                        && unit.kind != UnitKind::BarbarianLeader
                        && unit.kind.transport_capacity() == 0
                        && unit.moves_left > 0
                })
                .map(|unit| unit.id);
            if let Some(warrior) = warrior {
                self.assign_ai_ferry(boat, warrior);
                self.run_ai_ferry_after_handoff(boat);
                return;
            }
        }

        let guards: Vec<_> = self
            .player_units_ordered(leader.owner)
            .filter(|unit| unit.owner == leader.owner && unit.kind != UnitKind::BarbarianLeader)
            .filter(|unit| {
                let paths = self.leader_paths_mode(unit, None, true);
                paths.costs[Self::idx(leader.x, leader.y)] != i32::MAX
            })
            .map(|unit| unit.id)
            .collect();
        if !guards.is_empty() {
            let paths = self.leader_paths(&leader, None);
            for &tile in paths.order.iter().filter(|&&tile| tile != paths.start) {
                let point = Self::xy(tile);
                if self
                    .units
                    .iter()
                    .any(|unit| guards.contains(&unit.id) && (unit.x, unit.y) == point)
                {
                    if let Some(path) = paths.path(tile) {
                        self.leader_follow_path(id, path);
                    }
                    return;
                }
            }
        }

        let mut worst = None;
        let mut minimum = i32::MAX;
        for owner in 0..self.players.len() {
            for enemy in self
                .player_units_ordered(owner)
                .filter(|unit| unit.owner == owner && unit.owner != leader.owner)
            {
                let index = Self::idx(leader.x, leader.y);
                let cost = self.leader_paths(enemy, Some((index, leader.owner))).costs[index];
                if cost < minimum {
                    minimum = cost;
                    worst = Some(enemy.clone());
                }
            }
        }
        let Some(enemy) = worst else {
            self.units
                .iter_mut()
                .find(|unit| unit.id == id)
                .unwrap()
                .state = UnitState::Idle;
            return;
        };
        // The initial reverse query deliberately stayed PF_NORMAL; only
        // the selected enemy's ordinary map uses native fuel constraints.
        let mut enemy_paths =
            if let Some(paths) = super::fuel_paths::FuelPaths::new(self, &enemy, true) {
                EnemyDistances::Fuel(paths)
            } else {
                EnemyDistances::Normal(super::normal_paths::NormalPaths::enemy_map(self, &enemy))
            };
        let mut safest_cost = enemy_paths.cost(self, leader.x, leader.y).unwrap_or(-1);
        loop {
            let Some(actor) = self.units.iter().find(|unit| unit.id == id).cloned() else {
                return;
            };
            if actor.moves_left <= 0 {
                return;
            }
            let mut safest = (actor.x, actor.y);
            for (dx, dy) in ISOHEX_DIRS {
                let (x, y) = offset_native_by_map(actor.x, actor.y, dx, dy);
                if self.treaty_blocks_move(&actor, x, y)
                    || self.leader_nonallied_at(actor.owner, x, y)
                    || self.zone_blocks_step(&actor, (actor.x, actor.y), (x, y))
                    || self.movement_cost_for(&actor, x, y).is_none()
                {
                    continue;
                }
                if let Some(cost) = enemy_paths.cost(self, x, y)
                    && cost > safest_cost
                {
                    safest_cost = cost;
                    safest = (x, y);
                }
            }
            if safest == (actor.x, actor.y) {
                self.units
                    .iter_mut()
                    .find(|unit| unit.id == id)
                    .unwrap()
                    .state = UnitState::Idle;
                return;
            }
            let paths = self.leader_paths(&actor, None);
            if let Some(path) = paths.path(Self::idx(safest.0, safest.1)) {
                self.leader_follow_path(id, path);
            }
            if self
                .units
                .iter()
                .find(|unit| unit.id == id)
                .is_none_or(|unit| (unit.x, unit.y) == (actor.x, actor.y))
            {
                return;
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
        game.prune_unit_ai_state();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        game
    }

    #[test]
    fn nonmil_traversal_and_zero_attack_carrier_endpoints_preserve_real_execution_legality() {
        assert!(world().ai_normal_path_actions_playtest());
    }

    #[test]
    fn attack_sanity_omits_empty_city_attack_flag_but_default_map_treats_it_as_an_endpoint() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let city = offset_native_by_map(10, 20, 1, 0);
        let beyond = offset_native_by_map(10, 20, 2, 0);
        game.create_city(1, city.0, city.1);
        let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
        let index = Simulation::idx(beyond.0, beyond.1);
        assert_eq!(
            game.leader_paths(actor, None).costs[index],
            3 * MOVE_FRAGMENTS
        );
        assert_eq!(
            game.leader_paths_mode(actor, None, true).costs[index],
            2 * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn sanity_pf_assumes_unknown_source_scope_is_native_even_for_zero_attack_carrier() {
        let mut game = world();
        game.tile_mut(10, 20).terrain = Terrain::Ocean;
        let bridge = offset_native_by_map(10, 20, 1, 0);
        let target = offset_native_by_map(10, 20, 2, 0);
        game.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        game.tile_mut(bridge.0, bridge.1).known_by[HUMAN] = false;
        let id = game.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
        game.spawn_unit(BARBARIANS, UnitKind::Caravel, target.0, target.1);
        let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
        let index = Simulation::idx(target.0, target.1);
        assert_eq!(game.leader_paths(actor, None).costs[index], i32::MAX);
        assert_eq!(
            game.leader_paths_mode(actor, None, true).costs[index],
            3 * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn explorer_diplomatic_endpoint_costs_one_move_instead_of_ignored_terrain_cost() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        let target = offset_native_by_map(10, 20, 1, 0);
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!(
            game.leader_paths(actor, None).costs[Simulation::idx(target.0, target.1)],
            MOVE_FRAGMENTS
        );
        assert_eq!(game.movement_cost_for(actor, target.0, target.1), Some(2));
    }

    #[test]
    fn desktop_leader_fixture_exercises_refuge_and_escape() {
        assert!(world().ai_barbarian_leader_playtest());
    }

    #[test]
    fn leader_sentries_in_a_survivable_stack_but_idles_without_any_threat() {
        let mut game = world();
        let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        game.run_barbarian_leader(leader);
        assert_eq!(game.units[0].state, UnitState::Idle);
        assert_eq!((game.units[0].x, game.units[0].y), (10, 20));
        game.spawn_unit(BARBARIANS, UnitKind::Settlers, 10, 20);
        game.run_barbarian_leader(leader);
        assert_eq!(game.units[0].state, UnitState::Sentry);
        assert_eq!(game.units[0].moves_left, 2 * MOVE_FRAGMENTS);
    }

    #[test]
    fn leader_seeks_the_nearest_reachable_friendly_nonleader_without_a_strength_requirement() {
        let mut game = world();
        let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let close = offset_native_by_map(10, 20, 2, 0);
        let far = offset_native_by_map(10, 20, 0, 4);
        game.spawn_unit(BARBARIANS, UnitKind::Warrior, far.0, far.1);
        game.spawn_unit(BARBARIANS, UnitKind::Worker, close.0, close.1);
        game.run_barbarian_leader(leader);
        let unit = game.units.iter().find(|unit| unit.id == leader).unwrap();
        assert_eq!((unit.x, unit.y), close);
    }

    #[test]
    fn leader_keeps_reverse_aircraft_threat_but_does_not_invent_unreachable_escape_costs() {
        let mut game = world();
        let enemy = game.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
        let point = offset_native_by_map(10, 20, 1, 0);
        let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, point.0, point.1);
        let actor = game.units.iter().find(|unit| unit.id == enemy).unwrap();
        let index = Simulation::idx(point.0, point.1);
        assert!(game.leader_paths(actor, Some((index, BARBARIANS))).costs[index] < i32::MAX);
        game.run_barbarian_leader(leader);
        let unit = game.units.iter().find(|unit| unit.id == leader).unwrap();
        // No cities, bases, or carriers exist: native PF_FUEL cannot leave
        // the fighter's start, although reverse PF_NORMAL selected it.
        assert_eq!((unit.x, unit.y), point);
        assert_eq!(unit.state, UnitState::Idle);
        assert_eq!(unit.moves_left, 2 * MOVE_FRAGMENTS);
    }

    #[test]
    fn leader_flees_increasing_enemy_path_cost_and_does_not_randomly_wander() {
        let mut game = world();
        let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let enemy = offset_native_by_map(10, 20, -1, 0);
        game.spawn_unit(HUMAN, UnitKind::Diplomat, enemy.0, enemy.1);
        game.run_barbarian_leader(leader);
        let unit = game.units.iter().find(|unit| unit.id == leader).unwrap();
        assert!(game.map_distance(unit.x, unit.y, enemy.0, enemy.1) > 1);
        assert_eq!(unit.moves_left, 0);
    }

    #[test]
    fn leader_generated_zoc_can_limit_finite_enemy_cost_escape_before_moves_run_out() {
        let mut game = world();
        let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let enemy = offset_native_by_map(10, 20, -1, 0);
        game.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
        game.run_barbarian_leader(leader);
        let actor = game.units.iter().find(|unit| unit.id == leader).unwrap();
        assert_eq!(actor.moves_left, MOVE_FRAGMENTS);
        assert_eq!(actor.state, UnitState::Idle);
        assert!(game.map_distance(actor.x, actor.y, enemy.0, enemy.1) > 1);
    }

    #[test]
    fn refuge_sanity_gate_is_optimistic_about_unknown_non_native_terrain() {
        let mut game = world();
        for tile in &mut game.tiles {
            tile.known_by[BARBARIANS] = false;
        }
        game.tile_mut(10, 20).terrain = Terrain::Ocean;
        let id = game.spawn_unit(BARBARIANS, UnitKind::Caravel, 10, 20);
        let boat = game.units.iter().find(|unit| unit.id == id).unwrap();
        let target = offset_native_by_map(10, 20, 2, 0);
        let index = Simulation::idx(target.0, target.1);
        assert_eq!(game.leader_paths(boat, None).costs[index], i32::MAX);
        assert!(game.leader_paths_mode(boat, None, true).costs[index] < i32::MAX);
    }

    #[test]
    fn damaged_enemy_reverse_horizon_uses_its_current_rate_not_full_health_type_speed() {
        let mut game = world();
        let id = game.spawn_unit(HUMAN, UnitKind::Horseman, 10, 20);
        let mut actor = game
            .units
            .iter()
            .find(|unit| unit.id == id)
            .unwrap()
            .clone();
        let target = offset_native_by_map(10, 20, 5, 0);
        let index = Simulation::idx(target.0, target.1);
        assert_eq!(game.unit_move_rate(&actor), 2 * MOVE_FRAGMENTS);
        assert_eq!(
            game.leader_paths(&actor, Some((index, BARBARIANS))).costs[index],
            5 * MOVE_FRAGMENTS
        );
        actor.hp = actor.kind.stats().hp / 2;
        actor.moves_left = 0;
        assert_eq!(game.unit_move_rate(&actor), MOVE_FRAGMENTS);
        assert_eq!(
            game.leader_paths(&actor, Some((index, BARBARIANS))).costs[index],
            i32::MAX
        );
    }

    #[test]
    fn immobile_nuclear_type_does_not_become_a_reverse_threat_via_a_fake_minimum_rate() {
        let mut game = world();
        game.players[HUMAN].known.clear();
        let id = game.spawn_unit(HUMAN, UnitKind::Nuclear, 10, 20);
        let actor = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_eq!(game.unit_move_rate(actor), 0);
        let target = offset_native_by_map(10, 20, 1, 0);
        let index = Simulation::idx(target.0, target.1);
        assert_eq!(
            game.leader_paths(actor, Some((index, BARBARIANS))).costs[index],
            i32::MAX
        );
    }

    #[test]
    fn reverse_query_assumes_full_moves_and_rejects_four_complete_turns() {
        let mut game = world();
        let enemy = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let mut actor = game
            .units
            .iter()
            .find(|unit| unit.id == enemy)
            .unwrap()
            .clone();
        actor.moves_left = 0;
        let near = offset_native_by_map(10, 20, 3, 0);
        let far = offset_native_by_map(10, 20, 4, 0);
        let near_index = Simulation::idx(near.0, near.1);
        let far_index = Simulation::idx(far.0, far.1);
        assert_eq!(
            game.leader_paths(&actor, Some((near_index, BARBARIANS)))
                .costs[near_index],
            3 * MOVE_FRAGMENTS
        );
        assert_eq!(
            game.leader_paths(&actor, Some((far_index, BARBARIANS)))
                .costs[far_index],
            i32::MAX
        );
    }

    #[test]
    fn reverse_query_keeps_the_native_permissive_unarmed_and_nonnative_target_quirks() {
        let mut game = world();
        let enemy = game.spawn_unit(HUMAN, UnitKind::Settlers, 10, 20);
        let actor = game
            .units
            .iter()
            .find(|unit| unit.id == enemy)
            .unwrap()
            .clone();
        let target = offset_native_by_map(10, 20, 1, 0);
        game.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        let target_index = Simulation::idx(target.0, target.1);
        assert_eq!(
            game.leader_paths(&actor, Some((target_index, BARBARIANS)))
                .costs[target_index],
            MOVE_FRAGMENTS
        );
    }

    #[test]
    fn leader_releases_ferry_command_to_mobile_nonleader_cargo() {
        let mut game = world();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
        }
        let boat = game.spawn_unit(BARBARIANS, UnitKind::Caravel, 10, 20);
        let leader = game.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let warrior = game.spawn_unit(BARBARIANS, UnitKind::Warrior, 10, 20);
        assert!(game.board_specific_ship(leader, boat));
        assert!(game.board_specific_ship(warrior, boat));
        game.refresh_ai_ferries();
        assert!(game.assign_ai_ferry(boat, leader));
        game.run_barbarian_leader(leader);
        assert_eq!(game.ferry_boss(boat), Some(warrior));
        assert!(!game.ai_ferries.passengers.contains_key(&leader));
    }
}
