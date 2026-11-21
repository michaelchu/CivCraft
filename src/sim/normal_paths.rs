//! Default-AI GOTO profiles and native PF_NORMAL iteration. Fuelled actors
//! use PF_FUEL; human
//! click routes deliberately remain a separate client policy.
use super::movement::PathZone;
use super::movement_rules::{MovementRules, campaign_move_fragments};
use super::pf_actions::PfAction;
use super::*;

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

/// Indexed heap for normal path maps. Freeciv's map_index_pq replaces a
/// queued tile directly; ExplorerQueue::put otherwise scans the heap to find
/// that tile, making each decrease-key linear in the frontier size.
#[derive(Clone, Default)]
struct NormalPathQueue {
    heap: Vec<(i64, usize)>,
    positions: Vec<usize>,
}

impl NormalPathQueue {
    fn new(tile_count: usize) -> Self {
        Self {
            heap: vec![(0, 0)],
            positions: vec![0; tile_count],
        }
    }

    fn insert(&mut self, cost: i64, tile: usize) {
        debug_assert_eq!(self.positions[tile], 0);
        self.put(cost, tile);
    }

    fn put(&mut self, cost: i64, tile: usize) {
        let mut index = self.positions[tile];
        if index == 0 {
            self.heap.push((i64::MAX, tile));
            index = self.heap.len() - 1;
        } else if cost >= self.heap[index].0 {
            return;
        }
        while index > 1 && self.heap[index / 2].0 > cost {
            let parent = index / 2;
            self.heap[index] = self.heap[parent];
            self.positions[self.heap[index].1] = index;
            index = parent;
        }
        self.heap[index] = (cost, tile);
        self.positions[tile] = index;
    }

    fn pop(&mut self) -> Option<(i64, usize)> {
        if self.heap.len() <= 1 {
            return None;
        }
        let first = self.heap[1];
        let last = self.heap.pop().expect("nonempty path queue");
        self.positions[first.1] = 0;
        if self.heap.len() > 1 {
            let mut index = 1;
            while index * 2 < self.heap.len() {
                let mut child = index * 2;
                if child + 1 < self.heap.len() && self.heap[child + 1].0 < self.heap[child].0 {
                    child += 1;
                }
                if self.heap[child].0 >= last.0 {
                    break;
                }
                self.heap[index] = self.heap[child];
                self.positions[self.heap[index].1] = index;
                index = child;
            }
            self.heap[index] = last;
            self.positions[last.1] = index;
        }
        Some(first)
    }

    fn frontier_len(&self) -> usize {
        self.heap.len().saturating_sub(1)
    }
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
#[derive(Clone, Copy)]
struct Node {
    cost: i32,
    extra: u32,
    // A predecessor is one of the eight CIV3_DIRS entries.
    direction: Option<u8>,
    status: Status,
    behavior: Behavior,
    action: PfAction,
    known: bool,
    scope: u8,
    extra_tile: u16,
    zone: Option<PathZone>,
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
            zone: None,
        }
    }
}

#[derive(Clone)]
pub(in crate::sim) struct NormalPaths {
    map_wrap: MapWrap,
    actor: Unit,
    rate: i32,
    start: usize,
    current: usize,
    style: Style,
    policy: TilePolicy,
    movement_class: super::unit_catalog::UnitClass,
    ignores_terrain: bool,
    ignores_zoc: bool,
    coast_strict: bool,
    class_build_anywhere: bool,
    class_can_occupy_city: bool,
    omniscient: bool,
    main_sight: Option<std::sync::Arc<[bool]>>,
    allied_mask: u64,
    war_mask: u64,
    peace_mask: u64,
    risk: bool,
    nodes: Vec<Node>,
    queue: NormalPathQueue,
    yielded_start: bool,
    heuristic_goal: Option<usize>,
    /// Order the frontier by distance to `heuristic_goal` alone. Only valid
    /// when the caller needs reachability, not a cost or a path.
    reachability_only: bool,
    expanded: u64,
}

impl NormalPaths {
    pub(super) fn enemy_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Default, TilePolicy::Default, true, false)
    }
    pub(super) fn attack_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Attack, TilePolicy::Default, true, false)
    }
    pub(super) fn goto_sanity_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Attack, TilePolicy::Default, false, false)
    }
    pub(super) fn goto_map(sim: &Simulation, unit: &Unit, task: GotoTask) -> Self {
        let (style, policy, risk, _) = sim.source_goto_profile(unit, task);
        Self::new(sim, unit, style, policy, true, risk)
    }
    pub(super) fn overlap_map(sim: &Simulation, unit: &Unit) -> Self {
        Self::new(sim, unit, Style::Overlap, TilePolicy::Default, true, false)
    }
    pub(super) fn position_turn(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        let goal = self.reach(sim, x, y)?;
        Some(self.nodes[goal].cost / self.rate)
    }

    pub(super) fn position_turn_within(
        &mut self,
        sim: &Simulation,
        x: i32,
        y: i32,
        max_turn: i32,
    ) -> Option<i32> {
        let max_raw_cost = max_turn
            .saturating_add(1)
            .saturating_mul(self.rate)
            .saturating_sub(1);
        let goal = self.reach_within_raw_cost(sim, x, y, max_raw_cost)?;
        Some(self.nodes[goal].cost / self.rate)
    }

    /// Point-to-point variant for production candidate routing. On the
    /// selected C2C3 map every represented normal-movement edge costs at
    /// least one move fragment, so Civ III's exact neighbor-graph distance is
    /// an admissible and consistent lower bound.
    pub(super) fn position_turn_astar(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        let goal = self.map_wrap.index(x, y);
        self.heuristic_goal = Some(goal);
        let reached = self.reach(sim, x, y);
        self.heuristic_goal = None;
        sim.trace
            .count_operation("normal_paths.astar_query_expansions", self.expanded);
        let goal = reached?;
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
    fn new(
        sim: &Simulation,
        unit: &Unit,
        style: Style,
        policy: TilePolicy,
        omniscient: bool,
        risk: bool,
    ) -> Self {
        let start = sim.map_wrap.index(unit.x, unit.y);
        let &(movement_class, ref class_rules) =
            MovementRules::campaign_unit_movement_data(unit.kind);
        let ignores_terrain = super::action_rules::unit_type_flag(unit.kind, "IgTer")
            .expect("campaign C2C3 unit-type IgTer binding");
        let coast_strict = super::action_rules::unit_type_flag(unit.kind, "CoastStrict")
            .expect("campaign C2C3 CoastStrict unit flag");
        let (allied_mask, war_mask, peace_mask) = sim.pact_masks(unit.owner);
        let mut map = Self {
            map_wrap: sim.map_wrap,
            actor: unit.clone(),
            rate: sim.unit_move_rate(unit),
            start,
            current: start,
            style,
            policy,
            movement_class,
            ignores_terrain,
            ignores_zoc: Simulation::unit_ignores_zoc(unit.kind),
            coast_strict,
            class_build_anywhere: class_rules.build_anywhere,
            class_can_occupy_city: class_rules.can_occupy_city,
            omniscient,
            main_sight: (!omniscient).then(|| sim.observer_sight_layers(unit.owner).0),
            allied_mask,
            war_mask,
            peace_mask,
            risk,
            nodes: vec![Node::default(); sim.tiles.len()],
            queue: NormalPathQueue::new(sim.tiles.len()),
            yielded_start: false,
            heuristic_goal: None,
            reachability_only: false,
            expanded: 0,
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

    /// Returns positions in the same increasing-cost order as Freeciv's
    /// `pf_map_*_iterate` macros. Keeping this incremental lets callers stop
    /// once they have found a sufficient destination instead of eagerly
    /// expanding and allocating an entry for every reachable map tile.
    pub(super) fn next_position(&mut self, sim: &Simulation) -> Option<(i32, i32, i32, u32)> {
        if !self.yielded_start {
            self.yielded_start = true;
            return Some((self.actor.x, self.actor.y, 0, 0));
        }
        self.iterate(sim).then(|| {
            let (x, y) = self.map_wrap.position(self.current);
            let node = &self.nodes[self.current];
            (x, y, node.cost / self.rate, node.extra)
        })
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
            .extra_is_native_to_class(base.extra_rule_name(), class)
            .expect("selected C2C3 base extra nativity")
    }

    fn scope_native_tile(&self, sim: &Simulation, kind: UnitKind, x: i32, y: i32) -> bool {
        let observer = (!self.omniscient).then_some(self.actor.owner);
        if self.coast_strict && !sim.safe_ocean_at(x, y, observer) {
            return false;
        }
        debug_assert_eq!(kind, self.actor.kind);
        sim.pf_movement_native_to_class(self.movement_class, x, y)
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
        let city = sim.city_at_cached(x, y);
        if city.is_some_and(|city| {
            self.class_can_occupy_city
                || (city.owner < 64 && ((self.allied_mask >> city.owner) & 1) != 0)
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
                if carrier.owner >= 64
                    || ((self.allied_mask >> carrier.owner) & 1) == 0
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
                let at_base = city.is_some_and(|city| {
                    city.owner < 64 && ((self.allied_mask >> city.owner) & 1) != 0
                }) || Self::native_base(sim, carrier.kind, x, y);
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
        let allied_mask = self.allied_mask;
        sim.any_unit_at(x, y, |unit| {
            unit.owner >= 64 || ((allied_mask >> unit.owner) & 1) == 0
        }) || sim
            .city_at_cached(x, y)
            .is_some_and(|city| city.owner >= 64 || ((allied_mask >> city.owner) & 1) == 0)
    }

    fn initialize(&mut self, sim: &Simulation, index: usize, previous_scope: u8) -> bool {
        if self.nodes[index].status != Status::Uninit {
            return self.nodes[index].behavior != Behavior::Ignore;
        }
        sim.trace
            .count_operation("normal_paths.tiles_initialized", 1);
        let style_counter = match self.style {
            Style::Default => "default",
            Style::Attack => "attack",
            Style::Overlap => "overlap",
            Style::FerrySearch => "ferry_search",
        };
        sim.trace.count_operation(
            match style_counter {
                "default" => "normal_paths.default.tiles_initialized",
                "attack" => "normal_paths.attack.tiles_initialized",
                "overlap" => "normal_paths.overlap.tiles_initialized",
                _ => "normal_paths.ferry_search.tiles_initialized",
            },
            1,
        );
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
                    .is_some_and(|owner| owner < 64 && ((self.peace_mask >> owner) & 1) != 0)
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
                    self.omniscient || self.main_sight.as_ref().is_some_and(|sight| sight[index]),
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
            let fragments = campaign_move_fragments();
            self.nodes[index].extra_tile = (fragments * TURN_FACTOR / 12) as u16;
        } else if self.risk {
            self.nodes[index].extra_tile =
                sim.explorer_stack_risk_with_rate(&self.actor, x, y, self.rate) as u16;
        }
        true
    }

    fn total(&self, cost: i32, extra: u32) -> i64 {
        i64::from(
            cost.wrapping_mul(TURN_FACTOR)
                .wrapping_add(extra.wrapping_mul(self.rate as u32) as i32),
        )
    }

    fn queue_priority(&self, tile: usize, cost: i32, extra: u32) -> i64 {
        let total = self.total(cost, extra);
        let Some(goal) = self.heuristic_goal else {
            return total;
        };
        let (x, y) = self.map_wrap.position(tile);
        let (goal_x, goal_y) = self.map_wrap.position(goal);
        let distance = self.map_wrap.civ3_distance(x, y, goal_x, goal_y);
        let heuristic = i64::from(distance) * i64::from(TURN_FACTOR);
        if self.reachability_only {
            heuristic
        } else {
            total + heuristic
        }
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

    fn path_zone(&mut self, sim: &Simulation, index: usize) -> PathZone {
        *self.nodes[index].zone.get_or_insert_with(|| {
            let (x, y) = self.map_wrap.position(index);
            sim.live_path_zone(self.actor.owner, x, y)
        })
    }

    fn iterate(&mut self, sim: &Simulation) -> bool {
        sim.trace.count_operation("normal_paths.expansions", 1);
        self.expanded += 1;
        let (expansion_counter, neighbor_counter) = match self.style {
            Style::Default => (
                "normal_paths.default.expansions",
                "normal_paths.default.neighbor_checks",
            ),
            Style::Attack => (
                "normal_paths.attack.expansions",
                "normal_paths.attack.neighbor_checks",
            ),
            Style::Overlap => (
                "normal_paths.overlap.expansions",
                "normal_paths.overlap.neighbor_checks",
            ),
            Style::FerrySearch => (
                "normal_paths.ferry_search.expansions",
                "normal_paths.ferry_search.neighbor_checks",
            ),
        };
        sim.trace.count_operation(expansion_counter, 1);
        let current = self.current;
        let node = self.nodes[current];
        if node.behavior != Behavior::DontLeave
            && (node.scope != 0 || self.style == Style::FerrySearch)
            && (self.rate > 0 || node.cost < 0)
        {
            let (x, y) = self.map_wrap.position(current);
            for (direction, (dx, dy)) in CIV3_DIRS.into_iter().enumerate() {
                sim.trace.count_operation("normal_paths.neighbor_checks", 1);
                sim.trace.count_operation(neighbor_counter, 1);
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
                let dest = self.nodes[next];
                // Freeciv PF_NORMAL classifies ZOC per node. Retain live
                // occupancy and the attack exception for this stationary map.
                if dest.action != PfAction::Attack && !self.ignores_zoc && {
                    sim.trace.count_operation("normal_paths.zoc_checks", 1);
                    let from = self.path_zone(sim, current);
                    from != PathZone::Unrestricted
                        && self.path_zone(sim, next).blocks_entry_from(from)
                } {
                    continue;
                }
                let cost = if self.style == Style::FerrySearch {
                    if dest.scope & (NATIVE | CITY) == 0 {
                        0
                    } else if node.scope & (NATIVE | CITY) == 0 {
                        continue;
                    } else {
                        sim.raw_move_cost_for_class_with_war_mask(
                            (x, y),
                            (nx, ny),
                            self.war_mask,
                            self.movement_class,
                            self.ignores_terrain,
                        )
                    }
                } else if dest.action != PfAction::None {
                    if dest.action == PfAction::Attack {
                        if node.scope & NATIVE == 0 {
                            let mut actor = self.actor.clone();
                            (actor.x, actor.y) = (x, y);
                            if !sim.pf_action_step_possible(&actor, dest.action) {
                                continue;
                            }
                        }
                    } else if node.scope & (NATIVE | CITY | TRANSPORT) == 0 {
                        continue;
                    }
                    Simulation::pf_action_move_cost(self.actor.kind, dest.action, self.rate)
                } else if !dest.known {
                    MovementRules::campaign()
                        .unknown_tile_move_cost(self.actor.kind)
                        .expect("selected C2C3 unknown-tile movement cost")
                } else if dest.scope != 0 {
                    sim.trace
                        .count_operation("normal_paths.raw_move_cost_calls", 1);
                    sim.raw_move_cost_for_class_with_war_mask(
                        (x, y),
                        (nx, ny),
                        self.war_mask,
                        self.movement_class,
                        self.ignores_terrain,
                    )
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
                    self.nodes[next].direction = Some(direction as u8);
                    if dest.status == Status::Init {
                        self.nodes[next].status = Status::New;
                        sim.trace.count_operation("normal_paths.queue_inserts", 1);
                        let priority = self.queue_priority(next, cost, extra);
                        self.queue.insert(priority, next);
                    } else {
                        sim.trace
                            .count_operation("normal_paths.queue_decrease_keys", 1);
                        sim.trace.count_operation(
                            "normal_paths.queue_frontier_entries_at_decrease_key",
                            self.queue.frontier_len() as u64,
                        );
                        let priority = self.queue_priority(next, cost, extra);
                        self.queue.put(priority, next);
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

    fn reach_within_raw_cost(
        &mut self,
        sim: &Simulation,
        x: i32,
        y: i32,
        max_raw_cost: i32,
    ) -> Option<usize> {
        let goal = self.map_wrap.index(x, y);
        if goal == self.start {
            return (self.nodes[goal].cost <= max_raw_cost).then_some(goal);
        }
        if self.nodes[goal].status == Status::Processed {
            return (self.nodes[goal].cost <= max_raw_cost).then_some(goal);
        }
        if !self.initialize(sim, goal, 0) {
            return None;
        }
        while self.nodes[goal].status != Status::Processed {
            if self.nodes[self.current].cost > max_raw_cost {
                return None;
            }
            if !self.iterate(sim) {
                return None;
            }
        }
        (self.nodes[goal].cost <= max_raw_cost).then_some(goal)
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

    pub(super) fn movement_cost_within(
        &mut self,
        sim: &Simulation,
        x: i32,
        y: i32,
        max_cost: i32,
    ) -> Option<i32> {
        let max_raw_cost = max_cost
            .saturating_add(self.rate)
            .saturating_sub(self.actor.moves_left);
        let goal = self.reach_within_raw_cost(sim, x, y, max_raw_cost)?;
        Some(
            self.nodes[goal]
                .cost
                .wrapping_sub(self.rate)
                .wrapping_add(self.actor.moves_left),
        )
    }
    pub(super) fn reachable_within<'a>(
        &'a mut self,
        sim: &'a Simulation,
        limit: i32,
    ) -> impl Iterator<Item = (i32, i32, i32)> + 'a {
        std::iter::from_fn(move || {
            if !self.iterate(sim) {
                return None;
            }
            let cost = self.nodes[self.current]
                .cost
                .wrapping_sub(self.rate)
                .wrapping_add(self.actor.moves_left);
            let (x, y) = self.map_wrap.position(self.current);
            Some((x, y, cost))
        })
        .take_while(move |&(_, _, cost)| cost <= limit)
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
            let (dx, dy) = CIV3_DIRS[usize::from(direction)];
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
        let _profile = self.trace.profiler().scope("ai.goto.sanity");
        self.trace.count_operation("ai.goto.sanity_checks", 1);
        if let Some(mut map) = super::fuel_paths::FuelPaths::attack_sanity(self, unit) {
            let _fuel = self.trace.profiler().scope("ai.goto.sanity.fuel");
            return map.movement_cost(self, goal.0, goal.1).is_some();
        }
        let _normal = self.trace.profiler().scope("ai.goto.sanity.normal");
        // Only reachability matters here, so steer the search toward the goal.
        // Any complete search order answers the same question, and A* stops
        // as soon as the goal is settled instead of flooding the reachable map.
        let mut map = NormalPaths::goto_sanity_map(self, unit);
        map.heuristic_goal = Some(self.map_wrap.index(goal.0, goal.1));
        // Edge legality does not depend on accumulated cost while the unit can
        // move, so a goal-directed order reaches the same yes/no answer.
        map.reachability_only = map.rate > 0;
        map.movement_cost(self, goal.0, goal.1).is_some()
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
        self.ai_goto_path_after_sanity(unit, goal, task)
    }

    fn ai_goto_path_after_sanity(
        &self,
        unit: &Unit,
        goal: (i32, i32),
        task: GotoTask,
    ) -> Option<(Vec<(i32, i32)>, i32)> {
        let _profile = self.trace.profiler().scope("ai.goto.route_build");
        self.trace.count_operation("ai.goto.routes_built", 1);
        if let Some(mut map) = super::fuel_paths::FuelPaths::new(self, unit, true) {
            let cost = map.movement_cost(self, goal.0, goal.1)?;
            return Some((map.path(self, goal.0, goal.1)?, cost));
        }
        NormalPaths::goto_map(self, unit, task).path(self, goal.0, goal.1)
    }

    pub(super) fn ai_goto_unit(
        &mut self,
        id: u32,
        goal: (i32, i32),
        task: GotoTask,
    ) -> AiGotoResult {
        self.ai_goto_unit_checked(id, goal, task, false)
    }

    /// `sanity_checked` lets a caller that has just run `ai_goto_is_sane` for
    /// this unit and goal avoid repeating the identical search.
    pub(super) fn ai_goto_unit_checked(
        &mut self,
        id: u32,
        goal: (i32, i32),
        task: GotoTask,
        sanity_checked: bool,
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
        if (unit.x, unit.y) != goal && !sanity_checked && !self.ai_goto_is_sane(&unit, goal) {
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
            && let Some((path, _)) = self.ai_goto_path_after_sanity(&unit, goal, task)
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
        sim.tile_mut(target.0, target.1).known_by.set(HUMAN, false);

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
