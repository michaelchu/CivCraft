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
        self.units
            .iter()
            .any(|other| (other.x, other.y) == (x, y) && self.at_war(unit.owner, other.owner))
    }

    pub(super) fn detonate_nuclear_unit(&mut self, id: u32) -> bool {
        let Some(unit) = self
            .units
            .iter()
            .find(|unit| unit.id == id && unit.kind.is_nuclear_weapon())
            .cloned()
        else {
            return false;
        };
        self.nuclear_strike(&unit, unit.x, unit.y);
        true
    }

    pub(super) fn nuclear_strike(&mut self, weapon: &Unit, x: i32, y: i32) {
        self.remove_units_with_cargo(vec![weapon.id]);
        if let Some(city) = self
            .cities
            .iter()
            .filter(|city| city.owner != weapon.owner && city.has_building(Building::SDI_DEFENSE))
            .filter(|city| (city.y - y).abs() <= 4 && self.map_distance(x, y, city.x, city.y) <= 2)
            .min_by_key(|city| self.map_distance(x, y, city.x, city.y))
        {
            let owner = city.owner;
            self.add_event(format!(
                "SDI defenses at {} intercepted a nuclear strike.",
                city.name
            ));
            self.nation_scores[owner].units_killed += 1;
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

        // The ruleset uses a squared radius of two. On a hex map that covers
        // the center and its six neighbors.
        let affected: Vec<_> = (0..self.tiles.len())
            .map(|index| self.map_wrap.position(index))
            .filter(|&(tx, ty)| (y - ty).abs() <= 2 && self.map_distance(x, y, tx, ty) <= 1)
            .collect();
        let losses: Vec<_> = self
            .units
            .iter()
            .filter(|unit| affected.contains(&(unit.x, unit.y)))
            .map(|unit| unit.id)
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
    fn nuclear_production_and_movement_follow_world_wonder_and_player_tech() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].known.push(Tech::NUCLEAR_FISSION);
        assert!(!game.unit_available(HUMAN, UnitKind::Nuclear));
        game.completed_wonders.push(Building::MANHATTAN_PROJECT);
        assert!(game.unit_available(HUMAN, UnitKind::Nuclear));
        assert_eq!(game.unit_move_allowance(HUMAN, UnitKind::Nuclear), 0);
        game.players[HUMAN].known.push(Tech::ADVANCED_FLIGHT);
        assert_eq!(
            game.unit_move_allowance(HUMAN, UnitKind::Nuclear),
            8 * MOVE_FRAGMENTS
        );
        game.players[HUMAN].known.push(Tech::ROCKETRY);
        assert_eq!(
            game.unit_move_allowance(HUMAN, UnitKind::Nuclear),
            16 * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn stationary_detonation_hits_friendly_units_but_stops_after_one_hex() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = (10, 20);
        let inside = offset_native_by_map(x, y, 1, 0);
        let outside = offset_native_by_map(x, y, 2, 0);
        let missile = game.spawn_unit(HUMAN, UnitKind::Nuclear, x, y);
        let friend = game.spawn_unit(HUMAN, UnitKind::Warrior, inside.0, inside.1);
        let safe = game.spawn_unit(HUMAN, UnitKind::Warrior, outside.0, outside.1);
        assert!(game.apply(Command::Detonate(missile)));
        assert!(
            !game
                .units
                .iter()
                .any(|unit| [missile, friend].contains(&unit.id))
        );
        assert!(game.units.iter().any(|unit| unit.id == safe));
    }
}
