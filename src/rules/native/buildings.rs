//! Building catalog schema, requirement resolution and accessors.

use super::*;

/// Resource names a building requirement may target, in resolved-index order.
const RESOURCE_TARGETS: [&str; 9] = [
    "Aluminum",
    "Coal",
    "Horses",
    "Iron",
    "Oil",
    "Rubber",
    "Saltpeter",
    "Uranium",
    "Ivory",
];

#[allow(
    dead_code,
    reason = "schema_version and source are required for strict TOML decoding but not read at runtime"
)]
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
    pub(crate) genus: crate::rules::Genus,
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

/// Requirement vocabulary accepted in `buildings.toml`; an unknown name is a
/// parse error rather than a later string mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub(crate) enum NativeRequirementKind {
    Tech,
    Improvement,
    River,
    Lake,
    Sea,
    Water,
    MinSize,
    OwnerCityCount,
    OwnerArmyCount,
    OwnerBuildingCount,
    Resource,
    OwnerEliteUnit,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeBuildingRequirement {
    pub(crate) target: Option<String>,
    pub(crate) kind: NativeRequirementKind,
    #[serde(default)]
    pub(crate) value: i32,
    #[serde(default)]
    #[serde(rename = "count", alias = "second_value")]
    pub(crate) second_value: i32,
    pub(crate) present: bool,
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
    let building_ids: std::collections::HashMap<String, usize> = catalog
        .rules
        .iter()
        .map(|rule| (rule.id.clone(), rule.legacy_runtime_index))
        .collect();
    // Validate each requirement and resolve its named target to a runtime
    // index once, for every consumer including advisors that inspect the
    // native requirement records directly rather than BuildingData.
    for rule in &mut catalog.rules {
        let rule_id = rule.id.clone();
        for requirement in rule.requirements.iter_mut().chain(&mut rule.obsolete) {
            resolve_requirement(&rule_id, requirement, technologies, &building_ids)?;
        }
    }
    Ok(catalog)
}

fn resolve_requirement(
    rule_id: &str,
    requirement: &mut NativeBuildingRequirement,
    technologies: &TechnologyCatalog,
    building_ids: &std::collections::HashMap<String, usize>,
) -> Result<(), String> {
    let target = requirement.target.as_deref();
    let resolved = match requirement.kind {
        NativeRequirementKind::Tech => Some(
            target
                .and_then(|id| crate::rules::technologies::runtime_index_in(technologies, id))
                .ok_or_else(|| format!("unknown technology target in {rule_id}"))?,
        ),
        NativeRequirementKind::Improvement | NativeRequirementKind::OwnerBuildingCount => Some(
            target
                .and_then(|id| building_ids.get(id).copied())
                .ok_or_else(|| format!("unknown building target in {rule_id}"))?,
        ),
        NativeRequirementKind::Resource => Some(
            target
                .and_then(|id| RESOURCE_TARGETS.iter().position(|name| *name == id))
                .ok_or_else(|| format!("unknown resource target in {rule_id}"))?,
        ),
        NativeRequirementKind::River
        | NativeRequirementKind::Lake
        | NativeRequirementKind::Sea
        | NativeRequirementKind::Water
        | NativeRequirementKind::MinSize
        | NativeRequirementKind::OwnerCityCount
        | NativeRequirementKind::OwnerEliteUnit
        | NativeRequirementKind::OwnerArmyCount => {
            if target.is_some() {
                return Err(format!("unexpected named target in {rule_id}"));
            }
            None
        }
    };
    if requirement.value < 0 || requirement.second_value < 0 {
        return Err(format!("negative building requirement in {rule_id}"));
    }
    if let Some(index) = resolved {
        requirement.value =
            i32::try_from(index).map_err(|_| "resolved rule index overflows i32")?;
    }
    Ok(())
}

pub(crate) fn buildings() -> &'static BuildingCatalog {
    static CATALOG: OnceLock<BuildingCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_buildings(
            include_str!("../../../data/rulesets/civcraft/content/buildings.toml"),
            technologies(),
        )
        .unwrap_or_else(|error| panic!("invalid native buildings TOML: {error}"))
    })
}

pub(crate) fn building_rule(runtime_index: usize) -> Option<&'static NativeBuildingRule> {
    // `parse_buildings` assigns `legacy_runtime_index` from the rule's position.
    buildings().rules.get(runtime_index)
}
