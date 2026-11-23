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
}

pub(super) const WAR: Scenario = scenario("government-situation-war", war_setup);
pub(super) const PEACE: Scenario = scenario("government-situation-peace", peace_setup);
pub(super) const CYCLE: Scenario = scenario("government-situation-cycle", cycle_setup);

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
    }
    world.treasury(SUBJECT, 500);
    world.last_science(SUBJECT, 20);
    let pact = if case == Case::Peace {
        Pact::Peace
    } else {
        Pact::War
    };
    world.diplomacy(SUBJECT, RIVAL, pact, 1, [0; 2]);
    if case == Case::Peace {
        world.government(SUBJECT, Government::MONARCHY);
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

    /// Back to a government left within a few turns: A -> B -> A.
    fn flip_flops(&self) -> usize {
        let mut changes = Vec::new();
        for pair in self.governments.windows(2) {
            if pair[0] != pair[1] && pair[0] != "Anarchy" && pair[1] != "Anarchy" {
                changes.push((pair[0], pair[1]));
            }
        }
        changes
            .windows(2)
            .filter(|pair| pair[0].0 == pair[1].1 && pair[0].1 == pair[1].0)
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
        }
        checks
    }
}
