//! A deliberately wide culture empire against a compact one, through normal turns.
//!
//! Both nations start with the same culture buildings in every city. The wide
//! nation has three times as many cities. The scenario records how culture,
//! unrest and treasury scale, so a city-spam strategy that the empire-size and
//! upkeep rules fail to restrain shows up as a failed check.
use super::{Check, Scenario, ScenarioRun};
use crate::scenarios::{Control, RuntimeWorld, ScenarioKind};
use crate::{
    rules::Building,
    sim::{
        Government, HUMAN, Production, Simulation,
        fixtures::{CivPreset, WorldBuilder},
    },
    world::GeneratedMap,
};
use serde_json::{Value, json};

const WIDE: usize = 1;
const TALL: usize = HUMAN;
const TURNS: u32 = 40;
const CITY_SIZE: i32 = 4;
/// A set the early economy can carry. With Colosseums and Cathedrals as well,
/// both nations sell buildings from about turn 12 because their upkeep outruns
/// income, which is itself the brake on wide culture play.
const CULTURE_SET: [Building; 2] = [Building::TEMPLE, Building::LIBRARY];
/// Wide nation: a 4 by 3 grid at the minimum city distance.
const WIDE_SITES: [(i32, i32); 12] = [
    (6, 12),
    (9, 12),
    (12, 12),
    (15, 12),
    (6, 15),
    (9, 15),
    (12, 15),
    (15, 15),
    (6, 18),
    (9, 18),
    (12, 18),
    (15, 18),
];
/// Compact nation: a 2 by 2 grid in its own region.
const TALL_SITES: [(i32, i32); 4] = [(24, 32), (27, 32), (24, 35), (27, 35)];

pub(super) const SCENARIO: Scenario = Scenario::new(
    "culture-wide-vs-tall",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: None,
            },
            CivPreset {
                id: "inca",
                technologies: None,
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.clear_units_and_cities();
    world.fertile_tiles(None, None);
    for (owner, sites) in [(WIDE, &WIDE_SITES[..]), (TALL, &TALL_SITES[..])] {
        world.government_state(owner, Government::MONARCHY);
        world.treasury(owner, 600);
        for &site in sites {
            let city = world.founding_city(owner, site);
            world.city_population(city, CITY_SIZE, None);
            world.production_stock(city, Production::Building(Building::COINAGE), 0);
            for building in CULTURE_SET {
                world.culture_building(city, building);
            }
        }
    }
    world.settle_borders();
    world.finish();
    (game, Box::new(Run::default()))
}

#[derive(Clone, Copy, Default)]
struct Sample {
    cities: usize,
    culture: i64,
    per_turn: i64,
    gold: i32,
    disorder: usize,
}

#[derive(Default)]
struct Run {
    first: Option<[Sample; 2]>,
    last: Option<[Sample; 2]>,
    /// Upkeep losses (sold buildings, disbanded units) the wide nation took.
    wide_upkeep_losses: usize,
    trace_dropped: bool,
}

fn sample(game: &Simulation, owner: usize) -> Sample {
    let civilization = game
        .observe_civilizations()
        .into_iter()
        .find(|civilization| civilization.id == owner)
        .expect("scenario nation exists");
    Sample {
        cities: civilization.cities,
        culture: civilization.culture,
        per_turn: civilization.culture_per_turn,
        gold: civilization.gold,
        disorder: game
            .observe_cities()
            .into_iter()
            .filter(|city| city.owner == owner && city.disorder)
            .count(),
    }
}

fn describe(label: &str, s: Sample) -> String {
    format!(
        "{label}: {} cities, culture {}, +{}/turn, gold {}, {} in disorder",
        s.cities, s.culture, s.per_turn, s.gold, s.disorder
    )
}

impl ScenarioRun for Run {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["economy.upkeep_shortfall"]
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        self.wide_upkeep_losses += game
            .trace
            .pending_events()
            .iter()
            .filter(|event| {
                event.kind == "economy.upkeep_shortfall" && event.payload["owner"] == WIDE
            })
            .count();
        self.trace_dropped |= game.trace.dropped() > 0;
        let now = [sample(game, WIDE), sample(game, TALL)];
        if turn == 1 {
            self.first = Some(now);
        }
        self.last = Some(now);
        json!({"turn": turn, "wide": describe("wide", now[0]), "tall": describe("tall", now[1])})
    }

    fn finish(self: Box<Self>, completed_turns: u32) -> Vec<Check> {
        let (Some([wide0, tall0]), Some([wide, tall])) = (self.first, self.last) else {
            return vec![Check {
                name: "wide_tall_observed",
                passed: false,
                detail: "No turn was observed".into(),
            }];
        };
        // The compact nation also founds new cities during the run, so compare
        // totals against the starting city ratio rather than per-city rates.
        // The Palace adds one point to each capital.
        let disorder_share = |s: Sample| s.disorder * 100 / s.cities.max(1);
        let (wide_start, tall_start) = (wide0.cities as i64, tall0.cities as i64);
        vec![
            Check {
                name: "wide_tall_runs_full_length",
                passed: completed_turns >= TURNS,
                detail: format!(
                    "{completed_turns} of the {TURNS} turns the balance comparison needs were completed; run with --turns {TURNS}"
                ),
            },
            Check {
                name: "wide_tall_start_is_symmetric",
                passed: (wide0.per_turn - 1) * tall_start == (tall0.per_turn - 1) * wide_start,
                detail: format!("{}; {}", describe("wide", wide0), describe("tall", tall0)),
            },
            Check {
                name: "wide_culture_is_not_superlinear",
                // Allow 10% over the city ratio for rounding and AI differences.
                passed: wide.culture * tall_start <= tall.culture * wide_start * 11 / 10,
                detail: format!(
                    "After {completed_turns} of {TURNS} turns, {wide_start}:{tall_start} cities gave {}:{} culture. {}; {}",
                    wide.culture,
                    tall.culture,
                    describe("wide", wide),
                    describe("tall", tall)
                ),
            },
            Check {
                name: "wide_empire_stays_orderly",
                passed: disorder_share(wide) <= 25,
                detail: format!(
                    "{}% of the wide nation's cities are in disorder",
                    disorder_share(wide)
                ),
            },
            Check {
                name: "wide_empire_pays_for_its_buildings",
                passed: wide.gold >= 0 && self.wide_upkeep_losses == 0 && !self.trace_dropped,
                detail: format!(
                    "The wide treasury ends at {} gold after {} forced upkeep losses{}",
                    wide.gold,
                    self.wide_upkeep_losses,
                    if self.trace_dropped {
                        " (trace evidence was dropped)"
                    } else {
                        ""
                    }
                ),
            },
        ]
    }
}
