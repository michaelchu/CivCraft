//! Typed, read-only providers. Snapshot construction never advances gameplay.
use super::*;
use serde::Serialize;

/// Providers return typed values; analyzers never parse JSON to inspect state.
pub(crate) trait DomainObservation: std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;
    fn json(&self) -> serde_json::Value;
}
impl<T: Serialize + 'static> DomainObservation for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("snapshot must serialize")
    }
}

pub(crate) struct Snapshot {
    pub turn: i32,
    pub phase: trace::Phase,
    pub city_count: usize,
    sections: std::collections::BTreeMap<&'static str, Box<dyn DomainObservation>>,
}
impl Snapshot {
    pub fn new(turn: i32, phase: trace::Phase, city_count: usize) -> Self {
        Self {
            turn,
            phase,
            city_count,
            sections: Default::default(),
        }
    }
    pub fn insert<T: Serialize + 'static>(&mut self, key: &'static str, value: T) {
        assert!(
            !["turn", "phase", "city_count"].contains(&key),
            "reserved snapshot section"
        );
        assert!(
            !self.sections.contains_key(key),
            "duplicate snapshot provider"
        );
        self.sections.insert(key, Box::new(value));
    }
    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.sections.get(key)?.as_ref().as_any().downcast_ref()
    }
    pub fn cities(&self) -> Option<&Vec<CitySnapshot>> {
        self.get("cities")
    }
    pub fn civilizations(&self) -> Option<&Vec<CivilizationSnapshot>> {
        self.get("civilizations")
    }
    pub fn units(&self) -> Option<&Vec<UnitSnapshot>> {
        self.get("units")
    }
}
impl Serialize for Snapshot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.sections.len() + 3))?;
        map.serialize_entry("turn", &self.turn)?;
        map.serialize_entry("phase", &self.phase)?;
        map.serialize_entry("city_count", &self.city_count)?;
        for (key, value) in &self.sections {
            map.serialize_entry(key, &value.as_ref().json())?;
        }
        map.end()
    }
}
#[derive(Clone, Serialize)]
pub(crate) struct Specialists {
    pub scientists: i32,
    pub taxmen: i32,
    pub entertainers: i32,
}
#[derive(Clone, Serialize)]
pub(crate) struct TileYield {
    pub x: i32,
    pub y: i32,
    pub food: i32,
    pub shields: i32,
    pub trade: i32,
}
#[derive(Clone, Serialize)]
pub(crate) struct CitySnapshot {
    pub id: u32,
    pub owner: usize,
    pub name: String,
    pub population: i32,
    pub food: i32,
    pub food_output: i32,
    pub food_surplus: i32,
    pub growth_threshold: i32,
    pub food_to_growth: i32,
    pub projected_growth_turns: Option<i32>,
    pub size_limit: i32,
    pub production: String,
    pub production_population_cost: i32,
    pub support_food: i32,
    pub disorder: bool,
    pub specialists: Specialists,
    pub worked_tiles: Vec<TileYield>,
    pub growth_status: &'static str,
}
#[derive(Clone, Serialize)]
pub(crate) struct CivilizationSnapshot {
    pub id: usize,
    pub name: String,
    pub alive: bool,
    pub cities: usize,
    pub units: usize,
    pub gold: i32,
    pub score: i32,
}
#[derive(Clone, Serialize)]
pub(crate) struct UnitSnapshot {
    pub id: u32,
    pub owner: usize,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub moves_left: i32,
    pub state: String,
    pub can_found: bool,
    pub work_kind: Option<String>,
    pub work_left: i32,
    pub goto: Option<(i32, i32)>,
    pub settlement_target: Option<(i32, i32)>,
}

impl Simulation {
    pub(crate) fn observe_cities(&self) -> Vec<CitySnapshot> {
        self.cities()
            .iter()
            .map(|city| {
                let forecast = self.city_forecast(city);
                let food_surplus = forecast.food_surplus(city.size, forecast.support.food);
                let growth_threshold = Self::growth_box(city.size);
                let food_to_growth = (growth_threshold - city.food).max(0);
                let size_limit = self.city_size_limit(city);
                let population_cost = match city.production {
                    Production::Unit(kind) => kind.stats().pop_cost,
                    _ => 0,
                };
                CitySnapshot {
                    id: city.id,
                    owner: city.owner,
                    name: city.name.clone(),
                    population: city.size,
                    food: city.food,
                    food_output: forecast.food,
                    food_surplus,
                    growth_threshold,
                    food_to_growth,
                    projected_growth_turns: (food_surplus > 0)
                        .then(|| (food_to_growth + food_surplus - 1) / food_surplus),
                    size_limit,
                    production: city.production.name().into(),
                    production_population_cost: population_cost,
                    support_food: forecast.support.food,
                    disorder: forecast.disorder,
                    specialists: Specialists {
                        scientists: city.specialists[0],
                        taxmen: city.specialists[1],
                        entertainers: city.specialists[2],
                    },
                    worked_tiles: city
                        .worked_tiles
                        .iter()
                        .map(|&(x, y)| {
                            let (food, shields, trade) =
                                self.city_tile_yields(city, x, y, false, None);
                            TileYield {
                                x,
                                y,
                                food,
                                shields,
                                trade,
                            }
                        })
                        .collect(),
                    growth_status: if city.size >= size_limit {
                        "growth_limited_by_city_size"
                    } else if population_cost > 0 {
                        "production_consumes_population"
                    } else if food_surplus < 0 {
                        "food_deficit"
                    } else if food_surplus == 0 {
                        "food_balance_zero"
                    } else {
                        "positive_forecast_surplus"
                    },
                }
            })
            .collect()
    }
    pub(crate) fn observe_civilizations(&self) -> Vec<CivilizationSnapshot> {
        (0..self.civilization_count)
            .map(|owner| {
                let player = &self.players()[owner];
                CivilizationSnapshot {
                    id: owner,
                    name: player.name.clone(),
                    alive: self.player_alive(owner),
                    cities: self.cities_owned_by(owner).count(),
                    units: self.units_owned_by(owner).count(),
                    gold: player.gold,
                    score: self.civilization_score(owner),
                }
            })
            .collect()
    }
    pub(crate) fn observe_units(&self) -> Vec<UnitSnapshot> {
        self.units
            .iter()
            .map(|unit| UnitSnapshot {
                id: unit.id,
                owner: unit.owner,
                kind: unit.kind.name().into(),
                x: unit.x,
                y: unit.y,
                hp: unit.hp,
                moves_left: unit.moves_left,
                state: format!("{:?}", unit.state),
                can_found: unit.kind.stats().can_found,
                work_kind: unit.work_kind.map(|work| work.name().into()),
                work_left: unit.work_left,
                goto: self.unit_goto_tiles.get(&unit.id).copied(),
                settlement_target: self
                    .ai_settlement_targets
                    .get(&unit.id)
                    .map(|r| r.destination),
            })
            .collect()
    }
}
