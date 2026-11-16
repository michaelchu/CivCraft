//! Implements ui script behavior within the playtest subsystem.

use super::*;
#[path = "ui_script/combat.rs"]
mod combat;
#[path = "ui_script/economy.rs"]
mod economy;
#[path = "ui_script/naval_ai.rs"]
mod naval_ai;
#[path = "ui_script/worker_terrain.rs"]
mod worker_terrain;

fn reset_founded_city_scenario(game: &mut Game) {
    *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
    game.perform_action(UiAction::FoundCity);
    game.complete_playtest_presentation();
    // Founding opens the city screen; scenario controls below exercise the
    // map and unit UI and would otherwise be ignored while that modal is open.
    game.perform_action(UiAction::CloseCity);
}

/// The UI scripts in the order the self-test evaluates them each frame.
pub(super) const SCRIPTS: &[&[Step]] = &[
    worker_terrain::STEPS,
    combat::STEPS,
    economy::STEPS,
    naval_ai::STEPS,
];
