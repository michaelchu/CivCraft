//! Default-AI GOTO profiles and native PF_NORMAL iteration. Fuelled actors
//! use PF_FUEL; human
//! click routes deliberately remain a separate client policy.
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
    Trade,
    Wonder,
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum Zoc {
    Mine,
    Allied,
    No,
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
    zoc: Zoc,
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
            zoc: Zoc::Mine,
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
        let mut map = Self {
            map_wrap: sim.map_wrap,
            actor: unit.clone(),
            rate: sim.unit_move_rate(unit),
            start,
            current: start,
            style,
            policy,
            omniscient,
            risk,
            nodes: vec![Node::default(); sim.tiles.len()],
            queue: ExplorerQueue::default(),
        };
        map.initialize(sim, start, 0);
        if let Some(ship) = unit.transport.and_then(|id| sim.unit_by_id(id)) {
            map.nodes[start].scope |= TRANSPORT;
            if !Self::free_transport(unit.kind, ship.kind)
                && !sim
                    .cities
                    .iter()
                    .any(|city| (city.x, city.y) == (unit.x, unit.y))
                && !Self::native_base(sim, ship.kind, unit.x, unit.y)
            {
                map.nodes[start].behavior = Behavior::DontLeave;
            }
        }
        map.nodes[start].cost = map.rate.wrapping_sub(unit.moves_left);
        map.nodes[start].status = Status::Processed;
        map
    }

    fn free_transport(cargo: UnitKind, ship: UnitKind) -> bool {
        ship != UnitKind::Helicopter
            || matches!(
                cargo,
                UnitKind::Partisan | UnitKind::Marines | UnitKind::Paratroopers
            )
    }
    fn native_base(sim: &Simulation, kind: UnitKind, x: i32, y: i32) -> bool {
        match sim.tile(x, y).base {
            Some(Base::Fort | Base::Fortress) => !kind.is_aircraft() && !kind.is_naval(),
            Some(Base::Airstrip | Base::Airbase) => kind.is_aircraft(),
            None => false,
        }
    }

    fn scope(&self, sim: &Simulation, x: i32, y: i32, previous: u8) -> (u8, bool) {
        let kind = self.actor.kind;
        let mut scope = if sim.pf_native_tile(kind, x, y) {
            NATIVE
        } else {
            0
        };
        let city = sim.city_at(x, y);
        let build_anywhere = !kind.is_naval() && !kind.is_aircraft();
        if city
            .is_some_and(|city| kind.can_occupy_city() || sim.allied(self.actor.owner, city.owner))
            && (previous & CITY != 0
                || build_anywhere
                || self
                    .map_wrap
                    .neighbors(x, y)
                    .any(|(nx, ny)| sim.pf_native_tile(kind, nx, ny)))
        {
            scope |= CITY;
        }
        let mut can_disembark = false;
        if scope == 0 {
            for carrier in sim.tile_units_ordered(x, y) {
                if !sim.allied(self.actor.owner, carrier.owner)
                    || carrier.kind == kind
                    || !carrier.kind.can_carry(kind)
                    || kind.can_carry(carrier.kind)
                {
                    continue;
                }
                // PF asks whether the TYPE fits, not whether current cargo
                // slots are free. Execution performs the capacity check.
                let at_base = city.is_some_and(|city| sim.allied(self.actor.owner, city.owner))
                    || Self::native_base(sim, carrier.kind, x, y);
                if at_base || Self::free_transport(kind, carrier.kind) {
                    scope |= TRANSPORT;
                    can_disembark = true; // No persistent manual orders in supported runtime.
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
            if !self.actor.kind.ignores_zoc()
                && sim.cities.iter().all(|city| (city.x, city.y) != (x, y))
                && !sim.tile(x, y).terrain.is_water()
                && sim.under_foreign_control(self.actor.owner, x, y, false)
            {
                self.nodes[index].zoc = if sim.units.iter().any(|unit| (unit.x, unit.y) == (x, y)) {
                    Zoc::Allied
                } else {
                    Zoc::No
                };
            }
        } else {
            self.nodes[index].scope = NATIVE;
        }
        if self.style == Style::FerrySearch && sim.tile(x, y).terrain.is_water() {
            self.nodes[index].extra_tile = (MOVE_FRAGMENTS * TURN_FACTOR / 12) as u16;
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
            for (direction, (dx, dy)) in ISOHEX_DIRS.into_iter().enumerate() {
                let Some((nx, ny)) = self.map_wrap.offset(x, y, dx, dy) else {
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
                if node.zoc != Zoc::Mine && dest.zoc == Zoc::No {
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
                    if actor.kind.is_aircraft() {
                        MOVE_FRAGMENTS
                    } else if actor.kind.is_naval() {
                        2 * MOVE_FRAGMENTS
                    } else if actor.kind.stats().ignores_terrain {
                        4
                    } else {
                        6 * MOVE_FRAGMENTS
                    }
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
            let (dx, dy) = ISOHEX_DIRS[direction];
            path.push((dx, dy));
            let (x, y) = self.map_wrap.position(cursor);
            let (x, y) = self.map_wrap.offset(x, y, -dx, -dy)?;
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
        } else if unit.kind.stats().attack > 0
            && matches!(
                task,
                GotoTask::Attack | GotoTask::Defend | GotoTask::Escort | GotoTask::Hunter
            )
        {
            Style::Attack
        } else {
            Style::Default
        };
        let policy = if unit.kind.stats().can_work
            || unit.kind.stats().can_found
            || unit.kind == UnitKind::BarbarianLeader
            || ferry
        {
            TilePolicy::NoFights
        } else if unit.kind == UnitKind::Helicopter {
            TilePolicy::Default
        } else {
            match task {
                GotoTask::Attack
                | GotoTask::Defend
                | GotoTask::Escort
                | GotoTask::Hunter
                | GotoTask::Trade
                | GotoTask::Wonder => TilePolicy::NoIntermediateFights,
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
            destination = self
                .map_wrap
                .offset(destination.0, destination.1, dx, dy)
                .expect("planned step is real");
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
            let Some((x, y)) = self.map_wrap.offset(actor.x, actor.y, dx, dy) else {
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
            let result = self.try_move_unit(id, dx, dy);
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
