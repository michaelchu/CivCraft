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
        if unit.kind.no_veteran() || unit.veteran + 1 >= unit.kind.max_veteran_levels() {
            return;
        }
        let base = [50, 33, 20, 0][unit.veteran];
        let tribal = self.players[unit.owner].government == Government::Tribal
            && unit.kind.unit_class() == super::unit_catalog::UnitClass::Land
            && !unit.kind.is_civilian();
        let magellan = unit.kind.earns_naval_veterancy()
            && self.has_active_wonder(unit.owner, Building::MAGELLANS_EXPEDITION);
        self.try_promote(
            index,
            percent_of(base, modifier + 50 * i32::from(tribal || magellan)),
        );
    }

    pub(super) fn promote_worker(&mut self, id: u32) {
        let Some(index) = self.units.iter().position(|unit| unit.id == id) else {
            return;
        };
        let unit = &self.units[index];
        // Settlers/Migrants/Workers have only one veteran level. Non-Workers
        // actors (including military Fortress builders) return before RNG.
        if unit.kind.no_veteran()
            || unit.veteran + 1 >= unit.kind.max_veteran_levels()
            || !unit.kind.is_worker()
        {
            return;
        }
        let chance = if unit.kind == UnitKind::Engineer {
            [3, 2, 1, 0][unit.veteran]
        } else {
            0
        };
        // Transport's four-level system has zero work chances, but native
        // maybe_become_veteran_real still consumes its 0%-success roll.
        self.try_promote(index, chance);
    }

    fn try_promote(&mut self, index: usize, chance: i32) {
        if self.rand_below(100) < chance {
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
        // CivCraft's barracks are permanent infrastructure: one Barracks
        // continues training veterans as technology advances. The legacy II/III
        // catalog entries remain recognized for cities loaded from older saves.
        [
            Building::BARRACKS,
            Building::BARRACKS_II,
            Building::BARRACKS_III,
        ]
        .into_iter()
        .any(|building| city.has_building(building))
    }

    pub(super) fn trained_rank(&self, city: &City, kind: UnitKind) -> usize {
        if kind.no_veteran() || kind.max_veteran_levels() <= 1 {
            return 0;
        }
        let rank = if kind.is_diplomatic() {
            usize::from(matches!(
                self.players[city.owner].government,
                Government::Communism | Government::Federation
            ))
        } else if kind.is_naval() {
            usize::from(
                !kind.is_civilian()
                    && (city.has_building(Building::PORT_FACILITY)
                        || self.has_active_wonder(city.owner, Building::LIGHTHOUSE)),
            )
        } else if kind.is_aircraft() {
            usize::from(!kind.is_civilian() && city.has_building(Building::AIRPORT))
        } else if kind.unit_class().has_barracks() && !kind.is_civilian() {
            usize::from(self.active_barracks(city))
                + usize::from(self.has_active_wonder(city.owner, Building::SUN_TZUS_WAR_ACADEMY))
        } else {
            0
        };
        rank.min(kind.max_veteran_levels() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_roster_promotion_uses_native_class_flags_and_level_limits() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.players[HUMAN].government = Government::Tribal;
        for kind in UnitKind::ALL.iter().copied() {
            let id = game.spawn_unit(HUMAN, kind, 10, 24);
            game.set_random_seed(1); // Native first 3 passes Tribal 25%, while 0% still consumes a roll.
            let before = game.random_snapshot();
            let eligible = !kind.no_veteran() && kind.max_veteran_levels() > 1;
            if eligible {
                game.rand_below(100);
            }
            let expected_rng = game.random_snapshot();
            game.restore_random(before);
            game.promote_unit(id, 0);
            let tribal_land = kind.unit_class() == super::super::unit_catalog::UnitClass::Land
                && !kind.is_civilian();
            let unit = game.units.iter().find(|unit| unit.id == id).unwrap();
            assert_eq!(
                unit.veteran,
                usize::from(eligible && tribal_land),
                "{kind:?}"
            );
            assert_eq!(game.random_snapshot(), expected_rng, "{kind:?}");
        }
    }

    #[test]
    fn appended_roster_build_veterancy_matches_class_and_type_exclusions() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(HUMAN, 10, 24);
        game.players[HUMAN].government = Government::Communism;
        game.cities[0].buildings.extend([
            Building::BARRACKS,
            Building::SUN_TZUS_WAR_ACADEMY,
            Building::AIRPORT,
            Building::PORT_FACILITY,
        ]);
        let kind = |name| {
            UnitKind::ALL
                .iter()
                .copied()
                .find(|kind| kind.name() == name)
                .unwrap()
        };
        for name in ["Mech. Inf.", "Armor", "Artillery", "Howitzer"] {
            assert_eq!(game.trained_rank(&game.cities[0], kind(name)), 2, "{name}");
        }
        assert_eq!(game.trained_rank(&game.cities[0], kind("AWACS")), 1);
        assert_eq!(
            game.trained_rank(&game.cities[0], kind("Stealth Fighter")),
            1
        );
        assert_eq!(
            game.trained_rank(&game.cities[0], kind("Cruise Missile")),
            1
        );
        for name in ["Leader", "Barbarian Leader", "Nuclear", "Storm"] {
            assert_eq!(game.trained_rank(&game.cities[0], kind(name)), 0, "{name}");
        }
    }

    #[test]
    fn worker_promotion_draws_match_workers_flag_and_actual_veteran_levels() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for kind in [
            UnitKind::Settlers,
            UnitKind::Migrants,
            UnitKind::Worker,
            UnitKind::Engineer,
            UnitKind::Transport,
            UnitKind::Warrior,
        ] {
            let ranks: &[usize] = if matches!(
                kind,
                UnitKind::Settlers | UnitKind::Migrants | UnitKind::Worker
            ) {
                &[0] // Their native systems have only one valid veteran level.
            } else {
                &[0, 3]
            };
            for &rank in ranks {
                let id = game.spawn_unit(HUMAN, kind, 10, 24);
                game.units.last_mut().unwrap().veteran = rank;
                game.set_random_seed(2);
                let before = game.random_snapshot();
                let draws = rank < 3 && matches!(kind, UnitKind::Engineer | UnitKind::Transport);
                if draws {
                    game.rand_below(100);
                }
                let expected = game.random_snapshot();
                game.restore_random(before);
                game.promote_worker(id);
                assert_eq!(game.random_snapshot(), expected, "{kind:?} rank{rank}");
                assert_eq!(game.units.last().unwrap().veteran, rank);
            }
        }
    }

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
        game.set_random_seed(4);
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
