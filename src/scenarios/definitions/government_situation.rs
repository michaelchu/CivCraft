//! Situational AI governments: peace/war bias, trait contrast and stability.
//!
//! Each run pairs a militaristic nation (Rome) with an otherwise identical
//! non-militaristic one (India) in a matched comparison campaign, so the trait
//! effect is isolated from the rest of the world.

use super::*;
use crate::{
    rules::Tech,
    sim::{
        Government, Pact, Simulation, UnitKind,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
        trace::Collector,
    },
    world::{GeneratedMap, Terrain},
};
use serde_json::{Value, json};

const RIVAL: usize = 0;
const SUBJECT: usize = 1;
const EVENTS: &[&str] = &["government.changed"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    /// Both nations are at war from the first turn.
    War,
    /// Both nations are at peace and start in a military government.
    Peace,
    /// War, then peace, then war again, to catch flip-flops and loops.
    Cycle,
    /// A Republic whose city has been rioting: leave it once, never repeat it.
    Disorder,
}

pub(super) const WAR: Scenario = scenario("government-situation-war", war_setup);
pub(super) const PEACE: Scenario = scenario("government-situation-peace", peace_setup);
pub(super) const CYCLE: Scenario = scenario("government-situation-cycle", cycle_setup);
pub(super) const DISORDER: Scenario = scenario("government-situation-disorder", disorder_setup);

const fn scenario(id: &'static str, setup: crate::scenarios::ScenarioSetup) -> Scenario {
    Scenario::new(
        id,
        1,
        ScenarioKind::Runtime {
            control: Control::Human,
            world: RuntimeWorld::Custom,
            setup,
        },
    )
}

fn war_setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    build(map, Case::War)
}

fn peace_setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    build(map, Case::Peace)
}

fn cycle_setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    build(map, Case::Cycle)
}

fn disorder_setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    build(map, Case::Disorder)
}

fn build(map: GeneratedMap, case: Case) -> (Simulation, Box<dyn ScenarioRun>) {
    let technologies = [
        Tech::ALPHABET,
        Tech::BRONZE_WORKING,
        Tech::MONARCHY,
        Tech::THE_REPUBLIC,
    ];
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &["egypt", "rome"].map(|id| CivPreset {
            id,
            technologies: Some(&technologies),
        }),
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(100, -1000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    for (owner, position, size) in [
        (SUBJECT, (10, 24), 4),
        (SUBJECT, (14, 24), 4),
        (RIVAL, (10, 40), 3),
    ] {
        let city = world.city(owner, position, size);
        world.city_workers(city, size as usize);
        world.unit(owner, UnitKind::Spearman, position);
        if case == Case::Disorder && owner == SUBJECT && position == (10, 24) {
            world.city_disorder_streak(city, 10);
        }
    }
    world.treasury(SUBJECT, 500);
    world.last_science(SUBJECT, 20);
    let pact = if matches!(case, Case::Peace | Case::Disorder) {
        Pact::Peace
    } else {
        Pact::War
    };
    world.diplomacy(SUBJECT, RIVAL, pact, 1, [0; 2]);
    match case {
        Case::Peace => world.government(SUBJECT, Government::MONARCHY),
        Case::Disorder => world.government(SUBJECT, Government::REPUBLIC),
        _ => {}
    }
    world.finish();
    let mut control = game.clone();
    WorldBuilder::new(&mut control).civilization_identity(SUBJECT, "india");
    control.trace = Collector::enabled(EVENTS.iter().map(|kind| (*kind).into()).collect(), 1024);
    (
        game,
        Box::new(Pair {
            case,
            control,
            militaristic: Track::default(),
            peaceful: Track::default(),
        }),
    )
}

#[derive(Default)]
struct Track {
    start: Option<&'static str>,
    governments: Vec<&'static str>,
    revolutions: Vec<(u32, String, String)>,
}

impl Track {
    fn observe(&mut self, game: &Simulation, turn: u32) -> Value {
        let government = game.players()[SUBJECT].government.name();
        self.start.get_or_insert(government);
        self.governments.push(government);
        for event in game.trace.pending_events() {
            let payload = &event.payload;
            if event.kind == "government.changed"
                && payload["owner"] == SUBJECT
                && payload["reason"] == "revolution_started"
            {
                self.revolutions.push((
                    turn,
                    payload["previous"].as_str().unwrap_or_default().to_owned(),
                    payload["target"].as_str().unwrap_or_default().to_owned(),
                ));
            }
        }
        json!({"government": government, "revolutions": self.revolutions.len()})
    }

    fn last(&self) -> &'static str {
        self.governments.last().copied().unwrap_or_default()
    }

    /// A revolution into the government the nation already has is a loop.
    fn repeats(&self) -> usize {
        self.revolutions
            .iter()
            .filter(|(_, previous, target)| previous == target)
            .count()
    }

    /// A revolution back into the government the previous one left, within
    /// a few turns: A -> B -> A.
    fn flip_flops(&self) -> usize {
        self.revolutions
            .windows(2)
            .filter(|pair| pair[1].0 - pair[0].0 <= 30 && pair[1].2 == pair[0].1)
            .count()
    }
}

fn in_family(names: &[String], government: &str) -> bool {
    names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(government))
}

struct Pair {
    case: Case,
    control: Simulation,
    militaristic: Track,
    peaceful: Track,
}

impl ScenarioRun for Pair {
    fn trace_kinds(&self) -> &'static [&'static str] {
        EVENTS
    }

    fn comparison_game(&mut self) -> Option<&mut Simulation> {
        Some(&mut self.control)
    }

    fn before_turn(&mut self, game: &mut Simulation, completed_turns: u32) {
        if self.case != Case::Cycle {
            return;
        }
        let pact = match completed_turns {
            8 => Pact::Peace,
            16 => Pact::War,
            _ => return,
        };
        game.sign_pact(SUBJECT, RIVAL, pact);
        self.control.sign_pact(SUBJECT, RIVAL, pact);
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let militaristic = self.militaristic.observe(game, turn);
        let peaceful = self.peaceful.observe(&self.control, turn);
        self.control.trace.drain();
        json!({"rome": militaristic, "india": peaceful})
    }

    fn finish(self: Box<Self>, turns: u32) -> Vec<Check> {
        let catalog = crate::rules::native::government_situation();
        let (rome, india) = (&self.militaristic, &self.peaceful);
        let mut checks = vec![Check {
            name: "no_government_loops",
            passed: turns > 0
                && rome.repeats() == 0
                && india.repeats() == 0
                && rome.flip_flops() == 0
                && india.flip_flops() == 0,
            detail: format!(
                "Rome revolutions {:?}, India revolutions {:?}",
                rome.revolutions, india.revolutions
            ),
        }];
        match self.case {
            Case::War => checks.push(Check {
                name: "militaristic_holds_war_government",
                passed: in_family(&catalog.war, rome.last()),
                detail: format!(
                    "Rome ended in {}; India (non-militaristic) in {}",
                    rome.last(),
                    india.last()
                ),
            }),
            Case::Peace => checks.push(Check {
                name: "peace_moves_peaceful_civilization_and_militaristic_keeps_military",
                passed: in_family(&catalog.peace, india.last())
                    && in_family(&catalog.war, rome.last()),
                detail: format!(
                    "Both started in {}; Rome (militaristic) ended in {}, India in {}",
                    rome.start.unwrap_or_default(),
                    rome.last(),
                    india.last()
                ),
            }),
            Case::Cycle => checks.push(Check {
                name: "bounded_revolutions_across_war_peace_war",
                // Rome keeps its military government throughout; India takes the
                // peace government once and has no churn in either direction.
                passed: rome.revolutions.len() <= 1
                    && india.revolutions.len() <= 3
                    && in_family(&catalog.war, rome.last())
                    && india
                        .governments
                        .iter()
                        .any(|g| in_family(&catalog.peace, g)),
                detail: format!(
                    "Rome {} and India {} revolutions over war, peace, war; Rome {:?}, India {:?}",
                    rome.revolutions.len(),
                    india.revolutions.len(),
                    rome.governments,
                    india.governments
                ),
            }),
            Case::Disorder => checks.push(Check {
                name: "disorder_prone_government_is_left_once",
                passed: [rome, india].iter().all(|track| {
                    track.revolutions.first().is_some_and(|(_, from, to)| {
                        from == "Republic" && to != "Republic" && to != "Democracy"
                    }) && track.revolutions.len() == 1
                }),
                detail: format!(
                    "Rome revolutions {:?}, India revolutions {:?}",
                    rome.revolutions, india.revolutions
                ),
            }),
        }
        checks
    }
}

#[cfg(test)]
mod tests {
    //! Runs the registered runtimes through the headless runner at their fixed seeds.

    fn run(scenario: &str, seed: u64, turns: u32) -> serde_json::Value {
        let report = std::env::temp_dir().join(format!(
            "civcraft-{scenario}-{seed}-{}.json",
            std::process::id()
        ));
        let args: Vec<String> = [
            "--scenario",
            scenario,
            "--seed",
            &seed.to_string(),
            "--turns",
            &turns.to_string(),
            "--report",
            report.to_str().unwrap(),
        ]
        .map(String::from)
        .to_vec();
        crate::headless::run_cli(&args).unwrap_or_else(|e| panic!("{scenario} seed {seed}: {e}"));
        let value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        for extension in [
            "",
            ".changes.jsonl",
            ".events.jsonl",
            ".shared.jsonl",
            ".snapshots.jsonl",
        ] {
            let _ = std::fs::remove_file(format!("{}{extension}", report.display()));
        }
        value
    }

    #[test]
    fn government_situation_runtimes_pass_at_every_seed() {
        for (scenario, turns) in [
            ("government-situation-war", 8),
            ("government-situation-peace", 8),
            ("government-situation-cycle", 24),
            ("government-situation-disorder", 4),
        ] {
            for seed in [42, 7, 99] {
                let report = run(scenario, seed, turns);
                let total = report["summary"]["scenario_checks_total"].as_u64().unwrap();
                assert!(total > 0, "{scenario} made no checks");
                assert_eq!(
                    report["summary"]["scenario_checks_passed"]
                        .as_u64()
                        .unwrap(),
                    total,
                    "{scenario} seed {seed}: {}",
                    report["run"]["scenario"]["checks"]
                );
                let churn = report["findings"].as_array().unwrap().iter().any(|f| {
                    matches!(
                        f["kind"].as_str(),
                        Some("government_flip_flop" | "government_revolution_repeats_current")
                    )
                });
                assert!(!churn, "{scenario} seed {seed} reported government churn");
            }
        }
    }

    /// Slow (three 100-turn campaigns): run in release with
    /// `cargo test --release government_situation_long_runs -- --ignored`.
    #[test]
    #[ignore = "three 100-turn tiny-map campaigns; run in release mode"]
    fn government_situation_long_runs_have_no_government_churn() {
        for seed in [42, 7, 99] {
            let report = std::env::temp_dir().join(format!(
                "civcraft-government-long-{seed}-{}.json",
                std::process::id()
            ));
            let args: Vec<String> = [
                "--seed",
                &seed.to_string(),
                "--map-size",
                "tiny",
                "--turns",
                "100",
                "--report",
                report.to_str().unwrap(),
            ]
            .map(String::from)
            .to_vec();
            crate::headless::run_cli(&args).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
            for extension in [
                "",
                ".changes.jsonl",
                ".events.jsonl",
                ".shared.jsonl",
                ".snapshots.jsonl",
            ] {
                let _ = std::fs::remove_file(format!("{}{extension}", report.display()));
            }
            let churn: Vec<_> = value["findings"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| {
                    matches!(
                        f["kind"].as_str(),
                        Some("government_flip_flop" | "government_revolution_repeats_current")
                    )
                })
                .map(|f| f["explanation"].clone())
                .collect();
            assert!(churn.is_empty(), "seed {seed}: {churn:?}");
        }
    }
}
