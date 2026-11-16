//! Small control surface for deterministic, headless campaign runs.

use super::*;

impl Simulation {
    /// Treat every active civilization as computer controlled for simulation runs.
    pub(crate) fn enable_all_civilizations_as_ai(&mut self) {
        for player in self.players.iter_mut().take(self.civilization_count) {
            player.ai = true;
        }
    }

    /// Whether a civilization's seat is driven by the AI.
    #[cfg(test)]
    pub(crate) fn civilization_is_ai(&self, owner: usize) -> bool {
        self.players[owner].ai
    }

    /// Advance one full campaign turn through the normal turn orchestrator.
    pub(crate) fn simulate_turn(&mut self) {
        self.trace
            .begin_turn_profile(self.turn, self.cities.len(), self.units.len());
        self.end_turn();
        self.trace.finish_turn_profile();
    }
}
