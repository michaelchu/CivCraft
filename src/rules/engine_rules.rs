//! Typed engine ruleset data layered over the native section-file syntax.
//!
//! This module owns data shapes and shared conditional evaluation. It does not
//! own gameplay transitions; simulation subsystems provide a context that
//! resolves rule universals against their current state.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(test)]
use super::section_file;
use super::section_file::{Entry, Section, SectionFile, Value};

#[path = "engine_rules/action_enablers.rs"]
mod action_enablers;
#[path = "engine_rules/campaign.rs"]
mod campaign;
#[path = "engine_rules/civil_war.rs"]
mod civil_war;
#[path = "engine_rules/combat.rs"]
mod combat;
#[path = "engine_rules/disasters.rs"]
mod disasters;
#[path = "engine_rules/great_leader.rs"]
mod great_leader;
#[path = "engine_rules/identifiers.rs"]
mod identifiers;
#[path = "engine_rules/loader.rs"]
mod loader;
#[path = "engine_rules/settlement_ai.rs"]
mod settlement_ai;
#[path = "engine_rules/trade_routes.rs"]
mod trade_routes;
#[path = "engine_rules/veterancy.rs"]
mod veterancy;
pub(crate) use campaign::{DISASTER_FREQUENCY, INITIAL_CITY_RADIUS_SQ, MIGRATION_PML};
pub(crate) use civil_war::CivilWarRulesetSettings;
pub(crate) use combat::{
    CombatBonusKind, CombatRulesetSettings, TransportDefense, UnitCombatBonus,
};
pub(crate) use disasters::{Disaster, DisasterEffect, campaign_disasters};
pub(crate) use great_leader::GreatLeaderSettings;
pub(crate) use identifiers::{EffectKind, RequirementKind};
#[cfg(test)]
pub(crate) use loader::RULESET_FILES;
pub(crate) use settlement_ai::SettlementAiSettings;
#[allow(unused_imports)]
pub(crate) use trade_routes::{
    CaravanBonusStyle, GoodsSelection, TradeGoodFlag, TradeGoodRulesetRecord, TradeRevenueStyle,
    TradeRouteBonus, TradeRouteCancellation, TradeRouteRulesetSettings, TradeRouteType,
    TradeRouteTypeSettings,
};
pub(crate) use veterancy::{VeteranLevel, VeteranSystem, VeterancySettings};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedRuleFile {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) source: RuleFileSource,
    pub(crate) parsed: SectionFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuleFileSource {
    SelectedRuleset,
    DefaultRuleset,
    LegacyFilename,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RulesetData {
    pub(crate) name: String,
    /// The fixed Freeciv load order, with each file's original section order.
    pub(crate) files: Vec<LoadedRuleFile>,
    pub(crate) parameters: Vec<RuleParameter>,
    pub(crate) effects: EffectSet,
    pub(crate) action_enablers: Vec<ActionEnabler>,
    action_index: RuleNameIndex,
    pub(crate) content: Vec<ContentRecord>,
    /// Ordered records from Freeciv namespaces that do not have a dedicated
    /// CivCraft adapter yet (extras, resources, roads, disasters, and others).
    pub(crate) rule_records: Vec<RuleRecord>,
}

/// Immutable name-to-record positions, built in file order at load time.
/// Query comparison folds ASCII without allocating a lowercase string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RuleNameIndex(Vec<(String, Vec<usize>)>);

impl RuleNameIndex {
    fn new(names: impl IntoIterator<Item = (String, usize)>) -> Self {
        let mut entries = std::collections::BTreeMap::<String, Vec<usize>>::new();
        for (name, index) in names {
            let positions = entries.entry(name.to_ascii_lowercase()).or_default();
            if positions.last() != Some(&index) {
                positions.push(index);
            }
        }
        Self(entries.into_iter().collect())
    }

    fn get(&self, name: &str) -> &[usize] {
        self.0
            .binary_search_by(|(key, _)| {
                key.bytes()
                    .cmp(name.bytes().map(|byte| byte.to_ascii_lowercase()))
            })
            .map(|index| self.0[index].1.as_slice())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuleParameter {
    pub(crate) file: String,
    pub(crate) section: String,
    pub(crate) key: String,
    pub(crate) value: Value,
}

impl RuleParameter {
    /// Read an integer ruleset parameter without exposing the section-file
    /// parser's value representation to gameplay adapters.
    pub(crate) fn integer_value(&self) -> Result<Option<i64>, String> {
        match &self.value {
            Value::Integer(value) => Ok(Some(*value)),
            other => Err(format!(
                "{}.{} must be an integer, found {other:?}",
                self.section, self.key
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CalendarSettings {
    pub(crate) start_year: i32,
    pub(crate) skip_year_0: bool,
    pub(crate) fragments_per_year: u32,
    pub(crate) positive_label: String,
    pub(crate) negative_label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResearchCostStyle {
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TechnologyLeakageStyle {
    Embassies,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TechnologyUpkeepStyle {
    Cities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FreeTechnologyStyle {
    Cheapest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RevolutionType {
    RandomQuick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitUpkeepOutput {
    Food,
    Shield,
    Gold,
}

impl UnitUpkeepOutput {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MissingUnitUpkeepOutputSettings {
    pub(crate) protected_flags: Vec<String>,
    /// Ordered Freeciv action names attempted when this output cannot be paid.
    pub(crate) actions: Vec<String>,
    pub(crate) wipe: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MissingUnitUpkeepSettings {
    pub(crate) food: MissingUnitUpkeepOutputSettings,
    pub(crate) shield: MissingUnitUpkeepOutputSettings,
    pub(crate) gold: MissingUnitUpkeepOutputSettings,
}

impl MissingUnitUpkeepSettings {
    pub(crate) fn for_output(&self, output: UnitUpkeepOutput) -> &MissingUnitUpkeepOutputSettings {
        match output {
            UnitUpkeepOutput::Food => &self.food,
            UnitUpkeepOutput::Shield => &self.shield,
            UnitUpkeepOutput::Gold => &self.gold,
        }
    }
}

/// Effective C2C3 research and revolution inputs. The selected ruleset is the
/// authority: settings that Freeciv leaves at engine defaults are materialized
/// explicitly in CivCraft's editable derivative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResearchRulesetSettings {
    pub(crate) cost_style: ResearchCostStyle,
    pub(crate) base_tech_cost: i32,
    pub(crate) min_tech_cost: i32,
    pub(crate) leakage_style: TechnologyLeakageStyle,
    pub(crate) upkeep_style: TechnologyUpkeepStyle,
    pub(crate) upkeep_divider: i32,
    pub(crate) free_technology_style: FreeTechnologyStyle,
    pub(crate) tech_steal_allow_holes: bool,
    pub(crate) tech_trade_allow_holes: bool,
    pub(crate) tech_trade_loss_allow_holes: bool,
    pub(crate) tech_parasite_allow_holes: bool,
    pub(crate) tech_loss_allow_holes: bool,
    pub(crate) sciencebox_percent: i32,
    pub(crate) tech_penalty_percent: i32,
    pub(crate) tech_leak_percent: i32,
    pub(crate) tech_loss_forgiveness_percent: i32,
    pub(crate) tech_loss_restore_percent: i32,
    pub(crate) tech_lost_recv_percent: i32,
    pub(crate) tech_lost_donor_percent: i32,
    pub(crate) diplomatic_bulb_cost_percent: i32,
    pub(crate) revolution_type: RevolutionType,
    pub(crate) revolution_length: i32,
}

#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VictoryCondition {
    SpaceRace,
    Allied,
    Culture,
    WorldPeace,
}

impl VictoryCondition {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Requirement {
    /// Freeciv universal type, e.g. `Tech`, `Terrain`, or `UnitTypeFlag`,
    /// resolved from the rule file's spelling once at load.
    pub(crate) kind: RequirementKind,
    /// A string, integer, or boolean universal value. Native rulesets use
    /// strings for most names and may use integers for numeric universals.
    pub(crate) name: RequirementValue,
    /// Runtime identity resolved during ruleset loading for stable campaign
    /// catalogs; the original spelling remains available for diagnostics.
    pub(crate) resolved: Option<ResolvedRequirementValue>,
    pub(crate) range: RequirementRange,
    pub(crate) present: bool,
    pub(crate) survives: bool,
    pub(crate) quiet: bool,
    pub(crate) source: RuleSourceLocation,
}

impl Requirement {
    pub(crate) fn matches_name(&self, expected: &str) -> bool {
        match (&self.resolved, &self.name) {
            (Some(ResolvedRequirementValue::Tech(value)), _) => {
                value.name().eq_ignore_ascii_case(expected)
            }
            (Some(ResolvedRequirementValue::Building(value)), _) => {
                value.data().name.eq_ignore_ascii_case(expected)
            }
            (Some(ResolvedRequirementValue::Terrain(value)), _) => {
                value.name().eq_ignore_ascii_case(expected)
            }
            (_, RequirementValue::String(value)) => value.eq_ignore_ascii_case(expected),
            (_, RequirementValue::Integer(value)) => {
                value.to_string().eq_ignore_ascii_case(expected)
            }
            (_, RequirementValue::Bool(value)) => {
                (*value && expected.eq_ignore_ascii_case("TRUE"))
                    || (!*value && expected.eq_ignore_ascii_case("FALSE"))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedRequirementValue {
    Tech(crate::rules::Tech),
    Building(crate::rules::Building),
    Terrain(crate::world::Terrain),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuleSourceLocation {
    pub(crate) file: PathBuf,
    pub(crate) section: String,
    pub(crate) key: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RequirementValue {
    String(String),
    Integer(i64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequirementRange {
    Local,
    Tile,
    CAdjacent,
    Adjacent,
    City,
    Traderoute,
    Continent,
    Player,
    Team,
    Alliance,
    World,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectRecord {
    pub(crate) rule_name: String,
    /// Effect identifiers are defined by the Freeciv engine. Rule files are
    /// resolved to the typed identifier once at load.
    pub(crate) kind: EffectKind,
    pub(crate) value: i64,
    pub(crate) multiplier: Option<String>,
    pub(crate) requirements: Vec<Requirement>,
}

/// A ruleset's effects in file order, indexed by engine effect type so that
/// evaluating one type never visits the records of another.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EffectSet {
    records: Vec<EffectRecord>,
    /// Record positions per [`EffectKind`], preserving file order.
    by_kind: Vec<Vec<usize>>,
}

impl EffectSet {
    /// Effects of one engine type, in ruleset file order.
    pub(crate) fn of_kind(&self, kind: EffectKind) -> impl Iterator<Item = &EffectRecord> + '_ {
        self.by_kind
            .get(kind as usize)
            .into_iter()
            .flatten()
            .map(|&index| &self.records[index])
    }
}

impl From<Vec<EffectRecord>> for EffectSet {
    fn from(records: Vec<EffectRecord>) -> Self {
        let mut by_kind = vec![Vec::new(); EffectKind::ALL.len()];
        for (index, record) in records.iter().enumerate() {
            by_kind[record.kind as usize].push(index);
        }
        Self { records, by_kind }
    }
}

impl std::ops::Deref for EffectSet {
    type Target = [EffectRecord];

    fn deref(&self) -> &Self::Target {
        &self.records
    }
}

impl<'a> IntoIterator for &'a EffectSet {
    type Item = &'a EffectRecord;
    type IntoIter = std::slice::Iter<'a, EffectRecord>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionEnabler {
    pub(crate) rule_name: String,
    pub(crate) action: String,
    pub(crate) actor_requirements: Vec<Requirement>,
    pub(crate) target_requirements: Vec<Requirement>,
    pub(crate) fields: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContentKind {
    Unit,
    Building,
    Technology,
    Terrain,
    Government,
    Nation,
    Style,
    GovernmentMultiplier,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContentRecord {
    pub(crate) kind: ContentKind,
    /// Freeciv's internal identity: `rule_name` when supplied, otherwise the
    /// untranslated `name` value. The section tag is retained separately.
    pub(crate) rule_name: String,
    pub(crate) section: String,
    pub(crate) display_name: Option<String>,
    pub(crate) requirements: Vec<Requirement>,
    /// Technologies and other conditions that make a building obsolete in
    /// this selected ruleset. This is distinct from Civ III catalog data.
    pub(crate) obsolete_by: Vec<Requirement>,
    /// Keep native fields in source order so content adapters can read fields
    /// not yet consumed by gameplay without throwing them away.
    pub(crate) fields: Vec<Entry>,
}

impl ContentRecord {
    /// Read a scalar string field without exposing the native parser's value
    /// representation to gameplay adapters.
    pub(crate) fn string_field(&self, key: &str) -> Result<Option<&str>, String> {
        let Some(entry) = self.fields.iter().find(|entry| entry.key == key) else {
            return Ok(None);
        };
        match &entry.value {
            Value::String(value) => Ok(Some(value)),
            other => Err(format!(
                "{}.{} must be a string, found {other:?}",
                self.section, key
            )),
        }
    }

    /// Read a scalar integer field without exposing the native parser's value
    /// representation to gameplay adapters.
    pub(crate) fn integer_field(&self, key: &str) -> Result<Option<i64>, String> {
        let Some(entry) = self.fields.iter().find(|entry| entry.key == key) else {
            return Ok(None);
        };
        match &entry.value {
            Value::Integer(value) => Ok(Some(*value)),
            other => Err(format!(
                "{}.{} must be an integer, found {other:?}",
                self.section, key
            )),
        }
    }

    /// Read a string-list field without exposing the native parser's value
    /// representation to gameplay adapters. A single quoted string is
    /// normalized to a one-item list, as several ruleset fields permit either
    /// spelling.
    pub(crate) fn string_list_field(&self, key: &str) -> Result<Option<Vec<String>>, String> {
        string_list_field(&self.section, &self.fields, key)
    }

    /// Read a table with one string key column and one integer value column.
    /// Freeciv terrain `extra_settings` uses this shape for per-extra
    /// removal times; gameplay adapters can inspect it without accessing the
    /// section-file parser's generic `Value` enum.
    pub(crate) fn string_integer_table_field(
        &self,
        key: &str,
        string_column: &str,
        integer_column: &str,
    ) -> Result<Option<Vec<(String, i64)>>, String> {
        let Some(entry) = self.fields.iter().find(|entry| entry.key == key) else {
            return Ok(None);
        };
        let Value::Table { columns, rows } = &entry.value else {
            return Err(format!(
                "{}.{} must be a table, found {:?}",
                self.section, key, entry.value
            ));
        };
        let column_index = |name: &str| {
            columns
                .iter()
                .position(|column| column.eq_ignore_ascii_case(name))
        };
        let string_index = column_index(string_column).ok_or_else(|| {
            format!(
                "{}.{} is missing the {string_column:?} column",
                self.section, key
            )
        })?;
        let integer_index = column_index(integer_column).ok_or_else(|| {
            format!(
                "{}.{} is missing the {integer_column:?} column",
                self.section, key
            )
        })?;
        rows.iter()
            .enumerate()
            .map(|(row_index, row)| {
                let string_value = row.get(string_index).ok_or_else(|| {
                    format!("{}.{} row {} is missing {string_column:?}", self.section, key, row_index + 1)
                })?;
                let integer_value = row.get(integer_index).ok_or_else(|| {
                    format!("{}.{} row {} is missing {integer_column:?}", self.section, key, row_index + 1)
                })?;
                let Value::String(name) = string_value else {
                    return Err(format!(
                        "{}.{} row {} {string_column:?} must be a string, found {string_value:?}",
                        self.section,
                        key,
                        row_index + 1
                    ));
                };
                let Value::Integer(value) = integer_value else {
                    return Err(format!(
                        "{}.{} row {} {integer_column:?} must be an integer, found {integer_value:?}",
                        self.section,
                        key,
                        row_index + 1
                    ));
                };
                Ok((name.clone(), *value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuleRecord {
    pub(crate) file: String,
    pub(crate) namespace: String,
    pub(crate) section: String,
    pub(crate) rule_name: String,
    pub(crate) display_name: Option<String>,
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) fields: Vec<Entry>,
}

impl RuleRecord {
    /// Read a string field without exposing the native parser's value
    /// representation to gameplay adapters.
    pub(crate) fn string_field(&self, key: &str) -> Result<Option<&str>, String> {
        let Some(entry) = self
            .fields
            .iter()
            .find(|entry| entry.key.eq_ignore_ascii_case(key))
        else {
            return Ok(None);
        };
        match &entry.value {
            Value::String(value) => Ok(Some(value)),
            other => Err(format!(
                "{}.{} must be a string, found {other:?}",
                self.section, key
            )),
        }
    }

    /// Read an integer field without exposing the native parser's value
    /// representation to gameplay adapters.
    pub(crate) fn integer_field(&self, key: &str) -> Result<Option<i64>, String> {
        let Some(entry) = self
            .fields
            .iter()
            .find(|entry| entry.key.eq_ignore_ascii_case(key))
        else {
            return Ok(None);
        };
        match &entry.value {
            Value::Integer(value) => Ok(Some(*value)),
            other => Err(format!(
                "{}.{} must be an integer, found {other:?}",
                self.section, key
            )),
        }
    }

    /// Read a string-list field without exposing the native parser's value
    /// representation to gameplay adapters. A single quoted string is
    /// normalized to a one-item list, as several ruleset fields permit either
    /// spelling.
    pub(crate) fn string_list_field(&self, key: &str) -> Result<Option<Vec<String>>, String> {
        string_list_field(&self.section, &self.fields, key)
    }
}

fn string_list_field(
    section: &str,
    fields: &[Entry],
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    let mut entries = fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key));
    let Some(entry) = entries.next() else {
        return Ok(None);
    };
    if entries.next().is_some() {
        return Err(format!("{section}.{key} occurs more than once"));
    }
    match &entry.value {
        Value::String(value) => Ok(Some(vec![value.clone()])),
        Value::List(values) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                other => Err(format!(
                    "{section}.{key} must be a string list, found item {other:?}"
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        other => Err(format!(
            "{section}.{key} must be a string list, found {other:?}"
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadError {
    pub(crate) file: Option<PathBuf>,
    pub(crate) section: Option<String>,
    pub(crate) key: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}", file.display())?;
        }
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
        }
        if let Some(section) = &self.section {
            write!(f, " [{section}")?;
            if let Some(key) = &self.key {
                write!(f, ".{key}")?;
            }
            write!(f, "]")?;
        }
        if self.file.is_some() || self.section.is_some() {
            write!(f, ": ")?;
        }
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LoadError {}

impl RulesetData {
    fn build_lookup_indexes(&mut self) {
        self.action_index = RuleNameIndex::new(
            self.action_enablers
                .iter()
                .enumerate()
                .map(|(index, enabler)| (enabler.action.clone(), index)),
        );
    }

    pub(crate) fn action_enabled<A: RequirementContext, T: RequirementContext>(
        &self,
        action: &str,
        actor: &A,
        target: &T,
    ) -> Result<bool, ActionEvaluationError> {
        evaluate_action_enablers(
            self.action_index
                .get(action)
                .iter()
                .map(|&index| &self.action_enablers[index]),
            action,
            actor,
            target,
        )
    }

    /// Load the generic engine layer from `src/rules/engine_data/`.
    /// Native content catalogs construct independently, without import bindings.
    #[cfg(test)]
    pub(crate) fn load_civcraft(engine_dir: &Path) -> Result<Self, LoadError> {
        Self::load_from_directory(engine_dir, engine_dir, "civcraft")
    }

    /// Cache the single CivCraft campaign package once.
    pub(crate) fn campaign_default() -> &'static Self {
        static RULESET: OnceLock<RulesetData> = OnceLock::new();
        RULESET.get_or_init(|| {
            Self::load_bundled_campaign()
                .unwrap_or_else(|error| panic!("failed to load campaign ruleset: {error}"))
        })
    }

    #[cfg(test)]
    pub(crate) fn with_test_units(&self, units_source: &str) -> Self {
        let mut files = self.files.clone();
        let parsed = super::section_file::parse(units_source).expect("valid test units.ruleset");
        files.push(LoadedRuleFile {
            name: "units.ruleset".to_owned(),
            path: PathBuf::from("engine_data/units.ruleset"),
            source: RuleFileSource::SelectedRuleset,
            parsed,
        });
        Self::from_files(&self.name, files).expect("valid campaign + test units ruleset")
    }

    #[cfg(test)]
    pub(crate) fn test_unit_record_from_ruleset(source: &str, section: &str) -> ContentRecord {
        let rules = Self::campaign_default().with_test_units(source);
        rules
            .content
            .into_iter()
            .find(|record| {
                record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(section)
            })
            .expect("test unit section")
    }
}

impl RulesetData {
    /// The government-only contribution. Contextual effects remain part of
    /// the full evaluator for callers that supply a player or city.
    pub(crate) fn government_base_effect(&self, government: &str, kind: EffectKind) -> i64 {
        self.effects
            .of_kind(kind)
            .filter(|effect| {
                matches!(&effect.requirements[..], [requirement]
                    if requirement.kind == RequirementKind::Gov
                        && requirement.range == RequirementRange::Player
                        && requirement.present
                        && requirement.matches_name(government))
            })
            .map(|effect| {
                assert!(
                    effect.multiplier.is_none(),
                    "a government base effect must be a scalar"
                );
                effect.value
            })
            .sum()
    }
}

/// The one shared logical evaluator for Freeciv requirement conjunctions.
/// Implementations resolve a universal and its range against simulation state;
/// `present=false` negation is applied here for every subsystem.
pub(crate) trait RequirementContext {
    /// Resolve `kind` and `name` using Freeciv's case-insensitive rule-name
    /// lookup, the requested range, and the `survives` flag. Return an error
    /// when CivCraft has no resolver for this valid Freeciv requirement yet;
    /// unsupported must never collapse into false.
    fn is_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RequirementEvaluationError {
    Unsupported {
        kind: String,
        name: RequirementValue,
    },
}

pub(crate) fn requirements_met(
    requirements: &[Requirement],
    context: &impl RequirementContext,
) -> Result<bool, RequirementEvaluationError> {
    for requirement in requirements {
        if context.is_present(requirement)? != requirement.present {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Freeciv enablers are alternatives: one matching enabler enables an action.
/// Within each enabler, actor and target requirement lists are both required.
#[cfg(test)]
pub(crate) fn action_enabled<A, T>(
    enablers: &[ActionEnabler],
    action: &str,
    actor_context: &A,
    target_context: &T,
) -> Result<bool, ActionEvaluationError>
where
    A: RequirementContext,
    T: RequirementContext,
{
    evaluate_action_enablers(enablers.iter(), action, actor_context, target_context)
}

fn evaluate_action_enablers<'a, A: RequirementContext, T: RequirementContext>(
    enablers: impl IntoIterator<Item = &'a ActionEnabler>,
    action: &str,
    actor_context: &A,
    target_context: &T,
) -> Result<bool, ActionEvaluationError> {
    if !identifiers::ACTION_NAMES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(action))
    {
        return Err(ActionEvaluationError::UnknownAction(action.to_owned()));
    }
    for enabler in enablers {
        if enabler.action.eq_ignore_ascii_case(action)
            && requirements_met(&enabler.actor_requirements, actor_context)?
            && requirements_met(&enabler.target_requirements, target_context)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActionEvaluationError {
    UnknownAction(String),
    Requirement(RequirementEvaluationError),
}

impl From<RequirementEvaluationError> for ActionEvaluationError {
    fn from(error: RequirementEvaluationError) -> Self {
        Self::Requirement(error)
    }
}

/// Supplies the player-specific multiplier factors consumed by Freeciv
/// effects. The context returns a percentage (100 means unchanged).
pub(crate) trait EffectContext: RequirementContext {
    fn multiplier_percent(&self, multiplier: &str) -> Option<i64>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EffectEvaluationError {
    MissingMultiplier(String),
    Overflow,
    Requirement(RequirementEvaluationError),
}

impl From<RequirementEvaluationError> for EffectEvaluationError {
    fn from(error: RequirementEvaluationError) -> Self {
        Self::Requirement(error)
    }
}

/// Sum active effects of one engine-defined type using Freeciv's integer
/// percentage adjustment. Callers share this aggregation path and only supply
/// state-specific requirement and multiplier lookups.
pub(crate) fn effect_value(
    effects: &EffectSet,
    kind: EffectKind,
    context: &impl EffectContext,
) -> Result<i64, EffectEvaluationError> {
    let mut total = 0_i64;
    for effect in effects.of_kind(kind) {
        if !requirements_met(&effect.requirements, context)? {
            continue;
        }
        let contribution = match &effect.multiplier {
            Some(multiplier) => {
                let percent = context
                    .multiplier_percent(multiplier)
                    .ok_or_else(|| EffectEvaluationError::MissingMultiplier(multiplier.clone()))?;
                effect
                    .value
                    .checked_mul(percent)
                    .ok_or(EffectEvaluationError::Overflow)?
                    / 100
            }
            None => effect.value,
        };
        total = total
            .checked_add(contribution)
            .ok_or(EffectEvaluationError::Overflow)?;
    }
    Ok(total)
}

/// Evaluate one effect type under the campaign ruleset.
///
/// The campaign ruleset is validated at load, so an evaluation error means a
/// requirement resolver lacks a universal the ruleset uses. That is a
/// programming error rather than a recoverable game state.
pub(crate) fn campaign_effect(kind: EffectKind, context: &impl EffectContext) -> i64 {
    effect_value(&RulesetData::campaign_default().effects, kind, context)
        .unwrap_or_else(|error| panic!("could not evaluate {kind} effect: {error:?}"))
}

impl LoadError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            file: None,
            section: None,
            key: None,
            line: None,
            message: message.into(),
        }
    }

    fn at(file: &Path, message: impl Into<String>) -> Self {
        Self {
            file: Some(file.to_owned()),
            section: None,
            key: None,
            line: None,
            message: message.into(),
        }
    }

    fn at_line(
        file: &Path,
        section: &str,
        key: &str,
        line: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: Some(file.to_owned()),
            section: Some(section.to_owned()),
            key: Some(key.to_owned()),
            line: Some(line),
            message: message.into(),
        }
    }

    fn in_section(file: &Path, section: &Section, message: impl Into<String>) -> Self {
        Self {
            file: Some(file.to_owned()),
            section: Some(section.name.clone()),
            key: None,
            line: None,
            message: message.into(),
        }
    }

    fn in_key(file: &Path, section: &Section, key: &str, message: impl Into<String>) -> Self {
        let line = section
            .entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.line);
        Self {
            file: Some(file.to_owned()),
            section: Some(section.name.clone()),
            key: Some(key.to_owned()),
            line,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::section_file::{Entry, Value};
    use super::{
        ActionEnabler, ContentKind, EffectContext, EffectKind, EffectRecord, EffectSet,
        LoadedRuleFile, RULESET_FILES, Requirement, RequirementContext, RequirementEvaluationError,
        RequirementKind, RequirementRange, RequirementValue, RuleFileSource, RuleSourceLocation,
        RulesetData, action_enabled, effect_value, requirements_met,
    };
    use std::path::PathBuf;

    fn parsed_file(name: &str, source: &str) -> LoadedRuleFile {
        LoadedRuleFile {
            name: name.to_owned(),
            path: PathBuf::from(name),
            source: RuleFileSource::SelectedRuleset,
            parsed: super::section_file::parse(source).unwrap(),
        }
    }

    fn files_with_overrides(overrides: &[(&str, &str)]) -> Vec<LoadedRuleFile> {
        let mut files = RULESET_FILES
            .iter()
            .map(|name| parsed_file(name, ""))
            .collect::<Vec<_>>();
        for (name, source) in overrides {
            if let Some(index) = files.iter().position(|file| file.name == *name) {
                files[index] = parsed_file(name, source);
            } else {
                files.push(parsed_file(name, source));
            }
        }
        files
    }

    struct Context {
        present: Vec<(String, RequirementValue, RequirementRange)>,
        multiplier: Option<(String, i64)>,
    }

    impl RequirementContext for Context {
        fn is_present(
            &self,
            requirement: &Requirement,
        ) -> Result<bool, RequirementEvaluationError> {
            if !["Tech", "Gov", "Terrain", "Building"]
                .iter()
                .any(|kind| kind.eq_ignore_ascii_case(requirement.kind.name()))
            {
                return Err(RequirementEvaluationError::Unsupported {
                    kind: requirement.kind.name().to_owned(),
                    name: requirement.name.clone(),
                });
            }
            Ok(self.present.iter().any(|(kind, name, range)| {
                kind.eq_ignore_ascii_case(requirement.kind.name())
                    && name == &requirement.name
                    && range == &requirement.range
            }))
        }
    }

    impl EffectContext for Context {
        fn multiplier_percent(&self, multiplier: &str) -> Option<i64> {
            self.multiplier
                .as_ref()
                .filter(|(name, _)| name == multiplier)
                .map(|(_, percent)| *percent)
        }
    }

    fn req(kind: &str, name: &str, present: bool) -> Requirement {
        Requirement {
            kind: RequirementKind::parse(kind).expect("known requirement type"),
            name: RequirementValue::String(name.to_owned()),
            resolved: None,
            range: RequirementRange::Player,
            present,
            survives: false,
            quiet: false,
            source: RuleSourceLocation {
                file: PathBuf::new(),
                section: "fixture".to_owned(),
                key: "reqs".to_owned(),
                line: 1,
            },
        }
    }

    #[test]
    fn shared_requirement_evaluator_handles_conjunction_and_negation() {
        let context = Context {
            present: vec![(
                "Tech".to_owned(),
                RequirementValue::String("Writing".to_owned()),
                RequirementRange::Player,
            )],
            multiplier: None,
        };
        assert!(
            requirements_met(
                &[req("Tech", "Writing", true), req("Gov", "Anarchy", false)],
                &context
            )
            .unwrap()
        );
        assert_eq!(
            requirements_met(&[req("Tech", "Writing", false)], &context),
            Ok(false)
        );
    }

    #[test]
    fn recognized_but_unimplemented_requirement_does_not_become_absent() {
        let context = Context {
            present: vec![],
            multiplier: None,
        };
        assert!(matches!(
            requirements_met(&[req("Activity", "Road", true)], &context),
            Err(RequirementEvaluationError::Unsupported { .. })
        ));
    }

    #[test]
    fn indexed_campaign_actions_preserve_linear_lookup_results() {
        let rules = RulesetData::campaign_default();
        let context = Context {
            present: vec![],
            multiplier: None,
        };
        for action in super::identifiers::ACTION_NAMES
            .iter()
            .copied()
            .chain(["not an action"])
        {
            for name in [action.to_owned(), action.to_ascii_uppercase()] {
                assert_eq!(
                    rules.action_enabled(&name, &context, &context),
                    action_enabled(&rules.action_enablers, &name, &context, &context)
                );
            }
        }
    }

    #[test]
    fn action_enablers_are_alternatives_with_actor_and_target_conjunctions() {
        let enablers = vec![
            ActionEnabler {
                rule_name: "requires_iron".to_owned(),
                action: "Found City".to_owned(),
                actor_requirements: vec![req("Tech", "Iron Working", true)],
                target_requirements: vec![],
                fields: vec![],
            },
            ActionEnabler {
                rule_name: "requires_writing".to_owned(),
                action: "Found City".to_owned(),
                actor_requirements: vec![req("Tech", "Writing", true)],
                target_requirements: vec![req("Terrain", "Ocean", false)],
                fields: vec![],
            },
        ];
        let mut indexed = RulesetData::campaign_default().clone();
        indexed.action_enablers = enablers.clone();
        indexed.build_lookup_indexes();
        let actor = Context {
            present: vec![(
                "Tech".to_owned(),
                RequirementValue::String("Writing".to_owned()),
                RequirementRange::Player,
            )],
            multiplier: None,
        };
        let target = Context {
            present: vec![],
            multiplier: None,
        };
        for action in ["found city", "Build Road", "Not an action"] {
            assert_eq!(
                indexed.action_enabled(action, &actor, &target),
                action_enabled(&enablers, action, &actor, &target)
            );
        }
        assert_eq!(
            action_enabled(&enablers, "found city", &actor, &target),
            Ok(true)
        );
        assert_eq!(
            action_enabled(&enablers, "Build Road", &actor, &target),
            Ok(false)
        );
        assert!(matches!(
            action_enabled(&enablers, "Not an action", &actor, &target),
            Err(super::ActionEvaluationError::UnknownAction(_))
        ));
    }

    #[test]
    fn shared_effect_evaluator_sums_active_values_and_applies_percent_multiplier() {
        let effects = EffectSet::from(vec![
            EffectRecord {
                rule_name: "plain".to_owned(),
                kind: EffectKind::OutputBonus,
                value: 5,
                multiplier: None,
                requirements: vec![req("Tech", "Writing", true)],
            },
            EffectRecord {
                rule_name: "scaled".to_owned(),
                kind: EffectKind::OutputBonus,
                value: 10,
                multiplier: Some("Science".to_owned()),
                requirements: vec![],
            },
            EffectRecord {
                rule_name: "inactive".to_owned(),
                kind: EffectKind::OutputBonus,
                value: 100,
                multiplier: None,
                requirements: vec![req("Tech", "Mobile Warfare", true)],
            },
        ]);
        let context = Context {
            present: vec![(
                "Tech".to_owned(),
                RequirementValue::String("Writing".to_owned()),
                RequirementRange::Player,
            )],
            multiplier: Some(("Science".to_owned(), 150)),
        };
        assert_eq!(
            EffectKind::parse("output_bonus"),
            Some(EffectKind::OutputBonus)
        );
        assert_eq!(EffectKind::parse("Unrelated"), None);
        assert_eq!(
            effect_value(&effects, EffectKind::OutputBonus, &context),
            Ok(20)
        );
        assert_eq!(effect_value(&effects, EffectKind::Airlift, &context), Ok(0));
    }

    #[test]
    fn public_model_keeps_rule_names_separate_from_content_positions() {
        let record = super::ContentRecord {
            kind: ContentKind::Technology,
            rule_name: "mobile_warfare".to_owned(),
            section: "advance_mobile_warfare".to_owned(),
            display_name: Some("Mobile Warfare".to_owned()),
            requirements: vec![],
            obsolete_by: vec![],
            fields: vec![],
        };
        assert_eq!(record.rule_name, "mobile_warfare");
    }

    #[test]
    fn content_record_reads_validated_string_integer_table_fields() {
        let record = super::ContentRecord {
            kind: ContentKind::Terrain,
            rule_name: "grassland".to_owned(),
            section: "terrain_grassland".to_owned(),
            display_name: Some("Grassland".to_owned()),
            requirements: vec![],
            obsolete_by: vec![],
            fields: vec![Entry {
                key: "extra_settings".to_owned(),
                value: Value::Table {
                    columns: vec!["extra".to_owned(), "removal_time".to_owned()],
                    rows: vec![vec![
                        Value::String("Pollution".to_owned()),
                        Value::Integer(3),
                    ]],
                },
                line: 1,
            }],
        };
        assert_eq!(
            record
                .string_integer_table_field("extra_settings", "extra", "removal_time")
                .unwrap(),
            Some(vec![("Pollution".to_owned(), 3)])
        );
        assert!(
            record
                .string_integer_table_field("extra_settings", "extra", "missing")
                .is_err()
        );
    }

    #[test]
    fn loader_builds_typed_settings_effects_actions_and_content() {
        let files = files_with_overrides(&[
            (
                "game.ruleset",
                "[settings]\nset =\n { \"name\", \"value\"\n \"citymindist\", 3\n \"generator\", \"FRACTAL\"\n }\n[clause_advance]\ntype = \"Advance\"\n",
            ),
            (
                "effects.ruleset",
                "[effect_bonus]\ntype = \"Output_Bonus\"\nvalue = 5\nreqs =\n { \"type\", \"name\", \"range\"\n \"Building\", \"civcraft_library\", \"City\"\n }\n",
            ),
            (
                "buildings.ruleset",
                "[building_library]\nrule_name = \"civcraft_library\"\nname = \"Library\"\nreqs =\n { \"type\", \"name\", \"range\"\n \"Tech\", \"civ3_literacy\", \"Player\"\n }\nobsolete_by =\n { \"type\", \"name\", \"range\", \"survives\"\n \"Tech\", \"civ3_literacy\", \"Player\", FALSE\n }\n",
            ),
            (
                "units.ruleset",
                "[unit_scout]\nrule_name = \"civ3_scout\"\nname = \"Scout\"\n",
            ),
            (
                "techs.ruleset",
                "[advance_writing]\nrule_name = \"civ3_literacy\"\nname = \"Writing\"\n[advance_alphabet]\nname = \"Alphabet\"\n",
            ),
            (
                "terrain.ruleset",
                "[terrain_ocean]\nname = \"Ocean\"\n[resource_iron]\nname = \"Iron\"\nreqs =\n { \"type\", \"name\", \"range\"\n \"Terrain\", \"Ocean\", \"tile\"\n }\n[extra_mine]\nname = _(\"?extra:Mine\")\n[unitclass_land]\nname = \"Land\"\n",
            ),
        ]);
        let rules = RulesetData::from_files_with_enablers("fixture", files, Vec::new()).unwrap();

        assert!(
            rules
                .rule_records
                .iter()
                .any(|record| { record.namespace == "clause" && record.rule_name == "advance" })
        );
        assert_eq!(rules.effects.len(), 1);
        assert_eq!(
            rules.effects[0].requirements[0].range,
            RequirementRange::City
        );
        assert!(rules.content.iter().any(|record| {
            record.kind == ContentKind::Building
                && record.rule_name == "civcraft_library"
                && record.obsolete_by.len() == 1
                && record.obsolete_by[0].kind == RequirementKind::Tech
                && record.obsolete_by[0].range == RequirementRange::Player
                && !record.obsolete_by[0].survives
        }));
        assert!(rules.content.iter().any(|record| {
            record.kind == ContentKind::Unit && record.rule_name == "civ3_scout"
        }));
        assert!(rules.content.iter().any(|record| {
            record.kind == ContentKind::Technology && record.rule_name == "civ3_literacy"
        }));
        assert!(rules.content.iter().any(|record| {
            record.kind == ContentKind::Technology
                && record.rule_name == "Alphabet"
                && record.section == "advance_alphabet"
        }));
        assert!(rules.rule_records.iter().any(|record| {
            record.namespace == "resource"
                && record.rule_name == "Iron"
                && record.requirements[0].range == RequirementRange::Tile
        }));
        assert!(rules.rule_records.iter().any(|record| {
            record.namespace == "extra"
                && record.rule_name == "Mine"
                && record.display_name.as_deref() == Some("Mine")
        }));
        assert!(
            rules
                .rule_records
                .iter()
                .any(|record| { record.namespace == "unit_class" && record.rule_name == "Land" })
        );
    }

    #[test]
    fn loader_rejects_unknown_freeciv_engine_identifiers() {
        let invalid_cases = [
            (
                "buildings.ruleset",
                "[building_test]\nname = \"Test\"\nreqs =\n { \"type\", \"name\", \"range\"\n \"NotAUniversal\", \"value\", \"Player\"\n }\n",
                "unknown Freeciv requirement type",
            ),
            (
                "effects.ruleset",
                "[effect_test]\ntype = \"Not_An_Effect\"\n",
                "unknown Freeciv effect type",
            ),
        ];

        for (file, contents, expected_error) in invalid_cases {
            let files = files_with_overrides(&[(file, contents)]);
            let error =
                RulesetData::from_files_with_enablers("fixture", files, Vec::new()).unwrap_err();
            assert!(
                error.message.contains(expected_error),
                "unexpected error for {file}: {error}"
            );
        }

        let files = files_with_overrides(&[
            ("techs.ruleset", "[advance_writing]\nname = \"Writing\"\n"),
            (
                "buildings.ruleset",
                "[building_library]\nname = \"Library\"\nreqs =\n { \"type\", \"name\", \"range\"\n \"Tech\", \"Writng\", \"Player\"\n }\n",
            ),
        ]);
        let error =
            RulesetData::from_files_with_enablers("fixture", files, Vec::new()).unwrap_err();
        assert!(error.message.contains("unknown or unsupported value"));
        assert_eq!(error.section.as_deref(), Some("building_library"));
    }

    #[test]
    fn campaign_enablers_name_known_actions() {
        let enablers = &RulesetData::campaign_default().action_enablers;
        assert_eq!(enablers.len(), 89);
        for enabler in enablers {
            assert!(
                super::identifiers::ACTION_NAMES
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&enabler.action)),
                "enabler {} names unknown action {:?}",
                enabler.rule_name,
                enabler.action
            );
        }
    }
}
