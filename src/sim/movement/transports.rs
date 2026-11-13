//! Implements transports behavior within the movement subsystem.

use super::*;

impl Simulation {
    pub(in crate::sim) fn is_port(&self, city: &City) -> bool {
        self.map_wrap
            .civ3_neighbors(city.x, city.y)
            .any(|(x, y)| self.tile(x, y).terrain.is_water())
    }

    pub(in crate::sim) fn naval_city_access(&self, city: &City, kind: UnitKind) -> bool {
        if kind != UnitKind::Galley {
            return self.is_port(city);
        }
        let native = |tile: &Tile| {
            matches!(tile.terrain, Terrain::Coast | Terrain::Sea | Terrain::Lake) || tile.river
        };
        native(self.tile(city.x, city.y))
            || self
                .map_wrap
                .civ3_neighbors(city.x, city.y)
                .any(|(x, y)| native(self.tile(x, y)))
    }

    pub(crate) fn cargo_count(&self, carrier: u32) -> usize {
        self.units
            .iter()
            .filter(|unit| unit.transport == Some(carrier))
            .count()
    }

    pub(in crate::sim) fn boarding_target(&self, passenger: &Unit, x: i32, y: i32) -> Option<u32> {
        self.units
            .iter()
            .find(|ship| (ship.x, ship.y) == (x, y) && self.can_board_ship(passenger, ship))
            .map(|ship| ship.id)
    }

    pub(in crate::sim) fn can_board_ship(&self, passenger: &Unit, ship: &Unit) -> bool {
        passenger.transport != Some(ship.id)
            && passenger.id != ship.id
            && !self.transport_would_create_cycle(passenger.id, ship.id)
            && self.allied(passenger.owner, ship.owner)
            && self.ruleset_transport_pair_allowed_fail_closed(ship.kind, passenger.kind)
            && (ship.kind != UnitKind::Helicopter
                || self.helicopter_embark_access(passenger, ship.x, ship.y))
            && self.ruleset_transport_has_space_fail_closed(ship.kind, self.cargo_count(ship.id))
    }

    pub(in crate::sim) fn board_specific_ship(&mut self, unit_id: u32, ship_id: u32) -> bool {
        let Some(index) = self.unit_index_by_id(unit_id) else {
            return false;
        };
        let unit = &self.units[index];
        if unit.transport.is_some()
            || !self.units.iter().any(|ship| {
                ship.id == ship_id
                    && (ship.x, ship.y) == (unit.x, unit.y)
                    && self.can_board_ship(unit, ship)
            })
            || !self.action_available_fail_closed(unit_id, "Transport Board", (unit.x, unit.y))
        {
            return false;
        }
        self.set_transport(index, Some(ship_id));
        true
    }

    pub(in crate::sim) fn embark_adjacent_ship(&mut self, unit_id: u32, ship_id: u32) -> bool {
        let Some(index) = self.unit_index_by_id(unit_id) else {
            return false;
        };
        let passenger = &self.units[index];
        let Some(ship) = self.unit_by_id(ship_id) else {
            return false;
        };
        let (x, y) = (ship.x, ship.y);
        if passenger.moves_left <= 0
            || passenger.work_left > 0
            || passenger.state == UnitState::Sentry
            || passenger.transport.is_some()
            || !self
                .map_wrap
                .civ3_neighbors(passenger.x, passenger.y)
                .any(|tile| tile == (x, y))
            || !self.tile(x, y).terrain.is_water()
            || self.treaty_blocks_move(passenger, x, y)
            || !self.can_board_ship(passenger, ship)
            || self.best_defender_id(passenger, x, y).is_some()
            || !self.ai_guard_allows_step(passenger, (x, y))
            || !self.action_available_fail_closed(unit_id, "Transport Embark", (x, y))
        {
            return false;
        }
        let old_position = (passenger.x, passenger.y);
        let cost = self.raw_move_cost_for(passenger, x, y);
        let passenger = &mut self.units[index];
        (passenger.x, passenger.y) = (x, y);
        passenger.transport = Some(ship_id);
        passenger.moves_left = (passenger.moves_left - cost).max(0);
        passenger.moved = true;
        passenger.state = UnitState::Idle;
        self.list_order.unit(passenger);
        self.record_transport_change(unit_id, Some(ship_id));
        self.assign_ai_ferry(ship_id, unit_id);
        self.ai_guard_follow_step(unit_id, old_position);
        self.refresh_visibility();
        true
    }

    fn helicopter_embark_access(&self, passenger: &Unit, x: i32, y: i32) -> bool {
        self.ruleset_unit_can_embark_fail_closed(passenger.kind, UnitKind::Helicopter)
            || self.refuel_site(passenger.owner, x, y)
    }

    fn helicopter_disembark_access(&self, passenger: &Unit, x: i32, y: i32) -> bool {
        self.ruleset_unit_can_disembark_fail_closed(passenger.kind, UnitKind::Helicopter)
            || self.refuel_site(passenger.owner, x, y)
    }

    pub(in crate::sim) fn can_leave_transport(&self, passenger: &Unit) -> bool {
        passenger.transport.is_none_or(|id| {
            self.units
                .iter()
                .find(|unit| unit.id == id)
                .is_some_and(|carrier| {
                    carrier.kind != UnitKind::Helicopter
                        || self.helicopter_disembark_access(passenger, passenger.x, passenger.y)
                })
        })
    }

    pub(crate) fn transport_action(&self, unit: &Unit) -> Option<&'static str> {
        if unit.transport.is_some() {
            let can_exit = (unit.kind.is_aircraft()
                || !self.tile(unit.x, unit.y).terrain.is_water())
                && self.can_leave_transport(unit);
            (can_exit
                && self.action_available_for_unit_fail_closed(
                    unit,
                    "Transport Deboard",
                    (unit.x, unit.y),
                ))
            .then_some("Deboard")
        } else {
            (self.boarding_target(unit, unit.x, unit.y).is_some()
                && self.action_available_for_unit_fail_closed(
                    unit,
                    "Transport Board",
                    (unit.x, unit.y),
                ))
            .then_some("Board")
        }
    }

    pub(crate) fn can_unload_all(&self, carrier: &Unit) -> bool {
        carrier.owner == HUMAN
            && self
                .ruleset_transport_capacity(carrier.kind)
                .is_ok_and(|capacity| capacity > 0)
            && self.cargo_count(carrier.id) > 0
            && self.units.iter().any(|cargo| {
                cargo.transport == Some(carrier.id)
                    && self.transport_action(cargo) == Some("Deboard")
            })
    }

    pub(in crate::sim) fn unload_all(&mut self, carrier_id: u32) -> bool {
        let Some(carrier) = self.unit_by_id(carrier_id) else {
            return false;
        };
        if !self.can_unload_all(carrier) {
            return false;
        }
        let mut cargo: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.transport == Some(carrier_id))
            .filter(|unit| self.transport_action(unit) == Some("Deboard"))
            .map(|unit| unit.id)
            .collect();
        cargo.sort_unstable();
        let mut unloaded = false;
        for id in cargo {
            if let Some(index) = self.unit_index_by_id(id)
                && self.transport_action(&self.units[index]) == Some("Deboard")
            {
                self.set_transport(index, None);
                unloaded = true;
            }
        }
        unloaded
    }

    pub(in crate::sim) fn board_or_deboard(&mut self, unit_id: u32) -> bool {
        let Some(index) = self.unit_index_by_id(unit_id) else {
            return false;
        };
        let unit = &self.units[index];
        if self.transport_action(unit).is_none() {
            return false;
        }
        let carrier = if unit.transport.is_some() {
            None
        } else {
            self.boarding_target(unit, unit.x, unit.y)
        };
        self.set_transport(index, carrier);
        true
    }

    pub(in crate::sim) fn set_transport(&mut self, index: usize, carrier: Option<u32>) {
        self.record_transport_change(self.units[index].id, carrier);
        self.units[index].transport = carrier;
        self.units[index].state = UnitState::Idle;
        self.units[index].clear_work();
        let unit = self.units[index].clone();
        if let Some(ship) = carrier {
            self.assign_ai_ferry(ship, unit.id);
        } else {
            self.release_ai_ferry(unit.id);
        }
        self.reconcile_city_workers();
    }

    pub(in crate::sim) fn move_cargo(&mut self, transport: u32, x: i32, y: i32) {
        let mut transports = vec![transport];
        while let Some(transport) = transports.pop() {
            for passenger in self
                .units
                .iter_mut()
                .filter(|unit| unit.transport == Some(transport))
            {
                passenger.x = x;
                passenger.y = y;
                self.list_order.unit(passenger);
                passenger.clear_work();
                transports.push(passenger.id);
            }
        }
    }

    pub(in crate::sim) fn remove_units_with_cargo(&mut self, mut losses: Vec<u32>) {
        let mut index = 0;
        while let Some(&lost) = losses.get(index) {
            let mut cargo: Vec<_> = self
                .units
                .iter()
                .filter(|unit| unit.transport == Some(lost) && !losses.contains(&unit.id))
                .map(|unit| (!self.can_leave_transport(unit), unit.clone()))
                .collect();
            cargo.sort_by_key(|(helpless, _)| !*helpless);
            for (helpless, passenger) in cargo {
                if !self.rescue_transport_cargo(lost, &passenger, helpless, &losses) {
                    losses.push(passenger.id);
                }
            }
            index += 1;
        }
        self.units.retain(|unit| !losses.contains(&unit.id));
        for lost in losses {
            self.discard_unit_ai(lost);
        }
    }

    fn rescue_transport_cargo(
        &mut self,
        lost: u32,
        passenger: &Unit,
        helpless: bool,
        losses: &[u32],
    ) -> bool {
        let tile = (passenger.x, passenger.y);
        let can_survive = !helpless
            && (passenger.kind.is_aircraft() || !self.tile(tile.0, tile.1).terrain.is_water());
        let mut destination = tile;
        let mut replacement = None;
        if !can_survive {
            if !helpless {
                replacement = self
                    .units
                    .iter()
                    .filter(|ship| (ship.x, ship.y) == tile && self.can_board_ship(passenger, ship))
                    .filter_map(|ship| {
                        let mut outermost = ship;
                        let mut depth = 0;
                        let mut total_moves = 0;
                        loop {
                            if losses.contains(&outermost.id) || outermost.id == passenger.id {
                                return None;
                            }
                            total_moves += outermost.moves_left + self.unit_move_rate(outermost);
                            let Some(parent) = outermost.transport else {
                                break;
                            };
                            depth += 1;
                            if depth >= 5 {
                                return None;
                            }
                            outermost = self.unit_by_id(parent)?;
                        }
                        let busy = ship.state != UnitState::Idle || ship.work_left > 0;
                        let restricted_unload = ship.kind == UnitKind::Helicopter
                            && !self.helicopter_disembark_access(passenger, ship.x, ship.y);
                        Some((
                            (
                                busy,
                                restricted_unload,
                                depth,
                                -outermost.moves_left,
                                -total_moves,
                            ),
                            ship.id,
                        ))
                    })
                    .min_by_key(|(priority, _)| *priority)
                    .map(|(_, id)| id);
            }
            if replacement.is_none() {
                let carrier_kind = self.unit_by_id(lost).unwrap().kind;
                let Some(city) = self
                    .cities
                    .iter()
                    .filter(|city| {
                        city.owner == passenger.owner
                            && (!carrier_kind.is_naval()
                                || self.naval_city_access(city, carrier_kind))
                    })
                    .min_by_key(|city| self.map_distance(tile.0, tile.1, city.x, city.y))
                else {
                    return false;
                };
                destination = (city.x, city.y);
            }
        }

        let unit = self
            .units
            .iter_mut()
            .find(|unit| unit.id == passenger.id)
            .unwrap();
        unit.transport = replacement;
        if unit.state == UnitState::Sentry {
            unit.state = UnitState::Idle;
        }
        if destination != tile {
            (unit.x, unit.y) = destination;
            self.move_cargo(passenger.id, destination.0, destination.1);
        }
        true
    }
}
