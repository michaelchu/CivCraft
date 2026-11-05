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
        let ranks = match self.kind {
            UnitKind::Engineer => ["beginner", "seasoned", "senior", "expert"],
            UnitKind::Spy => ["informant", "handler", "agent", "spymaster"],
            UnitKind::Diplomat => ["attaché", "secretary", "envoy", "ambassador"],
            _ => ["green", "veteran", "hardened", "elite"],
        };
        ranks[self.veteran]
    }

    pub(super) fn veteran_power(&self) -> i32 {
        if self.kind.is_diplomatic() {
            [100, 105, 110, 115][self.veteran]
        } else {
            [100, 150, 175, 200][self.veteran]
        }
    }
}

impl Simulation {
    pub(super) fn promote_unit(&mut self, id: u32, modifier: i32) {
        let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
            return;
        };
        let unit = &self.units[index];
        let base = [50, 33, 20, 0][unit.veteran];
        if base == 0
            || matches!(
                unit.kind,
                UnitKind::Worker | UnitKind::Settlers | UnitKind::Migrants
            )
        {
            return;
        }
        let tribal = self.players[unit.owner].government == Government::Tribal
            && unit.kind.is_military()
            && !unit.kind.is_aircraft()
            && !unit.kind.is_naval()
            && !matches!(
                unit.kind,
                UnitKind::Chariot | UnitKind::Catapult | UnitKind::Cannon
            );
        let magellan = unit.kind.earns_naval_veterancy()
            && self.has_active_wonder(unit.owner, Building::MAGELLANS_EXPEDITION);
        self.try_promote(
            index,
            percent_of(base, modifier + 50 * i32::from(tribal || magellan)),
        );
    }

    pub(super) fn promote_worker(&mut self, id: u32) {
        if let Some(index) = self.units.iter().position(|unit| unit.id == id) {
            let chance = [3, 2, 1, 0][self.units[index].veteran];
            self.try_promote(index, chance);
        }
    }

    fn try_promote(&mut self, index: usize, chance: i32) {
        if chance > 0 && self.rand_below(100) < chance {
            let unit = &mut self.units[index];
            unit.veteran += 1;
            if unit.owner == HUMAN {
                let message = format!(
                    "Your {} reached {} rank.",
                    unit.kind.name(),
                    unit.rank_name()
                );
                self.add_event(message);
            }
        }
    }

    pub(super) fn active_barracks(&self, city: &City) -> bool {
        let modern = city.has_building(Building::BARRACKS_III);
        let industrial = city.has_building(Building::BARRACKS_II);
        modern
            || (industrial && !self.has_tech(city.owner, Tech::MOBILE_WARFARE))
            || (city.has_building(Building::BARRACKS)
                && !industrial
                && !self.has_tech(city.owner, Tech::GUNPOWDER))
    }

    pub(super) fn trained_rank(&self, city: &City, kind: UnitKind) -> usize {
        if kind.is_diplomatic() {
            return usize::from(matches!(
                self.players[city.owner].government,
                Government::Communism | Government::Federation
            ));
        }
        if kind.is_naval() {
            return usize::from(
                kind.earns_naval_veterancy()
                    && (city.has_building(Building::PORT_FACILITY)
                        || self.has_active_wonder(city.owner, Building::LIGHTHOUSE)),
            );
        }
        if kind.is_aircraft() {
            return usize::from(kind != UnitKind::Nuclear && city.has_building(Building::AIRPORT));
        }
        if kind.is_military() {
            return usize::from(self.active_barracks(city))
                + usize::from(self.has_active_wonder(city.owner, Building::SUN_TZUS_WAR_ACADEMY));
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engineer_work_promotion_preserves_progress_and_changes_the_next_turns_rate() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let engineer = game.terrain_work_playtest_setup();
        assert!(game.apply(Command::Work(engineer, WorkKind::Transform)));
        assert_eq!(game.units.last().unwrap().work_turns_left(), 12);
        game.work_promotion_playtest_tick();
        let unit = game.units.last().unwrap();
        assert_eq!(unit.veteran, 1);
        assert_eq!(unit.work_left, 220);
        assert_eq!(unit.work_turns_left(), 8);
        game.update_activities_and_restore();
        assert_eq!(game.units.last().unwrap().work_left, 190);
    }

    #[test]
    fn promotion_and_training_change_combat_power_and_diplomatic_rank() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0]
            .buildings
            .extend([Building::BARRACKS, Building::SUN_TZUS_WAR_ACADEMY]);
        assert_eq!(game.trained_rank(&game.cities[0], UnitKind::Warrior), 2);
        game.players[HUMAN].government = Government::Communism;
        assert_eq!(game.trained_rank(&game.cities[0], UnitKind::Spy), 1);
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        game.rng = 4;
        game.promote_unit(id, 100);
        let veteran = game.units.last().unwrap();
        assert_eq!(veteran.rank_name(), "veteran");
        assert_eq!(Simulation::attack_power(veteran), 15);
        let mut diplomat = veteran.clone();
        diplomat.kind = UnitKind::Diplomat;
        assert_eq!(diplomat.rank_name(), "secretary");
        assert_eq!(diplomat.veteran_power(), 105);
    }
}
