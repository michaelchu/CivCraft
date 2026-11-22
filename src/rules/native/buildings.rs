//! Building catalog schema, requirement resolution and accessors.

use super::*;

/// Resource names a building requirement may target, in resolved-index order.
const RESOURCE_TARGETS: [&str; 8] = [
    "Aluminum",
    "Coal",
    "Horses",
    "Iron",
    "Oil",
    "Rubber",
    "Saltpeter",
    "Uranium",
];

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
    for rule in &catalog.rules {
        for requirement in rule.requirements.iter().chain(&rule.obsolete) {
            match requirement.kind.as_str() {
                "Tech" => {
                    if requirement
                        .target
                        .as_deref()
                        .and_then(|id| {
                            crate::rules::technologies::runtime_index_in(technologies, id)
                        })
                        .is_none()
                    {
                        return Err(format!("unknown technology target in {}", rule.id));
                    }
                }
                "Improvement" | "OwnerBuildingCount" => {
                    if !requirement
                        .target
                        .as_deref()
                        .is_some_and(|id| building_ids.contains_key(id))
                    {
                        return Err(format!("unknown building target in {}", rule.id));
                    }
                }
                "Resource" => {
                    if !requirement
                        .target
                        .as_deref()
                        .is_some_and(|id| RESOURCE_TARGETS.contains(&id))
                    {
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
    for rule in &mut catalog.rules {
        for requirement in rule.requirements.iter_mut().chain(&mut rule.obsolete) {
            if let Some(id) = requirement.target.as_deref() {
                let index = match requirement.kind.as_str() {
                    "Tech" => {
                        crate::rules::technologies::runtime_index_in(technologies, id).unwrap()
                    }
                    "Improvement" | "OwnerBuildingCount" => building_ids[id],
                    "Resource" => RESOURCE_TARGETS
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
            include_str!("../../../data/rulesets/civcraft/content/buildings.toml"),
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
