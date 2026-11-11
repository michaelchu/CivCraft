//! Implements nuclear behavior within the sim subsystem.

use super::*;

impl Simulation {
    pub(super) fn manhattan_project_completed(&self) -> bool {
        self.completed_wonders
            .contains(&Building::MANHATTAN_PROJECT)
    }

    pub(super) fn nuclear_target_is_hostile(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if let Some(city) = self.city_at(x, y) {
            return self.at_war(unit.owner, city.owner);
        }
        self.tile(x, y)
            .owner
            .is_some_and(|owner| self.at_war(unit.owner, owner))
            || self
                .units
                .iter()
                .any(|other| (other.x, other.y) == (x, y) && self.at_war(unit.owner, other.owner))
    }

    pub(crate) fn can_begin_nuclear_strike(&self, unit: &Unit) -> bool {
        if unit.owner >= self.players.len()
            || !unit.kind.is_nuclear_weapon()
            || unit.moves_left <= 0
            || !self.manhattan_project_completed()
        {
            return false;
        }
        if unit.kind == UnitKind::ICBM {
            return unit.transport.is_none()
                && self
                    .city_at(unit.x, unit.y)
                    .is_some_and(|city| city.owner == unit.owner);
        }
        true
    }

    pub(super) fn run_ai_nuclear_strike(&mut self, id: u32) {
        let Some(unit) = self.unit_by_id(id).cloned() else {
            return;
        };
        if unit.owner == HUMAN || !self.can_begin_nuclear_strike(&unit) {
            return;
        }
        let city_target = self
            .cities
            .iter()
            .filter(|city| city.owner != unit.owner && self.at_war(unit.owner, city.owner))
            .filter(|city| self.can_launch_nuclear_strike_at(&unit, city.x, city.y))
            .max_by_key(|city| city.size)
            .map(|city| (city.x, city.y));
        let target = city_target.or_else(|| {
            self.units
                .iter()
                .filter(|other| {
                    other.owner != unit.owner
                        && self.at_war(unit.owner, other.owner)
                        && other.transport.is_none()
                })
                .filter(|other| self.can_launch_nuclear_strike_at(&unit, other.x, other.y))
                .max_by_key(|other| other.kind.stats().attack * 3 + other.kind.stats().defense * 2)
                .map(|other| (other.x, other.y))
        });
        if let Some((x, y)) = target {
            self.launch_nuclear_strike(id, x, y);
        }
    }

    pub(crate) fn can_launch_nuclear_strike_at(&self, unit: &Unit, x: i32, y: i32) -> bool {
        if !self.can_begin_nuclear_strike(unit)
            || !(0..self.map_wrap.width()).contains(&x)
            || !(0..self.map_wrap.height()).contains(&y)
            || !self.tile(x, y).known_by[unit.owner]
            || !self.nuclear_target_is_hostile(unit, x, y)
        {
            return false;
        }
        let within_range = unit.kind == UnitKind::ICBM
            || self.map_wrap.civ3_distance(unit.x, unit.y, x, y) <= unit.kind.bombard_range();
        within_range
    }

    pub(super) fn launch_nuclear_strike(&mut self, id: u32, x: i32, y: i32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == id).cloned() else {
            return false;
        };
        if !self.can_launch_nuclear_strike_at(&unit, x, y) {
            return false;
        }
        self.nuclear_strike(&unit, x, y);
        true
    }

    pub(super) fn nuclear_strike(&mut self, weapon: &Unit, x: i32, y: i32) {
        self.publish_visual_event(VisualEvent::Capture {
            unit: weapon.clone(),
        });
        self.remove_units_with_cargo(vec![weapon.id]);
        let target_owner = self
            .city_at(x, y)
            .map(|city| city.owner)
            .or(self.tile(x, y).owner);
        if weapon.kind == UnitKind::ICBM
            && target_owner.is_some_and(|owner| {
                owner != weapon.owner
                    && self.has_active_wonder(owner, Building::SDI_DEFENSE)
                    && self.rand_below(100) < 75
            })
        {
            self.add_event("SDI Defense intercepted the incoming ICBM.");
            return;
        }

        if let Some(owner) = self.tile(x, y).owner {
            self.record_provocation(owner, weapon.owner);
            self.record_diplomatic_incident(
                weapon.owner,
                Some(owner),
                diplomacy::DiplomaticIncident::Nuclear,
                false,
            );
        }

        // The manual describes the target square and adjacent squares. On the
        // Civ III grid this is the center and its eight neighbors.
        let affected: Vec<_> = (0..self.tiles.len())
            .map(|index| self.map_wrap.position(index))
            .filter(|&(tx, ty)| self.map_wrap.civ3_distance(x, y, tx, ty) <= 1)
            .collect();
        // The manual specifies a 50% survival chance for each unit in the
        // target or an adjacent square, including friendly units.
        let exposed: Vec<_> = self
            .units
            .iter()
            .filter(|unit| affected.contains(&(unit.x, unit.y)))
            .map(|unit| unit.id)
            .collect();
        let losses: Vec<_> = exposed
            .into_iter()
            .filter(|_| self.rand_below(2) == 0)
            .collect();
        let casualties = losses.len();
        self.nation_scores[weapon.owner].units_killed += casualties as i32;
        self.remove_units_with_cargo(losses);

        for (tx, ty) in affected {
            if let Some(index) = self.city_index_at(tx, ty) {
                let city = &mut self.cities[index];
                let loss = (49 * city.size + 50) / 100;
                city.size -= loss;
                city.remove_citizen_assignments(loss, self.map_wrap);
                self.remove_citizen_nationalities(index, loss);
                if loss > 0 {
                    self.rebuild_city_ownership_for_city_event(self.cities[index].id);
                }
            }
            if self.rand_below(2) == 1 {
                let tile = self.tile_mut(tx, ty);
                if !tile.terrain.is_water() {
                    tile.fallout = true;
                }
            }
        }
        self.refresh_visibility();
        self.check_game_over();
        self.add_event(format!(
            "A nuclear weapon detonated, destroying {casualties} units."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tactical_nuke_uses_source_technology_and_movement_rules() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].known.push(Tech::SPACE_FLIGHT);
        assert!(!game.unit_available(HUMAN, UnitKind::TacticalNuke));
        game.completed_wonders.push(Building::MANHATTAN_PROJECT);
        assert!(game.unit_available(HUMAN, UnitKind::TacticalNuke));
        assert_eq!(
            game.unit_move_allowance(HUMAN, UnitKind::TacticalNuke),
            UnitKind::TacticalNuke.stats().moves * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn stationary_detonation_uses_civ3_adjacent_squares_and_unit_survival_chance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        game.sign_pact(HUMAN, 1, Pact::War);
        game.tile_mut(x, y).owner = Some(1);
        game.tile_mut(x, y).known_by[HUMAN] = true;
        game.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let inside = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();
        let outside = game.map_wrap.civ3_offset(inside.0, inside.1, 2, 0).unwrap();
        let missile = game.spawn_unit(HUMAN, UnitKind::TacticalNuke, x, y);
        let friend = game.spawn_unit(HUMAN, UnitKind::Warrior, inside.0, inside.1);
        let safe = game.spawn_unit(HUMAN, UnitKind::Warrior, outside.0, outside.1);
        game.enable_visual_events();
        game.native_random = None;
        game.rng = 2; // First xorshift draw is zero: the unit does not survive.
        assert!(game.apply(Command::NuclearStrike(missile, x, y)));
        assert!(matches!(
            game.take_visual_events().front(),
            Some(VisualEvent::Capture { unit })
                if unit.id == missile && unit.kind == UnitKind::TacticalNuke
        ));
        assert!(
            !game
                .units
                .iter()
                .any(|unit| [missile, friend].contains(&unit.id))
        );
        assert!(game.units.iter().any(|unit| unit.id == safe));

        let mut survive = Simulation::new(crate::mapgen::generate(42));
        survive.sign_pact(HUMAN, 1, Pact::War);
        survive.tile_mut(x, y).owner = Some(1);
        survive.tile_mut(x, y).known_by[HUMAN] = true;
        survive.completed_wonders.push(Building::MANHATTAN_PROJECT);
        let missile = survive.spawn_unit(HUMAN, UnitKind::TacticalNuke, x, y);
        let friend = survive.spawn_unit(HUMAN, UnitKind::Warrior, inside.0, inside.1);
        survive.native_random = None;
        survive.rng = 1; // First xorshift draw is one: the unit survives.
        assert!(survive.apply(Command::NuclearStrike(missile, x, y)));
        assert!(survive.units.iter().any(|unit| unit.id == friend));
    }
}
