//! Implements native rules behavior within the regression scenarios subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_native_rule_scenarios(&mut self, frame: u32, game: &mut Game) {
        if frame == 766 {
            let (mapped, distinct, labels) = crate::client::native_campaign_unit_art_playtest();
            for (name, passed) in [
                ("civ3_campaign_unit_art_roles", mapped),
                ("civ3_campaign_unit_art_role_distinction", distinct),
                ("civ3_campaign_unit_art_labels", labels),
            ] {
                self.results.push((name, passed, "All 92 campaign units map to distinct source-art roles and have display labels.".to_owned()));
            }
        }

        if frame == 778 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let id = game.sim.playtest_harness().transport_hud_playtest_setup();
            game.perform_action(UiAction::SelectUnit(id));
        }
    }
}
