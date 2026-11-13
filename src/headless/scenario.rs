//! Reproducible starting states for headless simulation runs.
//!
//! Scenario setup is deliberately separate from analytics and reporting. A
//! named builder and a persisted campaign both produce the same `Simulation`
//! input consumed by the runner.

use crate::{mapgen, sim::Simulation};
use serde::Serialize;
use std::path::Path;

pub(crate) const IDS: &[&str] = &["generated-ai", "city-healing", "settlement-ferry"];

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Check {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Provenance {
    pub kind: &'static str,
    pub name: String,
    pub version: u16,
    pub checks: Vec<Check>,
}

enum Action {
    None,
    CityHealing,
    SettlementFerry,
}

pub(crate) struct Run {
    pub game: Simulation,
    pub provenance: Provenance,
    action: Action,
}

impl Run {
    pub fn has_actions(&self) -> bool {
        !matches!(self.action, Action::None)
    }

    pub fn execute_actions(&mut self) {
        self.provenance.checks = match self.action {
            Action::None => vec![],
            Action::CityHealing => vec![Check {
                name: "city_healing",
                passed: self.game.city_healing_playtest(),
                detail: "Barracks and ordinary city recovery used their expected rules".into(),
            }],
            Action::SettlementFerry => {
                let (queued, sequenced, founded) = self.game.settlement_ferry_production_playtest();
                vec![
                    Check {
                        name: "ferry_queued",
                        passed: queued,
                        detail: "AI selected a ferry for an overseas settlement plan".into(),
                    },
                    Check {
                        name: "settler_sequenced",
                        passed: sequenced,
                        detail: "AI sequenced a Settler after completing the ferry".into(),
                    },
                    Check {
                        name: "island_founded",
                        passed: founded,
                        detail: "AI transported its founder and established the island city".into(),
                    },
                ]
            }
        };
        self.action = Action::None;
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
        action: Action::None,
    })
}

pub(crate) fn build(
    name: &str,
    seed: u64,
    size: mapgen::Civ3MapSize,
    world: mapgen::Civ3WorldSettings,
) -> Result<Run, String> {
    let (mut game, action) = match name {
        "generated-ai" => {
            let game = Simulation::new(mapgen::generate_for_civ3_size(seed, size, world));
            (game, Action::None)
        }
        "city-healing" => {
            // Pin the historical fixture used by the original desktop
            // regression rather than inheriting campaign map options.
            (Simulation::new(mapgen::generate(seed)), Action::CityHealing)
        }
        "settlement-ferry" => {
            // Pin the historical fixture used by the original desktop
            // regression rather than inheriting campaign map options.
            (
                Simulation::new(mapgen::generate(seed)),
                Action::SettlementFerry,
            )
        }
        _ => {
            return Err(format!(
                "unknown scenario {name}; available: {}",
                IDS.join(",")
            ));
        }
    };
    game.enable_all_civilizations_as_ai();
    Ok(Run {
        game,
        provenance: Provenance {
            kind: "named",
            name: name.into(),
            version: 1,
            checks: vec![],
        },
        action,
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
        action: Action::None,
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
}
