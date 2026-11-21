//! Native TOML content for the CivCraft campaign.
//!
//! Catalogs decode directly into native definitions. Import records and BIQ
//! provenance are not required to construct or validate campaign rules.

#![allow(dead_code)]

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnitCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) units: Vec<NativeUnit>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeUnit {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) cost_shields: i32,
    pub(crate) population_cost: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
    pub(crate) moves: i32,
    pub(crate) bombard_strength: i32,
    pub(crate) bombard_range: i32,
    pub(crate) rate_of_fire: i32,
    pub(crate) operational_range: i32,
    pub(crate) capacity: usize,
    pub(crate) required_technology: String,
    pub(crate) required_resources: Vec<String>,
    pub(crate) upgrade_to: String,
    pub(crate) available_to: Vec<String>,
    pub(crate) unique_civilization: String,
    pub(crate) replaces: String,
    pub(crate) domain: UnitDomain,
    pub(crate) enslave_result: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) ai_roles: Vec<String>,
    pub(crate) hit_point_bonus: i32,
    pub(crate) art_folder: String,
    pub(crate) art_root: String,
    pub(crate) zone_of_control: i32,
    pub(crate) bombard_effects: i32,
    pub(crate) require_support: i32,
    pub(crate) create_craters: i32,
    pub(crate) worker_strength: f32,
    pub(crate) air_defense: i32,
    pub(crate) icon_index: i32,
    pub(crate) civilopedia_entry: String,
}

#[derive(Debug, Deserialize)]
pub(crate) enum UnitDomain {
    Land,
    Sea,
    Air,
}

impl UnitDomain {
    pub(crate) fn index(&self) -> i32 {
        match self {
            Self::Land => 0,
            Self::Sea => 1,
            Self::Air => 2,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CivilizationCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) civilizations: Vec<NativeCivilization>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildingCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) rules: Vec<NativeBuildingRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeBuildingRule {
    pub(crate) id: String,
    #[serde(skip)]
    pub(crate) legacy_runtime_index: usize,
    pub(crate) name: String,
    pub(crate) genus: String,
    pub(crate) cost: i32,
    pub(crate) upkeep: i32,
    #[serde(default)]
    pub(crate) culture_per_turn: i32,
    pub(crate) requirements: Vec<NativeBuildingRequirement>,
    pub(crate) obsolete: Vec<NativeBuildingRequirement>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
    #[serde(default)]
    pub(crate) selected_obsolescence: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeBuildingRequirement {
    pub(crate) target: Option<String>,
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) value: i32,
    #[serde(default)]
    #[serde(rename = "count", alias = "second_value")]
    pub(crate) second_value: i32,
    pub(crate) present: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TechnologyCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) technologies: Vec<NativeTechnology>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GovernmentCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) governments: Vec<NativeGovernment>,
}

impl GovernmentCatalog {
    pub(crate) fn get(&self, name: &str) -> Option<&NativeGovernment> {
        self.governments
            .iter()
            .find(|government| government.government.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeGovernment {
    pub(crate) government: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) required_technology: String,
    #[serde(skip)]
    pub(crate) technology: Option<super::Tech>,
    #[serde(default)]
    pub(crate) preferred_government: Option<String>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
    pub(crate) town_free_units: i32,
    pub(crate) city_free_units: i32,
    pub(crate) metropolis_free_units: i32,
    pub(crate) cost_per_excess_unit_gold: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EconomyCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) city: NativeCityEconomy,
    #[serde(default)]
    pub(crate) resources: Vec<NativeResource>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeResource {
    pub(crate) id: String,
    pub(crate) food: i32,
    pub(crate) shield: i32,
    pub(crate) trade: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeCityEconomy {
    pub(crate) city_min_distance: i32,
    pub(crate) food_cost: i32,
    pub(crate) happy_cost: i32,
    pub(crate) granary_food_initial: Vec<i32>,
    pub(crate) granary_food_increment: i32,
    pub(crate) foodbox_percent: i32,
    pub(crate) output_granularity: i32,
    pub(crate) city_center_minimum: [i32; 3],
    pub(crate) no_trade_size: i32,
    pub(crate) full_trade_size: i32,
    pub(crate) gold_upkeep_style: String,
    pub(crate) celebrate_size_limit: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettlementCatalog {
    pub(crate) barbarians: NativeBarbarianUnits,
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) ai: NativeSettlementAi,
    pub(crate) great_leader: NativeGreatLeaderRules,
    pub(crate) villages: NativeVillageRules,
    pub(crate) citizens: NativeCitizenRules,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBarbarianUnits {
    pub(crate) basic_unit: String,
    pub(crate) advanced_unit: String,
    pub(crate) sea_unit: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeCitizenRules {
    pub(crate) partisan_unit: String,
    pub(crate) nationality: bool,
    pub(crate) partisans_pct: i32,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub(crate) struct NativeGreatLeaderRules {
    pub(crate) cities_per_army: usize,
    pub(crate) attack_denominator: i32,
    pub(crate) attack_heroic_epic_denominator: i32,
    pub(crate) defense_denominator: i32,
    pub(crate) defense_heroic_epic_denominator: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeVillageRules {
    pub(crate) barbarian_onset_turn: i32,
    pub(crate) outcomes: Vec<NativeVillageOutcome>,
    pub(crate) map_reveal_radius_squared: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeVillageOutcome {
    pub(crate) weight: i32,
    pub(crate) reward: NativeVillageReward,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
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

    fn validate(&self) -> Result<(), &'static str> {
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

#[derive(Debug, Deserialize)]
pub(crate) struct ExperienceCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) levels: Vec<NativeExperienceLevel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeExperienceLevel {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_hit_points: i32,
    pub(crate) retreat_bonus_percent: i32,
    pub(crate) promotion_chance_per_mille: i32,
    pub(crate) default_rank: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeTechnology {
    pub(crate) id: String,
    /// Stable serialized `Tech` index. This is an identity bridge, not a
    /// second technology definition.
    pub(crate) runtime_index: usize,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    pub(crate) civilopedia_entry: String,
    pub(crate) cost: i32,
    pub(crate) era: i32,
    pub(crate) advance_icon: i32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) prerequisites: Vec<String>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
    #[serde(default = "native_required_for_era")]
    pub(crate) required_for_era: bool,
    #[serde(default)]
    pub(crate) era_gate: Option<i32>,
}

fn native_required_for_era() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeCivilization {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) noun: String,
    pub(crate) adjective: String,
    pub(crate) leader: String,
    pub(crate) primary_color_index: u8,
    pub(crate) secondary_color_index: u8,
    pub(crate) city_style: u8,
    pub(crate) starting_techs: Vec<String>,
    pub(crate) cities: Vec<String>,
    pub(crate) culture_group_index: i32,
    pub(crate) agricultural: bool,
    pub(crate) commercial: bool,
    pub(crate) expansionist: bool,
    pub(crate) industrious: bool,
    pub(crate) militaristic: bool,
    pub(crate) religious: bool,
    pub(crate) scientific: bool,
    pub(crate) seafaring: bool,
    pub(crate) starting_units: Vec<NativeUnitStack>,
    #[serde(default)]
    pub(crate) bonus_units: Vec<NativeUnitStack>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeUnitStack {
    pub(crate) id: String,
    pub(crate) count: i32,
}

fn validate_ids<'a>(ids: impl Iterator<Item = &'a str>, kind: &str) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        if id.is_empty() || !seen.insert(id) {
            return Err(format!("empty or duplicate native {kind} ID {id:?}"));
        }
    }
    Ok(())
}

pub(crate) fn parse_units(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<UnitCatalog, String> {
    let catalog: UnitCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    validate_ids(catalog.units.iter().map(|unit| unit.id.as_str()), "unit")?;
    for unit in &catalog.units {
        if !unit.required_technology.is_empty()
            && super::technologies::runtime_index_in(technologies, &unit.required_technology)
                .is_none()
        {
            return Err(format!("unknown technology ID for {}", unit.id));
        }
        for target in [&unit.upgrade_to, &unit.replaces, &unit.enslave_result] {
            if !target.is_empty()
                && !catalog
                    .units
                    .iter()
                    .any(|candidate| &candidate.id == target)
            {
                return Err(format!("unknown unit ID {target}"));
            }
        }
        if unit.required_resources.len() > 3 {
            return Err("unit supports at most three resource requirements".into());
        }
        if unit.cost_shields < 0 || unit.attack < 0 || unit.defense < 0 || unit.moves < 0 {
            return Err(format!("negative unit stats for {}", unit.id));
        }
        for capability in &unit.capabilities {
            if !super::unit_capabilities::CAPABILITIES.contains(&capability.as_str()) {
                return Err(format!("unknown unit capability {capability}"));
            }
        }
        for role in &unit.ai_roles {
            if !super::unit_capabilities::AI_ROLES.contains(&role.as_str()) {
                return Err(format!("unknown unit AI role {role}"));
            }
        }
    }
    Ok(catalog)
}

pub(crate) fn units() -> &'static UnitCatalog {
    static CATALOG: OnceLock<UnitCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_units(
            include_str!("../../data/rulesets/civcraft/content/units.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native units TOML: {error}"))
    })
}

pub(crate) fn parse_civilizations(source: &str) -> Result<CivilizationCatalog, String> {
    let catalog: CivilizationCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    validate_ids(
        catalog.civilizations.iter().map(|c| c.id.as_str()),
        "civilization",
    )?;
    Ok(catalog)
}

pub(crate) fn civilizations() -> &'static CivilizationCatalog {
    static CATALOG: OnceLock<CivilizationCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_civilizations(include_str!(
            "../../data/rulesets/civcraft/content/civilizations.toml"
        ))
        .unwrap_or_else(|error| panic!("invalid native civilizations TOML: {error}"))
    })
}

pub(crate) fn parse_buildings(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<BuildingCatalog, String> {
    let mut catalog: BuildingCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    validate_ids(
        catalog.rules.iter().map(|rule| rule.id.as_str()),
        "building",
    )?;
    for (index, rule) in catalog.rules.iter_mut().enumerate() {
        rule.legacy_runtime_index = index;
    }
    for rule in &catalog.rules {
        for requirement in rule.requirements.iter().chain(&rule.obsolete) {
            match requirement.kind.as_str() {
                "Tech" => {
                    if requirement
                        .target
                        .as_deref()
                        .and_then(|id| super::technologies::runtime_index_in(technologies, id))
                        .is_none()
                    {
                        return Err(format!("unknown technology target in {}", rule.id));
                    }
                }
                "Improvement" | "OwnerBuildingCount" => {
                    if !requirement
                        .target
                        .as_deref()
                        .is_some_and(|id| catalog.rules.iter().any(|candidate| candidate.id == id))
                    {
                        return Err(format!("unknown building target in {}", rule.id));
                    }
                }
                "Resource" => {
                    if !requirement.target.as_deref().is_some_and(|id| {
                        [
                            "Aluminum",
                            "Coal",
                            "Horses",
                            "Iron",
                            "Oil",
                            "Rubber",
                            "Saltpeter",
                            "Uranium",
                        ]
                        .contains(&id)
                    }) {
                        return Err(format!("unknown resource target in {}", rule.id));
                    }
                }
                "River" | "Lake" | "Sea" | "Water" | "MinSize" | "OwnerCityCount"
                | "OwnerEliteUnit" | "OwnerArmyCount" => {
                    if requirement.target.is_some() {
                        return Err(format!("unexpected named target in {}", rule.id));
                    }
                }
                other => return Err(format!("unsupported building requirement {other}")),
            }
            if requirement.value < 0 || requirement.second_value < 0 {
                return Err(format!("negative building requirement in {}", rule.id));
            }
        }
    }
    // Resolve once for every consumer, including advisors that inspect the
    // native requirement records directly rather than BuildingData.
    let building_ids: std::collections::HashMap<_, _> = catalog
        .rules
        .iter()
        .map(|rule| (rule.id.clone(), rule.legacy_runtime_index))
        .collect();
    for rule in &mut catalog.rules {
        for requirement in rule.requirements.iter_mut().chain(&mut rule.obsolete) {
            if let Some(id) = requirement.target.as_deref() {
                let index = match requirement.kind.as_str() {
                    "Tech" => super::technologies::runtime_index_in(technologies, id).unwrap(),
                    "Improvement" | "OwnerBuildingCount" => building_ids[id],
                    "Resource" => [
                        "Aluminum",
                        "Coal",
                        "Horses",
                        "Iron",
                        "Oil",
                        "Rubber",
                        "Saltpeter",
                        "Uranium",
                    ]
                    .iter()
                    .position(|name| *name == id)
                    .unwrap(),
                    _ => unreachable!("named reference was validated above"),
                };
                requirement.value =
                    i32::try_from(index).map_err(|_| "resolved rule index overflows i32")?;
            }
        }
    }
    Ok(catalog)
}

pub(crate) fn buildings() -> &'static BuildingCatalog {
    static CATALOG: OnceLock<BuildingCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_buildings(
            include_str!("../../data/rulesets/civcraft/content/buildings.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native buildings TOML: {error}"))
    })
}

pub(crate) fn building_rule(runtime_index: usize) -> Option<&'static NativeBuildingRule> {
    buildings()
        .rules
        .iter()
        .find(|rule| rule.legacy_runtime_index == runtime_index)
}

pub(crate) use super::policies::{
    DiplomacyCatalog, FaithDefinition, NegotiationPolicy, RelationshipPolicy, ReligionCatalog,
    StrategyCatalog, VassalageCatalog,
};

pub(crate) fn parse_technologies(source: &str) -> Result<TechnologyCatalog, String> {
    let catalog: TechnologyCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    super::technologies::validate(&catalog.technologies)?;
    Ok(catalog)
}

pub(crate) fn technologies() -> &'static TechnologyCatalog {
    static CATALOG: OnceLock<TechnologyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_technologies(include_str!(
            "../../data/rulesets/civcraft/content/technologies.toml"
        ))
        .unwrap_or_else(|error| panic!("invalid native technology catalog: {error}"))
    })
}

pub(crate) fn strategy() -> &'static StrategyCatalog {
    static CATALOG: OnceLock<StrategyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        super::policies::parse_strategy(include_str!(
            "../../data/rulesets/civcraft/content/strategy.toml"
        ))
        .unwrap_or_else(|error| panic!("invalid native strategy policy: {error}"))
    })
}

pub(crate) fn diplomacy() -> &'static DiplomacyCatalog {
    static CATALOG: OnceLock<DiplomacyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        super::policies::parse_diplomacy(
            include_str!("../../data/rulesets/civcraft/content/diplomacy.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native diplomacy policy: {error}"))
    })
}

pub(crate) fn vassalage() -> &'static VassalageCatalog {
    static CATALOG: OnceLock<VassalageCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        super::policies::parse_vassalage(
            include_str!("../../data/rulesets/civcraft/content/vassalage.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native vassalage policy: {error}"))
    })
}

pub(crate) fn religion() -> &'static ReligionCatalog {
    static CATALOG: OnceLock<ReligionCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        super::policies::parse_religion(
            include_str!("../../data/rulesets/civcraft/content/religion.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native religion rules: {error}"))
    })
}

/// Unified native content package validating every editable TOML file together.
pub(crate) struct ContentRuleset {
    pub(crate) units: UnitCatalog,
    pub(crate) civilizations: CivilizationCatalog,
    pub(crate) buildings: BuildingCatalog,
    pub(crate) technologies: TechnologyCatalog,
    pub(crate) governments: GovernmentCatalog,
    pub(crate) economy: EconomyCatalog,
    pub(crate) settlements: SettlementCatalog,
    pub(crate) experience: ExperienceCatalog,
    pub(crate) strategy: StrategyCatalog,
    pub(crate) diplomacy: DiplomacyCatalog,
    pub(crate) vassalage: VassalageCatalog,
    pub(crate) religion: ReligionCatalog,
}

impl ContentRuleset {
    pub(crate) fn load_from_directory(content_dir: &std::path::Path) -> Result<Self, String> {
        let read = |name: &str| {
            std::fs::read_to_string(content_dir.join(name))
                .map_err(|error| format!("failed to read {name}: {error}"))
        };
        let technologies = parse_technologies(&read("technologies.toml")?)?;
        let units = parse_units(&read("units.toml")?, &technologies)?;
        let civilizations = parse_civilizations(&read("civilizations.toml")?)?;
        let buildings = parse_buildings(&read("buildings.toml")?, &technologies)?;
        let governments = parse_governments(&read("governments.toml")?, &technologies)?;
        let economy: EconomyCatalog =
            toml::from_str(&read("economy.toml")?).map_err(|error| error.to_string())?;
        let settlements = parse_settlements(&read("settlements.toml")?, &units)?;
        let experience: ExperienceCatalog =
            toml::from_str(&read("experience_levels.toml")?).map_err(|error| error.to_string())?;
        let strategy = super::policies::parse_strategy(&read("strategy.toml")?)?;
        let diplomacy = super::policies::parse_diplomacy(&read("diplomacy.toml")?, &technologies)?;
        let vassalage = super::policies::parse_vassalage(&read("vassalage.toml")?, &technologies)?;
        let religion = super::policies::parse_religion(&read("religion.toml")?, &technologies)?;
        Ok(Self {
            units,
            civilizations,
            buildings,
            technologies,
            governments,
            economy,
            settlements,
            experience,
            strategy,
            diplomacy,
            vassalage,
            religion,
        })
    }
}

fn parse_governments(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<GovernmentCatalog, String> {
    let mut catalog: GovernmentCatalog =
        toml::from_str(source).map_err(|error| error.to_string())?;
    validate_ids(
        catalog.governments.iter().map(|g| g.government.as_str()),
        "government",
    )?;
    for government in &mut catalog.governments {
        if !government.required_technology.is_empty() {
            let index = super::technologies::runtime_index_in(
                technologies,
                &government.required_technology,
            )
            .ok_or_else(|| {
                format!(
                    "government {:?} requires unknown technology ID {:?}",
                    government.government, government.required_technology
                )
            })?;
            government.technology = Some(super::Tech(index));
        }
    }
    Ok(catalog)
}

pub(crate) fn governments() -> &'static GovernmentCatalog {
    static CATALOG: OnceLock<GovernmentCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_governments(
            include_str!("../../data/rulesets/civcraft/content/governments.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native government catalog: {error}"))
    })
}

pub(crate) fn economy() -> &'static EconomyCatalog {
    static CATALOG: OnceLock<EconomyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft/content/economy.toml"
        ))
        .expect("native economy TOML must be valid")
    })
}

pub(crate) fn parse_settlements(
    source: &str,
    units: &UnitCatalog,
) -> Result<SettlementCatalog, String> {
    let catalog: SettlementCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    catalog.villages.validate().map_err(str::to_owned)?;
    for id in [
        &catalog.barbarians.basic_unit,
        &catalog.barbarians.advanced_unit,
        &catalog.barbarians.sea_unit,
        &catalog.citizens.partisan_unit,
    ] {
        if !units.units.iter().any(|unit| unit.id == *id) {
            return Err(format!(
                "native settlement policy references unknown unit {id:?}"
            ));
        }
    }
    Ok(catalog)
}

pub(crate) fn settlements() -> &'static SettlementCatalog {
    static CATALOG: OnceLock<SettlementCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_settlements(
            include_str!("../../data/rulesets/civcraft/content/settlements.toml"),
            units(),
        )
        .unwrap_or_else(|error| panic!("invalid native settlement TOML: {error}"))
    })
}

pub(crate) fn experience() -> &'static ExperienceCatalog {
    static CATALOG: OnceLock<ExperienceCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft/content/experience_levels.toml"
        ))
        .expect("native experience TOML must be valid")
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn government_catalog_rejects_unknown_technology_ids() {
        let source = include_str!("../../data/rulesets/civcraft/content/governments.toml");
        let bad = source.replace("TECH_The_Republic", "TECH_The_Republiqq");
        let error = super::parse_governments(&bad, super::technologies()).unwrap_err();
        assert!(error.contains("republic") && error.contains("TECH_The_Republiqq"));
    }

    #[test]
    fn parse_buildings_and_units_reject_unknown_technology_ids() {
        let buildings_src = include_str!("../../data/rulesets/civcraft/content/buildings.toml");
        let bad_buildings = buildings_src.replace("TECH_Flight", "TECH_Missing_Flight");
        let err = super::parse_buildings(&bad_buildings, super::technologies()).unwrap_err();
        assert!(err.contains("building_airport"));

        let units_src = include_str!("../../data/rulesets/civcraft/content/units.toml");
        let bad_units = units_src.replace("TECH_Bronze_Working", "TECH_Missing_Bronze");
        let err = super::parse_units(&bad_units, super::technologies()).unwrap_err();
        assert!(err.contains("unknown technology ID"));
    }

    use super::*;

    #[test]
    fn village_weights_preserve_original_rolls_and_reject_invalid_tables() {
        let villages = &settlements().villages;
        assert_eq!(villages.total_weight(), 14);
        let labels: Vec<_> = (0..14)
            .map(|roll| match villages.reward_at(roll) {
                NativeVillageReward::Gold { amount } => format!("gold:{amount}"),
                NativeVillageReward::Technology => "technology".into(),
                NativeVillageReward::Reinforcements => "reinforcements".into(),
                NativeVillageReward::Barbarians => "barbarians".into(),
                NativeVillageReward::Settlement => "settlement".into(),
                NativeVillageReward::Map => "map".into(),
            })
            .collect();
        assert_eq!(
            labels,
            [
                "gold:25",
                "gold:50",
                "gold:50",
                "gold:50",
                "gold:100",
                "technology",
                "technology",
                "technology",
                "reinforcements",
                "reinforcements",
                "barbarians",
                "settlement",
                "map",
                "map"
            ]
        );
        for weights in [vec![], vec![0], vec![-1], vec![i32::MAX, 1]] {
            let invalid = NativeVillageRules {
                barbarian_onset_turn: 60,
                map_reveal_radius_squared: 30,
                outcomes: weights
                    .into_iter()
                    .map(|weight| NativeVillageOutcome {
                        weight,
                        reward: NativeVillageReward::Map,
                    })
                    .collect(),
            };
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn native_catalogs_have_expected_schema_and_campaign_coverage() {
        assert_eq!(units().schema_version, 1);
        assert_eq!(units().source, "Civilization III: Conquests");
        assert_eq!(units().units.len(), 92);
        assert_eq!(civilizations().schema_version, 1);
        assert_eq!(civilizations().source, "Civilization III: Conquests");
        assert_eq!(civilizations().civilizations.len(), 31);
        assert_eq!(buildings().schema_version, 1);
        assert_eq!(buildings().source, "Civilization III: Conquests");
        assert_eq!(buildings().rules.len(), 91);
        assert_eq!(technologies().schema_version, 1);
        assert_eq!(technologies().source, "Civilization III: Conquests");
        assert_eq!(technologies().technologies.len(), 85);
        assert_eq!(governments().schema_version, 1);
        assert_eq!(governments().governments.len(), 8);
        assert!(governments().governments.iter().all(|government| {
            !government.government.is_empty() && !government.name.is_empty()
        }));
        assert_eq!(
            governments()
                .governments
                .iter()
                .map(|government| government.government.as_str())
                .collect::<Vec<_>>(),
            [
                "anarchy",
                "despotism",
                "monarchy",
                "communism",
                "republic",
                "democracy",
                "fascism",
                "feudalism"
            ]
        );
        assert_eq!(
            governments().get("monarchy").unwrap().required_technology,
            "TECH_Monarchy"
        );
        assert_eq!(
            governments().get("republic").unwrap().required_technology,
            "TECH_The_Republic"
        );
        assert_eq!(economy().schema_version, 1);
        assert_eq!(economy().resources.len(), 20);
        assert_eq!(settlements().schema_version, 1);
        assert_eq!(experience().schema_version, 1);
        assert_eq!(experience().levels.len(), 4);
        assert_eq!(
            experience()
                .levels
                .iter()
                .map(|level| (
                    level.name.as_str(),
                    level.base_hit_points,
                    level.retreat_bonus_percent,
                    level.promotion_chance_per_mille,
                    level.default_rank,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("Conscript", 2, 34, 500, false),
                ("Regular", 3, 50, 250, true),
                ("Veteran", 4, 58, 125, false),
                ("Elite", 5, 66, 0, false),
            ]
        );
    }

    #[test]
    fn native_civilization_ids_and_starts_are_unique_and_resolved() {
        let catalog = civilizations();
        let ids = catalog
            .civilizations
            .iter()
            .map(|civilization| civilization.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), catalog.civilizations.len());
        for civilization in &catalog.civilizations {
            assert_eq!(civilization.starting_techs.len(), 2);
            assert_eq!(civilization.starting_units.len(), 2);
            assert!(
                civilization
                    .starting_units
                    .iter()
                    .all(|stack| stack.count > 0)
            );
        }
    }

    #[test]
    fn selected_obsolescence_policy_preserves_pre_removal_behavior() {
        let expected: Vec<String> = serde_json::from_str(include_str!(
            "../../tests/fixtures/building_obsolescence_policy_v1.json"
        ))
        .unwrap();
        let catalog = buildings();
        let actual: Vec<_> = catalog
            .rules
            .iter()
            .enumerate()
            .filter(|(index, rule)| {
                assert_eq!(*index, rule.legacy_runtime_index);
                rule.selected_obsolescence
            })
            .map(|(_, rule)| rule.id.as_str())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn native_technologies_have_resolved_prerequisite_lists() {
        let catalog = technologies();
        assert!(catalog.technologies.iter().all(|technology| {
            !technology.id.is_empty()
                && !technology.name.is_empty()
                && technology.prerequisites.len() <= 4
                && technology
                    .prerequisites
                    .iter()
                    .all(|prerequisite| !prerequisite.is_empty())
        }));
    }

    #[test]
    fn native_units_have_art_variants() {
        assert!(units().units.iter().all(|unit| {
            !unit.id.is_empty() && !unit.art_folder.is_empty() && !unit.art_root.is_empty()
        }));
    }

    #[test]
    fn unified_content_ruleset_loads_all_campaign_toml_files_from_directory() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/rulesets/civcraft/content");
        let loaded = ContentRuleset::load_from_directory(&dir)
            .expect("all 12 campaign content TOML files must load and validate");
        assert_eq!(loaded.units.units.len(), units().units.len());
        assert_eq!(loaded.buildings.rules.len(), buildings().rules.len());
        assert_eq!(
            loaded.technologies.technologies.len(),
            technologies().technologies.len()
        );
        assert_eq!(loaded.religion.faiths.len(), religion().faiths.len());
        assert_eq!(loaded.strategy.traits.len(), strategy().traits.len());
        assert_eq!(
            loaded.diplomacy.interests.len(),
            diplomacy().interests.len()
        );
        assert_eq!(
            loaded.vassalage.required_technology,
            vassalage().required_technology
        );
    }
}
