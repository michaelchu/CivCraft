//! The shared requirement, action-enabler and effect evaluators.
//!
//! Simulation subsystems implement [`RequirementContext`]; this module owns the
//! logical evaluation so every subsystem applies Freeciv conjunction, negation
//! and percentage-adjustment rules the same way.

use super::{
    ActionEnabler, EffectKind, EffectSet, Requirement, RequirementValue, RulesetData, identifiers,
};

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

pub(super) fn evaluate_action_enablers<'a, A: RequirementContext, T: RequirementContext>(
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
