pub(crate) mod config;
mod report;
pub(crate) mod scenario;

use crate::{
    analytics,
    sim::trace::{Collector, Phase},
};
use config::Config;
use std::io::Write;

pub(crate) fn run(config: Config) -> Result<(), String> {
    // Validate registration before generating a world or touching output files.
    let mut registry = analytics::registry::build(
        &config.analyzers,
        config.growth_stall_turns,
        config.settlement_every,
    )?;
    let mut provider_ids = registry
        .providers()
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    provider_ids.extend(config.providers.iter().cloned());
    let providers = analytics::providers::build(&provider_ids)?;
    let mut kinds = registry.event_kinds();
    for domain in &config.trace_domains {
        kinds.insert(if domain == "all" {
            "*".into()
        } else {
            format!("{domain}.*")
        });
    }
    let (mut game, scenario) = if let Some(path) = &config.load_campaign {
        scenario::load(path)?
    } else if let Some(name) = &config.scenario {
        scenario::build(name, config.seed, config.map_size, config.world)?
    } else {
        scenario::generated(config.seed, config.map_size, config.world)
    };
    let failed_scenario_checks = scenario
        .setup_checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.name)
        .collect::<Vec<_>>();
    game.trace = Collector::enabled(kinds, config.buffer_events);
    let mut report = report::Report::new(&config.report, scenario)?;
    let mut snapshot = analytics::providers::capture(&game, &providers, Phase::Initial);
    report.snapshot(&snapshot)?;
    registry.start(&snapshot);
    report.checkpoint(
        &config,
        &registry,
        &snapshot,
        0,
        None,
        game.game_over(),
        game.trace.dropped(),
    )?;
    let mut turns = 0;
    let mut reason = "turn_limit";
    let mut previous_dropped = 0;
    while turns < config.turns as usize {
        if game.game_over().is_some() {
            reason = "game_over";
            break;
        }
        game.simulate_turn();
        turns += 1;
        let dropped = game.trace.dropped();
        registry.gap(dropped - previous_dropped);
        previous_dropped = dropped;
        for event in game.trace.drain() {
            let retained = report.event(&event, &config)?;
            registry.event(&event, retained);
        }
        snapshot = analytics::providers::capture(&game, &providers, Phase::TurnEnd);
        report.snapshot(&snapshot)?;
        registry.boundary(&snapshot);
        if config.stop_on_finding != "none"
            && registry.output.findings.iter().any(|f| {
                f.status == "open"
                    && (config.stop_on_finding == "any"
                        || f.classification == "invariant_violation")
            })
        {
            reason = "finding";
            break;
        }
        if turns % config.checkpoint_every == 0 {
            report.checkpoint(
                &config,
                &registry,
                &snapshot,
                turns,
                None,
                game.game_over(),
                game.trace.dropped(),
            )?;
            println!(
                "Simulated {}/{} turns (game turn {}, {} cities, {} findings)",
                turns,
                config.turns,
                game.turn(),
                snapshot.city_count,
                registry.output.findings.len()
            );
            std::io::stdout().flush().map_err(|e| e.to_string())?;
        }
    }
    if game.game_over().is_some() {
        reason = "game_over";
    }
    registry.finish();
    report.checkpoint(
        &config,
        &registry,
        &snapshot,
        turns,
        Some(reason),
        game.game_over(),
        game.trace.dropped(),
    )?;
    println!(
        "Completed {turns} turns; stop reason: {reason}. Report: {}",
        config.report
    );
    if !failed_scenario_checks.is_empty() {
        return Err(format!(
            "scenario checks failed: {}",
            failed_scenario_checks.join(", ")
        ));
    }
    Ok(())
}

/// Run the headless CLI with arguments excluding the executable name.
pub fn run_cli(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--help") {
        println!("{}", config::HELP);
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-analyzers") {
        println!("{}", crate::analytics::registry::IDS.join("\n"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-scenarios") {
        println!("{}", scenario::IDS.join("\n"));
        return Ok(());
    }
    run(config::Config::parse(args)?)
}
