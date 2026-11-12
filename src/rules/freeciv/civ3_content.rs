//! Civ III source content attached to a Freeciv ruleset without changing its
//! stable runtime wrapper IDs.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Clone)]
struct Table {
    path: PathBuf,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn read(path: &Path) -> Result<Self, CatalogLoadError> {
        let text = fs::read_to_string(path).map_err(|error| CatalogLoadError {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
        let mut lines = text.lines();
        let headers = lines
            .next()
            .ok_or_else(|| CatalogLoadError {
                path: path.to_owned(),
                message: "catalog is empty".to_owned(),
            })?
            .split('\t')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if headers.is_empty() || headers.iter().any(String::is_empty) {
            return Err(Self::error(path, "catalog header contains an empty field"));
        }
        let mut unique = HashSet::new();
        if headers.iter().any(|name| !unique.insert(name.as_str())) {
            return Err(Self::error(
                path,
                "catalog header contains duplicate fields",
            ));
        }
        let mut rows = Vec::new();
        for (line_index, line) in lines.enumerate() {
            let values = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            if values.len() != headers.len() {
                return Err(Self::error(
                    path,
                    &format!(
                        "row {} has {} fields; expected {}",
                        line_index + 2,
                        values.len(),
                        headers.len()
                    ),
                ));
            }
            rows.push(values);
        }
        Ok(Self {
            path: path.to_owned(),
            headers,
            rows,
        })
    }

    fn error(path: &Path, message: &str) -> CatalogLoadError {
        CatalogLoadError {
            path: path.to_owned(),
            message: message.to_owned(),
        }
    }

    fn required(&self, name: &str) -> Result<usize, CatalogLoadError> {
        self.headers
            .iter()
            .position(|field| field == name)
            .ok_or_else(|| Self::error(&self.path, &format!("missing required field {name:?}")))
    }

    fn values<'a>(&self, row: &'a [String], name: &str) -> Result<&'a str, CatalogLoadError> {
        let index = self.required(name)?;
        Ok(&row[index])
    }

    fn source_identity(
        &self,
        row: &[String],
        field: &str,
        extra_indices: Option<&str>,
    ) -> Result<SourceRecordIdentity, CatalogLoadError> {
        let value = self.values(row, field)?;
        let (path_and_key, sha256) = value.split_once('@').ok_or_else(|| {
            Self::error(&self.path, &format!("invalid {field} provenance {value:?}"))
        })?;
        let (path, key) = path_and_key.split_once('#').ok_or_else(|| {
            Self::error(&self.path, &format!("invalid {field} provenance {value:?}"))
        })?;
        let (section, bracketed) = key.split_once('[').ok_or_else(|| {
            Self::error(&self.path, &format!("invalid source section key {key:?}"))
        })?;
        let end = bracketed
            .find(']')
            .ok_or_else(|| Self::error(&self.path, &format!("invalid source index key {key:?}")))?;
        let indices_text = extra_indices.unwrap_or(&bracketed[..end]);
        let mut record_indices = Vec::new();
        for token in indices_text.split(';').filter(|token| !token.is_empty()) {
            let index = token.parse::<usize>().map_err(|error| {
                Self::error(
                    &self.path,
                    &format!("invalid source record index {token:?}: {error}"),
                )
            })?;
            if !record_indices.contains(&index) {
                record_indices.push(index);
            }
        }
        if section.is_empty() || record_indices.is_empty() || sha256.len() != 64 {
            return Err(Self::error(
                &self.path,
                &format!("incomplete source provenance {value:?}"),
            ));
        }
        Ok(SourceRecordIdentity {
            path: path.to_owned(),
            section: section.to_owned(),
            key: key.to_owned(),
            record_indices,
            sha256: sha256.to_owned(),
        })
    }

    fn records(
        &self,
        kind: ContentKind,
        source_field: &str,
        source_index_field: Option<&str>,
    ) -> Result<Vec<SourceContentRecord>, CatalogLoadError> {
        let id_field = self.required("id")?;
        let name_field = self.required("name")?;
        let extra_indices_field = source_index_field
            .map(|field| self.required(field))
            .transpose()?;
        let mut ids = HashSet::new();
        self.rows
            .iter()
            .map(|row| {
                let catalog_id = row[id_field].clone();
                let display_name = row[name_field].clone();
                if catalog_id.is_empty() || display_name.is_empty() {
                    return Err(Self::error(
                        &self.path,
                        "source record has an empty id or name",
                    ));
                }
                if !ids.insert(catalog_id.clone()) {
                    return Err(Self::error(
                        &self.path,
                        &format!("duplicate catalog id {catalog_id:?}"),
                    ));
                }
                let extra_indices = extra_indices_field.map(|index| row[index].as_str());
                let source = self.source_identity(row, source_field, extra_indices)?;
                let fields = self
                    .headers
                    .iter()
                    .zip(row)
                    .map(|(name, value)| SourceField {
                        name: name.clone(),
                        value: value.clone(),
                    })
                    .collect();
                Ok(SourceContentRecord {
                    kind,
                    catalog_id,
                    display_name,
                    source,
                    fields,
                })
            })
            .collect()
    }
}

impl Civ3ContentCatalog {
    pub(crate) fn load(directory: &Path) -> Result<Self, CatalogLoadError> {
        let unit_table = Table::read(&directory.join("units.tsv"))?;
        let building_table = Table::read(&directory.join("buildings.tsv"))?;
        let building_bindings = Table::read(&directory.join("building_catalog_bindings.tsv"))?;
        let technology_table = Table::read(&directory.join("technologies.tsv"))?;
        let civilization_table = Table::read(&directory.join("civilizations.tsv"))?;
        let civilization_starts = Table::read(&directory.join("civilization_starts.tsv"))?;

        let mut records = Vec::new();
        records.extend(unit_table.records(
            ContentKind::Unit,
            "source_biq",
            Some("source_prototypes"),
        )?);
        let mut building_records = building_table.records(
            ContentKind::Building,
            "source_biq",
            Some("source_record_index"),
        )?;
        attach_building_bindings(&mut building_records, &building_bindings)?;
        records.extend(building_records);
        records.extend(technology_table.records(
            ContentKind::Technology,
            "source_biq",
            Some("source_record_index"),
        )?);

        let mut start_sources = std::collections::HashMap::new();
        for row in &civilization_starts.rows {
            let source = civilization_starts.source_identity(row, "source_biq", None)?;
            if source.section != "RACE" {
                return Err(Table::error(
                    &civilization_starts.path,
                    "civilization start provenance does not name a RACE record",
                ));
            }
            let index = source.record_indices[0];
            if start_sources.insert(index, row).is_some() {
                return Err(Table::error(
                    &civilization_starts.path,
                    &format!("duplicate RACE start profile at index {index}"),
                ));
            }
        }
        let civilization_id = civilization_table.required("id")?;
        let civilization_name = civilization_table.required("name")?;
        let race_index = civilization_table.required("source_race_index")?;
        for row in &civilization_table.rows {
            let id = &row[civilization_id];
            let index = row[race_index].parse::<usize>().map_err(|error| {
                Table::error(
                    &civilization_table.path,
                    &format!("invalid source RACE index for {id:?}: {error}"),
                )
            })?;
            let start = start_sources.get(&index).ok_or_else(|| {
                Table::error(
                    &civilization_table.path,
                    &format!(
                        "missing source start profile at RACE[{index}] for civilization {:?} ({id:?})",
                        row[civilization_name],
                    ),
                )
            })?;
            let source = civilization_starts.source_identity(start, "source_biq", None)?;
            if source.section != "RACE" || source.record_indices.first().copied() != Some(index) {
                return Err(Table::error(
                    &civilization_table.path,
                    &format!("RACE provenance does not match civilization {id:?}"),
                ));
            }
            let fields = civilization_table
                .headers
                .iter()
                .zip(row)
                .map(|(name, value)| SourceField {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect();
            records.push(SourceContentRecord {
                kind: ContentKind::Nation,
                catalog_id: id.clone(),
                display_name: row[civilization_name].clone(),
                source,
                fields,
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
                return Err(CatalogLoadError {
                    path: directory.to_owned(),
                    message: format!("expected {expected} {kind:?} source records, found {actual}"),
                });
            }
        }

        Ok(Self { records })
    }
}

fn attach_building_bindings(
    buildings: &mut [SourceContentRecord],
    bindings: &Table,
) -> Result<(), CatalogLoadError> {
    let binding_id = bindings.required("civ3_id")?;
    let binding_index = bindings.required("source_record_index")?;
    let runtime_index = bindings.required("legacy_runtime_index")?;
    let status = bindings.required("mapping_status")?;
    let note = bindings.required("mapping_note")?;
    let mut by_source_index = std::collections::HashMap::new();
    for row in &bindings.rows {
        let index = row[binding_index].parse::<usize>().map_err(|error| {
            Table::error(
                &bindings.path,
                &format!(
                    "invalid building source index {:?}: {error}",
                    row[binding_index]
                ),
            )
        })?;
        let runtime = if row[runtime_index].is_empty() {
            None
        } else {
            Some(row[runtime_index].parse::<usize>().map_err(|error| {
                Table::error(
                    &bindings.path,
                    &format!(
                        "invalid legacy Building index {:?}: {error}",
                        row[runtime_index]
                    ),
                )
            })?)
        };
        if row[status] != "mapped" && row[status] != "unmapped" {
            return Err(Table::error(
                &bindings.path,
                &format!("unknown building mapping status {:?}", row[status]),
            ));
        }
        if (row[status] == "mapped") != runtime.is_some() {
            return Err(Table::error(
                &bindings.path,
                &format!("mapping status/index disagree for {:?}", row[binding_id]),
            ));
        }
        if (row[status] == "unmapped") != !row[note].is_empty() {
            return Err(Table::error(
                &bindings.path,
                &format!(
                    "unmapped building {:?} needs an explicit note",
                    row[binding_id]
                ),
            ));
        }
        if by_source_index
            .insert(
                index,
                (
                    row[binding_id].clone(),
                    runtime.map_or_else(String::new, |value| value.to_string()),
                    row[status].clone(),
                    row[note].clone(),
                ),
            )
            .is_some()
        {
            return Err(Table::error(
                &bindings.path,
                &format!("duplicate building mapping for BLDG[{index}]"),
            ));
        }
    }
    if by_source_index.len() != buildings.len() {
        return Err(Table::error(
            &bindings.path,
            &format!(
                "building mapping covers {} records; expected {}",
                by_source_index.len(),
                buildings.len()
            ),
        ));
    }
    for building in buildings {
        let index = *building
            .source
            .record_indices
            .first()
            .ok_or_else(|| Table::error(&bindings.path, "building has no source BLDG index"))?;
        let (id, runtime, status, note) = by_source_index.get(&index).ok_or_else(|| {
            Table::error(
                &bindings.path,
                &format!("missing mapping row for BLDG[{index}]"),
            )
        })?;
        if building.catalog_id != *id {
            return Err(Table::error(
                &bindings.path,
                &format!(
                    "binding ID {id:?} does not match BLDG[{index}] {:?}",
                    building.catalog_id
                ),
            ));
        }
        building.fields.extend([
            SourceField {
                name: "legacy_runtime_index".to_owned(),
                value: runtime.clone(),
            },
            SourceField {
                name: "legacy_mapping_status".to_owned(),
                value: status.clone(),
            },
            SourceField {
                name: "legacy_mapping_note".to_owned(),
                value: note.clone(),
            },
        ]);
    }
    Ok(())
}
