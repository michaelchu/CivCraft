//! Technology availability, cost, and research progression.

use super::*;

/// Scripted steps in the order the self-test evaluates them.
pub(super) const STEPS: &[Step] = &[
    Step::at(779, |test, _game, buttons| {
        if test.act_passed {
            let oil = buttons
                .iter()
                .any(|(_, action)| matches!(action, UiAction::Mine));
            let cleanup = buttons.iter().any(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(crate::sim::WorkKind::Clean(_))
                )
            });
            let land = buttons.iter().any(|(_, action)| {
                matches!(
                    action,
                    UiAction::Irrigate
                        | UiAction::TerrainWork(
                            crate::sim::WorkKind::Cultivate | crate::sim::WorkKind::Plant
                        )
                )
            });
            test.results.push(("native_transport_worker_hud", oil && cleanup && !land,
                format!("Actual Transport HUD offered offshore oil work: {oil}; cleanup: {cleanup}; excluded land-only work: {}.", !land)));
        }
    }),
    Step::at(802, |test, game, _buttons| {
        if test.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            game.sim.playtest_harness().unset_goal_hud_playtest_setup();
            game.perform_action(UiAction::OpenResearch);
        }
    }),
    Step::at(803, |test, game, buttons| {
        if test.act_passed {
            let close = buttons.iter().find_map(|(_, action)| {
                matches!(action, UiAction::CloseResearch).then_some(action.clone())
            });
            let current = buttons.iter().any(|(_, action)| {
                matches!(action,
                UiAction::ResearchGoal(tech) if *tech == crate::rules::Tech::ALPHABET)
            });
            let phantom = buttons.iter().any(|(_, action)| {
                matches!(action,
                UiAction::ResearchGoal(tech) if *tech == crate::rules::Tech::UNSET)
            });
            let unset = game.sim.playtest_state().players()[HUMAN].research_goal
                == crate::rules::Tech::UNSET;
            test.results.push(("native_unset_goal_research_screen", unset && close.is_some() && current && !phantom,
                format!("Actual research screen opened with an unset goal: {unset}; current-project node visible: {current}; phantom unset node: {phantom}.")));
            if let Some(action) = close {
                game.perform_action(action);
            }
        }
    }),
];
