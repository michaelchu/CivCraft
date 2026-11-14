//! AI advisor scoring used to choose city improvements, research, defense, and spending.

mod advisor_buildings;
mod advisor_defenders;
mod advisor_gold;
mod advisor_production;
mod advisor_research;
mod advisor_threats;
#[cfg(test)]
use advisor_buildings::DefenseEffectContext;

use super::unit_catalog::{UnitClass, UnitFlag};
use super::*;

mod playtest;
use crate::rules::{Genus, Requirement};

const FOOD_WEIGHT: i32 = 30;
const SHIELD_WEIGHT: i32 = 17;
const TRADE_WEIGHT: i32 = 18;
const ADVISOR_POWER_DIVIDER: i32 = POWER_FACTOR * 3;

#[derive(Clone, Copy, Debug)]
pub(super) struct AiBuildChoice {
    pub(super) production: Production,
    pub(super) want: f64,
}

const DOMESTIC_WANT_MAX: f64 = 750.075;
pub(super) const MILITARY_EMERGENCY_WANT: f64 = 1000.1;

#[derive(Clone, Copy)]
pub(super) struct GuardCityAssessment {
    pub(super) danger: i32,
    pub(super) urgency: i32,
    pub(super) grave: i32,
    pub(super) defense: i32,
    pub(super) defense_igwall: i32,
}

impl Simulation {
    fn wonder_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        if self.ai_wonder_city(city.owner) != Some(city.id) {
            return None;
        }
        let candidates: Vec<_> = Building::all()
            .filter(|building| {
                matches!(
                    building.data().genus,
                    Genus::GreatWonder | Genus::SmallWonder
                ) && self.building_blocker(city, *building).is_none()
                    && !(building.data().genus == Genus::SmallWonder
                        && self.cities.iter().any(|other| {
                            other.owner == city.owner && other.has_building(*building)
                        }))
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let mut scratch = self.clone();
        candidates
            .into_iter()
            .filter_map(|building| {
                // Freeciv applies Builder to building_advisor_choose's
                // improvement choice, not to the separate wonder helper.
                let want = self.projected_wonder_want(&mut scratch, city, building);
                (want > 0.0).then_some(AiBuildChoice {
                    production: Production::Building(building),
                    want: want.min(DOMESTIC_WANT_MAX),
                })
            })
            .max_by(|a, b| a.want.total_cmp(&b.want))
    }

    fn ai_wonder_city(&self, owner: usize) -> Option<u32> {
        let owned: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .collect();
        // advbuilding.c keeps a productive, legal wonder already under way.
        if let Some(city) = owned.iter().find(|city| {
            self.city_forecast(city).shield_surplus() > 0
                && matches!(city.production, Production::Building(building)
                    if matches!(building.data().genus, Genus::GreatWonder | Genus::SmallWonder)
                        && self.building_blocker(city, building).is_none())
        }) {
            return Some(city.id);
        }
        if owned.is_empty() {
            return None;
        }
        let mut best = None;
        let mut best_value = 0;
        for &city in &owned {
            if self
                .city_threat_arrivals(city)
                .any(|(_, _, turns)| turns <= 1)
            {
                continue;
            }
            let mut value = self.city_forecast(city).shield_surplus();
            if self
                .map_wrap
                .civ3_neighbors(city.x, city.y)
                .any(|(x, y)| self.tile(x, y).terrain.is_water())
            {
                value /= 2;
            }
            if value > best_value {
                best = Some(city.id);
                best_value = value;
            }
        }
        best
    }

    fn domestic_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        let wonder_city = self.ai_wonder_city(city.owner) == Some(city.id);
        let forecast = self.city_forecast(city);
        let base_output = self.advisor_city_output_from_forecast(city, &forecast);
        Building::all()
            .filter(|building| {
                building.data().genus == Genus::Improvement
                    && self.building_blocker(city, *building).is_none()
            })
            .filter_map(|building| {
                let want = (self.domestic_improvement_want_with_forecast(
                    city,
                    building,
                    &forecast,
                    base_output,
                ) as f32
                    / if wonder_city { 5.0 } else { 1.0 }
                    * crate::sim::ai_traits::builder_building_want_factor())
                    as f64;
                (want > 0.0).then_some(AiBuildChoice {
                    production: Production::Building(building),
                    want: want.min(DOMESTIC_WANT_MAX),
                })
            })
            .max_by(|a, b| a.want.total_cmp(&b.want))
    }

    fn space_production_want(&self, city: &City, building: Building) -> f64 {
        // advdata.c chooses the first player on tied manufacturing/tech scores.
        let leader = |score: &dyn Fn(usize) -> i32| {
            (0..self.players.len())
                .fold(None, |best: Option<(usize, i32)>, owner| {
                    let value = score(owner);
                    if best.is_none_or(|(_, old)| value > old) {
                        Some((owner, value))
                    } else {
                        best
                    }
                })
                .map(|(owner, _)| owner)
        };
        let manufacturing = leader(&|owner| {
            self.cities
                .iter()
                .filter(|candidate| candidate.owner == owner)
                .map(|candidate| self.city_forecast(candidate).shield_surplus())
                .sum()
        });
        let technology = leader(&|owner| {
            Tech::all()
                .filter(|&tech| self.has_tech(owner, tech))
                .count() as i32
                + self.players[owner].future_techs * 5 / 2
        });
        let competitive = manufacturing == Some(city.owner) || technology == Some(city.owner);
        if building == Building::APOLLO_PROGRAM {
            // daieffects.c EFT_ENABLE_SPACE; unlike parts Apollo gets base 10.
            return if competitive { 160.0 } else { 10.0 };
        }
        let race_started = self.spaceships.iter().enumerate().any(|(owner, ship)| {
            owner != BARBARIANS && self.player_alive(owner) && ship.built != [0; 3]
        });
        if !competitive && !race_started {
            return 0.0;
        }
        let base = if self.ai_wonder_city(city.owner) == Some(city.id) {
            120.0
        } else {
            210.0
        };
        let started = self.spaceships[city.owner].built != [0; 3];
        // daieffects.c lowers space priority in the wonder city, and
        // daicity.c then divides every non-wonder building want by five.
        let want = base * if started { 3.0 } else { 1.0 };
        want / if base == 120.0 { 5.0 } else { 1.0 }
    }

    pub(super) fn offensive_production(&self, city: &City) -> Option<UnitKind> {
        self.offensive_build_choice(city)
            .map(|choice| match choice.production {
                Production::Unit(kind) => kind,
                Production::Building(_) => unreachable!(),
            })
    }

    fn offensive_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        self.offensive_advice(city).0
    }
}

#[cfg(test)]
#[path = "advisors/tests.rs"]
mod tests;
