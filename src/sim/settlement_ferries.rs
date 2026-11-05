use super::{terrain_work::neighbors, *};
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

    pub(super) fn settlement_production(&self, city: &City) -> Option<UnitKind> {
        let waiting = self.units.iter().find(|unit| {
            unit.owner == city.owner
                && unit.kind == UnitKind::Settlers
                && unit.transport.is_none()
                && (unit.x, unit.y) == (city.x, city.y)
        });
        if waiting.is_none()
            && (city.size < 3
                || self
                    .units
                    .iter()
                    .any(|unit| unit.owner == city.owner && unit.kind == UnitKind::Settlers)
                || self.cities.iter().any(|other| {
                    other.owner == city.owner
                        && other.id != city.id
                        && other.production == Production::Unit(UnitKind::Settlers)
                }))
        {
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
            return waiting.is_none().then_some(UnitKind::Settlers);
        }

        if !self.is_port(city) {
            return (waiting.is_none() && land > 0.0).then_some(UnitKind::Settlers);
        }
        let forecast = self.city_forecast(city);
        let surplus = forecast.shields - forecast.support.shields;
        if surplus > 0 && !forecast.disorder {
            let labels = self.continent_labels();
            let origin = labels[Self::idx(city.x, city.y)];
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
                return Some(kind);
            }
        }
        (waiting.is_none() && land > 0.0).then_some(UnitKind::Settlers)
    }

    fn settler_rendezvous_points(&self, founder: &Unit, ship: &Unit) -> Vec<Rendezvous> {
        let start = Self::idx(founder.x, founder.y);
        let ship_speed = self.unit_move_allowance(ship.owner, ship.kind);
        let land_speed = self.unit_move_allowance(founder.owner, founder.kind);
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
            let land_turns = (cost + land_speed - 1) / land_speed;
            let (x, y) = Self::xy(index);
            (traveler.x, traveler.y) = (x, y);
            let port = self.cities.iter().any(|city| {
                city.owner == founder.owner && (city.x, city.y) == (x, y) && self.is_port(city)
            });
            for pickup in port
                .then_some((x, y))
                .into_iter()
                .chain(neighbors(x, y).filter(|&(sx, sy)| {
                    self.tile(sx, sy).terrain.is_water()
                        && !self.treaty_blocks_move(&traveler, sx, sy)
                        && !self.units.iter().any(|unit| {
                            (unit.x, unit.y) == (sx, sy) && !self.allied(unit.owner, founder.owner)
                        })
                }))
            {
                let steps = *checked[Self::idx(pickup.0, pickup.1)].get_or_insert_with(|| {
                    if (ship.x, ship.y) == pickup {
                        return Some(0);
                    }
                    let path = self.path_for_unit(ship, pickup.0, pickup.1);
                    (!path.is_empty()).then_some(path.len() as i32)
                });
                if let Some(steps) = steps {
                    let sea_cost = steps * MOVE_FRAGMENTS;
                    let turns = land_turns.max((sea_cost + ship_speed - 1) / ship_speed);
                    let meeting = &mut rendezvous[Self::idx(pickup.0, pickup.1)];
                    if meeting.is_none_or(|current| (turns, cost, index) < current) {
                        *meeting = Some((turns, cost, index));
                    }
                }
            }
            for (nx, ny, step) in self.settler_land_steps(&traveler) {
                let next = Self::idx(nx, ny);
                if cost + step < costs[next] {
                    costs[next] = cost + step;
                    previous[next] = Some((index, wrapped_map_vector(x, y, nx, ny)));
                    queue.push(Reverse((cost + step, next)));
                }
            }
        }
        rendezvous
            .into_iter()
            .enumerate()
            .filter_map(|(tile, meeting)| {
                let (turns, _, mut cursor) = meeting?;
                let mut walk = Vec::new();
                while let Some((prior, direction)) = previous[cursor] {
                    walk.push(direction);
                    cursor = prior;
                }
                walk.reverse();
                Some(Rendezvous {
                    tile: Self::xy(tile),
                    turns,
                    walk,
                })
            })
            .collect()
    }

    fn best_settlement_ferry(
        &self,
        founder: &Unit,
        unavailable: impl Fn(u32) -> bool,
    ) -> Option<(u32, SettlementFerry)> {
        let labels = self.continent_labels();
        let origin =
            if founder.transport.is_some() && self.tile(founder.x, founder.y).terrain.is_water() {
                self.cities
                    .iter()
                    .find(|city| Some(city.id) == founder.home_city)
                    .map_or(0, |city| labels[Self::idx(city.x, city.y)])
            } else {
                labels[Self::idx(founder.x, founder.y)]
            };
        let mut best: Option<(u32, SettlementFerry)> = None;
        for ship in self.units.iter().filter(|ship| {
            ship.owner == founder.owner
                && ship.kind.is_naval()
                && ship.kind.can_carry(UnitKind::Settlers)
                && !unavailable(ship.id)
                && founder
                    .transport
                    .map_or_else(|| self.cargo_count(ship.id) == 0, |id| id == ship.id)
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
        let sea_speed = self.unit_move_allowance(ship.owner, ship.kind);
        let land_speed = self.unit_move_allowance(founder.owner, founder.kind);
        let turn_cost = sea_speed * land_speed;
        let mut sea_costs = vec![i32::MAX; self.tiles.len()];
        let mut sea_queue = BinaryHeap::new();
        let mut land_costs = vec![i32::MAX; self.tiles.len()];
        let mut beaches = vec![None; self.tiles.len()];
        let mut previous = vec![None; self.tiles.len()];
        let mut land_queue = BinaryHeap::new();
        for (meeting, rendezvous) in rendezvous.iter().enumerate() {
            let index = Self::idx(rendezvous.tile.0, rendezvous.tile.1);
            let cost = rendezvous.turns * turn_cost;
            sea_costs[index] = cost;
            sea_queue.push(Reverse((cost, index, meeting)));
        }
        let mut staged = ship.clone();
        while let Some(Reverse((cost, tile, meeting))) = sea_queue.pop() {
            if cost != sea_costs[tile] {
                continue;
            }
            let (x, y) = Self::xy(tile);
            (staged.x, staged.y) = (x, y);
            for (nx, ny) in neighbors(x, y) {
                let index = Self::idx(nx, ny);
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
            let (x, y) = Self::xy(index);
            if let Some(yields) = self.city_site_value(founder.owner, x, y) {
                let value = f64::from(yields) * discount - f64::from(unit_cost);
                if value > best_value {
                    best = Some(index);
                    best_value = value;
                }
            }
            (traveler.x, traveler.y) = (x, y);
            for (nx, ny, step) in self.settler_land_steps(&traveler) {
                let next = Self::idx(nx, ny);
                let cost = cost + step * sea_speed;
                if cost < land_costs[next] {
                    land_costs[next] = cost;
                    beaches[next] = None;
                    previous[next] = Some((index, wrapped_map_vector(x, y, nx, ny)));
                    land_queue.push(Reverse((cost, next)));
                }
            }
        }
        let destination = best?;
        let mut cursor = destination;
        let mut inland_path = Vec::new();
        while let Some((prior, direction)) = previous[cursor] {
            inland_path.push(direction);
            cursor = prior;
        }
        inland_path.reverse();
        let (approach, meeting) = beaches[cursor]?;
        Some(SettlementFerry {
            founder: founder.id,
            pickup: rendezvous[meeting].tile,
            pickup_walk: rendezvous[meeting].walk.clone(),
            approach,
            landing: Self::xy(cursor),
            destination: Self::xy(destination),
            inland_path,
            value: best_value,
        })
    }

    pub(super) fn dispatch_settlement_ferries(
        &self,
        owner: usize,
        merchant_ferries: &BTreeMap<u32, u32>,
    ) -> BTreeMap<u32, SettlementFerry> {
        let mut dispatch = BTreeMap::new();
        for founder in self
            .units
            .iter()
            .filter(|unit| unit.owner == owner && unit.kind == UnitKind::Settlers)
        {
            if let Some((ship, plan)) = self.best_settlement_ferry(founder, |id| {
                merchant_ferries.contains_key(&id) || dispatch.contains_key(&id)
            }) && (founder.transport.is_some()
                || self
                    .best_city_site(founder)
                    .is_none_or(|(_, _, land)| plan.value > land))
            {
                dispatch.insert(ship, plan);
            }
        }
        dispatch
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
            self.goto_unit(ship, plan.pickup.0, plan.pickup.1);
            if !self.board_specific_ship(founder.id, ship)
                && !self.embark_adjacent_ship(founder.id, ship)
            {
                return;
            }
        }
        let (x, y) = plan.approach;
        self.goto_unit(ship, x, y);
        if self
            .units
            .iter()
            .any(|unit| unit.id == ship && (unit.x, unit.y) == (x, y))
        {
            let (dx, dy) = wrapped_map_vector(x, y, plan.landing.0, plan.landing.1);
            if self.try_move_unit(founder.id, dx, dy) == MoveResult::Moved {
                self.follow_path(founder.id, plan.inland_path.iter().copied());
                if self
                    .units
                    .iter()
                    .any(|unit| unit.id == founder.id && (unit.x, unit.y) == plan.destination)
                {
                    self.found_city(founder.id);
                }
            }
        }
    }
}
