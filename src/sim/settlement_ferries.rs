use super::normal_paths::GotoTask;
use super::*;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

const SETTLEMENT_FERRIES: [UnitKind; 4] = [
    UnitKind::Trireme,
    UnitKind::Caravel,
    UnitKind::Galleon,
    UnitKind::Transport,
];

struct Rendezvous {
    tile: (i32, i32),
    turns: i32,
    walk: Vec<(i32, i32)>,
}

pub(super) struct SettlementFerry {
    pub(super) founder: u32,
    pickup: (i32, i32),
    pickup_walk: Vec<(i32, i32)>,
    approach: (i32, i32),
    landing: (i32, i32),
    destination: (i32, i32),
    inland_path: Vec<(i32, i32)>,
    value: f64,
}

impl Simulation {
    pub(super) fn ferry_tech_want(&self, owner: usize, tech: Tech) -> i32 {
        if owner == BARBARIANS
            || !self.players[owner].ai
            || tech != Tech::MAP_MAKING
            || self.has_tech(owner, tech)
            || SETTLEMENT_FERRIES
                .into_iter()
                .any(|kind| self.unit_available(owner, kind))
        {
            return 0;
        }
        let ferries: Vec<_> = self
            .units
            .iter()
            .filter(|ship| {
                ship.owner == owner
                    && ship.kind.is_naval()
                    && ship.kind.can_carry(UnitKind::Settlers)
                    && self.cargo_count(ship.id) == 0
            })
            .collect();
        let ports = self
            .cities
            .iter()
            .filter(|city| {
                city.owner == owner
                    && self.is_port(city)
                    && !ferries.iter().any(|ship| {
                        (ship.x, ship.y) == (city.x, city.y)
                            || !self.path_for_unit(ship, city.x, city.y).is_empty()
                    })
            })
            .count() as i32;
        // Freeciv's settlement advisor assigns this want to the first ferry role's tech.
        500 * ports
    }

    pub(super) fn settlement_production_choice(&self, city: &City) -> Option<(UnitKind, f64)> {
        let waiting = self.units.iter().find(|unit| {
            unit.owner == city.owner
                && unit.kind == UnitKind::Settlers
                && unit.transport.is_none()
                && (unit.x, unit.y) == (city.x, city.y)
        });
        if waiting.is_none() && city.size <= UnitKind::Settlers.stats().pop_cost {
            return None;
        }
        let virtual_founder = self.make_unit(0, city.owner, UnitKind::Settlers, city.x, city.y);
        let founder = waiting.unwrap_or(&virtual_founder);
        let land = self
            .best_city_site(founder)
            .map_or(0.0, |(_, _, value)| value);
        let existing = self
            .best_settlement_ferry(founder, |_| false)
            .map_or(0.0, |(_, plan)| plan.value);
        if existing > 0.0 {
            return waiting
                .is_none()
                .then_some((UnitKind::Settlers, existing.max(land)));
        }

        if !self.is_port(city) {
            return (waiting.is_none() && land > 0.0).then_some((UnitKind::Settlers, land));
        }
        let forecast = self.city_forecast(city);
        let surplus = forecast.shields - forecast.support.shields;
        if surplus > 0 && !forecast.disorder {
            let labels = self.continent_labels();
            let origin = labels[self.map_wrap.index(city.x, city.y)];
            let new_ferry = SETTLEMENT_FERRIES
                .into_iter()
                .filter(|&kind| self.unit_buildable(city, kind))
                .filter_map(|kind| {
                    let ship = self.make_unit(0, city.owner, kind, city.x, city.y);
                    let invested = if city.production == Production::Unit(kind) {
                        city.shields
                    } else {
                        0
                    };
                    let build_turns =
                        ((kind.stats().cost - invested).max(0) + surplus - 1) / surplus;
                    self.settlement_ferry_route(
                        founder,
                        &ship,
                        &labels,
                        origin,
                        &[Rendezvous {
                            tile: (city.x, city.y),
                            turns: build_turns,
                            walk: Vec::new(),
                        }],
                    )
                    .map(|plan| (kind, plan.value))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((kind, value)) = new_ferry
                && value > land
            {
                return Some((kind, value));
            }
        }
        (waiting.is_none() && land > 0.0).then_some((UnitKind::Settlers, land))
    }

    fn settler_rendezvous_points(&self, founder: &Unit, ship: &Unit) -> Vec<Rendezvous> {
        let start = self.map_wrap.index(founder.x, founder.y);
        let ship_speed = self.unit_move_rate(&ship).max(1);
        let land_speed = self.unit_move_rate(&founder).max(1);
        let land_initial = founder.moves_left.max(0);
        let sea_initial = ship.moves_left.max(0);
        let arrival = |cost: i32, initial: i32, speed: i32| {
            if cost <= initial {
                0
            } else {
                (cost - initial + speed - 1) / speed
            }
        };
        let mut costs = vec![i32::MAX; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        let mut checked = vec![None; self.tiles.len()];
        let mut rendezvous = vec![None; self.tiles.len()];
        costs[start] = 0;
        let mut traveler = founder.clone();
        while let Some(Reverse((cost, index))) = queue.pop() {
            if cost != costs[index] {
                continue;
            }
            let land_turns = arrival(cost, land_initial, land_speed);
            let arrival_remaining = if cost <= land_initial {
                land_initial - cost
            } else {
                land_speed - (cost - land_initial - 1) % land_speed - 1
            };
            let step_remaining = if cost < land_initial {
                land_initial - cost
            } else {
                land_speed - (cost - land_initial) % land_speed
            };
            let (x, y) = self.map_wrap.position(index);
            (traveler.x, traveler.y) = (x, y);
            traveler.moves_left = step_remaining;
            let port = self.cities.iter().any(|city| {
                self.allied(city.owner, founder.owner)
                    && (city.x, city.y) == (x, y)
                    && self.is_port(city)
            });
            for pickup in
                port.then_some((x, y))
                    .into_iter()
                    .chain(self.map_wrap.neighbors(x, y).filter(|&(sx, sy)| {
                        self.tile(sx, sy).terrain.is_water()
                            && !self.treaty_blocks_move(&traveler, sx, sy)
                            && !self.units.iter().any(|unit| {
                                (unit.x, unit.y) == (sx, sy)
                                    && !self.allied(unit.owner, founder.owner)
                            })
                    }))
            {
                let sea_cost = *checked[self.map_wrap.index(pickup.0, pickup.1)]
                    .get_or_insert_with(|| {
                        self.route_cost_within(ship, pickup.0, pickup.1, i32::MAX)
                    });
                if let Some(sea_cost) = sea_cost {
                    let sea_turns = arrival(sea_cost, sea_initial, ship_speed);
                    // Adjacent embark is a move, unlike loading on the same
                    // port tile. A spent walker must wait for movement refresh.
                    let land_ready =
                        land_turns + i32::from(pickup != (x, y) && arrival_remaining == 0);
                    let turns = land_ready.max(sea_turns);
                    let meeting = &mut rendezvous[self.map_wrap.index(pickup.0, pickup.1)];
                    if meeting.is_none_or(|current| (turns, cost, index) < current) {
                        *meeting = Some((turns, cost, index));
                    }
                }
            }
            for (nx, ny, step) in self.settler_land_steps(&traveler) {
                let next = self.map_wrap.index(nx, ny);
                let next_cost = cost + step.min(step_remaining);
                if next_cost < costs[next] {
                    costs[next] = next_cost;
                    previous[next] = Some((index, self.map_wrap.vector(x, y, nx, ny)));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        rendezvous
            .into_iter()
            .enumerate()
            .filter_map(|(tile, meeting)| {
                let (turns, _, cursor) = meeting?;
                let walk = super::settlement::reconstruct_path(&previous, cursor);
                Some(Rendezvous {
                    tile: self.map_wrap.position(tile),
                    turns,
                    walk,
                })
            })
            .collect()
    }

    pub(super) fn merchant_pickup_route(
        &self,
        passenger: &Unit,
        ship: &Unit,
    ) -> Option<((i32, i32), Vec<(i32, i32)>, i32)> {
        if !self.allied(passenger.owner, ship.owner)
            || !ship.kind.can_carry(passenger.kind)
            || (passenger.transport.is_some() && passenger.transport != Some(ship.id))
            || (passenger.transport.is_none()
                && self.cargo_count(ship.id) >= ship.kind.transport_capacity())
        {
            return None;
        }
        if passenger.transport == Some(ship.id) {
            return Some(((ship.x, ship.y), Vec::new(), 0));
        }
        self.settler_rendezvous_points(passenger, ship)
            .into_iter()
            .min_by_key(|meeting| {
                (
                    meeting.turns,
                    meeting.walk.len(),
                    self.map_distance(ship.x, ship.y, meeting.tile.0, meeting.tile.1),
                    meeting.tile,
                )
            })
            .map(|meeting| (meeting.tile, meeting.walk, meeting.turns))
    }

    fn best_settlement_ferry(
        &self,
        founder: &Unit,
        unavailable: impl Fn(u32) -> bool,
    ) -> Option<(u32, SettlementFerry)> {
        self.best_settlement_ferry_with_capacity(founder, unavailable, 1)
    }

    fn best_settlement_ferry_with_capacity(
        &self,
        founder: &Unit,
        unavailable: impl Fn(u32) -> bool,
        capacity: usize,
    ) -> Option<(u32, SettlementFerry)> {
        let labels = self.continent_labels();
        let origin =
            if founder.transport.is_some() && self.tile(founder.x, founder.y).terrain.is_water() {
                self.cities
                    .iter()
                    .find(|city| Some(city.id) == founder.home_city)
                    .map_or(0, |city| labels[self.map_wrap.index(city.x, city.y)])
            } else {
                labels[self.map_wrap.index(founder.x, founder.y)]
            };
        let mut best: Option<(u32, SettlementFerry)> = None;
        for ship in self.units.iter().filter(|ship| {
            ship.owner == founder.owner
                && self.ferry_can_take_unit(ship, founder)
                && ship.kind.is_naval()
                && ship.kind.can_carry(UnitKind::Settlers)
                && !unavailable(ship.id)
                && founder.transport.map_or_else(
                    || self.cargo_count(ship.id) + capacity <= ship.kind.transport_capacity(),
                    |id| id == ship.id,
                )
        }) {
            let rendezvous = if founder.transport.is_some() {
                vec![Rendezvous {
                    tile: (founder.x, founder.y),
                    turns: 0,
                    walk: Vec::new(),
                }]
            } else {
                self.settler_rendezvous_points(founder, ship)
            };
            if let Some(plan) =
                self.settlement_ferry_route(founder, ship, &labels, origin, &rendezvous)
                && best
                    .as_ref()
                    .is_none_or(|(_, previous)| plan.value > previous.value)
            {
                best = Some((ship.id, plan));
            }
        }
        best
    }

    fn settlement_ferry_route(
        &self,
        founder: &Unit,
        ship: &Unit,
        labels: &[usize],
        origin: usize,
        rendezvous: &[Rendezvous],
    ) -> Option<SettlementFerry> {
        let government = self.ai_goal_government(founder.owner);
        let reservations = self.settlement_citymap(founder.owner, Some(founder.id));
        let assigned = self
            .ai_settlement_targets
            .get(&founder.id)
            .filter(|reservation| reservation.owner == founder.owner)
            .map(|reservation| reservation.destination);
        let sea_speed = self.unit_move_rate(&ship);
        let land_speed = self.unit_move_rate(&founder);
        let turn_cost = sea_speed * land_speed;
        let mut sea_costs = vec![i32::MAX; self.tiles.len()];
        let mut sea_queue = BinaryHeap::new();
        let mut land_costs = vec![i32::MAX; self.tiles.len()];
        let mut beaches = vec![None; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        let mut land_queue = BinaryHeap::new();
        for (meeting, rendezvous) in rendezvous.iter().enumerate() {
            let index = self.map_wrap.index(rendezvous.tile.0, rendezvous.tile.1);
            let cost = rendezvous.turns * turn_cost;
            sea_costs[index] = cost;
            sea_queue.push(Reverse((cost, index, meeting)));
        }
        let mut staged = ship.clone();
        while let Some(Reverse((cost, tile, meeting))) = sea_queue.pop() {
            if cost != sea_costs[tile] {
                continue;
            }
            let (x, y) = self.map_wrap.position(tile);
            (staged.x, staged.y) = (x, y);
            for (nx, ny) in self.map_wrap.neighbors(x, y) {
                let index = self.map_wrap.index(nx, ny);
                let blocked = self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (nx, ny) && !self.allied(unit.owner, founder.owner)
                });
                if labels[index] > 0
                    && labels[index] != origin
                    && self.tile(x, y).terrain.is_water()
                    && !blocked
                    && !self.treaty_blocks_move(founder, nx, ny)
                    && !self.zone_blocks_step(founder, (x, y), (nx, ny))
                    && !self.cities.iter().any(|city| {
                        (city.x, city.y) == (nx, ny) && !self.allied(city.owner, founder.owner)
                    })
                {
                    let cost = cost + turn_cost;
                    if cost < land_costs[index] {
                        land_costs[index] = cost;
                        beaches[index] = Some(((x, y), meeting));
                        land_queue.push(Reverse((cost, index)));
                    }
                }
                if cost + MOVE_FRAGMENTS * land_speed < sea_costs[index]
                    && self.tile(nx, ny).terrain.is_water()
                    && self.movement_cost_for(&staged, nx, ny).is_some()
                    && !self.treaty_blocks_move(&staged, nx, ny)
                    && !blocked
                {
                    let cost = cost + MOVE_FRAGMENTS * land_speed;
                    sea_costs[index] = cost;
                    sea_queue.push(Reverse((cost, index, meeting)));
                }
            }
        }

        let mut traveler = founder.clone();
        traveler.transport = None;
        let mut best = None;
        let mut best_value = 0.0;
        let unit_cost = founder.kind.stats().cost + ship.kind.stats().cost / 3;
        while let Some(Reverse((cost, index))) = land_queue.pop() {
            if cost != land_costs[index] {
                continue;
            }
            let turns = cost / turn_cost;
            let discount = (23.0_f64 / 24.0).powi(3 * turns);
            let (x, y) = self.map_wrap.position(index);
            if assigned.is_none_or(|destination| destination == (x, y))
                && let Some(site) =
                    self.city_site_result(founder.owner, x, y, government, &reservations)
            {
                let value = site.value * discount - f64::from(unit_cost);
                if value > best_value {
                    best = Some(index);
                    best_value = value;
                }
            }
            (traveler.x, traveler.y) = (x, y);
            for (nx, ny, step) in self.settler_land_steps(&traveler) {
                let next = self.map_wrap.index(nx, ny);
                let cost = cost + step * sea_speed;
                if cost < land_costs[next] {
                    land_costs[next] = cost;
                    beaches[next] = None;
                    previous[next] = Some((index, self.map_wrap.vector(x, y, nx, ny)));
                    land_queue.push(Reverse((cost, next)));
                }
            }
        }
        let destination = best?;
        let mut cursor = destination;
        let inland_path = super::settlement::reconstruct_path(&previous, cursor);
        while previous[cursor].is_some() {
            cursor = previous[cursor].unwrap().0;
        }
        let (approach, meeting) = beaches[cursor]?;
        Some(SettlementFerry {
            founder: founder.id,
            pickup: rendezvous[meeting].tile,
            pickup_walk: rendezvous[meeting].walk.clone(),
            approach,
            landing: self.map_wrap.position(cursor),
            destination: self.map_wrap.position(destination),
            inland_path,
            value: best_value,
        })
    }

    pub(super) fn dispatch_settlement_ferries(
        &mut self,
        owner: usize,
        merchant_ferries: &BTreeMap<u32, u32>,
    ) -> BTreeMap<u32, SettlementFerry> {
        let mut dispatch = BTreeMap::new();
        let founders: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == owner && unit.kind == UnitKind::Settlers)
            .cloned()
            .collect();
        for founder in &founders {
            let assigned = self.ai_settlement_targets.contains_key(&founder.id);
            if !assigned && founder.work_left > 0 {
                continue;
            }
            let improvement = if assigned {
                0.0
            } else {
                self.worker_improvement_want(founder)
            };
            if let Some((ship, plan)) = self.best_settlement_ferry(founder, |id| {
                merchant_ferries.contains_key(&id) || dispatch.contains_key(&id)
            }) && (assigned || plan.value as f32 > improvement)
                && (founder.transport.is_some()
                    || self
                        .best_city_site(founder)
                        .is_none_or(|(_, _, land)| plan.value > land))
            {
                if !self.ai_settlement_targets.contains_key(&founder.id) {
                    self.reserve_settlement_site(founder, plan.destination);
                }
                let needs_guard = self.ai_guard_for_destination(
                    founder.id,
                    plan.destination.0,
                    plan.destination.1,
                );
                let actual = if needs_guard && founder.transport.is_none() {
                    self.best_settlement_ferry_with_capacity(
                        founder,
                        |id| merchant_ferries.contains_key(&id) || dispatch.contains_key(&id),
                        2,
                    )
                } else {
                    Some((ship, plan))
                };
                if let Some((ship, plan)) = actual {
                    if self.assign_ai_ferry(ship, founder.id) {
                        dispatch.insert(ship, plan);
                    }
                } else {
                    self.release_ai_ferry(founder.id);
                    self.request_ai_ferry(founder.id);
                }
            }
        }
        dispatch
    }

    pub(crate) fn ai_guard_ferry_capacity_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.ai_ferries = Default::default();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let island = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, island] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, home.0, home.1);
        let founder = self.spawn_unit(1, UnitKind::Settlers, home.0, home.1);
        let small = self.spawn_unit(1, UnitKind::Trireme, home.0, home.1);
        let cargo = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.units
            .iter_mut()
            .find(|u| u.id == cargo)
            .unwrap()
            .transport = Some(small);
        let far = offset_native_by_map(home.0, home.1, -2, 0);
        let roomy = self.spawn_unit(1, UnitKind::Trireme, far.0, far.1);
        let actor = self.units.iter().find(|u| u.id == founder).unwrap().clone();
        self.reserve_settlement_site(&actor, island);
        let speculative = self
            .best_settlement_ferry(&actor, |_| false)
            .is_some_and(|(id, _)| id == small);
        let guard = self.spawn_unit(1, UnitKind::Musketeers, home.0, home.1);
        self.assign_ai_guard(guard, guards::GuardTarget::Unit(founder));
        let actual = self.dispatch_settlement_ferries(1, &BTreeMap::new());
        let reselected = actual
            .get(&roomy)
            .is_some_and(|plan| plan.destination == island)
            && !actual.contains_key(&small)
            && self
                .ai_settlement_targets
                .get(&founder)
                .is_some_and(|target| target.destination == island);
        self.remove_units_with_cargo(vec![roomy]);
        let blocked = self
            .dispatch_settlement_ferries(1, &BTreeMap::new())
            .is_empty()
            && self
                .ai_settlement_targets
                .get(&founder)
                .is_some_and(|target| target.destination == island)
            && self.ferry_boss(small).is_none();
        (speculative, reselected, blocked)
    }

    pub(crate) fn rendezvous_route_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
        }
        let early = (10, 24);
        let later = offset_native_by_map(early.0, early.1, 10, 0);
        let island = offset_native_by_map(later.0, later.1, 1, 0);
        self.tile_mut(island.0, island.1).terrain = Terrain::Grassland;
        let labels = self.continent_labels();
        let founder = self.make_unit(0, 1, UnitKind::Settlers, early.0, early.1);
        let ship = self.make_unit(0, 1, UnitKind::Trireme, early.0, early.1);
        let meetings = [
            Rendezvous {
                tile: early,
                turns: 0,
                walk: Vec::new(),
            },
            Rendezvous {
                tile: later,
                turns: 2,
                walk: vec![(1, 0)],
            },
        ];
        let early_plan = self.settlement_ferry_route(&founder, &ship, &labels, 0, &meetings[..1]);
        let combined = self.settlement_ferry_route(&founder, &ship, &labels, 0, &meetings);
        early_plan.zip(combined).is_some_and(|(early, best)| {
            best.pickup == later
                && best.destination == island
                && best.pickup_walk == meetings[1].walk
                && best.value > early.value
        })
    }

    pub(super) fn run_settlement_ferry(&mut self, ship: u32, plan: &SettlementFerry) {
        let Some(founder) = self
            .units
            .iter()
            .find(|unit| unit.id == plan.founder)
            .cloned()
        else {
            return;
        };
        if founder.transport.is_none() {
            self.follow_path(founder.id, plan.pickup_walk.iter().copied());
            self.unit_goto_tiles.insert(ship, plan.pickup);
            self.ai_goto_unit(ship, plan.pickup, GotoTask::None);
            if !self.board_specific_ship(founder.id, ship)
                && !self.embark_adjacent_ship(founder.id, ship)
            {
                return;
            }
        }
        if !self.ai_guard_prepare_ferry(founder.id, ship) {
            return;
        }
        let (x, y) = plan.approach;
        self.ai_ferry_goto_waypoint(ship, (x, y), plan.destination);
        if self
            .units
            .iter()
            .any(|unit| unit.id == ship && (unit.x, unit.y) == (x, y))
        {
            let (dx, dy) = self.map_wrap.vector(x, y, plan.landing.0, plan.landing.1);
            if self.try_move_unit(founder.id, dx, dy) == MoveResult::Moved {
                self.follow_path(founder.id, plan.inland_path.iter().copied());
                if self.units.iter().any(|unit| {
                    unit.id == founder.id
                        && unit.moves_left > 0
                        && (unit.x, unit.y) == plan.destination
                }) {
                    self.found_city(founder.id);
                    self.ai_settlement_targets.remove(&founder.id);
                }
            }
        }
    }
}

#[cfg(test)]
mod pickup_tests {
    use super::*;

    #[test]
    fn actual_ferry_reselects_two_slots_for_guard_without_changing_city_goal() {
        let mut game = ocean_game();
        assert_eq!(game.ai_guard_ferry_capacity_playtest(), (true, true, true));
    }

    #[test]
    fn founder_can_plan_with_a_partly_loaded_available_ferry_but_not_a_full_one() {
        let mut game = ocean_game();
        let home = (10, 24);
        let island = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, island] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        game.create_city(1, home.0, home.1);
        let founder_id = game.spawn_unit(1, UnitKind::Settlers, home.0, home.1);
        let ship = game.spawn_unit(1, UnitKind::Trireme, home.0, home.1);
        let cargo = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == cargo)
            .unwrap()
            .transport = Some(ship);
        let founder = game
            .units
            .iter()
            .find(|unit| unit.id == founder_id)
            .unwrap()
            .clone();
        assert_eq!(
            game.best_settlement_ferry(&founder, |_| false)
                .map(|(id, _)| id),
            Some(ship)
        );
        let second = game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == second)
            .unwrap()
            .transport = Some(ship);
        assert!(game.best_settlement_ferry(&founder, |_| false).is_none());
    }

    fn ocean_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
        }
        game
    }

    #[test]
    fn merchant_can_board_from_ordinary_coast_without_a_city() {
        let mut game = ocean_game();
        let shore = (10, 24);
        let sea = offset_native_by_map(shore.0, shore.1, 1, 0);
        game.tile_mut(shore.0, shore.1).terrain = Terrain::Plains;
        let passenger = game.spawn_unit(1, UnitKind::Freight, shore.0, shore.1);
        let ship = game.spawn_unit(1, UnitKind::Caravel, sea.0, sea.1);
        let walker = game
            .units
            .iter()
            .find(|unit| unit.id == passenger)
            .unwrap()
            .clone();
        let ferry = game
            .units
            .iter()
            .find(|unit| unit.id == ship)
            .unwrap()
            .clone();
        let (pickup, path, turns) = game.merchant_pickup_route(&walker, &ferry).unwrap();
        assert_eq!(pickup, sea);
        assert!(path.is_empty());
        assert_eq!(turns, 0);
        assert!(game.embark_adjacent_ship(passenger, ship));
        assert_eq!(
            game.units
                .iter()
                .find(|unit| unit.id == passenger)
                .unwrap()
                .transport,
            Some(ship)
        );
    }

    #[test]
    fn rough_ground_cost_is_clipped_to_walking_movement_before_port_pickup() {
        let mut game = ocean_game();
        let start = (10, 24);
        let port = offset_native_by_map(start.0, start.1, 1, 0);
        game.tile_mut(start.0, start.1).terrain = Terrain::Plains;
        game.tile_mut(port.0, port.1).terrain = Terrain::Hills;
        game.create_city(1, port.0, port.1);
        let passenger = game.spawn_unit(1, UnitKind::Freight, start.0, start.1);
        let ship = game.spawn_unit(1, UnitKind::Caravel, port.0, port.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == passenger)
            .unwrap()
            .moves_left = 1;
        game.units
            .iter_mut()
            .find(|unit| unit.id == ship)
            .unwrap()
            .moves_left = 0;
        let walker = game
            .units
            .iter()
            .find(|unit| unit.id == passenger)
            .unwrap()
            .clone();
        let ferry = game
            .units
            .iter()
            .find(|unit| unit.id == ship)
            .unwrap()
            .clone();
        assert!(game.movement_cost_for(&walker, port.0, port.1).unwrap() > 1);
        let (pickup, path, turns) = game.merchant_pickup_route(&walker, &ferry).unwrap();
        assert_eq!(pickup, port);
        assert_eq!(path.len(), 1);
        assert_eq!(turns, 0);
        game.follow_path(passenger, path);
        assert_eq!(
            game.units
                .iter()
                .find(|unit| unit.id == passenger)
                .unwrap()
                .moves_left,
            0
        );
        assert!(game.board_specific_ship(passenger, ship));
    }

    #[test]
    fn same_tile_pickup_works_in_an_allied_port() {
        let mut game = ocean_game();
        let port = (10, 24);
        game.tile_mut(port.0, port.1).terrain = Terrain::Plains;
        game.create_city(HUMAN, port.0, port.1);
        game.sign_pact(1, HUMAN, Pact::Alliance);
        let passenger = game.make_unit(0, 1, UnitKind::Caravan, port.0, port.1);
        let ship = game.make_unit(0, 1, UnitKind::Caravel, port.0, port.1);
        let (pickup, path, turns) = game.merchant_pickup_route(&passenger, &ship).unwrap();
        assert_eq!(pickup, port);
        assert!(path.is_empty());
        assert_eq!(turns, 0);
    }
}
