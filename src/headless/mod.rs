pub(crate) mod config;
mod event_compaction;
mod freeciv;
#[cfg(feature = "desktop")]
mod minimap;
mod observation_changes;
pub(crate) mod read_report;
mod report;
pub(crate) mod scenario;
mod verbose_codec;

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
    // Minimap is a final-only export; never capture its map arrays in turn history.
    #[cfg(feature = "desktop")]
    let final_minimap = provider_ids.remove("minimap");
    let providers = analytics::providers::build(&provider_ids)?;
    let mut kinds = registry.event_kinds();
    for domain in &config.trace_domains {
        kinds.insert(if domain == "all" {
            "*".into()
        } else {
            format!("{domain}.*")
        });
    }
    let mut scenario = if let Some(path) = &config.load_campaign {
        scenario::load(path)?
    } else if let Some(name) = &config.scenario {
        scenario::build(name, config.seed, config.world)?
    } else {
        scenario::generated(config.seed, config.world)
    };
    kinds.extend(scenario.trace_kinds().iter().map(|kind| (*kind).to_owned()));
    scenario.game.trace = Collector::enabled(kinds, config.buffer_events);
    if provider_ids.contains("lifecycle") {
        scenario.game.start_lifecycle_observation();
    }
    if provider_ids.contains("research") {
        scenario.game.start_research_observation();
    }
    if config.profile_turns {
        scenario
            .game
            .trace
            .configure_profiling(crate::sim::trace::ProfileSelection {
                timing_only: config.profile_selection.timing_only,
                start_turn: config.profile_selection.start_turn,
                end_turn: config.profile_selection.end_turn,
                prefixes: config.profile_selection.prefixes.clone(),
            });
    }
    let mut report = report::Report::new(&config.report, scenario.provenance.clone())?;
    let mut snapshot = analytics::providers::capture(&scenario.game, &providers, Phase::Initial);
    report.snapshot(&snapshot, &config, true)?;
    scenario.game.trace.clear_lifecycle_changes();
    scenario.game.trace.clear_research_changes();
    registry.start(&snapshot);
    let mut previous_dropped = 0;
    if scenario.has_actions() {
        scenario.game.trace.phase(Phase::ScenarioActions);
        scenario.execute_actions();
        scenario.game.reconcile_lifecycle_observation();
        scenario.game.reconcile_research_observation();
        report.scenario(scenario.provenance.clone());
        let dropped = scenario.game.trace.dropped();
        registry.gap(dropped - previous_dropped);
        previous_dropped = dropped;
        for event in scenario.game.trace.drain() {
            let retained = report.event(&event, &config)?;
            registry.event(&event, retained);
        }
        snapshot =
            analytics::providers::capture(&scenario.game, &providers, Phase::ScenarioActions);
        registry.boundary(&snapshot);
        report.snapshot(&snapshot, &config, true)?;
        scenario.game.trace.clear_lifecycle_changes();
        scenario.game.trace.clear_research_changes();
    }
    report.checkpoint(
        &config,
        &registry,
        &snapshot,
        0,
        None,
        scenario.game.game_over(),
        scenario.game.trace.dropped(),
    )?;
    let mut turns = 0;
    let mut reason = "turn_limit";
    while turns < config.turns as usize {
        if scenario.game.game_over().is_some() {
            reason = "game_over";
            break;
        }
        scenario.simulate_turn(turns as u32);
        scenario.game.reconcile_lifecycle_observation();
        scenario.game.reconcile_research_observation();
        report.turn_profiles(scenario.game.trace.drain_profiles());
        turns += 1;
        scenario.observe_turn(turns as u32);
        let dropped = scenario.game.trace.dropped();
        registry.gap(dropped - previous_dropped);
        previous_dropped = dropped;
        for event in scenario.game.trace.drain() {
            let retained = report.event(&event, &config)?;
            registry.event(&event, retained);
        }
        snapshot = analytics::providers::capture(&scenario.game, &providers, Phase::TurnEnd);
        registry.boundary(&snapshot);
        let required = registry.output.findings.iter().any(|finding| {
            finding
                .evidence
                .iter()
                .any(|evidence| evidence.snapshot_turn == Some(snapshot.turn))
        });
        report.snapshot(&snapshot, &config, required)?;
        scenario.game.trace.clear_lifecycle_changes();
        scenario.game.trace.clear_research_changes();
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
            report.scenario(scenario.provenance.clone());
            report.checkpoint(
                &config,
                &registry,
                &snapshot,
                turns,
                None,
                scenario.game.game_over(),
                scenario.game.trace.dropped(),
            )?;
            println!(
                "Simulated {}/{} turns (game turn {}, {} cities, {} findings)",
                turns,
                config.turns,
                scenario.game.turn(),
                snapshot.city_count,
                registry.output.findings.len()
            );
            std::io::stdout().flush().map_err(|e| e.to_string())?;
        }
    }
    if scenario.game.game_over().is_some() {
        reason = "game_over";
    }
    scenario.finish(turns as u32);
    report.scenario(scenario.provenance.clone());
    let failed_scenario_checks = scenario
        .provenance
        .checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.name)
        .collect::<Vec<_>>();
    registry.finish();
    #[cfg(feature = "desktop")]
    if final_minimap {
        report.final_minimap(
            snapshot.turn,
            snapshot.phase,
            &scenario.game.observe_minimap(),
        )?;
    }
    report.checkpoint(
        &config,
        &registry,
        &snapshot,
        turns,
        Some(reason),
        scenario.game.game_over(),
        scenario.game.trace.dropped(),
    )?;
    if let Some(path) = &config.save_campaign {
        scenario
            .game
            .save_campaign(path)
            .map_err(|e| format!("failed to save campaign {}: {e}", path.display()))?;
    }
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
        println!("{}", scenario::names().collect::<Vec<_>>().join("\n"));
        return Ok(());
    }
    if args.chunks(2).any(|pair| pair[0] == "--engine") {
        return freeciv::run(args);
    }
    run(config::Config::parse(args)?)
}
