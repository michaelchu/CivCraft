use crate::scenarios::{Control, RuntimeWorld, ScenarioKind};
use crate::world::topology::offset_native_by_map;
use crate::{
    sim::{HUMAN, Pact, Simulation, UnitKind},
    world::GeneratedMap,
};
use serde_json::{Value, json};

use super::super::{Check, Scenario, ScenarioRun};
use crate::sim::fixtures::{CivPreset, WorldBuilder};

const ATTACKER: usize = 1;
const DEADLINE: u32 = 30;
type Position = (i32, i32);

pub(in crate::scenarios) const SCENARIO: Scenario = Scenario::new(
    "naval-ai-assault",
    2,
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
    world.start_at(16, -3000);
    let home = (10, 24);
    let point = |step| offset_native_by_map(home.0, home.1, step, 0);
    let island = [point(8), point(9)];
    for position in [home, island[0], island[1]] {
        world.grassland(position);
        world.mark_known(ATTACKER, position);
    }
    world.city(ATTACKER, home, 1);
    let target = world.city(HUMAN, island[1], 2);
    // An established hostile relationship avoids the grace period when
    // newly contacted civilizations accept ceasefires regardless of trust.
    world.diplomacy(ATTACKER, HUMAN, Pact::War, 1, [-10_000, -10_000]);
    let mut troops = Vec::new();
    for kind in [UnitKind::Marine, UnitKind::Warrior, UnitKind::Warrior] {
        troops.push(world.unit(ATTACKER, kind, home));
    }
    let ship_start = point(-1);
    let ship = world.unit(ATTACKER, UnitKind::Galleon, ship_start);
    for _ in 0..2 {
        troops.push(world.unit(ATTACKER, UnitKind::Marine, home));
    }
    world.finish();
    (
        game,
        Box::new(Assault {
            home,
            island,
            target,
            ship,
            ship_start,
            troops,
            embarked: Vec::new(),
            departure_turn: None,
            landing_turn: None,
            capture_turn: None,
            opening_guard: false,
        }),
    )
}

struct Assault {
    home: Position,
    island: [Position; 2],
    target: u32,
    ship: u32,
    ship_start: Position,
    troops: Vec<u32>,
    embarked: Vec<u32>,
    departure_turn: Option<u32>,
    landing_turn: Option<u32>,
    capture_turn: Option<u32>,
    opening_guard: bool,
}

impl ScenarioRun for Assault {
    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let ship = game.unit_by_id(self.ship);
        let target_owner = game.city_by_id(self.target).map(|city| city.owner);
        let troops: Vec<_> = self
            .troops
            .iter()
            .filter_map(|id| game.unit_by_id(*id))
            .filter(|unit| unit.owner == ATTACKER)
            .collect();
        let home_guard = troops.iter().any(|unit| {
            unit.kind == UnitKind::Marine
                && unit.transport.is_none()
                && (unit.x, unit.y) == self.home
        });
        if turn == 1 {
            self.opening_guard = home_guard;
        }
        if turn <= DEADLINE {
            for unit in &troops {
                if unit.transport == Some(self.ship) && !self.embarked.contains(&unit.id) {
                    self.embarked.push(unit.id);
                }
            }
            if troops.iter().any(|unit| {
                unit.transport == Some(self.ship)
                    && (unit.x, unit.y) != self.ship_start
                    && game.tile(unit.x, unit.y).terrain.is_water()
            }) {
                self.departure_turn.get_or_insert(turn);
            }
            if troops.iter().any(|unit| {
                unit.transport.is_none()
                    && self.embarked.contains(&unit.id)
                    && self.island.contains(&(unit.x, unit.y))
            }) {
                self.landing_turn.get_or_insert(turn);
            }
            if target_owner == Some(ATTACKER)
                && troops.iter().any(|unit| {
                    unit.transport.is_none()
                        && self.embarked.contains(&unit.id)
                        && (unit.x, unit.y) == self.island[1]
                })
            {
                self.capture_turn.get_or_insert(turn);
            }
        }
        json!({
            "ship_id": self.ship,
            "ship": ship.map(|unit| json!({"position": [unit.x, unit.y], "moves_left": unit.moves_left})),
            "troops": troops.iter().map(|unit| json!({
                "id": unit.id, "position": [unit.x, unit.y], "transport": unit.transport,
            })).collect::<Vec<_>>(),
            "home_guard": home_guard,
            "target_owner": target_owner,
            "at_war": game.at_war(ATTACKER, HUMAN),
        })
    }

    fn finish(self: Box<Self>, completed_turns: u32) -> Vec<Check> {
        let milestone = |name, turn: Option<u32>| Check {
            name,
            passed: turn.is_some(),
            detail: turn.map_or_else(
                || {
                    format!(
                        "Not observed in {} of {DEADLINE} allowed turns",
                        completed_turns.min(DEADLINE)
                    )
                },
                |turn| format!("Observed on completed turn {turn}"),
            ),
        };
        vec![
            Check {
                name: "naval_assault_guarded_home",
                passed: self.opening_guard,
                detail: "An original Marine guards the home city after the first turn".into(),
            },
            milestone("naval_assault_sailed_with_cargo", self.departure_turn),
            milestone("naval_assault_landed_cargo", self.landing_turn),
            milestone("naval_assault_captured_target", self.capture_turn),
        ]
    }
}
