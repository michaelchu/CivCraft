//! Build a ferry and founder, transport that founder, and settle through normal turns.
use super::{Check, Scenario, ScenarioRun};
use crate::scenarios::{Control, RuntimeWorld, ScenarioKind};
use crate::{
    rules::Tech,
    sim::{
        Production, Simulation, UnitKind,
        fixtures::{CivPreset, TilePreset, TileSurface, WorldBuilder},
    },
    world::{GeneratedMap, Resource, Terrain, TerrainBase, topology::offset_native_by_map},
};
use serde_json::{Value, json};

const OWNER: usize = 1;
const DEADLINE: u32 = 16;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "settlement-ferry",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    // Fixed identities remove seed-selected unit masks and policy traits from this workflow.
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: None,
            },
            CivPreset {
                id: "inca",
                technologies: Some(&[Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::MAP_MAKING]),
            },
        ],
    );
    let map = game.map_wrap;
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(16, -3000);
    // Keep the expedition's production choices focused on ordinary units and improvements.
    world.completed_great_wonders();
    // A quiet, isolated human city keeps the campaign alive during the voyage.
    world.grassland((30, 4));
    world.founding_city(0, (30, 4));
    let home = (10, 24);
    let island = offset_native_by_map(home.0, home.1, 6, 0);
    world.grassland(home);
    world.terrain(home, Terrain::Forest);
    world.grassland(island);
    let productive = offset_native_by_map(island.0, island.1, 1, 0);
    world.grassland(productive);
    world.mark_known(1, productive);
    world.mark_known(1, island);
    let fertile = TileSurface {
        terrain: Some(Terrain::Grassland),
        resource: Some(Some(Resource::Wheat)),
        irrigation: Some(true),
        river: Some(true),
        ..Default::default()
    };
    for (_, _, x, y) in map.city_radius(island.0, island.1) {
        world.grassland((x, y));
        world.tile_surface((x, y), fertile);
        world.mark_known(1, (x, y));
    }
    for (_, _, x, y) in map.city_radius(home.0, home.1) {
        if (x, y) != home {
            world.grassland((x, y));
            world.tile_surface((x, y), fertile);
        }
    }
    let city = world.founding_city_on(1, home, Terrain::Forest);
    world.city_population(city, 3, None);
    world.city_food(city, 20);
    // Normal AI purchases may fund the expedition; no shields or worklist are preset.
    world.treasury(1, 5000);
    world.reconcile_workers();
    world.finish();
    let worked = game.city_by_id(city).unwrap().worked_tiles.clone();
    let mut world = WorldBuilder::new(&mut game);
    for (_, _, x, y) in map.city_radius(home.0, home.1) {
        if (x, y) == home || worked.contains(&(x, y)) {
            continue;
        }
        world.tile_surface(
            (x, y),
            TileSurface {
                terrain: Some(Terrain::Sea),
                resource: Some(None),
                irrigation: Some(false),
                river: Some(false),
                ..Default::default()
            },
        );
        world.tile((x, y), TilePreset::new(Terrain::Sea, TerrainBase::Sea));
    }
    world.reconcile_workers();
    world.unit(1, UnitKind::Warrior, home);
    world.finish();
    (
        game,
        Box::new(Voyage {
            home: city,
            island,
            ferry_queued: None,
            ship: None,
            ferry_built: None,
            founder: None,
            settler_sequenced: None,
            embarked: false,
            founder_consumed: None,
            colony: None,
        }),
    )
}

struct Voyage {
    home: u32,
    island: (i32, i32),
    ferry_queued: Option<u32>,
    ship: Option<u32>,
    ferry_built: Option<u32>,
    founder: Option<u32>,
    settler_sequenced: Option<u32>,
    embarked: bool,
    founder_consumed: Option<(i32, i32)>,
    colony: Option<(u32, u32)>,
}

impl ScenarioRun for Voyage {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["lifecycle.changed"]
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let home = game.city_by_id(self.home);
        if turn <= DEADLINE {
            if home.is_some_and(|city| city.production == Production::Unit(UnitKind::Galley)) {
                self.ferry_queued.get_or_insert(turn);
            }
            for event in game
                .trace
                .pending_events()
                .iter()
                .filter(|event| event.kind == "lifecycle.changed")
            {
                let p = &event.payload;
                match p["event"].as_str() {
                    Some("unit_created")
                        if p["unit"]["owner"] == OWNER
                            && p["origin"]["source"] == "built"
                            && p["origin"]["city_id"] == self.home =>
                    {
                        let id = p["unit"]["id"].as_u64().unwrap() as u32;
                        if p["unit"]["kind"] == UnitKind::Galley.name() && self.ship.is_none() {
                            self.ship = Some(id);
                            self.ferry_built = Some(turn);
                            self.ferry_queued.get_or_insert(turn);
                        } else if p["unit"]["kind"] == UnitKind::Settler.name()
                            && self.ship.is_some()
                            && self.founder.is_none()
                        {
                            self.founder = Some(id);
                            self.settler_sequenced.get_or_insert(turn);
                        }
                    }
                    Some("unit_removed")
                        if p["unit"]["id"].as_u64() == self.founder.map(u64::from)
                            && self.founder.is_some()
                            && p["reason"] == "founded_city" =>
                    {
                        self.founder_consumed = Some((
                            p["unit"]["x"].as_i64().unwrap() as i32,
                            p["unit"]["y"].as_i64().unwrap() as i32,
                        ));
                    }
                    Some("city_created")
                        if p["city"]["owner"] == OWNER
                            && self.founder_consumed.is_some()
                            && self.embarked =>
                    {
                        let x = p["city"]["x"].as_i64().unwrap() as i32;
                        let y = p["city"]["y"].as_i64().unwrap() as i32;
                        if self.founder_consumed == Some((x, y))
                            && game
                                .map_wrap
                                .civ3_distance(x, y, self.island.0, self.island.1)
                                <= 2
                        {
                            self.colony
                                .get_or_insert((p["city"]["id"].as_u64().unwrap() as u32, turn));
                        }
                    }
                    _ => {}
                }
            }
            if self.ship.is_some()
                && home.is_some_and(|city| city.production == Production::Unit(UnitKind::Settler))
            {
                self.settler_sequenced.get_or_insert(turn);
            }
            if let Some(founder) = self.founder.and_then(|id| game.unit_by_id(id)) {
                self.embarked |= self.ship.is_some()
                    && founder.transport == self.ship
                    && game.tile(founder.x, founder.y).terrain.is_water();
            }
        }
        json!({
            "home_id": self.home,
            "home_production": home.map(|city| city.production.name()),
            "population": home.map(|city| city.size),
            "shields": home.map(|city| city.shields),
            "ferry_queued_turn": self.ferry_queued,
            "ferry_built_turn": self.ferry_built,
            "ship_id": self.ship,
            "founder_id": self.founder,
            "settler_sequenced_turn": self.settler_sequenced,
            "embarked": self.embarked,
            "island_founded_turn": self.colony.map(|(_, turn)| turn),
            "units": game.units().iter().filter(|unit| unit.owner == OWNER).map(|unit|
                json!({"id":unit.id,"kind":unit.kind.name(),"position":[unit.x,unit.y],"transport":unit.transport})).collect::<Vec<_>>(),
            "dropped_events": game.trace.dropped(),
        })
    }

    fn finish(self: Box<Self>, completed_turns: u32) -> Vec<Check> {
        let milestone = |name, turn: Option<u32>| Check {
            name,
            passed: turn.is_some(),
            detail: turn.map_or_else(
                || {
                    format!(
                        "Not observed in {} of {DEADLINE} allowed normal turns",
                        completed_turns.min(DEADLINE)
                    )
                },
                |turn| format!("Observed on completed turn {turn}"),
            ),
        };
        vec![
            milestone("ferry_queued", self.ferry_queued),
            milestone("settler_sequenced", self.settler_sequenced),
            milestone("island_founded", self.colony.map(|(_, turn)| turn)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::Collector;

    #[test]
    fn normal_turns_complete_the_voyage_across_seeds_and_repeat_exactly() {
        let mut reference = None;
        // Include both historically failing seeds and repeat the reference seed.
        for seed in [1, 18, 42, 42] {
            let (mut game, mut runtime) = SCENARIO.start(crate::mapgen::generate(seed));
            assert!(!runtime.has_actions());
            game.trace = Collector::enabled(
                runtime
                    .trace_kinds()
                    .iter()
                    .map(|kind| (*kind).to_owned())
                    .collect(),
                10000,
            );
            let mut observations = Vec::new();
            for turn in 1..=DEADLINE {
                assert!(
                    game.game_over().is_none(),
                    "seed {seed} ended before the deadline"
                );
                game.simulate_turn();
                observations.push(runtime.observe_turn(&game, turn));
                game.trace.drain();
            }
            let checks = runtime.finish(DEADLINE);
            assert_eq!(checks.len(), 3);
            for check in checks {
                assert!(
                    check.passed,
                    "seed {seed}: {}: {}. {:?}",
                    check.name,
                    check.detail,
                    observations.last()
                );
            }
            assert!(observations.last().unwrap()["embarked"].as_bool().unwrap());
            if seed == 42 {
                if let Some(first) = &reference {
                    assert_eq!(&observations, first);
                }
                reference = Some(observations);
            }
        }
    }
}
