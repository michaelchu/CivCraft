//! Implements early behavior within the regression scenarios subsystem.

use super::*;

/// Scripted steps in the order the self-test evaluates them.
pub(super) const STEPS: &[Step] = &[
    Step::at(419, |test, game, _buttons| {
        if test.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (marines, infantry, guard, unlocked) =
                game.sim.playtest_harness().marines_playtest_setup();
            let blocked = !game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(infantry, 1, 0);
            let fought = game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(marines, 1, 0)
                && game
                    .sim
                    .playtest_state()
                    .units()
                    .iter()
                    .all(|unit| unit.id != guard);
            let landed = game
                .sim
                .playtest_state()
                .units()
                .iter()
                .find(|unit| unit.id == marines)
                .is_some_and(|unit| {
                    unit.transport.is_none()
                        && game
                            .sim
                            .playtest_state()
                            .cities()
                            .iter()
                            .filter(|city| city.owner == HUMAN)
                            .any(|city| (city.x, city.y) == (unit.x, unit.y))
                });
            let mut uprising = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let raiders = uprising.uprising_playtest(true, true);
            test.results.push(("marines", unlocked && blocked && fought && landed && raiders,
                format!("Amphibious Warfare unlocked production: {unlocked}. The ordinary passenger was blocked: {blocked}. Marines fought and captured the city from a transport: {}. Advanced sea raiders included Marines: {raiders}.", fought && landed)));
        }
    }),
    Step::at(479, |test, game, _buttons| {
        if test.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (actor, _, gates) = game.sim.playtest_harness().paradrop_playtest_setup();
            test.results.push((
                "paratrooper_requirements",
                gates,
                "Advanced Flight unlocked the Civ III Paratrooper, and a tile beyond range was rejected."
                    .to_owned(),
            ));
            game.perform_action(UiAction::SelectUnit(actor));
        }
    }),
];
