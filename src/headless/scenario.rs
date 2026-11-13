//! Reproducible starting states for headless simulation runs.
//!
//! Scenario setup is deliberately separate from analytics and reporting. A
//! named builder and a persisted campaign both produce the same `Simulation`
//! input consumed by the runner.

use crate::{mapgen, sim::Simulation};
use serde::Serialize;
use std::path::Path;

pub(crate) const IDS: &[&str] = &["generated-ai", "city-healing"];

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
    pub setup_checks: Vec<Check>,
}

pub(crate) fn load(path: &Path) -> Result<(Simulation, Provenance), String> {
    let mut game = Simulation::load_campaign(path)
        .map_err(|error| format!("could not load campaign {}: {error}", path.display()))?;
    game.enable_all_civilizations_as_ai();
    Ok((
        game,
        Provenance {
            kind: "saved_campaign",
            name: path.display().to_string(),
            version: 1,
            setup_checks: vec![],
        },
    ))
}

pub(crate) fn build(
    name: &str,
    seed: u64,
    size: mapgen::Civ3MapSize,
    world: mapgen::Civ3WorldSettings,
) -> Result<(Simulation, Provenance), String> {
    let (game, checks) = match name {
        "generated-ai" => {
            let mut game = Simulation::new(mapgen::generate_for_civ3_size(seed, size, world));
            game.enable_all_civilizations_as_ai();
            (game, vec![])
        }
        "city-healing" => {
            // Pin the historical fixture used by the original desktop
            // regression rather than inheriting campaign map options.
            let mut game = Simulation::new(mapgen::generate(seed));
            let healed = game.city_healing_playtest();
            game.enable_all_civilizations_as_ai();
            (
                game,
                vec![Check {
                    name: "city_healing",
                    passed: healed,
                    detail: "Barracks and ordinary city recovery used their expected rules".into(),
                }],
            )
        }
        _ => {
            return Err(format!(
                "unknown scenario {name}; available: {}",
                IDS.join(",")
            ));
        }
    };
    Ok((
        game,
        Provenance {
            kind: "named",
            name: name.into(),
            version: 1,
            setup_checks: checks,
        },
    ))
}

pub(crate) fn generated(
    seed: u64,
    size: mapgen::Civ3MapSize,
    world: mapgen::Civ3WorldSettings,
) -> (Simulation, Provenance) {
    let mut game = Simulation::new(mapgen::generate_for_civ3_size(seed, size, world));
    game.enable_all_civilizations_as_ai();
    (
        game,
        Provenance {
            kind: "generated",
            name: "default".into(),
            version: 1,
            setup_checks: vec![],
        },
    )
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
        let (loaded, provenance) = load(&path).unwrap();
        assert_eq!(loaded.turn(), game.turn());
        assert_eq!(provenance.kind, "saved_campaign");
        std::fs::remove_file(path).unwrap();
    }
}
