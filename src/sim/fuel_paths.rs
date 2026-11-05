//! Native PF_FUEL distances and paths for air advisors and leader escape.
//! Parameters are frozen, while node initialization is lazy against the live
//! world, just as the enemy map retained by dai_manage_barbarian_leader.
use super::exploration::ExplorerQueue;
use super::*;

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
        let start = Simulation::idx(unit.x, unit.y);
        let mut map = Self {
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
        let (x, y) = Simulation::xy(index);
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
                let (nx, ny) = Simulation::xy(candidate);
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
            x,
            y,
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
        let (x, y) = Simulation::xy(index);
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
            let (x, y) = Simulation::xy(cursor);
            let (dx, dy) = ISOHEX_DIRS[direction];
            let (px, py) = offset_native_by_map(x, y, -dx, -dy);
            cursor = Simulation::idx(px, py);
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
                let (x, y) = Simulation::xy(index);
                for (direction, (dx, dy)) in ISOHEX_DIRS.into_iter().enumerate() {
                    let (nx, ny) = offset_native_by_map(x, y, dx, dy);
                    let next = Simulation::idx(nx, ny);
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
        let target = Simulation::idx(x, y);
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
        let mut index = Simulation::idx(x, y);
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
            let point = Simulation::xy(index);
            let previous = offset_native_by_map(point.0, point.1, -step.0, -step.1);
            index = Simulation::idx(previous.0, previous.1);
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
            let (x, y) = Simulation::xy(self.start);
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
            let (x, y) = Simulation::xy(index);
            result.push((x, y, cost));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Simulation {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        sim.prune_unit_ai_state();
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.base = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        sim
    }

    fn actor(sim: &mut Simulation, kind: UnitKind, position: (i32, i32)) -> Unit {
        let id = sim.spawn_unit(HUMAN, kind, position.0, position.1);
        sim.units.iter().find(|unit| unit.id == id).unwrap().clone()
    }

    #[test]
    fn desktop_fuel_fixture_observes_real_leader_dispatch_before_and_after_a_refuel_point() {
        assert!(world().ai_aircraft_fuel_path_playtest());
    }

    #[test]
    fn type_fuel_selects_the_map_even_when_instance_fuel_is_malformed() {
        let mut sim = world();
        let mut ship = actor(&mut sim, UnitKind::Caravel, (10, 20));
        ship.fuel = 1;
        assert!(FuelPaths::new(&sim, &ship, true).is_none());
        let mut plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
        plane.fuel = 0;
        let map = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(map.nodes[map.start].moves, moves_field(map.initial_bundle));
        assert_eq!(moves_field(-1), 4095);
        assert_eq!(required_field(-1), -1);
        assert_eq!(required_field(4096), -4096);
    }

    #[test]
    fn an_isolated_aircraft_without_any_refuel_point_cannot_leave_its_start() {
        let mut sim = world();
        let plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
        let target = offset_native_by_map(10, 20, 1, 0);
        sim.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, target.0, target.1);
        let mut map = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(map.movement_cost(&sim, 10, 20), Some(0));
        assert_eq!(map.movement_cost(&sim, target.0, target.1), None);
    }

    #[test]
    fn waiting_and_revisiting_preserve_the_earliest_position_separately_from_more_fuel() {
        let mut sim = world();
        sim.create_city(HUMAN, 10, 20);
        let start = offset_native_by_map(10, 20, 1, 0);
        let mut plane = actor(&mut sim, UnitKind::Fighter, start);
        plane.fuel = 1;
        plane.moves_left = 3 * MOVE_FRAGMENTS;
        let target = offset_native_by_map(10, 20, 4, 0);
        let earlier = offset_native_by_map(10, 20, 2, 0);
        let mut map = FuelPaths::new(&sim, &plane, true).unwrap();
        // Return one step, spend two MP waiting, then fly four steps.
        assert_eq!(
            map.movement_cost(&sim, target.0, target.1),
            Some(7 * MOVE_FRAGMENTS)
        );
        assert_eq!(
            map.movement_cost(&sim, earlier.0, earlier.1),
            Some(MOVE_FRAGMENTS)
        );
        assert_eq!(map.movement_cost(&sim, start.0, start.1), Some(0));
        assert!(
            map.positions
                .iter()
                .any(|position| position.previous.is_some())
        );
        let path = map.path(&sim, target.0, target.1).unwrap();
        assert_eq!(path.iter().filter(|&&step| step == (0, 0)).count(), 1);
        let mut point = start;
        let mut visited = Vec::new();
        for &(dx, dy) in &path {
            point = offset_native_by_map(point.0, point.1, dx, dy);
            visited.push(point);
        }
        assert_eq!(point, target);
        assert!(visited.contains(&start)); // B -> A, wait, B -> C -> D -> E.
        let live = sim
            .units
            .iter_mut()
            .find(|unit| unit.id == plane.id)
            .unwrap();
        live.fuel = plane.fuel;
        live.moves_left = plane.moves_left;
        let result = sim.follow_source_ai_path(plane.id, path);
        assert!(result.alive && result.moved && !result.at_goal);
        let live = sim.units.iter().find(|unit| unit.id == plane.id).unwrap();
        assert_eq!((live.x, live.y), (10, 20));
        assert_eq!(live.moves_left, 2 * MOVE_FRAGMENTS);
    }

    #[test]
    fn full_turn_boundary_refills_implicitly_but_partial_turn_refill_pays_its_wait() {
        let mut sim = world();
        sim.create_city(HUMAN, 10, 20);
        let mut plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
        plane.fuel = 1;
        let target = offset_native_by_map(10, 20, 8, 0);
        let mut boundary = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(
            boundary.movement_cost(&sim, target.0, target.1),
            Some(8 * MOVE_FRAGMENTS)
        );
        assert_eq!(
            boundary.path(&sim, target.0, target.1).unwrap().first(),
            Some(&(0, 0))
        );
        plane.moves_left = 3 * MOVE_FRAGMENTS;
        let mut partial = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(
            partial.movement_cost(&sim, target.0, target.1),
            Some(11 * MOVE_FRAGMENTS)
        );
        assert_eq!(
            partial.path(&sim, target.0, target.1).unwrap().first(),
            Some(&(0, 0))
        );
    }

    #[test]
    fn fuel_goto_executes_only_the_prefix_before_refuel_wait() {
        let mut sim = world();
        sim.create_city(HUMAN, 10, 20);
        let start = offset_native_by_map(10, 20, 1, 0);
        let target = offset_native_by_map(10, 20, 4, 0);
        let plane = actor(&mut sim, UnitKind::Fighter, start);
        let live = sim
            .units
            .iter_mut()
            .find(|unit| unit.id == plane.id)
            .unwrap();
        live.moves_left = 3 * MOVE_FRAGMENTS;
        live.fuel = 1;
        let plane = live.clone();
        assert!(sim.ai_goto_is_sane(&plane, target));
        let (path, cost) = sim
            .ai_goto_path(&plane, target, super::super::normal_paths::GotoTask::None)
            .unwrap();
        assert!(path.contains(&(0, 0)));
        assert_eq!(cost, 7 * MOVE_FRAGMENTS);
        let result = sim.ai_goto_unit(plane.id, target, super::super::normal_paths::GotoTask::None);
        assert!(result.alive && result.moved && !result.at_goal);
        let live = sim.units.iter().find(|unit| unit.id == plane.id).unwrap();
        assert_eq!((live.x, live.y), (10, 20));
        assert_eq!(live.moves_left, 2 * MOVE_FRAGMENTS);
    }

    #[test]
    fn one_attack_bomber_returns_to_refuel_instead_of_attacking_on_last_fuel_turn() {
        let mut sim = world();
        sim.create_city(HUMAN, 10, 20);
        let start = offset_native_by_map(10, 20, 1, 0);
        let target = offset_native_by_map(10, 20, 2, 0);
        let mut plane = actor(&mut sim, UnitKind::Bomber, start);
        sim.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, target.0, target.1);
        let rate = sim.unit_move_rate(&plane);
        plane.fuel = 1;
        let mut low = FuelPaths::new(&sim, &plane, true).unwrap();
        assert!(!low.attack_possible(rate, MOVE_FRAGMENTS));
        assert_eq!(low.movement_cost(&sim, target.0, target.1), Some(2 * rate));
        plane.fuel = plane.kind.fuel_capacity();
        let mut full = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(full.movement_cost(&sim, target.0, target.1), Some(rate));
    }

    #[test]
    fn fighter_attack_uses_source_return_requirement_and_missiles_can_be_suicidal() {
        let mut sim = world();
        sim.create_city(HUMAN, 10, 20);
        let mut fighter = actor(&mut sim, UnitKind::Fighter, (10, 20));
        fighter.fuel = 1;
        fighter.moves_left = MOVE_FRAGMENTS;
        let map = FuelPaths::new(&sim, &fighter, true).unwrap();
        assert!(map.attack_possible(MOVE_FRAGMENTS, 0));
        assert!(!map.attack_possible(MOVE_FRAGMENTS, MOVE_FRAGMENTS));
        let missile = actor(&mut sim, UnitKind::CruiseMissile, (10, 20));
        let map = FuelPaths::new(&sim, &missile, true).unwrap();
        assert!(map.attack_possible(MOVE_FRAGMENTS, 10 * MOVE_FRAGMENTS));
    }

    #[test]
    fn refuel_callback_keeps_enemy_base_and_omniscient_known_map_quirks() {
        let mut sim = world();
        let plane = actor(&mut sim, UnitKind::Fighter, (10, 20));
        let base = offset_native_by_map(10, 20, 1, 0);
        sim.tile_mut(base.0, base.1).base = Some(Base::Airstrip);
        sim.tile_mut(base.0, base.1).owner = Some(1);
        for tile in &mut sim.tiles {
            tile.known_by[HUMAN] = false;
        }
        let map = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(map.required_moves(&sim, Simulation::idx(base.0, base.1)), 0);
        assert_eq!(map.required_moves(&sim, Simulation::idx(10, 20)), -1);
        sim.tile_mut(base.0, base.1).known_by[HUMAN] = true;
        assert_eq!(
            map.required_moves(&sim, Simulation::idx(10, 20)),
            MOVE_FRAGMENTS
        );
        assert!(sim.refuel_site(HUMAN, base.0, base.1));
    }

    #[test]
    fn initialized_action_nodes_retain_native_cached_endpoints_when_the_live_stack_changes() {
        let mut sim = world();
        sim.create_city(HUMAN, 10, 20);
        let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
        let target = offset_native_by_map(10, 20, 1, 0);
        sim.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        let ship = sim.spawn_unit(1, UnitKind::Carrier, target.0, target.1);
        sim.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
        let mut cached = FuelPaths::new(&sim, &plane, true).unwrap();
        assert!(cached.initialize(&sim, Simulation::idx(target.0, target.1)));
        sim.units.retain(|unit| unit.id != ship);
        let mut fresh = FuelPaths::new(&sim, &plane, true).unwrap();
        assert_eq!(fresh.movement_cost(&sim, target.0, target.1), None);
        assert_eq!(
            cached.movement_cost(&sim, target.0, target.1),
            Some(sim.unit_move_rate(&plane))
        );
    }

    #[test]
    fn full_allied_carrier_is_a_possible_refuel_point_and_unreachable_aircraft_do_not_shield_a_ship()
     {
        let mut sim = world();
        sim.tile_mut(10, 20).terrain = Terrain::Ocean;
        let plane = actor(&mut sim, UnitKind::Bomber, (10, 20));
        let carrier = sim.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
        for _ in 0..8 {
            let cargo = sim.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
            assert!(sim.board_specific_ship(cargo, carrier));
        }
        assert_eq!(sim.cargo_count(carrier), 8);
        assert!(!sim.board_specific_ship(plane.id, carrier));
        let map = FuelPaths::new(&sim, &plane, true).unwrap();
        assert!(map.possible_base(&sim, 10, 20));
        let target = offset_native_by_map(10, 20, 1, 0);
        sim.tile_mut(target.0, target.1).terrain = Terrain::Ocean;
        let ship = sim.spawn_unit(1, UnitKind::Carrier, target.0, target.1);
        sim.spawn_unit(1, UnitKind::Fighter, target.0, target.1);
        assert_eq!(map.attack_at(&sim, target.0, target.1, true), Ok(true));
        sim.units.retain(|unit| unit.id != ship);
        assert_eq!(map.attack_at(&sim, target.0, target.1, true), Err(()));
    }
}

impl Simulation {
    pub(crate) fn ai_aircraft_fuel_path_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        self.spawn_unit(HUMAN, UnitKind::Fighter, 10, 20);
        let point = offset_native_by_map(10, 20, 1, 0);
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, point.0, point.1);
        self.run_ai_player(BARBARIANS);
        let unreachable = self.units.iter().any(|unit| {
            unit.id == leader && (unit.x, unit.y) == point && unit.moves_left == 2 * MOVE_FRAGMENTS
        });
        // A real refuel point makes the same aircraft's native fuel map
        // reachable; the leader can now follow increasing enemy costs.
        self.create_city(HUMAN, 10, 20);
        self.run_ai_player(BARBARIANS);
        unreachable
            && self.units.iter().any(|unit| {
                unit.id == leader
                    && unit.moves_left == 0
                    && self.map_distance(10, 20, unit.x, unit.y) > 1
            })
    }
}
