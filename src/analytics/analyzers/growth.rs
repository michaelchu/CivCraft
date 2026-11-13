use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{observation::Snapshot, trace::Entity};
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) struct Growth {
    threshold: u32,
    previous: BTreeMap<u32, (i32, i32)>,
    stalled: BTreeMap<u32, u32>,
}
impl Growth {
    pub fn new(threshold: u32) -> Self {
        Self {
            threshold,
            previous: BTreeMap::new(),
            stalled: BTreeMap::new(),
        }
    }
}
impl Analyzer for Growth {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "growth",
            version: 1,
            events: vec![],
            providers: vec!["cities"],
            configuration: json!({"consecutive_non_accumulating_turns":self.threshold}),
        }
    }
    fn on_start(&mut self, snapshot: &Snapshot, _: &mut AnalysisOutput) {
        self.previous = snapshot
            .cities()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|c| (c.id, (c.population, c.food)))
            .collect();
    }
    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        let Some(cities) = snapshot.cities() else {
            return;
        };
        for city in cities {
            let (population, food) = self
                .previous
                .get(&city.id)
                .copied()
                .unwrap_or((city.population, city.food));
            let count = self.stalled.entry(city.id).or_default();
            if city.food_surplus > 0
                && city.population == population
                && city.production_population_cost == 0
                && city.population < city.size_limit
                && city.food <= food
            {
                *count += 1;
            } else {
                *count = 0;
            }
            out.check(format!("city:{}",city.id), "positive_surplus_food_not_accumulating",
                vec![Entity::City(city.id),Entity::Civilization(city.owner)], *count >= self.threshold,
                "Positive forecast surplus has not increased stored food for the configured number of consecutive observations. This is a candidate, not proof of a growth bug.",
                json!({"city_id":city.id,"population":city.population,"food":city.food,"food_store_change":city.food-food,
                    "food_surplus":city.food_surplus,"growth_threshold":city.growth_threshold,"size_limit":city.size_limit,"non_accumulating_turns":count}));
        }
        self.previous = cities
            .iter()
            .map(|c| (c.id, (c.population, c.food)))
            .collect();
        self.stalled.retain(|id, _| self.previous.contains_key(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{
        observation::{CitySnapshot, Specialists},
        trace::Phase,
    };
    fn observation(turn: i32, food: i32) -> Snapshot {
        let mut snapshot = Snapshot::new(turn, Phase::TurnEnd, 1);
        snapshot.insert(
            "cities",
            vec![CitySnapshot {
                id: 1,
                owner: 0,
                name: "Test".into(),
                population: 2,
                food,
                food_output: 6,
                food_surplus: 2,
                growth_threshold: 20,
                food_to_growth: 20 - food,
                projected_growth_turns: Some(10),
                size_limit: 6,
                production: "Warrior".into(),
                production_population_cost: 0,
                support_food: 0,
                disorder: false,
                specialists: Specialists {
                    scientists: 0,
                    taxmen: 0,
                    entertainers: 0,
                },
                worked_tiles: vec![],
                growth_status: "food_accumulating",
            }],
        );
        snapshot
    }
    #[test]
    fn growth_threshold_and_explicit_recovery_match_previous_check() {
        let mut analyzer = Growth::new(2);
        let mut out = AnalysisOutput::default();
        analyzer.on_start(&observation(1, 0), &mut out);
        for (turn, food) in [(2, 0), (3, 0), (4, 2), (5, 2), (6, 2)] {
            out.context("growth", turn, Some(turn), None);
            analyzer.on_boundary(&observation(turn, food), &mut out);
        }
        assert_eq!(out.findings.len(), 2);
        assert_eq!(out.findings[0].first_observed_turn, 3);
        assert_eq!(out.findings[0].resolved_turn, Some(4));
        assert_eq!(out.findings[1].first_observed_turn, 6);
    }
}
