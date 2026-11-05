mod client;
mod mapgen;
mod playtest;
mod rules;
mod sim;

use client::{Assets, Game, UiAction, window_conf};
use macroquad::prelude::*;
use std::{env, fs, path::Path};

const QUICKSAVE_PATH: &str = "civcraft.save";
const AUTOSAVE_PATH: &str = "civcraft-autosave.save";

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
    let mut game = if let Some(index) = args.iter().position(|arg| arg == "--load") {
        let result = args
            .get(index + 1)
            .ok_or_else(|| "--load requires a save path".to_owned())
            .and_then(|path| {
                sim::Simulation::load_campaign(Path::new(path)).map_err(|error| error.to_string())
            });
        match result {
            Ok(simulation) => Game::from_simulation(simulation),
            Err(error) => {
                eprintln!("Could not load campaign. {error}");
                std::process::exit(1);
            }
        }
    } else {
        Game::new(mapgen::generate(seed))
    };
    println!("World seed: {}", game.sim.world_seed);
    game.autosave_enabled = !automated;
    let assets = Assets::new();
    let mut buttons: Vec<(Rect, UiAction)> = Vec::new();
    let mut frame = 0_u32;
    let mut playtest = self_test_path.clone().map(playtest::Playtest::new);

    loop {
        if !automated {
            game.handle_keyboard();
            game.handle_mouse(&buttons);
        }

        if let Some(playtest) = &mut playtest {
            playtest.advance(frame, &mut game, &buttons);
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
        if frame >= 805
            && let Some(playtest) = &playtest
        {
            playtest.finish(game.hud_elements_drawn);
            break;
        }
        next_frame().await;
    }
}
