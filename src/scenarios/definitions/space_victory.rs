//! A late-game race that begins before the contender has any spaceship parts.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Government, HUMAN, Pact, Simulation, UnitKind,
        fixtures::{CivPreset, TilePreset, TileSurface, WorldBuilder},
    },
    world::{GeneratedMap, Resource, Terrain, TerrainBase},
};
use serde_json::{Value, json};

const CONTENDER: usize = 1;
const DEADLINE: u32 = 110;
const MISSING: [Tech; 4] = [
    Tech::COMPUTERS,
    Tech::SPACE_FLIGHT,
    Tech::ECOLOGY,
    Tech::SUPERCONDUCTOR,
];

pub(super) const SCENARIO: Scenario = Scenario::new(
    "space-victory",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup,
    },
);

pub(crate) fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let known: Vec<_> = Tech::all().filter(|tech| tech.is_civ3_advance()).collect();
    let contender_known: Vec<_> = known
        .iter()
        .copied()
        .filter(|tech| !MISSING.contains(tech))
        .collect();
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: Some(&known),
            },
            CivPreset {
                id: "greece",
                technologies: Some(&contender_known),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(350, 1800);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::Plains),
        resource: Some(Some(Resource::Cattle)),
        road: Some(true),
        river: Some(true),
        irrigation: Some(true),
        ..Default::default()
    });
    for owner in [HUMAN, CONTENDER] {
        world.government_state(owner, Government::Democracy);
        world.economy(owner, 2000, [50, 30, 20]);
    }
    world.completed_great_wonders();

    let capital_location = (12, 16);
    let frontier_location = (22, 16);
    let enemy_border = (28, 16);
    let mut capital = 0;
    let mut frontier = 0;
    for (owner, position) in [
        (CONTENDER, capital_location),
        (CONTENDER, frontier_location),
        (CONTENDER, (5, 8)),
        (CONTENDER, (12, 8)),
        (CONTENDER, (19, 8)),
        (CONTENDER, (5, 16)),
        (CONTENDER, (5, 24)),
        (CONTENDER, (12, 24)),
        (CONTENDER, (19, 24)),
        (CONTENDER, (12, 32)),
        (HUMAN, (28, 24)),
        (HUMAN, enemy_border),
    ] {
        let city = world.city(owner, position, 12);
        let mut buildings = vec![
            Building::AQUEDUCT,
            Building::FACTORY,
            Building::HYDRO_PLANT,
            Building::MFG_PLANT,
            Building::LIBRARY,
            Building::UNIVERSITY,
            Building::MARKETPLACE,
            Building::BANK,
            Building::CATHEDRAL,
            Building::AMPHITHEATER,
            Building::MASS_TRANSIT,
        ];
        if position == capital_location || position == (28, 24) {
            buildings.push(Building::PALACE);
        }
        if owner != CONTENDER {
            buildings.push(Building::RESEARCH_LAB);
        }
        world.city_buildings(city, &buildings);
        world.city_workers(city, 12);
        world.unit(owner, UnitKind::MechInfantry, position);
        if position == capital_location {
            capital = city;
        }
        if position == frontier_location {
            frontier = city;
            for _ in 0..3 {
                world.unit(owner, UnitKind::MechInfantry, position);
            }
        }
        if position == enemy_border {
            for _ in 0..3 {
                world.unit(owner, UnitKind::Tank, (26, 16));
            }
        }
        // Rail corridors support developed cities using ordinary tile outputs.
        for x in position.0 - 2..=position.0 + 2 {
            for y in position.1 - 3..=position.1 + 3 {
                let mut tile = TilePreset::new(Terrain::Plains, TerrainBase::Plains);
                tile.resource = Some(Resource::Cattle);
                tile.railroad = true;
                tile.river = true;
                world.tile((x, y), tile);
            }
        }
    }
    world.diplomacy(CONTENDER, HUMAN, Pact::War, 1, [-10_000, -10_000]);
    world.mark_known(HUMAN, frontier_location);
    world.mark_known(CONTENDER, enemy_border);
    world.research_project(CONTENDER, Tech::UNSET, 0);
    world.finish();
    (
        game,
        Box::new(SpaceVictory {
            capital,
            frontier,
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct SpaceVictory {
    capital: u32,
    frontier: u32,
    planned: Option<u32>,
    first_part: Option<u32>,
    completed: Option<u32>,
    launch: Option<u32>,
    pressured_launch: bool,
    held_frontier: bool,
}

impl ScenarioRun for SpaceVictory {
    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let (built, arrival) = game.spaceship_progress(CONTENDER);
        let (rival, rival_arrival) = game.spaceship_progress(HUMAN);
        let strategy = game.strategic_priorities(CONTENDER);
        let research = game.players()[CONTENDER].researching;
        let capital_held = game
            .city_by_id(self.capital)
            .is_some_and(|city| city.owner == CONTENDER);
        let frontier = game
            .city_by_id(self.frontier)
            .filter(|city| city.owner == CONTENDER);
        let guarded = frontier.is_some_and(|city| {
            game.units().iter().any(|unit| {
                unit.owner == CONTENDER
                    && unit.kind.is_military()
                    && (unit.x, unit.y) == (city.x, city.y)
            })
        });
        if built == [0; 3]
            && strategy.space_program
            && (MISSING.contains(&research)
                || MISSING
                    .into_iter()
                    .any(|advance| game.has_tech(CONTENDER, advance)))
        {
            self.planned.get_or_insert(turn);
        }
        if built != [0; 3] {
            self.first_part.get_or_insert(turn);
        }
        if built == [32, 16, 12] {
            self.completed.get_or_insert(turn);
        }
        if arrival.is_some() && self.launch.is_none() {
            self.launch = Some(turn);
            self.pressured_launch = game.at_war(CONTENDER, HUMAN);
            self.held_frontier = capital_held && guarded;
        }
        json!({
            "research": research.name(),
            "plan": strategy.space_research_goal.map(Tech::name),
            "apollo": game.cities().iter().find(|city| city.has_building(Building::APOLLO_PROGRAM)).map(|city| city.owner),
            "built": built,
            "rival_built": rival,
            "arrival": arrival,
            "rival_arrival": rival_arrival,
            "war": game.at_war(CONTENDER, HUMAN),
            "frontier_guarded": guarded,
            "capital_held": capital_held,
            "space_program": strategy.space_program,
            "industrial_shields": game.cities_owned_by(CONTENDER)
                .filter(|city| city.has_building(Building::FACTORY))
                .map(|city| (city.id, game.city_forecast(city).shield_surplus()))
                .collect::<Vec<_>>(),
        })
    }

    fn finish(self: Box<Self>, _: u32) -> Vec<Check> {
        vec![
            Check {
                name: "space_research_precedes_first_part",
                passed: self
                    .planned
                    .zip(self.first_part)
                    .is_some_and(|(plan, first)| plan < first),
                detail: format!(
                    "Research commitment on {:?}, first part on {:?}",
                    self.planned, self.first_part
                ),
            },
            Check {
                name: "space_builds_full_ship_and_launches",
                passed: self
                    .completed
                    .zip(self.launch)
                    .is_some_and(|(complete, launch)| complete < launch && launch <= DEADLINE),
                detail: format!(
                    "Full assembly on {:?}, launch on {:?} within {DEADLINE} turns",
                    self.completed, self.launch
                ),
            },
            Check {
                name: "space_launches_during_war_with_defended_frontier",
                passed: self.pressured_launch && self.held_frontier,
                detail: format!(
                    "At launch: war {}, guarded frontier and capital {}",
                    self.pressured_launch, self.held_frontier
                ),
            },
        ]
    }
}
