//! Experience level catalog.

use super::*;

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

pub(crate) fn experience() -> &'static ExperienceCatalog {
    static CATALOG: OnceLock<ExperienceCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../../data/rulesets/civcraft/content/experience_levels.toml"
        ))
        .expect("native experience TOML must be valid")
    })
}
