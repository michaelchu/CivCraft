mod client;
mod mapgen;
mod rules;
mod sim;

use client::{Assets, Game, UiAction, window_conf};
use macroquad::prelude::*;
use sim::{Command, HUMAN, UnitKind, topology::ISOHEX_DIRS};
use std::{env, fs, path::Path};

fn argument_value(flag: &str) -> Option<String> {
    let args: Vec<String> = env::args().collect();
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn ensure_parent(path: &str) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn write_self_test(
    path: &str,
    move_passed: bool,
    move_detail: &str,
    act_passed: bool,
    act_detail: &str,
    hud_passed: bool,
    hud_detail: &str,
) {
    ensure_parent(path);
    let json = format!(
        "{{\"version\":1,\"steps\":[{{\"step\":\"move\",\"passed\":{},\"detail\":\"{}\"}},{{\"step\":\"act\",\"passed\":{},\"detail\":\"{}\"}},{{\"step\":\"hud\",\"passed\":{},\"detail\":\"{}\"}}]}}",
        move_passed,
        json_escape(move_detail),
        act_passed,
        json_escape(act_detail),
        hud_passed,
        json_escape(hud_detail),
    );
    let _ = fs::write(path, json);
}

#[macroquad::main(window_conf)]
async fn main() {
    let screenshot_path = argument_value("--screenshot");
    let self_test_path = argument_value("--self-test");
    let automated = screenshot_path.is_some() || self_test_path.is_some();
    let args: Vec<_> = env::args().collect();
    let seed = if let Some(index) = args.iter().position(|arg| arg == "--seed") {
        args.get(index + 1)
            .and_then(|arg| arg.parse::<u64>().ok())
            .unwrap_or_else(|| {
                eprintln!("--seed requires a nonnegative 64-bit integer");
                std::process::exit(1);
            })
    } else {
        mapgen::random_seed()
    };
    println!("World seed: {seed}");
    let map = mapgen::generate(seed);
    let assets = Assets::new();
    let mut game = Game::new(map);
    let mut buttons: Vec<(Rect, UiAction)> = Vec::new();
    let mut frame = 0_u32;
    let mut move_passed = false;
    let mut move_detail = "No scripted move completed".to_owned();
    let mut act_passed = false;
    let mut act_detail = "No scripted action completed".to_owned();

    let mut diplomacy_probe = None;
    let mut vision_probe = None;

    loop {
        if !automated {
            game.handle_keyboard();
            game.handle_mouse(&buttons);
        }

        if self_test_path.is_some() && frame == 12 {
            if let Some(explorer) = game
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
                            move_passed = distance > 0;
                            move_detail = format!(
                                "Explorer moved {distance} tile from {},{} to {},{} using terrain movement costs",
                                explorer.x, explorer.y, after.x, after.y
                            );
                        }
                        break;
                    }
                }
            }
        }

        if self_test_path.is_some() && frame == 30 {
            if let Some(settler_id) = game
                .sim
                .units()
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
                .map(|u| u.id)
            {
                let cities_before = game.sim.cities().len();
                game.selected_unit = Some(settler_id);
                game.perform_action(UiAction::FoundCity);
                let cities_after = game.sim.cities().len();
                act_passed = cities_after == cities_before + 1;
                act_detail = if act_passed {
                    "Settlers founded a city, claimed its workable tiles, and opened city production"
                        .to_owned()
                } else {
                    "Settlers could not found a city at the scripted starting tile".to_owned()
                };
            }
        }

        if self_test_path.is_some() && frame == 45 && act_passed {
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
            act_passed = rate_changed
                && player.gold == expected_gold
                && player.research == expected_research;
            act_detail = format!(
                "Founded a city, used the tax control, and ended the turn. Treasury {} (expected {expected_gold}), research {} (expected {expected_research}).",
                player.gold, player.research
            );
        }

        if self_test_path.is_some() && frame == 55 && act_passed {
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
            act_passed = city.specialists == [1, 0, 0] && before != after && science == 2;
            act_detail.push_str(&format!(
                " Scientist replaced a tile worker and provided {science} science."
            ));
        }

        if self_test_path.is_some() && frame == 65 && act_passed {
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
            act_passed = verified;
            act_detail.push_str(&format!(
                " Third supported unit charged one shield on the next turn: {verified}."
            ));
        }

        if self_test_path.is_some() && frame == 78 && act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleGovernment))
            {
                game.perform_action(*action);
            }
        }
        if self_test_path.is_some() && frame == 83 && act_passed {
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
            act_passed = locked
                && player.government == Government::Anarchy
                && player.revolution_target == Some(Government::Despotism);
            act_detail.push_str(&format!(
                " Government panel enforced the tech lock and started Anarchy: {act_passed}."
            ));
        }
        if self_test_path.is_some() && frame == 90 && act_passed {
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
            act_passed = adopted
                && upkeep.shields == 0
                && upkeep.gold > 0
                && game.sim.players()[HUMAN].gold == treasury;
            act_detail.push_str(&format!(
                " Despotism charged {} gold and {} shields in unit upkeep: {act_passed}.",
                upkeep.gold, upkeep.shields
            ));
        }

        if self_test_path.is_some() && frame == 98 && act_passed {
            game.selected_unit = game.sim.civil_war_playtest();
            act_passed = game.selected_unit.is_some();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleDiplomacy))
            {
                game.perform_action(*action);
            }
        }
        if self_test_path.is_some() && frame == 101 && act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Negotiate(1, sim::Pact::Ceasefire)))
            {
                game.perform_action(*action);
            }
            act_passed = game.sim.pact_turns(HUMAN, 1) == 16;
        }
        if self_test_path.is_some() && frame == 105 && act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            act_passed = game.sim.pact_turns(HUMAN, 1) == 16
                && !game
                    .sim
                    .apply(Command::Move(game.selected_unit.unwrap(), 1, 0));
            act_detail.push_str(&format!(
                " Senate veto preserved the ceasefire and blocked the attack: {act_passed}."
            ));
        }
        if self_test_path.is_some() && frame == 115 && act_passed {
            game.sim.apply(Command::Government(sim::Government::Tribal));
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            game.perform_action(UiAction::ToggleDiplomacy);
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
            act_passed = moved
                && cities == 4
                && game.sim.players()[1].government == sim::Government::Anarchy;
            if act_passed {
                game.sim.apply(Command::EndTurn);
                act_passed = game.sim.players()[1].government == sim::Government::Tribal
                    && game.sim.cities().iter().any(|city| city.owner == rebel);
            }
            act_detail.push_str(&format!(" Revolution lifted the veto and capital capture split off {cities} rebel cities: {act_passed}."));
        }

        if self_test_path.is_some() && frame == 122 && act_passed {
            game.perform_action(UiAction::ToggleDiplomacy);
        }
        if self_test_path.is_some() && frame == 125 && act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Negotiate(1, sim::Pact::Ceasefire)))
            {
                game.perform_action(*action);
            }
            let refused_early_peace = !game.sim.apply(Command::Negotiate(1, sim::Pact::Peace));
            act_passed = refused_early_peace && game.sim.pact(HUMAN, 1) == sim::Pact::Ceasefire;
            game.sim.diplomacy_playtest_technologies(false);
            game.sim.diplomacy_playtest_tick(12);
        }
        if self_test_path.is_some() && frame == 128 && act_passed {
            let offered = game.sim.pact_offer(1) == Some(sim::Pact::Peace);
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Negotiate(1, sim::Pact::Peace)))
            {
                game.perform_action(*action);
            }
            act_passed = offered && game.sim.pact(HUMAN, 1) == sim::Pact::Armistice;
            game.sim.diplomacy_playtest_technologies(true);
            game.sim.diplomacy_playtest_tick(16);
            diplomacy_probe = game.sim.diplomacy_playtest_unit(1);
            act_passed &= game.sim.pact(HUMAN, 1) == sim::Pact::Peace
                && diplomacy_probe
                    .is_some_and(|(unit, _, _)| !game.sim.apply(Command::Move(unit, 1, 0)));
            act_detail.push_str(&format!(" AI refused early peace, then offered it after the ceasefire held. Armistice matured and peace blocked entry: {act_passed}."));
        }
        if self_test_path.is_some() && frame == 132 && act_passed {
            vision_probe = game.sim.diplomacy_playtest_hidden_city(1);
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
            {
                game.perform_action(*action);
            }
        }
        if self_test_path.is_some() && frame == 136 && act_passed {
            let human_gold = game.sim.players()[HUMAN].gold;
            let ai_gold = game.sim.players()[1].gold;
            let offer = game.sim.treaty_offer(1).unwrap_or_default();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(*action);
            }
            let (unit, x, y) = diplomacy_probe.unwrap();
            let entered = game.sim.apply(Command::Move(unit, 1, 0));
            let stayed_allied = game
                .sim
                .cities()
                .iter()
                .any(|city| (city.x, city.y, city.owner) == (x, y, 1));
            let gold = offer.terms[0].gold;
            let exchanged = gold > 0
                && game.sim.players()[HUMAN].gold == human_gold - gold
                && game.sim.players()[1].gold == ai_gold + gold
                && game.sim.players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Writing")
                && game.sim.players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Pottery");
            let revealed = vision_probe.is_some_and(|(x, y)| game.sim.tile(x, y).visible);
            act_passed = entered
                && stayed_allied
                && game.sim.pact(HUMAN, 1) == sim::Pact::Alliance
                && exchanged
                && revealed;
            act_detail.push_str(&format!(" Reviewed and accepted an alliance bundle with Pottery, Writing, {gold} gold and live vision: {act_passed}."));
        }
        if self_test_path.is_some() && frame == 141 && act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            act_passed = vision_probe.is_some_and(|(x, y)| {
                let tile = game.sim.tile(x, y);
                tile.seen && !tile.visible
            });
            act_detail.push_str(&format!(
                " Ending the alliance removed live vision and retained map knowledge: {act_passed}."
            ));
            game.perform_action(UiAction::ToggleDiplomacy);
        }

        buttons = game.draw(&assets);
        frame += 1;

        if let Some(path) = &screenshot_path {
            if frame == 120 {
                ensure_parent(path);
                get_screen_data().export_png(path);
                if self_test_path.is_none() {
                    break;
                }
            }
        }
        if let Some(path) = &self_test_path {
            if frame >= 150 {
                let hud_passed = game.hud_elements_drawn >= 4;
                let hud_detail = format!(
                    "Drew {} HUD regions: turn and research, world map, selection details, commands and chronicle",
                    game.hud_elements_drawn
                );
                write_self_test(
                    path,
                    move_passed,
                    &move_detail,
                    act_passed,
                    &act_detail,
                    hud_passed,
                    &hud_detail,
                );
                break;
            }
        }
        next_frame().await;
    }
}
