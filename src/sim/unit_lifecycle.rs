//! Unit creation, loss, capture, and state cleanup.

use super::*;

impl Simulation {
    pub(in crate::sim) fn prune_unit_ai_state(&mut self) {
        let ids: std::collections::BTreeSet<_> = self
            .ai_ground_targets
            .keys()
            .chain(self.ai_settlement_targets.keys())
            .chain(self.ai_ferries.boats.keys())
            .chain(self.ai_ferries.passengers.keys())
            .chain(self.ai_guard_targets.keys())
            .chain(self.ai_unit_guards.keys())
            .chain(self.unit_goto_tiles.keys())
            .chain(self.ai_worker_tasks.iter())
            .copied()
            .filter(|id| !self.units.iter().any(|unit| unit.id == *id))
            .collect();
        for id in ids {
            self.discard_unit_ai(id);
        }
        self.refresh_ai_ferries();
        self.refresh_ai_guards();
        self.sync_list_order();
    }

    pub(in crate::sim) fn discard_unit_ai(&mut self, id: u32) {
        self.list_order.forget_unit(id);
        self.unit_goto_tiles.remove(&id);
        self.ai_worker_tasks.remove(&id);
        self.forget_ai_guard_unit(id);
        self.ai_ferries.forget(id);
        self.ai_ground_targets.remove(&id);
        self.ai_settlement_targets.remove(&id);
    }

    pub(in crate::sim) fn spawn_unit(
        &mut self,
        owner: usize,
        kind: UnitKind,
        x: i32,
        y: i32,
    ) -> u32 {
        let id = self.next_unit_id;
        self.next_unit_id += 1;
        self.units.push(self.make_unit(id, owner, kind, x, y));
        self.list_order.unit(self.units.last().unwrap());
        if Self::is_ai_ferry(kind) {
            self.ai_ferries.boats.insert(
                id,
                ferries::BoatRecord {
                    owner,
                    state: ferries::FerryState::Available,
                },
            );
        }
        self.refresh_diplomatic_contact_at(owner, x, y);
        id
    }

    pub(in crate::sim) fn make_unit(
        &self,
        id: u32,
        owner: usize,
        kind: UnitKind,
        x: i32,
        y: i32,
    ) -> Unit {
        let mut unit = Unit::new(id, owner, kind, x, y);
        unit.moves_left = self.unit_move_rate(&unit);
        unit
    }
    pub(crate) fn can_disband(&self, unit: &Unit, city: Option<&City>) -> bool {
        city.is_none_or(|city| {
            self.allied(unit.owner, city.owner)
                && self.map_wrap.civ3_distance(unit.x, unit.y, city.x, city.y) <= 1
        })
    }

    pub(in crate::sim) fn disband_unit(&mut self, id: u32, city_id: Option<u32>) -> bool {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return false;
        };
        let city = city_id.and_then(|id| self.city_index_by_id(id));
        if city_id.is_some() != city.is_some()
            || !self.can_disband(&unit, city.map(|index| &self.cities[index]))
        {
            return false;
        }
        if let Some(index) = city {
            let city = &mut self.cities[index];
            let cost = unit.kind.stats().cost;
            let shields = cost - cost / 2;
            city.shields += shields;
            city.protected_shields += shields;
            if let Some((_, original_shields)) = &mut city.production_before_switch {
                *original_shields += shields;
            }
            let name = city.name.clone();
            self.add_event(format!(
                "Disbanded {} for {shields} shields in {name}.",
                unit.kind.name()
            ));
        } else {
            self.add_event(format!("Disbanded {}.", unit.kind.name()));
        }

        self.remove_units_with_cargo(vec![id]);
        self.refresh_visibility();
        self.check_game_over();
        true
    }
}
