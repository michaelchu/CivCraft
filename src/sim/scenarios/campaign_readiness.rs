//! A campaign already producing, researching, and carrying aircraft.

use serde::Serialize;
use serde_json::{Value, json};

use super::{
    builder::{CivPreset, UnitReadiness, WorldBuilder},
    regression::{Check, Playtest, Scenario},
};
use crate::{
    rules::{Building, Tech},
    sim::{Pact, Production, Simulation, UnitKind, UnitState},
    world::GeneratedMap,
};

const HOME: (i32, i32) = (7, 6);
const WORKED: (i32, i32) = (8, 6);
const RATES: [i32; 3] = [60, 30, 10];
const TREASURY: i32 = 80;
const RESEARCHER: usize = 1;
const PROJECT: Tech = Tech::ALPHABET;
const BUILD: Production = Production::Unit(UnitKind::Worker);
const NEXT_BUILD: Production = Production::Building(Building::HARBOR);

pub(super) const SCENARIO: Scenario = Scenario::playtest("campaign-readiness-presets", setup);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn Playtest>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: Some(&[Tech::POTTERY, Tech::MASONRY, Tech::MAP_MAKING]),
            },
            CivPreset {
                id: "germany",
                technologies: Some(&[]),
            },
        ],
    );
    let progress = game.target_cost(RESEARCHER, PROJECT) - 1;
    let fighter_fuel = UnitKind::Fighter.fuel_capacity();
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(20, 1900);
    let foreign_port = (19, 29);
    for position in [HOME, WORKED, foreign_port] {
        world.grassland(position);
    }
    let city = world.city(0, HOME, 2);
    world.city(1, foreign_port, 1);
    world.diplomacy(0, 1, Pact::Peace, 1, [7500, 7500]);
    world.city_improvement(city, Building::CITY_WALLS);
    let ship = world.unit(0, UnitKind::Carrier, HOME);
    let aircraft = world.unit(0, UnitKind::Fighter, HOME);
    let guard = world.unit(0, UnitKind::Warrior, HOME);
    world.load_cargo(aircraft, ship);
    world.unit_readiness(
        aircraft,
        UnitReadiness {
            moves_left: 0,
            fuel: fighter_fuel,
            state: UnitState::Idle,
        },
    );
    world.unit_readiness(
        guard,
        UnitReadiness {
            moves_left: 0,
            fuel: 0,
            state: UnitState::Fortified,
        },
    );
    world.city_production(city, BUILD, BUILD.cost(), &[NEXT_BUILD]);
    world.city_workforce(city, &[WORKED], [0, 0, 1]);
    world.economy(RESEARCHER, TREASURY, RATES);
    world.research(RESEARCHER, PROJECT, progress, Some(PROJECT));
    world.finish();

    let ids = FixtureIds {
        city,
        ship,
        aircraft,
        guard,
    };
    let initial = State::capture(&game, &ids);
    (
        game,
        Box::new(Readiness {
            ids,
            initial,
            opening: None,
            progress,
            fighter_fuel,
        }),
    )
}

#[derive(Serialize)]
struct FixtureIds {
    city: u32,
    ship: u32,
    aircraft: u32,
    guard: u32,
}

struct Readiness {
    ids: FixtureIds,
    initial: State,
    opening: Option<State>,
    progress: i32,
    fighter_fuel: u8,
}

#[derive(Serialize)]
struct State {
    city_walls: bool,
    worker_exists: bool,
    production: Option<String>,
    shields: Option<i32>,
    worklist: Vec<String>,
    worked_tiles: Vec<(i32, i32)>,
    specialists: Option<[i32; 3]>,
    aboard: bool,
    aircraft_fuel: Option<u8>,
    aircraft_moves: Option<i32>,
    guard_state: Option<UnitState>,
    guard_moves: Option<i32>,
    gold: i32,
    rates: [i32; 3],
    researching: Tech,
    research_goal: Tech,
    progress: i32,
    project_known: bool,
}

impl State {
    fn capture(game: &Simulation, ids: &FixtureIds) -> Self {
        let city = game.city_by_id(ids.city);
        let aircraft = game.unit_by_id(ids.aircraft);
        let guard = game.unit_by_id(ids.guard);
        let player = &game.players()[RESEARCHER];
        Self {
            city_walls: city.is_some_and(|city| city.has_building(Building::CITY_WALLS)),
            worker_exists: game
                .units_owned_by(0)
                .any(|unit| unit.kind == UnitKind::Worker),
            production: city.map(|city| city.production.name().to_owned()),
            shields: city.map(|city| city.shields),
            worklist: city.map_or_else(Vec::new, |city| {
                city.worklist
                    .iter()
                    .map(|item| item.name().to_owned())
                    .collect()
            }),
            worked_tiles: city.map_or_else(Vec::new, |city| city.worked_tiles.clone()),
            specialists: city.map(|city| city.specialists),
            aboard: aircraft.is_some_and(|unit| unit.transport == Some(ids.ship)),
            aircraft_fuel: aircraft.map(|unit| unit.fuel),
            aircraft_moves: aircraft.map(|unit| unit.moves_left),
            guard_state: guard.map(|unit| unit.state),
            guard_moves: guard.map(|unit| unit.moves_left),
            gold: player.gold,
            rates: player.rates,
            researching: player.researching,
            research_goal: player.research_goal,
            progress: player.research,
            project_known: game.has_tech(RESEARCHER, PROJECT),
        }
    }
}

impl Playtest for Readiness {
    fn observe_turn(&mut self, game: &Simulation, completed_turns: u32) -> Value {
        let state = State::capture(game, &self.ids);
        let observation = json!({
            "ids": self.ids,
            "initial": (completed_turns == 1).then_some(&self.initial),
            "current": state,
        });
        if completed_turns == 1 {
            self.opening = Some(state);
        }
        observation
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let starting = &self.initial;
        let end = self.opening.as_ref();
        vec![
            Check {
                name: "readiness_city_setup",
                passed: starting.city_walls
                    && !starting.worker_exists
                    && starting.production.as_deref() == Some(BUILD.name())
                    && starting.shields == Some(BUILD.cost())
                    && starting.worklist == [NEXT_BUILD.name()]
                    && starting.worked_tiles == [WORKED]
                    && starting.specialists == Some([0, 0, 1]),
                detail: "City Walls, assigned citizens, stored shields, and a queued Harbor are present at setup".into(),
            },
            Check {
                name: "readiness_production_completed",
                passed: end.is_some_and(|state| {
                    state.worker_exists
                        && state.production.as_deref() == Some(NEXT_BUILD.name())
                        && state.worklist.is_empty()
                }),
                detail: "Turn 1 completes the Worker and advances the Harbor from the worklist".into(),
            },
            Check {
                name: "readiness_units",
                passed: starting.aboard
                    && starting.aircraft_fuel == Some(self.fighter_fuel)
                    && starting.aircraft_moves == Some(0)
                    && starting.guard_state == Some(UnitState::Fortified)
                    && starting.guard_moves == Some(0)
                    && end.is_some_and(|state| {
                        state.aboard && state.aircraft_moves.is_some_and(|moves| moves > 0)
                    }),
                detail: "The city starts with a fortified guard, and its embarked Fighter regains movement on turn 1".into(),
            },
            Check {
                name: "readiness_economy_setup",
                passed: starting.gold == TREASURY && starting.rates == RATES,
                detail: "The treasury and science, tax, and luxury rates match the fixture".into(),
            },
            Check {
                name: "readiness_research_completed",
                passed: !starting.project_known
                    && starting.researching == PROJECT
                    && starting.research_goal == PROJECT
                    && starting.progress == self.progress
                    && end.is_some_and(|state| state.project_known),
                detail: "Turn 1 completes the partially researched Alphabet".into(),
            },
        ]
    }
}
