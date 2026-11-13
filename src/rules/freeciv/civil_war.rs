//! Selected ruleset settings used by Freeciv's city-conquest civil-war path.

use super::{LoadError, RulesetData, Value, required_setting};

/// Freeciv's default campaign limits relevant to civil-war creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CivilWarRulesetSettings {
    pub(crate) city_threshold: usize,
    pub(crate) max_players: usize,
}

impl RulesetData {
    /// Read the editable server defaults recorded by the selected campaign
    /// ruleset. The engine's civilwarsize maximum (1000) disables the feature.
    pub(crate) fn civil_war_settings(&self) -> Result<CivilWarRulesetSettings, LoadError> {
        let integer = |name: &str| -> Result<usize, LoadError> {
            let setting = required_setting(self, name)?;
            let Value::Integer(value) = &setting.value else {
                return Err(LoadError::new(format!(
                    "game setting `{name}` must be an integer, found {:?}",
                    setting.value
                )));
            };
            usize::try_from(*value)
                .map_err(|_| LoadError::new(format!("game setting `{name}` must be nonnegative")))
        };

        let city_threshold = integer("civilwarsize")?;
        if !(2..=1000).contains(&city_threshold) {
            return Err(LoadError::new(format!(
                "game setting `civilwarsize` must be in 2..=1000, found {city_threshold}"
            )));
        }
        let max_players = integer("maxplayers")?;
        if !(1..=500).contains(&max_players) {
            return Err(LoadError::new(format!(
                "game setting `maxplayers` must be in 1..=500, found {max_players}"
            )));
        }
        Ok(CivilWarRulesetSettings {
            city_threshold,
            max_players,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_civil_war_limits_match_freeciv_defaults() {
        assert_eq!(
            RulesetData::campaign_default()
                .civil_war_settings()
                .unwrap(),
            CivilWarRulesetSettings {
                city_threshold: 10,
                max_players: 500,
            }
        );
    }
}
