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
            && let Some(unit) = game
                .sim
                .playtest_state()
                .units()
                .iter()
                .find(|unit| unit.owner == HUMAN && unit.kind != UnitKind::Settler)
                .cloned()
        {
            for direction in 0..CIV3_DIRS.len() {
                if game
                    .sim
                    .playtest_harness()
                    .apply(Command::MoveCiv3(unit.id, direction))
                {
                    if let Some(after) = game
                        .sim
                        .playtest_state()
                        .units()
                        .iter()
                        .find(|candidate| candidate.id == unit.id)
                    {
                        let distance = game
                            .sim
                            .playtest_state()
                            .map_distance(unit.x, unit.y, after.x, after.y);
                        self.move_passed = distance > 0;
                        self.move_detail = format!(
                            "{} moved {distance} tile from {},{} to {},{} using terrain movement costs",
                            unit.kind.name(),
                            unit.x,
                            unit.y,
                            after.x,
                            after.y
                        );
                    }
                    break;
                }
            }
        }

        if frame == 30
            && let Some(settler_id) = game
                .sim
                .playtest_state()
                .units()
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settler)
                .map(|u| u.id)
        {
            let cities_before = game.sim.playtest_state().cities().len();
            game.selected_unit = Some(settler_id);
            game.perform_action(UiAction::FoundCity);
            let cities_after = game.sim.playtest_state().cities().len();
            self.act_passed = cities_after == cities_before + 1;
            if self.act_passed {
                game.complete_playtest_presentation();
            }
            self.act_detail = if self.act_passed {
                "Settlers founded a city, claimed its workable tiles, and opened city production"
                    .to_owned()
            } else {
                "Settlers could not found a city at the scripted starting tile".to_owned()
            };
        }

        if (34..=54).contains(&frame) {
            self.advance_city_tiles(frame, game, buttons);
        }

        // Wait for the city panel and controls to be live before using them.
        if (43..=54).contains(&frame)
            && self.city_tiles_complete
            && !self.city_manager_closed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CloseCity))
        {
            game.perform_action(action.clone());
            self.city_manager_closed = true;
        }

        if (45..=54).contains(&frame)
            && self.city_manager_closed
            && !self.tax_step_complete
            && let Some((_, rate_button)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::RaiseRate(1)))
        {
            game.perform_action(rate_button.clone());
            let rate_changed = game.sim.playtest_state().players()[HUMAN].rates == [50, 50, 0];
            let city = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let trade = game.sim.playtest_state().city_yields(city).2;
            let [science, gold, _] = game.sim.playtest_state().city_trade(city, trade);
            let player = &game.sim.playtest_state().players()[HUMAN];
            let expected_gold =
                player.gold + gold - game.sim.playtest_state().building_upkeep(city);
            let expected_research = player.research + science;
            game.sim.playtest_harness().apply(Command::EndTurn);
            let player = &game.sim.playtest_state().players()[HUMAN];
            self.act_passed = rate_changed
                && player.gold == expected_gold
                && player.research == expected_research;
            self.act_detail.push_str(&format!(
                ". Used the tax control and ended the turn. Tax rate changed to the expected 50/50 split: {rate_changed}. Treasury {} (expected {expected_gold}), research {} (expected {expected_research}).",
                player.gold,
                player.research
            ));
            self.tax_step_complete = true;
        }
        if frame == 54 && !self.tax_step_complete {
            self.act_passed = false;
            self.act_detail
                .push_str(". The city closed, but the tax control did not become available.");
            self.tax_step_complete = true;
        }
        if (53..=54).contains(&frame)
            && self.tax_step_complete
            && self.act_passed
            && !self.city_focus_selected
            && let Some(city) = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
        {
            game.select_at(city.x, city.y);
            self.city_focus_selected = true;
        }

        // The city manager is modal: use its close control before touching HUD rates.
        if frame == 76
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CloseCity))
        {
            game.perform_action(action.clone());
        }

        if frame == 55 && self.act_passed {
            let city = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let city_id = city.id;
            let before = game.sim.playtest_state().city_yields(city);
            if let Some((_, action)) = buttons.iter().find(
                |(_, action)| matches!(action, UiAction::Specialist(id, 0, 1) if *id == city_id),
            ) {
                game.perform_action(action.clone());
            }
            let city = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.id == city_id)
                .unwrap();
            let after = game.sim.playtest_state().city_yields(city);
            let science = game.sim.playtest_state().city_trade(city, 0)[0];
            self.act_passed = city.specialists == [1, 0, 0] && before != after && science == 2;
            self.act_detail.push_str(&format!(
                " Scientist replaced a tile worker and provided {science} science."
            ));
        }

        if frame == 65 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Specialist(_, 0, -1)))
            {
                game.perform_action(action.clone());
            }
            let mut isolated = game.sim.isolated_playtest_copy();
            let verified = isolated.playtest_harness().despotism_upkeep_playtest();
            self.results.push((
                "unit_upkeep",
                verified,
                format!(
                    "Under Despotism, the third unit housed in a city exceeded C2C3's two free gold-upkeep slots and was charged one gold: {verified}."
                ),
            ));
        }

        if frame == 78 && self.act_passed {
            game.sim.playtest_harness().government_ui_playtest_setup();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleGovernment))
            {
                game.perform_action(action.clone());
            }
        }
        if frame == 83 && self.act_passed {
            use sim::Government;
            let locked = !buttons.iter().any(|(_, action)| {
                matches!(action, UiAction::ChangeGovernment(Government::Monarchy))
            });
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::ChangeGovernment(Government::Despotism))
            }) {
                game.perform_action(action.clone());
            }
            let player = &game.sim.playtest_state().players()[HUMAN];
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
            let finish = game.sim.playtest_state().players()[HUMAN]
                .revolution_turn
                .unwrap();
            game.perform_action(UiAction::ToggleGovernment);
            while game.sim.playtest_state().turn() < finish
                && game.sim.playtest_harness().apply(Command::EndTurn)
            {}
            let city = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let upkeep = game.sim.playtest_state().city_unit_upkeep(city);
            let [_, gold, luxury] = game
                .sim
                .playtest_state()
                .city_trade(city, game.sim.playtest_state().city_yields(city).2);
            let revenue = if game.sim.playtest_state().city_in_disorder(city, luxury) {
                0
            } else {
                gold
            };
            let treasury = game.sim.playtest_state().players()[HUMAN].gold + revenue
                - game.sim.playtest_state().building_upkeep(city)
                - upkeep.gold;
            let adopted =
                game.sim.playtest_state().players()[HUMAN].government == Government::Despotism;
            game.sim.playtest_harness().apply(Command::EndTurn);
            self.act_passed = adopted
                && upkeep.shields == 0
                && game.sim.playtest_state().players()[HUMAN].gold == treasury;
            self.act_detail.push_str(&format!(
                " Despotism applied {} gold and {} shield unit upkeep and settled the treasury correctly: {}.",
                upkeep.gold, upkeep.shields, self.act_passed
            ));
        }

        if frame == 98 && self.act_passed {
            if let Some((unit, direction)) = game.sim.playtest_harness().civil_war_playtest() {
                game.selected_unit = Some(unit);
                self.civil_war_attack_direction = Some(direction);
            } else {
                game.selected_unit = None;
                self.civil_war_attack_direction = None;
            }
            self.act_passed = game.selected_unit.is_some();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleDiplomacy))
            {
                game.perform_action(action.clone());
            }
        }
        if frame == 101 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::OpenDiplomacyWith(1)))
            {
                game.perform_action(action.clone());
            }
            self.act_passed = game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::War;
        }
        if frame == 102 && self.act_passed {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::ProposeTreaty(1, offer)
                    if offer.pact == Some(sim::Pact::Ceasefire))
            }) {
                game.perform_action(action.clone());
            }
            self.act_passed = game.sim.playtest_state().pact_turns(HUMAN, 1) == 16;
        }
        if frame == 105 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(action.clone());
            }
            let ceasefire_turns = game.sim.playtest_state().pact_turns(HUMAN, 1);
            let attack_blocked = !game.sim.playtest_harness().apply(Command::MoveCiv3(
                game.selected_unit.unwrap(),
                self.civil_war_attack_direction.unwrap(),
            ));
            self.act_passed = ceasefire_turns == 16 && attack_blocked;
            self.act_detail.push_str(&format!(
                " Senate veto preserved the {ceasefire_turns}-turn ceasefire and blocked the attack: {attack_blocked}."
            ));
        }
        if frame == 115 && self.act_passed {
            game.sim
                .playtest_harness()
                .apply(Command::Government(sim::Government::Despotism));
            let cancellation = game.sim.playtest_harness().apply(Command::CancelPact(1));
            game.perform_action(UiAction::ToggleDiplomacy);
            game.sim
                .playtest_harness()
                .civil_war_playtest_seed_capture();
            let rebel = game.sim.playtest_state().players().len();
            let moved = game.sim.playtest_harness().apply(Command::MoveCiv3(
                game.selected_unit.unwrap(),
                self.civil_war_attack_direction.unwrap(),
            ));
            let cities = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .filter(|city| city.owner == rebel)
                .count();
            let rival_cities = game
                .sim
                .playtest_state()
                .cities()
                .iter()
                .filter(|city| city.owner == 1)
                .count();
            self.act_passed = moved
                && cities == 4
                && game.sim.playtest_state().players()[1].government == sim::Government::Anarchy;
            if self.act_passed {
                game.sim
                    .playtest_harness()
                    .civil_war_playtest_resume_diplomacy();
                game.sim.playtest_harness().apply(Command::EndTurn);
                self.act_passed = game.sim.playtest_state().players()[1].government
                    == sim::Government::Despotism
                    && game
                        .sim
                        .playtest_state()
                        .cities()
                        .iter()
                        .any(|city| city.owner == rebel);
            }
            self.act_detail.push_str(&format!(" Revolution lifted the veto; pact cancellation {cancellation}, capital move {moved}, remaining rival cities {rival_cities}, rebel cities {cities}: {}.", self.act_passed));
        }

        if frame == 122 && self.act_passed {
            game.perform_action(UiAction::ToggleDiplomacy);
        }
        if frame == 125 && self.act_passed {
            game.perform_action(UiAction::OpenDiplomacyWith(1));
            let refused_early_peace = !game
                .sim
                .playtest_harness()
                .apply(Command::Negotiate(1, sim::Pact::Peace));
            self.act_passed = refused_early_peace
                && game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Ceasefire;
            let peace_refused = refused_early_peace;
            let ceasefire_held = game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Ceasefire;
            game.sim
                .playtest_harness()
                .diplomacy_playtest_technologies(false);
            game.sim.playtest_harness().diplomacy_playtest_friendship();
            game.sim.playtest_harness().diplomacy_playtest_tick(12);
            self.act_detail.push_str(&format!(
                " Diplomacy setup: early peace refused {peace_refused}, ceasefire held {ceasefire_held}, AI offer {:?}.",
                game.sim.playtest_state().pact_offer(1)
            ));
        }
        if frame == 128 && self.act_passed {
            let treaty_offer = game.sim.playtest_state().treaty_offer(1);
            let offered = treaty_offer
                .as_ref()
                .is_some_and(|offer| offer.pact == Some(sim::Pact::Peace));
            if let Some(offer) = treaty_offer.as_ref() {
                game.perform_action(UiAction::AcceptOffer(1, offer.clone()));
            }
            self.act_passed =
                offered && game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Armistice;
            let accepted_offer = game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Armistice;
            let acceptance_event = game
                .sim
                .playtest_state()
                .events()
                .last()
                .cloned()
                .unwrap_or_default();
            game.sim
                .playtest_harness()
                .diplomacy_playtest_technologies(true);
            game.sim.playtest_harness().diplomacy_playtest_tick(16);
            let matured = game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Peace;
            self.diplomacy_probe = game.sim.playtest_harness().diplomacy_playtest_unit(1);
            game.sim
                .playtest_harness()
                .diplomacy_playtest_alliance_offer_setup();
            game.sim.playtest_harness().diplomacy_playtest_tick(1);
            let peace_held = game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Peace;
            let entry_blocked = self.diplomacy_probe.is_some_and(|(unit, _, _, direction)| {
                !game
                    .sim
                    .playtest_harness()
                    .apply(Command::MoveCiv3(unit, direction))
            });
            self.act_passed &= peace_held && entry_blocked;
            self.act_detail.push_str(&format!(" AI refused early peace, then offered it after the ceasefire held. Offer presented {offered} ({treaty_offer:?}), accepted {accepted_offer} ({acceptance_event}), armistice matured {matured}, peace held {peace_held}, peace blocked entry {entry_blocked}: {}.", self.act_passed));
        }
        if frame == 132 && self.act_passed {
            self.vision_probe = game
                .sim
                .playtest_harness()
                .diplomacy_playtest_hidden_city(1);
            let offer = game
                .sim
                .playtest_state()
                .treaty_offer(1)
                .unwrap_or_default();
            self.act_passed = offer.pact == Some(sim::Pact::Alliance) && !offer.bundled();
            let diagnostics = if self.act_passed {
                String::new()
            } else {
                game.sim
                    .playtest_harness()
                    .diplomacy_playtest_alliance_offer_diagnostics()
            };
            self.act_detail.push_str(&format!(
                " Pure alliance proposal available: {} (offer {offer:?}, hidden-city probe: {}, {diagnostics}).",
                self.act_passed,
                self.vision_probe.is_some()
            ));
        }
        if frame == 136 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(action.clone());
            }
            let (unit, x, y, direction) = self.diplomacy_probe.unwrap();
            let entered = game
                .sim
                .playtest_harness()
                .apply(Command::MoveCiv3(unit, direction));
            self.act_passed = entered
                && game.sim.playtest_state().pact(HUMAN, 1) == sim::Pact::Alliance
                && game
                    .sim
                    .playtest_state()
                    .cities()
                    .iter()
                    .any(|city| (city.x, city.y, city.owner) == (x, y, 1));
            self.act_detail.push_str(&format!(
                " Reviewed a pure alliance proposal and entered the allied city peacefully: {}.",
                self.act_passed
            ));
        }
        if frame == 138 && self.act_passed {
            game.sim
                .playtest_harness()
                .diplomacy_playtest_allied_sharing();
        }
        if frame == 141
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
        {
            game.perform_action(action.clone());
        }
        if frame == 144 && self.act_passed {
            let offer = game
                .sim
                .playtest_state()
                .treaty_offer(1)
                .unwrap_or_default();
            let human_gold = game.sim.playtest_state().players()[HUMAN].gold;
            let ai_gold = game.sim.playtest_state().players()[1].gold;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(action.clone());
            }
            let gold = offer.terms[0].gold;
            self.act_passed = offer.pact.is_none()
                && gold > 0
                && game.sim.playtest_state().players()[HUMAN].gold == human_gold - gold
                && game.sim.playtest_state().players()[1].gold == ai_gold + gold
                && game.sim.playtest_state().players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Writing")
                && game.sim.playtest_state().players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Pottery")
                && self
                    .vision_probe
                    .is_some_and(|(x, y)| game.sim.playtest_state().tile(x, y).visible);
            self.act_detail.push_str(&format!(
                " Established allies exchanged Pottery, Writing, {gold} gold, and live vision: {}.",
                self.act_passed
            ));
        }
        if frame == 145 && self.act_passed {
            game.sim
                .playtest_harness()
                .diplomacy_playtest_allied_exchange();
        }
        if frame == 146
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
        {
            game.perform_action(action.clone());
        }
        if frame == 147 && self.act_passed {
            let offer = game
                .sim
                .playtest_state()
                .treaty_offer(1)
                .unwrap_or_default();
            let baseline = game
                .sim
                .playtest_harness()
                .diplomacy_playtest_allied_trade_baseline();
            let gold = game.sim.playtest_state().players()[HUMAN].gold;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(action.clone());
            }
            let paid = offer.terms[1].gold;
            self.act_passed = offer.pact.is_none()
                && baseline.is_some_and(|base| paid > base)
                && game.sim.playtest_state().players()[HUMAN].gold == gold + paid
                && game.sim.playtest_state().players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Warrior Code")
                && game.sim.playtest_state().players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Currency");
            self.act_detail.push_str(&format!(" Allied AI paid Warrior Code plus {paid} gold for Currency, above the {baseline:?}-gold base difference with current embassy leakage using a controlled retained prior-phase research snapshot: {}.", self.act_passed));
        }
        if frame == 148 && self.act_passed {
            game.perform_action(UiAction::CancelPact(1));
            let alliance_cancelled =
                game.sim.playtest_state().pact(HUMAN, 1) != sim::Pact::Alliance;
            let explicit_vision_retained = self
                .vision_probe
                .is_some_and(|(x, y)| game.sim.playtest_state().tile(x, y).visible);
            game.sim.playtest_harness().diplomacy_playtest_tick(1);
            let vision_revoked = self.vision_probe.is_some_and(|(x, y)| {
                let tile = game.sim.playtest_state().tile(x, y);
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

    fn advance_city_tiles(&mut self, frame: u32, game: &mut Game, buttons: &[(Rect, UiAction)]) {
        if self.city_tiles_complete || !self.act_passed {
            return;
        }
        let Some(city) = game
            .sim
            .playtest_state()
            .cities()
            .iter()
            .find(|city| city.owner == HUMAN)
        else {
            return;
        };
        let city_id = city.id;

        match self.city_tile_reassigned {
            None => {
                let tile = city.worked_tiles.first().copied();
                let tile_action = tile.and_then(|(tile_x, tile_y)| {
                    buttons.iter().find(|(_, action)| {
                        matches!(
                            action,
                            UiAction::WorkCityTile(id, x, y)
                                if *id == city_id && (*x, *y) == (tile_x, tile_y)
                        )
                    })
                });
                if let Some((_, action)) = tile_action {
                    game.perform_action(action.clone());
                    let reassigned = game
                        .sim
                        .playtest_state()
                        .cities()
                        .iter()
                        .find(|city| city.id == city_id)
                        .is_some_and(|city| {
                            city.worked_tiles.is_empty() && city.specialists[2] == 1
                        });
                    self.city_tile_reassigned = Some(reassigned);
                } else if let Some((_, action)) = buttons
                    .iter()
                    .find(|(_, action)| matches!(action, UiAction::CityTiles))
                {
                    game.perform_action(action.clone());
                }
            }
            Some(true) => {
                if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                    matches!(action, UiAction::Specialist(id, 2, -1) if *id == city_id)
                }) {
                    game.perform_action(action.clone());
                    let restored = game
                        .sim
                        .playtest_state()
                        .cities()
                        .iter()
                        .find(|city| city.id == city_id)
                        .is_some_and(|city| {
                            city.worked_tiles.len() == 1 && city.specialists[2] == 0
                        });
                    self.results.push((
                        "city_tiles",
                        restored,
                        format!(
                            "Worked tile reassigned to entertainer: true. Tile labor restored: {restored}."
                        ),
                    ));
                    self.city_tiles_complete = true;
                    if let Some((_, action)) = buttons
                        .iter()
                        .find(|(_, action)| matches!(action, UiAction::CityTiles))
                    {
                        game.perform_action(action.clone());
                    }
                }
            }
            Some(false) => {
                self.results.push((
                    "city_tiles",
                    false,
                    "The worked tile control did not reassign its citizen to an entertainer."
                        .to_owned(),
                ));
                self.city_tiles_complete = true;
            }
        }

        if frame == 54 && !self.city_tiles_complete {
            let reassigned = self.city_tile_reassigned.unwrap_or(false);
            self.results.push((
                "city_tiles",
                false,
                format!(
                    "Timed out waiting for the city tile controls. Reassignment completed: {reassigned}. Tile labor restored: false."
                ),
            ));
            self.city_tiles_complete = true;
        }
    }
}
