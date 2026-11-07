mod client;
mod mapgen;
mod playtest;
mod rules;
mod sim;
mod visual_test;

use client::{Assets, Game, Tileset, UiAction, window_conf};
use macroquad::prelude::*;
use std::{env, fs, path::Path};

const QUICKSAVE_PATH: &str = "civcraft.save";
const AUTOSAVE_PATH: &str = "civcraft-autosave.save";

fn parsed_argument(args: &[String], flag: &str) -> Result<Option<String>, String> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };
    args.get(index + 1)
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .map(Some)
        .ok_or_else(|| format!("{flag} requires a value"))
}
fn argument_value(flag: &str) -> Option<String> {
    match parsed_argument(&env::args().collect::<Vec<_>>(), flag) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn ensure_parent(path: &str) {
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let wrap_arg = argument_value("--wrap");
    let wrap = wrap_arg
        .as_deref()
        .unwrap_or("x")
        .parse::<sim::topology::MapWrap>()
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2);
        });
    let renderer_name = argument_value("--renderer").unwrap_or_else(|| "amplio2".to_owned());
    let renderer = client::create_renderer(&renderer_name).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let tileset_name = argument_value("--tileset").unwrap_or_else(|| "amplio2".to_owned());
    let theme = argument_value("--tileset-theme");
    let tileset = Tileset::select(&tileset_name, theme.as_deref()).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let screenshot_path = argument_value("--screenshot");
    let self_test_path = argument_value("--self-test");
    let ui_review = argument_value("--ui-review");
    let single_scene = argument_value("--ui-scene");
    if ui_review.is_some() && (self_test_path.is_some() || screenshot_path.is_some()) {
        eprintln!("--ui-review must run separately from --self-test and --screenshot");
        std::process::exit(2);
    }
    if single_scene.is_some() && ui_review.is_none() {
        eprintln!("--ui-scene requires --ui-review");
        std::process::exit(2);
    }
    let review_scenes: Vec<usize> = if let Some(name) = single_scene {
        match client::UI_REVIEW_SCENES
            .iter()
            .position(|scene| *scene == name)
        {
            Some(scene) => vec![scene],
            None => {
                eprintln!("Unknown UI scene: {name}");
                std::process::exit(2);
            }
        }
    } else {
        (0..client::UI_REVIEW_SCENES.len()).collect()
    };
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
            Ok(simulation) => {
                if wrap_arg.is_some() && simulation.map_wrap != wrap {
                    eprintln!(
                        "--wrap conflicts with the saved map's wrapping; start a new game to change topology"
                    );
                    std::process::exit(2);
                }
                Game::from_simulation_with_renderer(simulation, std::rc::Rc::clone(&renderer))
            }
            Err(error) => {
                eprintln!("Could not load campaign. {error}");
                std::process::exit(1);
            }
        }
    } else {
        Game::new_with_renderer(
            mapgen::generate_with_wrap(seed, wrap),
            std::rc::Rc::clone(&renderer),
        )
    };
    let assets = Assets::new(&tileset).unwrap_or_else(|error| {
        eprintln!("Could not load {} tileset. {error}", tileset.name());
        std::process::exit(2);
    });
    println!(
        "World seed: {} · wrap: {} · renderer: {} · tileset: {}",
        game.sim.world_seed,
        game.sim.map_wrap,
        renderer.name(),
        tileset.name()
    );
    game.autosave_enabled = !automated;
    game.intro_open = !automated && !args.iter().any(|arg| arg == "--load");
    if ui_review.is_some() {
        client::set_review_pointer(Some(vec2(-1.0, -1.0)));
    }
    let mut buttons: Vec<(Rect, UiAction)> = Vec::new();
    let mut frame = 0_u32;
    let mut ui_profile = ui_review
        .as_ref()
        .map(|_| visual_test::UiReviewProfile::new(game.sim.world_seed));
    let sources = ui_review.as_ref().map(|dir| {
        visual_test::UiReviewProfile::prepare_output(
            dir,
            review_scenes.iter().map(|&i| client::UI_REVIEW_SCENES[i]),
        )
        .expect("Prepare visual review output");
        let started = std::time::Instant::now();
        let sources = client::UiReviewSources::new(&game.sim);
        ui_profile
            .as_mut()
            .unwrap()
            .baseline_setup(started.elapsed().as_secs_f64() * 1000.0);
        sources
    });
    let mut sequence = client::UiReviewSequence::default();
    let mut changed_page = false;
    let mut playtest = self_test_path.clone().map(playtest::Playtest::new);

    loop {
        if !automated {
            game.handle_keyboard();
            game.handle_mouse(&buttons);
        }

        if let Some(playtest) = &mut playtest {
            playtest.advance(frame, &mut game, &buttons);
        }

        let scene = ui_review
            .as_ref()
            .map(|_| review_scenes[(frame / 10) as usize]);
        if let Some(scene) = scene {
            if frame.is_multiple_of(10) {
                changed_page = false;
                sequence = client::UiReviewSequence::default();
                let started = std::time::Instant::now();
                game.prepare_ui_review(scene, sources.as_ref().unwrap());
                ui_profile.as_mut().unwrap().setup(
                    scene,
                    client::UI_REVIEW_SCENES[scene],
                    started.elapsed().as_secs_f64() * 1000.0,
                );
            } else {
                let started = std::time::Instant::now();
                let result = game.step_ui_review(scene, frame % 10, &buttons, &mut sequence);
                let profile = ui_profile.as_mut().unwrap();
                profile.actions(scene, started.elapsed().as_secs_f64() * 1000.0);
                for (label, ok) in sequence.checks.drain(..) {
                    profile.check(scene, label, ok);
                }
                match result {
                    Ok(changed) => {
                        if frame % 10 == 6 {
                            changed_page = changed;
                        }
                    }
                    Err(error) => {
                        let _ = profile.finish(ui_review.as_ref().unwrap());
                        eprintln!(
                            "UI scenario {} failed: {error}",
                            client::UI_REVIEW_SCENES[scene]
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
        let draw_started = ui_profile.as_ref().map(|_| std::time::Instant::now());
        buttons = game.draw(&assets);
        if let (Some(scene), Some(started)) = (scene, draw_started) {
            let profile = ui_profile.as_mut().unwrap();
            profile.draw(
                scene,
                started.elapsed().as_secs_f64() * 1000.0,
                game.border_cache_stats(),
            );
            let valid = buttons.iter().all(|(rect, _)| {
                rect.x >= 0.0
                    && rect.y >= 0.0
                    && rect.right() <= screen_width() + 0.1
                    && rect.bottom() <= screen_height() + 0.1
            });
            profile.check(scene, "controls_inside_window", valid);
            if !valid {
                let _ = profile.finish(ui_review.as_ref().unwrap());
                eprintln!(
                    "UI controls outside window: {}",
                    client::UI_REVIEW_SCENES[scene]
                );
                std::process::exit(1);
            }
        }
        frame += 1;
        if let Some(dir) = &ui_review {
            let scene = scene.unwrap();
            let next = frame % 10 == 8;
            if frame % 10 == 5 || (next && changed_page) {
                let started = std::time::Instant::now();
                let image = get_screen_data();
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                image.bytes.hash(&mut hasher);
                let file = format!(
                    "{}{}.png",
                    client::UI_REVIEW_SCENES[scene],
                    if next { "-next" } else { "" }
                );
                image.export_png(
                    Path::new(dir)
                        .join(&file)
                        .to_str()
                        .expect("UTF-8 gallery path"),
                );
                let profile = ui_profile.as_mut().unwrap();
                if !profile.capture(
                    scene,
                    file,
                    started.elapsed().as_secs_f64() * 1000.0,
                    hasher.finish(),
                    next,
                ) {
                    let _ = profile.finish(dir);
                    eprintln!(
                        "UI page did not change pixels: {}",
                        client::UI_REVIEW_SCENES[scene]
                    );
                    std::process::exit(1);
                }
            }
            if frame >= review_scenes.len() as u32 * 10 {
                if !ui_profile
                    .as_ref()
                    .unwrap()
                    .finish(dir)
                    .expect("Write visual review report")
                {
                    std::process::exit(1);
                }
                break;
            }
        }
        if let Some(path) = &screenshot_path
            && frame == 120
        {
            ensure_parent(path);
            get_screen_data().export_png(path);
            if self_test_path.is_none() {
                break;
            }
        }
        if frame >= 805
            && let Some(playtest) = &playtest
        {
            if !playtest.finish(game.hud_elements_drawn) {
                std::process::exit(1);
            }
            break;
        }
        next_frame().await;
    }
}

#[cfg(test)]
mod argument_tests {
    use super::*;
    #[test]
    fn automation_flags_cannot_silently_fall_back_to_interactive_play() {
        for values in [
            vec!["game", "--ui-review"],
            vec!["game", "--ui-review", "--seed", "42"],
        ] {
            let args: Vec<String> = values.into_iter().map(str::to_owned).collect();
            assert!(parsed_argument(&args, "--ui-review").is_err());
        }
        let args: Vec<String> = ["game", "--ui-review", "/tmp/review"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            parsed_argument(&args, "--ui-review").unwrap(),
            Some("/tmp/review".into())
        );
        assert_eq!(parsed_argument(&args, "--self-test").unwrap(), None);
    }
}
