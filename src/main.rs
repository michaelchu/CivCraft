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
    let ui_review = argument_value("--ui-review");
    let automated = screenshot_path.is_some() || self_test_path.is_some() || ui_review.is_some();
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

        if ui_review.is_some() && frame % 10 == 0 {
            game.prepare_ui_review((frame / 10) as usize, seed);
        }
        if ui_review.is_some() && frame % 10 == 6 {
            game.advance_ui_review((frame / 10) as usize, &buttons);
        }
        buttons = game.draw(&assets);
        if ui_review.is_some() {
            for (rect, _) in &buttons {
                assert!(
                    rect.x >= 0.0
                        && rect.y >= 0.0
                        && rect.right() <= screen_width() + 0.1
                        && rect.bottom() <= screen_height() + 0.1,
                    "UI control outside window in {}: {:?}",
                    client::UI_REVIEW_SCENES[(frame / 10) as usize],
                    rect
                );
            }
        }
        frame += 1;

        if let Some(dir) = &ui_review {
            if frame % 10 == 5 {
                let name = client::UI_REVIEW_SCENES[(frame / 10) as usize];
                let path = format!("{dir}/{name}.png");
                ensure_parent(&path);
                get_screen_data().export_png(&path);
            }
            if frame % 10 == 8 {
                let name = client::UI_REVIEW_SCENES[(frame / 10) as usize];
                let path = Path::new(dir).join(format!("{name}-next.png"));
                get_screen_data().export_png(path.to_str().expect("UTF-8 gallery path"));
            }
            if frame >= client::UI_REVIEW_SCENES.len() as u32 * 10 {
                break;
            }
        }
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
