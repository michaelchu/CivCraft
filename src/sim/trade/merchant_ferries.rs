//! Ferry coordination for merchant units seeking trade deliveries.

use super::*;

impl Simulation {
    pub(super) fn inland_ferry_approach(
        &self,
        merchant: &Unit,
        ship: &Unit,
        city: &City,
        allowance: i32,
        remaining: i32,
        labels: &[usize],
    ) -> Option<(i32, i32, i32, (i32, i32))> {
        let destination = labels[self.map_wrap.index(city.x, city.y)];
        let mut best = None;
        for (index, &label) in labels.iter().enumerate() {
            if label != destination || label == 0 {
                continue;
            }
            let (lx, ly) = self.map_wrap.position(index);
            if self.treaty_blocks_move(merchant, lx, ly)
                || self.units.iter().any(|unit| {
                    (unit.x, unit.y) == (lx, ly) && !self.allied(merchant.owner, unit.owner)
                })
            {
                continue;
            }
            let sea_approach = self
                .map_wrap
                .neighbors(lx, ly)
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

    pub(super) fn ferry_delivery(
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
            let allowance = self.unit_move_rate(ship).max(MOVE_FRAGMENTS);
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
                endpoint = self.map_wrap.offset(endpoint.0, endpoint.1, dx, dy)?;
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
                let approach = self
                    .map_wrap
                    .neighbors(city.x, city.y)
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

    pub(in crate::sim) fn load_ai_merchant_ferries(&mut self, owner: usize) -> BTreeMap<u32, u32> {
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

    pub(in crate::sim) fn run_merchant_ferry(&mut self, ship: u32, pickup: Option<u32>) -> bool {
        if let Some(passenger) = pickup {
            let Some(unit) = self.unit_by_id(passenger).cloned() else {
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
            if (merchant.x, merchant.y) != (x, y)
                && let Some(city) = self.city_by_id(ferry.city)
            {
                self.ai_ferry_goto_waypoint(ship, (x, y), (city.x, city.y));
            }
            if let Some((lx, ly)) = ferry.landing {
                let arrived = self
                    .units
                    .iter()
                    .any(|unit| unit.id == ship && (unit.x, unit.y) == (x, y));
                if arrived {
                    let (dx, dy) = self.map_wrap.vector(x, y, lx, ly);
                    if self.try_move_unit(merchant.id, dx, dy) == MoveResult::Moved
                        && let Some(walker) = self
                            .units
                            .iter()
                            .find(|unit| unit.id == merchant.id)
                            .cloned()
                        && let Some(city) = self.city_by_id(ferry.city)
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
}
