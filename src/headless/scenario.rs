//! Reproducible starting states for headless simulation runs.
//!
//! Scenario setup is deliberately separate from analytics and reporting. A
//! named builder and a persisted campaign both produce the same `Simulation`
//! input consumed by the runner.

use crate::{
    mapgen::{self, MapSize},
    sim::Simulation,
    world_setup::{Generator, WorldSetup},
};
use serde::Serialize;
use std::path::Path;

use crate::scenarios::{self, Check, ScenarioRun};

pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    scenarios::all().map(|scenario| scenario.id)
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Provenance {
    pub kind: &'static str,
    pub name: String,
    pub version: u16,
    pub checks: Vec<Check>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<Observation>,
    #[serde(skip)]
    pub world: WorldProvenance,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Observation {
    completed_turns: u32,
    campaign_turn: i32,
    state: serde_json::Value,
}

#[derive(Clone, Debug)]
pub(crate) struct WorldProvenance {
    pub seed: u64,
    pub size: MapSize,
    pub generator: Option<Generator>,
    pub settings: Option<serde_json::Value>,
}

impl WorldProvenance {
    fn for_campaign(
        game: &Simulation,
        size: Option<MapSize>,
        generator: Option<Generator>,
        settings: Option<serde_json::Value>,
    ) -> Self {
        Self {
            seed: game.world_seed,
            size: size.unwrap_or_else(|| {
                MapSize::Custom(game.map_wrap.width(), game.map_wrap.height())
            }),
            generator,
            settings,
        }
    }
}

pub(crate) struct Run {
    pub game: Simulation,
    pub provenance: Provenance,
    runtime: Option<Box<dyn ScenarioRun>>,
}

fn advance_comparison_game(runtime: &mut dyn ScenarioRun) {
    if let Some(game) = runtime.comparison_game()
        && game.game_over().is_none()
    {
        game.simulate_turn();
    }
}

impl Run {
    pub fn has_actions(&self) -> bool {
        self.runtime
            .as_ref()
            .is_some_and(|runtime| runtime.has_actions())
    }

    pub fn execute_actions(&mut self) {
        if let Some(runtime) = &mut self.runtime
            && runtime.has_actions()
        {
            self.provenance
                .checks
                .extend(runtime.execute_actions(&mut self.game));
            for _ in 0..runtime.action_turns() {
                let before = self.game.turn();
                self.game.simulate_turn();
                if self.game.turn() != before {
                    advance_comparison_game(runtime.as_mut());
                }
            }
            self.provenance
                .checks
                .extend(runtime.complete_actions(&mut self.game));
        }
    }

    pub fn trace_kinds(&self) -> &'static [&'static str] {
        self.runtime
            .as_ref()
            .map_or(&[], |campaign| campaign.trace_kinds())
    }

    pub fn simulate_turn(&mut self, completed_turns: u32) {
        if let Some(campaign) = &mut self.runtime {
            self.game
                .trace
                .phase(crate::sim::trace::Phase::ScenarioActions);
            campaign.before_turn(&mut self.game, completed_turns);
        }
        self.game.simulate_turn();
        if let Some(runtime) = &mut self.runtime {
            advance_comparison_game(runtime.as_mut());
        }
    }

    pub fn observe_turn(&mut self, completed_turns: u32) {
        if let Some(campaign) = &mut self.runtime
            && campaign.records_turns()
        {
            let state = campaign.observe_turn(&self.game, completed_turns);
            self.provenance.observations.push(Observation {
                completed_turns,
                campaign_turn: self.game.turn(),
                state,
            });
        }
    }

    pub fn finish(&mut self, completed_turns: u32) {
        if let Some(campaign) = self.runtime.take() {
            self.provenance
                .checks
                .extend(campaign.finish(completed_turns));
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
            observations: vec![],
            world: WorldProvenance::for_campaign(&game, None, None, None),
        },
        game,
        runtime: None,
    })
}

pub(crate) fn build(name: &str, seed: u64, setup: WorldSetup) -> Result<Run, String> {
    let scenario = scenarios::find(name).ok_or_else(|| {
        format!(
            "unknown scenario {name}; available: {}",
            names().collect::<Vec<_>>().join(",")
        )
    })?;
    let (map, size, generator, settings) = if scenario.accepts_world_options() {
        (
            setup.generate(seed),
            Some(setup.size),
            setup.generator,
            Some(setup.settings_metadata()),
        )
    } else {
        if setup.specified {
            return Err(format!(
                "scenario {name} uses a fixed map and takes no world setup options"
            ));
        }
        (
            mapgen::generate(seed),
            None,
            Generator::Fractal,
            Some(setup.settings_metadata()),
        )
    };
    let (game, runtime) = scenario.start(map);
    Ok(Run {
        provenance: Provenance {
            kind: "named",
            name: name.into(),
            version: scenario.version,
            checks: vec![],
            observations: vec![],
            world: WorldProvenance::for_campaign(
                &game,
                size,
                (!scenario.uses_custom_world()).then_some(generator),
                (!scenario.uses_custom_world())
                    .then_some(settings)
                    .flatten(),
            ),
        },
        game,
        runtime: Some(runtime),
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
            observations: vec![],
            world: WorldProvenance::for_campaign(
                &game,
                Some(setup.size),
                Some(setup.generator),
                Some(setup.settings_metadata()),
            ),
        },
        game,
        runtime: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Paired {
        companion: Simulation,
        actions_pending: bool,
        before_calls: usize,
        before: Option<(u32, i32)>,
    }

    impl ScenarioRun for Paired {
        fn has_actions(&self) -> bool {
            self.actions_pending
        }
        fn execute_actions(&mut self, _game: &mut Simulation) -> Vec<Check> {
            self.actions_pending = false;
            vec![]
        }
        fn action_turns(&self) -> u32 {
            2
        }
        fn before_turn(&mut self, game: &mut Simulation, completed_turns: u32) {
            self.before_calls += 1;
            self.before = Some((completed_turns, game.turn()));
        }
        fn comparison_game(&mut self) -> Option<&mut Simulation> {
            Some(&mut self.companion)
        }
        fn observe_turn(&mut self, game: &Simulation, _turns: u32) -> serde_json::Value {
            serde_json::json!({
                "primary": game.turn(), "companion": self.companion.turn(),
                "before_calls": self.before_calls, "before": self.before,
            })
        }
    }

    fn world() -> WorldSetup {
        WorldSetup::parse(MapSize::Tiny, |_| Ok(None)).unwrap()
    }

    #[test]
    fn named_scenarios_are_validated() {
        assert!(build("missing", 42, world()).is_err());
    }

    #[test]
    fn runner_advances_companions_once_for_preparatory_and_ordinary_turns() {
        let mut run = generated(42, world());
        let initial = run.game.turn();
        run.runtime = Some(Box::new(Paired {
            companion: run.game.clone(),
            actions_pending: true,
            before_calls: 0,
            before: None,
        }));
        run.observe_turn(0);
        assert_eq!(
            run.provenance.observations.last().unwrap().state["companion"],
            initial
        );
        run.execute_actions();
        run.observe_turn(0);
        assert_eq!(
            run.provenance.observations.last().unwrap().state["companion"],
            initial + 2
        );
        assert_eq!(run.game.turn(), initial + 2);
        assert_eq!(
            run.provenance.observations.last().unwrap().state["before_calls"],
            0
        );
        run.execute_actions();
        assert_eq!(run.game.turn(), initial + 2);
        run.simulate_turn(0);
        run.observe_turn(1);
        let state = run.provenance.observations.last().unwrap().state.clone();
        assert_eq!(state["primary"], initial + 3);
        assert_eq!(state["companion"], initial + 3);
        assert_eq!(state["before_calls"], 1);
        assert_eq!(state["before"], serde_json::json!([0, initial + 2]));
        run.observe_turn(1);
        assert_eq!(run.provenance.observations.last().unwrap().state, state);
        let peer = run.runtime.as_mut().unwrap().comparison_game().unwrap();
        assert_eq!(
            run.game.encode_campaign().unwrap(),
            peer.encode_campaign().unwrap()
        );
    }

    #[test]
    fn observing_a_paired_trait_campaign_does_not_advance_or_mutate_campaign_state() {
        let mut run = build("trait-campaign", 42, world()).unwrap();
        let primary = run.game.encode_campaign().unwrap();
        let companion = run
            .runtime
            .as_mut()
            .unwrap()
            .comparison_game()
            .unwrap()
            .encode_campaign()
            .unwrap();
        run.observe_turn(0);
        assert_eq!(run.game.encode_campaign().unwrap(), primary);
        assert_eq!(
            run.runtime
                .as_mut()
                .unwrap()
                .comparison_game()
                .unwrap()
                .encode_campaign()
                .unwrap(),
            companion
        );
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
        let mut run = build("city-healing", 42, world()).unwrap();
        assert!(run.has_actions());
        assert!(run.provenance.checks.is_empty());
        run.execute_actions();
        assert!(!run.has_actions());
        assert!(run.provenance.checks.iter().all(|check| check.passed));
        let checks = run.provenance.checks.clone();
        run.execute_actions();
        run.observe_turn(0);
        run.finish(0);
        assert_eq!(run.provenance.checks, checks);
        assert!(run.provenance.observations.is_empty());
    }

    #[test]
    fn campaign_scenario_finishes_with_failed_checks_when_no_turn_was_run() {
        let mut run = build("naval-ship-combat", 42, world()).unwrap();
        assert!(!run.has_actions());
        assert!(run.provenance.checks.is_empty());
        run.finish(0);
        assert_eq!(run.provenance.version, 2);
        assert_eq!(run.provenance.checks.len(), 2);
        assert!(run.provenance.checks.iter().all(|check| !check.passed));
        let checks = run.provenance.checks.clone();
        run.finish(0);
        assert_eq!(run.provenance.checks, checks);
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
