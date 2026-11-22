//! The campaign's engine rules: effects, action enablers, extras, disasters and
//! settings, as Rust tables, plus the shared requirement evaluator.
//!
//! This module owns data shapes and shared conditional evaluation. It does not
//! own gameplay transitions; simulation subsystems provide a context that
//! resolves rule universals against their current state.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod action_enablers;
mod campaign;
mod combat;
mod disasters;
mod effect_table;
mod evaluator;
mod extras;
mod great_leader;
mod identifiers;
mod loader;
mod settlement_ai;
mod trade_routes;
mod veterancy;
pub(crate) use campaign::{
    CivilWarRulesetSettings, DISASTER_FREQUENCY, INITIAL_CITY_RADIUS_SQ, MIGRATION_PML,
    VeterancySettings,
};
pub(crate) use combat::{
    CombatBonusKind, CombatRulesetSettings, TransportDefense, UnitCombatBonus,
};
pub(crate) use disasters::{Disaster, DisasterEffect, campaign_disasters};
#[cfg(test)]
pub(crate) use evaluator::action_enabled;
use evaluator::evaluate_action_enablers;
pub(crate) use evaluator::{
    ActionEvaluationError, EffectContext, EffectEvaluationError, RequirementContext,
    RequirementEvaluationError, campaign_effect, effect_value, requirements_met,
};
pub(crate) use extras::{
    ExtraRule, RoadRule, campaign_extras, extra_by_any_name, extra_by_name, road_by_name,
};
pub(crate) use great_leader::GreatLeaderSettings;
pub(crate) use identifiers::{EffectKind, RequirementKind};
pub(crate) use settlement_ai::SettlementAiSettings;
pub(crate) use trade_routes::{
    CaravanBonusStyle, GoodsSelection, TradeGoodFlag, TradeGoodRulesetRecord, TradeRevenueStyle,
    TradeRouteBonus, TradeRouteCancellation, TradeRouteRulesetSettings, TradeRouteType,
    TradeRouteTypeSettings,
};
pub(crate) use veterancy::{VeteranLevel, VeteranSystem};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RulesetData {
    pub(crate) effects: EffectSet,
    pub(crate) action_enablers: Vec<ActionEnabler>,
    action_index: RuleNameIndex,
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
#[allow(
    dead_code,
    reason = "the tables name universals by string; numeric values are part of the Freeciv model"
)]
pub(crate) enum RequirementValue {
    String(String),
    Integer(i64),
    Bool(bool),
}

#[allow(
    dead_code,
    reason = "complete Freeciv range set; the tables use a subset"
)]
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
    /// Effects of one engine type, in table order.
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

    /// Cache the single CivCraft campaign package once.
    pub(crate) fn campaign_default() -> &'static Self {
        static RULESET: OnceLock<RulesetData> = OnceLock::new();
        RULESET.get_or_init(|| {
            Self::build_campaign()
                .unwrap_or_else(|error| panic!("failed to load campaign ruleset: {error}"))
        })
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

impl LoadError {
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
}

#[cfg(test)]
mod tests;
