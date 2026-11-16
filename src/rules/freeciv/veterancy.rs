//! Typed veterancy data from the selected Freeciv `units.ruleset`.

use super::{ContentKind, LoadError, RulesetData};
use crate::rules::freeciv_ini::Value;

const MAX_VETERAN_LEVELS: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VeteranLevel {
    pub(crate) name: String,
    pub(crate) base_raise_chance: i32,
    pub(crate) work_raise_chance: i32,
    pub(crate) power_factor: i32,
    pub(crate) move_bonus: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VeteranSystem {
    pub(crate) levels: Vec<VeteranLevel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VeterancySettings {
    pub(crate) upgrade_veteran_loss: usize,
    pub(crate) autoupgrade_veteran_loss: usize,
}

impl VeteranSystem {
    pub(crate) fn level(&self, index: usize) -> Option<&VeteranLevel> {
        self.levels.get(index)
    }
}

impl RulesetData {
    /// Resolve one selected unit type's veteran system, or the selected engine
    /// default when the type has no override. `unit_section` must be an
    /// explicit C2C3 unit section already bound by the campaign adapter.
    pub(crate) fn veteran_system(
        &self,
        unit_section: Option<&str>,
    ) -> Result<VeteranSystem, LoadError> {
        let global = self.parse_global_veteran_system()?;
        let Some(unit_section) = unit_section else {
            return Ok(global);
        };
        let mut records = self.content.iter().filter(|record| {
            record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(unit_section)
        });
        let Some(record) = records.next() else {
            return Err(LoadError::new(format!(
                "selected ruleset has no unit section [{unit_section}]"
            )));
        };
        if records.next().is_some() {
            return Err(LoadError::new(format!(
                "selected ruleset has duplicate unit section [{unit_section}]"
            )));
        }

        let fields = [
            "veteran_names",
            "veteran_base_raise_chance",
            "veteran_work_raise_chance",
            "veteran_power_fact",
            "veteran_move_bonus",
        ];
        let present = fields
            .iter()
            .filter(|field| {
                record
                    .fields
                    .iter()
                    .any(|entry| entry.key.eq_ignore_ascii_case(field))
            })
            .count();
        if present == 0 {
            return Ok(global);
        }
        if present != fields.len() {
            return Err(LoadError::new(format!(
                "selected [{unit_section}] defines only {present}/{} veteran fields",
                fields.len()
            )));
        }

        let names = record_values(&record.fields, "veteran_names")?
            .into_iter()
            .map(|value| match value {
                Value::String(name) => Ok(name),
                other => Err(LoadError::new(format!(
                    "selected [{unit_section}].veteran_names contains non-string value {other:?}"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let chances = record_ints(&record.fields, "veteran_base_raise_chance", unit_section)?;
        let work = record_ints(&record.fields, "veteran_work_raise_chance", unit_section)?;
        let power = record_ints(&record.fields, "veteran_power_fact", unit_section)?;
        let movement = record_ints(&record.fields, "veteran_move_bonus", unit_section)?;
        VeteranSystem::from_parts(
            &format!("[{unit_section}]"),
            names,
            chances,
            work,
            power,
            movement,
        )
    }

    /// Read the selected ruleset's unit-upgrade veteran losses. These values
    /// are in `game.ruleset [civstyle]`; they never come from a fallback
    /// gameplay ruleset.
    pub(crate) fn veterancy_settings(&self) -> Result<VeterancySettings, LoadError> {
        let read_loss = |key: &str| -> Result<usize, LoadError> {
            let parameter = super::required_parameter(self, "game.ruleset", "civstyle", key)?;
            let Value::Integer(value) = &parameter.value else {
                return Err(LoadError::new(format!(
                    "selected game.ruleset [civstyle].{key} must be an integer"
                )));
            };
            let loss = usize::try_from(*value).map_err(|_| {
                LoadError::new(format!(
                    "selected game.ruleset [civstyle].{key} must be nonnegative"
                ))
            })?;
            if loss > MAX_VETERAN_LEVELS {
                return Err(LoadError::new(format!(
                    "selected game.ruleset [civstyle].{key} exceeds the Freeciv limit of {MAX_VETERAN_LEVELS}"
                )));
            }
            Ok(loss)
        };
        Ok(VeterancySettings {
            upgrade_veteran_loss: read_loss("upgrade_veteran_loss")?,
            autoupgrade_veteran_loss: read_loss("autoupgrade_veteran_loss")?,
        })
    }

    fn parse_global_veteran_system(&self) -> Result<VeteranSystem, LoadError> {
        let names = parameter_values(self, "veteran_names")?
            .into_iter()
            .map(|value| match value {
                Value::String(name) => Ok(name),
                other => Err(LoadError::new(format!(
                    "selected units.ruleset [veteran_system].veteran_names contains non-string value {other:?}"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let chances = parameter_ints(self, "veteran_base_raise_chance")?;
        let work = parameter_ints(self, "veteran_work_raise_chance")?;
        let power = parameter_ints(self, "veteran_power_fact")?;
        let movement = parameter_ints(self, "veteran_move_bonus")?;
        VeteranSystem::from_parts(
            "units.ruleset [veteran_system]",
            names,
            chances,
            work,
            power,
            movement,
        )
    }
}

impl VeteranSystem {
    fn from_parts(
        source: &str,
        names: Vec<String>,
        chances: Vec<i32>,
        work: Vec<i32>,
        power: Vec<i32>,
        movement: Vec<i32>,
    ) -> Result<Self, LoadError> {
        let count = names.len();
        if count > MAX_VETERAN_LEVELS {
            return Err(LoadError::new(format!(
                "{source} has {count} veteran levels, exceeding the Freeciv limit of {MAX_VETERAN_LEVELS}"
            )));
        }
        if count == 0
            || [chances.len(), work.len(), power.len(), movement.len()]
                .into_iter()
                .any(|length| length != count)
        {
            return Err(LoadError::new(format!(
                "{source} has empty or unequal veteran field lengths (names={count}, chance={}, work={}, power={}, move={})",
                chances.len(),
                work.len(),
                power.len(),
                movement.len()
            )));
        }
        if power[0] != 100 {
            return Err(LoadError::new(format!(
                "{source} first veteran power factor must be 100, found {}",
                power[0]
            )));
        }
        for index in 0..count {
            if chances[index] < 0 || work[index] < 0 || power[index] < 0 || movement[index] < 0 {
                return Err(LoadError::new(format!(
                    "{source} veteran field at level {index} is negative"
                )));
            }
            if index > 0 && power[index] < power[index - 1] {
                return Err(LoadError::new(format!(
                    "{source} veteran power decreases at level {index}"
                )));
            }
            if index + 1 < count && (chances[index] > 100 || work[index] > 100) {
                return Err(LoadError::new(format!(
                    "{source} veteran raise chance at level {index} exceeds 100"
                )));
            }
        }
        if chances[count - 1] != 0 || work[count - 1] != 0 {
            return Err(LoadError::new(format!(
                "{source} final veteran level must have zero raise and work chances"
            )));
        }
        Ok(Self {
            levels: (0..count)
                .map(|index| VeteranLevel {
                    name: names[index].clone(),
                    base_raise_chance: chances[index],
                    work_raise_chance: work[index],
                    power_factor: power[index],
                    move_bonus: movement[index],
                })
                .collect(),
        })
    }
}

fn parameter_values(rules: &RulesetData, key: &str) -> Result<Vec<Value>, LoadError> {
    let parameter = super::required_parameter(rules, "units.ruleset", "veteran_system", key)?;
    values_from_value(
        &parameter.value,
        &format!("units.ruleset [veteran_system].{key}"),
    )
}

fn parameter_ints(rules: &RulesetData, key: &str) -> Result<Vec<i32>, LoadError> {
    integer_values(
        parameter_values(rules, key)?,
        &format!("units.ruleset [veteran_system].{key}"),
    )
}

fn record_values(
    entries: &[crate::rules::freeciv_ini::Entry],
    key: &str,
) -> Result<Vec<Value>, LoadError> {
    let mut found = entries
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key));
    let Some(entry) = found.next() else {
        return Err(LoadError::new(format!("missing veteran field {key}")));
    };
    if found.next().is_some() {
        return Err(LoadError::new(format!("repeated veteran field {key}")));
    }
    values_from_value(&entry.value, key)
}

fn record_ints(
    entries: &[crate::rules::freeciv_ini::Entry],
    key: &str,
    section: &str,
) -> Result<Vec<i32>, LoadError> {
    integer_values(record_values(entries, key)?, &format!("[{section}].{key}"))
}

fn values_from_value(value: &Value, source: &str) -> Result<Vec<Value>, LoadError> {
    match value {
        Value::List(values) => Ok(values.clone()),
        Value::String(_) | Value::Integer(_) => Ok(vec![value.clone()]),
        other => Err(LoadError::new(format!(
            "{source} must be a list or scalar, found {other:?}"
        ))),
    }
}

fn integer_values(values: Vec<Value>, source: &str) -> Result<Vec<i32>, LoadError> {
    values
        .into_iter()
        .map(|value| match value {
            Value::Integer(value) => i32::try_from(value).map_err(|_| {
                LoadError::new(format!("{source} contains an integer outside i32 range"))
            }),
            other => Err(LoadError::new(format!(
                "{source} contains non-integer value {other:?}"
            ))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c2c3_global_veteran_system_is_parsed_from_selected_ruleset() {
        let rules = RulesetData::campaign_default();
        let system = rules.veteran_system(None).unwrap();
        assert_eq!(system.levels.len(), 4);
        assert_eq!(
            system
                .levels
                .iter()
                .map(|level| (level.base_raise_chance, level.power_factor))
                .collect::<Vec<_>>(),
            [(50, 100), (33, 150), (20, 175), (0, 200)]
        );
        assert!(system.levels.iter().all(|level| level.move_bonus == 0));
    }

    #[test]
    fn unit_overrides_replace_the_selected_global_veteran_system() {
        let rules = RulesetData::campaign_default();
        let global = rules.veteran_system(None).unwrap();
        let warrior = rules.veteran_system(Some("unit_warriors")).unwrap();
        assert_eq!(warrior, global);

        let settler = rules.veteran_system(Some("unit_settlers")).unwrap();
        assert_eq!(settler.levels.len(), 1);
        assert_eq!(settler.levels[0].base_raise_chance, 0);
        assert_eq!(settler.levels[0].power_factor, 100);

        let spy = rules.veteran_system(Some("unit_spy")).unwrap();
        assert_eq!(
            spy.levels
                .iter()
                .map(|level| level.power_factor)
                .collect::<Vec<_>>(),
            [100, 105, 110, 115]
        );
    }

    #[test]
    fn upgrade_veterancy_losses_are_read_from_selected_civstyle() {
        assert_eq!(
            RulesetData::campaign_default()
                .veterancy_settings()
                .unwrap(),
            VeterancySettings {
                upgrade_veteran_loss: 1,
                autoupgrade_veteran_loss: 1,
            }
        );
    }
}
