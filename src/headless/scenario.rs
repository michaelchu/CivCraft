//! Reproducible starting states for headless simulation runs.
//!
//! Scenario setup is deliberately separate from analytics and reporting. A
//! named builder and a persisted campaign both produce the same `Simulation`
//! input consumed by the runner.

use crate::{
    mapgen::{self, Civ3MapSize},
    sim::Simulation,
    world_setup::{Generator, WorldSetup},
};
use serde::Serialize;
use std::path::Path;

use crate::sim::scenarios::regression::{self, Check, Fixture, Scenario};

pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    regression::all().map(|scenario| scenario.id)
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Provenance {
    pub kind: &'static str,
    pub name: String,
    pub version: u16,
    pub checks: Vec<Check>,
    #[serde(skip)]
    pub world: WorldProvenance,
}

#[derive(Clone, Debug)]
pub(crate) struct WorldProvenance {
    pub seed: u64,
    pub size: Civ3MapSize,
    pub generator: Option<Generator>,
    pub settings: Option<serde_json::Value>,
}

impl WorldProvenance {
    fn for_campaign(
        game: &Simulation,
        size: Option<Civ3MapSize>,
        generator: Option<Generator>,
    ) -> Self {
        Self {
            seed: game.world_seed,
            size: size.unwrap_or_else(|| {
                Civ3MapSize::Custom(game.map_wrap.width(), game.map_wrap.height())
            }),
            generator,
            settings: generator.map(Generator::settings_metadata),
        }
    }
}

pub(crate) struct Run {
    pub game: Simulation,
    pub provenance: Provenance,
    scenario: Option<&'static Scenario>,
}

impl Run {
    pub fn has_actions(&self) -> bool {
        self.scenario.is_some_and(Scenario::has_actions)
    }

    pub fn execute_actions(&mut self) {
        if let Some(scenario) = self.scenario.take() {
            self.provenance.checks = scenario.run(&mut self.game);
            self.game.enable_all_civilizations_as_ai();
        }
    }
}

pub(crate) fn load(path: &Path) -> Result<Run, String> {
    let mut game = Simulation::load_campaign(path)
        .map_err(|error| format!("could not load campaign {}: {error}", path.display()))?;
    game.enable_all_civilizations_as_ai();
    Ok(Run {
        provenance: Provenance {
            kind: "saved_campaign",
            name: path.display().to_string(),
            version: 1,
            checks: vec![],
            world: WorldProvenance::for_campaign(&game, None, None),
        },
        game,
        scenario: None,
    })
}

pub(crate) fn build(name: &str, seed: u64, setup: WorldSetup) -> Result<Run, String> {
    let scenario = regression::find(name).ok_or_else(|| {
        format!(
            "unknown scenario {name}; available: {}",
            names().collect::<Vec<_>>().join(",")
        )
    })?;
    let (mut game, size, generator) = match scenario.fixture {
        Fixture::Generated => (
            Simulation::new(setup.generate(seed)),
            Some(setup.size),
            setup.generator,
        ),
        Fixture::NewCampaign | Fixture::AllAiCampaign => {
            if setup.specified {
                return Err(format!(
                    "scenario {name} uses a fixed map and takes no world setup options"
                ));
            }
            (
                Simulation::new(mapgen::generate(seed)),
                None,
                Generator::Fractal,
            )
        }
    };
    scenario.prepare(&mut game);
    Ok(Run {
        provenance: Provenance {
            kind: "named",
            name: name.into(),
            version: 1,
            checks: vec![],
            world: WorldProvenance::for_campaign(&game, size, Some(generator)),
        },
        game,
        scenario: Some(scenario),
    })
}

pub(crate) fn generated(seed: u64, setup: WorldSetup) -> Run {
    let mut game = Simulation::new(setup.generate(seed));
    game.enable_all_civilizations_as_ai();
    Run {
        provenance: Provenance {
            kind: "generated",
            name: "default".into(),
            version: 1,
            checks: vec![],
            world: WorldProvenance::for_campaign(&game, Some(setup.size), Some(setup.generator)),
        },
        game,
        scenario: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> WorldSetup {
        WorldSetup::parse(Civ3MapSize::Tiny, |_| Ok(None)).unwrap()
    }

    #[test]
    fn named_scenarios_are_validated() {
        assert!(build("missing", 42, world()).is_err());
    }

    #[test]
    fn saved_campaigns_enter_through_the_same_scenario_boundary() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-headless-scenario-{}.save",
            std::process::id()
        ));
        let game = Simulation::new(world().generate(42));
        game.save_campaign(&path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.game.turn(), game.turn());
        assert_eq!(loaded.provenance.kind, "saved_campaign");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn named_actions_are_deferred_until_the_runner_executes_them() {
        let mut run = build("settlement-ferry", 42, world()).unwrap();
        assert!(run.has_actions());
        assert!(run.provenance.checks.is_empty());
        run.execute_actions();
        assert!(!run.has_actions());
        assert!(run.provenance.checks.iter().all(|check| check.passed));
    }

    #[test]
    fn combat_matrix_runs_map_combat_scenarios() {
        let mut run = build("combat-matrix", 42, world()).unwrap();
        assert!(run.has_actions());
        run.execute_actions();
        assert_eq!(run.provenance.checks.len(), 4);
        for check in &run.provenance.checks {
            assert!(check.passed, "{}: {}", check.name, check.detail);
        }
    }
}
