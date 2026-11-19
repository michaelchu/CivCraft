//! Exercise existing governments, diplomatic access and a city trade route.

use crate::sim::fixtures::{CivPreset, WorldBuilder};
use serde::Serialize;
use serde_json::{Value, json};

use super::{Check, Scenario, ScenarioRun};
use crate::{
    rules::Tech,
    sim::{Government, Pact, Simulation},
    world::GeneratedMap,
};

const REPUBLIC: usize = 0;
const MONARCHY: usize = 1;
const TURNS: usize = 2;
const GOODS: &str = "Goods";

pub(super) const SCENARIO: Scenario = Scenario::campaign("campaign-state-presets", setup);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "england",
                technologies: Some(&[Tech::THE_REPUBLIC]),
            },
            CivPreset {
                id: "germany",
                technologies: Some(&[Tech::MONARCHY]),
            },
            // Keep a third civilization alive so the alliance doesn't end the game.
            CivPreset {
                id: "rome",
                technologies: Some(&[]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(20, 1700);
    world.government(REPUBLIC, Government::Republic);
    world.government(MONARCHY, Government::Monarchy);

    let capital = (7, 6);
    let remote = (19, 29);
    for position in [capital, remote, (31, 42)] {
        world.grassland(position);
    }
    let from = world.city(REPUBLIC, capital, 1);
    let to = world.city(MONARCHY, remote, 1);
    world.city(2, (31, 42), 1);
    world.diplomacy(REPUBLIC, MONARCHY, Pact::Alliance, 1, [7500, 7500]);
    world.embassy(REPUBLIC, MONARCHY);
    world.shared_vision(MONARCHY, REPUBLIC);
    world.trade_route(from, to, GOODS);
    world.finish();

    let cities = [from, to];
    let initial = State::capture(&game, cities, remote);
    (
        game,
        Box::new(Presets {
            cities,
            remote,
            initial,
            turns: Vec::new(),
        }),
    )
}

struct Presets {
    cities: [u32; 2],
    remote: (i32, i32),
    initial: State,
    turns: Vec<State>,
}

#[derive(Serialize)]
struct State {
    governments: [Government; 2],
    revolutions_pending: bool,
    embassy: [bool; 2],
    shared_vision: [bool; 2],
    remote_city_visible_to_republic: bool,
    route_present: bool,
    route_income: [i32; 2],
}

impl State {
    fn capture(game: &Simulation, cities: [u32; 2], remote: (i32, i32)) -> Self {
        let [from, to] = cities;
        State {
            governments: [REPUBLIC, MONARCHY].map(|owner| game.players()[owner].government),
            revolutions_pending: [REPUBLIC, MONARCHY]
                .into_iter()
                .any(|owner| game.players()[owner].revolution_target.is_some()),
            embassy: [
                game.has_real_embassy(REPUBLIC, MONARCHY),
                game.has_real_embassy(MONARCHY, REPUBLIC),
            ],
            shared_vision: [
                game.gives_shared_vision(REPUBLIC, MONARCHY),
                game.gives_shared_vision(MONARCHY, REPUBLIC),
            ],
            remote_city_visible_to_republic: game.has_main_sight_at(REPUBLIC, remote.0, remote.1),
            route_present: game.trade_routes().iter().any(|route| {
                route.from_city_id == from
                    && route.to_city_id == to
                    && route.goods_rule_name == GOODS
            }),
            route_income: cities.map(|id| {
                game.city_by_id(id)
                    .map_or(0, |city| game.trade_route_income(city, None))
            }),
        }
    }
}

impl ScenarioRun for Presets {
    fn observe_turn(&mut self, game: &Simulation, completed_turns: u32) -> Value {
        let state = State::capture(game, self.cities, self.remote);
        let observation = json!({
            "city_ids": self.cities,
            "initial": (completed_turns == 1).then_some(&self.initial),
            "current": state,
        });
        if self.turns.len() < TURNS {
            self.turns.push(state);
        }
        observation
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let setup = &self.initial;
        let maintained = |matches: fn(&State) -> bool| {
            matches(setup) && self.turns.len() == TURNS && self.turns.iter().all(matches)
        };
        let detail = |expectation: &str| {
            format!(
                "{expectation}. Observed {} of {TURNS} required turns",
                self.turns.len()
            )
        };
        vec![
            Check {
                name: "presets_governments",
                passed: setup.governments == [Government::Republic, Government::Monarchy]
                    && !setup.revolutions_pending,
                detail: format!(
                    "Expected Republic and Monarchy at setup with no pending revolution. Found {:?}, pending revolution={}",
                    setup.governments, setup.revolutions_pending
                ),
            },
            Check {
                name: "presets_embassy",
                passed: setup.embassy == [true, false] && maintained(|s| s.embassy[0]),
                detail: detail("England starts with a one-way embassy in Germany and retains it"),
            },
            Check {
                name: "presets_shared_vision",
                passed: setup.shared_vision == [false, true]
                    && maintained(|s| s.shared_vision[1] && s.remote_city_visible_to_republic),
                detail: detail("Germany shares live sight of its remote capital with England"),
            },
            Check {
                name: "presets_trade_route",
                passed: maintained(|s| {
                    s.route_present && s.route_income.iter().all(|&income| income > 0)
                }),
                detail: detail(
                    "The preset city route remains active and generates income at both ends",
                ),
            },
        ]
    }
}
