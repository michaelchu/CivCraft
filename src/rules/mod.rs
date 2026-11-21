//! Static ruleset catalogs used by map, simulation, and client systems.
//!
//! Buildings and technologies are represented by compact identifiers with
//! associated catalog data, requirements, and display names. Civilization
//! data and city artwork are kept in their own modules. These catalogs describe
//! available rules; systems under `sim` decide when and how those rules apply.
//!
//! Catalog ordering can be observable in saves, UI lists, AI tie-breaking, and
//! reference parity. Prefer stable identifiers and explicit order when adding
//! or revising entries.

mod buildings;
pub(crate) use buildings::{Building, Genus, Requirement};

mod research;
pub(crate) use research::{Tech, TechEra};

pub(crate) mod civilizations;
pub(crate) mod content_adapter;
pub(crate) mod native;

// The shared model keeps engine mechanics data separate from Civ III source
// content; simulation adapters bind source identities to stable wrappers.
#[allow(dead_code)]
pub(crate) mod engine_rules;
mod section_file;

pub(crate) mod policies;
pub(crate) mod technologies;
pub(crate) mod terrain_catalog;
pub(crate) mod unit_capabilities;

/// Complete campaign ruleset pairing editable native TOML catalogs (`content/`)
/// with internal Freeciv engine mechanics (`engine/`).
#[allow(dead_code)]
pub(crate) struct Ruleset {
    pub(crate) content: native::ContentRuleset,
    pub(crate) engine: engine_rules::RulesetData,
}

#[allow(dead_code)]
impl Ruleset {
    pub(crate) fn load_from_directory(
        data_root: &std::path::Path,
        package_dir: &std::path::Path,
        name: &str,
    ) -> Result<Self, String> {
        let content = native::ContentRuleset::load_from_directory(&package_dir.join("content"))?;
        let engine = engine_rules::RulesetData::load_from_directory(
            data_root,
            &package_dir.join("engine"),
            name,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { content, engine })
    }

    pub(crate) fn load_civcraft(data_root: &std::path::Path) -> Result<Self, String> {
        Self::load_from_directory(data_root, &data_root.join("civcraft"), "civcraft")
    }
}

#[cfg(test)]
mod tests {
    use super::Ruleset;

    #[test]
    fn loads_full_civcraft_ruleset_package_from_directory() {
        let data_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let ruleset = Ruleset::load_civcraft(&data_root)
            .expect("full civcraft ruleset package (content + engine) must load");
        assert!(!ruleset.content.buildings.rules.is_empty());
        assert!(!ruleset.content.technologies.technologies.is_empty());
        assert!(!ruleset.engine.effects.is_empty());
    }
}
