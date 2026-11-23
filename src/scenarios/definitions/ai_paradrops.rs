//! Airborne reinforcement before an available conquest, through normal turns.
use super::{Check, Scenario, ScenarioRun};
use crate::scenarios::{Control, RuntimeWorld, ScenarioKind};
use crate::{
    rules::{Building, Tech},
    sim::{
        HUMAN, Pact, Production, Simulation, UnitKind,
        fixtures::{CivPreset, WorldBuilder, capture_known_tiles},
    },
    world::{GeneratedMap, topology::offset_native_by_map},
};
use serde_json::{Value, json};

const OWNER: usize = 1;
const DEADLINE: u32 = 2;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "ai-paradrops",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let known = [Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::ADVANCED_FLIGHT];
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: None,
            },
            CivPreset {
                id: "inca",
                technologies: Some(&known),
            },
        ],
    );
    let home = (10, 24);
    let front = offset_native_by_map(home.0, home.1, 6, 0);
    let enemy = offset_native_by_map(home.0, home.1, -6, 0);
    let threat = offset_native_by_map(front.0, front.1, 4, 0);
    let spotter = offset_native_by_map(threat.0, threat.1, 0, 1);
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(16, -3000);
    world.completed_great_wonders();
    for (owner, position, population) in [
        (OWNER, home, 1),
        (OWNER, front, 1),
        (HUMAN, enemy, 2),
        (HUMAN, (30, 4), 1),
    ] {
        world.grassland(position);
        let city = world.founding_city(owner, position);
        world.city_population(city, population, None);
        world.production_stock(city, Production::Building(Building::COINAGE), 0);
        world.mark_known(OWNER, position);
    }
    // A land approach makes the Horseman a real threat. It is too far away
    // for post-drop rampage, so this workflow does not depend on combat rolls.
    for step in 1..=4 {
        let position = offset_native_by_map(front.0, front.1, step, 0);
        world.grassland(position);
        world.mark_known(OWNER, position);
    }
    world.grassland(offset_native_by_map(enemy.0, enemy.1, 1, 0));
    // Vision support sits off the approach and stays in place until planning.
    world.grassland(spotter);
    world.unit(OWNER, UnitKind::Explorer, spotter);
    let hostile = world.unit(HUMAN, UnitKind::Horseman, threat);
    let guard = world.unit(OWNER, UnitKind::Warrior, home);
    let paratrooper = world.unit(OWNER, UnitKind::Paratrooper, home);
    // Prevent a last-activity rush from filling the empty front before planning.
    world.treasury(OWNER, 0);
    world.treasury(HUMAN, 5000);
    world.diplomacy(OWNER, HUMAN, Pact::War, 1, [-10_000; 2]);
    world.finish();
    capture_known_tiles(&mut game, OWNER);
    let enemy_city = game
        .cities()
        .iter()
        .find(|city| (city.x, city.y) == enemy)
        .unwrap()
        .id;
    let front_city = game
        .cities()
        .iter()
        .find(|city| (city.x, city.y) == front)
        .unwrap()
        .id;
    (
        game,
        Box::new(Reinforcement {
            home,
            front,
            front_city,
            enemy_city,
            hostile,
            guard,
            paratrooper,
            landed: None,
            home_guarded: None,
            defense_first: None,
        }),
    )
}

struct Reinforcement {
    home: (i32, i32),
    front: (i32, i32),
    front_city: u32,
    enemy_city: u32,
    hostile: u32,
    guard: u32,
    paratrooper: u32,
    landed: Option<u32>,
    home_guarded: Option<u32>,
    defense_first: Option<u32>,
}

impl ScenarioRun for Reinforcement {
    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let troop = game.unit_by_id(self.paratrooper);
        let guard = game.unit_by_id(self.guard);
        let enemy_owner = game.city_by_id(self.enemy_city).map(|city| city.owner);
        let front_owner = game.city_by_id(self.front_city).map(|city| city.owner);
        let hostile = game.unit_by_id(self.hostile);
        let landed = front_owner == Some(OWNER)
            && troop.is_some_and(|unit| unit.paradropped && (unit.x, unit.y) == self.front);
        if turn <= DEADLINE && landed {
            self.landed.get_or_insert(turn);
            if guard.is_some_and(|unit| unit.owner == OWNER && (unit.x, unit.y) == self.home) {
                self.home_guarded.get_or_insert(turn);
            }
            if enemy_owner == Some(HUMAN) && hostile.is_some() {
                self.defense_first.get_or_insert(turn);
            }
        }
        json!({
            "paratrooper": troop.map(|unit| json!({"id": unit.id, "position": [unit.x, unit.y], "paradropped": unit.paradropped})),
            "guard": guard.map(|unit| json!({"id": unit.id, "position": [unit.x, unit.y]})),
            "front": {"city_id": self.front_city, "position": self.front, "owner": front_owner},
            "enemy_city_owner": enemy_owner,
            "hostile": hostile.map(|unit| json!({"id": unit.id, "position": [unit.x, unit.y]})),
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
            milestone("paradrop_reinforced_front", self.landed),
            milestone("paradrop_preserved_home_guard", self.home_guarded),
            milestone("paradrop_prioritized_defense", self.defense_first),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_turns_reinforce_across_seeds_and_repeat_exactly() {
        let mut reference = None;
        for seed in [0, 1, 3, 18, 42, 99, u64::MAX, 42] {
            let (mut game, mut runtime) = SCENARIO.start(crate::mapgen::generate(seed));
            assert!(!runtime.has_actions());
            let mut observations = Vec::new();
            for turn in 1..=DEADLINE {
                assert!(
                    game.game_over().is_none(),
                    "seed {seed} ended before the deadline"
                );
                game.simulate_turn();
                observations.push(runtime.observe_turn(&game, turn));
            }
            for check in runtime.finish(DEADLINE) {
                assert!(
                    check.passed,
                    "seed {seed}: {}: {}. {observations:?}",
                    check.name, check.detail
                );
            }
            if seed == 42 {
                if let Some(first) = &reference {
                    assert_eq!(&observations, first);
                }
                reference = Some(observations);
            }
        }
    }

    #[test]
    fn finishing_before_any_turn_reports_unmet_milestones() {
        let (_, runtime) = SCENARIO.start(crate::test_support::seeded_map());
        assert!(runtime.finish(0).iter().all(|check| !check.passed));
    }
}
