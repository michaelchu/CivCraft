//! Ocean trade should connect coastal cities only when both have harbors.

use serde::Serialize;
use serde_json::{Value, json};

use super::{
    builder::{CivPreset, TilePreset, WorldBuilder},
    regression::{Check, Playtest, Scenario},
};
use crate::{
    rules::{Building, Tech},
    sim::{Pact, Simulation},
    world::{GeneratedMap, Terrain, TerrainBase},
};

pub(super) const SCENARIO: Scenario = Scenario::playtest("harbor-trade-route", setup);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn Playtest>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "england",
                technologies: Some(&[Tech::MAP_MAKING]),
            },
            CivPreset {
                id: "germany",
                technologies: Some(&[Tech::MAP_MAKING]),
            },
            CivPreset {
                id: "rome",
                technologies: Some(&[Tech::MAP_MAKING]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(20, 1200);
    // One land tile per city, all surrounded by the same body of sea.
    let home = (7, 6);
    let remote = (19, 29);
    let control = (31, 42);
    for position in [home, remote, control] {
        world.tile(
            position,
            TilePreset::new(Terrain::Plains, TerrainBase::Plains),
        );
    }
    let from = world.city(0, home, 1);
    let to = world.city(1, remote, 1);
    let control = world.city(2, control, 1);
    world.city_improvement(from, Building::HARBOR);
    world.city_improvement(to, Building::HARBOR);
    for (first, second) in [(0, 1), (0, 2), (1, 2)] {
        world.diplomacy(first, second, Pact::Peace, 1, [7500, 7500]);
    }
    world.finish();

    let ports = [from, to];
    let initial = State::capture(&game, ports, control);
    (
        game,
        Box::new(HarborTrade {
            ports,
            control,
            initial,
            first_turn: None,
        }),
    )
}

struct HarborTrade {
    ports: [u32; 2],
    control: u32,
    initial: State,
    first_turn: Option<State>,
}

#[derive(Serialize)]
struct State {
    harbors: [bool; 3],
    route_count: usize,
    port_route_goods: Option<String>,
    port_route_income: [i32; 2],
    control_routes: usize,
}

impl State {
    fn capture(game: &Simulation, ports: [u32; 2], control: u32) -> Self {
        let [from, to] = ports;
        Self {
            harbors: [from, to, control].map(|id| {
                game.city_by_id(id)
                    .is_some_and(|city| city.has_building(Building::HARBOR))
            }),
            route_count: game.trade_routes.len(),
            port_route_goods: game
                .trade_routes
                .iter()
                .find(|route| route.unordered_city_pair() == (from.min(to), from.max(to)))
                .map(|route| route.goods_rule_name.clone()),
            port_route_income: ports.map(|id| {
                game.city_by_id(id)
                    .map_or(0, |city| game.trade_route_income(city, None))
            }),
            control_routes: game
                .trade_routes
                .iter()
                .filter(|route| route.from_city_id == control || route.to_city_id == control)
                .count(),
        }
    }
}

impl Playtest for HarborTrade {
    fn observe_turn(&mut self, game: &Simulation, completed_turns: u32) -> Value {
        let state = State::capture(game, self.ports, self.control);
        let observation = json!({
            "port_city_ids": self.ports,
            "control_city_id": self.control,
            "initial": (completed_turns == 1).then_some(&self.initial),
            "current": state,
        });
        if completed_turns == 1 {
            self.first_turn = Some(state);
        }
        observation
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let end = self.first_turn.as_ref();
        vec![
            Check {
                name: "harbor_trade_starts_unconnected",
                passed: self.initial.harbors == [true, true, false]
                    && self.initial.route_count == 0,
                detail: format!(
                    "Starting harbors (ports then control): {:?}, starting routes: {}",
                    self.initial.harbors, self.initial.route_count,
                ),
            },
            Check {
                name: "harbor_trade_forms_on_first_turn",
                passed: self.initial.route_count == 0
                    && end.is_some_and(|s| {
                        s.harbors == [true, true, false]
                            && s.route_count == 1
                            && s.port_route_goods.as_deref() == Some("Goods")
                            && s.port_route_income.iter().all(|&income| income > 0)
                    }),
                detail: end.map_or_else(
                    || "The first campaign turn was not observed".into(),
                    |s| {
                        format!(
                            "After turn 1: {} route(s), port goods {:?}, port income {:?}",
                            s.route_count, s.port_route_goods, s.port_route_income,
                        )
                    },
                ),
            },
            Check {
                name: "harbor_trade_excludes_city_without_harbor",
                passed: end.is_some_and(|s| !s.harbors[2] && s.control_routes == 0),
                detail: "The coastal control city has no harbor and acquires no route".into(),
            },
        ]
    }
}
