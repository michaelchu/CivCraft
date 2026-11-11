//! Conquests air-unit Rebase mission validation and execution.

use super::*;

impl Simulation {
    pub(crate) fn can_begin_rebase(&self, unit: &Unit) -> bool {
        unit.owner == HUMAN
            && unit.kind.is_aircraft()
            && unit.kind.can_rebase()
            && unit.kind.fuel_capacity() > 0
            && !unit.kind.is_missile()
            && unit.transport.is_none()
            && unit.moves_left > 0
            && !unit.moved
    }

    pub(crate) fn can_rebase_to(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_rebase(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || (unit.x, unit.y) == (x, y)
            || self.map_wrap.civ3_distance(unit.x, unit.y, x, y)
                > super::super::air_mission_rules::air_mission_rules()
                    .rebase_operational_range_multiplier
                    * unit.kind.operational_range()
            || self
                .units
                .iter()
                .any(|other| (other.x, other.y) == (x, y) && !self.allied(unit.owner, other.owner))
        {
            return false;
        }

        self.cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && city.owner == unit.owner)
            || (self.tile(x, y).base == Some(Base::Airbase)
                && self.tile(x, y).owner == Some(unit.owner))
            || self.units.iter().any(|carrier| {
                (carrier.x, carrier.y) == (x, y)
                    && self.allied(unit.owner, carrier.owner)
                    && carrier.kind.can_carry(unit.kind)
                    && self.cargo_count(carrier.id) < carrier.kind.transport_capacity()
            })
    }

    pub(crate) fn rebase_aircraft(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        if !self.can_rebase_to(&unit, x, y) {
            return false;
        }
        let carrier = self
            .units
            .iter()
            .find(|carrier| {
                (carrier.x, carrier.y) == (x, y)
                    && self.allied(unit.owner, carrier.owner)
                    && carrier.kind.can_carry(unit.kind)
                    && self.cargo_count(carrier.id) < carrier.kind.transport_capacity()
            })
            .map(|carrier| carrier.id);
        let index = self.unit_index_by_id(id).unwrap();
        self.set_transport(index, carrier);
        let actor = &mut self.units[index];
        (actor.x, actor.y) = (x, y);
        actor.moves_left = 0;
        actor.moved = true;
        actor.state = UnitState::Idle;
        self.refresh_visibility();
        true
    }
}
