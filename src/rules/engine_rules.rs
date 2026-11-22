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
mod tests {
    use super::{
        ActionEnabler, EffectContext, EffectKind, EffectRecord, EffectSet, Requirement,
        RequirementContext, RequirementEvaluationError, RequirementKind, RequirementRange,
        RequirementValue, RuleSourceLocation, RulesetData, action_enabled, effect_value,
        requirements_met,
    };
    use std::path::PathBuf;

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
            },
            ActionEnabler {
                rule_name: "requires_writing".to_owned(),
                action: "Found City".to_owned(),
                actor_requirements: vec![req("Tech", "Writing", true)],
                target_requirements: vec![req("Terrain", "Ocean", false)],
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

    #[test]
    fn campaign_effects_are_loaded_in_table_order() {
        let effects = &RulesetData::campaign_default().effects;
        assert_eq!(effects.len(), 510);
        assert_eq!(effects[0].rule_name, "ai_cheat_food");
        assert!(effects.of_kind(EffectKind::MaxRates).count() > 0);
    }
}
