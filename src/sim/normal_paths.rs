//! Default-AI GOTO profiles and native PF_NORMAL iteration. Fuelled actors
//! use PF_FUEL; human
//! click routes deliberately remain a separate client policy.
use super::movement_rules::{MovementRules, c2c3_base_extra_rule_name};
use super::*;
use super::{exploration::ExplorerQueue, pf_actions::PfAction};

const NATIVE: u8 = 1;
const CITY: u8 = 2;
const TRANSPORT: u8 = 4;
const TURN_FACTOR: i32 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GotoTask {
    None,
    Attack,
    Defend,
    Escort,
    // Existing hunters execute their default-PF scan path directly; retain
    // the native Hunter GOTO profile for task-aware callers.
    #[allow(dead_code)]
    Hunter,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct AiGotoResult {
    pub(super) alive: bool,
    pub(super) moved: bool,
    pub(super) at_goal: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Default,
    Attack,
    Overlap,
    FerrySearch,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum TilePolicy {
    Default,
    NoFights,
    NoIntermediateFights,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Behavior {
    Normal,
    Ignore,
    DontLeave,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Uninit,
    Init,
    New,
    Processed,
}
#[derive(Clone)]
struct Node {
    cost: i32,
    extra: u32,
    direction: Option<usize>,
    status: Status,
    behavior: Behavior,
    action: PfAction,
    known: bool,
    scope: u8,
    extra_tile: u16,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            cost: 0,
            extra: 0,
            direction: None,
            status: Status::Uninit,
            behavior: Behavior::Normal,
            action: PfAction::None,
            known: false,
            scope: 0,
            extra_tile: 0,
        }
    }
}

pub(super) struct NormalPaths {
    map_wrap: MapWrap,
    actor: Unit,
    rate: i32,
    start: usize,
    current: usize,
    style: Style,
    policy: TilePolicy,
    class_build_anywhere: bool,
    class_can_occupy_city: bool,
    omniscient: bool,
    risk: bool,
    nodes: Vec<Node>,
    queue: ExplorerQueue,
}

impl NormalPaths {
    pub(super) fn enemy_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Default, TilePolicy::Default, true, false)
    }
    pub(super) fn attack_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Attack, TilePolicy::Default, true, false)
    }
    pub(super) fn overlap_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Overlap, TilePolicy::Default, true, false)
    }
    pub(super) fn position_turn(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        let goal = self.reach(sim, x, y)?;
        Some(self.nodes[goal].cost / self.rate)
    }
    pub(super) fn ferry_search_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(
            sim,
            unit,
            Style::FerrySearch,
            TilePolicy::NoFights,
            true,
            false,
        )
    }
    pub(super) fn positions(&mut self, sim: &Simulation) -> Vec<(i32, i32, i32, u32)> {
        let mut result = vec![(self.actor.x, self.actor.y, 0, 0)];
        while self.iterate(sim) {
            let (x, y) = self.map_wrap.position(self.current);
            let node = &self.nodes[self.current];
            result.push((x, y, node.cost / self.rate, node.extra));
        }
        result
    }
    fn new(
        sim: &Simulation,
        unit: &Unit,
        style: Style,
        policy: TilePolicy,
        omniscient: bool,
        risk: bool,
    ) -> Self {
        let start = sim.map_wrap.index(unit.x, unit.y);
        let class_rules = MovementRules::campaign()
            .unit_class_movement_data_for_kind(unit.kind)
            .expect("selected C2C3 unit-class movement data");
        let mut map = Self {
            map_wrap: sim.map_wrap,
            actor: unit.clone(),
            rate: sim.unit_move_rate(unit),
            start,
            current: start,
            style,
            policy,
            class_build_anywhere: class_rules.build_anywhere,
            class_can_occupy_city: class_rules.can_occupy_city,
            omniscient,
            risk,
            nodes: vec![Node::default(); sim.tiles.len()],
            queue: ExplorerQueue::default(),
        };
        map.initialize(sim, start, 0);
        if let Some(ship) = unit.transport.and_then(|id| sim.unit_by_id(id)) {
            map.nodes[start].scope |= TRANSPORT;
            let at_port = sim
                .city_at(unit.x, unit.y)
                .is_some_and(|city| sim.allied(unit.owner, city.owner))
                || Self::native_base(sim, ship.kind, unit.x, unit.y);
            let can_disembark =
                at_port || sim.ruleset_unit_can_disembark_fail_closed(unit.kind, ship.kind);
            let ship_has_orders = sim.unit_goto_tiles.contains_key(&ship.id);
            if !can_disembark || ship_has_orders {
                map.nodes[start].behavior = Behavior::DontLeave;
            }
        }
        map.nodes[start].cost = map.rate.wrapping_sub(unit.moves_left);
        map.nodes[start].status = Status::Processed;
        map
    }

    fn native_base(sim: &Simulation, kind: UnitKind, x: i32, y: i32) -> bool {
        let Some(base) = sim.tile(x, y).base else {
            return false;
        };
        let rules = MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(kind)
            .expect("selected C2C3 unit class");
        rules
            .extra_is_native_to_class(c2c3_base_extra_rule_name(base), class)
            .expect("selected C2C3 base extra nativity")
    }

    fn scope_native_tile(&self, sim: &Simulation, kind: UnitKind, x: i32, y: i32) -> bool {
        let coast_strict = super::action_rules::freeciv_unit_type_flag(kind, "CoastStrict")
            .expect("selected C2C3 CoastStrict unit flag");
        let observer = (!self.omniscient).then_some(self.actor.owner);
        if coast_strict && !sim.safe_ocean_at(x, y, observer) {
            return false;
        }
        sim.pf_movement_native_tile(kind, x, y)
    }

    fn scope(&self, sim: &Simulation, x: i32, y: i32, previous: u8) -> (u8, bool) {
        let kind = self.actor.kind;
        // PF also supports a city-channel path when citymindist is 1. CivCraft
        // has no equivalent city-channel search yet; prior-city continuation,
        // selected BuildAnywhere, and native-neighbor access remain supported.
        let mut scope = if self.scope_native_tile(sim, kind, x, y) {
            NATIVE
        } else {
            0
        };
        let city = sim.city_at(x, y);
        if city.is_some_and(|city| {
            self.class_can_occupy_city || sim.allied(self.actor.owner, city.owner)
        }) && (previous & CITY != 0
            || self.class_build_anywhere
            || self
                .map_wrap
                .civ3_neighbors(x, y)
                .any(|(nx, ny)| self.scope_native_tile(sim, kind, nx, ny)))
        {
            scope |= CITY;
        }
        let mut can_disembark = false;
        if scope == 0 {
            for carrier in sim.tile_units_ordered(x, y) {
                if !sim.allied(self.actor.owner, carrier.owner)
                    || carrier.kind == kind
                    || !sim.ruleset_transport_pair_allowed_fail_closed(carrier.kind, kind)
                    || sim.transport_would_create_cycle(self.actor.id, carrier.id)
                {
                    continue;
                }
                // PF checks type compatibility, not free slots. The shared
                // rule adapter covers direct reciprocal cargo types and the
                // live parent chain; Freeciv's deeper hypothetical cargo-type
                // recursion is not representable with CivCraft's current unit
                // nesting model. A stored GOTO target is the available proxy
                // for Freeciv's persistent transport orders.
                let at_base = city.is_some_and(|city| sim.allied(self.actor.owner, city.owner))
                    || Self::native_base(sim, carrier.kind, x, y);
                if at_base {
                    scope |= TRANSPORT;
                    can_disembark = !sim.unit_goto_tiles.contains_key(&carrier.id);
                    break;
                }
                if !sim.ruleset_unit_can_embark_fail_closed(kind, carrier.kind) {
                    continue;
                }
                scope |= TRANSPORT;
                if sim.ruleset_unit_can_disembark_fail_closed(kind, carrier.kind) {
                    can_disembark = !sim.unit_goto_tiles.contains_key(&carrier.id);
                    break;
                }
            }
        }
        (scope, can_disembark)
    }

    fn foreign_at(&self, sim: &Simulation, x: i32, y: i32) -> bool {
        sim.units
            .iter()
            .any(|unit| (unit.x, unit.y) == (x, y) && !sim.allied(self.actor.owner, unit.owner))
            || sim
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && !sim.allied(self.actor.owner, city.owner))
    }

    fn initialize(&mut self, sim: &Simulation, index: usize, previous_scope: u8) -> bool {
        if self.nodes[index].status != Status::Uninit {
            return self.nodes[index].behavior != Behavior::Ignore;
        }
        let (x, y) = self.map_wrap.position(index);
        self.nodes[index].status = Status::Init;
        let foreign = self.foreign_at(sim, x, y);
        self.nodes[index].behavior = match self.policy {
            TilePolicy::NoFights if foreign => Behavior::Ignore,
            TilePolicy::NoIntermediateFights if foreign => Behavior::DontLeave,
            _ => Behavior::Normal,
        };
        if self.nodes[index].behavior == Behavior::Ignore && index != self.start {
            return false;
        }
        let known = self.omniscient || sim.tile(x, y).known_by[self.actor.owner];
        self.nodes[index].known = known;
        if known {
            if !self.actor.kind.is_civilian()
                && sim
                    .tile(x, y)
                    .owner
                    .is_some_and(|owner| sim.pact(self.actor.owner, owner) == Pact::Peace)
            {
                if index != self.start {
                    self.nodes[index].behavior = Behavior::Ignore;
                    return false;
                }
                if self.nodes[index].behavior == Behavior::Normal {
                    self.nodes[index].behavior = Behavior::Ignore;
                }
            }
            let action = if self.style == Style::Overlap {
                PfAction::None
            } else {
                sim.pf_default_action(
                    self.actor.kind,
                    self.actor.owner,
                    (x, y),
                    true,
                    self.omniscient || sim.pf_main_sight(self.actor.owner, x, y),
                    self.style == Style::Default,
                )
            };
            if action == PfAction::Impossible {
                if index != self.start {
                    self.nodes[index].behavior = Behavior::Ignore;
                    return false;
                }
                if self.nodes[index].behavior == Behavior::Normal {
                    self.nodes[index].behavior = Behavior::Ignore;
                }
            } else {
                self.nodes[index].action = action;
                if action != PfAction::None {
                    self.nodes[index].behavior = Behavior::DontLeave;
                }
            }
            let (scope, can_disembark) = self.scope(sim, x, y, previous_scope);
            self.nodes[index].scope = scope;
            if scope == 0
                && self.nodes[index].action == PfAction::None
                && !matches!(self.style, Style::Overlap | Style::FerrySearch)
            {
                if index != self.start {
                    self.nodes[index].behavior = Behavior::Ignore;
                    return false;
                }
                if self.nodes[index].behavior == Behavior::Normal {
                    self.nodes[index].behavior = Behavior::Ignore;
                }
            } else if scope == TRANSPORT
                && !can_disembark
                && (index != self.start || self.actor.transport.is_none())
            {
                self.nodes[index].behavior = Behavior::DontLeave;
            }
        } else {
            self.nodes[index].scope = NATIVE;
        }
        if self.style == Style::FerrySearch && sim.tile(x, y).terrain.is_water() {
            let fragments = MovementRules::campaign()
                .move_fragments()
                .expect("selected C2C3 movement fragments");
            self.nodes[index].extra_tile = (fragments * TURN_FACTOR / 12) as u16;
        } else if self.risk {
            self.nodes[index].extra_tile = sim.explorer_stack_risk(&self.actor, x, y) as u16;
        }
        true
    }

    fn total(&self, cost: i32, extra: u32) -> i64 {
        i64::from(
            cost.wrapping_mul(TURN_FACTOR)
                .wrapping_add(extra.wrapping_mul(self.rate as u32) as i32),
        )
    }
    fn moves_left(&self, cost: i32) -> i32 {
        if cost <= 0 {
            self.rate.wrapping_sub(cost)
        } else if self.rate <= 0 {
            0
        } else {
            self.rate - cost % self.rate
        }
    }

    fn iterate(&mut self, sim: &Simulation) -> bool {
        let current = self.current;
        let node = self.nodes[current].clone();
        if node.behavior != Behavior::DontLeave
            && (node.scope != 0 || self.style == Style::FerrySearch)
            && (self.rate > 0 || node.cost < 0)
        {
            let (x, y) = self.map_wrap.position(current);
            let mut actor = self.actor.clone();
            (actor.x, actor.y) = (x, y);
            for (direction, (dx, dy)) in CIV3_DIRS.into_iter().enumerate() {
                let Some((nx, ny)) = self.map_wrap.civ3_offset(x, y, dx, dy) else {
                    continue;
                };
                let next = self.map_wrap.index(nx, ny);
                if self.nodes[next].status == Status::Processed {
                    continue;
                }
                if !self.initialize(sim, next, node.scope) {
                    continue;
                }
                let dest = self.nodes[next].clone();
                // PF_NORMAL and movement execution share the same C2C3 ZOC
                // test, including class/type flags, city/terrain exceptions,
                // and the direct-combat destination exception.
                if dest.action != PfAction::Attack
                    && sim.zone_blocks_route(&actor, (x, y), (nx, ny), false)
                {
                    continue;
                }
                let cost = if self.style == Style::FerrySearch {
                    if dest.scope & (NATIVE | CITY) == 0 {
                        0
                    } else if node.scope & (NATIVE | CITY) == 0 {
                        continue;
                    } else {
                        sim.raw_move_cost_for(&actor, nx, ny)
                    }
                } else if dest.action != PfAction::None {
                    if dest.action == PfAction::Attack {
                        if node.scope & NATIVE == 0
                            && !sim.pf_action_step_possible(&actor, dest.action)
                        {
                            continue;
                        }
                    } else if node.scope & (NATIVE | CITY | TRANSPORT) == 0 {
                        continue;
                    }
                    Simulation::pf_action_move_cost(actor.kind, dest.action, self.rate)
                } else if !dest.known {
                    MovementRules::campaign()
                        .unknown_tile_move_cost(actor.kind)
                        .expect("selected C2C3 unknown-tile movement cost")
                } else if dest.scope != 0 {
                    sim.raw_move_cost_for(&actor, nx, ny)
                } else if self.style == Style::Overlap && dest.scope & NATIVE == 0 {
                    self.rate
                } else {
                    continue;
                };
                let cost = node.cost.wrapping_add(cost.min(self.moves_left(node.cost)));
                let extra = node.extra.wrapping_add(u32::from(dest.extra_tile));
                if dest.status == Status::Init
                    || self.total(cost, extra) < self.total(dest.cost, dest.extra)
                {
                    self.nodes[next].cost = cost;
                    self.nodes[next].extra = extra;
                    self.nodes[next].direction = Some(direction);
                    if dest.status == Status::Init {
                        self.nodes[next].status = Status::New;
                        self.queue.insert(self.total(cost, extra), next);
                    } else {
                        self.queue.put(self.total(cost, extra), next);
                    }
                }
            }
        }
        let Some((_, next)) = self.queue.pop() else {
            return false;
        };
        self.current = next;
        self.nodes[next].status = Status::Processed;
        true
    }

    fn reach(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<usize> {
        let goal = self.map_wrap.index(x, y);
        if goal == self.start {
            return Some(goal);
        }
        if !self.initialize(sim, goal, 0) {
            return None;
        }
        while self.nodes[goal].status != Status::Processed {
            if !self.iterate(sim) {
                return None;
            }
        }
        Some(goal)
    }
    pub(super) fn movement_cost(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        let goal = self.reach(sim, x, y)?;
        Some(
            self.nodes[goal]
                .cost
                .wrapping_sub(self.rate)
                .wrapping_add(self.actor.moves_left),
        )
    }
    pub(super) fn reachable_within(&mut self, sim: &Simulation, limit: i32) -> Vec<(i32, i32)> {
        let mut positions = Vec::new();
        while self.iterate(sim) {
            let cost = self.nodes[self.current]
                .cost
                .wrapping_sub(self.rate)
                .wrapping_add(self.actor.moves_left);
            if cost > limit {
                break;
            }
            positions.push(self.map_wrap.position(self.current));
        }
        positions
    }
    pub(super) fn path(
        &mut self,
        sim: &Simulation,
        x: i32,
        y: i32,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        let mut cursor = self.reach(sim, x, y)?;
        let cost = self.nodes[cursor]
            .cost
            .wrapping_sub(self.rate)
            .wrapping_add(self.actor.moves_left);
        let mut path = Vec::new();
        while cursor != self.start {
            let direction = self.nodes[cursor].direction?;
            let (dx, dy) = CIV3_DIRS[direction];
            path.push((dx, dy));
            let (x, y) = self.map_wrap.position(cursor);
            let (x, y) = self.map_wrap.civ3_offset(x, y, -dx, -dy)?;
            cursor = self.map_wrap.index(x, y);
        }
        path.reverse();
        Some((path, cost))
    }
}

impl Simulation {
    fn source_goto_profile(&self, unit: &Unit, task: GotoTask) -> (Style, TilePolicy, bool, bool) {
        let ferry = Self::is_ai_ferry(unit.kind) && task != GotoTask::Hunter;
        let style = if ferry {
            Style::Overlap
        } else if unit.kind.attack_strength() > 0
            && matches!(
                task,
                GotoTask::Attack | GotoTask::Defend | GotoTask::Escort | GotoTask::Hunter
            )
        {
            Style::Attack
        } else {
            Style::Default
        };
        let policy = if unit.kind.stats().can_work || unit.kind.stats().can_found || ferry {
            TilePolicy::NoFights
        } else if unit.kind == UnitKind::Helicopter {
            TilePolicy::Default
        } else {
            match task {
                GotoTask::Attack | GotoTask::Defend | GotoTask::Escort | GotoTask::Hunter => {
                    TilePolicy::NoIntermediateFights
                }
                GotoTask::None => TilePolicy::Default,
            }
        };
        // long_path compares the unit tile to itself in the reference.
        // It is always false; do not relax ZOC for far-away destinations.
        let risk = unit.owner != BARBARIANS && !unit.kind.is_aircraft();
        (style, policy, risk, ferry)
    }

    pub(super) fn ai_goto_is_sane(&self, unit: &Unit, goal: (i32, i32)) -> bool {
        if let Some(mut map) = super::fuel_paths::FuelPaths::attack_sanity(self, unit) {
            return map.movement_cost(self, goal.0, goal.1).is_some();
        }
        let mut sanity =
            NormalPaths::new(self, unit, Style::Attack, TilePolicy::Default, false, false);
        sanity.movement_cost(self, goal.0, goal.1).is_some()
    }

    pub(super) fn ai_goto_path(
        &self,
        unit: &Unit,
        goal: (i32, i32),
        task: GotoTask,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        // dai_unit_goto_constrained first performs the known-map attack
        // parameter sanity check, even though the real AI path is omniscient.
        if !self.ai_goto_is_sane(unit, goal) {
            return None;
        }
        if let Some(mut map) = super::fuel_paths::FuelPaths::new(self, unit, true) {
            let cost = map.movement_cost(self, goal.0, goal.1)?;
            return Some((map.path(self, goal.0, goal.1)?, cost));
        }
        let (style, policy, risk, _) = self.source_goto_profile(unit, task);
        NormalPaths::new(self, unit, style, policy, true, risk).path(self, goal.0, goal.1)
    }

    pub(super) fn ai_goto_unit(
        &mut self,
        id: u32,
        goal: (i32, i32),
        task: GotoTask,
    ) -> AiGotoResult {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return AiGotoResult {
                alive: false,
                moved: false,
                at_goal: false,
            };
        };
        let ferry = self.source_goto_profile(&unit, task).3;
        if ferry {
            self.unit_goto_tiles.insert(id, goal);
        }
        if (unit.x, unit.y) != goal && !self.ai_goto_is_sane(&unit, goal) {
            if let Some(actor) = self.units.iter_mut().find(|actor| actor.id == id) {
                actor.state = UnitState::Idle;
            }
            return AiGotoResult {
                alive: true,
                moved: false,
                at_goal: false,
            };
        }
        let mut progressed = false;
        if (unit.x, unit.y) != goal
            && unit.moves_left > 0
            && let Some((path, _)) = self.ai_goto_path(&unit, goal, task)
        {
            progressed = self.follow_source_ai_path(id, path).moved;
        }
        let live = self.unit_by_id(id);
        AiGotoResult {
            alive: live.is_some(),
            moved: progressed,
            at_goal: live.is_some_and(|actor| (actor.x, actor.y) == goal),
        }
    }

    pub(super) fn follow_source_ai_path(&mut self, id: u32, path: Vec<(i32, i32)>) -> AiGotoResult {
        let Some(initial) = self.unit_by_id(id).cloned() else {
            return AiGotoResult {
                alive: false,
                moved: false,
                at_goal: false,
            };
        };
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
            unit.state = UnitState::Idle;
        }
        let mut destination = (initial.x, initial.y);
        for &(dx, dy) in &path {
            if (dx, dy) != (0, 0) {
                destination = self
                    .map_wrap
                    .civ3_offset(destination.0, destination.1, dx, dy)
                    .expect("planned step is real");
            }
        }
        let old_goal = self.unit_goto_tiles.insert(id, destination);
        let mut progressed = false;
        let length = path.len();
        for (position, (dx, dy)) in path.into_iter().enumerate() {
            // Duplicate positions in PF_FUEL paths mean wait for refueling;
            // they do not consume movement in this phase.
            if (dx, dy) == (0, 0) {
                break;
            }
            let Some(actor) = self.unit_by_id(id).cloned() else {
                break;
            };
            let Some((x, y)) = self.map_wrap.civ3_offset(actor.x, actor.y, dx, dy) else {
                continue;
            };
            let enemy =
                self.units.iter().any(|other| {
                    (other.x, other.y) == (x, y) && self.at_war(actor.owner, other.owner)
                }) || self
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == (x, y) && self.at_war(actor.owner, city.owner));
            if actor.moves_left <= 0
                || (enemy && (position + 1 != length || !self.players[actor.owner].ai))
            {
                break;
            }
            if actor.owner == BARBARIANS && self.tile(x, y).hut {
                break;
            }
            let cost = self.raw_move_cost_for(&actor, x, y);
            if !enemy
                && actor.moves_left <= cost
                && self.unit_move_rate(&actor) > cost
                && self.explorer_danger_at(&actor, x, y)
                && !self.explorer_danger_at(&actor, actor.x, actor.y)
            {
                break;
            }
            let result = CIV3_DIRS
                .iter()
                .position(|&step| step == (dx, dy))
                .map_or(MoveResult::Blocked, |direction| {
                    self.try_move_unit_civ3(id, direction)
                });
            if result == MoveResult::Blocked {
                break;
            }
            progressed = true;
        }
        if let Some(actor) = self.units.iter_mut().find(|unit| unit.id == id) {
            actor.state = initial.state;
            if let Some(goal) = old_goal {
                self.unit_goto_tiles.insert(id, goal);
            } else {
                self.unit_goto_tiles.remove(&id);
            }
        }
        let actor = self.unit_by_id(id);
        AiGotoResult {
            alive: actor.is_some(),
            moved: progressed,
            at_goal: actor.is_some_and(|actor| (actor.x, actor.y) == destination),
        }
    }
}

#[cfg(test)]
#[path = "normal_paths/tests.rs"]
mod tests;

#[cfg(test)]
mod selected_rules_tests {
    use super::*;

    fn world() -> Simulation {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.known_by.fill(true);
            tile.owner = None;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
        }
        sim
    }

    #[test]
    fn unknown_path_step_uses_selected_c2c3_unit_cost() {
        let mut sim = world();
        let id = sim.spawn_unit(HUMAN, UnitKind::Explorer, 10, 20);
        let actor = sim.unit_by_id(id).unwrap().clone();
        let target = sim.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        sim.tile_mut(target.0, target.1).known_by[HUMAN] = false;

        let expected = MovementRules::campaign()
            .unknown_tile_move_cost(actor.kind)
            .unwrap();
        let mut map = NormalPaths::new(
            &sim,
            &actor,
            Style::Default,
            TilePolicy::Default,
            false,
            false,
        );
        assert_eq!(map.movement_cost(&sim, target.0, target.1), Some(expected));
    }

    #[test]
    fn city_scope_uses_selected_build_anywhere_class_flag() {
        let mut sim = world();
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Sea;
        }
        let id = sim.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let actor = sim.unit_by_id(id).unwrap().clone();
        let target = sim.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        sim.create_city(HUMAN, target.0, target.1);
        sim.tile_mut(target.0, target.1).road = false;

        let class = MovementRules::campaign()
            .unit_class_movement_data_for_kind(actor.kind)
            .unwrap();
        assert!(class.build_anywhere);
        assert!(class.can_occupy_city);

        let map = NormalPaths::new(
            &sim,
            &actor,
            Style::Default,
            TilePolicy::Default,
            true,
            false,
        );
        assert_eq!(map.scope(&sim, target.0, target.1, 0).0, CITY);
    }

    #[test]
    fn city_scope_uses_selected_can_occupy_city_class_flag() {
        let mut sim = world();
        let id = sim.spawn_unit(HUMAN, UnitKind::Helicopter, 10, 20);
        let actor = sim.unit_by_id(id).unwrap().clone();
        let target = sim.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        sim.create_city(1, target.0, target.1);

        let class = MovementRules::campaign()
            .unit_class_movement_data_for_kind(actor.kind)
            .unwrap();
        assert!(!class.build_anywhere);
        assert!(class.can_occupy_city);

        let map = NormalPaths::new(
            &sim,
            &actor,
            Style::Default,
            TilePolicy::Default,
            true,
            false,
        );
        assert_ne!(map.scope(&sim, target.0, target.1, CITY).0 & CITY, 0);
    }

    #[test]
    fn safe_ocean_uses_c2c3_flags_and_six_way_x_wrap_neighbors() {
        let mut sim = world();
        let rules = MovementRules::campaign();
        let unsafe_terrain = [Terrain::Sea, Terrain::Ocean, Terrain::Lake]
            .into_iter()
            .find(|terrain| {
                rules
                    .terrain_has_flag(
                        *terrain,
                        super::super::movement_rules::TerrainFlag::UnsafeCoast,
                    )
                    .unwrap()
            })
            .expect("C2C3 has an unsafe-coast terrain");
        for tile in &mut sim.tiles {
            tile.terrain = unsafe_terrain;
        }
        let id = sim.spawn_unit(HUMAN, UnitKind::Galley, 0, 20);
        let actor = sim.unit_by_id(id).unwrap().clone();
        assert!(!sim.safe_ocean_at(actor.x, actor.y, None));

        let diagonal_only = sim.map_wrap.civ3_neighbor(actor.x, actor.y, 2).unwrap();
        sim.tile_mut(diagonal_only.0, diagonal_only.1).terrain = Terrain::Grassland;
        assert!(
            !sim.safe_ocean_at(actor.x, actor.y, None),
            "C2C3 ISO|HEX coast adjacency excludes CivCraft's two diagonal steps"
        );

        let cardinal = sim.map_wrap.civ3_neighbor(actor.x, actor.y, 1).unwrap();
        sim.tile_mut(cardinal.0, cardinal.1).terrain = Terrain::Grassland;
        assert!(sim.safe_ocean_at(actor.x, actor.y, None));

        sim.tile_mut(cardinal.0, cardinal.1).terrain = unsafe_terrain;
        let seam_cardinal = sim.map_wrap.civ3_neighbor(actor.x, actor.y, 7).unwrap();
        assert_eq!(seam_cardinal.0, sim.map_wrap.width() - 1);
        sim.tile_mut(seam_cardinal.0, seam_cardinal.1).terrain = Terrain::Grassland;
        assert!(sim.safe_ocean_at(actor.x, actor.y, None));
    }
}
