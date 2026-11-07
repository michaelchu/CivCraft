use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum FerryState {
    Available,
    Assigned(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum FerryRequest {
    Wanted,
    Assigned(u32),
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct BoatRecord {
    pub(super) owner: usize,
    pub(super) state: FerryState,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct PassengerRecord {
    pub(super) owner: usize,
    pub(super) request: FerryRequest,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct FerryRegistry {
    pub(super) boats: BTreeMap<u32, BoatRecord>,
    pub(super) passengers: BTreeMap<u32, PassengerRecord>,
}

impl FerryRegistry {
    pub(super) fn forget(&mut self, id: u32) {
        self.boats.remove(&id);
        self.release(id);
        for passenger in self.passengers.values_mut() {
            if passenger.request == FerryRequest::Assigned(id) {
                passenger.request = FerryRequest::Wanted;
            }
        }
    }

    fn release(&mut self, id: u32) {
        if let Some(record) = self.passengers.remove(&id)
            && let FerryRequest::Assigned(boat) = record.request
            && let Some(ferry) = self.boats.get_mut(&boat)
            && ferry.state == FerryState::Assigned(id)
        {
            ferry.state = FerryState::Available;
        }
    }

    pub(super) fn valid(&self, units: &[Unit]) -> bool {
        self.boats.iter().all(|(&id, boat)| {
            units.iter().any(|unit| {
                unit.id == id && unit.owner == boat.owner && Simulation::is_ai_ferry(unit.kind)
            }) && match boat.state {
                FerryState::Available => true,
                FerryState::Assigned(id) => units
                    .iter()
                    .any(|unit| unit.id == id && unit.owner == boat.owner),
            }
        }) && self.passengers.iter().all(|(&id, passenger)| {
            units
                .iter()
                .any(|unit| unit.id == id && unit.owner == passenger.owner)
                && match passenger.request {
                    FerryRequest::Wanted => true,
                    FerryRequest::Assigned(boat) => self
                        .boats
                        .get(&boat)
                        .is_some_and(|boat| boat.owner == passenger.owner),
                }
        })
    }
}

impl Simulation {
    pub(super) fn is_ai_ferry(kind: UnitKind) -> bool {
        // daiunit.c classifies transports by potential non-suicide cargo.
        // These three types represent the supported big-land, small-land,
        // and fueled aircraft cargo classes (including carriers/helicopters).
        (kind.is_naval() || kind.is_aircraft())
            && [UnitKind::Settlers, UnitKind::Warrior, UnitKind::Fighter]
                .into_iter()
                .any(|cargo| kind.can_carry(cargo))
    }

    pub(super) fn refresh_ai_ferries(&mut self) {
        let invalid: Vec<_> = self
            .ai_ferries
            .boats
            .iter()
            .filter_map(|(&id, boat)| {
                (!self.units.iter().any(|unit| {
                    unit.id == id && unit.owner == boat.owner && Self::is_ai_ferry(unit.kind)
                }))
                .then_some(id)
            })
            .chain(
                self.ai_ferries
                    .passengers
                    .iter()
                    .filter_map(|(&id, passenger)| {
                        (!self
                            .units
                            .iter()
                            .any(|unit| unit.id == id && unit.owner == passenger.owner))
                        .then_some(id)
                    }),
            )
            .collect();
        for id in invalid {
            self.ai_ferries.forget(id);
        }
        for unit in &self.units {
            if Self::is_ai_ferry(unit.kind) {
                self.ai_ferries.boats.entry(unit.id).or_insert(BoatRecord {
                    owner: unit.owner,
                    state: FerryState::Available,
                });
            }
        }
    }

    pub(super) fn ferry_available_count(&self, owner: usize) -> usize {
        self.units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && Self::is_ai_ferry(unit.kind)
                    && self
                        .ai_ferries
                        .boats
                        .get(&unit.id)
                        .is_none_or(|boat| boat.state == FerryState::Available)
            })
            .count()
    }

    pub(super) fn ferry_can_take_unit(&self, ship: &Unit, unit: &Unit) -> bool {
        if !Self::is_ai_ferry(ship.kind)
            || ship.owner != unit.owner
            || !ship.kind.can_carry(unit.kind)
        {
            return false;
        }
        self.ai_ferries.boats.get(&ship.id).is_none_or(|record| {
            record.state == FerryState::Available || record.state == FerryState::Assigned(unit.id)
        })
    }

    pub(super) fn ferry_can_take(&self, boat: u32, passenger: u32) -> bool {
        let Some(ship) = self.unit_by_id(boat) else {
            return false;
        };
        let Some(unit) = self.unit_by_id(passenger) else {
            return false;
        };
        self.ferry_can_take_unit(ship, unit)
    }

    pub(super) fn ferry_boss(&self, boat: u32) -> Option<u32> {
        self.ai_ferries
            .boats
            .get(&boat)
            .and_then(|record| match record.state {
                FerryState::Assigned(id) => Some(id),
                FerryState::Available => None,
            })
    }

    pub(super) fn request_ai_ferry(&mut self, id: u32) {
        if let Some(owner) = self.unit_by_id(id).map(|unit| unit.owner) {
            self.ai_ferries
                .passengers
                .entry(id)
                .or_insert(PassengerRecord {
                    owner,
                    request: FerryRequest::Wanted,
                });
        }
    }

    pub(super) fn assign_ai_ferry(&mut self, boat: u32, passenger: u32) -> bool {
        if !self.ferry_can_take(boat, passenger) {
            return false;
        }
        let owner = self
            .units
            .iter()
            .find(|unit| unit.id == boat)
            .unwrap()
            .owner;
        self.ai_ferries.release(passenger);
        self.ai_ferries.boats.insert(
            boat,
            BoatRecord {
                owner,
                state: FerryState::Assigned(passenger),
            },
        );
        self.ai_ferries.passengers.insert(
            passenger,
            PassengerRecord {
                owner,
                request: FerryRequest::Assigned(boat),
            },
        );
        true
    }

    pub(super) fn release_ai_ferry(&mut self, passenger: u32) {
        self.ai_ferries.release(passenger);
    }

    pub(super) fn refresh_ferry_boss(&mut self, boat: u32) {
        let Some(ship) = self
            .units
            .iter()
            .find(|unit| unit.id == boat && Self::is_ai_ferry(unit.kind))
        else {
            return;
        };
        let previous = self
            .ai_ferries
            .boats
            .get(&boat)
            .and_then(|record| match record.state {
                FerryState::Available => None,
                FerryState::Assigned(id) => Some(id),
            });
        if previous.is_some_and(|id| {
            self.units.iter().any(|unit| {
                unit.id == id
                    && unit.owner == ship.owner
                    && self.map_distance(unit.x, unit.y, ship.x, ship.y) <= 1
            })
        }) {
            return;
        }
        let mut last_escort = None;
        let candidate = self
            .cargo_units_ordered(boat)
            .filter(|unit| unit.owner == ship.owner)
            .find_map(|unit| {
                if self.ai_guard_targets.contains_key(&unit.id) {
                    last_escort = Some(unit.id);
                    None
                } else {
                    Some(unit.id)
                }
            })
            .or(last_escort);
        if let Some(candidate) = candidate {
            // daiferry.c temporarily abandons a distant boss, appoints cargo,
            // and otherwise restores the old boss until it is released.
            let owner = ship.owner;
            self.ai_ferries.boats.insert(
                boat,
                BoatRecord {
                    owner,
                    state: FerryState::Available,
                },
            );
            self.assign_ai_ferry(boat, candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ferry_classification_includes_air_transports_but_not_missile_only_platforms() {
        for kind in [UnitKind::Transport, UnitKind::Carrier, UnitKind::Helicopter] {
            assert!(Simulation::is_ai_ferry(kind));
        }
        assert!(!Simulation::is_ai_ferry(UnitKind::Submarine));
        assert!(!Simulation::is_ai_ferry(UnitKind::Destroyer));
    }

    #[test]
    fn boat_assignment_availability_is_independent_of_cargo_and_released_on_death() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_ferry_registry_playtest());
    }

    #[test]
    fn a_displaced_boss_can_rebook_another_available_ferry() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.ai_ferries = Default::default();
        let first_boat = game.spawn_unit(1, UnitKind::Transport, 10, 24);
        let second_boat = game.spawn_unit(1, UnitKind::Transport, 18, 24);
        let waiting = game.spawn_unit(1, UnitKind::Settlers, 18, 24);
        let cargo = game.spawn_unit(1, UnitKind::Freight, 10, 24);
        assert!(game.assign_ai_ferry(first_boat, waiting));
        game.units
            .iter_mut()
            .find(|unit| unit.id == cargo)
            .unwrap()
            .transport = Some(first_boat);
        game.refresh_ferry_boss(first_boat);
        assert_eq!(game.ferry_boss(first_boat), Some(cargo));
        assert!(!game.ferry_can_take(first_boat, waiting));
        assert!(game.ferry_can_take(second_boat, waiting));
        assert!(game.assign_ai_ferry(second_boat, waiting));
        assert_eq!(game.ferry_boss(first_boat), Some(cargo));
        assert_eq!(game.ferry_boss(second_boat), Some(waiting));
        assert!(game.ai_ferries.valid(&game.units));
    }

    #[test]
    fn loss_of_a_reserved_boat_returns_its_waiting_passenger_to_requests() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.ai_ferries = Default::default();
        let boat = game.spawn_unit(1, UnitKind::Transport, 10, 24);
        let passenger = game.spawn_unit(1, UnitKind::Settlers, 12, 24);
        assert!(game.assign_ai_ferry(boat, passenger));
        game.remove_units_with_cargo(vec![boat]);
        assert_eq!(
            game.ai_ferries.passengers[&passenger].request,
            FerryRequest::Wanted
        );
        assert!(game.ai_ferries.valid(&game.units));
    }

    #[test]
    fn cargo_boss_uses_first_non_escort_or_last_escort_in_boarding_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_cargo_order_playtest());
        let boat = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Carrier)
            .unwrap()
            .id;
        let first = game.cargo_units_ordered(boat).next().unwrap().id;
        assert!(game.board_or_deboard(first));
        assert!(game.board_or_deboard(first));
        let ids: Vec<_> = game.cargo_units_ordered(boat).map(|unit| unit.id).collect();
        game.ai_ferries.boats.get_mut(&boat).unwrap().state = FerryState::Available;
        game.refresh_ferry_boss(boat);
        assert_eq!(game.ferry_boss(boat), Some(ids[0]));
        game.ai_guard_targets
            .insert(ids[0], guards::GuardTarget::Unit(boat));
        game.ai_ferries.boats.get_mut(&boat).unwrap().state = FerryState::Available;
        game.refresh_ferry_boss(boat);
        assert_eq!(game.ferry_boss(boat), Some(ids[1]));
        game.ai_guard_targets
            .insert(ids[1], guards::GuardTarget::Unit(boat));
        game.ai_ferries.boats.get_mut(&boat).unwrap().state = FerryState::Available;
        game.refresh_ferry_boss(boat);
        assert_eq!(game.ferry_boss(boat), Some(*ids.last().unwrap()));
    }
}
