//! Opening-turn scenario setup for desktop gameplay automation.

use super::*;

impl Playtest {
    pub(super) fn advance_opening(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 12
            && let Some(explorer) = game
                .sim
                .units()
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Explorer)
                .cloned()
        {
            for (dx, dy) in ISOHEX_DIRS {
                if game.sim.apply(Command::Move(explorer.id, dx, dy)) {
                    if let Some(after) = game.sim.units().iter().find(|u| u.id == explorer.id) {
                        let distance = game
                            .sim
                            .map_distance(explorer.x, explorer.y, after.x, after.y);
                        self.move_passed = distance > 0;
                        self.move_detail = format!(
                            "Explorer moved {distance} tile from {},{} to {},{} using terrain movement costs",
                            explorer.x, explorer.y, after.x, after.y
                        );
                    }
                    break;
                }
            }
        }

        if frame == 30
            && let Some(settler_id) = game
                .sim
                .units()
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settler)
                .map(|u| u.id)
        {
            let cities_before = game.sim.cities().len();
            game.selected_unit = Some(settler_id);
            game.perform_action(UiAction::FoundCity);
            let cities_after = game.sim.cities().len();
            self.act_passed = cities_after == cities_before + 1;
            self.act_detail = if self.act_passed {
                "Settlers founded a city, claimed its workable tiles, and opened city production"
                    .to_owned()
            } else {
                "Settlers could not found a city at the scripted starting tile".to_owned()
            };
        }

        if frame == 34
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CityTiles))
        {
            game.perform_action(*action);
        }
        if frame == 36
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::WorkCityTile(..)))
        {
            game.perform_action(*action);
        }
        if frame == 38 && self.act_passed {
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let reassigned = city.worked_tiles.is_empty() && city.specialists[2] == 1;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Specialist(_, 2, -1)))
            {
                game.perform_action(*action);
            }
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let restored = city.worked_tiles.len() == 1 && city.specialists[2] == 0;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CityTiles))
            {
                game.perform_action(*action);
            }
            self.results.push(("city_tiles", reassigned && restored,
                format!("Worked tile reassigned to entertainer: {reassigned}. Tile labor restored: {restored}.")));
        }

        // The city manager is modal: use its close control before touching HUD rates.
        if (frame == 43 || frame == 76)
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CloseCity))
        {
            game.perform_action(*action);
        }
        if frame == 53
            && self.act_passed
            && let Some(city) = game.sim.cities().iter().find(|city| city.owner == HUMAN)
        {
            let (x, y) = (city.x, city.y);
            game.select_at(x, y);
        }
        if frame == 45 && self.act_passed {
            if let Some((_, rate_button)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::RaiseRate(1)))
            {
                game.perform_action(*rate_button);
            }
            let rate_changed = game.sim.players()[HUMAN].rates == [50, 50, 0];
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let trade = game.sim.city_yields(city).2;
            let [science, gold, _] = game.sim.city_trade(city, trade);
            let player = &game.sim.players()[HUMAN];
            let expected_gold = player.gold + gold - game.sim.building_upkeep(city);
            let expected_research = player.research + science;
            game.sim.apply(Command::EndTurn);
            let player = &game.sim.players()[HUMAN];
            self.act_passed = rate_changed
                && player.gold == expected_gold
                && player.research == expected_research;
            self.act_detail.push_str(&format!(
                ". Used the tax control and ended the turn. Treasury {} (expected {expected_gold}), research {} (expected {expected_research}).",
                player.gold, player.research
            ));
        }

        if frame == 55 && self.act_passed {
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let city_id = city.id;
            let before = game.sim.city_yields(city);
            if let Some((_, action)) = buttons.iter().find(
                |(_, action)| matches!(action, UiAction::Specialist(id, 0, 1) if *id == city_id),
            ) {
                game.perform_action(*action);
            }
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.id == city_id)
                .unwrap();
            let after = game.sim.city_yields(city);
            let science = game.sim.city_trade(city, 0)[0];
            self.act_passed = city.specialists == [1, 0, 0] && before != after && science == 2;
            self.act_detail.push_str(&format!(
                " Scientist replaced a tile worker and provided {science} science."
            ));
        }

        if frame == 65 && self.act_passed {
            game.sim.tribal_upkeep_playtest_setup();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Specialist(_, 0, -1)))
            {
                game.perform_action(*action);
            }
            let mut verified = false;
            for _ in 0..12 {
                let Some(city) = game.sim.cities().iter().find(|city| city.owner == HUMAN) else {
                    break;
                };
                let id = city.id;
                let support = game.sim.city_unit_upkeep(city);
                if support.shields == 1 {
                    let (_, gross, trade) = game.sim.city_yields(city);
                    let luxury = game.sim.city_trade(city, trade)[2];
                    let expected = (city.shields + gross - 1) % city.production.cost();
                    let disorder = game.sim.city_in_disorder(city, luxury);
                    game.sim.apply(Command::EndTurn);
                    verified = !disorder
                        && game
                            .sim
                            .cities()
                            .iter()
                            .any(|city| city.id == id && city.shields == expected);
                    break;
                }
                game.sim.apply(Command::EndTurn);
            }
            self.act_passed = verified;
            self.act_detail.push_str(&format!(
                " Under Tribal government, the third supported unit charged one shield on the next turn: {verified}."
            ));
        }

        if frame == 78
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleGovernment))
        {
            game.perform_action(*action);
        }
        if frame == 83 && self.act_passed {
            use sim::Government;
            let locked = !buttons.iter().any(|(_, action)| {
                matches!(action, UiAction::ChangeGovernment(Government::Monarchy))
            });
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::ChangeGovernment(Government::Despotism))
            }) {
                game.perform_action(*action);
            }
            let player = &game.sim.players()[HUMAN];
            self.act_passed = locked
                && player.government == Government::Anarchy
                && player.revolution_target == Some(Government::Despotism);
            self.act_detail.push_str(&format!(
                " Government panel enforced the tech lock and started Anarchy: {}.",
                self.act_passed
            ));
        }
        if frame == 90 && self.act_passed {
            use sim::Government;
            let finish = game.sim.players()[HUMAN].revolution_turn.unwrap();
            game.perform_action(UiAction::ToggleGovernment);
            while game.sim.turn() < finish && game.sim.apply(Command::EndTurn) {}
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let upkeep = game.sim.city_unit_upkeep(city);
            let [_, gold, luxury] = game.sim.city_trade(city, game.sim.city_yields(city).2);
            let revenue = if game.sim.city_in_disorder(city, luxury) {
                0
            } else {
                gold
            };
            let treasury = game.sim.players()[HUMAN].gold + revenue
                - game.sim.building_upkeep(city)
                - upkeep.gold;
            let adopted = game.sim.players()[HUMAN].government == Government::Despotism;
            game.sim.apply(Command::EndTurn);
            self.act_passed = adopted
                && upkeep.shields == 0
                && upkeep.gold > 0
                && game.sim.players()[HUMAN].gold == treasury;
            self.act_detail.push_str(&format!(
                " Despotism charged {} gold and {} shields in unit upkeep: {}.",
                upkeep.gold, upkeep.shields, self.act_passed
            ));
        }

        if frame == 98 && self.act_passed {
            game.selected_unit = game.sim.civil_war_playtest();
            self.act_passed = game.selected_unit.is_some();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleDiplomacy))
            {
                game.perform_action(*action);
            }
        }
        if frame == 101 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::OpenDiplomacyWith(1)))
            {
                game.perform_action(*action);
            }
            self.act_passed = game.sim.pact(HUMAN, 1) == sim::Pact::War;
        }
        if frame == 102 && self.act_passed {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::ProposeTreaty(1, offer)
                    if offer.pact == Some(sim::Pact::Ceasefire))
            }) {
                game.perform_action(*action);
            }
            self.act_passed = game.sim.pact_turns(HUMAN, 1) == 16;
        }
        if frame == 105 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            self.act_passed = game.sim.pact_turns(HUMAN, 1) == 16
                && !game
                    .sim
                    .apply(Command::Move(game.selected_unit.unwrap(), 1, 0));
            self.act_detail.push_str(&format!(
                " Senate veto preserved the ceasefire and blocked the attack: {}.",
                self.act_passed
            ));
        }
        if frame == 115 && self.act_passed {
            game.sim.apply(Command::Government(sim::Government::Despotism));
            let cancellation = game.sim.apply(Command::CancelPact(1));
            game.perform_action(UiAction::ToggleDiplomacy);
            game.sim.civil_war_playtest_seed_capture();
            let rebel = game.sim.players().len();
            let moved = game
                .sim
                .apply(Command::Move(game.selected_unit.unwrap(), 1, 0));
            let cities = game
                .sim
                .cities()
                .iter()
                .filter(|city| city.owner == rebel)
                .count();
            let rival_cities = game
                .sim
                .cities()
                .iter()
                .filter(|city| city.owner == 1)
                .count();
            self.act_passed = moved
                && cities == 4
                && game.sim.players()[1].government == sim::Government::Anarchy;
            if self.act_passed {
                game.sim.civil_war_playtest_resume_diplomacy();
                game.sim.apply(Command::EndTurn);
                self.act_passed = game.sim.players()[1].government == sim::Government::Despotism
                    && game.sim.cities().iter().any(|city| city.owner == rebel);
            }
            self.act_detail.push_str(&format!(" Revolution lifted the veto; pact cancellation {cancellation}, capital move {moved}, remaining rival cities {rival_cities}, rebel cities {cities}: {}.", self.act_passed));
        }

        if frame == 122 && self.act_passed {
            game.perform_action(UiAction::ToggleDiplomacy);
        }
        if frame == 125 && self.act_passed {
            game.perform_action(UiAction::OpenDiplomacyWith(1));
            let refused_early_peace = !game.sim.apply(Command::Negotiate(1, sim::Pact::Peace));
            self.act_passed =
                refused_early_peace && game.sim.pact(HUMAN, 1) == sim::Pact::Ceasefire;
            let peace_refused = refused_early_peace;
            let ceasefire_held = game.sim.pact(HUMAN, 1) == sim::Pact::Ceasefire;
            game.sim.diplomacy_playtest_technologies(false);
            game.sim.diplomacy_playtest_friendship();
            game.sim.diplomacy_playtest_tick(12);
            self.act_detail.push_str(&format!(
                " Diplomacy setup: early peace refused {peace_refused}, ceasefire held {ceasefire_held}, AI offer {:?}.",
                game.sim.pact_offer(1)
            ));
        }
        if frame == 128 && self.act_passed {
            let treaty_offer = game.sim.treaty_offer(1);
            let offered = treaty_offer.is_some_and(|offer| offer.pact == Some(sim::Pact::Peace));
            if let Some(offer) = treaty_offer {
                game.perform_action(UiAction::AcceptOffer(1, offer));
            }
            self.act_passed = offered && game.sim.pact(HUMAN, 1) == sim::Pact::Armistice;
            let accepted_offer = game.sim.pact(HUMAN, 1) == sim::Pact::Armistice;
            let acceptance_event = game.sim.events().last().cloned().unwrap_or_default();
            game.sim.diplomacy_playtest_technologies(true);
            game.sim.diplomacy_playtest_tick(16);
            let matured = game.sim.pact(HUMAN, 1) == sim::Pact::Peace;
            self.diplomacy_probe = game.sim.diplomacy_playtest_unit(1);
            game.sim.diplomacy_playtest_tick(1);
            let peace_held = game.sim.pact(HUMAN, 1) == sim::Pact::Peace;
            let entry_blocked = self
                .diplomacy_probe
                .is_some_and(|(unit, _, _)| !game.sim.apply(Command::Move(unit, 1, 0)));
            self.act_passed &= peace_held && entry_blocked;
            self.act_detail.push_str(&format!(" AI refused early peace, then offered it after the ceasefire held. Offer presented {offered} ({treaty_offer:?}), accepted {accepted_offer} ({acceptance_event}), armistice matured {matured}, peace held {peace_held}, peace blocked entry {entry_blocked}: {}.", self.act_passed));
        }
        if frame == 132 && self.act_passed {
            self.vision_probe = game.sim.diplomacy_playtest_hidden_city(1);
            let offer = game.sim.treaty_offer(1).unwrap_or_default();
            self.act_passed = offer.pact == Some(sim::Pact::Alliance) && !offer.bundled();
            self.act_detail.push_str(&format!(
                " Pure alliance proposal available: {}.",
                self.act_passed
            ));
        }
        if frame == 136 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(*action);
            }
            let (unit, x, y) = self.diplomacy_probe.unwrap();
            let entered = game.sim.apply(Command::Move(unit, 1, 0));
            self.act_passed = entered
                && game.sim.pact(HUMAN, 1) == sim::Pact::Alliance
                && game
                    .sim
                    .cities()
                    .iter()
                    .any(|city| (city.x, city.y, city.owner) == (x, y, 1));
            self.act_detail.push_str(&format!(
                " Reviewed a pure alliance proposal and entered the allied city peacefully: {}.",
                self.act_passed
            ));
        }
        if frame == 138 && self.act_passed {
            game.sim.diplomacy_playtest_allied_sharing();
        }
        if frame == 141
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
        {
            game.perform_action(*action);
        }
        if frame == 144 && self.act_passed {
            let offer = game.sim.treaty_offer(1).unwrap_or_default();
            let human_gold = game.sim.players()[HUMAN].gold;
            let ai_gold = game.sim.players()[1].gold;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(*action);
            }
            let gold = offer.terms[0].gold;
            self.act_passed = offer.pact.is_none()
                && gold > 0
                && game.sim.players()[HUMAN].gold == human_gold - gold
                && game.sim.players()[1].gold == ai_gold + gold
                && game.sim.players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Writing")
                && game.sim.players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Pottery")
                && self
                    .vision_probe
                    .is_some_and(|(x, y)| game.sim.tile(x, y).visible);
            self.act_detail.push_str(&format!(
                " Established allies exchanged Pottery, Writing, {gold} gold, and live vision: {}.",
                self.act_passed
            ));
        }
        if frame == 145 && self.act_passed {
            game.sim.diplomacy_playtest_allied_exchange();
        }
        if frame == 146
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
        {
            game.perform_action(*action);
        }
        if frame == 147 && self.act_passed {
            let offer = game.sim.treaty_offer(1).unwrap_or_default();
            let baseline = game.sim.diplomacy_playtest_allied_trade_baseline();
            let gold = game.sim.players()[HUMAN].gold;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(*action);
            }
            let paid = offer.terms[1].gold;
            self.act_passed = offer.pact.is_none()
                && baseline.is_some_and(|base| paid > base)
                && game.sim.players()[HUMAN].gold == gold + paid
                && game.sim.players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Warrior Code")
                && game.sim.players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Currency");
            self.act_detail.push_str(&format!(" Allied AI paid Warrior Code plus {paid} gold for Currency, above the {baseline:?}-gold base difference with current embassy leakage using a controlled retained prior-phase research snapshot: {}.", self.act_passed));
        }
        if frame == 148 && self.act_passed {
            game.perform_action(UiAction::CancelPact(1));
            let alliance_cancelled = game.sim.pact(HUMAN, 1) != sim::Pact::Alliance;
            let explicit_vision_retained = self
                .vision_probe
                .is_some_and(|(x, y)| game.sim.tile(x, y).visible);
            game.sim.diplomacy_playtest_tick(1);
            let vision_revoked = self.vision_probe.is_some_and(|(x, y)| {
                let tile = game.sim.tile(x, y);
                tile.seen && !tile.visible
            });
            self.act_passed = alliance_cancelled && explicit_vision_retained && vision_revoked;
            self.act_detail.push_str(&format!(
                " Explicit vision survived alliance cancellation, then the AI revoked its own grant on its next diplomacy phase while map knowledge remained: pact cancelled {alliance_cancelled}, vision retained {explicit_vision_retained}, revoked {vision_revoked}: {}.",
                self.act_passed
            ));
            game.perform_action(UiAction::ToggleDiplomacy);
        }
    }
}
