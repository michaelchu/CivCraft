//! Implements assault transports behavior within the ai subsystem.

use super::*;

impl Simulation {
    pub(super) fn raider_landing(
        &self,
        boat: &Unit,
        overseas: Option<(&[usize], usize)>,
        troops: Option<&[&Unit]>,
        companions: &[&Unit],
    ) -> Option<(i32, i32, i32, (i32, i32))> {
        self.cities
            .iter()
            .filter(|city| {
                self.at_war(city.owner, boat.owner)
                    && troops.is_none_or(|troops| self.assault_capture_chance(city, troops) >= 0.5)
                    && overseas.is_none_or(|(labels, origin)| {
                        labels[self.map_wrap.index(city.x, city.y)] != origin
                            && self.tile(city.x, city.y).known_by[boat.owner]
                    })
            })
            .flat_map(|city| {
                CIV3_DIRS
                    .into_iter()
                    .filter_map(move |(dx, dy)| self.map_wrap.civ3_offset(city.x, city.y, dx, dy))
            })
            .filter(|&(x, y)| !self.tile(x, y).terrain.is_water())
            .filter(|&(x, y)| {
                self.units
                    .iter()
                    .all(|unit| (unit.x, unit.y) != (x, y) || unit.owner == boat.owner)
                    && self
                        .cities
                        .iter()
                        .all(|city| (city.x, city.y) != (x, y) || city.owner == boat.owner)
            })
            .flat_map(|(lx, ly)| {
                self.map_wrap
                    .civ3_neighbors(lx, ly)
                    .filter_map(move |water| {
                        let direction = CIV3_DIRS.iter().copied().find(|&(dx, dy)| {
                            self.map_wrap.civ3_offset(water.0, water.1, dx, dy) == Some((lx, ly))
                        })?;
                        Some((water, direction))
                    })
            })
            .filter(|&((x, y), _)| self.tile(x, y).terrain.is_water())
            .filter_map(|((x, y), direction)| {
                let mut distance = 0;
                for ship in std::iter::once(boat).chain(companions.iter().copied()) {
                    if (ship.x, ship.y) != (x, y) {
                        let cost = self.route_cost_within(ship, x, y, i32::MAX)?;
                        let allowance = self.unit_move_rate(ship).max(1);
                        let turns = ((cost - ship.moves_left).max(0) + allowance - 1) / allowance;
                        distance = distance.max(turns);
                    }
                }
                Some((distance, x, y, direction))
            })
            .min_by_key(|candidate| candidate.0)
    }

    pub(crate) fn assault_transport_playtest_has_orders(&mut self, owner: usize) -> bool {
        !self
            .dispatch_assault_transports(owner, |_| false, |_| false)
            .is_empty()
    }

    pub(super) fn military_ferry_ready(&self, boat: u32) -> bool {
        if self
            .units
            .iter()
            .find(|unit| unit.id == boat)
            .is_some_and(|unit| {
                unit.hp < unit.kind.stats().hp
                    && self
                        .cities
                        .iter()
                        .any(|city| (city.x, city.y) == (unit.x, unit.y))
            })
        {
            return false;
        }
        self.ai_ferries
            .boats
            .get(&boat)
            .is_none_or(|record| match record.state {
                ferries::FerryState::Available => true,
                ferries::FerryState::Assigned(id) => self.units.iter().any(|unit| {
                    unit.id == id
                        && unit.kind.is_military()
                        && !unit.kind.is_aircraft()
                        && !unit.kind.is_naval()
                        && unit.transport == Some(boat)
                }),
            })
    }

    pub(super) fn coordinate_assault_fleets(
        &mut self,
        owner: usize,
        unavailable: impl Fn(u32) -> bool,
        reserved: impl Fn(u32) -> bool,
    ) -> std::collections::HashSet<u32> {
        let mut assigned = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return assigned;
        }
        let ports: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner && self.is_port(city))
            .map(|city| city.id)
            .collect();
        let labels = self.continent_labels();
        for port in ports {
            let city = self.city_by_id(port).unwrap();
            let origin = labels[self.map_wrap.index(city.x, city.y)];
            let overseas = Some((labels.as_slice(), origin));
            let boats: Vec<_> = self
                .units
                .iter()
                .filter(|boat| {
                    boat.owner == owner
                        && matches!(
                            boat.kind,
                            UnitKind::Trireme
                                | UnitKind::Caravel
                                | UnitKind::Galleon
                                | UnitKind::Transport
                        )
                        && !assigned.contains(&boat.id)
                        && !unavailable(boat.id)
                        && self.military_ferry_ready(boat.id)
                })
                .collect();
            let mut loaded = Vec::new();
            let mut troops = Vec::new();
            for boat in &boats {
                let cargo: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| unit.transport == Some(boat.id))
                    .collect();
                if !cargo.is_empty()
                    && cargo.iter().all(|unit| {
                        unit.home_city == Some(port)
                            && unit.kind.is_military()
                            && !unit.kind.is_aircraft()
                            && !unit.kind.is_naval()
                    })
                {
                    loaded.push(*boat);
                    troops.extend(cargo);
                }
            }

            let mut boarding = Vec::new();
            let mut guard = None;
            if loaded.len() < 2 {
                if !loaded.is_empty() || self.city_threats(city).next().is_some() {
                    continue;
                }
                let mut spare: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == owner
                            && (unit.x, unit.y) == (city.x, city.y)
                            && unit.kind.is_military()
                            && !unit.kind.is_aircraft()
                            && !unit.kind.is_naval()
                            && unit.transport.is_none()
                            && !assigned.contains(&unit.id)
                            && !reserved(unit.id)
                    })
                    .collect();
                spare.sort_by_key(|unit| {
                    (
                        std::cmp::Reverse(unit.kind.stats().defense * unit.hp),
                        unit.id,
                    )
                });
                if spare.len() < 3 {
                    continue;
                }
                guard = Some(spare.remove(0).id);
                spare.retain(|unit| unit.home_city.is_none_or(|home| home == port));
                let mut arrivals: Vec<_> = boats
                    .into_iter()
                    .filter_map(|boat| {
                        if self.cargo_count(boat.id) != 0
                            || !self.naval_city_access(city, boat.kind)
                        {
                            return None;
                        }
                        let cost = self.route_cost_within(boat, city.x, city.y, i32::MAX)?;
                        let allowance = self.unit_move_rate(boat).max(1);
                        Some((
                            boat,
                            ((cost - boat.moves_left).max(0) + allowance - 1) / allowance,
                        ))
                    })
                    .collect();
                arrivals.sort_by_key(|&(boat, steps)| (steps, boat.id));
                for (boat, _) in arrivals {
                    let mut passengers = Vec::new();
                    spare.retain(|unit| {
                        if passengers.len() < boat.kind.transport_capacity()
                            && boat.kind.can_carry(unit.kind)
                        {
                            passengers.push(*unit);
                            false
                        } else {
                            true
                        }
                    });
                    if passengers.is_empty() {
                        continue;
                    }
                    let mut staged = boat.clone();
                    (staged.x, staged.y) = (city.x, city.y);
                    if self
                        .raider_landing(&staged, overseas, Some(&passengers), &[])
                        .is_some()
                    {
                        // The normal dispatch can handle an expedition on one ship.
                        loaded.clear();
                        break;
                    }
                    boarding.extend(passengers.iter().map(|unit| (unit.id, boat.id)));
                    troops.extend(passengers);
                    loaded.push(boat);
                    if loaded.len() >= 2 {
                        let staged: Vec<_> = loaded
                            .iter()
                            .map(|ship| {
                                let mut ship = (*ship).clone();
                                (ship.x, ship.y) = (city.x, city.y);
                                ship
                            })
                            .collect();
                        let companions: Vec<_> = staged[1..].iter().collect();
                        if self
                            .raider_landing(&staged[0], overseas, Some(&troops), &companions)
                            .is_some()
                        {
                            break;
                        }
                    }
                }
            }
            if loaded.len() < 2 {
                continue;
            }
            let assembly = (city.x, city.y);
            let mut staged = Vec::new();
            let sailing_ships = if boarding.is_empty() {
                loaded.clone()
            } else {
                staged.extend(loaded.iter().map(|ship| {
                    let mut ship = (*ship).clone();
                    (ship.x, ship.y) = assembly;
                    ship
                }));
                staged.iter().collect()
            };
            let Some((_, x, y, (dx, dy))) = self.raider_landing(
                sailing_ships[0],
                overseas,
                Some(&troops),
                &sailing_ships[1..],
            ) else {
                continue;
            };
            let ship_ids: Vec<_> = loaded.iter().map(|ship| ship.id).collect();
            let troop_ids: Vec<_> = troops.iter().map(|unit| unit.id).collect();
            let assembling =
                !boarding.is_empty() && loaded.iter().any(|ship| (ship.x, ship.y) != assembly);
            if assembling {
                assigned.extend(guard);
                assigned.extend(ship_ids.iter().copied());
                assigned.extend(troop_ids.iter().copied());
                for id in ship_ids {
                    self.unit_goto_tiles.insert(id, assembly);
                    self.ai_goto_unit(id, assembly, GotoTask::None);
                }
                continue;
            }
            for (unit, ship) in boarding {
                if self.board_specific_ship(unit, ship) {
                    self.units
                        .iter_mut()
                        .find(|passenger| passenger.id == unit)
                        .unwrap()
                        .home_city = Some(port);
                }
            }
            assigned.extend(guard);
            assigned.extend(ship_ids.iter().copied());
            assigned.extend(troop_ids.iter().copied());
            for &ship in &ship_ids {
                self.refresh_ferry_boss(ship);
                if let Some(target) = self.map_wrap.civ3_offset(x, y, dx, dy) {
                    self.ai_ferry_goto_waypoint(ship, (x, y), target);
                }
            }
            if ship_ids.iter().all(|id| {
                self.units
                    .iter()
                    .any(|ship| ship.id == *id && (ship.x, ship.y) == (x, y))
            }) {
                for id in troop_ids {
                    if let Some(direction) = CIV3_DIRS.iter().position(|&step| step == (dx, dy)) {
                        self.try_move_unit_civ3(id, direction);
                    }
                    if self
                        .units
                        .iter()
                        .any(|unit| unit.id == id && unit.transport.is_none())
                    {
                        self.release_ai_ferry(id);
                    }
                }
            }
        }
        assigned
    }

    pub(super) fn dispatch_assault_transports(
        &mut self,
        owner: usize,
        unavailable: impl Fn(u32) -> bool,
        reserved: impl Fn(u32) -> bool,
    ) -> std::collections::HashSet<u32> {
        let mut assigned = std::collections::HashSet::new();
        if owner == BARBARIANS {
            return assigned;
        }
        let labels = self.continent_labels();
        let boats: Vec<_> = self
            .units
            .iter()
            .filter(|boat| {
                boat.owner == owner
                    && matches!(
                        boat.kind,
                        UnitKind::Trireme
                            | UnitKind::Caravel
                            | UnitKind::Galleon
                            | UnitKind::Transport
                    )
                    && self.cargo_count(boat.id) == 0
                    && !unavailable(boat.id)
                    && self.military_ferry_ready(boat.id)
            })
            .cloned()
            .collect();
        struct Pickup {
            cost: i32,
            port: u32,
            position: (i32, i32),
            guard: u32,
            troops: Vec<u32>,
        }
        for boat in boats {
            let mut best: Option<Pickup> = None;
            for port in self.cities.iter().filter(|port| {
                port.owner == owner
                    && self.naval_city_access(port, boat.kind)
                    && self.city_threats(port).next().is_none()
            }) {
                let position = (port.x, port.y);
                let Some((_, pickup_cost)) =
                    self.search_route(&boat, port.x, port.y, false, i32::MAX)
                else {
                    continue;
                };
                let mut troops: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == owner
                            && (unit.x, unit.y) == position
                            && unit.kind.is_military()
                            && !unit.kind.is_aircraft()
                            && !unit.kind.is_naval()
                            && unit.transport.is_none()
                            && !assigned.contains(&unit.id)
                            && !reserved(unit.id)
                    })
                    .collect();
                if troops.len() <= 1 {
                    continue;
                }
                troops.sort_by_key(|unit| {
                    (
                        std::cmp::Reverse(unit.kind.stats().defense * unit.hp),
                        unit.id,
                    )
                });
                let guard = troops.remove(0).id;
                troops.retain(|unit| boat.kind.can_carry(unit.kind));
                troops.truncate(boat.kind.transport_capacity());
                if troops.is_empty() {
                    continue;
                }
                let mut staged = boat.clone();
                (staged.x, staged.y) = position;
                let allowance = self.unit_move_rate(&boat).max(1);
                let pickup_turns =
                    ((pickup_cost - boat.moves_left).max(0) + allowance - 1) / allowance;
                staged.moves_left =
                    (boat.moves_left + pickup_turns * allowance - pickup_cost).max(0);
                let origin = labels[self.map_wrap.index(port.x, port.y)];
                let Some((distance, _, _, _)) =
                    self.raider_landing(&staged, Some((&labels, origin)), Some(&troops), &[])
                else {
                    continue;
                };
                let cost = pickup_turns + distance;
                if best.as_ref().is_none_or(|previous| cost < previous.cost) {
                    best = Some(Pickup {
                        cost,
                        port: port.id,
                        position,
                        guard,
                        troops: troops
                            .into_iter()
                            .take(boat.kind.transport_capacity())
                            .map(|unit| unit.id)
                            .collect(),
                    });
                }
            }
            let Some(plan) = best else {
                continue;
            };
            assigned.insert(boat.id);
            assigned.insert(plan.guard);
            assigned.extend(plan.troops.iter().copied());
            self.unit_goto_tiles.insert(boat.id, plan.position);
            self.ai_goto_unit(boat.id, plan.position, GotoTask::None);
            if let Some(staged) = self
                .units
                .iter()
                .find(|unit| unit.id == boat.id && (unit.x, unit.y) == plan.position)
                .cloned()
            {
                for id in plan.troops {
                    if self.board_specific_ship(id, boat.id) {
                        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
                        if unit.home_city.is_none() {
                            unit.home_city = Some(plan.port);
                        }
                    }
                }
                self.run_raider_transport(&staged);
            }
        }
        assigned
    }

    pub(in crate::sim) fn run_ai_ferry_after_handoff(&mut self, boat: u32) {
        if let Some(ship) = self.unit_by_id(boat).cloned() {
            self.run_raider_transport(&ship);
        }
    }

    pub(super) fn run_raider_transport(&mut self, boat: &Unit) {
        self.refresh_ferry_boss(boat.id);
        if !self.military_ferry_ready(boat.id) {
            return;
        }
        let cargo: Vec<_> = self
            .cargo_units_ordered(boat.id)
            .map(|unit| unit.id)
            .collect();
        if cargo.is_empty() {
            if boat.owner == BARBARIANS {
                self.wander_unit(boat.id);
            } else {
                self.run_idle_ai_ferry(boat.id);
            }
            return;
        }

        if let Some((direction, (dx, dy))) =
            CIV3_DIRS.into_iter().enumerate().find(|&(_, (dx, dy))| {
                let location = self.map_wrap.civ3_offset(boat.x, boat.y, dx, dy);
                self.cities.iter().any(|city| {
                    Some((city.x, city.y)) == location
                        && self.at_war(city.owner, boat.owner)
                        && (boat.owner == BARBARIANS
                            || cargo.iter().any(|id| {
                                self.units
                                    .iter()
                                    .find(|unit| unit.id == *id && unit.kind.has_marine_attack())
                                    .is_some_and(|marine| {
                                        self.assault_capture_chance(city, &[marine]) >= 0.5
                                    })
                            }))
                })
            })
        {
            for &id in &cargo {
                if self.units.iter().any(|unit| {
                    unit.id == id
                        && unit.kind.has_marine_attack()
                        && (boat.owner == BARBARIANS
                            || self.cities.iter().any(|city| {
                                let target = self.map_wrap.civ3_offset(boat.x, boat.y, dx, dy);
                                Some((city.x, city.y)) == target
                                    && self.assault_capture_chance(city, &[unit]) >= 0.5
                            }))
                }) && self.try_move_unit_civ3(id, direction) != MoveResult::Blocked
                {
                    if self
                        .units
                        .iter()
                        .any(|unit| unit.id == id && unit.transport.is_none())
                    {
                        self.release_ai_ferry(id);
                    }
                    return;
                }
            }
        }

        let labels = self.continent_labels();
        let origin = cargo.iter().find_map(|id| {
            let home = self.unit_by_id(*id)?.home_city?;
            let city = self.city_by_id(home)?;
            Some(labels[self.map_wrap.index(city.x, city.y)])
        });
        let troops: Vec<_> = self
            .units
            .iter()
            .filter(|unit| cargo.contains(&unit.id))
            .collect();
        let landing = self.raider_landing(
            boat,
            origin.map(|region| (labels.as_slice(), region)),
            (boat.owner != BARBARIANS).then_some(troops.as_slice()),
            &[],
        );
        let Some((_, x, y, (dx, dy))) = landing else {
            if boat.owner == BARBARIANS {
                self.wander_unit(boat.id);
            }
            return;
        };
        if (boat.x, boat.y) != (x, y)
            && let Some(target) = self.map_wrap.civ3_offset(x, y, dx, dy)
        {
            self.ai_ferry_goto_waypoint(boat.id, (x, y), target);
        }
        if self
            .units
            .iter()
            .any(|unit| unit.id == boat.id && (unit.x, unit.y) == (x, y))
        {
            for id in cargo {
                if let Some(direction) = CIV3_DIRS.iter().position(|&step| step == (dx, dy)) {
                    self.try_move_unit_civ3(id, direction);
                }
                if self
                    .units
                    .iter()
                    .any(|unit| unit.id == id && unit.transport.is_none())
                {
                    self.release_ai_ferry(id);
                }
            }
        }
    }
}
