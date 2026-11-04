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
            let expected_gold = player.gold + gold - sim::Simulation::building_upkeep(city);
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
