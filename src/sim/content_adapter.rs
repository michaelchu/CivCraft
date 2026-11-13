//! Explicit source-to-runtime identity bindings for the Civ III campaign
//! catalogs. These bindings keep existing serialized wrappers stable while
//! the Freeciv ruleset model is connected to gameplay one subsystem at a time.

use crate::rules::freeciv::{ContentKind, RulesetData, SourceContentRecord};
use crate::rules::{Building, Tech};
use crate::sim::UnitKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StableContentId {
    Unit(UnitKind),
    Building(Building),
    Technology(Tech),
    Civilization(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContentBinding {
    pub(crate) kind: ContentKind,
    pub(crate) source_id: String,
    pub(crate) source_indices: Vec<usize>,
    pub(crate) runtime_id: Option<StableContentId>,
    /// Empty for mapped rows. Missing legacy bindings carry a concrete reason.
    pub(crate) mapping_note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Civ3ContentBindings {
    pub(crate) records: Vec<ContentBinding>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FreecivTechBinding {
    Bound(Tech),
    MissingFromCiv3Catalog,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContentBindingError(String);

impl std::fmt::Display for ContentBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ContentBindingError {}

impl Civ3ContentBindings {
    pub(crate) fn build(ruleset: &RulesetData) -> Result<Self, ContentBindingError> {
        let mut records = Vec::with_capacity(ruleset.source_content.len());
        for source in &ruleset.source_content {
            records.push(bind_record(source)?);
        }
        for (kind, expected) in [
            (ContentKind::Unit, 92),
            (ContentKind::Building, 83),
            (ContentKind::Technology, 83),
            (ContentKind::Nation, 31),
        ] {
            let actual = records.iter().filter(|record| record.kind == kind).count();
            if actual != expected {
                return Err(ContentBindingError(format!(
                    "expected {expected} {kind:?} bindings, found {actual}"
                )));
            }
        }
        ensure_unique_runtime_ids(&records)?;
        Ok(Self { records })
    }

    pub(crate) fn count(&self, kind: ContentKind, mapped: bool) -> usize {
        self.records
            .iter()
            .filter(|record| record.kind == kind && record.runtime_id.is_some() == mapped)
            .count()
    }

    pub(crate) fn by_source_id(&self, kind: ContentKind, id: &str) -> Option<&ContentBinding> {
        self.records
            .iter()
            .find(|record| record.kind == kind && record.source_id == id)
    }
}

/// Resolve a Freeciv technology universal through the selected ruleset and
/// the explicit Civ III source catalog. A missing source counterpart is only
/// accepted for the documented Freeciv-only `Plastics` advance.
pub(crate) fn bind_freeciv_technology(
    ruleset: &RulesetData,
    freeciv_name: &str,
) -> Result<FreecivTechBinding, ContentBindingError> {
    let freeciv_record = ruleset
        .content
        .iter()
        .find(|record| {
            record.kind == ContentKind::Technology
                && (record.rule_name.eq_ignore_ascii_case(freeciv_name)
                    || record
                        .display_name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(freeciv_name)))
        })
        .ok_or_else(|| {
            ContentBindingError(format!(
                "Freeciv ruleset has no technology named {freeciv_name:?}"
            ))
        })?;
    let display_name = freeciv_record
        .display_name
        .as_deref()
        .unwrap_or(&freeciv_record.rule_name);
    if display_name.eq_ignore_ascii_case("Plastics") {
        return Ok(FreecivTechBinding::MissingFromCiv3Catalog);
    }

    let source = ruleset
        .source_content
        .iter()
        .find(|source| {
            source.kind == ContentKind::Technology
                && (source.display_name.eq_ignore_ascii_case(display_name)
                    || (display_name.eq_ignore_ascii_case("Superconductors")
                        && source.display_name.eq_ignore_ascii_case("Superconductor"))
                    || (display_name.eq_ignore_ascii_case("Amphibious Warfare")
                        && source.display_name.eq_ignore_ascii_case("Amphibious War"))
                    // Freeciv calls the draft-related advance Conscription;
                    // the Civ III catalog names that advance Nationalism.
                    || (display_name.eq_ignore_ascii_case("Conscription")
                        && source.display_name.eq_ignore_ascii_case("Nationalism"))
                    || (display_name.eq_ignore_ascii_case("Environmentalism")
                        && source.display_name.eq_ignore_ascii_case("Ecology"))
                    || (display_name.eq_ignore_ascii_case("Laser")
                        && source.display_name.eq_ignore_ascii_case("The Laser"))
                    || (display_name.eq_ignore_ascii_case("Mobile Warfare")
                        && source.display_name.eq_ignore_ascii_case("Motorized Transportation"))
                    || (display_name.eq_ignore_ascii_case("Nuclear Fission")
                        && source.display_name.eq_ignore_ascii_case("Fission")))
        })
        .ok_or_else(|| {
            ContentBindingError(format!(
                "Freeciv technology {display_name:?} has no Civ III source mapping"
            ))
        })?;
    let (runtime_id, _) = bind_technology(source)?;
    match runtime_id {
        Some(StableContentId::Technology(tech)) => Ok(FreecivTechBinding::Bound(tech)),
        None => Ok(FreecivTechBinding::MissingFromCiv3Catalog),
        Some(other) => Err(ContentBindingError(format!(
            "Freeciv technology {display_name:?} resolved to non-technology identity {other:?}"
        ))),
    }
}

fn bind_record(source: &SourceContentRecord) -> Result<ContentBinding, ContentBindingError> {
    let (runtime_id, mapping_note) = match source.kind {
        ContentKind::Unit => {
            let kind = UnitKind::from_conquests_id(&source.catalog_id).ok_or_else(|| {
                ContentBindingError(format!(
                    "Civ III unit {:?} has no stable UnitKind identity",
                    source.catalog_id
                ))
            })?;
            (Some(StableContentId::Unit(kind)), String::new())
        }
        ContentKind::Building => bind_building(source)?,
        ContentKind::Technology => bind_technology(source)?,
        ContentKind::Nation => {
            if crate::rules::civilizations::by_id(&source.catalog_id).is_none() {
                return Err(ContentBindingError(format!(
                    "Civ III civilization {:?} has no source-backed roster identity",
                    source.catalog_id
                )));
            }
            (
                Some(StableContentId::Civilization(source.catalog_id.clone())),
                String::new(),
            )
        }
        other => {
            return Err(ContentBindingError(format!(
                "unexpected Civ III source content kind {other:?}"
            )));
        }
    };
    Ok(ContentBinding {
        kind: source.kind,
        source_id: source.catalog_id.clone(),
        source_indices: source.source.record_indices.clone(),
        runtime_id,
        mapping_note,
    })
}

fn bind_building(
    source: &SourceContentRecord,
) -> Result<(Option<StableContentId>, String), ContentBindingError> {
    let field = |name: &str| {
        source
            .fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.value.as_str())
    };
    let status = field("legacy_mapping_status").ok_or_else(|| {
        ContentBindingError(format!(
            "Civ III building {:?} has no explicit legacy mapping status",
            source.catalog_id
        ))
    })?;
    let note = field("legacy_mapping_note").unwrap_or_default();
    let index = field("legacy_runtime_index").unwrap_or_default();
    match status {
        "mapped" => {
            let index = index.parse::<usize>().map_err(|error| {
                ContentBindingError(format!(
                    "invalid legacy Building index for {:?}: {error}",
                    source.catalog_id
                ))
            })?;
            if index >= Building::all().count() {
                return Err(ContentBindingError(format!(
                    "legacy Building index {index} for {:?} is out of range",
                    source.catalog_id
                )));
            }
            if !note.is_empty() {
                return Err(ContentBindingError(format!(
                    "mapped Civ III building {:?} has a mapping note",
                    source.catalog_id
                )));
            }
            Ok((
                Some(StableContentId::Building(Building(index))),
                String::new(),
            ))
        }
        "unmapped" if index.is_empty() && !note.is_empty() => Ok((None, note.to_owned())),
        _ => Err(ContentBindingError(format!(
            "invalid legacy Building mapping for {:?}: status={status:?}, index={index:?}, note={note:?}",
            source.catalog_id
        ))),
    }
}

fn bind_technology(
    source: &SourceContentRecord,
) -> Result<(Option<StableContentId>, String), ContentBindingError> {
    let runtime = Tech::all().find(|tech| tech.name().eq_ignore_ascii_case(&source.display_name));
    if let Some(tech) = runtime {
        return Ok((Some(StableContentId::Technology(tech)), String::new()));
    }
    if source.display_name.eq_ignore_ascii_case("Amphibious War") {
        return Ok((
            Some(StableContentId::Technology(Tech::AMPHIBIOUS_WARFARE)),
            "Civ III BIQ uses the shorter name; this source identity binds to the existing Amphibious Warfare save ID.".to_owned(),
        ));
    }
    Err(ContentBindingError(format!(
        "Civ III technology {:?} has no exact or explicit stable Tech mapping",
        source.display_name
    )))
}

fn ensure_unique_runtime_ids(records: &[ContentBinding]) -> Result<(), ContentBindingError> {
    let mut ids = std::collections::HashSet::new();
    for record in records.iter().filter(|record| record.runtime_id.is_some()) {
        let key = match record.runtime_id.as_ref().unwrap() {
            StableContentId::Unit(kind) => format!("Unit:{kind:?}"),
            StableContentId::Building(building) => format!("Building:{}", building.0),
            StableContentId::Technology(tech) => format!("Technology:{}", tech.0),
            StableContentId::Civilization(id) => format!("Civilization:{id}"),
        };
        if !ids.insert(key.clone()) {
            return Err(ContentBindingError(format!(
                "multiple Civ III source records map to stable runtime identity {key}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn civ3_source_content_binds_to_stable_ids_and_reports_catalog_gaps() {
        let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let rules = RulesetData::load_with_civ3_catalog(
            &data_root,
            "civcraft_civ2civ3",
            &data_root.join("civ3_conquests"),
        )
        .unwrap();
        let bindings = Civ3ContentBindings::build(&rules).unwrap();

        assert_eq!(
            bindings.count(ContentKind::Unit, true),
            UnitKind::ALL_NATIVE.len()
        );
        assert_eq!(bindings.count(ContentKind::Building, true), 47);
        assert_eq!(bindings.count(ContentKind::Building, false), 36);
        assert_eq!(bindings.count(ContentKind::Technology, true), 83);
        assert_eq!(bindings.count(ContentKind::Technology, false), 0);
        assert_eq!(bindings.count(ContentKind::Nation, true), 31);

        for (kind, record) in UnitKind::ALL_NATIVE.iter().zip(
            bindings
                .records
                .iter()
                .filter(|record| record.kind == ContentKind::Unit),
        ) {
            assert_eq!(record.runtime_id, Some(StableContentId::Unit(*kind)));
        }
        assert_eq!(
            bindings
                .by_source_id(ContentKind::Technology, "TECH_Amphibious_Warfare")
                .unwrap()
                .runtime_id,
            Some(StableContentId::Technology(Tech::AMPHIBIOUS_WARFARE))
        );
        let ironclads = bindings
            .by_source_id(ContentKind::Technology, "TECH_Ironclads")
            .unwrap();
        assert_eq!(
            ironclads.runtime_id,
            Some(StableContentId::Technology(Tech::IRONCLADS))
        );
        assert_eq!(ironclads.mapping_note, "");
        assert!(
            bindings
                .records
                .iter()
                .filter(|r| r.kind == ContentKind::Building && r.runtime_id.is_none())
                .all(|r| !r.mapping_note.is_empty())
        );
    }

    #[test]
    fn calendar_technology_effects_use_explicit_civ3_bindings() {
        let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let rules = RulesetData::load_with_civ3_catalog(
            &data_root,
            "civcraft_civ2civ3",
            &data_root.join("civ3_conquests"),
        )
        .unwrap();

        assert_eq!(
            bind_freeciv_technology(&rules, "Space Flight"),
            Ok(FreecivTechBinding::Bound(Tech::SPACE_FLIGHT))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Superconductors"),
            Ok(FreecivTechBinding::Bound(Tech::SUPERCONDUCTORS))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Nuclear Fission"),
            Ok(FreecivTechBinding::Bound(Tech::FISSION))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Conscription"),
            Ok(FreecivTechBinding::Bound(Tech::NATIONALISM))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Environmentalism"),
            Ok(FreecivTechBinding::Bound(Tech::ECOLOGY))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Laser"),
            Ok(FreecivTechBinding::Bound(Tech::THE_LASER))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Mobile Warfare"),
            Ok(FreecivTechBinding::Bound(Tech::MOTORIZED_TRANSPORTATION))
        );
        assert_eq!(
            bind_freeciv_technology(&rules, "Plastics"),
            Ok(FreecivTechBinding::MissingFromCiv3Catalog)
        );
    }
}
