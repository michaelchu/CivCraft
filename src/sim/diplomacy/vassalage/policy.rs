//! Validated mechanical and AI thresholds from content data.
use super::*;
use std::sync::OnceLock;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub(super) required_technology: String,
    pub(super) voluntary_minimum_turns: i32,
    pub(super) independence_size_percent: i64,
    pub(super) retained_land_percent: i64,
    pub(super) negotiation_cooldown_turns: i32,
    pub(super) submission_force_ratio: i64,
    pub(super) voluntary_minimum_trust: i32,
    pub(super) minimum_peace_value: i64,
    pub(super) maximum_protection_pressure_percent: i64,
}
pub(super) fn policy() -> &'static Policy {
    static POLICY: OnceLock<Policy> = OnceLock::new();
    POLICY.get_or_init(|| {
        let p: Policy = toml::from_str(include_str!(
            "../../../../data/rulesets/civcraft/content/vassalage.toml"
        ))
        .expect("valid vassalage policy");
        assert!(Tech::all().any(|t| t.name() == p.required_technology));
        assert!(p.voluntary_minimum_turns > 0 && p.negotiation_cooldown_turns > 0);
        assert!((1..=100).contains(&p.independence_size_percent));
        assert!((1..=100).contains(&p.retained_land_percent));
        assert!(p.submission_force_ratio > 1 && p.minimum_peace_value > 0);
        assert!((1..=1000).contains(&p.maximum_protection_pressure_percent));
        assert!((0..=10000).contains(&p.voluntary_minimum_trust));
        p
    })
}
