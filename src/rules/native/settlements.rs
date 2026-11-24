//! Settlement AI, Great Leader, village, barbarian and citizen rules.

use super::*;

pub(crate) struct SettlementCatalog {
    pub(crate) barbarians: NativeBarbarianUnits,
    pub(crate) ai: NativeSettlementAi,
    pub(crate) great_leader: NativeGreatLeaderRules,
    pub(crate) villages: NativeVillageRules,
    pub(crate) citizens: NativeCitizenRules,
}

pub(crate) struct NativeBarbarianUnits {
    pub(crate) basic_unit: String,
    pub(crate) advanced_unit: String,
    pub(crate) sea_unit: String,
}

pub(crate) struct NativeCitizenRules {
    pub(crate) partisan_unit: String,
    pub(crate) nationality: bool,
    pub(crate) partisans_pct: i32,
}

pub(crate) struct NativeSettlementAi {
    pub(crate) food_priority: i32,
    pub(crate) shield_priority: i32,
    pub(crate) trade_priority: i32,
    pub(crate) research_priority_percent: i32,
    pub(crate) no_research_priority: i32,
    pub(crate) growth_priority: i32,
    pub(crate) growth_potential_deemphasis: i32,
    pub(crate) preferred_land_tiles_per_city: i32,
    pub(crate) expansion_cost_priority: i32,
    pub(crate) free_expansion_cities: i32,
    pub(crate) growth_food_threshold: i32,
    pub(crate) perfection: i32,
    pub(crate) good_enough: i32,
    pub(crate) discount_period: i32,
    pub(crate) defense_emphasis: i32,
    pub(crate) naval_emphasis: i32,
    pub(crate) wonder_city_min_cities: i32,
    pub(crate) wonder_city_pop_founder_divisor: i32,
    pub(crate) wonder_city_other_founder_divisor: i32,
    pub(crate) expansionist_trait_baseline: i32,
    pub(crate) ferry_cost_divisor: i32,
    pub(crate) ferry_tech_want: i32,
}

pub(crate) struct NativeGreatLeaderRules {
    pub(crate) cities_per_army: usize,
    pub(crate) attack_denominator: i32,
    pub(crate) attack_heroic_epic_denominator: i32,
    pub(crate) defense_denominator: i32,
    pub(crate) defense_heroic_epic_denominator: i32,
    /// A kill only counts toward a Leader when the loser's strength is at least
    /// this percentage of the winner's, so weak units cannot be farmed.
    pub(crate) min_victory_strength_percent: i32,
    /// Chance per first-in-the-world technology of a Scientific Great Leader,
    /// and the higher chance for a civilization with the scientific trait.
    /// Community-documented (forum posts), not from official documentation.
    pub(crate) scientific_leader_percent: i32,
    pub(crate) scientific_leader_trait_percent: i32,
}

pub(crate) struct NativeVillageRules {
    pub(crate) barbarian_onset_turn: i32,
    pub(crate) outcomes: Vec<NativeVillageOutcome>,
    pub(crate) map_reveal_radius_squared: i32,
}

pub(crate) struct NativeVillageOutcome {
    pub(crate) weight: i32,
    pub(crate) reward: NativeVillageReward,
}

pub(crate) enum NativeVillageReward {
    Gold { amount: i32 },
    Technology,
    Reinforcements,
    Barbarians,
    Settlement,
    Map,
}

impl NativeVillageRules {
    pub(crate) fn total_weight(&self) -> i32 {
        self.outcomes.iter().map(|outcome| outcome.weight).sum()
    }

    pub(crate) fn reward_at(&self, mut roll: i32) -> &NativeVillageReward {
        for outcome in &self.outcomes {
            if roll < outcome.weight {
                return &outcome.reward;
            }
            roll -= outcome.weight;
        }
        unreachable!("village roll must be below the validated total weight")
    }

    pub(super) fn validate(&self) -> Result<(), &'static str> {
        if self.outcomes.is_empty() || self.map_reveal_radius_squared < 0 {
            return Err("village outcomes need rewards and a nonnegative reveal radius");
        }
        let mut total = 0_i32;
        for outcome in &self.outcomes {
            if outcome.weight <= 0 {
                return Err("village outcome weights must be positive");
            }
            total = total
                .checked_add(outcome.weight)
                .ok_or("village weight total overflows")?;
            if matches!(outcome.reward, NativeVillageReward::Gold { amount } if amount < 0) {
                return Err("village gold must be nonnegative");
            }
        }
        Ok(())
    }
}

pub(crate) fn settlements() -> &'static SettlementCatalog {
    static CATALOG: OnceLock<SettlementCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog = SettlementCatalog {
            barbarians: NativeBarbarianUnits {
                basic_unit: "warrior".to_owned(),
                advanced_unit: "horseman".to_owned(),
                sea_unit: "galley".to_owned(),
            },
            ai: NativeSettlementAi {
                food_priority: 30,
                shield_priority: 17,
                trade_priority: 18,
                research_priority_percent: 120,
                no_research_priority: 1,
                growth_priority: 45,
                growth_potential_deemphasis: 4,
                preferred_land_tiles_per_city: 14,
                expansion_cost_priority: 400,
                free_expansion_cities: 3,
                growth_food_threshold: 2,
                perfection: 3,
                good_enough: 250,
                discount_period: 24,
                defense_emphasis: 20,
                naval_emphasis: 20,
                wonder_city_min_cities: 3,
                wonder_city_pop_founder_divisor: 5,
                wonder_city_other_founder_divisor: 2,
                expansionist_trait_baseline: 50,
                ferry_cost_divisor: 3,
                ferry_tech_want: 500,
            },
            great_leader: NativeGreatLeaderRules {
                cities_per_army: 4,
                attack_denominator: 16,
                attack_heroic_epic_denominator: 12,
                defense_denominator: 32,
                defense_heroic_epic_denominator: 24,
                min_victory_strength_percent: 50,
                scientific_leader_percent: 3,
                scientific_leader_trait_percent: 5,
            },
            villages: NativeVillageRules {
                barbarian_onset_turn: 60,
                map_reveal_radius_squared: 30,
                outcomes: vec![
                    NativeVillageOutcome {
                        weight: 1,
                        reward: NativeVillageReward::Gold { amount: 25 },
                    },
                    NativeVillageOutcome {
                        weight: 3,
                        reward: NativeVillageReward::Gold { amount: 50 },
                    },
                    NativeVillageOutcome {
                        weight: 1,
                        reward: NativeVillageReward::Gold { amount: 100 },
                    },
                    NativeVillageOutcome {
                        weight: 3,
                        reward: NativeVillageReward::Technology,
                    },
                    NativeVillageOutcome {
                        weight: 2,
                        reward: NativeVillageReward::Reinforcements,
                    },
                    NativeVillageOutcome {
                        weight: 1,
                        reward: NativeVillageReward::Barbarians,
                    },
                    NativeVillageOutcome {
                        weight: 1,
                        reward: NativeVillageReward::Settlement,
                    },
                    NativeVillageOutcome {
                        weight: 2,
                        reward: NativeVillageReward::Map,
                    },
                ],
            },
            citizens: NativeCitizenRules {
                partisan_unit: "guerilla".to_owned(),
                nationality: true,
                partisans_pct: 75,
            },
        };
        catalog
            .villages
            .validate()
            .expect("native village outcomes must be valid");
        for id in [
            &catalog.barbarians.basic_unit,
            &catalog.barbarians.advanced_unit,
            &catalog.barbarians.sea_unit,
            &catalog.citizens.partisan_unit,
        ] {
            assert!(
                units().units.iter().any(|unit| unit.id == *id),
                "native settlement policy references unknown unit {id:?}"
            );
        }
        catalog
    })
}
