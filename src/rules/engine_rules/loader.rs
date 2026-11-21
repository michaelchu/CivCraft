//! Filesystem loading and typed decoding for native Freeciv rulesets.

use crate::rules::content_adapter::{FreecivTechBinding, bind_freeciv_technology};
use crate::rules::engine_rules::RequirementKind;
use crate::rules::{Building, Tech};
use crate::world::Terrain;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::section_file::{self, Entry, Section, SectionFile, Value};
use super::{
    ActionEnabler, ActionRecord, ContentKind, ContentRecord, EffectRecord, LoadError,
    LoadedRuleFile, Requirement, RequirementRange, RequirementValue, ResolvedRequirementValue,
    RuleFileSource, RuleParameter, RuleRecord, RuleSourceLocation, RulesetData, SettingRecord,
};

// The bundle and the filesystem loader use the same file list and order.
macro_rules! campaign_files {
    ($($name:literal),+ $(,)?) => {
        pub(crate) const RULESET_FILES: &[&str] = &[$($name),+];
        fn embedded_ruleset_source(path: &Path) -> Option<&'static str> {
            match path.strip_prefix("civcraft/engine").ok().and_then(Path::to_str) {
                $(Some($name) => Some(include_str!(
                    concat!("../../../data/rulesets/civcraft/engine/", $name)
                )),)+
                _ => None,
            }
        }
    };
}

campaign_files!(
    "game.ruleset",
    "actions.ruleset",
    "units.ruleset",
    "terrain.ruleset",
    "effects.ruleset",
);

impl RulesetData {
    /// Load one named ruleset. Missing files use Freeciv's whole-file
    /// `default/` fallback; no sections or keys are merged across files.
    pub(crate) fn load(data_root: &Path, name: &str) -> Result<Self, LoadError> {
        Self::load_from_directory(data_root, &data_root.join(name), name)
    }

    /// Load a ruleset whose selected files live in a package subdirectory.
    /// Shared Freeciv fallback files remain rooted at `data_root`.
    pub(crate) fn load_from_directory(
        data_root: &Path,
        ruleset_dir: &Path,
        name: &str,
    ) -> Result<Self, LoadError> {
        let mut files = Vec::new();
        for file_name in RULESET_FILES {
            let (path, source) = resolve_ruleset_file(data_root, ruleset_dir, name, file_name)?;
            let parsed = load_section_file(data_root, &path, &mut Vec::new())?;
            files.push(LoadedRuleFile {
                name: (*file_name).to_owned(),
                path,
                source,
                parsed,
            });
        }
        Self::from_files(name, files)
    }

    pub(super) fn load_bundled_campaign() -> Result<Self, LoadError> {
        let mut files = Vec::new();
        for &name in RULESET_FILES {
            let path = Path::new("civcraft/engine").join(name);
            let parsed = load_embedded_section_file(&path, &mut Vec::new())?;
            files.push(LoadedRuleFile {
                name: name.to_owned(),
                path,
                source: RuleFileSource::SelectedRuleset,
                parsed,
            });
        }
        Self::from_files("civcraft", files)
    }

    pub(crate) fn from_files(name: &str, files: Vec<LoadedRuleFile>) -> Result<Self, LoadError> {
        for required in RULESET_FILES {
            if !files.iter().any(|file| file.name == *required) {
                return Err(LoadError::new(format!(
                    "missing required ruleset file {required}"
                )));
            }
        }

        let mut settings = Vec::new();
        let mut parameters = Vec::new();
        let mut effects = Vec::new();
        let mut actions = Vec::new();
        let mut action_enablers = Vec::new();
        let mut content = Vec::new();
        let mut rule_records = Vec::new();

        for loaded in &files {
            let file_name = &loaded.name;
            for section in &loaded.parsed.sections {
                for entry in &section.entries {
                    if file_name == "game.ruleset"
                        && section.name == "settings"
                        && entry.key == "set"
                    {
                        settings.extend(parse_settings(&loaded.path, section, entry)?);
                    }
                    if !is_record_section(&section.name) && entry.key != "*include" {
                        parameters.push(RuleParameter {
                            file: file_name.clone(),
                            section: section.name.clone(),
                            key: entry.key.clone(),
                            value: entry.value.clone(),
                        });
                    }
                }

                if file_name == "effects.ruleset" && section.name.starts_with("effect_") {
                    effects.push(parse_effect(&loaded.path, section)?);
                }
                if file_name == "actions.ruleset" && section.name.starts_with("action_") {
                    actions.push(parse_action(&loaded.path, section)?);
                }
                if file_name == "actions.ruleset" && section.name.starts_with("enabler_") {
                    action_enablers.push(parse_action_enabler(&loaded.path, section)?);
                }
                if let Some(kind) = content_kind(file_name, &section.name) {
                    content.push(parse_content(&loaded.path, section, kind)?);
                } else if let Some(namespace) = record_namespace(&section.name) {
                    rule_records.push(parse_rule_record(
                        &loaded.path,
                        file_name,
                        section,
                        namespace,
                    )?);
                }
            }
        }

        let mut rules = Self {
            name: name.to_owned(),
            files,
            settings,
            parameters,
            effects: effects.into(),
            actions,
            action_enablers,
            action_index: Default::default(),
            content,
            rule_records,
        };
        validate_requirement_values(&rules)?;
        resolve_requirement_values(&mut rules);
        rules.build_lookup_indexes();
        Ok(rules)
    }
}

/// Resolve runtime-backed requirement names once, at ruleset load time. Names
/// without a stable CivCraft identity intentionally remain strings: they are
/// still valid Freeciv content, but cannot be evaluated against campaign
/// state by the runtime adapters.
pub(super) fn resolve_requirement_values(rules: &mut RulesetData) {
    let mut effects: Vec<_> = rules.effects.iter().cloned().collect();
    for effect in &mut effects {
        resolve_requirements(&mut effect.requirements);
    }
    rules.effects = effects.into();
    for enabler in &mut rules.action_enablers {
        resolve_requirements(&mut enabler.actor_requirements);
        resolve_requirements(&mut enabler.target_requirements);
    }
    for record in &mut rules.content {
        resolve_requirements(&mut record.requirements);
        resolve_requirements(&mut record.obsolete_by);
    }
    for record in &mut rules.rule_records {
        resolve_requirements(&mut record.requirements);
    }
}

fn resolve_requirements(requirements: &mut [Requirement]) {
    for requirement in requirements {
        let RequirementValue::String(name) = &requirement.name else {
            continue;
        };
        let resolved = match requirement.kind {
            RequirementKind::Tech => resolve_tech(name).map(ResolvedRequirementValue::Tech),
            RequirementKind::Building => {
                resolve_building(name).map(ResolvedRequirementValue::Building)
            }
            RequirementKind::Terrain => {
                resolve_terrain(name).map(ResolvedRequirementValue::Terrain)
            }
            _ => None,
        };
        if let Some(resolved) = resolved {
            requirement.resolved = Some(resolved);
        }
    }
}

fn resolve_tech(name: &str) -> Option<Tech> {
    match bind_freeciv_technology(name).ok()? {
        FreecivTechBinding::Bound(tech) => Some(tech),
        FreecivTechBinding::MissingFromCiv3Catalog => None,
    }
}

fn resolve_building(name: &str) -> Option<Building> {
    Building::all().find(|building| {
        building.data().name.eq_ignore_ascii_case(name)
            || (building.data().name.eq_ignore_ascii_case("Harbor")
                && name.eq_ignore_ascii_case("Harbour"))
            || (building
                .data()
                .name
                .eq_ignore_ascii_case("Shakespeare's Theater")
                && name.eq_ignore_ascii_case("Shakespeare's Theatre"))
    })
}

fn resolve_terrain(name: &str) -> Option<Terrain> {
    match name.to_ascii_lowercase().as_str() {
        "ocean" => Some(Terrain::Sea),
        "deep ocean" => Some(Terrain::Ocean),
        "swamp" => Some(Terrain::Marsh),
        _ => [
            Terrain::Sea,
            Terrain::Ocean,
            Terrain::Lake,
            Terrain::Glacier,
            Terrain::Jungle,
            Terrain::Desert,
            Terrain::Forest,
            Terrain::Grassland,
            Terrain::Hills,
            Terrain::Mountains,
            Terrain::Plains,
            Terrain::Marsh,
            Terrain::Tundra,
            Terrain::Coast,
            Terrain::FloodPlains,
            Terrain::Volcano,
            Terrain::GrasslandShield,
        ]
        .into_iter()
        .find(|terrain| terrain.name().eq_ignore_ascii_case(name)),
    }
}

fn resolve_ruleset_file(
    data_root: &Path,
    ruleset_dir: &Path,
    ruleset_name: &str,
    file_name: &str,
) -> Result<(PathBuf, RuleFileSource), LoadError> {
    let selected = ruleset_dir.join(file_name);
    if selected.is_file() {
        return Ok((selected, RuleFileSource::SelectedRuleset));
    }
    let default = data_root.join("default").join(file_name);
    if default.is_file() {
        return Ok((default, RuleFileSource::DefaultRuleset));
    }
    let stem = file_name.strip_suffix(".ruleset").unwrap_or(file_name);
    let legacy = data_root.join(format!("{ruleset_name}_{stem}.ruleset"));
    if legacy.is_file() {
        return Ok((legacy, RuleFileSource::LegacyFilename));
    }
    Err(LoadError::new(format!(
        "no {file_name} found in {} or {}",
        ruleset_dir.display(),
        data_root.join("default").display()
    )))
}

fn load_section_file(
    data_root: &Path,
    path: &Path,
    include_stack: &mut Vec<PathBuf>,
) -> Result<SectionFile, LoadError> {
    let canonical_root = data_root
        .canonicalize()
        .map_err(|error| LoadError::at(data_root, format!("cannot resolve data root: {error}")))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|error| LoadError::at(path, format!("cannot read ruleset file: {error}")))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(LoadError::at(path, "ruleset include escapes the data root"));
    }
    let source = fs::read_to_string(&canonical_path).map_err(|error| {
        LoadError::at(
            &canonical_path,
            format!("cannot read ruleset file: {error}"),
        )
    })?;
    parse_with_includes(&canonical_path, &source, include_stack, |name, stack| {
        load_section_file(data_root, &data_root.join(name), stack)
    })
}

fn load_embedded_section_file(
    path: &Path,
    include_stack: &mut Vec<PathBuf>,
) -> Result<SectionFile, LoadError> {
    let source = embedded_ruleset_source(path)
        .ok_or_else(|| LoadError::at(path, "file is absent from the embedded campaign ruleset"))?;
    parse_with_includes(path, source, include_stack, |name, stack| {
        load_embedded_section_file(Path::new(name), stack)
    })
}

fn parse_with_includes(
    path: &Path,
    source: &str,
    include_stack: &mut Vec<PathBuf>,
    load_include: impl Fn(&str, &mut Vec<PathBuf>) -> Result<SectionFile, LoadError>,
) -> Result<SectionFile, LoadError> {
    if include_stack.iter().any(|ancestor| ancestor == path) {
        return Err(LoadError::at(path, "recursive ruleset include"));
    }
    include_stack.push(path.to_owned());
    let parsed = section_file::parse(source).map_err(|error| LoadError {
        file: Some(path.to_owned()),
        section: (!error.context.is_empty()).then_some(error.context),
        key: None,
        line: Some(error.line),
        message: error.message,
    })?;
    let mut expanded = SectionFile::default();

    for section in parsed.sections {
        let mut current = Section {
            name: section.name.clone(),
            entries: Vec::new(),
        };
        for entry in section.entries {
            if entry.key == "*include" {
                let Value::String(include_name) = entry.value else {
                    return Err(LoadError::at_line(
                        path,
                        &section.name,
                        &entry.key,
                        entry.line,
                        "include path must be a string",
                    ));
                };
                if !current.entries.is_empty() {
                    expanded.sections.push(current);
                    current = Section {
                        name: section.name.clone(),
                        entries: Vec::new(),
                    };
                }
                let included = load_include(&include_name, include_stack)?;
                expanded.sections.extend(included.sections);
            } else {
                current.entries.push(entry);
            }
        }
        expanded.sections.push(current);
    }

    include_stack.pop();
    merge_repeated_sections(expanded, path)
}

fn merge_repeated_sections(
    file: SectionFile,
    source_path: &Path,
) -> Result<SectionFile, LoadError> {
    let mut sections: Vec<Section> = Vec::new();
    let mut positions: BTreeMap<String, usize> = BTreeMap::new();
    for section in file.sections {
        if let Some(position) = positions.get(&section.name).copied() {
            let existing = &mut sections[position].entries;
            for entry in section.entries {
                if existing.iter().any(|previous| previous.key == entry.key) {
                    return Err(LoadError::at_line(
                        source_path,
                        &section.name,
                        &entry.key,
                        entry.line,
                        format!(
                            "duplicate ruleset entry {:?} after include expansion",
                            entry.key
                        ),
                    ));
                }
                existing.push(entry);
            }
        } else {
            for (index, entry) in section.entries.iter().enumerate() {
                if section.entries[..index]
                    .iter()
                    .any(|previous| previous.key == entry.key)
                {
                    return Err(LoadError::at_line(
                        source_path,
                        &section.name,
                        &entry.key,
                        entry.line,
                        format!("duplicate ruleset entry {:?}", entry.key),
                    ));
                }
            }
            positions.insert(section.name.clone(), sections.len());
            sections.push(section);
        }
    }
    Ok(SectionFile { sections })
}

fn parse_settings(
    file: &Path,
    section: &Section,
    entry: &Entry,
) -> Result<Vec<SettingRecord>, LoadError> {
    let rows = table_rows(file, section, entry)?;
    rows.into_iter()
        .map(|row| {
            let name = row_string(file, section, entry, &row, "name")?;
            let value = row_value(file, section, entry, &row, "value")?.clone();
            Ok(SettingRecord { name, value })
        })
        .collect()
}

fn parse_effect(file: &Path, section: &Section) -> Result<EffectRecord, LoadError> {
    let kind = field_string(file, section, "type")?
        .ok_or_else(|| LoadError::in_section(file, section, "effect record is missing `type`"))?;
    validate_identifier(
        file,
        section,
        "type",
        &kind,
        super::identifiers::EFFECT_NAMES,
        "effect type",
    )?;
    let kind = super::EffectKind::parse(&kind).expect("validated effect type");
    let value = match field_value(section, "value") {
        Some(Value::Integer(value)) => *value,
        None | Some(Value::Empty) => 1,
        Some(_) => {
            return Err(LoadError::in_key(
                file,
                section,
                "value",
                "expected an integer",
            ));
        }
    };
    let multiplier = field_string(file, section, "multiplier")?;
    let requirements = field_requirements(file, section, "reqs")?;
    Ok(EffectRecord {
        rule_name: section_suffix(section, "effect_"),
        kind,
        value,
        multiplier,
        requirements,
    })
}

fn parse_action(file: &Path, section: &Section) -> Result<ActionRecord, LoadError> {
    let action = field_string(file, section, "action")?
        .ok_or_else(|| LoadError::in_section(file, section, "action record is missing `action`"))?;
    validate_identifier(
        file,
        section,
        "action",
        &action,
        super::identifiers::ACTION_NAMES,
        "action",
    )?;
    Ok(ActionRecord {
        rule_name: section_suffix(section, "action_"),
        action,
        fields: section.entries.clone(),
    })
}

fn parse_action_enabler(file: &Path, section: &Section) -> Result<ActionEnabler, LoadError> {
    let action = field_string(file, section, "action")?.ok_or_else(|| {
        LoadError::in_section(file, section, "action enabler is missing `action`")
    })?;
    validate_identifier(
        file,
        section,
        "action",
        &action,
        super::identifiers::ACTION_NAMES,
        "action",
    )?;
    Ok(ActionEnabler {
        rule_name: section_suffix(section, "enabler_"),
        action,
        actor_requirements: field_requirements(file, section, "actor_reqs")?,
        target_requirements: field_requirements(file, section, "target_reqs")?,
        fields: section.entries.clone(),
    })
}

fn parse_content(
    file: &Path,
    section: &Section,
    kind: ContentKind,
) -> Result<ContentRecord, LoadError> {
    let display_name = field_string(file, section, "name")?
        .ok_or_else(|| LoadError::in_section(file, section, "content record is missing `name`"))?;
    let display_name = strip_translation_qualifier(&display_name);
    let rule_name =
        field_string(file, section, "rule_name")?.unwrap_or_else(|| display_name.clone());
    let requirements = match kind {
        ContentKind::Building => field_requirements(file, section, "reqs")?,
        ContentKind::Unit => field_requirements(file, section, "reqs")?,
        ContentKind::Technology => field_requirements(file, section, "research_reqs")?,
        ContentKind::Terrain => field_requirements(file, section, "reqs")?,
        ContentKind::Government => field_requirements(file, section, "reqs")?,
        ContentKind::GovernmentMultiplier => field_requirements(file, section, "reqs")?,
        ContentKind::Nation | ContentKind::Style => Vec::new(),
    };
    let obsolete_by = if kind == ContentKind::Building {
        field_requirements(file, section, "obsolete_by")?
    } else {
        Vec::new()
    };
    Ok(ContentRecord {
        kind,
        rule_name,
        section: section.name.clone(),
        display_name: Some(display_name),
        requirements,
        obsolete_by,
        fields: section.entries.clone(),
    })
}

fn parse_rule_record(
    file: &Path,
    file_name: &str,
    section: &Section,
    namespace: &str,
) -> Result<RuleRecord, LoadError> {
    if namespace == "user_effect" {
        for key in ["type", "ai_valued_as"] {
            let value = field_string(file, section, key)?.ok_or_else(|| {
                LoadError::in_section(file, section, format!("user effect is missing `{key}`"))
            })?;
            validate_identifier(
                file,
                section,
                key,
                &value,
                super::identifiers::EFFECT_NAMES,
                "effect type",
            )?;
        }
    }
    let display_name =
        field_string(file, section, "name")?.map(|name| strip_translation_qualifier(&name));
    // Some Freeciv record families have no display name. In those cases the
    // section suffix is their native stable identifier (for example
    // [clause_advance] and [resource_iron]).
    let section_id = record_section_id(section, namespace);
    let rule_name = field_string(file, section, "rule_name")?
        .or_else(|| display_name.clone())
        .unwrap_or(section_id);
    let mut requirements = field_requirements(file, section, "reqs")?;
    requirements.extend(field_requirements(file, section, "research_reqs")?);
    Ok(RuleRecord {
        file: file_name.to_owned(),
        namespace: namespace.to_owned(),
        section: section.name.clone(),
        rule_name,
        display_name,
        requirements,
        fields: section.entries.clone(),
    })
}

fn strip_translation_qualifier(name: &str) -> String {
    if let Some(qualified) = name.strip_prefix('?')
        && let Some((_, untranslated)) = qualified.split_once(':')
    {
        return untranslated.to_owned();
    }
    name.to_owned()
}

fn validate_identifier(
    file: &Path,
    section: &Section,
    key: &str,
    value: &str,
    names: &[&str],
    kind: &str,
) -> Result<(), LoadError> {
    if names.iter().any(|known| known.eq_ignore_ascii_case(value)) {
        Ok(())
    } else {
        Err(LoadError::in_key(
            file,
            section,
            key,
            format!("unknown Freeciv {kind} {value:?}"),
        ))
    }
}

fn record_section_id(section: &Section, namespace: &str) -> String {
    let prefix = match namespace {
        "super_specialist" => "super_specialist_",
        "citystyle" => "citystyle_",
        "musicstyle" => "musicstyle_",
        "nation_set" => "nset_",
        "nation_group" => "ngroup_",
        "nation" => "nation_",
        "clause" => "clause_",
        "extra" => "extra_",
        "base" => "base_",
        "road" => "road_",
        "resource" => "resource_",
        "goods" => "goods_",
        "specialist" => "specialist_",
        "unit_class" => "unitclass_",
        "disaster" => "disaster_",
        "achievement" => "achievement_",
        "tech_class" => "techclass_",
        "tile_definition" => "tiledef_",
        "counter" => "counter_",
        "user_effect" => "ueff_",
        "multiplier" => "multiplier_",
        _ => "",
    };
    section
        .name
        .strip_prefix(prefix)
        .unwrap_or(&section.name)
        .to_owned()
}

fn field_requirements(
    file: &Path,
    section: &Section,
    key: &str,
) -> Result<Vec<Requirement>, LoadError> {
    let Some(entry) = section.entries.iter().find(|entry| entry.key == key) else {
        return Ok(Vec::new());
    };
    table_rows(file, section, entry)?
        .into_iter()
        .map(|row| parse_requirement(file, section, entry, &row))
        .collect()
}

fn parse_requirement(
    file: &Path,
    section: &Section,
    entry: &Entry,
    row: &BTreeMap<String, Value>,
) -> Result<Requirement, LoadError> {
    for column in row.keys() {
        if !matches!(
            column.as_str(),
            "type" | "name" | "range" | "present" | "survives" | "quiet"
        ) {
            return Err(LoadError::in_key(
                file,
                section,
                &entry.key,
                format!("unsupported requirement column {column:?}"),
            ));
        }
    }
    let kind = row_string(file, section, entry, row, "type")?;
    validate_identifier(
        file,
        section,
        &entry.key,
        &kind,
        super::identifiers::REQUIREMENT_KINDS,
        "requirement type",
    )?;
    let kind = super::RequirementKind::parse(&kind).expect("validated requirement type");
    let name = match row_value(file, section, entry, row, "name")? {
        Value::String(value) => RequirementValue::String(value.clone()),
        Value::Integer(value) => RequirementValue::Integer(*value),
        Value::Bool(value) => RequirementValue::Bool(*value),
        _ => {
            return Err(LoadError::in_key(
                file,
                section,
                &entry.key,
                "requirement name must be scalar",
            ));
        }
    };
    let range_name = row_string(file, section, entry, row, "range")?;
    let range = match range_name.to_ascii_lowercase().as_str() {
        "local" => RequirementRange::Local,
        "tile" => RequirementRange::Tile,
        "cadjacent" => RequirementRange::CAdjacent,
        "adjacent" => RequirementRange::Adjacent,
        "city" => RequirementRange::City,
        "traderoute" => RequirementRange::Traderoute,
        "continent" => RequirementRange::Continent,
        "player" => RequirementRange::Player,
        "team" => RequirementRange::Team,
        "alliance" => RequirementRange::Alliance,
        "world" => RequirementRange::World,
        _ => {
            return Err(LoadError::in_key(
                file,
                section,
                &entry.key,
                format!("unknown requirement range {range_name:?}"),
            ));
        }
    };
    Ok(Requirement {
        kind,
        name,
        resolved: None,
        range,
        present: row_bool(file, section, entry, row, "present", true)?,
        survives: row_bool(file, section, entry, row, "survives", false)?,
        quiet: row_bool(file, section, entry, row, "quiet", false)?,
        source: RuleSourceLocation {
            file: file.to_owned(),
            section: section.name.clone(),
            key: entry.key.clone(),
            line: entry.line,
        },
    })
}

fn validate_requirement_values(rules: &RulesetData) -> Result<(), LoadError> {
    let mut requirements = Vec::new();
    requirements.extend(rules.effects.iter().flat_map(|record| &record.requirements));
    requirements.extend(rules.action_enablers.iter().flat_map(|record| {
        record
            .actor_requirements
            .iter()
            .chain(&record.target_requirements)
    }));
    requirements.extend(
        rules
            .content
            .iter()
            .flat_map(|record| record.requirements.iter().chain(&record.obsolete_by)),
    );
    requirements.extend(
        rules
            .rule_records
            .iter()
            .flat_map(|record| &record.requirements),
    );

    for requirement in requirements {
        if requirement_value_is_valid(requirement, rules) {
            continue;
        }
        let source = &requirement.source;
        let name =
            requirement_name_text(&requirement.name).unwrap_or_else(|| "<non-scalar>".into());
        return Err(LoadError::at_line(
            &source.file,
            &source.section,
            &source.key,
            source.line,
            format!(
                "unknown or unsupported value {name:?} for Freeciv requirement type {:?}",
                requirement.kind
            ),
        ));
    }
    Ok(())
}

fn requirement_value_is_valid(requirement: &Requirement, rules: &RulesetData) -> bool {
    let Some(name) = requirement_name_text(&requirement.name) else {
        return false;
    };
    match requirement.kind {
        RequirementKind::None => true,
        RequirementKind::Achievement => record_name_exists(rules, "achievement", &name),
        RequirementKind::Action => {
            super::identifiers::contains_case_insensitive(super::identifiers::ACTION_NAMES, &name)
        }
        RequirementKind::Activity => {
            super::identifiers::contains_case_insensitive(super::identifiers::ACTIVITY_NAMES, &name)
        }
        RequirementKind::Tech => {
            content_name_exists(rules, ContentKind::Technology, &name)
                || (rules.name == "civcraft" && bind_freeciv_technology(&name).is_ok())
        }
        RequirementKind::Age => integer_value(&name).is_some_and(|value| value > 0),
        RequirementKind::Ai => {
            super::identifiers::contains_case_insensitive(super::identifiers::AI_LEVEL_NAMES, &name)
        }
        RequirementKind::CityStatus => super::identifiers::contains_case_insensitive(
            super::identifiers::CITY_STATUS_NAMES,
            &name,
        ),
        RequirementKind::CityTile => super::identifiers::contains_case_insensitive(
            super::identifiers::CITY_TILE_NAMES,
            &name,
        ),
        RequirementKind::Counter => record_name_exists(rules, "counter", &name),
        RequirementKind::DiplRel
        | RequirementKind::DiplRelTile
        | RequirementKind::DiplRelTileOther
        | RequirementKind::DiplRelUnitAny
        | RequirementKind::DiplRelUnitAnyOther => super::identifiers::contains_case_insensitive(
            super::identifiers::DIPLO_REL_NAMES,
            &name,
        ),
        RequirementKind::Extra => record_name_exists(rules, "extra", &name),
        RequirementKind::ExtraFlag => flag_name_exists(
            rules,
            &name,
            "terrain.ruleset",
            "extra_flags",
            super::identifiers::EXTRA_FLAG_NAMES,
        ),
        RequirementKind::FormAge => integer_value(&name).is_some_and(|value| value > 0),
        RequirementKind::FutureTechs => integer_value(&name).is_some_and(|value| value > 0),
        RequirementKind::Good => record_name_exists(rules, "goods", &name),
        RequirementKind::Gov => {
            content_name_exists(rules, ContentKind::Government, &name)
                || (rules.name == "civcraft"
                    && (crate::rules::native::governments().get(&name).is_some()
                        || ["Tribal", "Fundamentalism", "Federation"]
                            .iter()
                            .any(|gov| gov.eq_ignore_ascii_case(&name))))
        }
        RequirementKind::GovFlag => flag_name_exists(
            rules,
            &name,
            "governments.ruleset",
            "flags",
            super::identifiers::GOV_FLAG_NAMES,
        ),
        RequirementKind::Building => building_name_exists(rules, &name),
        RequirementKind::BuildingFlag => flag_name_exists(
            rules,
            &name,
            "buildings.ruleset",
            "building_flags",
            super::identifiers::BUILDING_FLAG_NAMES,
        ),
        RequirementKind::BuildingGenus => super::identifiers::contains_case_insensitive(
            super::identifiers::IMPR_GENUS_NAMES,
            &name,
        ),
        RequirementKind::MaxLatitude | RequirementKind::MinLatitude => {
            integer_value(&name).is_some_and(|value| (-1000..=1000).contains(&value))
        }
        RequirementKind::MaxTopUnitsOnTile | RequirementKind::MaxUnitsOnTile => {
            integer_value(&name).is_some_and(|value| value >= 0)
        }
        RequirementKind::MinCalFrag => integer_value(&name).is_some_and(|value| value >= 0),
        RequirementKind::MinCities
        | RequirementKind::MinCulture
        | RequirementKind::MinForeignPct
        | RequirementKind::MinHitPoints
        | RequirementKind::MinMoveFrags
        | RequirementKind::MinSize
        | RequirementKind::MinTechs
        | RequirementKind::MinVeteran
        | RequirementKind::MinYear => integer_value(&name).is_some_and(|value| {
            if requirement.kind == RequirementKind::MinYear {
                true
            } else {
                value > 0
            }
        }),
        RequirementKind::Nation | RequirementKind::Nationality | RequirementKind::OriginalOwner => {
            content_name_exists(rules, ContentKind::Nation, &name)
                || name.eq_ignore_ascii_case("Animal Kingdom")
        }
        RequirementKind::NationGroup => {
            record_name_exists(rules, "nation_group", &name)
                || name.eq_ignore_ascii_case("Barbarian")
        }
        RequirementKind::OutputType => super::identifiers::contains_case_insensitive(
            super::identifiers::OUTPUT_TYPE_NAMES,
            &name,
        ),
        RequirementKind::PlayerFlag => super::identifiers::contains_case_insensitive(
            super::identifiers::PLAYER_FLAG_NAMES,
            &name,
        ),
        RequirementKind::PlayerState => super::identifiers::contains_case_insensitive(
            super::identifiers::PLAYER_STATE_NAMES,
            &name,
        ),
        RequirementKind::RoadFlag => flag_name_exists(
            rules,
            &name,
            "terrain.ruleset",
            "road_flags",
            super::identifiers::ROAD_FLAG_NAMES,
        ),
        RequirementKind::ServerSetting => rules
            .settings
            .iter()
            .any(|setting| setting.name.eq_ignore_ascii_case(&name)),
        RequirementKind::Site => building_name_exists(rules, &name),
        RequirementKind::Specialist => {
            record_name_exists(rules, "specialist", &name)
                || ["Elvis", "scientist", "taxman"]
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(&name))
        }
        RequirementKind::Style => content_name_exists(rules, ContentKind::Style, &name),
        RequirementKind::TechFlag => flag_name_exists(
            rules,
            &name,
            "techs.ruleset",
            "flags",
            super::identifiers::TECH_FLAG_NAMES,
        ),
        RequirementKind::Terrain => crate::rules::terrain_catalog::terrain_spec(&name).is_some(),
        RequirementKind::TerrainAlter => super::identifiers::contains_case_insensitive(
            super::identifiers::TERRAIN_ALTER_NAMES,
            &name,
        ),
        RequirementKind::TerrainClass => super::identifiers::contains_case_insensitive(
            super::identifiers::TERRAIN_CLASS_NAMES,
            &name,
        ),
        RequirementKind::TerrainFlag => flag_name_exists(
            rules,
            &name,
            "terrain.ruleset",
            "flags",
            super::identifiers::TERRAIN_FLAG_NAMES,
        ),
        RequirementKind::Tiledef | RequirementKind::TiledefConnected => {
            record_name_exists(rules, "tile_definition", &name)
        }
        RequirementKind::Topology => {
            super::identifiers::contains_case_insensitive(super::identifiers::TOPOLOGY_NAMES, &name)
        }
        RequirementKind::UnitClassFlag => flag_name_exists(
            rules,
            &name,
            "units.ruleset",
            "class_flags",
            super::identifiers::UNIT_CLASS_FLAG_NAMES,
        ),
        RequirementKind::UnitClass => record_name_exists(rules, "unit_class", &name),
        RequirementKind::UnitState => super::identifiers::contains_case_insensitive(
            super::identifiers::UNIT_STATE_NAMES,
            &name,
        ),
        RequirementKind::UnitTypeFlag => flag_name_exists(
            rules,
            &name,
            "units.ruleset",
            "flags",
            super::identifiers::UNIT_TYPE_FLAG_NAMES,
        ),
        RequirementKind::UnitType => content_name_exists(rules, ContentKind::Unit, &name),
        RequirementKind::Wrap => {
            super::identifiers::contains_case_insensitive(super::identifiers::WRAP_NAMES, &name)
        } // A known type without a source-backed resolver is rejected explicitly.
    }
}

fn requirement_name_text(value: &RequirementValue) -> Option<String> {
    match value {
        RequirementValue::String(value) => Some(value.clone()),
        RequirementValue::Integer(value) => Some(value.to_string()),
        RequirementValue::Bool(value) => Some(if *value { "TRUE" } else { "FALSE" }.to_owned()),
    }
}

fn integer_value(value: &str) -> Option<i64> {
    value
        .parse::<i64>()
        .ok()
        .filter(|number| i32::try_from(*number).is_ok())
}

fn content_name_exists(rules: &RulesetData, kind: ContentKind, name: &str) -> bool {
    rules
        .content
        .iter()
        .any(|record| record.kind == kind && record.rule_name.eq_ignore_ascii_case(name))
}

fn building_name_exists(rules: &RulesetData, name: &str) -> bool {
    content_name_exists(rules, ContentKind::Building, name)
        || (rules.name == "civcraft"
            && (crate::rules::native::buildings()
                .rules
                .iter()
                .any(|building| building.name.eq_ignore_ascii_case(name))
                || ["Harbour", "Shakespeare's Theatre"]
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(name))))
}

fn record_name_exists(rules: &RulesetData, namespace: &str, name: &str) -> bool {
    rules
        .rule_records
        .iter()
        .any(|record| record.namespace == namespace && record.rule_name.eq_ignore_ascii_case(name))
}

fn flag_name_exists(
    rules: &RulesetData,
    name: &str,
    file: &str,
    key: &str,
    built_in: &[&str],
) -> bool {
    super::identifiers::contains_case_insensitive(built_in, name)
        || rules.parameters.iter().any(|parameter| {
            parameter.file == file
                && parameter.section == "control"
                && parameter.key == key
                && value_contains_name(&parameter.value, name)
        })
}

fn value_contains_name(value: &Value, name: &str) -> bool {
    match value {
        Value::String(value) => value.eq_ignore_ascii_case(name),
        Value::List(values) => values.iter().any(|value| value_contains_name(value, name)),
        Value::Table { rows, .. } => rows
            .iter()
            .flatten()
            .any(|value| value_contains_name(value, name)),
        Value::Integer(value) => value.to_string().eq_ignore_ascii_case(name),
        Value::Bool(value) => (if *value { "TRUE" } else { "FALSE" }).eq_ignore_ascii_case(name),
        Value::Float(value) => value.eq_ignore_ascii_case(name),
        Value::Empty => false,
    }
}

fn table_rows(
    file: &Path,
    section: &Section,
    entry: &Entry,
) -> Result<Vec<BTreeMap<String, Value>>, LoadError> {
    match &entry.value {
        Value::Empty => Ok(Vec::new()),
        Value::Table { columns, rows } => rows
            .iter()
            .map(|row| {
                Ok(row
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let key = columns
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| format!("extra_column_{}", index + 1));
                        (key, value.clone())
                    })
                    .collect())
            })
            .collect(),
        _ => Err(LoadError::in_key(
            file,
            section,
            &entry.key,
            "expected a brace table",
        )),
    }
}

fn row_value<'a>(
    file: &Path,
    section: &Section,
    entry: &Entry,
    row: &'a BTreeMap<String, Value>,
    column: &str,
) -> Result<&'a Value, LoadError> {
    row.get(column).ok_or_else(|| {
        LoadError::in_key(
            file,
            section,
            &entry.key,
            format!("table row is missing required column {column:?}"),
        )
    })
}

fn row_string(
    file: &Path,
    section: &Section,
    entry: &Entry,
    row: &BTreeMap<String, Value>,
    column: &str,
) -> Result<String, LoadError> {
    match row_value(file, section, entry, row, column)? {
        Value::String(value) => Ok(value.clone()),
        Value::Integer(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(LoadError::in_key(
            file,
            section,
            &entry.key,
            format!("table column {column:?} must contain a scalar"),
        )),
    }
}

fn row_bool(
    file: &Path,
    section: &Section,
    entry: &Entry,
    row: &BTreeMap<String, Value>,
    column: &str,
    default: bool,
) -> Result<bool, LoadError> {
    match row.get(column) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::String(value)) if value.eq_ignore_ascii_case("true") => Ok(true),
        Some(Value::String(value)) if value.eq_ignore_ascii_case("false") => Ok(false),
        Some(_) => Err(LoadError::in_key(
            file,
            section,
            &entry.key,
            format!("table column {column:?} must be boolean"),
        )),
    }
}

fn field_value<'a>(section: &'a Section, key: &str) -> Option<&'a Value> {
    section
        .entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| &entry.value)
}

fn field_string(file: &Path, section: &Section, key: &str) -> Result<Option<String>, LoadError> {
    match field_value(section, key) {
        None | Some(Value::Empty) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(LoadError::in_key(file, section, key, "expected a string")),
    }
}

fn section_suffix(section: &Section, prefix: &str) -> String {
    section
        .name
        .strip_prefix(prefix)
        .unwrap_or(&section.name)
        .to_owned()
}

fn record_namespace(section: &str) -> Option<&'static str> {
    [
        ("super_specialist_", "super_specialist"),
        ("citystyle_", "citystyle"),
        ("musicstyle_", "musicstyle"),
        ("nset_", "nation_set"),
        ("ngroup_", "nation_group"),
        ("nation_", "nation"),
        ("clause_", "clause"),
        ("extra_", "extra"),
        ("base_", "base"),
        ("road_", "road"),
        ("resource_", "resource"),
        ("goods_", "goods"),
        ("specialist_", "specialist"),
        ("unitclass_", "unit_class"),
        ("disaster_", "disaster"),
        ("achievement_", "achievement"),
        ("techclass_", "tech_class"),
        ("tiledef_", "tile_definition"),
        ("counter_", "counter"),
        ("ueff_", "user_effect"),
        ("multiplier_", "multiplier"),
    ]
    .iter()
    .find_map(|(prefix, namespace)| section.starts_with(prefix).then_some(*namespace))
}

fn is_record_section(name: &str) -> bool {
    if record_namespace(name).is_some() {
        return true;
    }
    [
        "unit_",
        "building_",
        "advance_",
        "terrain_",
        "government_",
        "nation_",
        "style_",
        "multiplier_",
        "action_",
        "enabler_",
        "effect_",
        "ueff_",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}

fn content_kind(file: &str, section: &str) -> Option<ContentKind> {
    let (expected_file, prefix, kind) = if section.starts_with("unit_") {
        ("units.ruleset", "unit_", ContentKind::Unit)
    } else if section.starts_with("building_") {
        ("buildings.ruleset", "building_", ContentKind::Building)
    } else if section.starts_with("advance_") {
        ("techs.ruleset", "advance_", ContentKind::Technology)
    } else if section.starts_with("terrain_") {
        ("terrain.ruleset", "terrain_", ContentKind::Terrain)
    } else if section.starts_with("government_") {
        (
            "governments.ruleset",
            "government_",
            ContentKind::Government,
        )
    } else if section.starts_with("nation_") {
        ("nations.ruleset", "nation_", ContentKind::Nation)
    } else if section.starts_with("style_") {
        ("styles.ruleset", "style_", ContentKind::Style)
    } else if section.starts_with("multiplier_") {
        (
            "governments.ruleset",
            "multiplier_",
            ContentKind::GovernmentMultiplier,
        )
    } else {
        return None;
    };
    (expected_file == file && section.starts_with(prefix)).then_some(kind)
}

#[cfg(test)]
mod bundle_tests {
    use super::*;

    #[test]
    fn compiled_requirements_and_runtime_use_the_same_technology_aliases() {
        let rules = RulesetData::campaign_default();
        let tech_requirements = rules
            .action_enablers
            .iter()
            .flat_map(|enabler| {
                enabler
                    .actor_requirements
                    .iter()
                    .chain(&enabler.target_requirements)
            })
            .chain(rules.effects.iter().flat_map(|effect| &effect.requirements))
            .chain(
                rules
                    .content
                    .iter()
                    .flat_map(|record| record.requirements.iter().chain(&record.obsolete_by)),
            )
            .chain(
                rules
                    .rule_records
                    .iter()
                    .flat_map(|record| &record.requirements),
            )
            .filter(|requirement| requirement.kind == RequirementKind::Tech)
            .collect::<Vec<_>>();
        for (alias, technology) in [("Radio", Tech::FLIGHT), ("Conscription", Tech::NATIONALISM)] {
            let requirement = tech_requirements
                .iter()
                .find(|req| matches!(&req.name, RequirementValue::String(name) if name.eq_ignore_ascii_case(alias)))
                .unwrap();
            assert_eq!(
                requirement.resolved,
                Some(ResolvedRequirementValue::Tech(technology))
            );
            assert_eq!(
                bind_freeciv_technology(alias),
                Ok(FreecivTechBinding::Bound(technology))
            );
        }
        assert_eq!(
            bind_freeciv_technology("Plastics"),
            Ok(FreecivTechBinding::MissingFromCiv3Catalog)
        );
        assert!(bind_freeciv_technology("Unknown modification").is_err());
    }

    #[test]
    fn embedded_campaign_matches_the_filesystem_ruleset() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let disk = RulesetData::load_civcraft(&root).unwrap();
        let bundled = RulesetData::campaign_default();
        assert_eq!(disk.files.len(), bundled.files.len());
        for (disk, bundled) in disk.files.iter().zip(&bundled.files) {
            assert_eq!(disk.name, bundled.name);
            assert_eq!(disk.parsed, bundled.parsed);
        }
    }
}
