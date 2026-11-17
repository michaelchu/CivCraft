//! Terrain and road fields used by city production and climate.
use super::*;

#[derive(Clone, Copy)]
pub(crate) struct TerrainYields {
    pub(crate) base: [i32; 3],
    pub(crate) road_percent: [i32; 3],
    pub(crate) irrigation: i32,
    pub(crate) mining: i32,
}

#[derive(Clone, Copy)]
pub(crate) struct RoadYields {
    pub(crate) increment: [i32; 3],
    pub(crate) constant: [i32; 3],
    pub(crate) bonus: [i32; 3],
}

impl RulesetData {
    pub(crate) fn campaign_terrain(&self, name: &str) -> &ContentRecord {
        let mut records = self.content.iter().filter(|record| {
            record.kind == ContentKind::Terrain && record.rule_name.eq_ignore_ascii_case(name)
        });
        let record = records
            .next()
            .unwrap_or_else(|| panic!("missing campaign terrain {name:?}"));
        assert!(
            records.next().is_none(),
            "duplicate campaign terrain {name:?}"
        );
        record
    }

    pub(crate) fn terrain_parameter(&self, key: &str) -> Result<i32, LoadError> {
        let value = required_parameter(self, "terrain.ruleset", "parameters", key)?
            .integer_value()
            .map_err(LoadError::new)?
            .ok_or_else(|| {
                LoadError::new(format!("terrain parameter {key:?} must be an integer"))
            })?;
        i32::try_from(value)
            .map_err(|_| LoadError::new(format!("terrain parameter {key:?} is outside i32 range")))
    }

    pub(crate) fn road_yields(&self, name: &str) -> Result<RoadYields, LoadError> {
        let record = self
            .rule_records
            .iter()
            .find(|record| {
                record.file == "terrain.ruleset"
                    && record.namespace == "road"
                    && record.rule_name.eq_ignore_ascii_case(name)
            })
            .ok_or_else(|| LoadError::new(format!("missing campaign road {name:?}")))?;
        let read = |key| rule_record_integer(self, record, key);
        Ok(RoadYields {
            increment: [
                read("food_incr")?,
                read("shield_incr")?,
                read("trade_incr")?,
            ],
            constant: [
                read("food_incr_const")?,
                read("shield_incr_const")?,
                read("trade_incr_const")?,
            ],
            bonus: [
                read("food_bonus")?,
                read("shield_bonus")?,
                read("trade_bonus")?,
            ],
        })
    }
}

impl ContentRecord {
    pub(crate) fn terrain_yields(&self) -> Result<TerrainYields, LoadError> {
        let read = |key| {
            let value = self
                .integer_field(key)
                .map_err(LoadError::new)?
                .unwrap_or_default();
            i32::try_from(value).map_err(|_| {
                terrain_content_error(
                    None,
                    self,
                    key,
                    "terrain output is outside i32 range".to_owned(),
                )
            })
        };
        Ok(TerrainYields {
            base: [read("food")?, read("shield")?, read("trade")?],
            road_percent: [
                read("road_food_incr_pct")?,
                read("road_shield_incr_pct")?,
                read("road_trade_incr_pct")?,
            ],
            irrigation: read("irrigation_food_incr")?,
            mining: read("mining_shield_incr")?,
        })
    }

    pub(crate) fn terrain_has_flag(&self, name: &str) -> bool {
        terrain_flag_is_set(self, None, name).expect("campaign terrain flags must be strings")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_and_road_modifiers_follow_the_selected_engine_records() {
        let mut rules = RulesetData::campaign_default().clone();
        let grass = rules
            .content
            .iter_mut()
            .find(|record| record.kind == ContentKind::Terrain && record.rule_name == "Grassland")
            .unwrap();
        grass
            .fields
            .iter_mut()
            .find(|field| field.key == "food")
            .unwrap()
            .value = Value::Integer(7);
        grass
            .fields
            .iter_mut()
            .find(|field| field.key == "warmer_wetter_result")
            .unwrap()
            .value = Value::String("Plains".to_owned());
        let road = rules
            .rule_records
            .iter_mut()
            .find(|record| {
                record.namespace == "road" && record.rule_name.eq_ignore_ascii_case("road")
            })
            .unwrap();
        road.fields
            .iter_mut()
            .find(|field| field.key == "trade_incr")
            .unwrap()
            .value = Value::Integer(3);

        let grass = rules.campaign_terrain("Grassland");
        assert_eq!(grass.terrain_yields().unwrap().base[0], 7);
        assert_eq!(
            grass.string_field("warmer_wetter_result").unwrap(),
            Some("Plains")
        );
        assert_eq!(rules.road_yields("Road").unwrap().increment[2], 3);
    }
}
