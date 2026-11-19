//! Typed research preparation for deterministic fixtures.

use super::WorldBuilder;
use crate::rules::{Tech, civilizations};

impl WorldBuilder<'_> {
    /// Override advisor weights for a controlled policy comparison.
    pub(crate) fn research_wants(&mut self, owner: usize, wants: &[(Tech, f32)]) {
        self.game.clear_ai_tech_wants(owner);
        for &(technology, want) in wants {
            self.game.add_ai_tech_want(owner, technology, want);
        }
    }

    /// Seed the previous accounting phase without generating research events.
    pub(crate) fn last_science(&mut self, owner: usize, science: i32) {
        self.game.players[owner].last_science = science;
    }

    /// Change only identity while comparing policy preferences over the same world.
    pub(crate) fn civilization_identity(&mut self, owner: usize, id: &str) {
        assert!(civilizations::by_id(id).is_some());
        self.game.players[owner].civilization_id = Some(id.into());
    }

    pub(crate) fn research_goal(&mut self, owner: usize, goal: Tech) {
        self.game.players[owner].research_goal = goal;
    }

    pub(crate) fn known_technologies(&mut self, owner: usize, technologies: &[Tech]) {
        self.game.players[owner].known = technologies.to_vec();
    }

    pub(crate) fn ai_control(&mut self, owner: usize, enabled: bool) {
        self.game.players[owner].ai = enabled;
    }

    pub(crate) fn research_project(&mut self, owner: usize, target: Tech, progress: i32) {
        assert!(progress >= 0);
        self.game.players[owner].researching = target;
        self.game.players[owner].research = progress;
    }

    pub(crate) fn research_progress(&mut self, owner: usize, progress: i32) {
        assert!(progress >= 0);
        self.game.players[owner].research = progress;
    }

    pub(crate) fn add_technology(&mut self, owner: usize, technology: Tech) {
        self.game.players[owner].known.push(technology);
    }
}
