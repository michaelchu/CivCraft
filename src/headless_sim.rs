//! Headless seeded campaign runner for gameplay observation.

// This binary reuses the simulation and map generation modules without the UI,
// so many APIs used by the desktop binary are intentionally unused here.
#![allow(dead_code, unused_imports)]

mod mapgen;
mod random;
mod rules;
mod sim;
mod world;

use std::{
    collections::HashMap,
    env, fs,
    io::{self, Write},
    path::Path,
};

fn value(args: &[String], flag: &str, fallback: &str) -> Result<String, String> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(fallback.to_owned());
    };
    args.get(index + 1)
        .filter(|next| !next.starts_with("--"))
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn json_string(text: &str) -> String {
    let mut result = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c <= '\u{1f}' => result.push(' '),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

fn appended_events<'a>(previous: &[String], current: &'a [String]) -> &'a [String] {
    let overlap = (0..=previous.len().min(current.len()))
        .rev()
        .find(|&count| previous[previous.len() - count..] == current[..count])
        .unwrap_or(0);
    &current[overlap..]
}

fn production_population_cost(city: &sim::City) -> i32 {
    match city.production {
        sim::Production::Unit(kind) => kind.stats().pop_cost,
        sim::Production::Building(_) => 0,
    }
}

fn write_report(
    path: &str,
    seed: u64,
    map_size: mapgen::Civ3MapSize,
    world_settings: mapgen::Civ3WorldSettings,
    requested_turns: u32,
    game: &sim::Simulation,
    findings: &[String],
    history: &[String],
) -> Result<(), String> {
    let game_over = game
        .game_over()
        .map(json_string)
        .unwrap_or_else(|| "null".into());
    let report = format!(
        "{{\"version\":3,\"seed\":{seed},\"map_size\":{},\"world_settings\":{},\"requested_turns\":{requested_turns},\"turns_simulated\":{},\"final_turn\":{},\"game_over\":{game_over},\"findings\":[{}],\"history\":[{}]}}\n",
        json_string(&map_size.to_string()),
        json_string(&format!("{world_settings:?}")),
        history.len(),
        game.turn(),
        findings.join(","),
        history.join(",")
    );
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create report directory: {error}"))?;
    }
    fs::write(path, report).map_err(|error| format!("could not write {path}: {error}"))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let seed = value(&args, "--seed", "42")?
        .parse::<u64>()
        .map_err(|_| "--seed must be a nonnegative integer".to_owned())?;
    let turns = value(&args, "--turns", "100")?
        .parse::<u32>()
        .ok()
        .filter(|turns| *turns > 0)
        .ok_or_else(|| "--turns must be a positive integer".to_owned())?;
    let map_size = value(&args, "--map-size", "tiny")?.parse::<mapgen::Civ3MapSize>()?;
    let world_settings = mapgen::Civ3WorldSettings {
        ocean_coverage: value(&args, "--ocean-coverage", "70")?.parse()?,
        climate: value(&args, "--climate", "normal")?.parse()?,
        age: value(&args, "--world-age", "4")?.parse()?,
        landform: value(&args, "--landform", "continents")?.parse()?,
        temperature: value(&args, "--temperature", "temperate")?.parse()?,
    };
    let checkpoint_every = value(&args, "--checkpoint-every", "10")?
        .parse::<usize>()
        .ok()
        .filter(|interval| *interval > 0)
        .ok_or_else(|| "--checkpoint-every must be a positive integer".to_owned())?;
    let report_path = value(&args, "--report", "headless-simulation.json")?;

    let map = mapgen::generate_for_civ3_size(seed, map_size, world_settings);
    let mut game = sim::Simulation::new(map);
    game.enable_all_civilizations_as_ai();

    let mut history = Vec::with_capacity(turns as usize);
    let mut previous_population: HashMap<u32, i32> = game
        .cities()
        .iter()
        .map(|city| (city.id, city.size))
        .collect();
    let mut previous_food: HashMap<u32, i32> = game
        .cities()
        .iter()
        .map(|city| (city.id, city.food))
        .collect();
    let mut non_growth_turns: HashMap<u32, u32> = HashMap::new();
    let mut non_accumulating_food_turns: HashMap<u32, u32> = HashMap::new();
    let mut findings = Vec::new();
    let mut previous_events = game.events().to_vec();
    for _ in 0..turns {
        if game.game_over().is_some() {
            break;
        }
        game.simulate_turn();
        let new_events = appended_events(&previous_events, game.events());
        let cities = game
            .cities()
            .iter()
            .map(|city| {
                let forecast = game.city_forecast(city);
                let food_surplus = forecast.food_surplus(city.size, forecast.support.food);
                let growth_threshold = sim::Simulation::growth_box(city.size);
                let population_change = previous_population
                    .get(&city.id)
                    .map_or(0, |previous| city.size - previous);
                let food_store_change = previous_food
                    .get(&city.id)
                    .map_or(0, |previous| city.food - previous);
                let stagnant = non_growth_turns.entry(city.id).or_default();
                if population_change != 0 || food_surplus <= 0 {
                    *stagnant = 0;
                } else {
                    *stagnant += 1;
                }
                let food_to_growth = (growth_threshold - city.food).max(0);
                let projected_growth_turns = if food_surplus > 0 {
                    (food_to_growth + food_surplus - 1) / food_surplus
                } else {
                    -1
                };
                let expected_cycle_turns = if food_surplus > 0 {
                    (growth_threshold + food_surplus - 1) / food_surplus
                } else {
                    -1
                };
                let population_cost = production_population_cost(city);
                let size_limit = game.city_size_limit(city);
                let non_accumulating = non_accumulating_food_turns.entry(city.id).or_default();
                if food_surplus > 0
                    && population_change == 0
                    && population_cost == 0
                    && city.size < size_limit
                    && food_store_change <= 0
                {
                    *non_accumulating += 1;
                } else {
                    *non_accumulating = 0;
                }
                let potential_stall = *non_accumulating >= 2;
                let growth_status = if city.size >= size_limit {
                    "growth_limited_by_city_size"
                } else if population_cost > 0 {
                    "production_consumes_population"
                } else if potential_stall {
                    "positive_surplus_not_reaching_food_store"
                } else if food_surplus < 0 {
                    "food_deficit"
                } else if food_surplus == 0 {
                    "food_balance_zero"
                } else if city.food + food_surplus >= growth_threshold {
                    "growth_projected_next_city_turn"
                } else {
                    "food_accumulating"
                };
                if potential_stall {
                    findings.push(format!(
                        "{{\"kind\":\"positive_surplus_food_not_accumulating\",\"turn\":{},\"city_id\":{},\"owner\":{},\"population\":{},\"food\":{},\"food_store_change\":{},\"food_surplus\":{},\"growth_threshold\":{},\"size_limit\":{},\"non_accumulating_turns\":{}}}",
                        game.turn(), city.id, city.owner, city.size, city.food,
                        food_store_change, food_surplus, growth_threshold, size_limit,
                        non_accumulating
                    ));
                }
                format!(
                    "{{\"id\":{},\"owner\":{},\"name\":{},\"population\":{},\"population_change\":{},\"food\":{},\"food_store_change\":{},\"food_output\":{},\"food_surplus\":{},\"growth_threshold\":{},\"food_to_growth\":{},\"projected_growth_turns\":{},\"expected_cycle_turns\":{},\"non_growth_turns\":{},\"non_accumulating_food_turns\":{},\"potential_stall\":{},\"growth_status\":{},\"size_limit\":{},\"production\":{},\"production_population_cost\":{},\"support_food\":{},\"disorder\":{},\"specialists\":[{},{},{}],\"worked_tiles\":[{}]}}",
                    city.id,
                    city.owner,
                    json_string(&city.name),
                    city.size,
                    population_change,
                    city.food,
                    food_store_change,
                    forecast.food,
                    food_surplus,
                    growth_threshold,
                    food_to_growth,
                    projected_growth_turns,
                    expected_cycle_turns,
                    stagnant,
                    non_accumulating,
                    potential_stall,
                    json_string(growth_status),
                    size_limit,
                    json_string(city.production.name()),
                    population_cost,
                    forecast.support.food,
                    forecast.disorder,
                    city.specialists[0],
                    city.specialists[1],
                    city.specialists[2],
                    city.worked_tiles
                        .iter()
                        .map(|&(x, y)| {
                            let (food, shields, trade) = game.diagnostic_tile_yields(city, x, y);
                            format!("{{\"x\":{x},\"y\":{y},\"food\":{food},\"shields\":{shields},\"trade\":{trade}}}")
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        previous_population = game
            .cities()
            .iter()
            .map(|city| (city.id, city.size))
            .collect();
        previous_food = game
            .cities()
            .iter()
            .map(|city| (city.id, city.food))
            .collect();
        if game.turn() >= 11 && game.turn() % 10 == 1 {
            for owner in 0..game.civilization_count {
                let city_count = game.cities_owned_by(owner).count();
                let unit_count = game.units_owned_by(owner).count();
                if game.diagnostic_player_alive(owner) && city_count == 0 {
                    findings.push(format!(
                        "{{\"kind\":\"civilization_without_cities_candidate\",\"turn\":{},\"owner\":{},\"name\":{},\"units\":{unit_count},\"unit_details\":[{}]}}",
                        game.turn(), owner, json_string(&game.players()[owner].name),
                        game.diagnostic_units_json(owner)
                    ));
                }
            }
        }
        let players = (0..game.civilization_count)
            .map(|owner| {
                let player = &game.players()[owner];
                format!(
                    "{{\"id\":{owner},\"name\":{},\"cities\":{},\"units\":{},\"unit_details\":[{}],\"gold\":{},\"score\":{}}}",
                    json_string(&player.name),
                    game.cities_owned_by(owner).count(),
                    game.units_owned_by(owner).count(),
                    game.diagnostic_units_json(owner),
                    player.gold,
                    game.civilization_score(owner)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        history.push(format!(
            "{{\"turn\":{},\"city_count\":{},\"cities\":[{cities}],\"civilizations\":[{players}],\"events\":[{}]}}",
            game.turn(),
            game.cities().len(),
            new_events
                .iter()
                .map(|event| json_string(event))
                .collect::<Vec<_>>()
                .join(",")
        ));
        previous_events = game.events().to_vec();
        if history.len() % checkpoint_every == 0 {
            write_report(
                &report_path,
                seed,
                map_size,
                world_settings,
                turns,
                &game,
                &findings,
                &history,
            )?;
            println!(
                "Simulated {}/{} turns (game turn {}, {} cities)",
                history.len(),
                turns,
                game.turn(),
                game.cities().len()
            );
            io::stdout()
                .flush()
                .map_err(|error| format!("could not flush progress: {error}"))?;
        }
    }

    write_report(
        &report_path,
        seed,
        map_size,
        world_settings,
        turns,
        &game,
        &findings,
        &history,
    )?;
    println!(
        "Completed through turn {}. Report: {}",
        game.turn(),
        report_path
    );
    Ok(())
}
