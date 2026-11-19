//! Shared, read-only AI policy. Advisor legality and emergency rules still apply.
use super::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TraitWeights {
    economy: i32,
    science: i32,
    military: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: u32,
    crisis_economy_bonus: i32,
    war_military_bonus: i32,
    preparation_military_bonus: i32,
    space_science_bonus: i32,
    deficit_reserve_turns: i32,
    upkeep_buffer_turns: i32,
    reserve_refill_turns: i32,
    research_interest_want: i32,
    traits: BTreeMap<String, TraitWeights>,
}

fn policy() -> &'static Policy {
    static POLICY: OnceLock<Policy> = OnceLock::new();
    POLICY.get_or_init(|| {
        let policy: Policy = toml::from_str(include_str!(
            "../../data/rulesets/civcraft/content/strategy.toml"
        ))
        .expect("valid shared AI strategy policy");
        assert_eq!(policy.schema_version, 1);
        for value in [
            policy.crisis_economy_bonus,
            policy.war_military_bonus,
            policy.preparation_military_bonus,
            policy.space_science_bonus,
            policy.deficit_reserve_turns,
            policy.upkeep_buffer_turns,
            policy.reserve_refill_turns,
            policy.research_interest_want,
        ] {
            assert!((0..=100).contains(&value));
        }
        assert!(policy.reserve_refill_turns > 0);
        assert_eq!(policy.traits.len(), 8);
        for (name, weights) in &policy.traits {
            assert!(ai_traits::TRAITS.contains(&name.as_str()));
            for weight in [weights.economy, weights.science, weights.military] {
                assert!((0..=100).contains(&weight));
            }
        }
        policy
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Posture {
    Develop,
    Recover,
    Defend,
    Prepare,
    SpaceRace,
}

/// Relative weights, not budget percentages. Inputs use only this nation's
/// finances, commitments and spaceship; no opponent's private research queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct StrategicPriorities {
    pub economy: i32,
    pub science: i32,
    pub military: i32,
    pub posture: Posture,
    pub economic_crisis: bool,
    pub wars: usize,
    pub preparing: bool,
    pub space_program: bool,
    // Full diagnostics retain the amounts; lean changes omit these noisy inputs.
    pub projected_income: i32,
    pub gold_reserve: i32,
}

impl StrategicPriorities {
    /// This gates optional expansion, never defense, revenge or ally obligations.
    pub(crate) fn can_start_optional_war(&self) -> bool {
        !self.economic_crisis && self.wars == 0 && !self.space_program
    }
}

impl Simulation {
    pub(crate) fn strategic_upkeep_buffer(&self, owner: usize) -> i32 {
        let (buildings, city_support): (i32, i32) = self
            .cities_owned_by(owner)
            .map(|city| {
                let forecast = self.city_forecast(city);
                (forecast.building_upkeep, forecast.support.gold)
            })
            .fold((0, 0), |(buildings, units), (building, unit)| {
                (buildings + building, units + unit)
            });
        let national_support: i32 = self
            .national_unit_gold_upkeep(owner)
            .paid
            .iter()
            .map(|(_, cost)| cost)
            .sum();
        // A government with generous free support must still retain cash for
        // the transition and a newly available government's higher upkeep.
        let transition_support = std::iter::once(Government::during_revolution())
            .chain(
                Government::targets()
                    .iter()
                    .copied()
                    .filter(|&government| self.government_available(owner, government)),
            )
            .map(|government| {
                self.national_unit_gold_upkeep_for_government(owner, government)
                    .paid
                    .iter()
                    .map(|(_, cost)| cost)
                    .sum::<i32>()
            })
            .max()
            .unwrap_or(0);
        buildings
            .saturating_add(city_support.max(national_support).max(transition_support))
            .saturating_mul(policy().upkeep_buffer_turns)
    }

    pub(crate) fn strategic_priorities(&self, owner: usize) -> StrategicPriorities {
        let rules = policy();
        let mut economy = 100;
        let mut science = 100;
        let mut military = 100;
        if let Some(civ) = self.ai_civilization(owner) {
            for (name, weights) in &rules.traits {
                if ai_traits::trait_enabled(civ, name) {
                    economy += weights.economy;
                    science += weights.science;
                    military += weights.military;
                }
            }
        }
        let base_reserve = self.ai_gold_reserve(owner);
        let projected_income = self.ai_projected_income(owner);
        let gold_reserve = base_reserve
            .saturating_add(self.strategic_upkeep_buffer(owner))
            .saturating_add(
                projected_income
                    .saturating_neg()
                    .max(0)
                    .saturating_mul(rules.deficit_reserve_turns),
            );
        let economic_crisis = projected_income < 0 && self.players[owner].gold < gold_reserve;
        // An uncontacted player, a defeated rival or a barbarian's permanent
        // hostility does not turn a nation into a wartime economy.
        let wars = (0..self.players.len())
            .filter(|&target| {
                self.player_alive(target)
                    && !self.player_has_no_diplomacy(target)
                    && self.contacted_pact(owner, target) == Some(Pact::War)
            })
            .count();
        let preparing = self.ai_war_plans.iter().any(|(&(planner, target), plan)| {
            planner == owner
                && self.player_alive(target)
                && self.contacted_pact(owner, target).is_some()
                && !self.at_war(owner, target)
                && plan.countdown >= 0
        });
        let space_program = self
            .victory_enabled(crate::rules::engine_rules::VictoryCondition::SpaceRace)
            && self.spaceships[owner].built != [0; 3];
        if economic_crisis {
            economy += rules.crisis_economy_bonus;
        }
        if wars > 0 {
            military += rules.war_military_bonus;
        } else if preparing {
            military += rules.preparation_military_bonus;
        }
        if space_program {
            science += rules.space_science_bonus;
        }
        let posture = if economic_crisis {
            Posture::Recover
        } else if wars > 0 {
            Posture::Defend
        } else if preparing {
            Posture::Prepare
        } else if space_program {
            Posture::SpaceRace
        } else {
            Posture::Develop
        };
        StrategicPriorities {
            economy,
            science,
            military,
            posture,
            economic_crisis,
            wars,
            preparing,
            space_program,
            projected_income,
            gold_reserve,
        }
    }

    pub(super) fn strategic_research_want(
        &self,
        owner: usize,
        tech: Tech,
        want: f32,
        priorities: &StrategicPriorities,
        cities: usize,
    ) -> f32 {
        let percent = self.strategic_technology_percent(owner, tech, Some(priorities));
        // Existing negative advice stays negative. A small trait/need interest
        // lets a useful long-term goal enter the prerequisite propagation pass.
        want * percent as f32 / 100.0
            + if want >= 0.0 {
                cities as f32
                    * policy().research_interest_want as f32
                    * (percent - 100).max(0) as f32
                    / 100.0
            } else {
                0.0
            }
    }
}

pub(super) fn reserve_refill_turns() -> i32 {
    policy().reserve_refill_turns
}

#[cfg(test)]
mod tests;
