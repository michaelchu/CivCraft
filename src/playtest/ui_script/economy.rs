//! Tax rates, income, upkeep, and city economic calculations.

use super::*;

/// Scripted steps in the order the self-test evaluates them.
pub(super) const STEPS: &[Step] = &[
    Step::at(302, |test, game, _buttons| {
        if test.act_passed {
            let mut outcomes = [false; 2];
            for (index, nuclear) in [false, true].into_iter().enumerate() {
                super::reset_founded_city_scenario(game);
                outcomes[index] = game.sim.playtest_harness().run_disaster_playtest(nuclear);
            }
            test.results.push(("city_disasters", outcomes == [true, true],
                format!("Industrial accident reduced population and left pollution: {}. Nuclear accident reduced population and left fallout: {}.", outcomes[0], outcomes[1])));
        }
    }),
    Step::at(304, |test, game, _buttons| {
        if test.act_passed {
            let mut sizes = [0; 2];
            for (index, protected) in [false, true].into_iter().enumerate() {
                super::reset_founded_city_scenario(game);
                sizes[index] = game.sim.playtest_harness().run_plague_playtest(protected);
            }
            test.results.push((
                "city_plague",
                sizes == [8, 9],
                format!(
                    "City size after the same plague roll without and with an Aqueduct: {sizes:?}."
                ),
            ));
        }
    }),
    Step::at(315, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let scenario = game.sim.playtest_harness().airlift_playtest_setup();
            test.airlift = Some(scenario);
            game.perform_action(UiAction::SelectUnit(scenario.0));
        }
    }),
    Step::at(316, |test, game, buttons| {
        if let Some((first, second, destination)) = test.airlift {
            if let Some((_, action)) = buttons.iter().find(
                |(_, action)| matches!(action, UiAction::Airlift(city) if *city == destination),
            ) {
                game.perform_action(action.clone());
            }
            let city = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.id == destination)
                .unwrap();
            let arrived = game.sim.playtest_state().units().iter().any(|unit| {
                unit.id == first && (unit.x, unit.y) == (city.x, city.y) && unit.moves_left == 0
            });
            let limited = !game
                .sim
                .playtest_harness()
                .apply(Command::Airlift(second, destination));
            test.results.push(("airlift", arrived && limited,
                format!("Warrior airlifted to a city without an Airport: {arrived}. Second departure from the same Airport was blocked: {limited}.")));
        }
    }),
    Step::at(318, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            game.sim.playtest_harness().spaceship_playtest_setup();
        }
    }),
    Step::at(319, |test, game, buttons| {
        if test.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleSpaceship))
        {
            game.perform_action(action.clone());
        }
    }),
    Step::at(320, |test, game, buttons| {
        if test.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::LaunchSpaceship))
            {
                game.perform_action(action.clone());
            }
            let launched = game
                .sim
                .playtest_state()
                .spaceship_status(HUMAN)
                .is_some_and(|status| status.starts_with("Spaceship arrives"));
            game.sim
                .playtest_harness()
                .spaceship_playtest_finish_flight();
            let won = game
                .sim
                .playtest_state()
                .game_over()
                .is_some_and(|message| message.contains("Alpha Centauri"));
            test.results.push((
                "space_race",
                launched && won,
                format!(
                    "Spaceship launched through the HUD: {launched}. Arrival ended the game: {won}."
                ),
            ));
        }
    }),
    Step::at(321, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            crate::sim::scenarios::ui_review::manual_spaceship_playtest_setup(
                &mut game.sim.playtest_harness(),
            );
            game.perform_action(UiAction::ToggleSpaceship);
        }
    }),
    Step::at(322, |test, game, buttons| {
        if test.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleManualAssembly))
        {
            game.perform_action(action.clone());
            game.sim
                .playtest_harness()
                .manual_spaceship_playtest_build_structural();
        }
    }),
    Step::at(323, |test, game, buttons| {
        if test.act_passed {
            let root = crate::sim::ShipPart::Structural(0);
            let child = crate::sim::ShipPart::Structural(1);
            let offered = |part| {
                buttons.iter().find(|(_, action)| matches!(action, UiAction::PlaceShipPart(candidate) if *candidate == part))
            };
            let blocked = offered(child).is_none();
            if let Some((_, action)) = offered(root) {
                game.perform_action(action.clone());
            }
            let (unplaced, structure, _, _, manual) = game.sim.playtest_state().ship_assembly();
            let installed = manual && structure[0] && unplaced[0] == 0;
            game.perform_action(UiAction::ToggleSpaceship);
            test.results.push(("manual_spaceship", blocked && installed,
                format!("Disconnected structural slot was unavailable: {blocked}. The assembly control installed the root slot: {installed}.")));
        }
    }),
    Step::at(325, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            game.sim.playtest_harness().turn_limit_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let ranked = game
                .sim
                .playtest_state()
                .game_over()
                .is_some_and(|message| message.starts_with("Turn limit reached"))
                && game
                    .sim
                    .playtest_state()
                    .score_standings()
                    .first()
                    .is_some_and(|(owner, _)| *owner == HUMAN);
            test.results.push(("score_endgame", ranked, format!("The final turn ended the game and ranked the highest-scoring surviving civilization first: {ranked}.")));
        }
    }),
    Step::at(328, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let bonus = game.sim.playtest_harness().achievement_playtest_setup();
            let mapped = game
                .sim
                .playtest_state()
                .events()
                .iter()
                .any(|event| event.contains("Entire Map Known"));
            test.results.push(("achievements", mapped && bonus >= 20,
                format!("Apollo revealed the map and earned the mapping achievement: {mapped}. Score bonus from newly earned achievements: {bonus} points.")));
        }
    }),
    Step::at(331, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let (unit, city) = game.sim.playtest_harness().multicultural_playtest_setup();
            game.perform_action(UiAction::SelectUnit(unit));
            let captured = game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(unit, 1, 0)
                && game.sim.playtest_state().cities().iter().any(|target| {
                    target.id == city
                        && target.owner == HUMAN
                        && target.size == 2
                        && target.domestic_citizens() == 0
                });
            game.perform_action(UiAction::EndTurn);
            let awarded = game
                .sim
                .playtest_state()
                .events()
                .iter()
                .any(|event| event.contains("Multicultural"));
            test.results.push(("citizen_nationality", captured && awarded,
                format!("Captured city retained its two surviving foreign citizens: {captured}. Multicultural was awarded: {awarded}.")));
        }
    }),
    Step::at(334, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let (unit, city) = game.sim.playtest_harness().partisan_playtest_setup();
            game.perform_action(UiAction::SelectUnit(unit));
            let captured = game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(unit, 1, 0)
                && game
                    .sim
                    .playtest_state()
                    .cities()
                    .iter()
                    .any(|target| target.id == city && target.owner == HUMAN);
            let resistance = game
                .sim
                .playtest_state()
                .units()
                .iter()
                .filter(|unit| {
                    unit.owner == 1
                        && unit.kind == UnitKind::Guerilla
                        && unit.state == sim::UnitState::Fortified
                })
                .count();
            test.results.push(("partisan_resistance", captured && resistance > 0,
                format!("City captured: {captured}. Fortified Partisans spawned for the former owner: {resistance}.")));
        }
    }),
    Step::at(337, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let (before, after, history) = game.sim.playtest_harness().amphitheater_playtest();
            test.results.push(("amphitheater", before && !after && history == 3,
                format!("Electricity let an Amphitheater end disorder in a size-eight city: {}. Selected C2C3 Palace and Colosseum effects added this turn's history: {history}.", before && !after)));
        }
    }),
    Step::at(340, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let (advance, host, rival) = game.sim.playtest_harness().science_wonder_playtest();
            test.results.push(("science_wonders", advance && host == 28 && rival == 20,
                format!("Great Library granted a free advance: {advance}. Host and rival science output with Libraries: {host} and {rival}.")));
        }
    }),
    Step::at(343, |test, game, _buttons| {
        if test.act_passed {
            super::reset_founded_city_scenario(game);
            let (hp, docked_moves, sea_moves, rank) =
                game.sim.playtest_harness().naval_bonus_playtest();
            test.results.push(("naval_bonuses", (hp, docked_moves, sea_moves, rank) == (3, 54, 40, 1),
                format!("Port repaired the cruiser to {hp} HP and grants build rank {rank}. Repaired and damaged ships received {docked_moves} and {sea_moves} movement fragments.")));
        }
    }),
];
