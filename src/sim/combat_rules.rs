//! Cached access to the selected ruleset's combat settings.

use crate::rules::engine_rules::CombatRulesetSettings;

pub(super) fn selected() -> &'static CombatRulesetSettings {
    &CombatRulesetSettings::CAMPAIGN
}
