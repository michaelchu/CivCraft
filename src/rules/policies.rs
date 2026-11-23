//! Typed, validated policy catalogs loaded from `data/rulesets/civcraft/content/`.
//!
//! Moving these definitions under `rules` keeps simulation modules free of
//! direct TOML loading and allows a single `ContentRuleset` load/validation
//! boundary across all editable campaign files.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use super::Tech;
use super::native::TechnologyCatalog;

#[derive(Default)]
pub(crate) struct TraitWeights {
    pub(crate) economy: i32,
    pub(crate) science: i32,
    pub(crate) military: i32,
}

pub(crate) struct StrategyCatalog {
    pub(crate) crisis_economy_bonus: i32,
    pub(crate) war_military_bonus: i32,
    pub(crate) preparation_military_bonus: i32,
    pub(crate) space_science_bonus: i32,
    pub(crate) space_max_missing_techs: usize,
    pub(crate) space_min_city_shields: i32,
    pub(crate) space_research_want: i32,
    pub(crate) deficit_reserve_turns: i32,
    pub(crate) upkeep_buffer_turns: i32,
    pub(crate) reserve_refill_turns: i32,
    pub(crate) research_interest_want: i32,
    pub(crate) traits: BTreeMap<String, TraitWeights>,
}

pub(crate) struct RelationshipPolicy {
    pub(crate) trade_gain: i32,
    pub(crate) support_gain: i32,
    pub(crate) breach_grievance: i32,
    pub(crate) rebuild_gain: i32,
    pub(crate) trade_cooldown_turns: i32,
    pub(crate) support_cooldown_turns: i32,
    pub(crate) decay_turns: i32,
    pub(crate) positive_decay: i32,
    pub(crate) grievance_decay: i32,
    pub(crate) caution_threshold: i32,
    pub(crate) attitude_divisor: i32,
    pub(crate) trade_score_divisor: i32,
    pub(crate) maximum_trade_adjustment_percent: i32,
    pub(crate) war_threshold_per_score: i32,
}

pub(crate) struct NegotiationPolicy {
    pub(crate) aid_minimum_trust: i32,
    pub(crate) aid_surplus_percent: i32,
    pub(crate) maximum_aid: i32,
    pub(crate) map_minimum_trust: i32,
    pub(crate) map_minimum_tiles: usize,
    pub(crate) map_tiles_per_gold: i64,
    pub(crate) map_seller_percent: i64,
    pub(crate) map_trait_bonus_percent: i64,
    pub(crate) maximum_map_value: i64,
    pub(crate) peace_income_horizon: i32,
    pub(crate) peace_visible_force_ratio: i64,
    pub(crate) peace_force_value: i64,
    pub(crate) maximum_peace_value: i64,
}

pub(crate) struct DiplomacyCatalog {
    pub(crate) cadence_turns: i32,
    pub(crate) technologies_per_side: usize,
    pub(crate) seller_percent: i64,
    pub(crate) commercial_margin_percent: i64,
    pub(crate) liquidity_discount_percent: i64,
    pub(crate) scientific_bonus_percent: i64,
    pub(crate) trait_bonus_percent: i64,
    pub(crate) research_goal_bonus_percent: i64,
    pub(crate) military_need_bonus_percent: i64,
    pub(crate) military_minimum_trust: i32,
    pub(crate) military_minimum_power: i32,
    pub(crate) victory_technologies: Vec<String>,
    pub(crate) interests: BTreeMap<String, Vec<String>>,
    pub(crate) relationships: RelationshipPolicy,
    pub(crate) negotiations: NegotiationPolicy,
}

/// Situational AI government bias (Civ III-inspired, owner-authorized). No
/// government is barred: these percentages only scale advisor scores.
pub(crate) struct GovernmentSituationCatalog {
    /// Governments favored while the nation is at peace.
    pub(crate) peace: Vec<String>,
    /// Governments favored while at war with a real civilization.
    pub(crate) war: Vec<String>,
    /// Score bonus for the favored family of the current situation.
    pub(crate) situation_bias_percent: i32,
    /// Share of the wartime bias felt by a nation without a wartime trait.
    pub(crate) unwarlike_war_scale_percent: i32,
    /// Extra score bonus for the war family, in peace or war, for a nation
    /// with one of `wartime_traits`.
    pub(crate) trait_war_bias_percent: i32,
    /// Verified civilization traits that bias a nation toward the war family.
    pub(crate) wartime_traits: Vec<String>,
    /// A different government replaces the current one only when its biased
    /// score exceeds the current biased score by this percent.
    pub(crate) retention_margin_percent: i32,
}

pub(crate) struct VassalageCatalog {
    pub(crate) required_technology: String,
    pub(crate) voluntary_minimum_turns: i32,
    pub(crate) independence_size_percent: i64,
    pub(crate) retained_land_percent: i64,
    pub(crate) negotiation_cooldown_turns: i32,
    pub(crate) submission_force_ratio: i64,
    pub(crate) voluntary_minimum_trust: i32,
    pub(crate) minimum_peace_value: i64,
    pub(crate) maximum_protection_pressure_percent: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReligionAiRules {
    pub(crate) culture_goal_points: i64,
    pub(crate) culture_goal_bonus: i32,
    pub(crate) religious_culture_bonus: i32,
    pub(crate) recovery_gold_bonus: i32,
    pub(crate) content_growth_value: i32,
    pub(crate) content_unrest_value: i32,
    pub(crate) adoption_city_value: i32,
    pub(crate) switching_city_cost: i32,
    pub(crate) diplomacy_value: i32,
    pub(crate) ally_weight: i32,
    pub(crate) friend_weight: i32,
    pub(crate) local_defense_percent: i32,
    pub(crate) spread_percent: i32,
    pub(crate) foreign_spread_value: i32,
    pub(crate) research_cap_per_city: i32,
    pub(crate) research_native_priority_per_city: i32,
    pub(crate) research_max_steps: usize,
    pub(crate) research_cost_scale: i32,
    pub(crate) maximum_cities: usize,
    pub(crate) maximum_spread_targets: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FaithDefinition {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) tech: String,
    #[serde(skip)]
    pub(crate) resolved_tech: Option<Tech>,
    #[serde(default)]
    pub(crate) content: i32,
    #[serde(default)]
    pub(crate) culture: i32,
    #[serde(default)]
    pub(crate) gold: i32,
    #[serde(default)]
    pub(crate) science: i32,
    #[serde(default)]
    pub(crate) local_pressure: i32,
    #[serde(default)]
    pub(crate) outgoing_pressure: i32,
    #[serde(default)]
    pub(crate) trade_pressure: i32,
}

impl FaithDefinition {
    pub(crate) fn tech(&self) -> Tech {
        self.resolved_tech.expect("validated faith technology")
    }

    pub(crate) fn tech_name(&self) -> &'static str {
        self.tech().name()
    }

    pub(crate) fn bonus_description(&self) -> String {
        let mut parts = Vec::new();
        for (v, label) in [
            (self.content, "content citizen"),
            (self.culture, "culture"),
            (self.gold, "gold"),
            (self.science, "science"),
            (self.local_pressure, "local influence"),
            (self.outgoing_pressure, "outgoing pressure"),
            (self.trade_pressure, "trade spread pressure"),
        ] {
            if v > 0 {
                parts.push(format!("+{v} {label}"));
            }
        }
        parts.join(", ")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReligionCatalog {
    pub(crate) faiths: Vec<FaithDefinition>,
    pub(crate) ai: ReligionAiRules,
    pub(crate) founding_influence: i32,
    pub(crate) maximum_influence: i32,
    pub(crate) unaligned_weight: i32,
    pub(crate) presence_percent: i32,
    pub(crate) conversion_percent: i32,
    pub(crate) retention_percent: i32,
    pub(crate) lead_percent: i32,
    pub(crate) decay_divisor: i32,
    pub(crate) holy_pressure: i32,
    pub(crate) community_pressure: i32,
    pub(crate) local_distance: i32,
    pub(crate) local_pressure: i32,
    pub(crate) trade_pressure: i32,
    pub(crate) incoming_cap: i32,
    pub(crate) cooldown_turns: i32,
    pub(crate) tolerant_culture: i32,
    pub(crate) devout_content: i32,
    pub(crate) devout_pressure: i32,
    pub(crate) missionary_pressure: i32,
    pub(crate) shared_attitude: i32,
    pub(crate) devout_difference: i32,
    pub(crate) missionary_difference: i32,
    pub(crate) ai_minimum_improvement: i32,
}

pub(crate) fn default_strategy() -> StrategyCatalog {
    let w = |economy, science, military| TraitWeights {
        economy,
        science,
        military,
    };
    StrategyCatalog {
        crisis_economy_bonus: 60,
        war_military_bonus: 40,
        preparation_military_bonus: 20,
        space_science_bonus: 40,
        space_max_missing_techs: 9,
        space_min_city_shields: 8,
        space_research_want: 200,
        deficit_reserve_turns: 3,
        upkeep_buffer_turns: 2,
        reserve_refill_turns: 5,
        research_interest_want: 4,
        traits: BTreeMap::from([
            ("agricultural".to_owned(), w(15, 0, 0)),
            ("commercial".to_owned(), w(20, 0, 0)),
            ("expansionist".to_owned(), w(10, 0, 5)),
            ("industrious".to_owned(), w(20, 0, 0)),
            ("militaristic".to_owned(), w(0, 0, 25)),
            ("religious".to_owned(), w(10, 0, 0)),
            ("scientific".to_owned(), w(0, 25, 0)),
            ("seafaring".to_owned(), w(10, 5, 0)),
        ]),
    }
}

pub(crate) fn default_diplomacy() -> DiplomacyCatalog {
    let list = |names: &[&str]| names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let interests = BTreeMap::from([
        (
            "agricultural".to_owned(),
            list(&["Pottery", "Construction", "Sanitation", "Refrigeration"]),
        ),
        (
            "commercial".to_owned(),
            list(&["Currency", "Banking", "Economics", "The Corporation"]),
        ),
        (
            "expansionist".to_owned(),
            list(&["Map Making", "Navigation", "Magnetism", "Steam Power"]),
        ),
        (
            "industrious".to_owned(),
            list(&[
                "Masonry",
                "Construction",
                "Engineering",
                "Steam Power",
                "Industrialization",
                "Replaceable Parts",
            ]),
        ),
        (
            "militaristic".to_owned(),
            list(&[
                "Iron Working",
                "Feudalism",
                "Chivalry",
                "Military Tradition",
                "Nationalism",
                "Motorized Transportation",
                "Rocketry",
            ]),
        ),
        (
            "religious".to_owned(),
            list(&[
                "Ceremonial Burial",
                "Mysticism",
                "Polytheism",
                "Monarchy",
                "Theology",
            ]),
        ),
        (
            "scientific".to_owned(),
            list(&[
                "Writing",
                "Literature",
                "Education",
                "Theory of Gravity",
                "Scientific Method",
                "Computers",
            ]),
        ),
        (
            "seafaring".to_owned(),
            list(&["Map Making", "Navigation", "Magnetism", "Combustion"]),
        ),
    ]);
    DiplomacyCatalog {
        cadence_turns: 6,
        technologies_per_side: 8,
        seller_percent: 75,
        commercial_margin_percent: 10,
        liquidity_discount_percent: 10,
        scientific_bonus_percent: 10,
        trait_bonus_percent: 20,
        research_goal_bonus_percent: 35,
        military_need_bonus_percent: 25,
        military_minimum_trust: 1000,
        military_minimum_power: 3,
        victory_technologies: list(&["Space Flight", "Satellites", "Superconductor"]),
        interests,
        relationships: RelationshipPolicy {
            trade_gain: 8,
            support_gain: 20,
            breach_grievance: 50,
            rebuild_gain: 4,
            trade_cooldown_turns: 6,
            support_cooldown_turns: 12,
            decay_turns: 12,
            positive_decay: 10,
            grievance_decay: 5,
            caution_threshold: 40,
            attitude_divisor: 5,
            trade_score_divisor: 10,
            maximum_trade_adjustment_percent: 10,
            war_threshold_per_score: 20,
        },
        negotiations: NegotiationPolicy {
            aid_minimum_trust: 5000,
            aid_surplus_percent: 25,
            maximum_aid: 200,
            map_minimum_trust: 1000,
            map_minimum_tiles: 8,
            map_tiles_per_gold: 2,
            map_seller_percent: 65,
            map_trait_bonus_percent: 25,
            maximum_map_value: 300,
            peace_income_horizon: 6,
            peace_visible_force_ratio: 2,
            peace_force_value: 10,
            maximum_peace_value: 500,
        },
    }
}

pub(crate) fn default_government_situation() -> GovernmentSituationCatalog {
    let list = |names: &[&str]| names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    GovernmentSituationCatalog {
        peace: list(&["Republic", "Democracy"]),
        war: list(&["Monarchy", "Communism", "Fascism", "Feudalism"]),
        situation_bias_percent: 20,
        unwarlike_war_scale_percent: 50,
        trait_war_bias_percent: 20,
        wartime_traits: list(&["militaristic"]),
        retention_margin_percent: 15,
    }
}

pub(crate) fn default_vassalage() -> VassalageCatalog {
    VassalageCatalog {
        required_technology: "Feudalism".to_owned(),
        voluntary_minimum_turns: 10,
        independence_size_percent: 50,
        retained_land_percent: 50,
        negotiation_cooldown_turns: 10,
        submission_force_ratio: 3,
        voluntary_minimum_trust: 5000,
        minimum_peace_value: 100,
        maximum_protection_pressure_percent: 150,
    }
}

impl ReligionAiRules {
    fn validate(&self) -> Result<(), String> {
        if !(1..=20000).contains(&self.culture_goal_points) {
            return Err("religion ai.culture_goal_points out of 1..=20000".into());
        }
        for v in [
            self.culture_goal_bonus,
            self.religious_culture_bonus,
            self.recovery_gold_bonus,
            self.content_growth_value,
            self.content_unrest_value,
            self.adoption_city_value,
            self.switching_city_cost,
            self.diplomacy_value,
            self.local_defense_percent,
            self.spread_percent,
            self.foreign_spread_value,
            self.research_cap_per_city,
            self.research_native_priority_per_city,
        ] {
            if !(0..=200).contains(&v) {
                return Err(format!("religion AI parameter {v} out of 0..=200"));
            }
        }
        if !(1..=5).contains(&self.ally_weight) || !(1..=5).contains(&self.friend_weight) {
            return Err("religion AI ally/friend weights out of 1..=5".into());
        }
        if !(1..=10).contains(&self.research_max_steps)
            || !(1..=1000).contains(&self.research_cost_scale)
            || !(1..=32).contains(&self.maximum_cities)
            || !(1..=128).contains(&self.maximum_spread_targets)
        {
            return Err("religion AI search limits out of range".into());
        }
        Ok(())
    }
}

pub(crate) fn parse_religion(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<ReligionCatalog, String> {
    let mut r: ReligionCatalog = toml::from_str(source).map_err(|err| err.to_string())?;
    if !(1..=10000).contains(&r.maximum_influence)
        || !(1..=r.maximum_influence).contains(&r.founding_influence)
        || !(1..=1000).contains(&r.unaligned_weight)
        || !(1..=1000).contains(&r.decay_divisor)
    {
        return Err("religion influence/decay constants out of range".into());
    }
    if !(1..=100).contains(&r.presence_percent)
        || r.presence_percent > r.retention_percent
        || r.retention_percent >= r.conversion_percent
        || r.conversion_percent > 100
        || !(1..=100).contains(&r.lead_percent)
    {
        return Err("religion threshold percentages invalid".into());
    }
    for v in [
        r.holy_pressure,
        r.community_pressure,
        r.local_pressure,
        r.trade_pressure,
        r.incoming_cap,
        r.devout_pressure,
        r.missionary_pressure,
    ] {
        if !(0..=100).contains(&v) {
            return Err(format!("religion pressure value {v} out of 0..=100"));
        }
    }
    if !(1..=10).contains(&r.local_distance) || !(1..=100).contains(&r.cooldown_turns) {
        return Err("religion distance or cooldown out of range".into());
    }
    for v in [
        r.tolerant_culture,
        r.devout_content,
        r.ai_minimum_improvement,
    ] {
        if !(0..=10).contains(&v) {
            return Err(format!("religion bonus/threshold {v} out of 0..=10"));
        }
    }
    if !(0..=20).contains(&r.shared_attitude)
        || !(-20..=0).contains(&r.devout_difference)
        || !(-20..=0).contains(&r.missionary_difference)
    {
        return Err("religion attitude modifiers out of range".into());
    }
    r.ai.validate()?;
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    if r.faiths.len() != 7 {
        return Err(format!("expected 7 faiths, found {}", r.faiths.len()));
    }
    for f in &mut r.faiths {
        if !(1..=7).contains(&f.id) || !ids.insert(f.id) || !names.insert(f.name.clone()) {
            return Err(format!("invalid or duplicate faith {} ({})", f.id, f.name));
        }
        let tech_record = technologies
            .technologies
            .iter()
            .find(|t| t.id == f.tech)
            .ok_or_else(|| format!("faith {:?} references unknown tech {:?}", f.name, f.tech))?;
        f.resolved_tech = Some(Tech(tech_record.runtime_index));
        for v in [
            f.content,
            f.culture,
            f.gold,
            f.science,
            f.local_pressure,
            f.outgoing_pressure,
            f.trade_pressure,
        ] {
            if !(0..=2).contains(&v) {
                return Err(format!("faith {:?} bonus {v} out of 0..=2", f.name));
            }
        }
    }
    Ok(r)
}
