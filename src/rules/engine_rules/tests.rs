//! Requirement, enabler and effect evaluator tests.

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
    fn is_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
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
