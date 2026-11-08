//! Espionage mission rules and AI planning.

use super::*;

impl Playtest {
    pub(super) fn advance_espionage_ui(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 209 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.embassy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 211 && self.act_passed {
            let diplomat = game.selected_unit;
            let action = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::EstablishEmbassy(_)));
            if let Some((_, action)) = action {
                game.perform_action(*action);
            }
            let established = game.sim.has_embassy(HUMAN, 1) && !game.sim.has_embassy(1, HUMAN);
            let became_ambassador =
                diplomat.is_some_and(|id| game.sim.units().iter().all(|unit| unit.id != id));
            game.perform_action(UiAction::ToggleDiplomacy);
            self.results.push(("embassy", established && became_ambassador,
                format!("Established a one-way embassy through the Diplomat control: {established}. Diplomat became ambassador: {became_ambassador}.")));
        }
        if frame == 214 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.embassy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 216
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::InvestigateCity(_)))
        {
            game.perform_action(*action);
        }
        if frame == 218 && self.act_passed {
            let showed_report = buttons
                .iter()
                .any(|(_, action)| matches!(action, UiAction::CloseInvestigation));
            let retained_unit = game.selected_unit.is_some_and(|id| {
                game.sim.units().iter().any(|unit| {
                    unit.id == id
                        && unit.moves_left
                            == sim::MOVE_FRAGMENTS * UnitKind::Diplomat.stats().moves - 1
                })
            });
            game.perform_action(UiAction::CloseInvestigation);
            self.results.push(("investigation", showed_report && retained_unit,
                format!("Opened the investigation report: {showed_report}. Diplomat survived and spent one movement fragment: {retained_unit}.")));
        }
        if frame == 221 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (diplomat, _) = game.sim.bribe_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 223 && self.act_passed {
            let gold = game.sim.players()[HUMAN].gold;
            let action = buttons.iter().find_map(|(_, action)| match action {
                UiAction::BribeUnit(target, price) => Some((*action, *target, *price)),
                _ => None,
            });
            let transferred = action.is_some_and(|(action, id, price)| {
                let Some(target) = game.sim.units().iter().find(|unit| unit.id == id).cloned()
                else {
                    return false;
                };
                game.perform_action(action);
                game.sim.players()[HUMAN].gold == gold - price
                    && game.sim.units().iter().all(|unit| unit.id != id)
                    && game.sim.units().iter().any(|unit| {
                        unit.owner == HUMAN
                            && unit.kind == target.kind
                            && (unit.x, unit.y) == (target.x, target.y)
                            && unit.home_city.is_some()
                    })
            });
            self.results.push(("bribery", transferred,
                format!("Bribed a foreign Warrior through the unit control and paid the displayed price: {transferred}.")));
        }
        if frame == 226 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.technology_theft_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 228 && self.act_passed {
            let diplomat = game.selected_unit;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::StealTechnology(_)))
            {
                game.perform_action(*action);
            }
            let stolen = game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY);
            let captured =
                diplomat.is_some_and(|id| game.sim.units().iter().all(|unit| unit.id != id));
            self.results.push(("technology_theft", stolen && captured,
                format!("Stole Pottery through the Diplomat control: {stolen}. Diplomat captured after mission: {captured}.")));
        }
        if frame == 231 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.sabotage_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 233 && self.act_passed {
            let diplomat = game.selected_unit;
            let action = buttons.iter().find_map(|(_, action)| match action {
                UiAction::SabotageCity(city) => Some((*action, *city)),
                _ => None,
            });
            let stock_destroyed = action.is_some_and(|(action, city)| {
                game.perform_action(action);
                game.sim
                    .cities()
                    .iter()
                    .any(|target| target.id == city && target.shields == 0)
            });
            let captured =
                diplomat.is_some_and(|id| game.sim.units().iter().all(|unit| unit.id != id));
            self.results.push(("sabotage", stock_destroyed && captured,
                format!("Destroyed a foreign city's shield stock through the Diplomat control: {stock_destroyed}. Diplomat captured after mission: {captured}.")));
        }
        if frame == 236 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let spy = game.sim.spy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(spy));
        }
        if frame == 238 && self.act_passed {
            let spy = game.selected_unit;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::StealTechnology(_)))
            {
                game.perform_action(*action);
            }
            let stolen = game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY);
            let escaped =
                spy.is_some_and(|id| {
                    game.sim.units().iter().any(|unit| {
                        unit.id == id
                            && unit.kind == UnitKind::Spy
                            && unit.moves_left == 0
                            && game.sim.cities().iter().any(|city| {
                                city.owner == HUMAN && (city.x, city.y) == (unit.x, unit.y)
                            })
                    })
                });
            self.results.push(("spy_escape", stolen && escaped,
                format!("Spy stole Pottery from a previously targeted city: {stolen}. Escaped to a friendly city: {escaped}.")));
        }
        if frame == 241 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let spy = game.sim.spy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(spy));
        }
        if frame == 243
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::SpyTargets(Some(_))))
        {
            game.perform_action(*action);
        }
        if frame == 245 && self.act_passed {
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TargetedTheft(_, crate::rules::Tech::POTTERY)
                )
            });
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY)
            });
            self.results.push((
                "targeted_espionage",
                completed,
                format!(
                    "Opened the Spy target picker and selected Pottery for theft: {completed}."
                ),
            ));
        }
        if frame == 248 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (explorer, spy) = game.sim.stealth_playtest_setup();
            game.perform_action(UiAction::SelectUnit(explorer));
            let visible = |game: &Game| {
                game.sim
                    .units()
                    .iter()
                    .find(|unit| unit.id == spy)
                    .is_some_and(|unit| game.sim.unit_visible_to_human(unit))
            };
            let hidden = !visible(game);
            let revealed = game.sim.apply(Command::Move(explorer, 1, 0)) && visible(game);
            self.results.push(("spy_stealth", hidden && revealed,
                format!("Foreign Spy hidden outside detection range: {hidden}. Revealed by an adjacent Explorer: {revealed}.")));
        }
    }
}
