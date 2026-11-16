//! Implements native rules behavior within the regression scenarios subsystem.

use super::*;

/// Scripted steps in the order the self-test evaluates them.
pub(super) const STEPS: &[Step] = &[
    Step::at(766, |test, _game, _buttons| {
        let (mapped, distinct, labels) = crate::client::native_campaign_unit_art_playtest();
        for (name, passed) in [
            ("civ3_campaign_unit_art_roles", mapped),
            ("civ3_campaign_unit_art_role_distinction", distinct),
            ("civ3_campaign_unit_art_labels", labels),
        ] {
            test.results.push((
                name,
                passed,
                "All 92 campaign units map to distinct source-art roles and have display labels."
                    .to_owned(),
            ));
        }
    }),
    Step::at(778, |test, game, _buttons| {
        if test.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let id = game.sim.playtest_harness().transport_hud_playtest_setup();
            game.perform_action(UiAction::SelectUnit(id));
        }
    }),
];
