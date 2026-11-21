//! A late-game race that begins before the contender has any spaceship parts.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        Government, HUMAN, Pact, Simulation, UnitKind,
        drive::Drive,
        fixtures::{CivPreset, TilePreset, TileSurface, WorldBuilder},
        lifecycle::CityRemoval,
        trace::Entity,
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

pub(super) const INDUSTRIAL_LOSS: Scenario = Scenario::new(
    "space-industrial-loss",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: setup_industrial_loss,
    },
);

pub(crate) fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (game, audit) = campaign(map);
    (game, Box::new(audit))
}

fn campaign(map: GeneratedMap) -> (Simulation, SpaceVictory) {
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
        world.government_state(owner, Government::DEMOCRACY);
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
        SpaceVictory {
            capital,
            frontier,
            ..Default::default()
        },
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

const PARTS: [Building; 3] = [
    Building::SPACE_STRUCTURAL,
    Building::SPACE_COMPONENT,
    Building::SPACE_MODULE,
];
const RECOVERY_DEADLINE: u32 = 100;

fn setup_industrial_loss(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (game, baseline) = campaign(map);
    (
        game,
        Box::new(IndustrialLoss {
            frontier: baseline.frontier,
            ..Default::default()
        }),
    )
}

#[derive(Default)]
struct IndustrialLoss {
    frontier: u32,
    loss: Option<(u32, u32, Building)>,
    parts_before: [usize; 3],
    original_part_sites: Vec<u32>,
    completed_part_sites: Vec<u32>,
    replacement: Option<(u32, u32)>,
    launch: Option<u32>,
    defended_at_launch: bool,
}

impl ScenarioRun for IndustrialLoss {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["production.completed"]
    }

    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        let (built, arrival) = game.spaceship_progress(CONTENDER);
        if self.loss.is_some() || built.iter().sum::<usize>() < 8 || arrival.is_some() {
            return;
        }
        let site = game
            .cities_owned_by(CONTENDER)
            .filter(|city| city.id != self.frontier && !city.has_building(Building::PALACE))
            .filter_map(|city| match city.production {
                crate::sim::Production::Building(part) if PARTS.contains(&part) => {
                    Some((game.city_forecast(city).shield_surplus(), city.id, part))
                }
                _ => None,
            })
            .max_by_key(|&(shields, city, _)| (shields, std::cmp::Reverse(city)));
        if let Some((_, city, part)) = site {
            self.loss = Some((completed + 1, city, part));
            self.parts_before = built;
            self.original_part_sites = game
                .cities_owned_by(CONTENDER)
                .filter(|city| city.production == crate::sim::Production::Building(part))
                .map(|city| city.id)
                .collect();
            game.remove_city_because(city, CityRemoval::Disaster, None);
        }
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let (built, arrival) = game.spaceship_progress(CONTENDER);
        let war = game.at_war(CONTENDER, HUMAN);
        let frontier_guarded = game.city_by_id(self.frontier).is_some_and(|city| {
            city.owner == CONTENDER
                && game.units().iter().any(|unit| {
                    unit.owner == CONTENDER
                        && unit.kind.is_military()
                        && (unit.x, unit.y) == (city.x, city.y)
                })
        });
        if let Some((_, _, part)) = self.loss {
            for event in game.trace.pending_events().iter().filter(|event| {
                event.kind == "production.completed"
                    && event.entities.contains(&Entity::Civilization(CONTENDER))
                    && event.payload["item"] == part.data().name
            }) {
                if let Some(city) = event.entities.iter().find_map(|entity| match entity {
                    Entity::City(id) => Some(*id),
                    _ => None,
                }) {
                    // An existing site can take on more work after completing
                    // its first in-flight copy.
                    if !self.original_part_sites.contains(&city)
                        || self.completed_part_sites.contains(&city)
                    {
                        self.replacement.get_or_insert((turn, city));
                    }
                    self.completed_part_sites.push(city);
                }
            }
            if arrival.is_some() && self.launch.is_none() {
                self.launch = Some(turn);
                self.defended_at_launch = war && frontier_guarded;
            }
        }
        json!({
            "industrial_loss": self.loss.map(|(turn, city, part)| json!({"turn": turn, "city": city, "part": part.data().name})),
            "built": built, "arrival": arrival, "active_war": war, "frontier_guarded": frontier_guarded,
            "replacement_completion": self.replacement,
            "part_sites": game.cities_owned_by(CONTENDER).filter_map(|city| match city.production {
                crate::sim::Production::Building(part) if PARTS.contains(&part) => Some((city.id, part.data().name)),
                _ => None,
            }).collect::<Vec<_>>(),
        })
    }

    fn finish(self: Box<Self>, _: u32) -> Vec<Check> {
        vec![
            Check {
                name: "space_loses_active_part_city",
                passed: self.loss.is_some(),
                detail: format!(
                    "Lost {:?} with parts already built {:?}",
                    self.loss
                        .map(|(turn, city, part)| (turn, city, part.data().name)),
                    self.parts_before
                ),
            },
            Check {
                name: "space_replaces_lost_production",
                passed: self.replacement.is_some(),
                detail: format!(
                    "New or repeat completion of the interrupted part type (turn, city) {:?}",
                    self.replacement
                ),
            },
            Check {
                name: "space_launches_after_industrial_loss",
                passed: self.launch.is_some_and(|turn| turn <= RECOVERY_DEADLINE)
                    && self.defended_at_launch,
                detail: format!(
                    "Launch {:?} by turn {RECOVERY_DEADLINE}, defended frontier during war {}",
                    self.launch, self.defended_at_launch
                ),
            },
        ]
    }
}
