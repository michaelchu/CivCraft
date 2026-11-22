//! Unit catalog schema, validation and accessor.

use super::*;

#[allow(
    dead_code,
    reason = "schema_version and source are required for strict TOML decoding but not read at runtime"
)]
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
    pub(crate) category: UnitCategory,
    pub(crate) cost_shields: i32,
    #[serde(default)]
    pub(crate) population_cost: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
    pub(crate) moves: i32,
    #[serde(default)]
    pub(crate) bombard_strength: i32,
    #[serde(default)]
    pub(crate) bombard_range: i32,
    #[serde(default)]
    pub(crate) rate_of_fire: i32,
    #[serde(default)]
    pub(crate) operational_range: i32,
    #[serde(default)]
    pub(crate) capacity: usize,
    #[serde(default)]
    pub(crate) required_technology: String,
    #[serde(default)]
    pub(crate) required_resources: Vec<String>,
    #[serde(default)]
    pub(crate) upgrade_to: String,
    /// Optional extra roster limit for a non-unique unit, as civilization
    /// nouns. Unique units use `unique_civilization` instead.
    #[serde(default)]
    pub(crate) restricted_to: Vec<String>,
    #[serde(default)]
    pub(crate) unique_civilization: String,
    #[serde(default)]
    pub(crate) replaces: String,
    pub(crate) domain: UnitDomain,
    #[serde(default)]
    pub(crate) enslave_result: String,
    #[serde(default)]
    pub(crate) capabilities: Vec<String>,
    #[serde(default)]
    pub(crate) ai_roles: Vec<String>,
    #[serde(default)]
    pub(crate) hit_point_bonus: i32,
    #[serde(skip)]
    pub(crate) art_folder: String,
    #[serde(skip)]
    pub(crate) art_root: String,
    #[serde(default)]
    pub(crate) zone_of_control: i32,
    #[serde(skip)]
    pub(crate) bombard_effects: i32,
    #[serde(default = "default_require_support")]
    pub(crate) require_support: i32,
    #[serde(default)]
    pub(crate) worker_strength: f32,
    #[serde(default)]
    pub(crate) air_defense: i32,
}

fn default_require_support() -> i32 {
    1
}

/// How a unit relates to civilization rosters and wonder creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnitCategory {
    Standard,
    Unique,
    WonderCreated,
}

impl UnitCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Unique => "unique",
            Self::WonderCreated => "wonder_created",
        }
    }
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
    civilizations: &CivilizationCatalog,
) -> Result<UnitCatalog, String> {
    let mut catalog: UnitCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    for unit in &mut catalog.units {
        (unit.art_folder, unit.art_root) = super::bindings::unit_art(&unit.id, &unit.name);
        unit.bombard_effects = i32::from(super::bindings::has_bombard_effects(&unit.id));
    }
    validate_ids(catalog.units.iter().map(|unit| unit.id.as_str()), "unit")?;
    let unit_ids: std::collections::HashSet<&str> =
        catalog.units.iter().map(|unit| unit.id.as_str()).collect();
    for unit in &catalog.units {
        let nouns: Vec<&str> = civilizations
            .civilizations
            .iter()
            .map(|civilization| civilization.noun.as_str())
            .collect();
        for noun in unit
            .restricted_to
            .iter()
            .chain((!unit.unique_civilization.is_empty()).then_some(&unit.unique_civilization))
        {
            if !nouns.contains(&noun.as_str()) {
                return Err(format!("unknown civilization {noun} for {}", unit.id));
            }
        }
        if !unit.restricted_to.is_empty() && !unit.unique_civilization.is_empty() {
            return Err(format!(
                "{} cannot set both unique_civilization and restricted_to",
                unit.id
            ));
        }
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
            civilizations(),
        )
        .unwrap_or_else(|error| panic!("invalid native units TOML: {error}"))
    })
}
