//! Implements worker terrain behavior within the ui script subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_worker_terrain_ui(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 150 && self.act_passed {
            let probe = game
                .sim
                .playtest_simulation_mut()
                .transport_playtest_setup();
            game.perform_action(UiAction::SelectUnit(probe.1));
            self.transport_probe = Some(probe);
        }
        if frame == 152
            && let Some((ship, passenger, shore)) = self.transport_probe
        {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::BoardOrDeboard))
            {
                game.perform_action(*action);
            }
            let boarded = game.sim.playtest_simulation().cargo_count(ship) == 1;
            let sailed = game
                .sim
                .playtest_simulation_mut()
                .apply(Command::Move(ship, 1, 0));
            let deboard_blocked = !game
                .sim
                .playtest_simulation_mut()
                .apply(Command::BoardOrDeboard(passenger));
            let landed = game
                .sim
                .playtest_simulation_mut()
                .apply(Command::Move(passenger, 1, 0))
                && game.sim.playtest_simulation().units().iter().any(|unit| {
                    unit.id == passenger && (unit.x, unit.y) == shore && unit.transport.is_none()
                });
            self.results.push(("transport", boarded && sailed && deboard_blocked && landed,
                format!("Boarded in port: {boarded}. Sailed with cargo: {sailed}. Blocked unloading at sea: {deboard_blocked}. Landed on shore: {landed}.")));
        }
        if frame == 155 && self.act_passed {
            let (x, y) = game
                .sim
                .playtest_simulation_mut()
                .production_playtest_setup();
            game.select_at(x, y);
        }
        if frame == 157 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::RushProduction(_)))
            {
                game.perform_action(*action);
            }
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ProductionPage(1)))
            {
                game.perform_action(*action);
            }
        }
        if frame == 159 && self.act_passed {
            let rushed = game
                .sim
                .playtest_simulation()
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .is_some_and(|city| {
                    city.bought_production && city.shields == UnitKind::Warrior.stats().cost
                })
                && game.sim.playtest_simulation().players()[HUMAN].gold == 50;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::SellBuilding(_, Building::GRANARY)))
            {
                game.perform_action(*action);
            }
            let sold = game.sim.playtest_simulation().players()[HUMAN].gold
                == 50 + Building::GRANARY.data().cost
                && game
                    .sim
                    .playtest_simulation()
                    .cities()
                    .iter()
                    .find(|city| city.owner == HUMAN)
                    .is_some_and(|city| !city.buildings.contains(&Building::GRANARY));
            self.results.push((
                "city_transactions",
                rushed && sold,
                format!("Rushed Warrior for 50 gold: {rushed}. Sold Granary: {sold}."),
            ));
        }
        if frame == 160 && self.act_passed {
            game.perform_action(UiAction::ProductionPage(0));
        }
        if frame == 162 && self.act_passed {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::EditWorklist(
                        _,
                        WorklistEdit::Append(Production::Unit(UnitKind::Worker))
                    )
                )
            }) {
                game.perform_action(*action);
            }
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::WorklistPage(Some(0))))
            {
                game.perform_action(*action);
            }
        }
        if frame == 164 && self.act_passed {
            let visible = buttons.iter().any(|(_, action)| {
                matches!(action, UiAction::EditWorklist(_, WorklistEdit::Remove(0)))
            });
            let turn_ended = game.sim.playtest_simulation_mut().apply(Command::EndTurn);
            let advanced = turn_ended
                && game
                    .sim
                    .playtest_simulation()
                    .cities()
                    .iter()
                    .find(|city| city.owner == HUMAN)
                    .is_some_and(|city| {
                        city.production == Production::Unit(UnitKind::Worker)
                            && city.worklist.is_empty()
                    });
            self.results.push(("worklist", visible && advanced,
                format!("Queued Worker in city UI: {visible}. Advanced to Worker after production: {advanced}.")));
        }
        if frame == 166 && self.act_passed {
            let worker = game.sim.playtest_simulation_mut().farmland_playtest_setup();
            game.perform_action(UiAction::SelectUnit(worker));
        }
        if frame == 168 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Irrigate))
            {
                game.perform_action(*action);
            }
            let work = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_simulation()
                        .units()
                        .iter()
                        .find(|unit| unit.id == id)
                })
                .filter(|unit| unit.work_kind == Some(sim::WorkKind::Farmland))
                .map(|unit| (unit.x, unit.y, unit.work_turns_left()));
            let completed = work.is_some_and(|(x, y, turns)| {
                (0..turns).all(|_| game.sim.playtest_simulation_mut().apply(Command::EndTurn))
                    && game.sim.playtest_simulation().tile(x, y).farmland
            });
            self.results.push(("farmland", completed, format!("Started Farmland using the worker control and completed it over end turns: {completed}.")));
        }
        if frame == 177 && self.act_passed {
            let (warrior, tile) = game.sim.playtest_simulation_mut().pillage_playtest_setup();
            self.pillage_probe = Some(tile);
            game.perform_action(UiAction::SelectUnit(warrior));
        }
        if frame == 179
            && let Some((x, y)) = self.pillage_probe
        {
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Pillage(sim::PillageTarget::Railroad))
                )
            });
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                game.sim.playtest_simulation().tile(x, y).railroad
                    && game.sim.playtest_simulation_mut().apply(Command::EndTurn)
                    && !game.sim.playtest_simulation().tile(x, y).railroad
                    && game.sim.playtest_simulation().tile(x, y).road
            });
            self.results.push(("pillage", completed, format!("Pillaged the Railroad through the unit control and preserved its Road: {completed}.")));
        }
        if frame == 182 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            game.perform_action(UiAction::FoundCity);
            game.perform_action(UiAction::CloseCity);
            let worker = game.sim.playtest_simulation_mut().base_playtest_setup();
            game.perform_action(UiAction::SelectUnit(worker));
        }
        if frame == 183 || frame == 186 {
            let target = if frame == 183 {
                sim::Base::Airstrip
            } else {
                sim::Base::Airbase
            };
            if !buttons.iter().any(|(_,action)| matches!(action, UiAction::TerrainWork(sim::WorkKind::BuildBase(base)) if *base == target))
                && let Some((_, action)) = buttons.iter().find(|(_,action)| matches!(action,UiAction::ActionPage(_)))
            {
                game.perform_action(*action);
            }
        }
        if frame == 184 || frame == 187 {
            let target = if frame == 184 {
                sim::Base::Airstrip
            } else {
                sim::Base::Airbase
            };
            let position = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_simulation()
                        .units()
                        .iter()
                        .find(|unit| unit.id == id)
                })
                .map(|unit| (unit.x, unit.y));
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action, UiAction::TerrainWork(sim::WorkKind::BuildBase(base)) if *base == target
                )
            });
            let mut started = false;
            let completed = position.zip(action).is_some_and(|((x, y), (_, action))| {
                game.perform_action(*action);
                started = game.selected_unit.is_some_and(|id| {
                    game.sim.playtest_simulation().units().iter().any(|unit| {
                        unit.id == id && unit.work_kind == Some(sim::WorkKind::BuildBase(target))
                    })
                });
                started
                    && game.sim.playtest_simulation_mut().apply(Command::EndTurn)
                    && game.sim.playtest_simulation().tile(x, y).base == Some(target)
            });
            self.results.push((
                if frame == 184 { "airstrip" } else { "airbase" },
                completed,
                format!(
                    "Built {} through the Worker control: {completed}. Control available: {}. Started: {started}.",
                    target.name(), action.is_some()
                ),
            ));
        }
        if frame == 190 && self.act_passed {
            let worker = game.sim.playtest_simulation_mut().cleanup_playtest_setup();
            game.perform_action(UiAction::SelectUnit(worker));
        }
        if frame == 192 && self.act_passed {
            let position = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_simulation()
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
                game.perform_action(*action);
                (0..2).all(|_| game.sim.playtest_simulation_mut().apply(Command::EndTurn))
                    && !game.sim.playtest_simulation().tile(x, y).fallout
                    && game.sim.playtest_simulation().tile(x, y).pollution
            });
            self.results.push(("cleanup", completed,
                format!("Cleaned Fallout through the Worker control while leaving Pollution intact: {completed}.")));
        }
        if frame == 195 && self.act_passed {
            let ship = game
                .sim
                .playtest_simulation_mut()
                .naval_cleanup_playtest_setup();
            game.perform_action(UiAction::SelectUnit(ship));
        }
        if frame == 197 && self.act_passed {
            let position = game
                .selected_unit
                .and_then(|id| {
                    game.sim
                        .playtest_simulation()
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
                game.perform_action(*action);
                (0..3).all(|_| game.sim.playtest_simulation_mut().apply(Command::EndTurn))
                    && !game.sim.playtest_simulation().tile(x, y).pollution
            });
            self.results.push((
                "naval_cleanup",
                completed,
                format!(
                    "Transport cleaned pollution at sea through its unit control: {completed}."
                ),
            ));
        }
        if frame == 200 && self.act_passed {
            let ship = game
                .sim
                .playtest_simulation_mut()
                .oil_platform_playtest_setup();
            game.perform_action(UiAction::SelectUnit(ship));
        }
        if frame == 202 && self.act_passed {
            let action = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Mine));
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                let site = game
                    .selected_unit
                    .and_then(|id| {
                        game.sim
                            .playtest_simulation()
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
                    (0..2).all(|_| game.sim.playtest_simulation_mut().apply(Command::EndTurn))
                        && game.sim.playtest_simulation().tile(x, y).oil_platform
                })
            });
            self.results.push(("oil_platform", completed,
                format!("Transport built a deep-ocean Oil Platform in two turns through its unit control: {completed}.")));
        }
        if frame == 205 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (warrior, diplomat, destination) = game
                .sim
                .playtest_simulation_mut()
                .zone_control_playtest_setup();
            game.perform_action(UiAction::SelectUnit(warrior));
            let blocked = !game
                .sim
                .playtest_simulation_mut()
                .apply(Command::Move(warrior, 1, 0));
            game.perform_action(UiAction::SelectUnit(diplomat));
            let bypassed = game
                .sim
                .playtest_simulation_mut()
                .apply(Command::Move(diplomat, 1, 0))
                && game
                    .sim
                    .playtest_simulation()
                    .units()
                    .iter()
                    .any(|unit| unit.id == diplomat && (unit.x, unit.y) == destination);
            self.results.push((
                "zone_control",
                blocked && bypassed,
                format!(
                    "Foreign zone blocked a Warrior: {blocked}. Diplomat bypassed it: {bypassed}."
                ),
            ));
        }
    }
}
