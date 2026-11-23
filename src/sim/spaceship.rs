//! Civ III spaceship: ten distinct components, each built once.
//!
//! The Conquests Civilopedia lists ten components that "may be built in any
//! order" and "must be built prior to launch". The owner confirmed on
//! 2026-11-22 that the space race is won as soon as the tenth part is built:
//! there is no assembly, launch step, flight time or success roll.

use super::*;

pub(super) const SPACE_PART_PROJECTS: [(Building, usize); 10] = [
    (Building::SS_THRUSTERS, 1),
    (Building::SS_ENGINE, 1),
    (Building::SS_DOCKING_BAY, 1),
    (Building::SS_COCKPIT, 1),
    (Building::SS_FUEL_CELLS, 1),
    (Building::SS_LIFE_SUPPORT_SYSTEM, 1),
    (Building::SS_STASIS_CHAMBER, 1),
    (Building::SS_STORAGE_SUPPLY, 1),
    (Building::SS_PLANETARY_PARTY_LOUNGE, 1),
    (Building::SS_EXTERIOR_CASING, 1),
];

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Spaceship {
    pub(super) built: [usize; 10],
}

impl Spaceship {
    pub(super) fn add_part(&mut self, part: usize) {
        self.built[part] = 1;
    }

    pub(super) fn started(&self) -> bool {
        self.built.iter().any(|&n| n > 0)
    }

    pub(super) fn complete(&self) -> bool {
        self.built.iter().all(|&n| n > 0)
    }

    pub(super) fn parts_built(&self) -> usize {
        self.built.iter().filter(|&&n| n > 0).count()
    }

    /// Score awarded for a finished ship (none until all ten parts exist).
    pub(super) fn score(&self) -> i32 {
        0
    }
}

impl Simulation {
    pub(crate) fn spaceship_status(&self, owner: usize) -> Option<String> {
        let ship = &self.spaceships[owner];
        (self.apollo_completed || ship.started()).then(|| {
            format!(
                "Spaceship parts: {} / {}",
                ship.parts_built(),
                SPACE_PART_PROJECTS.len()
            )
        })
    }

    pub(crate) fn spaceship_parts(&self, owner: usize) -> Vec<(&'static str, bool)> {
        SPACE_PART_PROJECTS
            .iter()
            .enumerate()
            .map(|(part, &(building, _))| {
                (building.data().name, self.spaceships[owner].built[part] > 0)
            })
            .collect()
    }

    #[cfg(any(test, feature = "scenarios"))]
    pub(crate) fn spaceship_progress(&self, owner: usize) -> [usize; 10] {
        self.spaceships[owner].built
    }

    /// Records a finished part and ends the game when it is the tenth.
    pub(super) fn complete_space_part(&mut self, owner: usize, part: usize) {
        self.spaceships[owner].add_part(part);
        if self.spaceships[owner].complete()
            && self.victory_enabled(crate::rules::engine_rules::VictoryCondition::SpaceRace)
            && self.player_alive(owner)
            && self.game_over.is_none()
        {
            self.game_over = Some(format!(
                "{} completed the spaceship and won the space race.",
                self.players[owner].name
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ship_is_complete_only_with_all_ten_distinct_parts() {
        let mut ship = Spaceship::default();
        for part in 0..9 {
            ship.add_part(part);
            ship.add_part(part);
            assert!(!ship.complete());
        }
        ship.add_part(9);
        assert!(ship.complete());
        assert_eq!(ship.parts_built(), 10);
    }

    #[test]
    fn tenth_part_wins_the_space_race_immediately() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for part in 0..9 {
            game.complete_space_part(HUMAN, part);
        }
        assert!(game.game_over.is_none());
        game.complete_space_part(HUMAN, 9);
        assert!(
            game.game_over
                .as_deref()
                .is_some_and(|text| text.contains("space race"))
        );
    }
}
