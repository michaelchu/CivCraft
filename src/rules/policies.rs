//! Typed, validated policy catalogs loaded from `data/rulesets/civcraft/content/`.
//!
//! Moving these definitions under `rules` keeps simulation modules free of
//! direct TOML loading and allows a single `ContentRuleset` load/validation
//! boundary across all editable campaign files.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use super::Tech;
use super::native::TechnologyCatalog;

pub(crate) const TRAIT_NAMES: &[&str] = &[
    "agricultural",
    "commercial",
    "expansionist",
    "industrious",
    "militaristic",
    "religious",
    "scientific",
    "seafaring",
];

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TraitWeights {
    pub(crate) economy: i32,
    pub(crate) science: i32,
    pub(crate) military: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StrategyCatalog {
    pub(crate) schema_version: u32,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiplomacyCatalog {
    pub(crate) schema_version: u32,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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

fn technology_name_exists(technologies: &TechnologyCatalog, name: &str) -> bool {
    technologies
        .technologies
        .iter()
        .any(|tech| tech.display_name.as_deref().unwrap_or(&tech.name) == name || tech.name == name)
        || Tech::all().any(|tech| tech.name() == name)
}

pub(crate) fn parse_strategy(source: &str) -> Result<StrategyCatalog, String> {
    let policy: StrategyCatalog = toml::from_str(source).map_err(|err| err.to_string())?;
    if policy.schema_version != 1 {
        return Err(format!(
            "unsupported strategy schema_version {}",
            policy.schema_version
        ));
    }
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
        if !(0..=100).contains(&value) {
            return Err(format!("strategy value {value} out of range 0..=100"));
        }
    }
    if policy.reserve_refill_turns <= 0
        || policy.space_max_missing_techs == 0
        || policy.space_min_city_shields <= 0
        || policy.space_research_want <= 0
    {
        return Err("strategy space/reserve thresholds must be positive".into());
    }
    if policy.traits.len() != TRAIT_NAMES.len() {
        return Err(format!(
            "expected {} trait entries in strategy policy, found {}",
            TRAIT_NAMES.len(),
            policy.traits.len()
        ));
    }
    for (name, weights) in &policy.traits {
        if !TRAIT_NAMES.contains(&name.as_str()) {
            return Err(format!("unknown strategy trait {name:?}"));
        }
        for weight in [weights.economy, weights.science, weights.military] {
            if !(0..=100).contains(&weight) {
                return Err(format!(
                    "strategy weight {weight} for {name:?} out of 0..=100"
                ));
            }
        }
    }
    Ok(policy)
}

impl RelationshipPolicy {
    fn validate(&self) -> Result<(), String> {
        for value in [
            self.trade_gain,
            self.support_gain,
            self.breach_grievance,
            self.rebuild_gain,
            self.positive_decay,
            self.grievance_decay,
            self.caution_threshold,
        ] {
            if !(1..=100).contains(&value) {
                return Err(format!("relationship score value {value} out of 1..=100"));
            }
        }
        for value in [
            self.trade_cooldown_turns,
            self.support_cooldown_turns,
            self.decay_turns,
            self.attitude_divisor,
            self.trade_score_divisor,
        ] {
            if !(1..=1000).contains(&value) {
                return Err(format!("relationship turn/divisor {value} out of 1..=1000"));
            }
        }
        if !(0..=20).contains(&self.maximum_trade_adjustment_percent) {
            return Err("maximum_trade_adjustment_percent out of 0..=20".into());
        }
        if !(0..=100).contains(&self.war_threshold_per_score) {
            return Err("war_threshold_per_score out of 0..=100".into());
        }
        Ok(())
    }
}

impl NegotiationPolicy {
    fn validate(&self) -> Result<(), String> {
        if !(0..=10000).contains(&self.aid_minimum_trust)
            || !(0..=10000).contains(&self.map_minimum_trust)
        {
            return Err("negotiation trust threshold out of 0..=10000".into());
        }
        if !(1..=50).contains(&self.aid_surplus_percent)
            || !(1..=10000).contains(&self.maximum_aid)
            || !(1..=10000).contains(&self.map_minimum_tiles)
        {
            return Err("negotiation aid/map threshold out of range".into());
        }
        for value in [
            self.map_tiles_per_gold,
            self.map_seller_percent,
            self.map_trait_bonus_percent,
            self.maximum_map_value,
            i64::from(self.peace_income_horizon),
            self.peace_visible_force_ratio,
            self.peace_force_value,
            self.maximum_peace_value,
        ] {
            if !(1..=1000).contains(&value) {
                return Err(format!("negotiation parameter {value} out of 1..=1000"));
            }
        }
        if self.map_seller_percent >= 100 {
            return Err("map_seller_percent must be < 100".into());
        }
        Ok(())
    }
}

pub(crate) fn parse_diplomacy(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<DiplomacyCatalog, String> {
    let policy: DiplomacyCatalog = toml::from_str(source).map_err(|err| err.to_string())?;
    if policy.schema_version != 1 {
        return Err(format!(
            "unsupported diplomacy schema_version {}",
            policy.schema_version
        ));
    }
    policy.relationships.validate()?;
    policy.negotiations.validate()?;
    if policy.cadence_turns <= 0 || !(1..=16).contains(&policy.technologies_per_side) {
        return Err("diplomacy cadence_turns or technologies_per_side out of range".into());
    }
    if !(1..=100).contains(&policy.seller_percent) {
        return Err("diplomacy seller_percent out of 1..=100".into());
    }
    if policy.interests.len() != TRAIT_NAMES.len() {
        return Err("all Conquests traits need trade interests".into());
    }
    if policy.military_minimum_power <= 0 {
        return Err("diplomacy military_minimum_power must be positive".into());
    }
    for (name, tech_names) in &policy.interests {
        if !TRAIT_NAMES.contains(&name.as_str()) {
            return Err(format!("unknown diplomacy interest trait {name:?}"));
        }
        for technology in tech_names {
            if !technology_name_exists(technologies, technology) {
                return Err(format!("unknown diplomacy policy technology {technology}"));
            }
        }
    }
    for technology in &policy.victory_technologies {
        if !technology_name_exists(technologies, technology) {
            return Err(format!("unknown diplomacy victory technology {technology}"));
        }
    }
    Ok(policy)
}

pub(crate) fn parse_vassalage(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<VassalageCatalog, String> {
    let p: VassalageCatalog = toml::from_str(source).map_err(|err| err.to_string())?;
    if !technology_name_exists(technologies, &p.required_technology) {
        return Err(format!(
            "unknown vassalage required_technology {:?}",
            p.required_technology
        ));
    }
    if p.voluntary_minimum_turns <= 0 || p.negotiation_cooldown_turns <= 0 {
        return Err("vassalage turn timers must be positive".into());
    }
    if !(1..=100).contains(&p.independence_size_percent)
        || !(1..=100).contains(&p.retained_land_percent)
    {
        return Err("vassalage percentage thresholds out of 1..=100".into());
    }
    if p.submission_force_ratio <= 1 || p.minimum_peace_value <= 0 {
        return Err("vassalage submission_force_ratio or minimum_peace_value invalid".into());
    }
    if !(1..=1000).contains(&p.maximum_protection_pressure_percent)
        || !(0..=10000).contains(&p.voluntary_minimum_trust)
    {
        return Err("vassalage pressure or trust threshold out of range".into());
    }
    Ok(p)
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
