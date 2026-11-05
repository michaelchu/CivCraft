use super::normal_paths::GotoTask;
use super::{terrain_work::neighbors, *};
use crate::rules::Genus;
use std::collections::BTreeMap;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(super) struct MerchantMission {
    pub(super) city: u32,
    pub(super) wonder: bool,
    pub(super) owner: usize,
}

#[derive(Clone, Copy)]
enum MerchantPolicy {
    National,
    Allied,
    NonWar,
}

struct MerchantPlan {
    value: f64,
    city: u32,
    wonder: bool,
    approach: (i32, i32),
    turns: i32,
}

struct FerryDelivery {
    value: f64,
    ship: u32,
    city: u32,
    approach: (i32, i32),
    landing: Option<(i32, i32)>,
    wonder: bool,
    pickup: (i32, i32),
    pickup_walk: Vec<(i32, i32)>,
}

fn worthwhile_marketplace(windfall: f64, cost: i32, surplus: i32, turns: i32) -> bool {
    windfall * 0.95_f64.powi(turns) >= f64::from(cost) * 0.95_f64.powi(-cost / surplus.max(2))
}

impl Simulation {
    fn merchant_can_act(&self, unit: &Unit, city: &City) -> bool {
        matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
            && unit.moves_left > 0
            && (unit.y - city.y).abs() <= 2
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
    }

    fn can_trade(&self, from: &City, to: &City) -> bool {
        from.owner != to.owner && !self.at_war(from.owner, to.owner)
    }

    fn home_for_merchant(&self, unit: &Unit) -> Option<&City> {
        self.cities
            .iter()
            .find(|city| Some(city.id) == unit.home_city && city.owner == unit.owner)
    }

    pub(crate) fn can_deliver_goods(&self, unit: &Unit, city: &City) -> bool {
        self.merchant_can_act(unit, city)
            && self
                .home_for_merchant(unit)
                .is_some_and(|home| self.can_trade(home, city))
    }

    pub(crate) fn can_help_wonder(&self, unit: &Unit, city: &City) -> bool {
        self.merchant_can_act(unit, city)
            && self.allied(unit.owner, city.owner)
            && matches!(city.production, Production::Building(building)
                if building.data().genus == Genus::GreatWonder && city.shields < building.data().cost)
    }

    pub(super) fn help_wonder(&mut self, unit: u32, city: u32) -> bool {
        let Some(merchant) = self.units.iter().find(|item| item.id == unit) else {
            return false;
        };
        let Some(index) = self
            .cities
            .iter()
            .position(|item| item.id == city && self.can_help_wonder(merchant, item))
        else {
            return false;
        };
        let contribution = merchant.kind.stats().cost;
        let city = &mut self.cities[index];
        city.shields += contribution;
        city.caravan_shields += contribution;
        if let Some((_, original_shields)) = &mut city.production_before_switch {
            *original_shields += contribution;
        }
        let name = city.name.clone();
        self.remove_units_with_cargo(vec![unit]);
        self.refresh_visibility();
        self.add_event(format!(
            "A merchant contributed {contribution} shields to the wonder in {name}."
        ));
        true
    }

    fn weighted_trade_distance(&self, a: &City, b: &City) -> i32 {
        self.map_distance(a.x, a.y, b.x, b.y) * 40 / MAP_W.max(MAP_H)
    }

    fn same_continent(&self, a: &City, b: &City) -> bool {
        let mut seen = vec![false; self.tiles.len()];
        let mut queue = vec![(a.x, a.y)];
        seen[Self::idx(a.x, a.y)] = true;
        while let Some((x, y)) = queue.pop() {
            if (x, y) == (b.x, b.y) {
                return true;
            }
            for (nx, ny) in neighbors(x, y) {
                let index = Self::idx(nx, ny);
                if !seen[index] && !self.tiles[index].terrain.is_water() {
                    seen[index] = true;
                    queue.push((nx, ny));
                }
            }
        }
        false
    }

    fn route_value(&self, a: &City, b: &City) -> i32 {
        let value = self.weighted_trade_distance(a, b) + a.size + b.size;
        value * if self.same_continent(a, b) { 1 } else { 2 } / 12
    }

    fn route_partners(&self, city: u32) -> impl Iterator<Item = &City> {
        self.trade_routes.iter().filter_map(move |&(a, b)| {
            let partner = if a == city {
                b
            } else if b == city {
                a
            } else {
                return None;
            };
            self.cities.iter().find(|other| other.id == partner)
        })
    }

    pub(super) fn route_trade(&self, city: &City) -> i32 {
        self.route_partners(city.id)
            .filter(|other| self.can_trade(city, other))
            .map(|other| self.route_value(city, other))
            .sum()
    }

    pub(super) fn trade_illness(&self, city: &City) -> i32 {
        self.route_partners(city.id)
            .filter(|other| self.can_trade(city, other))
            .filter(|other| other.last_plague.is_some_and(|turn| self.turn - turn < 5))
            .map(|other| 0.5 * f64::from(city.size * other.size).sqrt())
            .sum::<f64>() as i32
    }

    fn route_to_replace(&self, city: &City, value: i32) -> Result<Option<u32>, ()> {
        let partners: Vec<_> = self.route_partners(city.id).collect();
        if partners.len() < 2 {
            return Ok(None);
        }
        let weakest = partners
            .iter()
            .map(|other| (self.route_value(city, other), other.id))
            .min();
        match weakest {
            Some((old, id)) if old < value => Ok(Some(id)),
            _ => Err(()),
        }
    }

    fn remove_route(&mut self, a: u32, b: u32) {
        self.trade_routes
            .retain(|&pair| pair != (a, b) && pair != (b, a));
    }

    pub(super) fn prune_trade_routes(&mut self) {
        let mut routes = std::mem::take(&mut self.trade_routes);
        routes.retain(|&(a, b)| {
            let from = self.cities.iter().find(|city| city.id == a);
            let to = self.cities.iter().find(|city| city.id == b);
            from.zip(to)
                .is_some_and(|(from, to)| self.can_trade(from, to))
        });
        self.trade_routes = routes;
    }

    fn route_replacements(&self, from: &City, to: &City) -> Option<(Option<u32>, Option<u32>)> {
        if self
            .route_partners(from.id)
            .any(|partner| partner.id == to.id)
        {
            return None;
        }
        let value = self.route_value(from, to);
        self.route_to_replace(from, value)
            .ok()
            .zip(self.route_to_replace(to, value).ok())
    }

    fn delivery_gold(&self, from: &City, to: &City) -> i32 {
        let base = (self.weighted_trade_distance(from, to) + 10)
            * (self.city_yields(from).2 + self.city_yields(to).2)
            / 24;
        let bonus = if self.has_tech(from.owner, Tech::RAILROAD) {
            -415.0
        } else {
            585.0
        };
        (f64::from(base) * 2.0_f64.powf(bonus / 1000.0)).ceil() as i32
    }

    fn merchant_trade_value(
        &self,
        merchant: &Unit,
        home: &City,
        destination: &City,
        turns: i32,
    ) -> f64 {
        let replacements = self.route_replacements(home, destination);
        let recurring = replacements.map_or(0, |(old_home, old_destination)| {
            let loss = |city: &City, old: Option<u32>| {
                old.and_then(|id| self.cities.iter().find(|other| other.id == id))
                    .map_or(0, |other| self.route_value(city, other))
            };
            let mut gain = self.route_value(home, destination) - loss(home, old_home);
            // caravan.c:one_city_trade_benefit also accounts for the back
            // route lost by one of our other cities at the foreign endpoint.
            if let Some(old) =
                old_destination.and_then(|id| self.cities.iter().find(|c| c.id == id))
            {
                if old.owner == merchant.owner {
                    gain -= self.route_value(destination, old);
                }
            }
            gain
        });
        let windfall = f64::from(self.delivery_gold(home, destination));
        if replacements.is_none() {
            let cost = merchant.kind.stats().cost;
            let surplus = self.city_forecast(home).shield_surplus().max(2);
            // A marketplace-only delivery must repay the alternative of
            // producing gold (caravan.c:get_discounted_reward).
            if !worthwhile_marketplace(windfall, cost, surplus, turns) {
                return 0.0;
            }
        }
        windfall + f64::from(recurring) / (1.0 - 0.95)
    }

    fn merchant_wonder_value(&self, merchant: &Unit, city: &City, turns: i32) -> f64 {
        // caravan.c:wonder_benefit compares rush costs at the arrival stock,
        // including the double price when construction has not started.
        let stock = city.shields + turns * self.city_forecast(city).shield_surplus();
        let rush = |stock: i32| {
            4 * (city.production.cost() - stock).max(0) * if stock == 0 { 2 } else { 1 }
        };
        f64::from(2 * (rush(stock) - rush(stock + merchant.kind.stats().cost)))
    }

    fn inland_ferry_approach(
        &self,
        merchant: &Unit,
        ship: &Unit,
        city: &City,
        allowance: i32,
        remaining: i32,
        labels: &[usize],
    ) -> Option<(i32, i32, i32, (i32, i32))> {
        let destination = labels[Self::idx(city.x, city.y)];
        let mut best = None;
        for (index, &label) in labels.iter().enumerate() {
            if label != destination || label == 0 {
                continue;
            }
            let (lx, ly) = Self::xy(index);
            if self.treaty_blocks_move(merchant, lx, ly)
                || self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (lx, ly) && !self.allied(merchant.owner, unit.owner)
                })
            {
                continue;
            }
            let sea_approach = neighbors(lx, ly)
                .filter(|&(x, y)| {
                    self.tile(x, y).terrain.is_water()
                        && !self.units.iter().any(|unit| {
                            (unit.x, unit.y) == (x, y) && !self.allied(ship.owner, unit.owner)
                        })
                })
                .filter_map(|(x, y)| {
                    let cost = self.route_cost_within(ship, x, y, i32::MAX)?;
                    let sea_turns = ((cost - remaining).max(0) + allowance - 1) / allowance;
                    Some((sea_turns, x, y))
                })
                .min();
            let Some((sea_turns, x, y)) = sea_approach else {
                continue;
            };

            let mut walker = merchant.clone();
            (walker.x, walker.y) = (x, y);
            walker.transport = None;
            let foot_allowance = self.unit_move_rate(&walker);
            let mut landing_turn = sea_turns;
            if landing_turn > 0 {
                walker.moves_left = foot_allowance;
            } else if walker.moves_left <= 0 {
                landing_turn += 1;
                walker.moves_left = foot_allowance;
            }
            let Some(landing_cost) = self.movement_cost_for(&walker, lx, ly) else {
                continue;
            };
            walker.moves_left = (walker.moves_left - landing_cost).max(0);
            (walker.x, walker.y) = (lx, ly);
            let Some((foot_cost, _, _)) = self.merchant_land_approach(&walker, city) else {
                continue;
            };
            // Delivery requires a movement fragment after the last walking step.
            let walking_turns = if foot_cost < walker.moves_left {
                0
            } else {
                1 + (foot_cost - walker.moves_left) / foot_allowance
            };
            let candidate = (landing_turn + walking_turns, x, y, (lx, ly));
            if best.is_none_or(|previous| candidate < previous) {
                best = Some(candidate);
            }
        }
        best
    }

    fn ferry_delivery(
        &self,
        merchant: &Unit,
        ready_in: i32,
        reserved: &BTreeMap<u32, u32>,
    ) -> Option<FerryDelivery> {
        self.ferry_delivery_for(merchant, ready_in, reserved, None)
    }

    fn ferry_delivery_for(
        &self,
        merchant: &Unit,
        ready_in: i32,
        reserved: &BTreeMap<u32, u32>,
        only_ship: Option<u32>,
    ) -> Option<FerryDelivery> {
        self.ferry_delivery_for_capacity(merchant, ready_in, reserved, only_ship, 1)
    }

    fn ferry_delivery_for_capacity(
        &self,
        merchant: &Unit,
        ready_in: i32,
        reserved: &BTreeMap<u32, u32>,
        only_ship: Option<u32>,
        capacity: usize,
    ) -> Option<FerryDelivery> {
        let home = self.home_for_merchant(merchant)?;
        let mut best: Option<FerryDelivery> = None;
        let mut labels = None;
        for ship in self.units.iter().filter(|ship| {
            ship.owner == merchant.owner
                && only_ship.is_none_or(|id| ship.id == id)
                && ship.kind.is_naval()
                && self.ferry_can_take_unit(ship, merchant)
                && ship.kind.can_carry(merchant.kind)
                && !reserved.contains_key(&ship.id)
                && if let Some(id) = merchant.transport {
                    id == ship.id
                } else {
                    self.cargo_count(ship.id) + capacity <= ship.kind.transport_capacity()
                }
        }) {
            let mut staged = ship.clone();
            let allowance = self.unit_move_rate(&ship).max(MOVE_FRAGMENTS);
            let mut remaining = if ready_in > 0 {
                allowance
            } else {
                ship.moves_left
            };
            staged.moves_left = remaining;
            let Some((pickup, pickup_walk, meeting)) =
                self.merchant_pickup_route(merchant, &staged)
            else {
                continue;
            };
            let Some(cost) = self.route_cost_within(&staged, pickup.0, pickup.1, i32::MAX) else {
                continue;
            };
            let sea_turns = ((cost - remaining).max(0) + allowance - 1) / allowance;
            let pickup_turns = meeting.max(sea_turns);
            remaining = if pickup_turns > sea_turns {
                allowance
            } else {
                (remaining + pickup_turns * allowance - cost).max(0)
            };
            (staged.x, staged.y) = pickup;
            staged.moves_left = remaining;
            let mut passenger = merchant.clone();
            let foot_allowance = self.unit_move_rate(&passenger).max(1);
            let mut endpoint = (passenger.x, passenger.y);
            for &(dx, dy) in &pickup_walk {
                endpoint = offset_native_by_map(endpoint.0, endpoint.1, dx, dy);
            }
            let walk_cost = self
                .route_cost_within(&passenger, endpoint.0, endpoint.1, i32::MAX)
                .unwrap_or(0);
            let walking_turns = ((walk_cost - passenger.moves_left.max(0)).max(0) + foot_allowance
                - 1)
                / foot_allowance;
            passenger.moves_left = if ready_in > 0 || pickup_turns > walking_turns {
                foot_allowance
            } else {
                (passenger.moves_left + pickup_turns * foot_allowance - walk_cost).max(0)
            };
            if passenger.transport.is_none() && pickup != endpoint {
                passenger.moves_left = (passenger.moves_left - MOVE_FRAGMENTS).max(0);
            }
            (passenger.x, passenger.y) = pickup;
            for city in self.cities.iter().filter(|city| {
                self.ai_merchant_missions
                    .get(&merchant.id)
                    .is_some_and(|mission| mission.city == city.id)
            }) {
                let approach = neighbors(city.x, city.y)
                    .filter(|&(x, y)| {
                        self.tile(x, y).terrain.is_water()
                            && !self.units.iter().any(|unit| {
                                (unit.x, unit.y) == (x, y) && !self.allied(ship.owner, unit.owner)
                            })
                    })
                    .filter_map(|(x, y)| {
                        self.route_cost_within(&staged, x, y, i32::MAX)
                            .map(|cost| (cost, x, y))
                    })
                    .min();
                let (journey, x, y, landing) = if let Some((cost, x, y)) = approach {
                    let sailing_turns = ((cost - remaining).max(0) + allowance - 1) / allowance;
                    let delivery_turn = sailing_turns.max(i32::from(passenger.moves_left <= 0));
                    (delivery_turn, x, y, None)
                } else {
                    let labels = labels.get_or_insert_with(|| self.continent_labels());
                    let Some((turns, x, y, landing)) = self.inland_ferry_approach(
                        &passenger, &staged, city, allowance, remaining, labels,
                    ) else {
                        continue;
                    };
                    (turns, x, y, Some(landing))
                };
                let turns = ready_in + pickup_turns + journey;
                let wonder = self
                    .ai_merchant_missions
                    .get(&merchant.id)
                    .is_some_and(|mission| mission.wonder);
                let value = if wonder {
                    self.merchant_wonder_value(merchant, city, turns)
                } else {
                    self.merchant_trade_value(merchant, home, city, turns)
                } * 0.95_f64.powi(turns);
                if best.as_ref().is_none_or(|plan| value > plan.value) {
                    best = Some(FerryDelivery {
                        value,
                        ship: ship.id,
                        city: city.id,
                        landing,
                        wonder,
                        pickup,
                        pickup_walk: pickup_walk.clone(),
                        approach: (x, y),
                    });
                }
            }
        }
        best
    }

    pub(super) fn load_ai_merchant_ferries(&mut self, owner: usize) -> BTreeMap<u32, u32> {
        self.merchant_mission_attempts.retain(|id| {
            self.units
                .iter()
                .any(|unit| unit.id == *id && unit.owner != owner)
        });
        let merchants: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
            })
            .cloned()
            .collect();
        for merchant in merchants {
            self.prepare_merchant_mission(&merchant);
        }

        let waiting: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && unit.transport.is_none()
                    && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
                    && self.ai_merchant_missions.contains_key(&unit.id)
            })
            .cloned()
            .collect();
        let mut dispatch = BTreeMap::new();
        for merchant in waiting {
            if self.merchant_plan(&merchant, 0).is_none()
                && let Some(ferry) = self.ferry_delivery(&merchant, 0, &dispatch)
            {
                let needs_guard = self
                    .cities
                    .iter()
                    .find(|c| c.id == ferry.city)
                    .map(|c| (c.x, c.y))
                    .is_some_and(|(x, y)| self.ai_guard_for_destination(merchant.id, x, y));
                let actual = if needs_guard {
                    self.ferry_delivery_for_capacity(&merchant, 0, &dispatch, None, 2)
                } else {
                    Some(ferry)
                };
                if let Some(ferry) = actual {
                    if self.assign_ai_ferry(ferry.ship, merchant.id) {
                        dispatch.insert(ferry.ship, merchant.id);
                    }
                } else {
                    self.release_ai_ferry(merchant.id);
                    self.request_ai_ferry(merchant.id);
                }
            }
        }
        dispatch
    }

    pub(super) fn run_merchant_ferry(&mut self, ship: u32, pickup: Option<u32>) -> bool {
        if let Some(passenger) = pickup {
            let Some(unit) = self.units.iter().find(|unit| unit.id == passenger).cloned() else {
                return false;
            };
            let Some(plan) = self.ferry_delivery_for(&unit, 0, &BTreeMap::new(), Some(ship)) else {
                return true;
            };
            self.follow_path(passenger, plan.pickup_walk);
            self.unit_goto_tiles.insert(ship, plan.pickup);
            self.ai_goto_unit(ship, plan.pickup, GotoTask::None);
            if !self.board_specific_ship(passenger, ship)
                && !self.embark_adjacent_ship(passenger, ship)
            {
                return true;
            }
        }
        let Some(merchant) = self
            .cargo_units_ordered(ship)
            .find(|unit| {
                self.ferry_boss(ship).is_none_or(|id| id == unit.id)
                    && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
            })
            .cloned()
        else {
            return false;
        };
        if !self.ai_guard_prepare_ferry(merchant.id, ship) {
            return true;
        }
        if let Some(ferry) = self.ferry_delivery(&merchant, 0, &BTreeMap::new()) {
            let (x, y) = ferry.approach;
            if (merchant.x, merchant.y) != (x, y) {
                if let Some(city) = self.cities.iter().find(|city| city.id == ferry.city) {
                    self.ai_ferry_goto_waypoint(ship, (x, y), (city.x, city.y));
                }
            }
            if let Some((lx, ly)) = ferry.landing {
                let arrived = self
                    .units
                    .iter()
                    .any(|unit| unit.id == ship && (unit.x, unit.y) == (x, y));
                if arrived {
                    let (dx, dy) = wrapped_map_vector(x, y, lx, ly);
                    if self.try_move_unit(merchant.id, dx, dy) == MoveResult::Moved
                        && let Some(walker) = self
                            .units
                            .iter()
                            .find(|unit| unit.id == merchant.id)
                            .cloned()
                        && let Some(city) = self.cities.iter().find(|city| city.id == ferry.city)
                        && let Some((cost, ax, ay)) = self.merchant_land_approach(&walker, city)
                    {
                        if cost > 0 {
                            self.ai_goto_unit(
                                walker.id,
                                (ax, ay),
                                if ferry.wonder {
                                    GotoTask::Wonder
                                } else {
                                    GotoTask::Trade
                                },
                            );
                        }
                        if ferry.wonder {
                            self.help_wonder(walker.id, ferry.city);
                        } else {
                            self.deliver_goods(walker.id, ferry.city);
                        }
                    }
                }
            } else {
                if ferry.wonder {
                    self.help_wonder(merchant.id, ferry.city);
                } else {
                    self.deliver_goods(merchant.id, ferry.city);
                }
            }
        } else if !self.tile(merchant.x, merchant.y).terrain.is_water() {
            self.board_or_deboard(merchant.id);
        }
        true
    }

    fn merchant_land_approach(&self, merchant: &Unit, city: &City) -> Option<(i32, i32, i32)> {
        let current = (merchant.x, merchant.y);
        if self.map_distance(current.0, current.1, city.x, city.y) <= 1
            && (current.1 - city.y).abs() <= 2
        {
            Some((0, current.0, current.1))
        } else {
            neighbors(city.x, city.y)
                .chain([(city.x, city.y)])
                .filter(|&(x, y)| !self.tile(x, y).terrain.is_water())
                .filter_map(|(x, y)| {
                    if self.best_defender_id(merchant, x, y).is_some() {
                        return None;
                    }
                    self.route_cost_within(merchant, x, y, i32::MAX)
                        .map(|cost| (cost, x, y))
                })
                .min()
        }
    }

    fn merchant_needs_boat(&self, merchant: &Unit, city: &City) -> bool {
        self.merchant_land_approach(merchant, city).is_none()
    }

    fn merchant_tired_of_waiting(&self, merchant: &Unit, city: &City) -> bool {
        let Some(home) = self.home_for_merchant(merchant) else {
            return false;
        };
        if merchant.transport.is_some()
            || !self.same_continent(
                home,
                &City {
                    x: merchant.x,
                    y: merchant.y,
                    ..home.clone()
                },
            )
            || !self.merchant_needs_boat(merchant, city)
        {
            return false;
        }
        let born = self
            .merchant_birth_turns
            .get(&merchant.id)
            .copied()
            .unwrap_or(self.turn);
        let age = self.turn.saturating_sub(born);
        let available = self.ferry_available_count(merchant.owner) > 0;
        age > 20 || (age > 15 && !available)
    }

    fn prepare_merchant_mission(&mut self, merchant: &Unit) {
        // dai_manage_unit returns before task management for exhausted units.
        if merchant.moves_left <= 0 {
            return;
        }
        if !self.merchant_mission_attempts.insert(merchant.id) {
            return;
        }
        self.merchant_birth_turns
            .entry(merchant.id)
            .or_insert(self.turn);
        let snapshot;
        let merchant = if self.home_for_merchant(merchant).is_none() {
            let labels = self.continent_labels();
            let continent = labels[Self::idx(merchant.x, merchant.y)];
            let Some(city) = self
                .cities
                .iter()
                .filter(|city| {
                    city.owner == merchant.owner && labels[Self::idx(city.x, city.y)] == continent
                })
                .min_by_key(|city| self.map_distance(merchant.x, merchant.y, city.x, city.y))
            else {
                return;
            };
            let position = (city.x, city.y);
            self.ai_goto_unit(merchant.id, position, GotoTask::None);
            let Some(index) = self
                .units
                .iter()
                .position(|unit| unit.id == merchant.id && (unit.x, unit.y) == position)
            else {
                return;
            };
            if !self.change_home_city(index) {
                return;
            }
            snapshot = self.units[index].clone();
            &snapshot
        } else {
            merchant
        };
        let mut national_only = false;
        if let Some(mission) = self.ai_merchant_missions.get(&merchant.id).copied() {
            if let Some(city) = self.cities.iter().find(|city| city.id == mission.city) {
                let valid = mission.owner == merchant.owner
                    && self.allied(merchant.owner, city.owner)
                    && if mission.wonder {
                        matches!(city.production, Production::Building(building) if building.data().genus == Genus::GreatWonder)
                            && city.shields < city.production.cost()
                    } else {
                        self.home_for_merchant(merchant).is_some_and(|home| {
                            self.can_trade(home, city)
                                && self.route_replacements(home, city).is_some()
                        })
                    };
                if valid {
                    if !self.merchant_tired_of_waiting(merchant, city) {
                        if merchant.transport.is_some() || self.merchant_needs_boat(merchant, city)
                        {
                            self.request_ai_ferry(merchant.id);
                        } else {
                            self.release_ai_ferry(merchant.id);
                        }
                        return;
                    }
                    national_only = true;
                }
            }
            self.ai_merchant_missions.remove(&merchant.id);
            self.unit_goto_tiles.remove(&merchant.id);
            self.release_ai_ferry(merchant.id);
        }
        // daiunit.c draws this even before a tired passenger forces national-only.
        let random_policy = if self.rand_below(2) != 0 {
            MerchantPolicy::NonWar
        } else {
            MerchantPolicy::Allied
        };
        let policy = if national_only {
            MerchantPolicy::National
        } else {
            random_policy
        };
        let ignore_transit = !national_only
            && self.home_for_merchant(merchant).is_some_and(|home| {
                self.cities.iter().any(|city| {
                    self.allied(merchant.owner, city.owner)
                        && self.can_trade(home, city)
                        && !self.same_continent(home, city)
                })
            });
        if let Some(plan) = self.merchant_plan_for(merchant, 0, policy, ignore_transit, None) {
            if let Some(city) = self.cities.iter().find(|city| city.id == plan.city)
                && self.merchant_needs_boat(merchant, city)
            {
                self.request_ai_ferry(merchant.id);
            }

            if let Some(city) = self.cities.iter().find(|c| c.id == plan.city) {
                self.unit_goto_tiles.insert(merchant.id, (city.x, city.y));
            }
            self.ai_merchant_missions.insert(
                merchant.id,
                MerchantMission {
                    city: plan.city,
                    wonder: plan.wonder,
                    owner: merchant.owner,
                },
            );
        }
    }

    fn merchant_plan(&self, merchant: &Unit, ready_in: i32) -> Option<MerchantPlan> {
        let mission = self.ai_merchant_missions.get(&merchant.id).copied()?;
        self.merchant_plan_for(
            merchant,
            ready_in,
            MerchantPolicy::NonWar,
            false,
            Some(mission),
        )
    }

    fn merchant_plan_for(
        &self,
        merchant: &Unit,
        ready_in: i32,
        policy: MerchantPolicy,
        ignore_transit: bool,
        mission: Option<MerchantMission>,
    ) -> Option<MerchantPlan> {
        let home = self.home_for_merchant(merchant);
        let mut best: Option<MerchantPlan> = None;
        let mut cities: Vec<_> = self.cities.iter().collect();
        cities.sort_by_key(|city| (city.owner, city.id));
        for city in cities {
            if mission.is_some_and(|mission| mission.city != city.id)
                || !match policy {
                    MerchantPolicy::National => city.owner == merchant.owner,
                    MerchantPolicy::Allied => self.allied(merchant.owner, city.owner),
                    MerchantPolicy::NonWar => !self.at_war(merchant.owner, city.owner),
                }
            {
                continue;
            }
            let trade = home.filter(|home| {
                self.can_trade(home, city) && !mission.is_some_and(|mission| mission.wonder)
            });
            let wonder = (city.owner == merchant.owner
                || mission.is_some_and(|mission| mission.wonder)
                    && self.allied(merchant.owner, city.owner))
                && (ready_in == 0 || Some(city.id) != merchant.home_city)
                && matches!(city.production,
                Production::Building(building) if building.data().genus == Genus::GreatWonder
                    && city.shields < building.data().cost);
            if trade.is_none() && !wonder {
                continue;
            }
            let approach = if ignore_transit {
                Some((0, city.x, city.y))
            } else {
                self.merchant_land_approach(merchant, city)
            };
            let Some((cost, x, y)) = approach else {
                continue;
            };
            let remaining = merchant.moves_left.max(0);
            let turns = if ignore_transit {
                0
            } else {
                ready_in
                    + if cost < remaining {
                        0
                    } else {
                        1 + (cost - remaining) / self.unit_move_rate(&merchant).max(1)
                    }
            };
            let value = if let Some(home) = trade {
                self.merchant_trade_value(merchant, home, city, turns)
            } else {
                self.merchant_wonder_value(merchant, city, turns)
            } * 0.95_f64.powi(turns);
            if (mission.is_some() || value > 0.0)
                && best.as_ref().is_none_or(|plan| {
                    value > plan.value || (value == plan.value && turns < plan.turns)
                })
            {
                best = Some(MerchantPlan {
                    value,
                    city: city.id,
                    wonder,
                    approach: (x, y),
                    turns,
                });
            }
        }
        best
    }

    pub(super) fn merchant_production_choice(&self, _city: &City) -> Option<(UnitKind, f64)> {
        // daidomestic.c:dai_choose_trade_route checks national destinations
        // first and examines foreign ones only when NationalIC > National.
        // Civ2Civ3 sets both national percentages to zero, so dest_city_found
        // remains false. Existing merchants may still trade; wonder helpers
        // have a separate production advisor.
        None
    }

    pub(crate) fn ai_merchant_valuation_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        let origin = (10, 24);
        for (step, owner, size) in [
            (0, 1, 30),
            (10, HUMAN, 30),
            (1, HUMAN, 1),
            (2, HUMAN, 20),
            (11, 1, 1),
            (12, 2, 10),
        ] {
            let (x, y) = offset_native_by_map(origin.0, origin.1, step, 0);
            self.create_city(owner, x, y);
            self.cities.last_mut().unwrap().size = size;
        }
        self.sign_pact(1, HUMAN, Pact::Alliance);
        self.sign_pact(2, HUMAN, Pact::Alliance);
        self.trade_routes = vec![
            (self.cities[0].id, self.cities[2].id),
            (self.cities[0].id, self.cities[3].id),
            (self.cities[1].id, self.cities[4].id),
            (self.cities[1].id, self.cities[5].id),
        ];
        let home = self.cities[0].clone();
        let destination = self.cities[1].clone();
        let mut merchant = self.make_unit(0, 1, UnitKind::Freight, home.x, home.y);
        merchant.home_city = Some(home.id);
        let charged = self.merchant_trade_value(&merchant, &home, &destination, 0);
        let lost = self.route_value(&destination, &self.cities[4]);
        self.cities[4].owner = 2;
        let uncharged = self.merchant_trade_value(&merchant, &home, &destination, 0);
        let accounts_for_partner =
            lost > 0 && ((uncharged - charged) - f64::from(lost) / 0.05).abs() < 0.0001;
        let mut wonder = home;
        wonder.production = Production::Building(Building::PYRAMIDS);
        wonder.shields = 0;
        let starting = self.merchant_wonder_value(&merchant, &wonder, 0);
        wonder.shields = 1;
        let started = self.merchant_wonder_value(&merchant, &wonder, 0);
        accounts_for_partner
            && starting > started
            && (started - f64::from(8 * merchant.kind.stats().cost)).abs() < 0.0001
    }

    pub(crate) fn ai_merchant_coastal_pickup_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_merchant_missions.clear();
        self.merchant_mission_attempts.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.hut = false;
            tile.river = false;
            tile.known_by[1] = true;
        }
        let home = (10, 24);
        let coast = offset_native_by_map(home.0, home.1, 1, 0);
        let sea = offset_native_by_map(home.0, home.1, 2, 0);
        let destination = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, coast, destination] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, home.0, home.1);
        let origin = self.cities.last().unwrap().id;
        self.create_city(HUMAN, destination.0, destination.1);
        let target = self.cities.last().unwrap().id;
        self.sign_pact(1, HUMAN, Pact::Alliance);
        let passenger = self.spawn_unit(1, UnitKind::Freight, coast.0, coast.1);
        self.units.last_mut().unwrap().home_city = Some(origin);
        let ship = self.spawn_unit(1, UnitKind::Galleon, sea.0, sea.1);
        let ordinary_coast = !self.cities.iter().any(|city| (city.x, city.y) == coast);
        self.run_ai_player(1);
        // Galleon is a native ferry (Transport is an adv.worker). Its lower
        // move rate needs a second phase to complete this sea crossing.
        if self.units.iter().any(|unit| unit.id == passenger) {
            self.update_activities_and_restore();
            self.run_ai_player(1);
        }
        ordinary_coast
            && self.trade_routes.contains(&(origin, target))
            && self.units.iter().all(|unit| unit.id != passenger)
            && self
                .units
                .iter()
                .any(|unit| unit.id == ship && (unit.x, unit.y) != sea)
    }

    pub(crate) fn ai_merchant_missions_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_merchant_missions.clear();
        self.merchant_mission_attempts.clear();
        self.turn = 0;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.road = true;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
        }
        let home = (10, 24);
        for (step, owner, size) in [(0, 1, 20), (5, HUMAN, 20), (6, 2, 60)] {
            let (x, y) = offset_native_by_map(home.0, home.1, step, 0);
            self.create_city(owner, x, y);
            self.cities.last_mut().unwrap().size = size;
        }
        self.sign_pact(1, HUMAN, Pact::Alliance);
        self.sign_pact(1, 2, Pact::Peace);
        let actor = self.spawn_unit(1, UnitKind::Freight, home.0, home.1);
        self.units.last_mut().unwrap().home_city = Some(self.cities[0].id);
        let merchant = self.units.last().unwrap().clone();
        self.set_random_seed(1); // Native fc_rand(2)'s first draw is0: allied-only.
        self.prepare_merchant_mission(&merchant);
        let allied = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[1].id);
        let rng = self.random_snapshot();
        self.cities[2].size = 100;
        self.load_ai_merchant_ferries(1);
        let retained = self.random_snapshot() == rng
            && self
                .ai_merchant_missions
                .get(&actor)
                .is_some_and(|m| m.city == self.cities[1].id);
        self.ai_merchant_missions.remove(&actor);
        self.merchant_mission_attempts.clear();
        self.set_random_seed(2); // Native first draw is1: profitable nonwar destination.
        self.prepare_merchant_mission(&merchant);
        let neutral = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[2].id);
        allied && retained && neutral && self.ai_merchant_boat_timeout_playtest()
    }

    fn ai_merchant_boat_timeout_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_merchant_missions.clear();
        self.merchant_mission_attempts.clear();
        self.turn = 0;
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let destination = (10, 40);
        for (owner, (x, y)) in [(1, home), (HUMAN, destination)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.sign_pact(1, HUMAN, Pact::Alliance);
        let actor = self.spawn_unit(1, UnitKind::Freight, home.0, home.1);
        self.units.last_mut().unwrap().home_city = Some(self.cities[0].id);
        self.load_ai_merchant_ferries(1);
        let assigned = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[1].id);
        self.cities[0].production = Production::Building(Building::PYRAMIDS);
        self.turn = 15;
        self.load_ai_merchant_ferries(1);
        let waited = self
            .ai_merchant_missions
            .get(&actor)
            .is_some_and(|m| m.city == self.cities[1].id);
        self.turn = 16;
        self.load_ai_merchant_ferries(1);
        assigned
            && waited
            && self
                .ai_merchant_missions
                .get(&actor)
                .is_some_and(|m| m.city == self.cities[0].id && m.wonder)
    }

    pub(super) fn run_ai_merchant(&mut self, merchant: &Unit) {
        if !self.ai_merchant_missions.contains_key(&merchant.id) {
            self.prepare_merchant_mission(merchant);
        }
        let Some(snapshot) = self
            .units
            .iter()
            .find(|unit| unit.id == merchant.id)
            .cloned()
        else {
            return;
        };
        let merchant = &snapshot;
        let Some(MerchantPlan {
            city,
            wonder,
            approach: (x, y),
            ..
        }) = self.merchant_plan(merchant, 0)
        else {
            return;
        };
        if (merchant.x, merchant.y) != (x, y) {
            self.ai_goto_unit(
                merchant.id,
                (x, y),
                if wonder {
                    GotoTask::Wonder
                } else {
                    GotoTask::Trade
                },
            );
        }
        if wonder {
            self.help_wonder(merchant.id, city);
        } else {
            self.deliver_goods(merchant.id, city);
        }
    }

    pub(super) fn deliver_goods(&mut self, unit: u32, city: u32) -> bool {
        self.prune_trade_routes();
        let Some(merchant) = self.units.iter().find(|item| item.id == unit) else {
            return false;
        };
        let Some(to) = self
            .cities
            .iter()
            .find(|item| item.id == city && self.can_deliver_goods(merchant, item))
        else {
            return false;
        };
        let from = self.home_for_merchant(merchant).unwrap();
        let owner = merchant.owner;
        let (a, b) = (from.id, to.id);
        let replacements = self.route_replacements(from, to);
        let gold = self.delivery_gold(from, to);
        let destination = to.name.clone();
        if let Some((old_home, old_dest)) = replacements {
            if let Some(old) = old_home {
                self.remove_route(a, old);
            }
            if let Some(old) = old_dest {
                self.remove_route(b, old);
            }
            self.trade_routes.push((a, b));
        }
        self.players[owner].gold += gold;
        self.remove_units_with_cargo(vec![unit]);
        self.refresh_visibility();
        let outcome = if replacements.is_some() {
            "Trade route established"
        } else {
            "Goods delivered"
        };
        self.add_event(format!("{outcome} in {destination}. Earned {gold} gold."));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merchant_actual_ferry_capacity_reselects_for_guard_and_keeps_mission() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        game.ai_ferries = Default::default();
        game.ai_merchant_missions.clear();
        game.ai_guard_targets.clear();
        game.ai_unit_guards.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
        }
        let home = (10, 24);
        let destination = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, destination] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        game.create_city(1, home.0, home.1);
        let origin = game.cities.last().unwrap().id;
        game.create_city(1, destination.0, destination.1);
        let city = game.cities.last().unwrap().id;
        game.cities.last_mut().unwrap().production = Production::Building(Building::PYRAMIDS);
        let merchant = game.spawn_unit(1, UnitKind::Caravan, home.0, home.1);
        game.units.last_mut().unwrap().home_city = Some(origin);
        game.ai_merchant_missions.insert(
            merchant,
            MerchantMission {
                city,
                wonder: true,
                owner: 1,
            },
        );
        let small = game.spawn_unit(1, UnitKind::Trireme, home.0, home.1);
        game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        game.units.last_mut().unwrap().transport = Some(small);
        let farther = offset_native_by_map(home.0, home.1, -2, 0);
        let roomy = game.spawn_unit(1, UnitKind::Trireme, farther.0, farther.1);
        let actor = game
            .units
            .iter()
            .find(|u| u.id == merchant)
            .unwrap()
            .clone();
        assert_eq!(
            game.ferry_delivery(&actor, 0, &BTreeMap::new())
                .map(|p| p.ship),
            Some(small)
        );
        let guard = game.spawn_unit(1, UnitKind::Partisan, home.0, home.1);
        game.assign_ai_guard(guard, guards::GuardTarget::Unit(merchant));
        let actual = game.load_ai_merchant_ferries(1);
        assert_eq!(actual.get(&roomy), Some(&merchant));
        assert!(!actual.contains_key(&small));
        assert_eq!(
            game.ai_merchant_missions.get(&merchant).map(|m| m.city),
            Some(city)
        );
        game.remove_units_with_cargo(vec![roomy]);
        assert!(game.load_ai_merchant_ferries(1).is_empty());
        assert_eq!(
            game.ai_merchant_missions.get(&merchant).map(|m| m.city),
            Some(city)
        );
    }

    #[test]
    fn merchant_values_reciprocal_route_losses_and_unstarted_wonders() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_merchant_valuation_playtest());
    }

    #[test]
    fn marketplace_opportunity_cost_includes_the_journey_discount() {
        assert!(worthwhile_marketplace(100.0, 50, 5, 0));
        assert!(!worthwhile_marketplace(100.0, 50, 5, 5));
    }

    #[test]
    fn marketplace_deliveries_must_repay_production_opportunity_cost() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.create_city(1, 10, 24);
        game.create_city(HUMAN, 14, 24);
        game.sign_pact(1, HUMAN, Pact::Peace);
        let home = game.cities[0].clone();
        let dest = game.cities[1].clone();
        game.trade_routes.push((home.id, dest.id));
        let merchant = game.make_unit(0, 1, UnitKind::Freight, home.x, home.y);
        assert!(game.delivery_gold(&home, &dest) < merchant.kind.stats().cost);
        assert_eq!(game.merchant_trade_value(&merchant, &home, &dest, 0), 0.0);
    }

    #[test]
    fn ferry_landing_counts_only_the_movement_available_to_the_caravan() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.road = false;
            tile.railroad = false;
        }
        let sea = (10, 24);
        let landing = offset_native_by_map(sea.0, sea.1, 1, 0);
        let destination = offset_native_by_map(sea.0, sea.1, 2, 0);
        game.tile_mut(landing.0, landing.1).terrain = Terrain::Mountains;
        game.tile_mut(destination.0, destination.1).terrain = Terrain::Grassland;
        game.create_city(HUMAN, destination.0, destination.1);
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let mut ship = game.make_unit(0, 1, UnitKind::Transport, sea.0, sea.1);
        ship.moves_left = 0;
        let allowance = game.unit_move_allowance(ship.owner, ship.kind);
        let mut merchant = game.make_unit(0, 1, UnitKind::Caravan, sea.0, sea.1);
        let labels = game.continent_labels();
        let city = &game.cities[0];

        assert_eq!(
            game.inland_ferry_approach(&merchant, &ship, city, allowance, 0, &labels),
            Some((1, sea.0, sea.1, landing))
        );
        merchant.moves_left = 0;
        assert_eq!(
            game.inland_ferry_approach(&merchant, &ship, city, allowance, 0, &labels)
                .map(|(turns, _, _, _)| turns),
            Some(2)
        );
    }

    #[test]
    fn generic_merchant_ferry_fixtures_pick_up_and_deliver_with_native_galleons() {
        for mode in 0..3 {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            let settler = game
                .units
                .iter()
                .find(|u| u.owner == HUMAN && u.kind.stats().can_found)
                .unwrap()
                .id;
            assert!(game.apply(Command::FoundCity(settler)));
            let (merchant, ferry, home, destination) = match mode {
                0 => game.ai_merchant_ferry_playtest_setup(),
                1 => game.ai_ferry_dispatch_playtest_setup(),
                _ => game.ai_inland_ferry_playtest_setup(),
            };
            assert_eq!(
                game.units.iter().find(|u| u.id == ferry).unwrap().kind,
                UnitKind::Galleon
            );
            let position =
                |game: &Simulation, id| game.units.iter().find(|u| u.id == id).map(|u| (u.x, u.y));
            let initial = position(&game, ferry);
            let waiting = position(&game, merchant);
            let mut turns = 0;
            while turns < [1, 5, 4][mode] && !game.merchant_playtest_route_exists(home, destination)
            {
                assert!(game.apply(Command::EndTurn));
                turns += 1;
                if mode == 1 && turns == 1 {
                    assert_ne!(position(&game, ferry), initial);
                    assert_eq!(position(&game, merchant), waiting);
                }
            }
            assert_eq!(turns, [1, 3, 1][mode]);
            assert!(
                game.merchant_playtest_route_exists(home, destination),
                "mode{mode} {turns}"
            );
            assert!(game.units.iter().all(|u| u.id != merchant));
            assert!(game.units.iter().any(|u| u.id == ferry && (mode != 2 || game.tile(u.x,u.y).terrain.is_water())));
        }
    }

    #[test]
    fn coastal_merchants_board_and_deliver_without_standing_in_a_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_merchant_coastal_pickup_playtest());
    }

    #[test]
    fn exhausted_merchants_do_not_change_tasks_or_consume_random_draws() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 24);
        game.cities[0].production = Production::Building(Building::PYRAMIDS);
        let id = game.spawn_unit(1, UnitKind::Freight, 10, 24);
        let unit = game.units.last_mut().unwrap();
        unit.home_city = Some(game.cities[0].id);
        unit.moves_left = 0;
        let rng = game.random_snapshot();
        game.load_ai_merchant_ferries(1);
        assert_eq!(game.random_snapshot(), rng);
        assert!(!game.ai_merchant_missions.contains_key(&id));
        assert!(!game.merchant_mission_attempts.contains(&id));
    }

    #[test]
    fn a_captured_merchant_keeps_its_birth_age_and_loses_the_old_mission() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 24);
        game.turn = 2;
        let old = game.spawn_unit(1, UnitKind::Freight, 10, 24);
        let victim = game.units.last().unwrap().clone();
        game.ai_merchant_missions.insert(
            old,
            MerchantMission {
                owner: 1,
                city: game.cities[0].id,
                wonder: true,
            },
        );
        game.turn = 18;
        game.transfer_unit(&victim, HUMAN, None);
        let gained = game.units.last().unwrap();
        assert_ne!(gained.id, old);
        assert_eq!(game.merchant_birth_turns.get(&gained.id), Some(&2));
        assert!(!game.merchant_birth_turns.contains_key(&old));
        assert!(!game.ai_merchant_missions.contains_key(&old));
    }

    #[test]
    fn lost_home_recovery_chooses_a_city_on_the_current_continent() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.hut = false;
        }
        for y in 18..=42 {
            for x in 0..=8 {
                game.tile_mut(x, y).terrain = Terrain::Grassland;
            }
        }
        game.create_city(1, 6, 40);
        game.tile_mut(11, 24).terrain = Terrain::Grassland;
        game.create_city(1, 11, 24);
        let id = game.spawn_unit(1, UnitKind::Freight, 8, 24);
        game.units.last_mut().unwrap().home_city = Some(u32::MAX);
        let merchant = game.units.last().unwrap().clone();
        game.prepare_merchant_mission(&merchant);
        let merchant = game.units.iter().find(|unit| unit.id == id).unwrap();
        assert_ne!((merchant.x, merchant.y), (8, 24));
        assert!(merchant.x <= 8);
    }

    #[test]
    fn merchant_assignments_retain_destinations_and_apply_foreign_scope_and_boat_patience() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_merchant_missions_playtest());
    }

    #[test]
    fn ai_freight_can_help_a_wonder_under_construction() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 20);
        game.cities[0].production = Production::Building(Building::PYRAMIDS);
        let freight = game.spawn_unit(1, UnitKind::Freight, 10, 20);
        game.units.last_mut().unwrap().home_city = Some(game.cities[0].id);
        let unit = game.units.last().unwrap().clone();
        game.run_ai_merchant(&unit);
        assert_eq!(game.cities[0].shields, 50);
        assert!(game.units.iter().all(|unit| unit.id != freight));
    }

    #[test]
    fn repeat_deliveries_pay_gold_and_plague_spreads_only_over_active_routes() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let (caravan, _, home, foreign) = game.merchant_playtest_setup();
        let owner = game.players[HUMAN].gold;
        let repeat = game
            .units
            .iter()
            .find(|unit| unit.id == caravan)
            .unwrap()
            .clone();
        assert!(game.apply(Command::DeliverGoods(caravan, foreign)));
        assert_eq!(game.trade_routes, [(home, foreign)]);
        assert!(game.players[HUMAN].gold > owner);
        let after_first = game.players[HUMAN].gold;
        let duplicate = game.spawn_unit(HUMAN, UnitKind::Freight, repeat.x, repeat.y);
        game.units.last_mut().unwrap().home_city = Some(home);
        assert!(game.apply(Command::DeliverGoods(duplicate, foreign)));
        assert_eq!(game.trade_routes, [(home, foreign)]);
        assert!(game.players[HUMAN].gold > after_first);

        let home_index = game.cities.iter().position(|city| city.id == home).unwrap();
        let partner = game
            .cities
            .iter_mut()
            .find(|city| city.id == foreign)
            .unwrap();
        partner.last_plague = Some(game.turn);
        game.cities[home_index].size = 9;
        assert_eq!(game.trade_illness(&game.cities[home_index]), 4);
        game.turn += 5;
        assert_eq!(game.trade_illness(&game.cities[home_index]), 0);
        assert!(game.cancel_pact(HUMAN, 1));
        assert!(game.trade_routes.is_empty());
    }
}
