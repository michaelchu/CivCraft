//! Native PF_FUEL distances and paths for air advisors and leader escape.
//! Parameters are frozen, while node initialization is lazy against the live
//! world, just as the enemy map retained by dai_manage_barbarian_leader.
use super::exploration::ExplorerQueue;
use super::*;

mod playtest;

const TURN_FACTOR: i32 = 65_536;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Uninit,
    Init,
    New,
    Processed,
    Waiting,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Behavior {
    Normal,
    DontLeave,
    Ignore,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Position {
    cost: i32,
    moves: u16,
    direction: Option<usize>,
    previous: Option<usize>,
}

#[derive(Clone)]
struct Node {
    cost: i32,
    moves: u16, // Native unsigned 12-bit field.
    direction: Option<usize>,
    status: Status,
    behavior: Behavior,
    attack: bool,
    known: bool,
    required: i32,       // Native signed 13-bit field.
    step_cost: [u16; 6], // Cached MC + 2, zero means not yet computed.
    pos: Option<usize>,
    segment: Option<usize>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            cost: 0,
            moves: 0,
            direction: None,
            status: Status::Uninit,
            behavior: Behavior::Normal,
            attack: false,
            known: false,
            required: 0,
            step_cost: [0; 6],
            pos: None,
            segment: None,
        }
    }
}

pub(super) struct FuelPaths {
    map_wrap: MapWrap,
    actor: Unit,
    rate: i32,
    type_fuel: i32,
    initial_bundle: i32,
    omniscient: bool,
    city_attack: bool,
    current: usize,
    start: usize,
    nodes: Vec<Node>,
    positions: Vec<Position>,
    queue: ExplorerQueue,
    waited_queue: ExplorerQueue,
}

fn moves_field(value: i32) -> u16 {
    (value & 0xfff) as u16
}
fn required_field(value: i32) -> i32 {
    ((value & 0x1fff) ^ 0x1000) - 0x1000
}

impl FuelPaths {
    pub(super) fn new(sim: &Simulation, unit: &Unit, omniscient: bool) -> Option<Self> {
        Self::with_actions(sim, unit, omniscient, true)
    }

    pub(super) fn attack_sanity(sim: &Simulation, unit: &Unit) -> Option<Self> {
        Self::with_actions(sim, unit, false, false)
    }

    fn with_actions(
        sim: &Simulation,
        unit: &Unit,
        omniscient: bool,
        city_attack: bool,
    ) -> Option<Self> {
        // pft_fill_parameter selects fuel mode by TYPE fuel, never stale
        // instance fuel on a naval/ground unit loaded from an old save.
        let type_fuel = i32::from(unit.kind.fuel_capacity());
        if type_fuel == 0 {
            return None;
        }
        let rate = sim.unit_move_rate(unit);
        let initial_bundle = unit
            .moves_left
            .wrapping_add((i32::from(unit.fuel) - 1).wrapping_mul(rate));
        let start = sim.map_wrap.index(unit.x, unit.y);
        let mut map = Self {
            map_wrap: sim.map_wrap,
            actor: unit.clone(),
            rate,
            type_fuel,
            initial_bundle,
            omniscient,
            city_attack,
            current: start,
            start,
            nodes: vec![Node::default(); sim.tiles.len()],
            positions: Vec::new(),
            queue: ExplorerQueue::default(),
            waited_queue: ExplorerQueue::default(),
        };
        map.initialize(sim, start);
        let budget = moves_field(initial_bundle);
        map.nodes[start].moves = budget;
        map.nodes[start].cost = map.full_moves().wrapping_sub(i32::from(budget));
        map.nodes[start].status = Status::Processed;
        let position = map.snapshot(start);
        map.positions.push(position);
        map.nodes[start].pos = Some(0);
        map.nodes[start].segment = Some(0);
        Some(map)
    }

    fn full_moves(&self) -> i32 {
        self.rate.wrapping_mul(self.type_fuel)
    }
    fn suicide(&self) -> bool {
        self.actor.kind.is_missile()
    }
    fn all_attack_moves(&self) -> bool {
        matches!(self.actor.kind, UnitKind::Bomber | UnitKind::StealthBomber)
    }

    fn main_sight(&self, sim: &Simulation, x: i32, y: i32) -> bool {
        sim.pf_main_sight(self.actor.owner, x, y)
    }

    fn possible_base(&self, sim: &Simulation, x: i32, y: i32) -> bool {
        let known = self.omniscient || sim.tile(x, y).known_by[self.actor.owner];
        if !known {
            return false;
        }
        if sim
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && sim.allied(self.actor.owner, city.owner))
        {
            return true;
        }
        // Neither native PF nor actual refueling tests base ownership.
        // PF remains optimistic about unseen bases and carrier capacity.
        if matches!(sim.tile(x, y).base, Some(Base::Airstrip | Base::Airbase)) {
            return true;
        }
        if !self.omniscient && !self.main_sight(sim, x, y) {
            return false;
        }
        sim.tile_units_ordered(x, y).any(|carrier| {
            (carrier.x, carrier.y) == (x, y)
            && sim.allied(self.actor.owner, carrier.owner)
            && carrier.kind != self.actor.kind && carrier.kind.can_carry(self.actor.kind)
            // pf_transport_check does not test available cargo slots.
            && !self.actor.kind.can_carry(carrier.kind)
        })
    }

    fn required_moves(&self, sim: &Simulation, index: usize) -> i32 {
        let (x, y) = self.map_wrap.position(index);
        if self.possible_base(sim, x, y) {
            return 0;
        }
        let maximum = self.initial_bundle.max(self.full_moves()) / MOVE_FRAGMENTS;
        sim.tiles
            .iter()
            .enumerate()
            .filter_map(|(candidate, tile)| {
                // Deliberate pf_tools.c quirk: this test still applies when
                // omniscience is TRUE in the enclosing PF parameters.
                if !tile.known_by[self.actor.owner] {
                    return None;
                }
                let (nx, ny) = self.map_wrap.position(candidate);
                let distance = sim.map_distance(x, y, nx, ny);
                (distance <= maximum && self.possible_base(sim, nx, ny)).then_some(distance)
            })
            .min()
            .map_or(-1, |distance| distance * MOVE_FRAGMENTS)
    }

    fn attack_at(&self, sim: &Simulation, x: i32, y: i32, seen: bool) -> Result<bool, ()> {
        match sim.pf_default_action(
            self.actor.kind,
            self.actor.owner,
            (x, y),
            true,
            seen,
            self.city_attack,
        ) {
            super::pf_actions::PfAction::Impossible => Err(()),
            super::pf_actions::PfAction::None => Ok(false),
            _ => Ok(true),
        }
    }

    fn initialize(&mut self, sim: &Simulation, index: usize) -> bool {
        if self.nodes[index].status != Status::Uninit {
            return self.nodes[index].behavior != Behavior::Ignore;
        }
        self.nodes[index].status = Status::Init;
        let (x, y) = self.map_wrap.position(index);
        let known = self.omniscient || sim.tile(x, y).known_by[self.actor.owner];
        self.nodes[index].known = known;
        if known {
            if !self.actor.kind.is_civilian()
                && sim
                    .tile(x, y)
                    .owner
                    .is_some_and(|owner| sim.pact(self.actor.owner, owner) == Pact::Peace)
            {
                self.nodes[index].behavior = Behavior::Ignore;
                if index != self.start {
                    return false;
                }
            }
            let seen = self.omniscient || self.main_sight(sim, x, y);
            match self.attack_at(sim, x, y, seen) {
                Err(()) => {
                    self.nodes[index].behavior = Behavior::Ignore;
                    return index == self.start;
                }
                Ok(true) => {
                    self.nodes[index].attack = true;
                    self.nodes[index].behavior = Behavior::DontLeave;
                    self.nodes[index].required = 0;
                    return true;
                }
                Ok(false) => (),
            }
        }
        let required = required_field(self.required_moves(sim, index));
        self.nodes[index].required = required;
        if required == -1 {
            self.nodes[index].behavior = if index == self.start {
                Behavior::DontLeave
            } else {
                Behavior::Ignore
            };
            return index == self.start;
        }
        true
    }

    fn snapshot(&self, index: usize) -> Position {
        let node = &self.nodes[index];
        Position {
            cost: node.cost,
            moves: node.moves,
            direction: node.direction,
            previous: None,
        }
    }

    fn same_snapshot(&self, position: usize, index: usize) -> bool {
        let snapshot = self.snapshot(index);
        let old = &self.positions[position];
        snapshot.cost == old.cost
            && snapshot.moves == old.moves
            && snapshot.direction == old.direction
    }

    fn create_segment(&mut self, index: usize) {
        let mut cursor = index;
        let mut tail = self.positions.len();
        self.positions.push(self.snapshot(cursor));
        self.nodes[cursor].pos = Some(tail);
        loop {
            let Some(direction) = self.nodes[cursor].direction else {
                break;
            };
            let (x, y) = self.map_wrap.position(cursor);
            let (dx, dy) = ISOHEX_DIRS[direction];
            let Some((px, py)) = self.map_wrap.offset(x, y, -dx, -dy) else {
                continue;
            };
            cursor = self.map_wrap.index(px, py);
            if let Some(position) = self.nodes[cursor].pos
                && self.same_snapshot(position, cursor)
            {
                self.positions[tail].previous = Some(position);
                break;
            }
            let previous = self.positions.len();
            self.positions.push(self.snapshot(cursor));
            self.nodes[cursor].pos = Some(previous);
            self.positions[tail].previous = Some(previous);
            tail = previous;
            if self.nodes[cursor].required == 0 || self.nodes[cursor].direction.is_none() {
                break;
            }
        }
    }

    fn priority(&self, cost: i32, moves: i32, required: i32) -> i64 {
        // No get_EC callback is installed for the frozen enemy map. Keep
        // native signed-32 priority arithmetic, including negative offsets.
        i64::from(
            TURN_FACTOR
                .wrapping_mul(cost)
                .wrapping_sub(moves.wrapping_sub(required)),
        )
    }
    fn waited_priority(cost: i32, safety: i32) -> i64 {
        i64::from(
            TURN_FACTOR
                .wrapping_mul(cost.wrapping_add(1))
                .wrapping_sub(safety)
                .wrapping_sub(1),
        )
    }
    fn fill_cost(&self, cost: i32, moves: u16) -> i32 {
        cost.wrapping_add(i32::from(moves) % self.rate)
    }
    fn dangerous(&self, index: usize) -> bool {
        self.nodes[index].pos.is_none_or(|position| {
            i32::from(self.positions[position].moves) < self.nodes[index].required
                && !self.nodes[index].attack
        })
    }
    fn attack_possible(&self, before: i32, source_required: i32) -> bool {
        if self.suicide() {
            true
        } else if self.all_attack_moves() {
            before > self.rate
        } else {
            before - MOVE_FRAGMENTS >= source_required
        }
    }

    fn iterate(&mut self, sim: &Simulation) -> bool {
        let mut index = self.current;
        let mut waited = false;
        loop {
            let node = self.nodes[index].clone();
            if node.behavior != Behavior::DontLeave && (self.rate > 0 || node.cost < 0) {
                let mut budget = i32::from(node.moves);
                if node.required == 0
                    && self.rate > 0
                    && budget % self.rate == 0
                    && node.cost >= self.actor.moves_left
                {
                    budget = self.full_moves();
                    self.nodes[index].moves = moves_field(budget);
                }
                let (x, y) = self.map_wrap.position(index);
                for (direction, (dx, dy)) in ISOHEX_DIRS.into_iter().enumerate() {
                    let Some((nx, ny)) = self.map_wrap.offset(x, y, dx, dy) else {
                        continue;
                    };
                    let next = self.map_wrap.index(nx, ny);
                    if matches!(self.nodes[next].status, Status::Processed | Status::Waiting)
                        && self.nodes[next].required == 0
                    {
                        continue;
                    }
                    if !self.initialize(sim, next) {
                        continue;
                    }
                    let destination = self.nodes[next].clone();
                    let mut cost = if destination.step_cost[direction] == 0 {
                        let cost = if destination.attack {
                            if self.all_attack_moves() || self.suicide() {
                                self.rate
                            } else {
                                MOVE_FRAGMENTS
                            }
                        } else {
                            MOVE_FRAGMENTS
                        };
                        self.nodes[next].step_cost[direction] = cost.wrapping_add(2) as u16;
                        cost
                    } else if destination.step_cost[direction] == 1 {
                        continue;
                    } else {
                        i32::from(destination.step_cost[direction]) - 2
                    };
                    let remaining = if self.rate > 0 {
                        let remainder = budget % self.rate;
                        if remainder == 0 { self.rate } else { remainder }
                    } else {
                        budget
                    };
                    cost = cost.min(remaining);
                    let moves = budget - cost;
                    if moves < destination.required && (!self.suicide() || moves < 0) {
                        continue;
                    }
                    if destination.attack && !self.attack_possible(budget, node.required) {
                        continue;
                    }
                    cost = node.cost.wrapping_add(cost);
                    let priority = self.priority(cost, moves, destination.required);
                    let old_priority = if let Some(position) = destination.segment {
                        self.priority(
                            self.positions[position].cost,
                            i32::from(self.positions[position].moves),
                            destination.required,
                        )
                    } else {
                        self.priority(
                            destination.cost,
                            i32::from(destination.moves),
                            destination.required,
                        )
                    };
                    if destination.status == Status::Init
                        || (destination.status == Status::New && priority < old_priority)
                    {
                        self.nodes[next].cost = cost;
                        self.nodes[next].moves = moves_field(moves);
                        self.nodes[next].direction = Some(direction);
                        self.create_segment(next);
                        if destination.status == Status::Init {
                            self.nodes[next].status = Status::New;
                            self.queue.insert(priority, next);
                        } else if priority < old_priority {
                            self.queue.put(priority, next);
                        }
                        continue;
                    }
                    if waited
                        && destination.status != Status::New
                        && destination.required != 0
                        && (moves > i32::from(destination.moves))
                    {
                        self.nodes[next].cost = cost;
                        self.nodes[next].moves = moves_field(moves);
                        self.nodes[next].direction = Some(direction);
                        self.waited_queue.insert(
                            Self::waited_priority(cost, moves - destination.required),
                            next,
                        );
                    }
                }
            }
            if node.status == Status::Waiting {
                self.nodes[index].status = Status::Processed;
            } else if node.required == 0
                && !node.attack
                && i32::from(self.nodes[index].moves) < self.full_moves()
                && self.rate > 0
                && (i32::from(self.nodes[index].moves) % self.rate != 0
                    || node.cost < self.actor.moves_left)
            {
                self.nodes[index].status = Status::Waiting;
                self.queue.insert(
                    Self::waited_priority(
                        self.fill_cost(node.cost, self.nodes[index].moves),
                        self.full_moves(),
                    ),
                    index,
                );
            }
            let use_waited = self.queue.peek().is_none()
                || self.waited_queue.peek().is_some_and(|(priority, _)| {
                    self.queue.peek().is_some_and(|(main, _)| priority < main)
                });
            if use_waited {
                let Some((_, next)) = self.waited_queue.pop() else {
                    return false;
                };
                index = next;
                self.current = next;
                waited = true;
            } else {
                let Some((_, next)) = self.queue.pop() else {
                    return false;
                };
                index = next;
                self.current = next;
                waited = self.nodes[index].status == Status::Waiting;
                if waited {
                    self.nodes[index].cost =
                        self.fill_cost(self.nodes[index].cost, self.nodes[index].moves);
                    self.nodes[index].moves = moves_field(self.full_moves());
                } else if !self.dangerous(index) {
                    self.nodes[index].status = Status::Processed;
                    self.nodes[index].segment = self.nodes[index].pos;
                    return true;
                }
            }
        }
    }

    pub(super) fn movement_cost(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<i32> {
        let target = self.map_wrap.index(x, y);
        if target == self.start {
            return Some(0);
        }
        if !self.initialize(sim, target) {
            return None;
        }
        while self.nodes[target].segment.is_none() {
            if !self.iterate(sim) {
                return None;
            }
        }
        let position = self.positions[self.nodes[target].segment.unwrap()];
        let cost = position
            .cost
            .wrapping_sub(self.full_moves())
            .wrapping_add(self.initial_bundle);
        (cost != -1).then_some(cost)
    }

    // pf_fuel_map_construct_path retains the selected immutable segments,
    // including repeated visits and duplicate refuel positions (waiting).
    pub(super) fn path(&mut self, sim: &Simulation, x: i32, y: i32) -> Option<Vec<(i32, i32)>> {
        self.movement_cost(sim, x, y)?;
        let mut index = self.map_wrap.index(x, y);
        if index == self.start {
            return Some(Vec::new());
        }
        let mut segment = self.nodes[index].segment?;
        let mut backwards = Vec::new();
        loop {
            if self.nodes[index].required == 0 && Some(segment) != self.nodes[index].segment {
                backwards.push((0, 0));
                segment = self.nodes[index].segment?;
            }
            let position = self.positions[segment];
            let Some(direction) = position.direction else {
                break;
            };
            let step = ISOHEX_DIRS[direction];
            backwards.push(step);
            let point = self.map_wrap.position(index);
            let previous = self.map_wrap.offset(point.0, point.1, -step.0, -step.1)?;
            index = self.map_wrap.index(previous.0, previous.1);
            segment = position.previous?;
        }
        backwards.reverse();
        Some(backwards)
    }

    // A fresh map's native iteration order, including its fuel/slack tie
    // priorities. Air advisors stop at the first out-of-budget position.
    pub(super) fn reachable_within(
        &mut self,
        sim: &Simulation,
        limit: i32,
        strict: bool,
        include_start: bool,
    ) -> Vec<(i32, i32, i32)> {
        let mut result = Vec::new();
        if include_start && (!strict && limit >= 0 || strict && limit > 0) {
            let (x, y) = self.map_wrap.position(self.start);
            result.push((x, y, 0));
        }
        while self.iterate(sim) {
            let index = self.current;
            let Some(segment) = self.nodes[index].segment else {
                continue;
            };
            let cost = self.positions[segment]
                .cost
                .wrapping_sub(self.full_moves())
                .wrapping_add(self.initial_bundle);
            if cost > limit || strict && cost == limit {
                break;
            }
            let (x, y) = self.map_wrap.position(index);
            result.push((x, y, cost));
        }
        result
    }
}

#[cfg(test)]
#[path = "fuel_paths/tests.rs"]
mod tests;

impl Simulation {}
