//! Typed selected-ruleset combat settings.

use super::{
    ContentKind, ContentRecord, LoadError, RulesetData, Value, required_parameter, required_setting,
};

/// C2C3 combat-bonus operations supported by Freeciv's combat calculations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CombatBonusKind {
    DefenseMultiplier,
    DefenseMultiplierPct,
    DefenseDivider,
    DefenseDividerPct,
    LowFirepower,
}

/// Selected-ruleset policy for defending while carried by a transporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransportDefense {
    Blocked,
    Alight,
    Always,
}

/// A unit's bonus against a target unit-type flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnitCombatBonus {
    pub(crate) flag: String,
    pub(crate) kind: CombatBonusKind,
    pub(crate) value: i32,
}

/// Combat parameters and game settings that affect Freeciv's military rules.
/// Every field is read from the selected C2C3 derivative; `killstack` and
/// `killcitizen` were not written there upstream, so CivCraft records the
/// pinned engine defaults explicitly in its `[settings]` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CombatRulesetSettings {
    pub(crate) tired_attack: bool,
    pub(crate) only_killing_makes_veteran: bool,
    pub(crate) only_real_fight_makes_veteran: bool,
    pub(crate) combat_odds_scaled_veterancy: bool,
    pub(crate) damage_reduces_bombard_rate: bool,
    pub(crate) low_firepower_badwallattacker: i32,
    pub(crate) low_firepower_pearl_harbor: i32,
    pub(crate) low_firepower_combat_bonus: i32,
    pub(crate) low_firepower_nonnat_bombard: i32,
    pub(crate) nuke_pop_loss_pct: i32,
    pub(crate) nuke_defender_survival_chance_pct: i32,
    pub(crate) killstack: bool,
    pub(crate) killcitizen: bool,
    pub(crate) unreachable_protects: bool,
    pub(crate) occupychance: i32,
    pub(crate) razechance: i32,
    pub(crate) savepalace: bool,
    pub(crate) homecaughtunits: bool,
}

impl RulesetData {
    /// Resolve a selected unit type's `tp_defense` policy. Freeciv's ruleset
    /// loader defaults an omitted field to `TDT_ALIGHT` (`server/ruleset/
    /// ruleload.c`); unmapped Civ III units use that engine default only after
    /// their C2C3 class mapping has been established by the caller.
    pub(crate) fn unit_transport_defense(
        &self,
        unit_section: &str,
    ) -> Result<TransportDefense, LoadError> {
        let mut records = self.content.iter().filter(|record| {
            record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(unit_section)
        });
        let record = records.next().ok_or_else(|| {
            LoadError::new(format!(
                "selected ruleset has no unit section [{unit_section}]"
            ))
        })?;
        if records.next().is_some() {
            return Err(LoadError::new(format!(
                "selected ruleset has duplicate unit section [{unit_section}]"
            )));
        }
        let value = record
            .string_field("tp_defense")
            .map_err(|error| {
                LoadError::new(format!("invalid [{unit_section}].tp_defense: {error}"))
            })?
            .unwrap_or("Alight");
        match value.to_ascii_lowercase().as_str() {
            "blocked" => Ok(TransportDefense::Blocked),
            "alight" => Ok(TransportDefense::Alight),
            "always" => Ok(TransportDefense::Always),
            _ => Err(LoadError::new(format!(
                "selected [{unit_section}].tp_defense has unsupported value {value:?}"
            ))),
        }
    }

    /// Read the combat-bonus table from one selected-ruleset unit section.
    /// A unit without a `bonuses` field has no combat bonuses.
    pub(crate) fn unit_combat_bonuses(
        &self,
        unit_section: &str,
    ) -> Result<Vec<UnitCombatBonus>, LoadError> {
        let mut records = self.content.iter().filter(|record| {
            record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(unit_section)
        });
        let record = records.next().ok_or_else(|| {
            LoadError::new(format!(
                "selected ruleset has no unit section [{unit_section}]"
            ))
        })?;
        if records.next().is_some() {
            return Err(LoadError::new(format!(
                "selected ruleset has duplicate unit section [{unit_section}]"
            )));
        }

        parse_unit_combat_bonuses(record)
    }

    /// Load the required combat parameters and combat-related game settings
    /// from the selected ruleset. Missing or malformed inputs fail closed;
    /// this adapter does not borrow values from another ruleset.
    pub(crate) fn combat_settings(&self) -> Result<CombatRulesetSettings, LoadError> {
        let boolean_parameter = |name: &str| -> Result<bool, LoadError> {
            let parameter = required_parameter(self, "game.ruleset", "combat_rules", name)?;
            match &parameter.value {
                Value::Bool(value) => Ok(*value),
                other => Err(LoadError::new(format!(
                    "game.ruleset [combat_rules].{name} must be a boolean, found {other:?}"
                ))),
            }
        };
        let integer_parameter = |name: &str| -> Result<i32, LoadError> {
            let parameter = required_parameter(self, "game.ruleset", "combat_rules", name)?;
            let value = parameter
                .integer_value()
                .map_err(|error| LoadError::new(format!("invalid combat rules: {error}")))?
                .ok_or_else(|| {
                    LoadError::new(format!(
                        "game.ruleset [combat_rules].{name} has no integer value"
                    ))
                })?;
            i32::try_from(value).map_err(|_| {
                LoadError::new(format!(
                    "game.ruleset [combat_rules].{name} is outside the supported integer range"
                ))
            })
        };
        let percentage_parameter = |name: &str| -> Result<i32, LoadError> {
            let value = integer_parameter(name)?;
            if !(0..=100).contains(&value) {
                return Err(LoadError::new(format!(
                    "game.ruleset [combat_rules].{name} must be in 0..=100, found {value}"
                )));
            }
            Ok(value)
        };
        let boolean_setting = |name: &str| -> Result<bool, LoadError> {
            let setting = required_setting(self, name)?;
            match &setting.value {
                Value::Bool(value) => Ok(*value),
                other => Err(LoadError::new(format!(
                    "game setting `{name}` must be a boolean, found {other:?}"
                ))),
            }
        };
        let control_boolean_setting = |name: &str| -> Result<bool, LoadError> {
            let setting = required_setting(self, name)?;
            match &setting.value {
                Value::Bool(value) => Ok(*value),
                Value::String(value) if value.eq_ignore_ascii_case("ENABLED") => Ok(true),
                Value::String(value) if value.eq_ignore_ascii_case("DISABLED") => Ok(false),
                other => Err(LoadError::new(format!(
                    "game setting `{name}` must be a boolean or ENABLED/DISABLED control, found {other:?}"
                ))),
            }
        };
        let percentage_setting = |name: &str| -> Result<i32, LoadError> {
            let setting = required_setting(self, name)?;
            let value = match &setting.value {
                Value::Integer(value) => i32::try_from(*value).map_err(|_| {
                    LoadError::new(format!("game setting `{name}` is outside i32 range"))
                })?,
                other => {
                    return Err(LoadError::new(format!(
                        "game setting `{name}` must be an integer, found {other:?}"
                    )));
                }
            };
            if !(0..=100).contains(&value) {
                return Err(LoadError::new(format!(
                    "game setting `{name}` must be in 0..=100, found {value}"
                )));
            }
            Ok(value)
        };

        Ok(CombatRulesetSettings {
            tired_attack: boolean_parameter("tired_attack")?,
            only_killing_makes_veteran: boolean_parameter("only_killing_makes_veteran")?,
            only_real_fight_makes_veteran: boolean_parameter("only_real_fight_makes_veteran")?,
            combat_odds_scaled_veterancy: boolean_parameter("combat_odds_scaled_veterancy")?,
            damage_reduces_bombard_rate: boolean_parameter("damage_reduces_bombard_rate")?,
            low_firepower_badwallattacker: integer_parameter("low_firepower_badwallattacker")?,
            low_firepower_pearl_harbor: integer_parameter("low_firepower_pearl_harbor")?,
            low_firepower_combat_bonus: integer_parameter("low_firepower_combat_bonus")?,
            low_firepower_nonnat_bombard: integer_parameter("low_firepower_nonnat_bombard")?,
            nuke_pop_loss_pct: percentage_parameter("nuke_pop_loss_pct")?,
            nuke_defender_survival_chance_pct: percentage_parameter(
                "nuke_defender_survival_chance_pct",
            )?,
            killstack: boolean_setting("killstack")?,
            killcitizen: boolean_setting("killcitizen")?,
            unreachable_protects: control_boolean_setting("unreachableprotects")?,
            occupychance: percentage_setting("occupychance")?,
            razechance: percentage_setting("razechance")?,
            savepalace: boolean_setting("savepalace")?,
            homecaughtunits: boolean_setting("homecaughtunits")?,
        })
    }
}

fn parse_unit_combat_bonuses(record: &ContentRecord) -> Result<Vec<UnitCombatBonus>, LoadError> {
    let mut fields = record
        .fields
        .iter()
        .filter(|field| field.key.eq_ignore_ascii_case("bonuses"));
    let Some(entry) = fields.next() else {
        return Ok(Vec::new());
    };
    if fields.next().is_some() {
        return Err(LoadError::new(format!(
            "{}.bonuses is specified more than once",
            record.section
        )));
    }

    let Value::Table { columns, rows } = &entry.value else {
        return Err(LoadError::new(format!(
            "{}.bonuses must be a table, found {:?}",
            record.section, entry.value
        )));
    };

    let mut column_indices = std::collections::BTreeMap::new();
    for (index, column) in columns.iter().enumerate() {
        let normalized = column.to_ascii_lowercase();
        if !matches!(normalized.as_str(), "flag" | "type" | "value" | "quiet") {
            return Err(LoadError::new(format!(
                "{}.bonuses has unsupported column {column:?}",
                record.section
            )));
        }
        if column_indices.insert(normalized.clone(), index).is_some() {
            return Err(LoadError::new(format!(
                "{}.bonuses has duplicate {column:?} column",
                record.section
            )));
        }
    }
    let column_index = |column: &str| {
        column_indices.get(column).copied().ok_or_else(|| {
            LoadError::new(format!(
                "{}.bonuses is missing the {column:?} column",
                record.section
            ))
        })
    };
    let flag_index = column_index("flag")?;
    let type_index = column_index("type")?;
    let value_index = column_index("value")?;
    let quiet_index = column_indices.get("quiet").copied();

    rows.iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_number = row_index + 1;
            if row.len() != columns.len() {
                return Err(LoadError::new(format!(
                    "{}.bonuses row {row_number} has {} values for {} columns",
                    record.section,
                    row.len(),
                    columns.len()
                )));
            }

            let flag = match &row[flag_index] {
                Value::String(flag) if !flag.trim().is_empty() => flag.clone(),
                other => {
                    return Err(LoadError::new(format!(
                        "{}.bonuses row {row_number} flag must be a nonempty string, found {other:?}",
                        record.section
                    )));
                }
            };
            let kind = match &row[type_index] {
                Value::String(kind) => parse_combat_bonus_kind(kind).ok_or_else(|| {
                    LoadError::new(format!(
                        "{}.bonuses row {row_number} has unsupported combat bonus type {kind:?}",
                        record.section
                    ))
                })?,
                other => {
                    return Err(LoadError::new(format!(
                        "{}.bonuses row {row_number} type must be a string, found {other:?}",
                        record.section
                    )));
                }
            };
            let value = match &row[value_index] {
                Value::Integer(value) => i32::try_from(*value).map_err(|_| {
                    LoadError::new(format!(
                        "{}.bonuses row {row_number} value is outside i32 range",
                        record.section
                    ))
                })?,
                other => {
                    return Err(LoadError::new(format!(
                        "{}.bonuses row {row_number} value must be an integer, found {other:?}",
                        record.section
                    )));
                }
            };
            if let Some(quiet_index) = quiet_index
                && !matches!(row[quiet_index], Value::Bool(_))
            {
                return Err(LoadError::new(format!(
                    "{}.bonuses row {row_number} quiet must be a boolean, found {:?}",
                    record.section, row[quiet_index]
                )));
            }

            Ok(UnitCombatBonus { flag, kind, value })
        })
        .collect()
}

fn parse_combat_bonus_kind(name: &str) -> Option<CombatBonusKind> {
    match name.to_ascii_lowercase().as_str() {
        "defensemultiplier" => Some(CombatBonusKind::DefenseMultiplier),
        "defensemultiplierpct" => Some(CombatBonusKind::DefenseMultiplierPct),
        "defensedivider" => Some(CombatBonusKind::DefenseDivider),
        "defensedividerpct" => Some(CombatBonusKind::DefenseDividerPct),
        "lowfirepower" => Some(CombatBonusKind::LowFirepower),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_record_mut<'a>(rules: &'a mut RulesetData, section: &str) -> &'a mut ContentRecord {
        rules
            .content
            .iter_mut()
            .find(|record| {
                record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(section)
            })
            .unwrap()
    }

    fn bonuses_entry_mut(record: &mut ContentRecord) -> &mut super::super::Entry {
        record
            .fields
            .iter_mut()
            .find(|field| field.key.eq_ignore_ascii_case("bonuses"))
            .unwrap()
    }

    #[test]
    fn selected_unit_combat_bonuses_parse_destroyer_and_aegis_records() {
        let rules = RulesetData::campaign_default();
        assert_eq!(
            rules.unit_combat_bonuses("unit_destroyer").unwrap(),
            vec![UnitCombatBonus {
                flag: "Submarine".to_owned(),
                kind: CombatBonusKind::DefenseMultiplier,
                value: 1,
            }]
        );
        assert_eq!(
            rules.unit_combat_bonuses("unit_aegis_cruiser").unwrap(),
            vec![UnitCombatBonus {
                flag: "AirAttacker".to_owned(),
                kind: CombatBonusKind::DefenseMultiplier,
                value: 4,
            }]
        );
        assert!(
            rules
                .unit_combat_bonuses("unit_submarine")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unit_transport_defense_uses_selected_values_and_engine_default() {
        let rules = RulesetData::campaign_default();
        assert_eq!(
            rules.unit_transport_defense("unit_settlers").unwrap(),
            TransportDefense::Alight
        );
        assert_eq!(
            rules.unit_transport_defense("unit_migrants").unwrap(),
            TransportDefense::Alight
        );

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_settlers");
        record
            .fields
            .retain(|field| !field.key.eq_ignore_ascii_case("tp_defense"));
        assert_eq!(
            rules.unit_transport_defense("unit_settlers").unwrap(),
            TransportDefense::Alight
        );

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_settlers");
        let field = record
            .fields
            .iter_mut()
            .find(|field| field.key.eq_ignore_ascii_case("tp_defense"))
            .unwrap();
        field.value = Value::String("blocked".to_owned());
        assert_eq!(
            rules.unit_transport_defense("unit_settlers").unwrap(),
            TransportDefense::Blocked
        );

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_settlers");
        let field = record
            .fields
            .iter_mut()
            .find(|field| field.key.eq_ignore_ascii_case("tp_defense"))
            .unwrap();
        field.value = Value::String("defend_always".to_owned());
        assert!(rules.unit_transport_defense("unit_settlers").is_err());
    }

    #[test]
    fn unit_combat_bonus_parser_rejects_malformed_and_unsupported_data() {
        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let entry = record
            .fields
            .iter()
            .find(|field| field.key == "bonuses")
            .unwrap()
            .clone();
        record.fields.push(entry);
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { rows, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        rows[0][2] = Value::String("1".to_owned());
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { rows, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        rows[0][1] = Value::String("UnknownBonus".to_owned());
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { rows, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        rows[0].pop();
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        bonuses_entry_mut(record).value = Value::List(Vec::new());
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());
    }

    #[test]
    fn unit_combat_bonus_parser_rejects_invalid_columns_and_row_types() {
        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { columns, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        columns[0] = "target".to_owned();
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { columns, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        columns.push("flag".to_owned());
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { rows, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        rows[0][0] = Value::Integer(1);
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { rows, .. } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        rows[0][1] = Value::Bool(true);
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let record = unit_record_mut(&mut rules, "unit_destroyer");
        let Value::Table { columns, rows } = &mut bonuses_entry_mut(record).value else {
            panic!("campaign bonuses are a table");
        };
        columns.push("quiet".to_owned());
        rows[0].push(Value::String("FALSE".to_owned()));
        assert!(rules.unit_combat_bonuses("unit_destroyer").is_err());
    }

    #[test]
    fn campaign_combat_settings_load_selected_c2c3_values() {
        assert_eq!(
            RulesetData::campaign_default().combat_settings().unwrap(),
            CombatRulesetSettings {
                tired_attack: true,
                only_killing_makes_veteran: false,
                only_real_fight_makes_veteran: true,
                combat_odds_scaled_veterancy: true,
                damage_reduces_bombard_rate: true,
                low_firepower_badwallattacker: 1,
                low_firepower_pearl_harbor: 1,
                low_firepower_combat_bonus: 1,
                low_firepower_nonnat_bombard: 1,
                nuke_pop_loss_pct: 49,
                nuke_defender_survival_chance_pct: 0,
                killstack: true,
                killcitizen: true,
                unreachable_protects: true,
                occupychance: 100,
                razechance: 10,
                savepalace: true,
                homecaughtunits: true,
            }
        );
    }

    #[test]
    fn combat_settings_reject_missing_duplicated_and_malformed_values() {
        let mut rules = RulesetData::campaign_default().clone();
        rules.parameters.retain(|parameter| {
            !(parameter.file == "game.ruleset"
                && parameter.section == "combat_rules"
                && parameter.key == "tired_attack")
        });
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        let parameter = rules
            .parameters
            .iter()
            .find(|parameter| {
                parameter.file == "game.ruleset"
                    && parameter.section == "combat_rules"
                    && parameter.key == "tired_attack"
            })
            .unwrap()
            .clone();
        rules.parameters.push(parameter);
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "tired_attack")
            .unwrap()
            .value = Value::Integer(1);
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "killstack")
            .unwrap()
            .value = Value::String("TRUE".to_owned());
        assert!(rules.combat_settings().is_err());
    }

    #[test]
    fn combat_settings_validate_percent_ranges_and_required_game_settings() {
        let mut rules = RulesetData::campaign_default().clone();
        rules
            .parameters
            .iter_mut()
            .find(|parameter| parameter.key == "nuke_pop_loss_pct")
            .unwrap()
            .value = Value::Integer(101);
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "occupychance")
            .unwrap()
            .value = Value::Integer(-1);
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules.settings.retain(|setting| setting.name != "killstack");
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .retain(|setting| setting.name != "unreachableprotects");
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .retain(|setting| setting.name != "killcitizen");
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .retain(|setting| setting.name != "homecaughtunits");
        assert!(rules.combat_settings().is_err());

        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .retain(|setting| setting.name != "savepalace");
        assert!(rules.combat_settings().is_err());
    }

    #[test]
    fn conquest_engine_settings_remain_editable_in_selected_ruleset() {
        let mut rules = RulesetData::campaign_default().clone();
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "razechance")
            .unwrap()
            .value = Value::Integer(37);
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "savepalace")
            .unwrap()
            .value = Value::Bool(false);
        rules
            .settings
            .iter_mut()
            .find(|setting| setting.name == "homecaughtunits")
            .unwrap()
            .value = Value::Bool(false);

        let settings = rules.combat_settings().unwrap();
        assert_eq!(settings.razechance, 37);
        assert!(!settings.savepalace);
        assert!(!settings.homecaughtunits);
    }
}
