//! Unit catalog schema, validation and accessor.

use super::*;

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

pub(crate) fn parse_units(
    source: &str,
    technologies: &TechnologyCatalog,
) -> Result<UnitCatalog, String> {
    let catalog: UnitCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    validate_ids(catalog.units.iter().map(|unit| unit.id.as_str()), "unit")?;
    let unit_ids: std::collections::HashSet<&str> =
        catalog.units.iter().map(|unit| unit.id.as_str()).collect();
    for unit in &catalog.units {
        if !unit.required_technology.is_empty()
            && crate::rules::technologies::runtime_index_in(technologies, &unit.required_technology)
                .is_none()
        {
            return Err(format!("unknown technology ID for {}", unit.id));
        }
        for target in [&unit.upgrade_to, &unit.replaces, &unit.enslave_result] {
            if !target.is_empty() && !unit_ids.contains(target.as_str()) {
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
            if !crate::rules::unit_capabilities::CAPABILITIES.contains(&capability.as_str()) {
                return Err(format!("unknown unit capability {capability}"));
            }
        }
        for role in &unit.ai_roles {
            if !crate::rules::unit_capabilities::AI_ROLES.contains(&role.as_str()) {
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
            include_str!("../../../data/rulesets/civcraft/content/units.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native units TOML: {error}"))
    })
}
