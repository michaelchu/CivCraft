//! Application entry point and runtime coordinator for CivCraft.
//!
//! Startup parses command-line options, chooses the map topology and
//! tileset, then either creates a new [`sim::Simulation`] or restores one from
//! a campaign save. The same frame loop serves interactive play, screenshots,
//! desktop playtests, and UI review captures; automated modes disable ordinary
//! input and autosaving so a run has reproducible side effects.
//!
//! Keep mode validation and shared initialization here. Gameplay rules belong
//! in `sim`, while rendering and user interaction belong in `client`.

use crate::{client, ensure_parent, mapgen, playtest, sim, visual_test};

use crate::client::{Assets, Game, MusicPlayer, Tileset, UiAction, UnitSoundPlayer};
use macroquad::prelude::*;
use std::{env, path::Path};

/// Reads the value following `flag`, reporting an error when the flag has no value.
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
/// Reads one command-line option and exits with a usage error if it is malformed.
fn argument_value(flag: &str) -> Option<String> {
    match parsed_argument(&env::args().collect::<Vec<_>>(), flag) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn parse_map_size(value: &str) -> Result<mapgen::Civ3MapSize, String> {
    value.parse()
}

/// Starts the game or one of its automated capture and playtest modes.
pub async fn run_desktop() {
    if env::args().any(|arg| arg == "--wrap") {
        eprintln!("--wrap has been removed; CivCraft uses the Civ III horizontal map topology.");
        std::process::exit(2);
    }
    if env::args().any(|arg| arg == "--tileset-theme") {
        eprintln!("--tileset-theme has been removed; choose a built-in tileset with --tileset.");
        std::process::exit(2);
    }
    // Civ III campaigns use a horizontally wrapped map with bounded poles.
    // Dimensions can be selected independently of wrapping so generation,
    // simulation storage, and rendering all receive one consistent topology.
    let map_size = argument_value("--map-size")
        .as_deref()
        .map(parse_map_size)
        .transpose()
        .unwrap_or_else(|error| {
            eprintln!("Invalid --map-size value: {error}");
            std::process::exit(2);
        })
        .unwrap_or(mapgen::Civ3MapSize::Small);
    map_size.topology().unwrap_or_else(|error| {
        eprintln!("Invalid --map-size value: {error}");
        std::process::exit(2);
    });
    let ocean_coverage = argument_value("--ocean-coverage")
        .unwrap_or_else(|| "70".to_owned())
        .parse::<mapgen::OceanCoverage>()
        .unwrap_or_else(|error| {
            eprintln!("Invalid --ocean-coverage value: {error}");
            std::process::exit(2);
        });
    let climate = argument_value("--climate")
        .unwrap_or_else(|| "normal".to_owned())
        .parse::<mapgen::Climate>()
        .unwrap_or_else(|error| {
            eprintln!("Invalid --climate value: {error}");
            std::process::exit(2);
        });
    let age = argument_value("--world-age")
        .unwrap_or_else(|| "4".to_owned())
        .parse::<mapgen::WorldAge>()
        .unwrap_or_else(|error| {
            eprintln!("Invalid --world-age value: {error}");
            std::process::exit(2);
        });
    let landform = argument_value("--landform")
        .unwrap_or_else(|| "continents".to_owned())
        .parse::<mapgen::Landform>()
        .unwrap_or_else(|error| {
            eprintln!("Invalid --landform value: {error}");
            std::process::exit(2);
        });
    let temperature = argument_value("--temperature")
        .unwrap_or_else(|| "temperate".to_owned())
        .parse::<mapgen::Temperature>()
        .unwrap_or_else(|error| {
            eprintln!("Invalid --temperature value: {error}");
            std::process::exit(2);
        });
    let world_settings = mapgen::Civ3WorldSettings {
        ocean_coverage,
        climate,
        age,
        landform,
        temperature,
    };
    let tileset_name = argument_value("--tileset").unwrap_or_else(|| "civ3".to_owned());
    let tileset = Tileset::select(&tileset_name).unwrap_or_else(|error| {
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
            Ok(simulation) => Game::from_simulation(simulation),
            Err(error) => {
                eprintln!("Could not load campaign. {error}");
                std::process::exit(1);
            }
        }
    } else {
        let generated = match argument_value("--map-generator").as_deref() {
            None | Some("fractal") => {
                mapgen::generate_for_civ3_size(seed, map_size, world_settings)
            }
            Some("opensimplex") => mapgen::generate_for_civ3_size_with(
                &mapgen::OpenSimplexMapGenerator,
                seed,
                map_size,
                world_settings,
            ),
            Some(value) => {
                eprintln!(
                    "Invalid --map-generator value '{value}'; expected fractal or opensimplex"
                );
                std::process::exit(2);
            }
        };
        Game::from_simulation(sim::Simulation::new(generated))
    };
    let assets = Assets::new(&tileset).unwrap_or_else(|error| {
        eprintln!("Could not load {} tileset. {error}", tileset.name());
        std::process::exit(2);
    });
    println!(
        "World seed: {} · map size: {} ({}×{}) · wrap: {} · renderer: Civ III · tileset: {}",
        game.world_seed(),
        if args.iter().any(|arg| arg == "--load") {
            format!("{}x{}", game.map_wrap().width(), game.map_wrap().height())
        } else {
            map_size.to_string()
        },
        game.map_wrap().width(),
        game.map_wrap().height(),
        game.map_wrap(),
        tileset.name()
    );
    game.autosave_enabled = !automated;
    let mut music = MusicPlayer::new(!automated);
    let mut unit_sounds = UnitSoundPlayer::new(!automated);
    if ui_review.is_some() {
        client::set_review_pointer(Some(vec2(-1.0, -1.0)));
    }
    let mut buttons: Vec<(Rect, UiAction)> = Vec::new();
    let mut frame = 0_u32;
    let mut ui_profile = ui_review
        .as_ref()
        .map(|_| visual_test::UiReviewProfile::new(game.world_seed()));
    let sources = ui_review.as_ref().map(|dir| {
        visual_test::UiReviewProfile::prepare_output(
            dir,
            review_scenes.iter().map(|&i| client::UI_REVIEW_SCENES[i]),
        )
        .expect("Prepare visual review output");
        let started = std::time::Instant::now();
        let sources = game.ui_review_sources();
        ui_profile
            .as_mut()
            .unwrap()
            .baseline_setup(started.elapsed().as_secs_f64() * 1000.0);
        sources
    });
    let mut sequence = client::UiReviewSequence::default();
    let mut changed_page = false;
    let mut playtest = self_test_path
        .clone()
        .map(|path| playtest::Playtest::new(path, seed));
    let review_duration = |scene: usize| if (13..=15).contains(&scene) { 120 } else { 10 };
    let review_total_frames = review_scenes
        .iter()
        .map(|&scene| review_duration(scene))
        .sum::<u32>();

    loop {
        if !automated {
            game.handle_keyboard();
            game.handle_mouse(&buttons);
        }

        if let Some(playtest) = &mut playtest {
            playtest.advance(frame, &mut game, &buttons);
        }

        let music_cue = game
            .sim
            .music_cue_view(sim::HUMAN)
            .expect("human player has a music cue");
        music
            .update(
                music_cue.era,
                music_cue.culture_group,
                game.music_muted,
                game.music_volume,
            )
            .await;

        let review_position = ui_review.as_ref().and_then(|_| {
            let mut scene_start = 0;
            for &scene in &review_scenes {
                let duration = review_duration(scene);
                if frame < scene_start + duration {
                    let local_frame = frame - scene_start;
                    let phase_index = ((local_frame * 10) / duration).min(9);
                    let phase = if (local_frame * 10).is_multiple_of(duration) {
                        phase_index
                    } else {
                        0
                    };
                    return Some((scene, local_frame, phase));
                }
                scene_start += duration;
            }
            None
        });
        let scene = review_position.map(|(scene, _, _)| scene);
        if let Some((scene, local_frame, phase)) = review_position {
            if local_frame == 0 {
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
                let result = game.step_ui_review(scene, phase, &buttons, &mut sequence);
                let profile = ui_profile.as_mut().unwrap();
                profile.actions(scene, started.elapsed().as_secs_f64() * 1000.0);
                for (label, ok) in sequence.checks.drain(..) {
                    profile.check(scene, label, ok);
                }
                match result {
                    Ok(changed) => {
                        if phase == 6 || (phase == 7 && matches!(scene, 14 | 15)) {
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
        let sound_requests = game
            .take_unit_sound_requests()
            .into_iter()
            .filter_map(|request| {
                assets
                    .unit_sound_path(request.kind, request.era_index, &request.action)
                    .map(|path| (path, request.delay_seconds))
            })
            .collect::<Vec<_>>();
        unit_sounds
            .update(sound_requests, game.sfx_muted, game.sfx_volume)
            .await;
        if let (Some(scene), Some(started)) = (scene, draw_started) {
            let profile = ui_profile.as_mut().unwrap();
            profile.draw(
                scene,
                started.elapsed().as_secs_f64() * 1000.0,
                game.border_cache_stats(&assets),
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
            let (scene, _, phase) = review_position.expect("review scene is active");
            let next = phase == 8 && changed_page;
            let main_capture_phase = if scene == 13 { 8 } else { 5 };
            if phase == main_capture_phase || next {
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
                if matches!(scene, 31 | 32) {
                    let polar_edge_has_no_black_gaps =
                        game.ui_review_polar_edge_has_no_black_gaps(&image, scene == 31);
                    profile.check(
                        scene,
                        "polar_edge_has_no_black_gaps",
                        polar_edge_has_no_black_gaps,
                    );
                    if !polar_edge_has_no_black_gaps {
                        let _ = profile.finish(dir);
                        eprintln!(
                            "Polar edge contains black gaps: {}",
                            client::UI_REVIEW_SCENES[scene]
                        );
                        std::process::exit(1);
                    }
                }
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
            if frame >= review_total_frames {
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
            && let Some(playtest) = &mut playtest
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
    fn map_size_argument_matches_civ3_presets_and_validates_custom_dimensions() {
        for (name, dimensions) in [
            ("tiny", (60, 60)),
            ("small", (80, 80)),
            ("standard", (100, 100)),
            ("large", (130, 130)),
            ("huge", (160, 160)),
        ] {
            let wrap = parse_map_size(name).unwrap().topology().unwrap();
            assert_eq!((wrap.width(), wrap.height()), dimensions);
        }
        assert_eq!(
            parse_map_size("80x60")
                .unwrap()
                .topology()
                .unwrap()
                .tile_count(),
            4800
        );
        for invalid in ["80", "81x60", "8x60", "513x60", "80xno", "giant"] {
            assert!(parse_map_size(invalid).is_err(), "accepted {invalid}");
        }
    }

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
