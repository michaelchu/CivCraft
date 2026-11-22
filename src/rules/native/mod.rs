//! Native TOML content for the CivCraft campaign.
//!
//! Catalogs decode directly into native definitions. Import records and BIQ
//! provenance are not required to construct or validate campaign rules.
//!
//! Each catalog lives in its own submodule with its schema, validation and
//! cached accessor; this module re-exports them so callers keep one path.

#![allow(dead_code)]

use serde::Deserialize;
use std::sync::OnceLock;

mod buildings;
mod civilizations;
mod economy;
mod experience;
mod governments;
mod policies;
mod settlements;
mod technologies;
mod units;

pub(crate) use buildings::*;
pub(crate) use civilizations::*;
pub(crate) use economy::*;
pub(crate) use experience::*;
pub(crate) use governments::*;
pub(crate) use policies::*;
pub(crate) use settlements::*;
pub(crate) use technologies::*;
pub(crate) use units::*;

fn validate_ids<'a>(ids: impl Iterator<Item = &'a str>, kind: &str) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        if id.is_empty() || !seen.insert(id) {
            return Err(format!("empty or duplicate native {kind} ID {id:?}"));
        }
    }
    Ok(())
}

/// Unified native content package validating every editable TOML file together.
pub(crate) struct ContentRuleset {
    pub(crate) units: UnitCatalog,
    pub(crate) civilizations: CivilizationCatalog,
    pub(crate) buildings: BuildingCatalog,
    pub(crate) technologies: TechnologyCatalog,
    pub(crate) experience: ExperienceCatalog,
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
        let experience: ExperienceCatalog =
            toml::from_str(&read("experience_levels.toml")?).map_err(|error| error.to_string())?;
        let religion = super::policies::parse_religion(&read("religion.toml")?, &technologies)?;
        Ok(Self {
            units,
            civilizations,
            buildings,
            technologies,
            experience,
            religion,
        })
    }
}

#[cfg(test)]
mod tests;
