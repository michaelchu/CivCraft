//! Experience level catalog.

use super::*;

#[allow(
    dead_code,
    reason = "schema_version and source are required for strict TOML decoding but not read at runtime"
)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExperienceCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) levels: Vec<NativeExperienceLevel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeExperienceLevel {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_hit_points: i32,
    pub(crate) retreat_bonus_percent: i32,
    pub(crate) promotion_chance_per_mille: i32,
    pub(crate) default_rank: bool,
}

pub(crate) fn parse_experience(source: &str) -> Result<ExperienceCatalog, String> {
    let catalog: ExperienceCatalog = toml::from_str(source).map_err(|error| error.to_string())?;
    validate_ids(
        catalog.levels.iter().map(|level| level.id.as_str()),
        "experience level",
    )?;
    Ok(catalog)
}

pub(crate) fn experience() -> &'static ExperienceCatalog {
    static CATALOG: OnceLock<ExperienceCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_experience(include_str!(
            "../../../data/rulesets/civcraft/content/experience_levels.toml"
        ))
        .unwrap_or_else(|error| panic!("invalid native experience TOML: {error}"))
    })
}
