//! Cached access to the selected ruleset's combat settings.

use crate::rules::freeciv::{CombatRulesetSettings, RulesetData};
use std::sync::OnceLock;

pub(super) fn selected() -> &'static CombatRulesetSettings {
    static SETTINGS: OnceLock<CombatRulesetSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        RulesetData::campaign_default()
            .combat_settings()
            .expect("campaign C2C3 combat settings are valid")
    })
}
