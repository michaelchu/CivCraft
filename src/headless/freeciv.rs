use super::config::parse_options;
use crate::mapgen::Civ3MapSize;
use serde_json::json;
use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::Instant,
};

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let options = parse_options(args, |flag| {
        matches!(
            flag,
            "--engine" | "--seed" | "--turns" | "--map-size" | "--report" | "--freeciv-server"
        )
    })?;
    let value = |flag, default| options.get(flag).copied().unwrap_or(default);
    if value("--engine", "") != "freeciv" {
        return Err("--engine accepts freeciv; omit it to run CivCraft".into());
    }
    let seed = value("--seed", "42")
        .parse::<i32>()
        .ok()
        .filter(|&seed| seed > 0)
        .ok_or("Freeciv --seed must be between 1 and 2147483647")?;
    let turns = value("--turns", "30")
        .parse::<usize>()
        .ok()
        .filter(|turns| (1..=32767).contains(turns))
        .ok_or("Freeciv --turns must be between 1 and 32767")?;
    let size: Civ3MapSize = value("--map-size", "tiny").parse()?;
    let map = size.topology()?;
    let (width, height) = (map.width(), map.height());
    let civilizations = size.civilization_count();
    let script = format!(
        "rulesetdir civ2civ3\nset mapseed {seed}\nset gameseed {seed}\n\
         set minplayers 0\nset aifill {civilizations}\nset unitwaittime 0\n\
         set timeout -1\nset ec_turns 0\nset autosaves \"\"\nset mapsize XYSIZE\n\
         set xsize {width}\nset ysize {height}\nset endturn {turns}\nhard\nstart\n"
    );

    let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reference/freeciv");
    let server = options.get("--freeciv-server").map_or_else(
        || {
            reference.join(format!(
                "build-headless/freeciv-server{}",
                env::consts::EXE_SUFFIX
            ))
        },
        PathBuf::from,
    );
    let data = env::var_os("FREECIV_DATA_PATH").unwrap_or_else(|| reference.join("data").into());
    let start = Instant::now();
    let mut child = Command::new(&server)
        .args(["--bind", "127.0.0.1", "--port", "0", "--debug", "v"])
        .env("LC_ALL", "C")
        .env("FREECIV_DATA_PATH", data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "could not start {}: {error}. Build the Freeciv server as described in README.md or pass --freeciv-server",
                server.display()
            )
        })?;
    let script_result = child.stdin.take().unwrap().write_all(script.as_bytes());
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not wait for Freeciv: {error}"))?;
    let wall_seconds = start.elapsed().as_secs_f64();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() || script_result.is_err() {
        return Err(format!(
            "Freeciv run failed ({}):\n{stdout}{stderr}",
            output.status
        ));
    }

    let samples = stdout
        .lines()
        .filter_map(|line| line.split_once("End/start-turn server/ai activities: "))
        .map(|(_, timing)| {
            timing
                .strip_suffix(" seconds")
                .and_then(|seconds| seconds.parse::<f64>().ok())
                .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
                .ok_or_else(|| format!("invalid Freeciv CPU timing: {timing}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let reached_limit = stdout.contains("Game ended as the turn limit was exceeded.");
    if samples.is_empty()
        || samples.len() > turns
        || (reached_limit && samples.len() != turns)
        || !stdout.contains(&format!("Creating a map of size {width} x {height} ="))
    {
        return Err(format!(
            "Freeciv produced unexpected benchmark output:\n{stdout}{stderr}"
        ));
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let max = samples.iter().copied().fold(0.0, f64::max);
    let reason = if reached_limit {
        "turn_limit"
    } else {
        "game_over"
    };
    let report_path = value("--report", "freeciv-benchmark.json");
    let report = json!({
        "engine": "freeciv",
        "ruleset": "civ2civ3",
        "map_size": size.to_string(),
        "width": width,
        "height": height,
        "civilizations": civilizations,
        "seed": seed,
        "requested_turns": turns,
        "completed_turns": samples.len(),
        "stop_reason": reason,
        "wall_seconds": wall_seconds,
        "server_ai_cpu_seconds": samples,
        "mean_server_ai_cpu_seconds": mean,
        "max_server_ai_cpu_seconds": max,
    });
    let contents = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    fs::write(report_path, contents)
        .map_err(|error| format!("could not write {report_path}: {error}"))?;
    println!(
        "Freeciv finished {}/{turns} turns ({reason}) on {width}x{height} with {civilizations} civs in {wall_seconds:.2}s. Report: {report_path}",
        samples.len()
    );
    Ok(())
}
