//! Typed units preparation for deterministic fixtures.

use super::WorldBuilder;
#[cfg(test)]
use crate::sim::Simulation;
use crate::sim::UnitKind;

impl WorldBuilder<'_> {
    pub(crate) fn unit(&mut self, owner: usize, kind: UnitKind, position: (i32, i32)) -> u32 {
        assert!(
            owner < self.game.civilization_count || owner == crate::sim::BARBARIANS,
            "inactive scenario unit owner {owner}"
        );
        self.game.spawn_unit(owner, kind, position.0, position.1)
    }

    pub(crate) fn unit_experience_rank(&mut self, id: u32, rank: usize) {
        let unit = self.unit_mut(id);
        assert!(rank < crate::rules::native::experience().levels.len());
        unit.veteran = rank;
        unit.hp = unit.max_hit_points();
    }

    pub(crate) fn unit_hit_points(&mut self, id: u32, hit_points: i32) {
        let unit = self.unit_mut(id);
        assert!((1..=unit.max_hit_points()).contains(&hit_points));
        unit.hp = hit_points;
    }

    pub(super) fn unit_mut(&mut self, id: u32) -> &mut crate::sim::Unit {
        let index = self
            .game
            .unit_index_by_id(id)
            .expect("fixture unit must exist");
        &mut self.game.units[index]
    }

    /// Seed the previous turn's position for AI policies that track movement.
    pub(crate) fn previous_unit_position(&mut self, id: u32, position: (i32, i32)) {
        assert!(
            self.game.unit_by_id(id).is_some(),
            "fixture unit must exist"
        );
        self.game.previous_naval_positions.insert(id, position);
    }

    pub(crate) fn unit_home_city(&mut self, unit_id: u32, home: u32) {
        assert!(self.game.city_by_id(home).is_some());
        self.unit_mut(unit_id).home_city = Some(home);
    }

    pub(crate) fn unit_moved(&mut self, unit_id: u32, moved: bool) {
        self.unit_mut(unit_id).moved = moved;
    }

    pub(crate) fn clear_units(&mut self) {
        self.game.units.clear();
    }

    pub(crate) fn clear_units_and_cities(&mut self) {
        self.game.units.clear();
        self.game.cities.clear();
    }

    pub(crate) fn legacy_combat_rank(&mut self, id: u32, rank: usize) {
        self.unit_mut(id).c2c3_veteran = rank;
    }
}

#[cfg(test)]
pub(crate) fn give_unit_full_movement(sim: &mut Simulation, id: u32, kind: UnitKind) {
    sim.units
        .iter_mut()
        .find(|unit| unit.id == id)
        .unwrap()
        .moves_left = kind.stats().moves * crate::sim::movement_rules::campaign_move_fragments();
}
