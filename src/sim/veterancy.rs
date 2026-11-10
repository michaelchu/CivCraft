//! Unit veteran levels and combat experience.

use super::*;

impl Unit {
    pub(super) fn activity_rate(&self) -> i32 {
        ACTIVITY_FACTOR * self.kind.stats().moves.max(1) * self.veteran_power() / 100
    }

    pub(crate) fn work_turns_left(&self) -> i32 {
        let rate = self.activity_rate();
        (self.work_left + rate - 1) / rate
    }

    pub(crate) fn rank_name(&self) -> &'static str {
        ["conscript", "regular", "veteran", "elite"][self.veteran.min(3)]
    }

    pub(super) fn veteran_power(&self) -> i32 {
        // Conquests rank changes hit points; it does not change combat odds.
        100
    }
}

impl Simulation {
    pub(super) fn promote_civ3_unit(&mut self, id: u32, defeated_owner: usize) {
        let Some(index) = self.unit_index_by_id(id) else {
            return;
        };
        let unit = &self.units[index];
        if unit.kind.conquests_id().is_none() || unit.kind.no_veteran() {
            return;
        }
        let mut chance =
            crate::sim::civ3_unit_rules::experience_rank(unit.veteran).promotion_chance_per_mille;
        if chance == 0 {
            return;
        }
        if crate::sim::civilization_start_rules::get(&self.players[unit.owner].name)
            .is_some_and(|profile| profile.is_militaristic())
        {
            chance *= 2;
        }
        if defeated_owner == BARBARIANS {
            chance /= 2;
        }
        if self.rand_below(1000) >= chance {
            return;
        }

        let unit = &mut self.units[index];
        let previous_max = unit.max_hit_points();
        unit.veteran = (unit.veteran + 1).min(3);
        let gained = unit.max_hit_points() - previous_max;
        unit.hp = (unit.hp + gained).min(unit.max_hit_points());
        if unit.owner == HUMAN {
            let message = format!(
                "Your {} reached {} rank.",
                unit.kind.name(),
                unit.rank_name()
            );
            self.add_event(message);
        }
    }

    pub(super) fn active_barracks(&self, city: &City) -> bool {
        city.has_building(Building::BARRACKS)
    }

    pub(super) fn trained_rank(&self, city: &City, kind: UnitKind) -> usize {
        let default_rank = crate::sim::civ3_unit_rules::default_experience_rank();
        if kind.no_veteran() || kind.max_veteran_levels() <= 1 {
            return default_rank;
        }
        let rank = if kind.unit_class().has_barracks() && !kind.is_civilian() {
            usize::from(
                self.active_barracks(city)
                    || self.has_active_wonder(city.owner, Building::SUN_TZUS_WAR_ACADEMY),
            )
        } else {
            0
        };
        (default_rank + rank).min(kind.max_veteran_levels() - 1)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civ3_rank_changes_hit_points_without_changing_combat_power() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0]
            .buildings
            .extend([Building::BARRACKS, Building::SUN_TZUS_WAR_ACADEMY]);
        assert_eq!(game.trained_rank(&game.cities[0], UnitKind::Warrior), 2);
        game.players[HUMAN].government = Government::Communism;
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        let regular = game.unit_by_id(id).unwrap().clone();
        let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.veteran = 2;
        unit.hp = unit.max_hit_points();
        let veteran = game.unit_by_id(id).unwrap();
        assert_eq!(veteran.rank_name(), "veteran");
        assert_eq!(veteran.max_hit_points(), 4);
        assert_eq!(veteran.hp, 4);
        assert_eq!(
            Simulation::attack_power(veteran),
            Simulation::attack_power(&regular)
        );
    }
}
