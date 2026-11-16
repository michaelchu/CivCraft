//! Implements worker terrain behavior within the ui script subsystem.

use super::*;

/// Scripted steps in the order the self-test evaluates them.
pub(super) const STEPS: &[Step] = &[
    Step::at(150, |test, game, _buttons| {
        if test.act_passed {
            let probe = game.sim.playtest_harness().transport_playtest_setup();
            game.perform_action(UiAction::SelectUnit(probe.1));
            test.transport_probe = Some(probe);
        }
    }),
    Step::at(152, |test, game, buttons| {
        if let Some((ship, passenger, shore)) = test.transport_probe {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::BoardOrDeboard))
            {
                game.perform_action(action.clone());
            }
            let boarded = game.sim.playtest_state().cargo_count(ship) == 1;
            let sailed = game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(ship, 1, 0);
            let deboard_blocked = !game
                .sim
                .playtest_harness()
                .apply(Command::BoardOrDeboard(passenger));
            let landed = game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(passenger, 1, 0)
                && game.sim.playtest_state().units().iter().any(|unit| {
                    unit.id == passenger && (unit.x, unit.y) == shore && unit.transport.is_none()
                });
            test.results.push(("transport", boarded && sailed && deboard_blocked && landed,
                format!("Boarded in port: {boarded}. Sailed with cargo: {sailed}. Blocked unloading at sea: {deboard_blocked}. Landed on shore: {landed}.")));
        }
    }),
    Step::at(155, |test, game, _buttons| {
        if test.act_passed {
            let (x, y) = game.sim.playtest_harness().production_playtest_setup();
            game.select_at(x, y);
        }
    }),
    Step::at(157, |test, game, buttons| {
        if test.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::RushProduction(_)))
            {
                game.perform_action(action.clone());
            }
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ProductionPage(1)))
            {
                game.perform_action(action.clone());
            }
        }
    }),
    Step::at(159, |test, game, buttons| {
        if test.act_passed {
            let rushed = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .is_some_and(|city| {
                    city.bought_production && city.shields == UnitKind::Warrior.stats().cost
                })
                && game.sim.playtest_state().players()[HUMAN].gold == 50;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::SellBuilding(_, Building::GRANARY)))
            {
                game.perform_action(action.clone());
            }
            let sold = game.sim.playtest_state().players()[HUMAN].gold
                == 50 + Building::GRANARY.data().cost
                && game
                    .sim
                    .playtest_state()
                    .cities()
                    .iter()
                    .find(|city| city.owner == HUMAN)
                    .is_some_and(|city| !city.buildings.contains(&Building::GRANARY));
            test.results.push((
                "city_transactions",
                rushed && sold,
                format!("Rushed Warrior for 50 gold: {rushed}. Sold Granary: {sold}."),
            ));
        }
    }),
    Step::at(160, |test, game, _buttons| {
        if test.act_passed {
            game.perform_action(UiAction::ProductionPage(0));
        }
    }),
    Step::at(162, |test, game, buttons| {
        if test.act_passed {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::EditWorklist(
                        _,
                        WorklistEdit::Append(Production::Unit(UnitKind::Worker))
                    )
                )
            }) {
                game.perform_action(action.clone());
            }
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::WorklistPage(Some(0))))
            {
                game.perform_action(action.clone());
            }
        }
    }),
    Step::at(164, |test, game, buttons| {
        if test.act_passed {
            let visible = buttons.iter().any(|(_, action)| {
                matches!(action, UiAction::EditWorklist(_, WorklistEdit::Remove(0)))
            });
            let turn_ended = game.sim.playtest_harness().apply(Command::EndTurn);
            let advanced = turn_ended
                && game
                    .sim
                    .playtest_state()
                    .cities()
                    .iter()
                    .find(|city| city.owner == HUMAN)
                    .is_some_and(|city| {
                        city.production == Production::Unit(UnitKind::Worker)
                            && city.worklist.is_empty()
                    });
            test.results.push(("worklist", visible && advanced,
                format!("Queued Worker in city UI: {visible}. Advanced to Worker after production: {advanced}.")));
        }
    }),
    Step::at(166, |test, game, _buttons| {
        if test.act_passed {
            let worker = game.sim.playtest_harness().farmland_playtest_setup();
            game.perform_action(UiAction::SelectUnit(worker));
        }
    }),
    Step::at(168, |test, game, buttons| {
        if test.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Irrigate))
            {
                game.perform_action(action.clone());
            }
            let work = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_state()
                        .units()
                        .iter()
                        .find(|unit| unit.id == id)
                })
                .filter(|unit| unit.work_kind == Some(sim::WorkKind::Farmland))
                .map(|unit| (unit.x, unit.y, unit.work_turns_left()));
            let completed = work.is_some_and(|(x, y, turns)| {
                (0..turns).all(|_| game.sim.playtest_harness().apply(Command::EndTurn))
                    && game.sim.playtest_state().tile(x, y).farmland
            });
            test.results.push(("farmland", completed, format!("Started Farmland using the worker control and completed it over end turns: {completed}.")));
        }
    }),
    Step::at(177, |test, game, _buttons| {
        if test.act_passed {
            let (warrior, tile) = game.sim.playtest_harness().pillage_playtest_setup();
            test.pillage_probe = Some(tile);
            game.perform_action(UiAction::SelectUnit(warrior));
        }
    }),
    Step::at(179, |test, game, buttons| {
        if let Some((x, y)) = test.pillage_probe {
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Pillage(sim::PillageTarget::Railroad))
                )
            });
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(action.clone());
                game.sim.playtest_state().tile(x, y).railroad
                    && game.sim.playtest_harness().apply(Command::EndTurn)
                    && !game.sim.playtest_state().tile(x, y).railroad
                    && game.sim.playtest_state().tile(x, y).road
            });
            test.results.push(("pillage", completed, format!("Pillaged the Railroad through the unit control and preserved its Road: {completed}.")));
        }
    }),
    Step::at(182, |_test, game, _buttons| {
        *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
        let worker = game.sim.playtest_harness().base_playtest_setup();
        game.perform_action(UiAction::SelectUnit(worker));
    }),
    Step::at_either(183, 186, |test, game, buttons| {
        let target = if test.frame == 183 {
            sim::Base::Airstrip
        } else {
            sim::Base::Airbase
        };
        if !buttons.iter().any(|(_,action)| matches!(action, UiAction::TerrainWork(sim::WorkKind::BuildBase(base)) if *base == target))
                && let Some((_, action)) = buttons.iter().find(|(_,action)| matches!(action,UiAction::ActionPage(_)))
            {
                game.perform_action(action.clone());
            }
    }),
    Step::at_either(184, 187, |test, game, buttons| {
        let target = if test.frame == 184 {
            sim::Base::Airstrip
        } else {
            sim::Base::Airbase
        };
        let (completed, available, started) = build_base_from_control(game, buttons, target);
        test.results.push((
                if test.frame == 184 { "airstrip" } else { "airbase" },
                completed,
                format!(
                    "Built {} through the Worker control: {completed}. Control available: {}. Started: {started}.",
                    target.name(), available
                ),
            ));
    }),
    Step::at(190, |test, game, _buttons| {
        if test.act_passed {
            let worker = game.sim.playtest_harness().cleanup_playtest_setup();
            game.perform_action(UiAction::SelectUnit(worker));
        }
    }),
    Step::at(192, |test, game, buttons| {
        if test.act_passed {
            let position = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_state()
                        .units()
                        .iter()
                        .find(|unit| unit.id == id)
                })
                .map(|unit| (unit.x, unit.y));
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Clean(sim::Contamination::Fallout))
                )
            });
            let completed = position.zip(action).is_some_and(|((x, y), (_, action))| {
                game.perform_action(action.clone());
                (0..3).all(|_| game.sim.playtest_harness().apply(Command::EndTurn))
                    && !game.sim.playtest_state().tile(x, y).fallout
                    && game.sim.playtest_state().tile(x, y).pollution
            });
            test.results.push(("cleanup", completed,
                format!("Cleaned Fallout through the Worker control while leaving Pollution intact: {completed}.")));
        }
    }),
    Step::at(195, |test, game, _buttons| {
        if test.act_passed {
            let ship = game.sim.playtest_harness().naval_cleanup_playtest_setup();
            game.perform_action(UiAction::SelectUnit(ship));
        }
    }),
    Step::at(197, |test, game, buttons| {
        if test.act_passed {
            let position = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_state()
                        .units()
                        .iter()
                        .find(|unit| unit.id == id)
                })
                .map(|unit| (unit.x, unit.y));
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Clean(sim::Contamination::Pollution))
                )
            });
            let completed = position.zip(action).is_some_and(|((x, y), (_, action))| {
                game.perform_action(action.clone());
                (0..3).all(|_| game.sim.playtest_harness().apply(Command::EndTurn))
                    && !game.sim.playtest_state().tile(x, y).pollution
            });
            test.results.push((
                "naval_cleanup",
                completed,
                format!(
                    "Transport cleaned pollution at sea through its unit control: {completed}."
                ),
            ));
        }
    }),
    Step::at(200, |test, game, _buttons| {
        if test.act_passed {
            let ship = game.sim.playtest_harness().oil_platform_playtest_setup();
            game.perform_action(UiAction::SelectUnit(ship));
        }
    }),
    Step::at(202, |test, game, buttons| {
        if test.act_passed {
            let action = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Mine));
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(action.clone());
                let site = game
                    .selected_unit
                    .and_then(|id| {
                        game.sim
                            .playtest_state()
                            .units()
                            .iter()
                            .find(|unit| unit.id == id)
                    })
                    .filter(|unit| {
                        unit.work_kind == Some(sim::WorkKind::OilPlatform)
                            && unit.work_turns_left() == 2
                    })
                    .map(|unit| (unit.x, unit.y));
                site.is_some_and(|(x, y)| {
                    (0..2).all(|_| game.sim.playtest_harness().apply(Command::EndTurn))
                        && game.sim.playtest_state().tile(x, y).oil_platform
                })
            });
            test.results.push(("oil_platform", completed,
                format!("Transport built a deep-ocean Oil Platform in two turns through its unit control: {completed}.")));
        }
    }),
    Step::at(205, |test, game, _buttons| {
        if test.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (warrior, diplomat, destination) =
                game.sim.playtest_harness().zone_control_playtest_setup();
            game.perform_action(UiAction::SelectUnit(warrior));
            let blocked = !game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(warrior, 1, 0);
            game.perform_action(UiAction::SelectUnit(diplomat));
            let bypassed = game
                .sim
                .playtest_harness()
                .apply_move_civ3_by_offset(diplomat, 1, 0)
                && game
                    .sim
                    .playtest_state()
                    .units()
                    .iter()
                    .any(|unit| unit.id == diplomat && (unit.x, unit.y) == destination);
            test.results.push((
                "zone_control",
                blocked && bypassed,
                format!(
                    "Foreign zone blocked a Warrior: {blocked}. Diplomat bypassed it: {bypassed}."
                ),
            ));
        }
    }),
];

fn build_base_from_control(
    game: &mut Game,
    buttons: &[(Rect, UiAction)],
    target: sim::Base,
) -> (bool, bool, bool) {
    let action = buttons.iter().find(|(_, action)| {
        matches!(action, UiAction::TerrainWork(sim::WorkKind::BuildBase(base)) if *base == target)
    });
    let available = action.is_some();
    let mut started = false;
    let completed = action.is_some_and(|(_, action)| {
        game.perform_action(action.clone());
        let work = game
            .selected_unit
            .and_then(|id| {
                game.sim
                    .playtest_state()
                    .units()
                    .iter()
                    .find(|unit| unit.id == id)
            })
            .filter(|unit| unit.work_kind == Some(sim::WorkKind::BuildBase(target)))
            .map(|unit| (unit.x, unit.y, unit.work_turns_left()));
        started = work.is_some();
        work.is_some_and(|(x, y, turns)| {
            (0..turns).all(|_| game.sim.playtest_harness().apply(Command::EndTurn))
                && game.sim.playtest_state().tile(x, y).base == Some(target)
        })
    });
    (completed, available, started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airstrip_control_runs_the_full_worker_duration() {
        let mut game = Game::new(sim::Simulation::new(crate::mapgen::generate(42)));
        let worker = game.sim.playtest_harness().base_playtest_setup();
        game.perform_action(UiAction::SelectUnit(worker));
        let buttons = [(
            Rect::new(0.0, 0.0, 1.0, 1.0),
            UiAction::TerrainWork(sim::WorkKind::BuildBase(sim::Base::Airstrip)),
        )];

        assert_eq!(
            build_base_from_control(&mut game, &buttons, sim::Base::Airstrip),
            (true, true, true)
        );
    }
}
