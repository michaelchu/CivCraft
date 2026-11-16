//! Implements military behavior within the regression scenarios subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_military_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 481 && self.act_passed {
            let button = buttons
                .iter()
                .any(|(_, action)| matches!(action, UiAction::BeginParadrop));
            let actor = game.selected_unit;
            let target = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.owner == 1)
                .map(|city| (city.x, city.y))
                .unwrap();
            if button {
                game.perform_action(UiAction::BeginParadrop);
                game.perform_action(UiAction::ParadropTo(target.0, target.1));
            }
            let landed = game.sim.playtest_state().units().iter().any(|unit| {
                Some(unit.id) == actor
                    && (unit.x, unit.y) == target
                    && unit.moves_left == crate::sim::MOVE_FRAGMENTS
                    && !game.sim.playtest_state().can_begin_paradrop(unit)
            });
            let captured = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .any(|city| city.owner == HUMAN && (city.x, city.y) == target);
            self.results.push(("paradrop", button && landed && captured,
                format!("HUD offered the action: {button}. Paratroopers landed with movement and could not drop again: {landed}. They captured the undefended enemy city: {captured}.")));
        }
    }
}
