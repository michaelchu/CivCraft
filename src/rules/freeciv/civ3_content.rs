//! Civ III source content attached to a Freeciv ruleset without changing its
//! stable runtime wrapper IDs.

use std::path::PathBuf;

use super::ContentKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceContentRecord {
    pub(crate) kind: ContentKind,
    /// Stable Civ III catalog identity (the catalog's `id` column).
    pub(crate) catalog_id: String,
    pub(crate) display_name: String,
    pub(crate) source: SourceRecordIdentity,
    /// Original catalog fields are retained as strings. The ruleset adapter
    /// interprets them only when mapping a rule into gameplay.
    pub(crate) fields: Vec<SourceField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceRecordIdentity {
    pub(crate) path: String,
    pub(crate) section: String,
    pub(crate) key: String,
    pub(crate) record_indices: Vec<usize>,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceField {
    pub(crate) name: String,
    pub(crate) value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Civ3ContentCatalog {
    pub(crate) records: Vec<SourceContentRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogLoadError {
    path: PathBuf,
    message: String,
}

impl std::fmt::Display for CatalogLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for CatalogLoadError {}

impl Civ3ContentCatalog {
    pub(crate) fn load() -> Result<Self, CatalogLoadError> {
        Self::load_native()
    }

    fn load_native() -> Result<Self, CatalogLoadError> {
        let mut records = Vec::new();
        for unit in &crate::rules::native::units().units {
            records.push(SourceContentRecord {
                kind: ContentKind::Unit,
                catalog_id: unit.id.clone(),
                display_name: unit.name.clone(),
                source: native_source_identity(&unit.source_biq, &unit.source_prototypes)?,
                fields: vec![
                    native_field("id", &unit.id),
                    native_field("name", &unit.name),
                    native_integer_field("cost_shields", unit.cost_shields),
                    native_integer_field("attack", unit.attack),
                    native_integer_field("defense", unit.defense),
                    native_integer_field("moves", unit.moves),
                ],
            });
        }
        let building_catalog = crate::rules::native::buildings();
        for building in &building_catalog.buildings {
            let binding = building_catalog
                .bindings
                .iter()
                .find(|binding| binding.civ3_id == building.id)
                .ok_or_else(|| native_catalog_error("building binding is missing"))?;
            let mut fields = vec![
                native_field("id", &building.id),
                native_field("name", &building.name),
                native_integer_field("cost", building.cost),
                native_field("flags_hex", &building.flags_hex),
                native_field("legacy_mapping_status", &binding.mapping_status),
                native_field("legacy_mapping_note", &binding.mapping_note),
            ];
            if binding.legacy_runtime_index >= 0 {
                fields.push(native_integer_field(
                    "legacy_runtime_index",
                    binding.legacy_runtime_index,
                ));
            }
            records.push(SourceContentRecord {
                kind: ContentKind::Building,
                catalog_id: building.id.clone(),
                display_name: building.name.clone(),
                source: native_source_identity(
                    &building.source_biq,
                    &[building.source_record_index as i32],
                )?,
                fields,
            });
        }
        for technology in &crate::rules::native::technologies().technologies {
            records.push(SourceContentRecord {
                kind: ContentKind::Technology,
                catalog_id: technology.id.clone(),
                display_name: technology.name.clone(),
                source: native_source_identity(
                    &technology.source_biq,
                    &[technology.source_record_index as i32],
                )?,
                fields: vec![
                    native_field("id", &technology.id),
                    native_field("name", &technology.name),
                    native_integer_field("cost", technology.cost),
                    native_integer_field("prerequisite_4_index", technology.prerequisite_4_index),
                ],
            });
        }
        for civilization in &crate::rules::native::civilizations().civilizations {
            records.push(SourceContentRecord {
                kind: ContentKind::Nation,
                catalog_id: civilization.id.clone(),
                display_name: civilization.name.clone(),
                source: native_source_identity(
                    &civilization.source_biq,
                    &[civilization.source_race_index as i32],
                )?,
                fields: vec![
                    native_field("id", &civilization.id),
                    native_field("name", &civilization.name),
                    native_field("source_id", &civilization.source_id),
                ],
            });
        }
        for (kind, expected) in [
            (ContentKind::Unit, 92),
            (ContentKind::Building, 83),
            (ContentKind::Technology, 83),
            (ContentKind::Nation, 31),
        ] {
            let actual = records.iter().filter(|record| record.kind == kind).count();
            if actual != expected {
                return Err(native_catalog_error(&format!(
                    "expected {expected} {kind:?} records, found {actual}"
                )));
            }
        }
        Ok(Self { records })
    }
}

fn native_catalog_error(message: &str) -> CatalogLoadError {
    CatalogLoadError {
        path: PathBuf::from("data/rulesets/civcraft/content"),
        message: message.to_owned(),
    }
}

fn native_field(name: &str, value: &str) -> SourceField {
    SourceField {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn native_integer_field(name: &str, value: i32) -> SourceField {
    native_field(name, &value.to_string())
}

fn native_source_identity(
    value: &str,
    source_indices: &[i32],
) -> Result<SourceRecordIdentity, CatalogLoadError> {
    let (path_and_key, sha256) = value
        .split_once('@')
        .ok_or_else(|| native_catalog_error(&format!("invalid source provenance {value:?}")))?;
    let (path, key) = path_and_key
        .split_once('#')
        .ok_or_else(|| native_catalog_error(&format!("invalid source key {path_and_key:?}")))?;
    let (section, bracketed) = key
        .split_once('[')
        .ok_or_else(|| native_catalog_error(&format!("invalid source record key {key:?}")))?;
    let end = bracketed
        .find(']')
        .ok_or_else(|| native_catalog_error(&format!("invalid source record key {key:?}")))?;
    let indices = if source_indices.is_empty() {
        vec![bracketed[..end].parse::<usize>().map_err(|error| {
            native_catalog_error(&format!("invalid source record index: {error}"))
        })?]
    } else {
        source_indices
            .iter()
            .map(|index| {
                usize::try_from(*index).map_err(|error| {
                    native_catalog_error(&format!("invalid source index: {error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if section.is_empty() || indices.is_empty() || sha256.len() != 64 {
        return Err(native_catalog_error("incomplete source provenance"));
    }
    Ok(SourceRecordIdentity {
        path: path.to_owned(),
        section: section.to_owned(),
        key: key.to_owned(),
        record_indices: indices,
        sha256: sha256.to_owned(),
    })
}
