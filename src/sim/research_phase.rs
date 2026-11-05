//! Checks for the native lifetime and accumulation of AI research feedback.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_RESEARCH_SAVE: AtomicU64 = AtomicU64::new(0);

impl Simulation {
    pub(crate) fn unset_goal_hud_playtest_setup(&mut self) {
        self.players[HUMAN].research_goal = Tech::BRONZE_WORKING;
        self.players[HUMAN].researching = Tech::ALPHABET;
        self.discover(HUMAN, Tech::BRONZE_WORKING);
    }

    pub(crate) fn native_unset_goal_save_playtest(&self) -> bool {
        let mut game = self.clone();
        let human_goal = game.players[HUMAN].research_goal;
        let command_guard = !game.apply(Command::Research(Tech::UNSET))
            && game.players[HUMAN].research_goal == human_goal;
        game.players[1].research_goal = Tech::UNSET;
        let path = std::env::temp_dir().join(format!(
            "civcraft-unset-goal-{}-{}.save",
            std::process::id(),
            NEXT_RESEARCH_SAVE.fetch_add(1, Ordering::Relaxed)
        ));
        let roundtrip = game
            .save_campaign(&path)
            .and_then(|_| Self::load_campaign(&path))
            .is_ok_and(|loaded| {
                loaded.players[1].research_goal == Tech::UNSET
                    && !loaded.tech_available(1, Tech::UNSET)
                    && loaded.research_target(1) != Tech::UNSET
            });
        game.players[1].researching = Tech::UNSET;
        let invalid_target =
            game.save_campaign(&path).is_ok() && Self::load_campaign(&path).is_err();
        game.players[1].researching = Tech::ALPHABET;
        game.players[1].known.push(Tech::UNSET);
        let invalid_known =
            game.save_campaign(&path).is_ok() && Self::load_campaign(&path).is_err();
        let _ = std::fs::remove_file(path);
        command_guard && roundtrip && invalid_target && invalid_known
    }

    pub(crate) fn native_research_phase_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.ai_tech_wants.clear();
        let initial_zero = Tech::all().all(|tech| game.advisor_tech_want(1, tech) == 0.0);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1.75);
        game.players[1].known.push(Tech::ALPHABET);
        game.players[1].rates = [0, 100, 0];
        let retained = initial_zero && game.advisor_tech_want(1, Tech::ALPHABET) == 1.75;

        game.clear_ai_tech_wants(1);
        game.add_ai_tech_want(1, Tech::ALPHABET, 16_777_216.0);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1.0);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1.0);
        let ordered = game.advisor_tech_want(1, Tech::ALPHABET) == 16_777_216.0;
        game.clear_ai_tech_wants(1);
        let cleared = game.advisor_tech_want(1, Tech::ALPHABET) == 0.0;

        game.units.clear();
        game.cities.clear();
        for p in [(10, 24), (18, 28), (26, 32)] {
            game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
            game.create_city(1, p.0, p.1);
        }
        game.add_ai_tech_want(1, Tech::ALPHABET, 42.5);
        let rebel = game.players.len();
        game.split_civil_war(1);
        let copied = game.players.len() == rebel + 1
            && game.advisor_tech_want(rebel, Tech::ALPHABET) == 42.5;
        game.add_ai_tech_want(rebel, Tech::ALPHABET, 1.0);
        let independent = game.advisor_tech_want(1, Tech::ALPHABET) == 42.5;

        let path = std::env::temp_dir().join(format!(
            "civcraft-research-phase-{}-{}.save",
            std::process::id(),
            NEXT_RESEARCH_SAVE.fetch_add(1, Ordering::Relaxed)
        ));
        let loaded_zero = game
            .save_campaign(&path)
            .and_then(|_| Self::load_campaign(&path))
            .is_ok_and(|loaded| {
                loaded.ai_tech_wants.is_empty()
                    && Tech::all().all(|tech| loaded.advisor_tech_want(1, tech) == 0.0)
            });
        let _ = std::fs::remove_file(path);
        (
            retained,
            ordered && cleared,
            copied && independent,
            loaded_zero,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unset_goal_roundtrips_without_becoming_an_invention_or_current_target() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_unset_goal_save_playtest());
    }

    #[test]
    fn research_feedback_is_ordered_retained_copied_and_transient_on_load() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(
            game.native_research_phase_playtest(),
            (true, true, true, true)
        );
    }

    #[test]
    fn live_last_activities_replace_previous_phase_wants() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(1, 10, 24);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1_000_000.0);
        game.run_ai_last_activities(1);
        assert!(game.ai_tech_wants.get(&1).is_some());
        assert!(game.advisor_tech_want(1, Tech::ALPHABET) < 1_000_000.0);
        let before = game.ai_tech_wants[&1].clone();
        for tech in Tech::all() {
            let _ = game.advisor_tech_want(1, tech);
        }
        assert_eq!(game.ai_tech_wants[&1], before);
    }
}
