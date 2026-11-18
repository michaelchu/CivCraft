//! An isolated hunt on open water, driven by the regular naval AI.

use crate::{
    sim::{HUMAN, Simulation, Unit, UnitKind, offset_native_by_map},
    world::GeneratedMap,
};
use serde_json::{Value, json};

use super::super::{
    builder::WorldBuilder,
    regression::{Check, Playtest, Scenario},
};

const HUNTER: usize = 1;
const ELITE: usize = 3;

pub(in crate::sim::scenarios) const SCENARIO: Scenario =
    Scenario::playtest("naval-ship-combat", setup);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn Playtest>) {
    let mut game = WorldBuilder::ocean_campaign(map);
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(16, 1900);
    let start = (10, 24);
    let target = offset_native_by_map(start.0, start.1, 1, 0);
    let attacker = world.unit(HUNTER, UnitKind::Submarine, start);
    let defender = world.unit(HUMAN, UnitKind::Battleship, target);
    // Give the hunter favorable odds against a damaged capital ship.
    world.unit_experience_rank(attacker, ELITE);
    world.unit_hit_points(defender, 1);
    // The faster ship is closing in. The hunter uses that prior position
    // to decide whether an intercept is possible.
    let previous = offset_native_by_map(start.0, start.1, 2, 0);
    world.previous_unit_position(defender, previous);
    world.war(HUNTER, HUMAN, 1, [-10_000, -10_000]);
    world.refresh_visibility();
    // These are the only ships and there are no cities that can build more.
    // A new combat kill credited to the hunter therefore identifies the duel.
    let starting_kills = game.nation_scores[HUNTER].units_killed;
    (
        game,
        Box::new(Duel {
            attacker,
            defender,
            starting_kills,
            opening: None,
        }),
    )
}

struct Duel {
    attacker: u32,
    defender: u32,
    starting_kills: i32,
    opening: Option<Outcome>,
}

struct Outcome {
    attacker_survived: bool,
    defender_sunk: bool,
    combat_kills: i32,
}

impl Playtest for Duel {
    fn observe_turn(&mut self, game: &Simulation, completed_turns: u32) -> Value {
        let attacker = game.unit_by_id(self.attacker);
        let defender = game.unit_by_id(self.defender);
        let combat_kills = game.nation_scores[HUNTER].units_killed - self.starting_kills;
        if completed_turns == 1 {
            self.opening = Some(Outcome {
                attacker_survived: attacker.is_some(),
                defender_sunk: defender.is_none(),
                combat_kills,
            });
        }
        let ship = |id, unit: Option<&Unit>| {
            json!({
                "id": id,
                "state": unit.map(|unit| json!({
                    "position": [unit.x, unit.y],
                    "hit_points": unit.hp,
                    "moves_left": unit.moves_left,
                })),
            })
        };
        json!({
            "attacker": ship(self.attacker, attacker),
            "defender": ship(self.defender, defender),
            "combat_kills": combat_kills,
        })
    }

    fn finish(self: Box<Self>, _completed_turns: u32) -> Vec<Check> {
        let opening = self.opening.as_ref();
        vec![
            Check {
                name: "naval_combat_defender_sunk",
                passed: opening.is_some_and(|turn| turn.defender_sunk && turn.combat_kills == 1),
                detail: opening.map_or_else(
                    || "The first campaign turn was not observed".into(),
                    |turn| {
                        format!(
                            "After one turn: Battleship removed={}, hunter combat kills={}",
                            turn.defender_sunk, turn.combat_kills,
                        )
                    },
                ),
            },
            Check {
                name: "naval_combat_attacker_survived",
                passed: opening.is_some_and(|turn| turn.attacker_survived),
                detail: "The original Submarine survives the opening engagement".into(),
            },
        ]
    }
}
