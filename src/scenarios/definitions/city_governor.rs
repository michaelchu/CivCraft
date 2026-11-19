//! Paired campaigns that exercise citizen assignments with a human seat.
use crate::scenarios::{Control, RuntimeWorld, ScenarioKind};

use crate::sim::fixtures::{CivPreset, WorldBuilder};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

use super::{Check, Scenario, ScenarioRun};
use crate::world::{GeneratedMap, TerrainBase};
use crate::{sim::*, world::Terrain};

pub(super) const GOVERNED: Scenario = Scenario::new(
    "city-governor-campaign",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |map| setup(map, true),
    },
);
pub(super) const CONTROL: Scenario = Scenario::new(
    "city-governor-control",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |map| setup(map, false),
    },
);

pub(super) const FOOD_SHOCK: Scenario = Scenario::new(
    "city-governor-food-shock",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |map| food_shock(map, true),
    },
);
pub(super) const FOOD_SHOCK_CONTROL: Scenario = Scenario::new(
    "city-governor-food-shock-control",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |map| food_shock(map, false),
    },
);

pub(super) const UNREST_SHOCK: Scenario = Scenario::new(
    "city-governor-unrest-shock",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |map| unrest_shock(map, true),
    },
);
pub(super) const UNREST_SHOCK_CONTROL: Scenario = Scenario::new(
    "city-governor-unrest-shock-control",
    2,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |map| unrest_shock(map, false),
    },
);

fn unrest_shock(map: GeneratedMap, governed: bool) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: None,
            },
            CivPreset {
                id: "germany",
                technologies: None,
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    for (x, y) in [(8, 8), (18, 8), (25, 22)] {
        for dy in -3..=3 {
            for dx in -3..=3 {
                world.grassland((x + dx, y + dy));
            }
        }
    }
    let city = world.city(HUMAN, (8, 8), 5);
    world.city(HUMAN, (18, 8), 1);
    world.city(1, (25, 22), 1);
    world.finish();
    WorldBuilder::new(&mut game).city_workers(city, 4);
    audit(game, governed, None, Some(city))
}

fn food_shock(map: GeneratedMap, governed: bool) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: None,
            },
            CivPreset {
                id: "germany",
                technologies: None,
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    for (x, y) in [(8, 8), (18, 8), (25, 22)] {
        for dy in -3..=3 {
            for dx in -3..=3 {
                world.grassland((x + dx, y + dy));
            }
        }
    }
    let city = world.city(HUMAN, (8, 8), 4);
    let neighbor = world.city(HUMAN, (18, 8), 1);
    world.city(1, (25, 22), 1);
    world.finish();
    for id in [city, neighbor] {
        game.apply(Command::SetCityGovernor {
            city_id: id,
            state: CityGovernor {
                active: true,
                settings: GovernorSettings::PRESETS[0].1,
            },
        });
    }
    // Strip food from the tiles the capital was working, while keeping
    // the tiles legally workable and leaving replacement grassland.
    let worked = game.city_by_id(city).unwrap().worked_tiles.clone();
    let mut world = WorldBuilder::new(&mut game);
    for position in worked {
        world.tile(
            position,
            crate::sim::fixtures::TilePreset::new(Terrain::Desert, TerrainBase::Desert),
        );
    }
    world.finish();
    audit(game, governed, Some(city), None)
}

fn setup(map: GeneratedMap, governed: bool) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = Simulation::new(map);
    let capital = game
        .units_owned_by(HUMAN)
        .find(|unit| unit.kind == UnitKind::Settler)
        .map(|unit| (unit.id, unit.x, unit.y));
    if let Some((id, x, y)) = capital
        && game.apply(Command::FoundCity(id))
    {
        // Prefer a legal nearby site to exercise overlapping work radii.
        let mut sites = Vec::new();
        for sy in 0..game.map_wrap.height() {
            for sx in 0..game.map_wrap.width() {
                let distance = game.map_distance(x, y, sx, sy);
                if (3..=7).contains(&distance)
                    && game.can_found_at(sx, sy)
                    && game.tile(sx, sy).owner.is_none_or(|owner| owner == HUMAN)
                    && game.units().iter().all(|unit| (unit.x, unit.y) != (sx, sy))
                {
                    sites.push((distance, sy, sx));
                }
            }
        }
        sites.sort_unstable();
        if let Some(&(_, sy, sx)) = sites.first() {
            let settler = WorldBuilder::new(&mut game).unit(HUMAN, UnitKind::Settler, (sx, sy));
            game.apply(Command::FoundCity(settler));
        }
    }

    audit(game, governed, None, None)
}

fn audit(
    mut game: Simulation,
    governed: bool,
    food_shock: Option<u32>,
    unrest_shock: Option<u32>,
) -> (Simulation, Box<dyn ScenarioRun>) {
    if unrest_shock.is_some() {
        WorldBuilder::new(&mut game).governors_active(false);
    }
    let initial: Vec<_> = game
        .cities_owned_by(HUMAN)
        .map(|city| CitySample::capture(&game, city))
        .collect();
    if let Some(shock_city) = unrest_shock.filter(|_| governed) {
        WorldBuilder::new(&mut game).founding_governor(shock_city);
    } else if !governed {
        // Start both arms with identical citizen assignments. Disable the
        // control arm in one step so release order cannot change them.
        WorldBuilder::new(&mut game).governors_active(false);
    }
    (
        game,
        Box::new(CampaignAudit {
            governed,
            food_shock,
            unrest_shock,
            first_shock_food: None,
            initial,
            totals: Totals::default(),
            last: BTreeSet::new(),
            human_seat: true,
            complete_trace: true,
        }),
    )
}

#[derive(Serialize)]
struct CitySample {
    id: u32,
    population: i32,
    governor_active: bool,
    worked_tiles: Vec<(i32, i32)>,
    specialists: [i32; 3],
    forecast: [i32; 6],
    disorder_after_city_processing: bool,
    projected_disorder: bool,
}

impl CitySample {
    fn capture(game: &Simulation, city: &City) -> Self {
        Self {
            id: city.id,
            population: city.size,
            governor_active: game.governor_controls(city.id),
            worked_tiles: city.worked_tiles.clone(),
            specialists: city.specialists,
            forecast: game.city_manager_surplus(city, &game.city_forecast(city)),
            disorder_after_city_processing: game.city_forecast(city).disorder,
            projected_disorder: game.city_forecast(city).disorder,
        }
    }
}

#[derive(Default, Serialize)]
struct Totals {
    processed_city_turns: u32,
    food_deficit_city_turns: u32,
    disorder_city_turns: u32,
    starvation_events: u32,
    cities_destroyed_by_famine: u32,
    governor_releases: u32,
    active_governor_city_observations: u32,
}

struct CampaignAudit {
    governed: bool,
    food_shock: Option<u32>,
    unrest_shock: Option<u32>,
    first_shock_food: Option<i64>,
    initial: Vec<CitySample>,
    totals: Totals,
    last: BTreeSet<u32>,
    human_seat: bool,
    complete_trace: bool,
}

impl ScenarioRun for CampaignAudit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["cities.growth_applied", "cities.governor_released"]
    }

    fn observe_turn(&mut self, game: &Simulation, completed_turns: u32) -> Value {
        self.human_seat &= !game.players()[HUMAN].ai;
        self.complete_trace &= game.trace.dropped() == 0;
        let cities: Vec<_> = game
            .cities_owned_by(HUMAN)
            .map(|city| CitySample::capture(game, city))
            .collect();
        self.last = cities.iter().map(|city| city.id).collect();
        let mut releases = Vec::new();
        let mut growth = Vec::new();
        for event in game
            .trace
            .pending_events()
            .iter()
            .filter(|event| event.entities.contains(&trace::Entity::Civilization(HUMAN)))
        {
            let id = event.entities.iter().find_map(|entity| match entity {
                trace::Entity::City(id) => Some(*id),
                _ => None,
            });
            match event.kind.as_str() {
                "cities.governor_released" => {
                    self.totals.governor_releases += 1;
                    releases.push(id);
                }
                "cities.growth_applied" => {
                    self.totals.processed_city_turns += 1;
                    if completed_turns == 1 && id.is_some() && id == self.food_shock {
                        self.first_shock_food = event.payload["applied_surplus"].as_i64();
                    }
                    self.totals.food_deficit_city_turns += u32::from(
                        event.payload["applied_surplus"]
                            .as_i64()
                            .is_some_and(|food| food < 0),
                    );
                    match event.payload["outcome"].as_str() {
                        Some("starved") => self.totals.starvation_events += 1,
                        Some("destroyed") => self.totals.cities_destroyed_by_famine += 1,
                        _ => {}
                    }
                    growth.push(json!({"city": id, "result": event.payload}));
                }
                _ => {}
            }
        }
        self.totals.disorder_city_turns += cities
            .iter()
            .filter(|city| city.disorder_after_city_processing)
            .count() as u32;
        self.totals.active_governor_city_observations +=
            cities.iter().filter(|city| city.governor_active).count() as u32;
        json!({
            "governed": self.governed,
            "food_shock_city": self.food_shock,
            "unrest_shock_city": self.unrest_shock,
            "initial": (completed_turns == 1).then_some(&self.initial),
            "cities": cities,
            "growth": growth,
            "governor_releases": releases,
            "totals": self.totals,
        })
    }

    fn finish(self: Box<Self>, completed_turns: u32) -> Vec<Check> {
        let mut checks = vec![
            Check {
                name: "human_governor_campaign_observed",
                passed: self.human_seat
                    && self.complete_trace
                    && self.initial.len() >= 2
                    && self.totals.processed_city_turns > 0,
                detail: format!(
                    "{} turns, {} starting cities, {} processed city turns, human seat: {}, complete trace: {}.",
                    completed_turns,
                    self.initial.len(),
                    self.totals.processed_city_turns,
                    self.human_seat,
                    self.complete_trace
                ),
            },
            Check {
                name: "human_governor_campaign_cities_retained",
                passed: self.initial.iter().all(|city| self.last.contains(&city.id)),
                detail: format!("{} human cities remain.", self.last.len()),
            },
        ];
        if let Some(shock_city) = self.food_shock {
            let starting_food = self
                .initial
                .iter()
                .find(|city| city.id == shock_city)
                .map(|city| city.forecast[0]);
            checks.push(Check {
                name: "human_governor_food_shock_response",
                passed: starting_food.is_some_and(|food| food < 0)
                    && self.first_shock_food.is_some_and(|food| (food >= 0) == self.governed),
                detail: format!("City {shock_city}: food surplus before processing {starting_food:?}, first applied surplus {:?}.",
                    self.first_shock_food),
            });
        }
        if let Some(shock_city) = self.unrest_shock {
            let starting_disorder = self
                .initial
                .iter()
                .find(|city| city.id == shock_city)
                .is_some_and(|city| city.projected_disorder);
            let disorder_turns = self.totals.disorder_city_turns;
            checks.push(Check {
                name: "human_governor_unrest_response",
                passed: starting_disorder && (disorder_turns == 0) == self.governed,
                detail: format!("City {shock_city} starts in projected disorder: {starting_disorder}; city disorder turns after processing: {disorder_turns}."),
            });
        }
        if self.governed && self.unrest_shock.is_none() {
            let totals = &self.totals;
            checks.push(Check {
                name: "human_governor_campaign_no_intervention_needed",
                passed: self.initial.iter().all(|city| city.governor_active)
                    && totals.governor_releases == 0 && totals.food_deficit_city_turns == 0
                    && totals.disorder_city_turns == 0 && totals.starvation_events == 0
                    && totals.cities_destroyed_by_famine == 0,
                detail: format!("{} initially active, {} releases, {} food deficit turns, {} disorder turns, {} starvation events, {} cities lost to famine.",
                    self.initial.iter().filter(|city| city.governor_active).count(),
                    totals.governor_releases, totals.food_deficit_city_turns, totals.disorder_city_turns,
                    totals.starvation_events, totals.cities_destroyed_by_famine),
            });
        }
        checks
    }
}
