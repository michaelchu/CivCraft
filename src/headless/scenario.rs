//! Reproducible starting states for headless simulation runs.
//!
//! Scenario setup is deliberately separate from analytics and reporting. A
//! named builder and a persisted campaign both produce the same `Simulation`
//! input consumed by the runner.

use crate::{
    mapgen,
    sim::{
        Simulation,
        scenarios::regression::{self, Check, Fixture, Scenario},
    },
};
use serde::Serialize;
use std::path::Path;

/// The generated all-AI campaign used when no regression scenario is named.
const GENERATED_AI: &str = "generated-ai";

/// Identifiers accepted by `--scenario`, in listing order.
pub(crate) fn ids() -> Vec<&'static str> {
    std::iter::once(GENERATED_AI)
        .chain(regression::all().map(|scenario| scenario.id))
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Provenance {
    pub kind: &'static str,
    pub name: String,
    pub version: u16,
    pub checks: Vec<Check>,
}

pub(crate) struct Run {
    pub game: Simulation,
    pub provenance: Provenance,
    /// The registered scenario whose actions have not run yet.
    pending: Option<&'static Scenario>,
}

impl Run {
    pub fn has_actions(&self) -> bool {
        self.pending.is_some()
    }

    pub fn execute_actions(&mut self) {
        if let Some(scenario) = self.pending.take() {
            self.provenance.checks = scenario.run(&mut self.game);
        }
        self.game.enable_all_civilizations_as_ai();
    }
}

pub(crate) fn load(path: &Path) -> Result<Run, String> {
    let mut game = Simulation::load_campaign(path)
        .map_err(|error| format!("could not load campaign {}: {error}", path.display()))?;
    game.enable_all_civilizations_as_ai();
    Ok(Run {
        game,
        provenance: Provenance {
            kind: "saved_campaign",
            name: path.display().to_string(),
            version: 1,
            checks: vec![],
        },
        pending: None,
    })
}

pub(crate) fn build(
    name: &str,
    seed: u64,
    size: mapgen::Civ3MapSize,
    world: mapgen::Civ3WorldSettings,
) -> Result<Run, String> {
    let (mut game, pending) = if name == GENERATED_AI {
        let game = Simulation::new(mapgen::generate_for_civ3_size(seed, size, world));
        (game, None)
    } else {
        let scenario = regression::find(name).ok_or_else(|| {
            format!("unknown scenario {name}; run --list-scenarios for the available names")
        })?;
        // Regression scenarios pin the reference map generator rather than
        // inheriting campaign map options.
        (Simulation::new(mapgen::generate(seed)), Some(scenario))
    };
    // A scenario that starts from a new campaign keeps its human seat until
    // its actions have run; the runner then simulates every seat as AI.
    if pending.is_none_or(|scenario| scenario.fixture == Fixture::AllAiCampaign) {
        game.enable_all_civilizations_as_ai();
    }
    Ok(Run {
        game,
        provenance: Provenance {
            kind: "named",
            name: name.into(),
            version: 1,
            checks: vec![],
        },
        pending,
    })
}

pub(crate) fn generated(
    seed: u64,
    size: mapgen::Civ3MapSize,
    world: mapgen::Civ3WorldSettings,
) -> Run {
    let mut game = Simulation::new(mapgen::generate_for_civ3_size(seed, size, world));
    game.enable_all_civilizations_as_ai();
    Run {
        game,
        provenance: Provenance {
            kind: "generated",
            name: "default".into(),
            version: 1,
            checks: vec![],
        },
        pending: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_scenarios_are_validated() {
        assert!(build("missing", 42, mapgen::Civ3MapSize::Tiny, Default::default()).is_err());
    }

    #[test]
    fn saved_campaigns_enter_through_the_same_scenario_boundary() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-headless-scenario-{}.save",
            std::process::id()
        ));
        let game = Simulation::new(mapgen::generate_for_civ3_size(
            42,
            mapgen::Civ3MapSize::Tiny,
            Default::default(),
        ));
        game.save_campaign(&path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.game.turn(), game.turn());
        assert_eq!(loaded.provenance.kind, "saved_campaign");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn named_actions_are_deferred_until_the_runner_executes_them() {
        let mut run = build(
            "settlement-ferry",
            42,
            mapgen::Civ3MapSize::Tiny,
            Default::default(),
        )
        .unwrap();
        assert!(run.has_actions());
        assert!(run.provenance.checks.is_empty());
        run.execute_actions();
        assert!(!run.has_actions());
        assert!(run.provenance.checks.iter().all(|check| check.passed));
    }

    #[test]
    fn combat_matrix_runs_map_combat_scenarios() {
        let mut run = build(
            "combat-matrix",
            42,
            mapgen::Civ3MapSize::Tiny,
            Default::default(),
        )
        .unwrap();
        assert!(run.has_actions());
        run.execute_actions();
        assert_eq!(run.provenance.checks.len(), 4);
        for check in &run.provenance.checks {
            assert!(check.passed, "{}: {}", check.name, check.detail);
        }
    }

    #[test]
    fn desktop_regressions_are_listed_and_keep_the_human_seat_until_they_run() {
        assert!(ids().contains(&"ai-paradrops"));
        let mut run = build(
            "ai-paradrops",
            42,
            mapgen::Civ3MapSize::Tiny,
            Default::default(),
        )
        .unwrap();
        assert!(!crate::sim::PlaytestState::new(&run.game).players()[crate::sim::HUMAN].ai);
        run.execute_actions();
        assert!(crate::sim::PlaytestState::new(&run.game).players()[crate::sim::HUMAN].ai);
        assert!(run.provenance.checks.iter().all(|check| check.passed));
    }
}
