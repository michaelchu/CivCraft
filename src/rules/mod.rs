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

// The shared model keeps Freeciv mechanics data separate from Civ III source
// content; simulation adapters bind source identities to stable wrappers.
#[allow(dead_code)]
pub(crate) mod freeciv;
mod freeciv_ini;
