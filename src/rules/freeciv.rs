//! Typed Freeciv ruleset data layered over the native section-file syntax.
//!
//! This module owns data shapes and shared conditional evaluation. It does not
//! own gameplay transitions; simulation subsystems provide a context that
//! resolves rule universals against their current state.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const FREECIV_OCEAN_DEPTH_MINIMUM: i32 = 1;
const FREECIV_OCEAN_DEPTH_MAXIMUM: i32 = 100;

#[cfg(test)]
use super::freeciv_ini;
use super::freeciv_ini::{Entry, Section, SectionFile, Value};

#[path = "freeciv/civ3_content.rs"]
mod civ3_content;
#[path = "freeciv/civil_war.rs"]
mod civil_war;
#[path = "freeciv/combat.rs"]
mod combat;
#[path = "freeciv/identifiers.rs"]
mod identifiers;
#[path = "freeciv/loader.rs"]
mod loader;
#[path = "freeciv/veterancy.rs"]
mod veterancy;
#[allow(unused_imports)]
pub(crate) use civ3_content::{
    Civ3ContentCatalog, SourceContentRecord, SourceField, SourceRecordIdentity,
};
pub(crate) use civil_war::CivilWarRulesetSettings;
pub(crate) use combat::{
    CombatBonusKind, CombatRulesetSettings, TransportDefense, UnitCombatBonus,
};
#[cfg(test)]
pub(crate) use loader::RULESET_FILES;
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
    pub(crate) settings: Vec<SettingRecord>,
    pub(crate) parameters: Vec<RuleParameter>,
    pub(crate) effects: Vec<EffectRecord>,
    pub(crate) actions: Vec<ActionRecord>,
    pub(crate) action_enablers: Vec<ActionEnabler>,
    pub(crate) content: Vec<ContentRecord>,
    /// Source-authoritative Civ III records supplied to the explicit campaign
    /// adapter. These remain separate from Freeciv's native records until a
    /// runtime crosswalk binds their identities to stable CivCraft IDs.
    pub(crate) source_content: Vec<SourceContentRecord>,
    /// Ordered records from Freeciv namespaces that do not have a dedicated
    /// CivCraft adapter yet (extras, resources, roads, disasters, and others).
    pub(crate) rule_records: Vec<RuleRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingRecord {
    pub(crate) name: String,
    pub(crate) value: Value,
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

    pub(crate) fn integer_list_value(&self) -> Result<Option<Vec<i64>>, String> {
        match &self.value {
            Value::List(values) => values
                .iter()
                .map(|value| match value {
                    Value::Integer(value) => Ok(*value),
                    other => Err(format!(
                        "{}.{} must be an integer list, found item {other:?}",
                        self.section, self.key
                    )),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            other => Err(format!(
                "{}.{} must be an integer list, found {other:?}",
                self.section, self.key
            )),
        }
    }

    pub(crate) fn string_value(&self) -> Result<Option<&str>, String> {
        match &self.value {
            Value::String(value) => Ok(Some(value)),
            other => Err(format!(
                "{}.{} must be a string, found {other:?}",
                self.section, self.key
            )),
        }
    }

    pub(crate) fn string_list_value(&self) -> Result<Option<Vec<String>>, String> {
        match &self.value {
            Value::String(value) => Ok(Some(vec![value.clone()])),
            Value::List(values) => values
                .iter()
                .map(|value| match value {
                    Value::String(value) => Ok(value.clone()),
                    other => Err(format!(
                        "{}.{} must be a string list, found item {other:?}",
                        self.section, self.key
                    )),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            other => Err(format!(
                "{}.{} must be a string list, found {other:?}",
                self.section, self.key
            )),
        }
    }

    pub(crate) fn boolean_value(&self) -> Result<Option<bool>, String> {
        match &self.value {
            Value::Bool(value) => Ok(Some(*value)),
            other => Err(format!(
                "{}.{} must be a boolean, found {other:?}",
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
pub(crate) enum StartPositionSetting {
    Default,
    Single,
    TwoOrThree,
    All,
    Variable,
}

impl StartPositionSetting {
    fn parse(value: &str) -> Result<Self, LoadError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DEFAULT" => Ok(Self::Default),
            "SINGLE" => Ok(Self::Single),
            "2OR3" => Ok(Self::TwoOrThree),
            "ALL" => Ok(Self::All),
            "VARIABLE" => Ok(Self::Variable),
            _ => Err(LoadError::new(format!(
                "campaign `startpos` setting has unknown value {value:?}"
            ))),
        }
    }

    /// Generator 5 adds one to `extra_div`; the pinned server chooses zero
    /// extra divisions only for DEFAULT and ALL start placement.
    pub(crate) fn fractal_divisions(self, player_count: usize) -> i32 {
        let extra_divisions = match self {
            Self::Default | Self::All => 0,
            Self::Single | Self::TwoOrThree | Self::Variable => player_count as i32,
        };
        5 + 1 + extra_divisions
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OceanDepthSettings {
    pub(crate) lake: i32,
    pub(crate) lake_unsafe_coast: bool,
    pub(crate) ocean: i32,
    pub(crate) ocean_unsafe_coast: bool,
    pub(crate) deep_ocean: i32,
    pub(crate) deep_ocean_unsafe_coast: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HutPlacementSettings {
    pub(crate) land_only: bool,
    pub(crate) minimum_latitude: i32,
    pub(crate) maximum_latitude: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MapGenerationSettings {
    pub(crate) landmass_percent: i32,
    pub(crate) startpos: StartPositionSetting,
    pub(crate) temperature: i32,
    pub(crate) steepness: i32,
    pub(crate) wetness: i32,
    /// Freeciv server `specials` setting, parts per thousand.
    pub(crate) specials_per_thousand: i32,
    /// Freeciv server `huts` setting, huts per thousand tiles.
    pub(crate) huts_per_thousand: i32,
    /// C2C3 terrain parameter controlling ocean-resource eligibility.
    pub(crate) ocean_resources: bool,
    pub(crate) hut_placement: HutPlacementSettings,
    pub(crate) ocean_depths: OceanDepthSettings,
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
pub(crate) enum HappyBorders {
    Disabled,
    National,
    Allied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitUpkeepOutput {
    Food,
    Shield,
    Gold,
}

impl UnitUpkeepOutput {
    fn parameter_name(self) -> &'static str {
        match self {
            Self::Food => "food",
            Self::Shield => "shield",
            Self::Gold => "gold",
        }
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnitHappinessRulesetSettings {
    pub(crate) borders_enabled: bool,
    pub(crate) happy_borders: HappyBorders,
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

impl MapGenerationSettings {
    pub(crate) fn ocean_coverage_percent(self) -> i32 {
        100 - self.landmass_percent
    }

    pub(crate) fn shore_level(self) -> i32 {
        1000 * self.ocean_coverage_percent() / 100
    }
}

/// Map-generation properties attached to one non-ocean Freeciv terrain.
/// Values retain the C2C3 ruleset's integer weights; omitted properties are
/// represented as zero, matching Freeciv's terrain-ruleset loader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerrainMapgenProperties {
    pub(crate) rule_name: String,
    pub(crate) section: String,
    pub(crate) starter: bool,
    pub(crate) unsafe_coast: bool,
    /// C2C3 tile outputs and activity values consumed by Freeciv's
    /// `server/generator/startpos.c::get_tile_value` scoring formula.
    pub(crate) food: i32,
    pub(crate) shield: i32,
    pub(crate) trade: i32,
    pub(crate) road_trade_incr_pct: i32,
    pub(crate) road_time: i32,
    pub(crate) irrigation_food_incr: i32,
    pub(crate) irrigation_time: i32,
    pub(crate) mining_shield_incr: i32,
    pub(crate) mining_time: i32,
    pub(crate) mountainous: i32,
    pub(crate) green: i32,
    pub(crate) foliage: i32,
    pub(crate) tropical: i32,
    pub(crate) temperate: i32,
    pub(crate) cold: i32,
    pub(crate) frozen: i32,
    pub(crate) wet: i32,
    pub(crate) dry: i32,
    pub(crate) ocean_depth: i32,
}

/// Ruleset-wide output modifiers used while evaluating Freeciv start sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StartPositionScoringSettings {
    pub(crate) road_trade_increment: i32,
    pub(crate) irrigation_percent: i32,
    pub(crate) desert_river_irrigation_percent: i32,
    pub(crate) mining_percent: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VictoryCondition {
    SpaceRace,
    Allied,
    Culture,
    WorldPeace,
}

impl VictoryCondition {
    fn rule_name(self) -> &'static str {
        match self {
            Self::SpaceRace => "SPACERACE",
            Self::Allied => "ALLIED",
            Self::Culture => "CULTURE",
            Self::WorldPeace => "WORLDPEACE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Requirement {
    /// Freeciv universal type, e.g. `Tech`, `Terrain`, or `UnitTypeFlag`.
    pub(crate) kind: String,
    /// A string, integer, or boolean universal value. Native rulesets use
    /// strings for most names and may use integers for numeric universals.
    pub(crate) name: RequirementValue,
    pub(crate) range: RequirementRange,
    pub(crate) present: bool,
    pub(crate) survives: bool,
    pub(crate) quiet: bool,
    pub(crate) source: RuleSourceLocation,
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
    /// Effect identifiers are defined by the Freeciv engine, so keep the
    /// selected ruleset's spelling intact for engine-side dispatch.
    pub(crate) kind: String,
    pub(crate) value: i64,
    pub(crate) multiplier: Option<String>,
    pub(crate) requirements: Vec<Requirement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionRecord {
    pub(crate) rule_name: String,
    pub(crate) action: String,
    pub(crate) fields: Vec<Entry>,
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

/// A selected-ruleset government, retaining stable ruleset identity and the
/// requirements and effect attributes that define its behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GovernmentRulesetRecord {
    /// Stable Freeciv rule name used by requirements and saves.
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) section: String,
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) flags: Vec<String>,
    pub(crate) ai_better: Option<String>,
    /// Effects whose government requirements match this government. Other
    /// requirements are preserved and evaluated by the shared effect engine.
    pub(crate) attributes: Vec<EffectRecord>,
}

impl GovernmentRulesetRecord {
    /// Evaluate an effect such as `Max_Rates` using Freeciv's shared evaluator.
    /// The caller supplies the simulation-specific requirements and multiplier
    /// context; this record supplies the selected ruleset's government-scoped
    /// effect definitions.
    pub(crate) fn effect_value(
        &self,
        kind: &str,
        context: &impl EffectContext,
    ) -> Result<i64, EffectEvaluationError> {
        effect_value(&self.attributes, kind, context)
    }
}

/// Government records in selected ruleset order, plus the rule selected for
/// Freeciv's temporary government during a revolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GovernmentRulesetCatalog {
    pub(crate) during_revolution_id: String,
    pub(crate) governments: Vec<GovernmentRulesetRecord>,
}

impl GovernmentRulesetCatalog {
    pub(crate) fn get(&self, id: &str) -> Option<&GovernmentRulesetRecord> {
        self.governments
            .iter()
            .find(|government| government.id.eq_ignore_ascii_case(id))
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
    /// Load the Freeciv mechanics ruleset and attach Civ III Conquests source
    /// records from an explicitly selected catalog directory.
    pub(crate) fn load_with_civ3_catalog(
        data_root: &Path,
        ruleset_name: &str,
        civ3_catalog_dir: &Path,
    ) -> Result<Self, LoadError> {
        let mut rules = Self::load(data_root, ruleset_name)?;
        let source_catalog = Civ3ContentCatalog::load(civ3_catalog_dir)
            .map_err(|error| LoadError::new(error.to_string()))?;
        rules.source_content = source_catalog.records;
        Ok(rules)
    }

    /// The current single-player campaign uses the CivCraft-owned C2C3
    /// baseline. Cache the parsed data once; later ruleset selection can move
    /// this ownership into a campaign configuration without changing callers.
    pub(crate) fn campaign_default() -> &'static Self {
        static RULESET: OnceLock<RulesetData> = OnceLock::new();
        RULESET.get_or_init(|| {
            let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
            Self::load_with_civ3_catalog(
                &data_root,
                "civcraft_civ2civ3",
                &data_root.join("civ3_conquests"),
            )
            .unwrap_or_else(|error| panic!("failed to load campaign ruleset: {error}"))
        })
    }

    /// Resolve the selected C2C3 research and revolution inputs. This method
    /// intentionally rejects styles other than the ones used by the pinned
    /// campaign ruleset rather than substituting a built-in fallback.
    pub(crate) fn research_settings(&self) -> Result<ResearchRulesetSettings, LoadError> {
        let parameter_string = |section: &str, key: &str| -> Result<String, LoadError> {
            let parameter = required_parameter(self, "game.ruleset", section, key)?;
            parameter
                .string_value()
                .map_err(|error| LoadError::new(format!("invalid research rules: {error}")))?
                .map(str::to_owned)
                .ok_or_else(|| {
                    LoadError::new(format!(
                        "game.ruleset [{section}].{key} has no string value"
                    ))
                })
        };
        let parameter_integer = |section: &str, key: &str| -> Result<i32, LoadError> {
            let parameter = required_parameter(self, "game.ruleset", section, key)?;
            let value = parameter
                .integer_value()
                .map_err(|error| LoadError::new(format!("invalid research rules: {error}")))?
                .ok_or_else(|| {
                    LoadError::new(format!(
                        "game.ruleset [{section}].{key} has no integer value"
                    ))
                })?;
            i32::try_from(value).map_err(|_| {
                LoadError::new(format!(
                    "game.ruleset [{section}].{key} is outside the supported integer range"
                ))
            })
        };
        let parameter_bool = |section: &str, key: &str| -> Result<bool, LoadError> {
            let parameter = required_parameter(self, "game.ruleset", section, key)?;
            match &parameter.value {
                Value::Bool(value) => Ok(*value),
                other => Err(LoadError::new(format!(
                    "game.ruleset [{section}].{key} must be a boolean, found {other:?}"
                ))),
            }
        };
        let setting_integer = |name: &str| -> Result<i32, LoadError> {
            let setting = required_setting(self, name)?;
            let value = match &setting.value {
                Value::Integer(value) => *value,
                other => {
                    return Err(LoadError::new(format!(
                        "game setting `{name}` must be an integer, found {other:?}"
                    )));
                }
            };
            i32::try_from(value)
                .map_err(|_| LoadError::new(format!("game setting `{name}` is outside i32 range")))
        };
        let setting_string = |name: &str| -> Result<String, LoadError> {
            let setting = required_setting(self, name)?;
            match &setting.value {
                Value::String(value) => Ok(value.clone()),
                other => Err(LoadError::new(format!(
                    "game setting `{name}` must be a string, found {other:?}"
                ))),
            }
        };
        let checked_setting_integer =
            |name: &str, minimum: i32, maximum: i32| -> Result<i32, LoadError> {
                let value = setting_integer(name)?;
                if !(minimum..=maximum).contains(&value) {
                    return Err(LoadError::new(format!(
                        "game setting `{name}` must be in {minimum}..={maximum}, found {value}"
                    )));
                }
                Ok(value)
            };

        let cost_style = match parameter_string("research", "tech_cost_style")?
            .to_ascii_lowercase()
            .as_str()
        {
            "linear" => ResearchCostStyle::Linear,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported campaign tech_cost_style {value:?}"
                )));
            }
        };
        let leakage_style = match parameter_string("research", "tech_leakage")?
            .to_ascii_lowercase()
            .as_str()
        {
            "embassies" => TechnologyLeakageStyle::Embassies,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported campaign tech_leakage {value:?}"
                )));
            }
        };
        let upkeep_style = match parameter_string("research", "tech_upkeep_style")?
            .to_ascii_lowercase()
            .as_str()
        {
            "cities" => TechnologyUpkeepStyle::Cities,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported campaign tech_upkeep_style {value:?}"
                )));
            }
        };
        let free_technology_style = match parameter_string("research", "free_tech_method")?
            .to_ascii_lowercase()
            .as_str()
        {
            "cheapest" => FreeTechnologyStyle::Cheapest,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported campaign free_tech_method {value:?}"
                )));
            }
        };
        let revolution_type = match setting_string("revolentype")?.to_ascii_uppercase().as_str() {
            "RANDQUICK" => RevolutionType::RandomQuick,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported campaign revolentype {value:?}"
                )));
            }
        };

        Ok(ResearchRulesetSettings {
            cost_style,
            base_tech_cost: parameter_integer("research", "base_tech_cost")?,
            min_tech_cost: parameter_integer("research", "min_tech_cost")?,
            leakage_style,
            upkeep_style,
            upkeep_divider: parameter_integer("research", "tech_upkeep_divider")?,
            free_technology_style,
            tech_steal_allow_holes: parameter_bool("civstyle", "tech_steal_allow_holes")?,
            tech_trade_allow_holes: parameter_bool("civstyle", "tech_trade_allow_holes")?,
            tech_trade_loss_allow_holes: parameter_bool("civstyle", "tech_trade_loss_allow_holes")?,
            tech_parasite_allow_holes: parameter_bool("civstyle", "tech_parasite_allow_holes")?,
            tech_loss_allow_holes: parameter_bool("civstyle", "tech_loss_allow_holes")?,
            sciencebox_percent: checked_setting_integer("sciencebox", 1, 10_000)?,
            tech_penalty_percent: checked_setting_integer("techpenalty", 0, 100)?,
            tech_leak_percent: checked_setting_integer("techleak", 0, 300)?,
            tech_loss_forgiveness_percent: checked_setting_integer("techlossforgiveness", -1, 200)?,
            tech_loss_restore_percent: checked_setting_integer("techlossrestore", -1, 100)?,
            tech_lost_recv_percent: checked_setting_integer("techlost_recv", 0, 100)?,
            tech_lost_donor_percent: checked_setting_integer("techlost_donor", 0, 100)?,
            diplomatic_bulb_cost_percent: checked_setting_integer("diplbulbcost", 0, 100)?,
            revolution_type,
            revolution_length: checked_setting_integer("revolen", 1, 20)?,
        })
    }

    pub(crate) fn calendar_settings(&self) -> Result<CalendarSettings, LoadError> {
        let read = |key: &str| {
            self.parameters
                .iter()
                .find(|parameter| {
                    parameter.file == "game.ruleset"
                        && parameter.section == "calendar"
                        && parameter.key == key
                })
                .map(|parameter| &parameter.value)
                .ok_or_else(|| LoadError::new(format!("calendar is missing `{key}`")))
        };
        let start_year = match read("start_year")? {
            Value::Integer(value) => i32::try_from(*value)
                .map_err(|_| LoadError::new("calendar start_year is outside i32 range"))?,
            other => {
                return Err(LoadError::new(format!(
                    "calendar start_year must be an integer, found {other:?}"
                )));
            }
        };
        let skip_year_0 = match read("skip_year_0")? {
            Value::Bool(value) => *value,
            other => {
                return Err(LoadError::new(format!(
                    "calendar skip_year_0 must be a boolean, found {other:?}"
                )));
            }
        };
        let fragments_per_year = match read("fragments")? {
            Value::Integer(value) => u32::try_from(*value)
                .map_err(|_| LoadError::new("calendar fragments must be nonnegative"))?,
            other => {
                return Err(LoadError::new(format!(
                    "calendar fragments must be an integer, found {other:?}"
                )));
            }
        };
        let label = |key: &str| -> Result<String, LoadError> {
            match read(key)? {
                Value::String(value) => Ok(value.clone()),
                other => Err(LoadError::new(format!(
                    "calendar {key} must be a string, found {other:?}"
                ))),
            }
        };
        Ok(CalendarSettings {
            start_year,
            skip_year_0,
            fragments_per_year,
            positive_label: label("positive_label")?,
            negative_label: label("negative_label")?,
        })
    }

    /// Read and validate the C2C3 height-mask settings. CivCraft's current
    /// Civ III world-setup controls remain a separate UI layer and are not
    /// substituted for these ruleset values.
    pub(crate) fn map_generation_settings(&self) -> Result<MapGenerationSettings, LoadError> {
        let terrain_source_file = self
            .files
            .iter()
            .find(|file| file.name == "terrain.ruleset");
        let read = |key: &str| {
            let mut matches = self
                .settings
                .iter()
                .filter(|setting| setting.name.eq_ignore_ascii_case(key));
            let value = matches
                .next()
                .map(|setting| &setting.value)
                .ok_or_else(|| {
                    LoadError::new(format!("campaign ruleset is missing `{key}` setting"))
                })?;
            if matches.next().is_some() {
                return Err(LoadError::new(format!(
                    "campaign ruleset has duplicate `{key}` settings"
                )));
            }
            Ok(value)
        };

        let landmass_percent = match read("landmass")? {
            Value::Integer(value) => i32::try_from(*value)
                .map_err(|_| LoadError::new("campaign `landmass` is outside i32 range"))?,
            other => {
                return Err(LoadError::new(format!(
                    "campaign `landmass` setting must be an integer, found {other:?}"
                )));
            }
        };
        if !(15..=85).contains(&landmass_percent) {
            return Err(LoadError::new(format!(
                "campaign `landmass` must be between 15 and 85, found {landmass_percent}"
            )));
        }
        let temperature = match read("temperature")? {
            Value::Integer(value) => i32::try_from(*value)
                .map_err(|_| LoadError::new("campaign `temperature` is outside i32 range"))?,
            other => {
                return Err(LoadError::new(format!(
                    "campaign `temperature` setting must be an integer, found {other:?}"
                )));
            }
        };
        if !(0..=100).contains(&temperature) {
            return Err(LoadError::new(format!(
                "campaign `temperature` must be between 0 and 100, found {temperature}"
            )));
        }
        let steepness = match read("steepness")? {
            Value::Integer(value) => i32::try_from(*value)
                .map_err(|_| LoadError::new("campaign `steepness` is outside i32 range"))?,
            other => {
                return Err(LoadError::new(format!(
                    "campaign `steepness` setting must be an integer, found {other:?}"
                )));
            }
        };
        if !(0..=100).contains(&steepness) {
            return Err(LoadError::new(format!(
                "campaign `steepness` must be between 0 and 100, found {steepness}"
            )));
        }
        let wetness = match read("wetness")? {
            Value::Integer(value) => i32::try_from(*value)
                .map_err(|_| LoadError::new("campaign `wetness` is outside i32 range"))?,
            other => {
                return Err(LoadError::new(format!(
                    "campaign `wetness` setting must be an integer, found {other:?}"
                )));
            }
        };
        if !(0..=100).contains(&wetness) {
            return Err(LoadError::new(format!(
                "campaign `wetness` must be between 0 and 100, found {wetness}"
            )));
        }
        let read_map_count = |key: &str, minimum: i32, maximum: i32| {
            let value = match read(key)? {
                Value::Integer(value) => i32::try_from(*value).map_err(|_| {
                    LoadError::new(format!("campaign `{key}` is outside i32 range"))
                })?,
                other => {
                    return Err(LoadError::new(format!(
                        "campaign `{key}` setting must be an integer, found {other:?}"
                    )));
                }
            };
            if !(minimum..=maximum).contains(&value) {
                return Err(LoadError::new(format!(
                    "campaign `{key}` must be between {minimum} and {maximum}, found {value}"
                )));
            }
            Ok(value)
        };
        let specials_per_thousand = read_map_count("specials", 0, 1000)?;
        let huts_per_thousand = read_map_count("huts", 0, 500)?;
        let startpos = match read("startpos")? {
            Value::String(value) => StartPositionSetting::parse(value)?,
            other => {
                return Err(LoadError::new(format!(
                    "campaign `startpos` setting must be a string, found {other:?}"
                )));
            }
        };
        let mut ocean_resource_parameters = self.parameters.iter().filter(|parameter| {
            parameter.file.eq_ignore_ascii_case("terrain.ruleset")
                && parameter.section.eq_ignore_ascii_case("parameters")
                && parameter.key.eq_ignore_ascii_case("ocean_resources")
        });
        let ocean_resources = match ocean_resource_parameters.next().map(|p| &p.value) {
            Some(Value::Bool(value)) => *value,
            Some(other) => {
                return Err(LoadError::new(format!(
                    "campaign terrain `ocean_resources` parameter must be a boolean, found {other:?}"
                )));
            }
            None => {
                return Err(LoadError::new(
                    "campaign terrain.ruleset is missing `ocean_resources` parameter",
                ));
            }
        };
        if ocean_resource_parameters.next().is_some() {
            return Err(LoadError::new(
                "campaign terrain.ruleset has duplicate `ocean_resources` parameters",
            ));
        }
        let hut_record = {
            let mut matches = self.rule_records.iter().filter(|record| {
                record.namespace.eq_ignore_ascii_case("extra")
                    && record.rule_name.eq_ignore_ascii_case("Hut")
            });
            let record = matches
                .next()
                .ok_or_else(|| LoadError::new("campaign ruleset is missing the Hut extra"))?;
            if matches.next().is_some() {
                return Err(LoadError::new("campaign ruleset has duplicate Hut extras"));
            }
            record
        };
        let mut hut_land_only = false;
        let mut hut_minimum_latitude = None;
        let mut hut_maximum_latitude = None;
        for requirement in &hut_record.requirements {
            if requirement.range != RequirementRange::Tile || !requirement.present {
                return Err(LoadError::new(format!(
                    "campaign Hut extra has unsupported {:?} requirement range/presence",
                    requirement.kind
                )));
            }
            match requirement.kind.to_ascii_lowercase().as_str() {
                "terrainclass" => match &requirement.name {
                    RequirementValue::String(name) if name.eq_ignore_ascii_case("Land") => {
                        hut_land_only = true;
                    }
                    other => {
                        return Err(LoadError::new(format!(
                            "campaign Hut extra has unsupported TerrainClass requirement {other:?}"
                        )));
                    }
                },
                "minlatitude" | "maxlatitude" => {
                    let value = match &requirement.name {
                        RequirementValue::Integer(value) => {
                            i32::try_from(*value).map_err(|_| {
                                LoadError::new(format!(
                                    "campaign Hut `{}` is outside i32 range",
                                    requirement.kind
                                ))
                            })?
                        }
                        RequirementValue::String(value) => value.parse::<i32>().map_err(|_| {
                            LoadError::new(format!(
                                "campaign Hut `{}` must be an integer, found {value:?}",
                                requirement.kind
                            ))
                        })?,
                        other => {
                            return Err(LoadError::new(format!(
                                "campaign Hut `{}` must be an integer, found {other:?}",
                                requirement.kind
                            )));
                        }
                    };
                    let slot = if requirement.kind.eq_ignore_ascii_case("MinLatitude") {
                        &mut hut_minimum_latitude
                    } else {
                        &mut hut_maximum_latitude
                    };
                    if slot.replace(value).is_some() {
                        return Err(LoadError::new(format!(
                            "campaign Hut extra repeats `{}`",
                            requirement.kind
                        )));
                    }
                }
                other => {
                    return Err(LoadError::new(format!(
                        "campaign Hut extra has unsupported requirement {other:?}"
                    )));
                }
            }
        }
        let hut_placement = HutPlacementSettings {
            land_only: hut_land_only,
            minimum_latitude: hut_minimum_latitude.ok_or_else(|| {
                LoadError::new("campaign Hut extra is missing `MinLatitude` requirement")
            })?,
            maximum_latitude: hut_maximum_latitude.ok_or_else(|| {
                LoadError::new("campaign Hut extra is missing `MaxLatitude` requirement")
            })?,
        };
        if !hut_placement.land_only
            || hut_placement.minimum_latitude > hut_placement.maximum_latitude
            || !(-1000..=1000).contains(&hut_placement.minimum_latitude)
            || !(-1000..=1000).contains(&hut_placement.maximum_latitude)
        {
            return Err(LoadError::new(
                "campaign Hut extra has invalid land or latitude requirements",
            ));
        }
        let terrain_unsafe_coast = |section: &str| -> Result<bool, LoadError> {
            let mut matches = self
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Terrain && record.section == section);
            let terrain = matches.next().ok_or_else(|| {
                LoadError::new(format!("campaign ruleset is missing `{section}` terrain"))
            })?;
            if matches.next().is_some() {
                return Err(LoadError::new(format!(
                    "campaign ruleset has duplicate `{section}` terrains"
                )));
            }
            terrain_flag_is_set(terrain, terrain_source_file, "UnsafeCoast")
        };
        let read_ocean_depth = |section: &str| {
            let mut matches = self
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Terrain && record.section == section);
            let terrain = matches.next().ok_or_else(|| {
                LoadError::new(format!("campaign ruleset is missing `{section}` terrain"))
            })?;
            if matches.next().is_some() {
                return Err(LoadError::new(format!(
                    "campaign ruleset has duplicate `{section}` terrains"
                )));
            }
            let depth = terrain
                .integer_field("property_ocean_depth")
                .map_err(LoadError::new)?
                .ok_or_else(|| {
                    LoadError::new(format!(
                        "campaign terrain `{section}` is missing `property_ocean_depth`"
                    ))
                })?;
            let depth = i32::try_from(depth).map_err(|_| {
                LoadError::new(format!(
                    "campaign terrain `{section}` ocean depth is outside i32 range"
                ))
            })?;
            if !(0..=FREECIV_OCEAN_DEPTH_MAXIMUM).contains(&depth) {
                return Err(LoadError::new(format!(
                    "campaign terrain `{section}` ocean depth must be between 0 and 100"
                )));
            }
            Ok(depth)
        };
        let ocean_depths = OceanDepthSettings {
            lake: read_ocean_depth("terrain_lake")?,
            lake_unsafe_coast: terrain_unsafe_coast("terrain_lake")?,
            ocean: read_ocean_depth("terrain_ocean")?,
            ocean_unsafe_coast: terrain_unsafe_coast("terrain_ocean")?,
            deep_ocean: read_ocean_depth("terrain_deep_ocean")?,
            deep_ocean_unsafe_coast: terrain_unsafe_coast("terrain_deep_ocean")?,
        };
        if [
            ocean_depths.lake,
            ocean_depths.ocean,
            ocean_depths.deep_ocean,
        ]
        .into_iter()
        .all(|depth| depth < FREECIV_OCEAN_DEPTH_MINIMUM)
        {
            return Err(LoadError::new(
                "campaign ruleset has no ocean terrain eligible for Freeciv map generation",
            ));
        }
        Ok(MapGenerationSettings {
            landmass_percent,
            startpos,
            temperature,
            steepness,
            wetness,
            specials_per_thousand,
            huts_per_thousand,
            ocean_resources,
            hut_placement,
            ocean_depths,
        })
    }

    /// Return non-ocean terrain mapgen properties in the selected ruleset's
    /// terrain section order. Freeciv defaults each omitted `property_*` to
    /// zero; malformed values are reported with their terrain section/key.
    pub(crate) fn terrain_mapgen_properties(
        &self,
    ) -> Result<Vec<TerrainMapgenProperties>, LoadError> {
        let source_file = self
            .files
            .iter()
            .find(|file| file.name == "terrain.ruleset");
        let terrain_records = self
            .content
            .iter()
            .filter(|record| record.kind == ContentKind::Terrain);
        let mut properties = Vec::new();

        for record in terrain_records {
            let class_entry = record.fields.iter().find(|entry| entry.key == "class");
            let class = match class_entry.map(|entry| &entry.value) {
                Some(Value::String(value)) => value.as_str(),
                Some(other) => {
                    return Err(terrain_content_error(
                        source_file,
                        record,
                        "class",
                        format!("terrain class must be a string, found {other:?}"),
                    ));
                }
                None => {
                    return Err(terrain_content_error(
                        source_file,
                        record,
                        "class",
                        "terrain record is missing `class`",
                    ));
                }
            };
            if class.eq_ignore_ascii_case("Oceanic") {
                continue;
            }

            let read_property = |key: &str| -> Result<i32, LoadError> {
                let value = match record.integer_field(key) {
                    Ok(Some(value)) => value,
                    Ok(None) => return Ok(0),
                    Err(message) => {
                        return Err(terrain_content_error(source_file, record, key, message));
                    }
                };
                i32::try_from(value).map_err(|_| {
                    terrain_content_error(
                        source_file,
                        record,
                        key,
                        format!("terrain property is outside i32 range: {value}"),
                    )
                })
            };
            let starter = terrain_flag_is_set(record, source_file, "Starter")?;
            let unsafe_coast = terrain_flag_is_set(record, source_file, "UnsafeCoast")?;

            properties.push(TerrainMapgenProperties {
                rule_name: record.rule_name.clone(),
                section: record.section.clone(),
                starter,
                unsafe_coast,
                food: read_property("food")?,
                shield: read_property("shield")?,
                trade: read_property("trade")?,
                road_trade_incr_pct: read_property("road_trade_incr_pct")?,
                road_time: read_property("road_time")?,
                irrigation_food_incr: read_property("irrigation_food_incr")?,
                irrigation_time: read_property("irrigation_time")?,
                mining_shield_incr: read_property("mining_shield_incr")?,
                mining_time: read_property("mining_time")?,
                mountainous: read_property("property_mountainous")?,
                green: read_property("property_green")?,
                foliage: read_property("property_foliage")?,
                tropical: read_property("property_tropical")?,
                temperate: read_property("property_temperate")?,
                cold: read_property("property_cold")?,
                frozen: read_property("property_frozen")?,
                wet: read_property("property_wet")?,
                dry: read_property("property_dry")?,
                ocean_depth: read_property("property_ocean_depth")?,
            });
        }

        Ok(properties)
    }

    /// Resolve the C2C3 road and activity output modifiers used by
    /// `server/generator/startpos.c::get_tile_value`.
    pub(crate) fn start_position_scoring_settings(
        &self,
    ) -> Result<StartPositionScoringSettings, LoadError> {
        let road_trade_increment = self
            .rule_records
            .iter()
            .find(|record| {
                record.namespace == "road" && record.rule_name.eq_ignore_ascii_case("Road")
            })
            .map(|record| rule_record_integer(self, record, "trade_incr"))
            .transpose()?
            .unwrap_or(0);
        let has_requirement = |effect: &EffectRecord, kind: &str, name: &str, present: bool| {
            effect.requirements.iter().any(|requirement| {
                requirement.kind.eq_ignore_ascii_case(kind)
                    && matches!(
                        &requirement.name,
                        RequirementValue::String(value) if value.eq_ignore_ascii_case(name)
                    )
                    && requirement.range == RequirementRange::Tile
                    && requirement.present == present
            })
        };
        let sum_effects = |effects: Vec<&EffectRecord>| {
            effects
                .into_iter()
                .map(|effect| effect.value)
                .sum::<i64>()
                .clamp(0, i64::from(i32::MAX)) as i32
        };
        let irrigation_percent = sum_effects(
            self.effects
                .iter()
                .filter(|effect| effect.kind.eq_ignore_ascii_case("Irrigation_Pct"))
                .filter(|effect| has_requirement(effect, "Extra", "Irrigation", true))
                .filter(|effect| !has_requirement(effect, "Extra", "River", true))
                .collect(),
        );
        let desert_river_irrigation_percent = sum_effects(
            self.effects
                .iter()
                .filter(|effect| effect.kind.eq_ignore_ascii_case("Irrigation_Pct"))
                .filter(|effect| has_requirement(effect, "Terrain", "Desert", true))
                .filter(|effect| has_requirement(effect, "Extra", "River", true))
                .filter(|effect| has_requirement(effect, "Extra", "Oasis", false))
                .filter(|effect| has_requirement(effect, "Extra", "Irrigation", true))
                .collect(),
        );
        let mining_percent = sum_effects(
            self.effects
                .iter()
                .filter(|effect| effect.kind.eq_ignore_ascii_case("Mining_Pct"))
                .filter(|effect| has_requirement(effect, "Extra", "Mine", true))
                .collect(),
        );
        Ok(StartPositionScoringSettings {
            road_trade_increment,
            irrigation_percent,
            desert_river_irrigation_percent,
            mining_percent,
        })
    }

    /// Read Freeciv's bitwise `victories` server setting from the selected
    /// ruleset's explicit campaign defaults. The engine default is not a
    /// ruleset value, so the CivCraft derivative records it instead of
    /// inheriting it implicitly.
    pub(crate) fn victory_enabled(&self, condition: VictoryCondition) -> Result<bool, LoadError> {
        let value = self
            .settings
            .iter()
            .find(|setting| setting.name.eq_ignore_ascii_case("victories"))
            .map(|setting| &setting.value)
            .ok_or_else(|| LoadError::new("campaign ruleset is missing `victories` setting"))?;
        let Value::String(value) = value else {
            return Err(LoadError::new(format!(
                "campaign `victories` setting must be a string, found {value:?}"
            )));
        };

        let mut seen = std::collections::BTreeSet::new();
        let mut enabled = false;
        for name in value.split('|').map(str::trim) {
            if name.is_empty() {
                if value.trim().is_empty() {
                    continue;
                }
                return Err(LoadError::new(
                    "campaign `victories` setting contains an empty condition",
                ));
            }
            let canonical = match name.to_ascii_uppercase().as_str() {
                "SPACERACE" => "SPACERACE",
                "ALLIED" => "ALLIED",
                "CULTURE" => "CULTURE",
                "WORLDPEACE" => "WORLDPEACE",
                _ => {
                    return Err(LoadError::new(format!(
                        "campaign `victories` setting contains unknown condition {name:?}"
                    )));
                }
            };
            if !seen.insert(canonical) {
                return Err(LoadError::new(format!(
                    "campaign `victories` setting repeats condition {name:?}"
                )));
            }
            enabled |= canonical == condition.rule_name();
        }
        Ok(enabled)
    }
}

fn required_parameter<'a>(
    rules: &'a RulesetData,
    file: &str,
    section: &str,
    key: &str,
) -> Result<&'a RuleParameter, LoadError> {
    let mut matches = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case(file)
            && parameter.section.eq_ignore_ascii_case(section)
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let parameter = matches.next().ok_or_else(|| {
        LoadError::new(format!(
            "selected ruleset is missing {file} [{section}].{key}"
        ))
    })?;
    if matches.next().is_some() {
        return Err(LoadError::new(format!(
            "selected ruleset repeats {file} [{section}].{key}"
        )));
    }
    Ok(parameter)
}

fn optional_parameter<'a>(
    rules: &'a RulesetData,
    file: &str,
    section: &str,
    key: &str,
) -> Result<Option<&'a RuleParameter>, LoadError> {
    let mut matches = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case(file)
            && parameter.section.eq_ignore_ascii_case(section)
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let parameter = matches.next();
    if matches.next().is_some() {
        return Err(LoadError::new(format!(
            "selected ruleset repeats {file} [{section}].{key}"
        )));
    }
    Ok(parameter)
}

fn required_setting<'a>(
    rules: &'a RulesetData,
    name: &str,
) -> Result<&'a SettingRecord, LoadError> {
    let mut matches = rules
        .settings
        .iter()
        .filter(|setting| setting.name.eq_ignore_ascii_case(name));
    let setting = matches
        .next()
        .ok_or_else(|| LoadError::new(format!("selected ruleset is missing `{name}` setting")))?;
    if matches.next().is_some() {
        return Err(LoadError::new(format!(
            "selected ruleset repeats `{name}` setting"
        )));
    }
    Ok(setting)
}

impl RulesetData {
    /// Decode the selected Freeciv missing-upkeep auto-performer inputs.
    /// Omitted protected/action lists are empty; omitted wipe values use the
    /// defaults in the pinned server ruleset loader (food/gold TRUE, shield
    /// FALSE).
    pub(crate) fn missing_unit_upkeep_settings(
        &self,
    ) -> Result<MissingUnitUpkeepSettings, LoadError> {
        let read_list = |key: &str| -> Result<Vec<String>, LoadError> {
            let Some(parameter) =
                optional_parameter(self, "cities.ruleset", "missing_unit_upkeep", key)?
            else {
                return Ok(Vec::new());
            };
            parameter
                .string_list_value()
                .map_err(|error| LoadError::new(format!("invalid missing upkeep rules: {error}")))
                .map(|value| value.unwrap_or_default())
        };
        let read_bool = |key: &str, default: bool| -> Result<bool, LoadError> {
            match optional_parameter(self, "cities.ruleset", "missing_unit_upkeep", key)? {
                Some(parameter) => parameter
                    .boolean_value()
                    .map_err(|error| {
                        LoadError::new(format!("invalid missing upkeep rules: {error}"))
                    })
                    .map(|value| value.unwrap_or(default)),
                None => Ok(default),
            }
        };
        let read_output = |output: UnitUpkeepOutput,
                           default_wipe: bool|
         -> Result<MissingUnitUpkeepOutputSettings, LoadError> {
            let prefix = output.parameter_name();
            Ok(MissingUnitUpkeepOutputSettings {
                protected_flags: read_list(&format!("{prefix}_protected"))?,
                actions: read_list(&format!("{prefix}_unit_act"))?,
                wipe: read_bool(&format!("{prefix}_wipe"), default_wipe)?,
            })
        };

        Ok(MissingUnitUpkeepSettings {
            food: read_output(UnitUpkeepOutput::Food, true)?,
            shield: read_output(UnitUpkeepOutput::Shield, false)?,
            gold: read_output(UnitUpkeepOutput::Gold, true)?,
        })
    }

    /// Resolve the border settings used by Freeciv's `unit_being_aggressive`.
    /// The engine defaults are explicit in the CivCraft-owned derivative.
    pub(crate) fn unit_happiness_settings(
        &self,
    ) -> Result<UnitHappinessRulesetSettings, LoadError> {
        let string_setting = |name: &str| -> Result<String, LoadError> {
            let setting = required_setting(self, name)?;
            match &setting.value {
                Value::String(value) => Ok(value.clone()),
                other => Err(LoadError::new(format!(
                    "game setting `{name}` must be a string, found {other:?}"
                ))),
            }
        };

        let borders_enabled = match string_setting("borders")?.to_ascii_uppercase().as_str() {
            "DISABLED" => false,
            "ENABLED" | "SEE_INSIDE" | "EXPAND" => true,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported borders setting {value:?}"
                )));
            }
        };
        let happy_borders = match string_setting("happyborders")?
            .to_ascii_uppercase()
            .as_str()
        {
            "DISABLED" => HappyBorders::Disabled,
            "NATIONAL" => HappyBorders::National,
            "ALLIED" => HappyBorders::Allied,
            value => {
                return Err(LoadError::new(format!(
                    "unsupported happyborders setting {value:?}"
                )));
            }
        };
        Ok(UnitHappinessRulesetSettings {
            borders_enabled,
            happy_borders,
        })
    }

    /// Decode the selected Freeciv government's records and government-scoped
    /// effects without imposing a CivCraft enum or a built-in government list.
    pub(crate) fn government_catalog(&self) -> Result<GovernmentRulesetCatalog, LoadError> {
        let revolution_parameter = required_parameter(
            self,
            "governments.ruleset",
            "governments",
            "during_revolution",
        )?;
        let during_revolution_name = revolution_parameter
            .string_value()
            .map_err(|message| LoadError::new(format!("invalid revolution government: {message}")))?
            .ok_or_else(|| {
                LoadError::new("governments.ruleset [governments].during_revolution is empty")
            })?;
        if during_revolution_name.trim().is_empty() {
            return Err(LoadError::new(
                "governments.ruleset [governments].during_revolution is empty",
            ));
        }

        let source_file = self
            .files
            .iter()
            .find(|file| file.name.eq_ignore_ascii_case("governments.ruleset"));
        let source_records: Vec<_> = self
            .content
            .iter()
            .filter(|record| record.kind == ContentKind::Government)
            .collect();
        if source_records.is_empty() {
            return Err(LoadError::new(
                "selected governments.ruleset has no government records",
            ));
        }

        let mut governments = Vec::with_capacity(source_records.len());
        for source in source_records {
            let id = source.rule_name.trim();
            if id.is_empty() {
                return Err(government_content_error(
                    source_file,
                    source,
                    "rule_name",
                    "government rule name cannot be empty",
                ));
            }
            if governments
                .iter()
                .any(|government: &GovernmentRulesetRecord| government.id.eq_ignore_ascii_case(id))
            {
                return Err(government_content_error(
                    source_file,
                    source,
                    "rule_name",
                    format!("duplicate government rule name {id:?}"),
                ));
            }

            let name = source.display_name.clone().ok_or_else(|| {
                government_content_error(
                    source_file,
                    source,
                    "name",
                    "government is missing its display name",
                )
            })?;
            let flags = source
                .string_list_field("flags")
                .map_err(|message| government_content_error(source_file, source, "flags", message))?
                .unwrap_or_default();
            let ai_better = source
                .string_field("ai_better")
                .map_err(|message| {
                    government_content_error(source_file, source, "ai_better", message)
                })?
                .map(str::to_owned);
            if let Some(better) = ai_better.as_deref()
                && !self.content.iter().any(|candidate| {
                    candidate.kind == ContentKind::Government
                        && candidate.rule_name.eq_ignore_ascii_case(better)
                })
            {
                return Err(government_content_error(
                    source_file,
                    source,
                    "ai_better",
                    format!("unknown preferred government {better:?}"),
                ));
            }

            let attributes = self
                .effects
                .iter()
                .filter(|effect| government_effect_applies_to(effect, id))
                .cloned()
                .collect();
            governments.push(GovernmentRulesetRecord {
                id: id.to_owned(),
                name,
                section: source.section.clone(),
                requirements: source.requirements.clone(),
                flags,
                ai_better,
                attributes,
            });
        }

        let during_revolution_id = governments
            .iter()
            .find(|government| government.id.eq_ignore_ascii_case(during_revolution_name))
            .map(|government| government.id.clone())
            .ok_or_else(|| {
                LoadError::new(format!(
                    "governments.ruleset [governments].during_revolution references unknown government {during_revolution_name:?}"
                ))
            })?;

        Ok(GovernmentRulesetCatalog {
            during_revolution_id,
            governments,
        })
    }
}

fn government_effect_applies_to(effect: &EffectRecord, government_id: &str) -> bool {
    let mut found_government_requirement = false;
    for requirement in effect
        .requirements
        .iter()
        .filter(|requirement| requirement.kind.eq_ignore_ascii_case("Gov"))
    {
        found_government_requirement = true;
        let matches = matches!(
            &requirement.name,
            RequirementValue::String(name) if name.eq_ignore_ascii_case(government_id)
        );
        if requirement.present != matches {
            return false;
        }
    }
    found_government_requirement
}

fn government_content_error(
    source_file: Option<&LoadedRuleFile>,
    record: &ContentRecord,
    key: &str,
    message: impl Into<String>,
) -> LoadError {
    LoadError {
        file: source_file.map(|file| file.path.clone()),
        section: Some(record.section.clone()),
        key: Some(key.to_owned()),
        line: record
            .fields
            .iter()
            .find(|entry| entry.key.eq_ignore_ascii_case(key))
            .map(|entry| entry.line),
        message: message.into(),
    }
}

fn terrain_content_error(
    source_file: Option<&LoadedRuleFile>,
    record: &ContentRecord,
    key: &str,
    message: impl Into<String>,
) -> LoadError {
    LoadError {
        file: source_file.map(|file| file.path.clone()),
        section: Some(record.section.clone()),
        key: Some(key.to_owned()),
        line: record
            .fields
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.line),
        message: message.into(),
    }
}

fn terrain_flag_is_set(
    record: &ContentRecord,
    source_file: Option<&LoadedRuleFile>,
    flag_name: &str,
) -> Result<bool, LoadError> {
    let Some(entry) = record
        .fields
        .iter()
        .find(|entry| entry.key.eq_ignore_ascii_case("flags"))
    else {
        return Ok(false);
    };
    match &entry.value {
        Value::List(flags) => flags.iter().try_fold(false, |found, flag| match flag {
            Value::String(name) => Ok(found || name.eq_ignore_ascii_case(flag_name)),
            other => Err(terrain_content_error(
                source_file,
                record,
                "flags",
                format!("terrain flags must be strings, found {other:?}"),
            )),
        }),
        Value::String(flag) => Ok(flag.eq_ignore_ascii_case(flag_name)),
        Value::Empty => Ok(false),
        other => Err(terrain_content_error(
            source_file,
            record,
            "flags",
            format!("terrain flags must be a string list, found {other:?}"),
        )),
    }
}

fn rule_record_integer(
    rules: &RulesetData,
    record: &RuleRecord,
    key: &str,
) -> Result<i32, LoadError> {
    let Some(entry) = record.fields.iter().find(|entry| entry.key == key) else {
        return Ok(0);
    };
    match &entry.value {
        Value::Integer(value) => i32::try_from(*value).map_err(|_| LoadError {
            file: rules
                .files
                .iter()
                .find(|file| file.name == record.file)
                .map(|file| file.path.clone()),
            section: Some(record.section.clone()),
            key: Some(key.to_owned()),
            line: Some(entry.line),
            message: format!("road output value is outside i32 range: {value}"),
        }),
        other => Err(LoadError {
            file: rules
                .files
                .iter()
                .find(|file| file.name == record.file)
                .map(|file| file.path.clone()),
            section: Some(record.section.clone()),
            key: Some(key.to_owned()),
            line: Some(entry.line),
            message: format!("road output value must be an integer, found {other:?}"),
        }),
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
    UnknownEffectType(String),
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
    effects: &[EffectRecord],
    kind: &str,
    context: &impl EffectContext,
) -> Result<i64, EffectEvaluationError> {
    if !identifiers::EFFECT_NAMES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(kind))
    {
        return Err(EffectEvaluationError::UnknownEffectType(kind.to_owned()));
    }
    let mut total = 0_i64;
    for effect in effects {
        if !effect.kind.eq_ignore_ascii_case(kind)
            || !requirements_met(&effect.requirements, context)?
        {
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
    use super::freeciv_ini::{Entry, Value};
    use super::{
        ActionEnabler, ActionRecord, ContentKind, EffectContext, EffectRecord, FreeTechnologyStyle,
        HappyBorders, LoadedRuleFile, MissingUnitUpkeepOutputSettings, MissingUnitUpkeepSettings,
        RULESET_FILES, Requirement, RequirementContext, RequirementEvaluationError,
        RequirementRange, RequirementValue, ResearchCostStyle, ResearchRulesetSettings,
        RevolutionType, RuleFileSource, RuleSourceLocation, RulesetData, TechnologyLeakageStyle,
        TechnologyUpkeepStyle, UnitHappinessRulesetSettings, UnitUpkeepOutput, VictoryCondition,
        action_enabled, effect_value, requirements_met,
    };
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn campaign_victory_conditions_are_explicit_and_validated() {
        let mut rules = RulesetData::campaign_default().clone();
        assert!(rules.victory_enabled(VictoryCondition::SpaceRace).unwrap());
        assert!(rules.victory_enabled(VictoryCondition::Allied).unwrap());
        assert!(!rules.victory_enabled(VictoryCondition::Culture).unwrap());

        let index = rules
            .settings
            .iter()
            .position(|setting| setting.name == "victories")
            .unwrap();
        rules.settings[index].value = super::Value::String("ALLIED".to_owned());
        assert!(rules.victory_enabled(VictoryCondition::Allied).unwrap());
        assert!(!rules.victory_enabled(VictoryCondition::SpaceRace).unwrap());

        rules.settings[index].value = super::Value::String("ALLIED|UNKNOWN".to_owned());
        assert!(rules.victory_enabled(VictoryCondition::Allied).is_err());
        rules.settings[index].value = super::Value::String("ALLIED|ALLIED".to_owned());
        assert!(rules.victory_enabled(VictoryCondition::Allied).is_err());
    }

    #[test]
    fn campaign_map_generation_settings_are_explicit_and_validated() {
        let mut rules = RulesetData::campaign_default().clone();
        assert_eq!(rules.settings.len(), 40);
        let defaults = rules.map_generation_settings().unwrap();
        assert_eq!(defaults.landmass_percent, 30);
        assert_eq!(defaults.ocean_coverage_percent(), 70);
        assert_eq!(defaults.shore_level(), 700);
        assert_eq!(defaults.startpos.fractal_divisions(8), 6);
        assert_eq!(defaults.specials_per_thousand, 250);
        assert_eq!(defaults.huts_per_thousand, 15);
        assert!(!defaults.ocean_resources);
        assert!(defaults.hut_placement.land_only);
        assert_eq!(defaults.hut_placement.minimum_latitude, -980);
        assert_eq!(defaults.hut_placement.maximum_latitude, 980);
        assert_eq!(
            defaults.ocean_depths,
            super::OceanDepthSettings {
                lake: 0,
                lake_unsafe_coast: false,
                ocean: 35,
                ocean_unsafe_coast: true,
                deep_ocean: 70,
                deep_ocean_unsafe_coast: true,
            }
        );

        let landmass = rules
            .settings
            .iter()
            .position(|setting| setting.name == "landmass")
            .unwrap();
        for value in [15, 85] {
            rules.settings[landmass].value = super::Value::Integer(i64::from(value));
            assert_eq!(
                rules.map_generation_settings().unwrap().landmass_percent,
                value
            );
        }
        for value in [14, 86] {
            rules.settings[landmass].value = super::Value::Integer(i64::from(value));
            assert!(rules.map_generation_settings().is_err());
        }
        rules.settings[landmass].value = super::Value::Integer(30);

        let startpos = rules
            .settings
            .iter()
            .position(|setting| setting.name == "startpos")
            .unwrap();
        for (value, expected_divisions) in [
            ("DEFAULT", 6),
            ("ALL", 6),
            ("SINGLE", 14),
            ("2or3", 14),
            ("VARIABLE", 14),
        ] {
            rules.settings[startpos].value = super::Value::String(value.to_owned());
            assert_eq!(
                rules
                    .map_generation_settings()
                    .unwrap()
                    .startpos
                    .fractal_divisions(8),
                expected_divisions
            );
        }
        rules.settings[startpos].value = super::Value::String("UNKNOWN".to_owned());
        assert!(rules.map_generation_settings().is_err());

        let mut no_generated_ocean = RulesetData::campaign_default().clone();
        for section in ["terrain_lake", "terrain_ocean", "terrain_deep_ocean"] {
            let terrain = no_generated_ocean
                .content
                .iter_mut()
                .find(|record| record.kind == ContentKind::Terrain && record.section == section)
                .unwrap();
            terrain
                .fields
                .iter_mut()
                .find(|field| field.key == "property_ocean_depth")
                .unwrap()
                .value = super::Value::Integer(0);
        }
        assert!(no_generated_ocean.map_generation_settings().is_err());
    }

    #[test]
    fn campaign_research_settings_are_ruleset_owned_and_typed() {
        let rules = RulesetData::campaign_default();
        let settings = rules.research_settings().unwrap();
        assert_eq!(
            settings,
            ResearchRulesetSettings {
                cost_style: ResearchCostStyle::Linear,
                base_tech_cost: 10,
                min_tech_cost: 10,
                leakage_style: TechnologyLeakageStyle::Embassies,
                upkeep_style: TechnologyUpkeepStyle::Cities,
                upkeep_divider: 6000,
                free_technology_style: FreeTechnologyStyle::Cheapest,
                tech_steal_allow_holes: false,
                tech_trade_allow_holes: false,
                tech_trade_loss_allow_holes: false,
                tech_parasite_allow_holes: false,
                tech_loss_allow_holes: false,
                sciencebox_percent: 100,
                tech_penalty_percent: 100,
                tech_leak_percent: 100,
                tech_loss_forgiveness_percent: 0,
                tech_loss_restore_percent: 50,
                tech_lost_recv_percent: 0,
                tech_lost_donor_percent: 0,
                diplomatic_bulb_cost_percent: 0,
                revolution_type: RevolutionType::RandomQuick,
                revolution_length: 5,
            }
        );
    }

    #[test]
    fn campaign_unit_happiness_settings_are_ruleset_owned_and_typed() {
        let rules = RulesetData::campaign_default();
        assert_eq!(
            rules.unit_happiness_settings().unwrap(),
            UnitHappinessRulesetSettings {
                borders_enabled: true,
                happy_borders: HappyBorders::National,
            }
        );

        let mut rules = rules.clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "happyborders")
            .unwrap()
            .value = Value::String("ALLIED".to_owned());
        assert_eq!(
            rules.unit_happiness_settings().unwrap().happy_borders,
            HappyBorders::Allied
        );

        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "happyborders")
            .unwrap()
            .value = Value::String("unknown".to_owned());
        assert!(rules.unit_happiness_settings().is_err());
    }

    #[test]
    fn campaign_missing_unit_upkeep_settings_preserve_lists_and_loader_defaults() {
        let rules = RulesetData::campaign_default();
        let settings = rules.missing_unit_upkeep_settings().unwrap();
        assert_eq!(
            settings,
            MissingUnitUpkeepSettings {
                food: MissingUnitUpkeepOutputSettings {
                    protected_flags: vec!["EvacuateFirst".to_owned()],
                    actions: Vec::new(),
                    wipe: true,
                },
                shield: MissingUnitUpkeepOutputSettings {
                    protected_flags: vec!["EvacuateFirst".to_owned()],
                    actions: vec![
                        "Help Wonder".to_owned(),
                        "Disband Unit Recover".to_owned(),
                        "Disband Unit".to_owned(),
                    ],
                    wipe: false,
                },
                gold: MissingUnitUpkeepOutputSettings {
                    protected_flags: Vec::new(),
                    actions: Vec::new(),
                    wipe: true,
                },
            }
        );
        assert_eq!(
            settings.for_output(UnitUpkeepOutput::Shield).actions,
            vec![
                "Help Wonder".to_owned(),
                "Disband Unit Recover".to_owned(),
                "Disband Unit".to_owned(),
            ]
        );

        let mut rules = rules.clone();
        rules.parameters.retain(|parameter| {
            !(parameter.file.eq_ignore_ascii_case("cities.ruleset")
                && parameter
                    .section
                    .eq_ignore_ascii_case("missing_unit_upkeep"))
        });
        assert_eq!(
            rules.missing_unit_upkeep_settings().unwrap(),
            MissingUnitUpkeepSettings {
                food: MissingUnitUpkeepOutputSettings {
                    protected_flags: Vec::new(),
                    actions: Vec::new(),
                    wipe: true,
                },
                shield: MissingUnitUpkeepOutputSettings {
                    protected_flags: Vec::new(),
                    actions: Vec::new(),
                    wipe: false,
                },
                gold: MissingUnitUpkeepOutputSettings {
                    protected_flags: Vec::new(),
                    actions: Vec::new(),
                    wipe: true,
                },
            }
        );
    }

    #[test]
    fn missing_unit_upkeep_settings_reject_malformed_and_duplicate_values() {
        let mut rules = RulesetData::campaign_default().clone();
        let actions = rules
            .parameters
            .iter_mut()
            .find(|parameter| {
                parameter.file == "cities.ruleset"
                    && parameter.section == "missing_unit_upkeep"
                    && parameter.key == "shield_unit_act"
            })
            .unwrap();
        actions.value = Value::List(vec![
            Value::String("Help Wonder".to_owned()),
            Value::Bool(true),
        ]);
        assert!(rules.missing_unit_upkeep_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let wipe = rules
            .parameters
            .iter_mut()
            .find(|parameter| {
                parameter.file == "cities.ruleset"
                    && parameter.section == "missing_unit_upkeep"
                    && parameter.key == "food_wipe"
            })
            .unwrap();
        wipe.value = Value::String("TRUE".to_owned());
        assert!(rules.missing_unit_upkeep_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let duplicate = rules
            .parameters
            .iter()
            .find(|parameter| {
                parameter.file == "cities.ruleset"
                    && parameter.section == "missing_unit_upkeep"
                    && parameter.key == "food_wipe"
            })
            .unwrap()
            .clone();
        rules.parameters.push(duplicate);
        assert!(rules.missing_unit_upkeep_settings().is_err());
    }

    #[test]
    fn campaign_research_settings_reject_missing_duplicate_and_malformed_inputs() {
        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .retain(|setting| setting.name != "sciencebox");
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let setting = rules
            .settings
            .iter()
            .find(|setting| setting.name == "sciencebox")
            .unwrap()
            .clone();
        rules.settings.push(setting);
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "tech_cost_style")
            .unwrap()
            .value = super::Value::String("Classic".to_owned());
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "tech_trade_allow_holes")
            .unwrap()
            .value = super::Value::String("FALSE".to_owned());
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "base_tech_cost")
            .unwrap()
            .value = super::Value::String("10".to_owned());
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "techpenalty")
            .unwrap()
            .value = super::Value::String("100".to_owned());
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "techpenalty")
            .unwrap()
            .value = super::Value::Integer(101);
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "revolen")
            .unwrap()
            .value = super::Value::Integer(21);
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .retain(|parameter| parameter.key != "min_tech_cost");
        assert!(rules.research_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let parameter = rules
            .parameters
            .iter()
            .find(|parameter| parameter.key == "base_tech_cost")
            .unwrap()
            .clone();
        rules.parameters.push(parameter);
        assert!(rules.research_settings().is_err());
    }

    fn parsed_file(name: &str, source: &str) -> LoadedRuleFile {
        LoadedRuleFile {
            name: name.to_owned(),
            path: PathBuf::from(name),
            source: RuleFileSource::SelectedRuleset,
            parsed: super::freeciv_ini::parse(source).unwrap(),
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
                .any(|kind| kind.eq_ignore_ascii_case(&requirement.kind))
            {
                return Err(RequirementEvaluationError::Unsupported {
                    kind: requirement.kind.clone(),
                    name: requirement.name.clone(),
                });
            }
            Ok(self.present.iter().any(|(kind, name, range)| {
                kind.eq_ignore_ascii_case(&requirement.kind)
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
            kind: kind.to_owned(),
            name: RequirementValue::String(name.to_owned()),
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
        let effects = vec![
            EffectRecord {
                rule_name: "plain".to_owned(),
                kind: "Output_Bonus".to_owned(),
                value: 5,
                multiplier: None,
                requirements: vec![req("Tech", "Writing", true)],
            },
            EffectRecord {
                rule_name: "scaled".to_owned(),
                kind: "Output_Bonus".to_owned(),
                value: 10,
                multiplier: Some("Science".to_owned()),
                requirements: vec![],
            },
            EffectRecord {
                rule_name: "inactive".to_owned(),
                kind: "Output_Bonus".to_owned(),
                value: 100,
                multiplier: None,
                requirements: vec![req("Tech", "Mobile Warfare", true)],
            },
        ];
        let context = Context {
            present: vec![(
                "Tech".to_owned(),
                RequirementValue::String("Writing".to_owned()),
                RequirementRange::Player,
            )],
            multiplier: Some(("Science".to_owned(), 150)),
        };
        assert_eq!(effect_value(&effects, "output_bonus", &context), Ok(20));
        assert!(matches!(
            effect_value(&effects, "Unrelated", &context),
            Err(super::EffectEvaluationError::UnknownEffectType(_))
        ));
    }

    #[test]
    fn public_model_keeps_rule_names_separate_from_content_positions() {
        let record = super::ContentRecord {
            kind: ContentKind::Technology,
            rule_name: "mobile_warfare".to_owned(),
            section: "advance_mobile_warfare".to_owned(),
            display_name: Some("Mobile Warfare".to_owned()),
            requirements: vec![],
            fields: vec![],
        };
        let action = ActionRecord {
            rule_name: "build_city".to_owned(),
            action: "Found City".to_owned(),
            fields: vec![],
        };
        assert_eq!(record.rule_name, "mobile_warfare");
        assert_eq!(action.action, "Found City");
    }

    #[test]
    fn content_record_reads_validated_string_integer_table_fields() {
        let record = super::ContentRecord {
            kind: ContentKind::Terrain,
            rule_name: "grassland".to_owned(),
            section: "terrain_grassland".to_owned(),
            display_name: Some("Grassland".to_owned()),
            requirements: vec![],
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
                "actions.ruleset",
                "[action_found_city]\naction = \"Found City\"\n[enabler_found_city]\naction = \"Found City\"\nactor_reqs =\n { \"type\", \"name\", \"range\", \"present\"\n \"Tech\", \"civ3_literacy\", \"Player\", TRUE\n }\ntarget_reqs =\n { \"type\", \"name\", \"range\"\n \"Terrain\", \"Ocean\", \"Tile\"\n }\n",
            ),
            (
                "effects.ruleset",
                "[effect_bonus]\ntype = \"Output_Bonus\"\nvalue = 5\nreqs =\n { \"type\", \"name\", \"range\"\n \"Building\", \"civcraft_library\", \"City\"\n }\n",
            ),
            (
                "buildings.ruleset",
                "[building_library]\nrule_name = \"civcraft_library\"\nname = \"Library\"\nreqs =\n { \"type\", \"name\", \"range\"\n \"Tech\", \"civ3_literacy\", \"Player\"\n }\n",
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
        let rules = RulesetData::from_files("fixture", files).unwrap();

        assert_eq!(rules.settings.len(), 2);
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
        assert_eq!(rules.actions.len(), 1);
        assert_eq!(rules.action_enablers.len(), 1);
        assert!(rules.action_enablers[0].actor_requirements[0].present);
        assert_eq!(
            rules.action_enablers[0].target_requirements[0].range,
            RequirementRange::Tile
        );
        assert!(rules.content.iter().any(|record| {
            record.kind == ContentKind::Building && record.rule_name == "civcraft_library"
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
                "actions.ruleset",
                "[action_test]\naction = \"Not An Action\"\n",
                "unknown Freeciv action",
            ),
            (
                "effects.ruleset",
                "[effect_test]\ntype = \"Not_An_Effect\"\n",
                "unknown Freeciv effect type",
            ),
        ];

        for (file, contents, expected_error) in invalid_cases {
            let files = files_with_overrides(&[(file, contents)]);
            let error = RulesetData::from_files("fixture", files).unwrap_err();
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
        let error = RulesetData::from_files("fixture", files).unwrap_err();
        assert!(error.message.contains("unknown or unsupported value"));
        assert_eq!(error.section.as_deref(), Some("building_library"));
    }

    #[test]
    fn file_loader_uses_whole_file_defaults_and_expands_includes() {
        let root = std::env::temp_dir().join(format!(
            "civcraft-ruleset-loader-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let ruleset_dir = root.join("fixture");
        let default_dir = root.join("default");
        fs::create_dir_all(&ruleset_dir).unwrap();
        fs::create_dir_all(&default_dir).unwrap();

        for name in RULESET_FILES {
            if *name == "units.ruleset" {
                continue;
            }
            let contents = if *name == "nations.ruleset" {
                "[compatibility]\nfirst = \"parent\"\n*include \"default/nationlist.ruleset\"\n"
            } else {
                "[datafile]\nformat_version = 1\n"
            };
            fs::write(ruleset_dir.join(name), contents).unwrap();
        }
        fs::write(
            default_dir.join("units.ruleset"),
            "[unit_from_default]\nname = \"Default Unit\"\n",
        )
        .unwrap();
        fs::write(
            default_dir.join("nationlist.ruleset"),
            "[compatibility]\nsecond = \"included\"\n[nation_fixture]\nrule_name = \"fixture_nation\"\nname = \"Fixture Nation\"\n",
        )
        .unwrap();

        let rules = RulesetData::load(&root, "fixture").unwrap();
        assert_eq!(
            rules
                .files
                .iter()
                .find(|file| file.name == "units.ruleset")
                .unwrap()
                .source,
            RuleFileSource::DefaultRuleset
        );
        assert!(rules.content.iter().any(|record| {
            record.kind == ContentKind::Nation && record.rule_name == "fixture_nation"
        }));
        let nations = rules
            .files
            .iter()
            .find(|file| file.name == "nations.ruleset")
            .unwrap();
        let compatibility = nations
            .parsed
            .sections
            .iter()
            .filter(|section| section.name == "compatibility")
            .collect::<Vec<_>>();
        assert_eq!(compatibility.len(), 1);
        assert_eq!(compatibility[0].entries.len(), 2);

        fs::write(
            default_dir.join("nationlist.ruleset"),
            "[compatibility]\nfirst = \"duplicate\"\n[nation_fixture]\nrule_name = \"fixture_nation\"\nname = \"Fixture Nation\"\n",
        )
        .unwrap();
        let duplicate_error = RulesetData::load(&root, "fixture").unwrap_err();
        assert!(duplicate_error.message.contains("duplicate ruleset entry"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn packaged_baseline_loads_only_selected_civcraft_ruleset_files() {
        let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let rules = RulesetData::load(&data_root, "civcraft_civ2civ3").unwrap();

        assert_eq!(rules.name, "civcraft_civ2civ3");
        assert_eq!(rules.files.len(), RULESET_FILES.len());
        assert!(
            rules
                .files
                .iter()
                .all(|file| file.source == RuleFileSource::SelectedRuleset)
        );
        assert_eq!(rules.effects.len(), 508);
        assert_eq!(rules.actions.len(), 19);
        assert_eq!(rules.action_enablers.len(), 89);
        assert_eq!(
            rules
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Unit)
                .count(),
            57
        );
        assert_eq!(
            rules
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Building)
                .count(),
            73
        );
        assert_eq!(
            rules
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Technology)
                .count(),
            87
        );
        assert!(
            data_root
                .join("civcraft_civ2civ3/UPSTREAM_MANIFEST.tsv")
                .is_file()
        );
    }

    #[test]
    fn campaign_ruleset_keeps_civ3_source_catalog_separate_and_identifiable() {
        let data_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/rulesets");
        let rules = RulesetData::load_with_civ3_catalog(
            &data_root,
            "civcraft_civ2civ3",
            &data_root.join("civ3_conquests"),
        )
        .unwrap();

        assert_eq!(
            rules
                .content
                .iter()
                .filter(|r| r.kind == ContentKind::Unit)
                .count(),
            57
        );
        assert_eq!(
            rules
                .content
                .iter()
                .filter(|r| r.kind == ContentKind::Building)
                .count(),
            73
        );
        assert_eq!(
            rules
                .content
                .iter()
                .filter(|r| r.kind == ContentKind::Technology)
                .count(),
            87
        );
        for (kind, expected) in [
            (ContentKind::Unit, 92),
            (ContentKind::Building, 83),
            (ContentKind::Technology, 83),
            (ContentKind::Nation, 31),
        ] {
            assert_eq!(
                rules
                    .source_content
                    .iter()
                    .filter(|record| record.kind == kind)
                    .count(),
                expected
            );
        }

        let find_source = |kind, id: &str| {
            rules
                .source_content
                .iter()
                .find(|record| record.kind == kind && record.catalog_id == id)
                .unwrap()
        };
        let warrior = find_source(ContentKind::Unit, "warrior");
        assert_eq!(warrior.source.section, "PRTO");
        assert_eq!(warrior.source.record_indices, [6]);
        assert_eq!(
            warrior.source.sha256,
            "1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c"
        );
        assert!(
            warrior
                .fields
                .iter()
                .any(|field| field.name == "attack" && field.value == "1")
        );

        let palace = find_source(ContentKind::Building, "BLDG_Palace");
        assert_eq!(palace.source.key, "BLDG[0]");
        assert!(
            palace
                .fields
                .iter()
                .any(|field| field.name == "cost" && field.value == "10")
        );
        assert!(palace.fields.iter().any(|field| field.name == "flags_hex"));

        let fascism = find_source(ContentKind::Technology, "TECH_Fascism");
        assert_eq!(fascism.source.record_indices, [82]);
        assert!(
            fascism
                .fields
                .iter()
                .any(|field| field.name == "prerequisite_4_index")
        );

        let rome = find_source(ContentKind::Nation, "rome");
        assert_eq!(rome.source.section, "RACE");
        assert_eq!(rome.source.record_indices, [1]);
    }
}

#[cfg(test)]
mod government_catalog_tests {
    use super::{
        ContentKind, GovernmentRulesetCatalog, Requirement, RequirementContext,
        RequirementEvaluationError, RequirementRange, RequirementValue, RulesetData,
    };

    struct GovernmentContext {
        government: String,
    }

    impl RequirementContext for GovernmentContext {
        fn is_present(
            &self,
            requirement: &Requirement,
        ) -> Result<bool, RequirementEvaluationError> {
            Ok(requirement.kind.eq_ignore_ascii_case("Gov")
                && requirement.range == RequirementRange::Player
                && matches!(
                    &requirement.name,
                    RequirementValue::String(name)
                        if name.eq_ignore_ascii_case(&self.government)
                ))
        }
    }

    impl super::EffectContext for GovernmentContext {
        fn multiplier_percent(&self, _multiplier: &str) -> Option<i64> {
            None
        }
    }

    fn campaign_governments() -> GovernmentRulesetCatalog {
        RulesetData::campaign_default()
            .government_catalog()
            .unwrap()
    }

    #[test]
    fn campaign_government_catalog_exposes_order_requirements_flags_and_rates() {
        let catalog = campaign_governments();
        assert_eq!(
            catalog.during_revolution_id, "Anarchy",
            "governments.ruleset selects Anarchy during revolution"
        );
        assert_eq!(
            catalog
                .governments
                .iter()
                .map(|government| government.id.as_str())
                .collect::<Vec<_>>(),
            [
                "Anarchy",
                "Tribal",
                "Despotism",
                "Monarchy",
                "Communism",
                "Fundamentalism",
                "Federation",
                "Republic",
                "Democracy",
            ],
            "records preserve selected ruleset order"
        );

        let anarchy = catalog.get("anarchy").unwrap();
        assert_eq!(anarchy.name, "Anarchy");
        assert!(anarchy.requirements.is_empty());

        let monarchy = catalog.get("Monarchy").unwrap();
        assert_eq!(monarchy.requirements.len(), 1);
        assert_eq!(monarchy.requirements[0].kind.to_ascii_lowercase(), "tech");
        assert_eq!(
            monarchy.requirements[0].name,
            RequirementValue::String("Monarchy".to_owned())
        );
        assert_eq!(monarchy.requirements[0].range, RequirementRange::Player);
        assert_eq!(
            catalog.get("Tribal").unwrap().ai_better.as_deref(),
            Some("Republic")
        );
        assert!(
            catalog
                .get("Despotism")
                .unwrap()
                .flags
                .iter()
                .any(|flag| flag == "ShieldToGold")
        );

        let tribal = catalog.get("Tribal").unwrap();
        let rate = tribal
            .effect_value(
                "Max_Rates",
                &GovernmentContext {
                    government: "Tribal".to_owned(),
                },
            )
            .unwrap();
        assert_eq!(rate, 60, "the rate comes from the selected effects.ruleset");
    }

    #[test]
    fn government_catalog_rejects_bad_revolution_ids_duplicate_ids_and_attributes() {
        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .iter_mut()
            .find(|parameter| {
                parameter.file == "governments.ruleset"
                    && parameter.section == "governments"
                    && parameter.key == "during_revolution"
            })
            .unwrap()
            .value = super::Value::String("MissingGovernment".to_owned());
        assert!(rules.government_catalog().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let tribal = rules
            .content
            .iter_mut()
            .find(|record| record.kind == ContentKind::Government && record.rule_name == "Tribal")
            .unwrap();
        tribal.rule_name = "Despotism".to_owned();
        assert!(rules.government_catalog().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let tribal = rules
            .content
            .iter_mut()
            .find(|record| record.kind == ContentKind::Government && record.rule_name == "Tribal")
            .unwrap();
        tribal
            .fields
            .iter_mut()
            .find(|field| field.key == "ai_better")
            .unwrap()
            .value = super::Value::String("MissingGovernment".to_owned());
        assert!(rules.government_catalog().is_err());
    }
}
