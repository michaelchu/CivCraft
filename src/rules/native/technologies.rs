//! Technology catalog schema, validation and accessor.

use super::*;

#[allow(
    dead_code,
    reason = "schema_version and source are required for strict TOML decoding but not read at runtime"
)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TechnologyCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) technologies: Vec<NativeTechnology>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeTechnology {
    pub(crate) id: String,
    /// Stable serialized `Tech` index, bound in `bindings.rs` rather than the TOML.
    #[serde(skip)]
    pub(crate) runtime_index: usize,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    pub(crate) cost: i32,
    pub(crate) era: i32,
    #[serde(default)]
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

pub(crate) fn parse_technologies(source: &str) -> Result<TechnologyCatalog, String> {
    let mut catalog: TechnologyCatalog =
        toml::from_str(source).map_err(|error| error.to_string())?;
    for technology in &mut catalog.technologies {
        technology.runtime_index = super::bindings::technology_save_index(&technology.id)
            .ok_or_else(|| format!("technology {} has no saved index binding", technology.id))?;
    }
    crate::rules::technologies::validate(&catalog.technologies)?;
    Ok(catalog)
}

pub(crate) fn technologies() -> &'static TechnologyCatalog {
    static CATALOG: OnceLock<TechnologyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_technologies(include_str!(
            "../../../data/rulesets/civcraft/content/technologies.toml"
        ))
        .unwrap_or_else(|error| panic!("invalid native technology catalog: {error}"))
    })
}
