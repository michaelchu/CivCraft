//! Application entry point and runtime coordinator for CivCraft.
//!
//! Startup parses command-line options, chooses the map topology and
//! tileset, then either creates a new [`sim::Simulation`] or restores one from
//! a campaign save. The same frame loop serves interactive play, screenshots,
//! and UI review captures; automated modes disable ordinary
//! input and autosaving so a run has reproducible side effects.
//!
//! Keep mode validation and shared initialization here. Gameplay rules belong
//! in `sim`, while rendering and user interaction belong in `client`.

use crate::{client, ensure_parent, mapgen, sim, visual_test, world_setup::WorldSetup};

use crate::client::{Assets, Game, MusicPlayer, UiAction, UnitSoundPlayer};
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

/// Starts the game or one of its automated capture modes.
pub async fn run_desktop() {
    if env::args().any(|arg| arg == "--wrap") {
        eprintln!("--wrap has been removed; CivCraft uses the Civ III horizontal map topology.");
        std::process::exit(2);
    }
    if env::args().any(|arg| arg == "--tileset-theme") {
        eprintln!("--tileset-theme has been removed; choose a built-in tileset with --tileset.");
        std::process::exit(2);
    }
    let args: Vec<_> = env::args().collect();
    let world = WorldSetup::parse(mapgen::Civ3MapSize::Small, |flag| {
        parsed_argument(&args, flag)
    })
    .unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let load = argument_value("--load");
    if load.is_some() && world.specified {
        eprintln!("world setup options require a newly generated world");
        std::process::exit(2);
    }
    let tileset_name = argument_value("--tileset").unwrap_or_else(|| "civ3".to_owned());
    let smoke_test = env::args().any(|arg| arg == "--smoke-test");
    let smoke_turns = argument_value("--smoke-turns")
        .map(|value| {
            value
                .parse::<u32>()
                .ok()
                .filter(|turns| *turns > 0)
                .unwrap_or_else(|| {
                    eprintln!("--smoke-turns must be a positive integer");
                    std::process::exit(2);
                })
        })
        .unwrap_or(3);
    if argument_value("--smoke-turns").is_some() && !smoke_test {
        eprintln!("--smoke-turns requires --smoke-test");
        std::process::exit(2);
    }
    let screenshot_path = argument_value("--screenshot");
    let ui_review = argument_value("--ui-review");
    let ui_baseline = argument_value("--ui-baseline");
    let similarity_argument = argument_value("--ui-min-similarity");
    let ui_min_similarity = similarity_argument
        .as_deref()
        .unwrap_or("0.99")
        .parse::<f64>()
        .ok()
        .filter(|score| (0.0..=1.0).contains(score))
        .unwrap_or_else(|| {
            eprintln!("--ui-min-similarity must be a number from 0 to 1");
            std::process::exit(2);
        });
    let single_scene = argument_value("--ui-scene");
    if ui_review.is_some() && screenshot_path.is_some() {
        eprintln!("--ui-review must run separately from --screenshot");
        std::process::exit(2);
    }
    if single_scene.is_some() && ui_review.is_none() {
        eprintln!("--ui-scene requires --ui-review");
        std::process::exit(2);
    }
    if smoke_test && (screenshot_path.is_some() || ui_review.is_some()) {
        eprintln!("--smoke-test cannot be combined with --screenshot or --ui-review");
        std::process::exit(2);
    }
    if (ui_baseline.is_some() || similarity_argument.is_some()) && ui_review.is_none() {
        eprintln!("--ui-baseline and --ui-min-similarity require --ui-review");
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
    let automated = smoke_test || screenshot_path.is_some() || ui_review.is_some();
    if screenshot_path.is_some() || ui_review.is_some() {
        // Configure readback on the render thread before any screenshot frame.
        unsafe { get_internal_gl().quad_context.enable_screen_readback() };
    }
    let asset_context = client::AssetContext::from_env(!automated).unwrap_or_else(|error| {
        eprintln!("Could not configure assets: {error}");
        std::process::exit(2);
    });
    let tileset = asset_context
        .select_tileset(&tileset_name)
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2);
        });
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
    let mut game = if let Some(path) = load.as_deref() {
        let simulation = sim::Simulation::load_campaign(Path::new(path)).unwrap_or_else(|error| {
            eprintln!("Could not load campaign. {error}");
            std::process::exit(1);
        });
        Game::from_simulation(simulation)
    } else {
        Game::from_simulation(sim::Simulation::new(world.generate(seed)))
    };
    let assets = Assets::new(&tileset, &asset_context).unwrap_or_else(|error| {
        eprintln!("Could not load {} tileset. {error}", tileset.name());
        std::process::exit(2);
    });
    println!(
        "World seed: {} · map size: {} ({}×{}) · generator: {} · wrap: {} · renderer: Civ III · tileset: {}",
        game.world_seed(),
        if load.is_some() {
            format!("{}x{}", game.map_wrap().width(), game.map_wrap().height())
        } else {
            world.size.to_string()
        },
        game.map_wrap().width(),
        game.map_wrap().height(),
        load.as_ref()
            .map_or(world.generator.name(), |_| "saved campaign"),
        game.map_wrap(),
        tileset.name()
    );
    game.autosave_enabled = !automated;
    let mut music = MusicPlayer::new(&asset_context);
    let mut unit_sounds = UnitSoundPlayer::new(&asset_context);
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
    let mut completed_smoke_turns = 0;
    let mut smoke_final_frame_pending = false;
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
        if smoke_test {
            if smoke_final_frame_pending {
                let result = if game.sim.game_over_view().is_some() {
                    "game over"
                } else {
                    "turn limit"
                };
                println!(
                    "Smoke test passed: rendered startup and post-turn state after processing {completed_smoke_turns} turn(s) ({result})"
                );
                break;
            }
            if game.sim.game_over_view().is_none() && completed_smoke_turns < smoke_turns {
                game.simulate_turn_for_smoke_test();
                completed_smoke_turns += 1;
                println!(
                    "Smoke test: completed turn {completed_smoke_turns}/{smoke_turns} (game turn {})",
                    game.turn_for_smoke_test()
                );
            }
            if completed_smoke_turns >= smoke_turns || game.sim.game_over_view().is_some() {
                smoke_final_frame_pending = true;
            }
        }
        if let Some(dir) = &ui_review {
            let (scene, _, phase) = review_position.expect("review scene is active");
            let next = phase == 8 && changed_page;
            let main_capture_phase = if scene == 13 { 8 } else { 5 };
            if phase == main_capture_phase || next {
                let started = std::time::Instant::now();
                let image = visual_test::capture_screen().unwrap_or_else(|error| {
                    let profile = ui_profile.as_mut().unwrap();
                    profile.check(scene, "screen_capture", false);
                    let _ = profile.finish(dir);
                    eprintln!(
                        "Could not capture {}: {error}",
                        client::UI_REVIEW_SCENES[scene]
                    );
                    std::process::exit(1);
                });
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                image.bytes.hash(&mut hasher);
                let file = format!(
                    "{}{}.png",
                    client::UI_REVIEW_SCENES[scene],
                    if next { "-next" } else { "" }
                );
                let output = Path::new(dir).join(&file);
                image.export_png(output.to_str().expect("UTF-8 gallery path"));
                let profile = ui_profile.as_mut().unwrap();
                let similarity = ui_baseline.as_ref().map(|baseline_dir| {
                    let baseline = Path::new(baseline_dir).join(&file);
                    let diff = Path::new(dir).join(format!(
                        "{}-diff.png",
                        file.strip_suffix(".png").unwrap_or(&file)
                    ));
                    visual_test::compare_capture(&output, &baseline, &diff).unwrap_or_else(
                        |error| {
                            profile.check(scene, "baseline_similarity", false);
                            let _ = profile.finish(dir);
                            eprintln!(
                                "Could not compare UI baseline for {}: {error}",
                                client::UI_REVIEW_SCENES[scene]
                            );
                            std::process::exit(1);
                        },
                    )
                });
                if let Some(score) = similarity {
                    let passed = score >= ui_min_similarity;
                    profile.check(scene, "baseline_similarity", passed);
                    if !passed {
                        let _ = profile.capture(
                            scene,
                            file,
                            started.elapsed().as_secs_f64() * 1000.0,
                            hasher.finish(),
                            next,
                            Some(score),
                        );
                        let _ = profile.finish(dir);
                        eprintln!(
                            "UI baseline similarity {:.6} is below {:.6}: {}",
                            score,
                            ui_min_similarity,
                            client::UI_REVIEW_SCENES[scene]
                        );
                        std::process::exit(1);
                    }
                }
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
                    similarity,
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
            visual_test::capture_screen()
                .unwrap_or_else(|error| {
                    eprintln!("Could not capture screenshot: {error}");
                    std::process::exit(1);
                })
                .export_png(path);
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
            let wrap = name
                .parse::<mapgen::Civ3MapSize>()
                .unwrap()
                .topology()
                .unwrap();
            assert_eq!((wrap.width(), wrap.height()), dimensions);
        }
        assert_eq!(
            "80x60"
                .parse::<mapgen::Civ3MapSize>()
                .unwrap()
                .topology()
                .unwrap()
                .tile_count(),
            4800
        );
        for invalid in ["80", "81x60", "8x60", "513x60", "80xno", "giant"] {
            assert!(
                invalid.parse::<mapgen::Civ3MapSize>().is_err(),
                "accepted {invalid}"
            );
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
    }

    #[test]
    fn smoke_turns_can_be_read_when_smoke_test_mode_is_selected() {
        let args: Vec<String> = ["game", "--smoke-test", "--smoke-turns", "3"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            parsed_argument(&args, "--smoke-turns").unwrap(),
            Some("3".into())
        );
    }
}
