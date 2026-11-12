//! Ruleset-backed movement data adapted to Civ III runtime identities.
//!
//! CivCraft keeps its Civ III terrain and unit enums as stable simulation IDs.
//! This module is the explicit crosswalk from those IDs to the selected C2C3
//! ruleset records. It only reads data; movement callers can migrate to these
//! validated accessors independently.

use crate::rules::freeciv::{ContentKind, ContentRecord, RuleRecord, RulesetData};

use super::unit_catalog::UnitClass;
use super::{Base, Terrain, UnitKind};

#[derive(Clone, Copy)]
pub(super) struct MovementRules<'a> {
    rules: &'a RulesetData,
}

impl<'a> MovementRules<'a> {
    pub(super) fn new(rules: &'a RulesetData) -> Self {
        Self { rules }
    }

    pub(super) fn campaign() -> Self {
        Self::new(RulesetData::campaign_default())
    }

    /// Resolve all movement and worker activity times for the C2C3 terrain
    /// corresponding to a Civ III terrain identity.
    pub(super) fn terrain_data(
        &self,
        terrain: Terrain,
    ) -> Result<TerrainMovementData, MovementRulesError> {
        let rule_name = c2c3_terrain_rule_name(terrain)
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let record = unique_content_record(self.rules, ContentKind::Terrain, rule_name, "terrain")?;
        let movement_cost = required_nonnegative_integer(record, "movement_cost")?;
        let activity_times = TerrainActivityTimes {
            base: required_nonnegative_integer(record, "base_time")?,
            road: required_nonnegative_integer(record, "road_time")?,
            irrigation: required_nonnegative_integer(record, "irrigation_time")?,
            mining: required_nonnegative_integer(record, "mining_time")?,
            cultivate: required_nonnegative_integer(record, "cultivate_time")?,
            plant: required_nonnegative_integer(record, "plant_time")?,
            transform: required_nonnegative_integer(record, "transform_time")?,
        };
        Ok(TerrainMovementData {
            rule_name: record.rule_name.clone(),
            movement_cost,
            activity_times,
            cultivate_result: optional_terrain_result_name(record, "cultivate_result")?,
            plant_result: optional_terrain_result_name(record, "plant_result")?,
            transform_result: optional_terrain_result_name(record, "transform_result")?,
        })
    }

    /// Resolve a result name read from a selected C2C3 terrain record back to
    /// CivCraft's stable terrain enum. C2C3-only names are rejected explicitly.
    pub(super) fn terrain_result(&self, rule_name: &str) -> Result<Terrain, MovementRulesError> {
        c2c3_terrain_result_to_civ3(rule_name)
    }

    pub(super) fn terrain_movement_cost(
        &self,
        terrain: Terrain,
    ) -> Result<i32, MovementRulesError> {
        // Volcano is a Civ III-only terrain identity. The C2C3 movement
        // fallback is deliberately scoped to movement cost; all other
        // terrain lookups (identity, flags, nativity, and work times) remain
        // unsupported for Volcano.
        let movement_terrain = match terrain {
            Terrain::Volcano => Terrain::Mountains,
            terrain => terrain,
        };
        Ok(self.terrain_data(movement_terrain)?.movement_cost)
    }

    /// Selected ruleset movement-point fragments; road move_cost values are
    /// already expressed in these fragments.
    pub(super) fn move_fragments(&self) -> Result<i32, MovementRulesError> {
        self.integer_parameter("terrain.ruleset", "parameters", "move_fragments", true)
    }

    /// Cost applied to unit types that ignore terrain movement costs.
    pub(super) fn igter_cost(&self) -> Result<i32, MovementRulesError> {
        self.integer_parameter("terrain.ruleset", "parameters", "igter_cost", false)
    }

    fn integer_parameter(
        &self,
        file: &str,
        section: &str,
        key: &str,
        strictly_positive: bool,
    ) -> Result<i32, MovementRulesError> {
        let mut matches = self.rules.parameters.iter().filter(|parameter| {
            parameter.file.eq_ignore_ascii_case(file)
                && parameter.section.eq_ignore_ascii_case(section)
                && parameter.key.eq_ignore_ascii_case(key)
        });
        let Some(parameter) = matches.next() else {
            return Err(MovementRulesError::MissingRecord {
                namespace: format!("{file} [{section}]"),
                rule_name: key.to_owned(),
            });
        };
        if matches.next().is_some() {
            return Err(MovementRulesError::DuplicateRecord {
                namespace: format!("{file} [{section}]"),
                rule_name: key.to_owned(),
            });
        }
        let value = parameter
            .integer_value()
            .map_err(|reason| invalid_field(&parameter.section, key, reason))?
            .ok_or_else(|| {
                invalid_field(&parameter.section, key, "required parameter is missing")
            })?;
        let value = checked_i32(&parameter.section, key, value)?;
        if value < 0 || strictly_positive && value == 0 {
            return Err(invalid_field(
                &parameter.section,
                key,
                format!(
                    "expected a {} integer, found {value}",
                    if strictly_positive {
                        "positive"
                    } else {
                        "nonnegative"
                    }
                ),
            ));
        }
        Ok(value)
    }

    pub(super) fn terrain_activity_times(
        &self,
        terrain: Terrain,
    ) -> Result<TerrainActivityTimes, MovementRulesError> {
        Ok(self.terrain_data(terrain)?.activity_times)
    }

    /// Freeciv `terrain_extra_removal_time(..., ACTIVITY_CLEAN, extra)`.
    /// A positive extra-level removal time overrides terrain settings;
    /// otherwise the selected terrain table value is scaled by the extra's
    /// removal-time factor.
    pub(super) fn terrain_extra_removal_time(
        &self,
        terrain: Terrain,
        extra_rule_name: &str,
    ) -> Result<i32, MovementRulesError> {
        let (_extra, direct, factor) = self.extra_removal_config(extra_rule_name)?;
        if direct != 0 {
            return Ok(direct);
        }
        let terrain_rule_name = c2c3_terrain_rule_name(terrain)
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let record = unique_content_record(
            self.rules,
            ContentKind::Terrain,
            terrain_rule_name,
            "terrain",
        )?;
        let time = self.terrain_extra_setting(record, extra_rule_name)?;
        time.checked_mul(factor).ok_or_else(|| {
            invalid_field(
                &record.section,
                "extra_settings",
                "clean removal time exceeds i32 range after extra factor scaling",
            )
        })
    }

    /// Freeciv `terrain_extra_removal_time(..., ACTIVITY_PILLAGE, extra)`.
    pub(super) fn terrain_pillage_time(
        &self,
        terrain: Terrain,
        extra_rule_name: &str,
    ) -> Result<i32, MovementRulesError> {
        let (_extra, direct, factor) = self.extra_removal_config(extra_rule_name)?;
        if direct != 0 {
            return Ok(direct);
        }
        let terrain_rule_name = c2c3_terrain_rule_name(terrain)
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let record = unique_content_record(
            self.rules,
            ContentKind::Terrain,
            terrain_rule_name,
            "terrain",
        )?;
        optional_nonnegative_content_integer(record, "pillage_time", 1)?
            .checked_mul(factor)
            .ok_or_else(|| {
                invalid_field(
                    &record.section,
                    "pillage_time",
                    "pillage time exceeds i32 range after extra factor scaling",
                )
            })
    }

    fn extra_removal_config(
        &self,
        extra_rule_name: &str,
    ) -> Result<(&RuleRecord, i32, i32), MovementRulesError> {
        let extra = unique_rule_record(self.rules, "extra", extra_rule_name)?;
        let direct = optional_nonnegative_rule_integer(extra, "removal_time", 0)?;
        let factor = optional_nonnegative_rule_integer(extra, "removal_time_factor", 1)?;
        Ok((extra, direct, factor))
    }

    fn terrain_extra_setting(
        &self,
        terrain: &ContentRecord,
        extra_rule_name: &str,
    ) -> Result<i32, MovementRulesError> {
        let count = terrain
            .fields
            .iter()
            .filter(|entry| entry.key.eq_ignore_ascii_case("extra_settings"))
            .count();
        if count > 1 {
            return Err(invalid_field(
                &terrain.section,
                "extra_settings",
                "field occurs more than once",
            ));
        }
        let rows = terrain
            .string_integer_table_field("extra_settings", "extra", "removal_time")
            .map_err(|reason| invalid_field(&terrain.section, "extra_settings", reason))?
            .unwrap_or_default();
        let mut matches = rows
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case(extra_rule_name));
        let Some((_, value)) = matches.next() else {
            return Ok(0);
        };
        if matches.next().is_some() {
            return Err(invalid_field(
                &terrain.section,
                "extra_settings",
                format!("extra {extra_rule_name:?} occurs more than once"),
            ));
        }
        let value = checked_i32(&terrain.section, "extra_settings.removal_time", *value)?;
        if value < 0 {
            return Err(invalid_field(
                &terrain.section,
                "extra_settings.removal_time",
                format!("expected a nonnegative integer, found {value}"),
            ));
        }
        Ok(value)
    }

    /// Return the selected terrain record's Freeciv `native_to` unit-class
    /// names, preserving entries such as `Merchant` that have no Civ III
    /// runtime class equivalent.
    pub(super) fn terrain_native_classes(
        &self,
        terrain: Terrain,
    ) -> Result<Vec<String>, MovementRulesError> {
        let rule_name = c2c3_terrain_rule_name(terrain)
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let record = unique_content_record(self.rules, ContentKind::Terrain, rule_name, "terrain")?;
        required_content_string_list(record, "native_to")
    }

    pub(super) fn terrain_has_flag(
        &self,
        terrain: Terrain,
        flag: TerrainFlag,
    ) -> Result<bool, MovementRulesError> {
        let rule_name = c2c3_terrain_rule_name(terrain)
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let record = unique_content_record(self.rules, ContentKind::Terrain, rule_name, "terrain")?;
        let flags = required_content_string_values(record, "flags")?;
        Ok(has_flag(&flags, flag.rule_name()))
    }

    pub(super) fn terrain_is_native_to_class(
        &self,
        terrain: Terrain,
        class: UnitClass,
    ) -> Result<bool, MovementRulesError> {
        let classes = self.terrain_native_classes(terrain)?;
        Ok(has_flag(&classes, c2c3_unit_class_rule_name(class)))
    }

    /// Check tile nativity from selected terrain and extra records while
    /// retaining Civ III terrain and base IDs at the call boundary.
    pub(super) fn tile_native_to_class(
        &self,
        terrain: Terrain,
        road: bool,
        railroad: bool,
        river: bool,
        base: Option<Base>,
        class: UnitClass,
    ) -> Result<bool, MovementRulesError> {
        if self.terrain_is_native_to_class(terrain, class)? {
            return Ok(true);
        }
        if road && self.extra_is_native_to_class("Road", class)? {
            return Ok(true);
        }
        if railroad && self.extra_is_native_to_class("Railroad", class)? {
            return Ok(true);
        }
        if river && self.extra_is_native_to_class("River", class)? {
            return Ok(true);
        }
        if let Some(base) = base {
            if self.extra_is_native_to_class(c2c3_base_extra_rule_name(base), class)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Movement-only nativity uses the closest C2C3 rugged terrain for
    /// Civ III Volcano. Generic terrain identity, worker/action, and flag
    /// lookups intentionally continue to reject Volcano.
    pub(super) fn movement_tile_native_to_class(
        &self,
        terrain: Terrain,
        road: bool,
        railroad: bool,
        river: bool,
        base: Option<Base>,
        class: UnitClass,
    ) -> Result<bool, MovementRulesError> {
        let movement_terrain = match terrain {
            Terrain::Volcano => Terrain::Mountains,
            terrain => terrain,
        };
        self.tile_native_to_class(movement_terrain, road, railroad, river, base, class)
    }

    /// Pure PF-style step cost over observed/source tile data. Road costs are
    /// already in `move_fragments`; terrain costs are whole movement points.
    /// Legality, city access, and transport scopes stay with the caller.
    pub(super) fn step_cost(
        &self,
        source: MovementTile,
        destination: MovementTile,
        class: UnitClass,
        cardinal: bool,
        restricted_infra: bool,
        ignores_terrain: bool,
    ) -> Result<i32, MovementRulesError> {
        let move_fragments = self.move_fragments()?;
        let igter_cost = self.igter_cost()?;
        if !self.unit_class_movement_data(class)?.terrain_speed {
            return Ok(move_fragments);
        }
        if !self.movement_tile_native_to_class(
            source.terrain,
            source.road,
            source.railroad,
            source.river,
            source.base,
            class,
        )? || !self.movement_tile_native_to_class(
            destination.terrain,
            destination.road,
            destination.railroad,
            destination.river,
            destination.base,
            class,
        )? {
            return Ok(if ignores_terrain {
                igter_cost
            } else {
                move_fragments
            });
        }

        let mut cost = self.terrain_movement_cost(destination.terrain)? * move_fragments;
        if !restricted_infra {
            for (road, connected) in [
                (RoadType::Railroad, source.railroad && destination.railroad),
                (RoadType::Road, source.road && destination.road),
                (RoadType::River, source.river && destination.river),
            ] {
                if !connected
                    || !self.extra_is_native_to_class(road.extra_name(), class)?
                    || !road_move_mode_applies(self.road_move_mode(road)?, cardinal)
                {
                    continue;
                }
                let road_cost = self.road_move_cost(road)?;
                // Freeciv uses -1 to declare an extra with no move bonus.
                if road_cost >= 0 {
                    cost = cost.min(road_cost);
                }
            }
        }
        Ok(if ignores_terrain {
            cost.min(igter_cost)
        } else {
            cost
        })
    }

    /// Freeciv `utype_unknown_move_cost`: choose the worst movement cost over
    /// selected terrains native to this unit class, then apply the terrain
    /// access penalty and selected movement fragments. Action-success move
    /// cost effects are not included: evaluating them requires the unit-type
    /// action-enabler cache, which this ruleset-only adapter does not model.
    pub(super) fn unknown_tile_move_cost(&self, kind: UnitKind) -> Result<i32, MovementRulesError> {
        let class = self.unit_class_for_kind(kind)?;
        let class_data = self.unit_class_movement_data(class)?;
        let fragments = self.move_fragments()?;
        let ignores_terrain = super::action_rules::freeciv_unit_type_flag(kind, "IgTer")
            .ok_or_else(|| {
                invalid_field(
                    &format!("unit {kind:?}"),
                    "flags",
                    "no selected C2C3 unit-type IgTer binding",
                )
            })?;

        let mut movement_cost = if !class_data.terrain_speed {
            fragments
        } else if ignores_terrain {
            self.igter_cost()?
        } else {
            let mut worst_terrain_cost = 1;
            for terrain in self
                .rules
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Terrain)
            {
                let flags = required_content_string_values(terrain, "flags")?;
                if has_flag(&flags, TerrainFlag::NotGenerated.rule_name()) {
                    // C2C3's inaccessible sentinel intentionally has no
                    // native_to list and cannot be entered by any unit.
                    continue;
                }
                let native_classes = required_content_string_list(terrain, "native_to")?;
                if has_flag(&native_classes, c2c3_unit_class_rule_name(class)) {
                    let cost = required_nonnegative_integer(terrain, "movement_cost")?;
                    worst_terrain_cost = worst_terrain_cost.max(cost);
                }
            }
            worst_terrain_cost.checked_mul(fragments).ok_or_else(|| {
                invalid_field(
                    "unknown tile movement cost",
                    "movement_cost",
                    "native terrain cost exceeds i32 range after movement-fragment scaling",
                )
            })?
        };

        // Freeciv doubles this estimate if any terrain with a nonempty
        // native_to list excludes the unit class. It intentionally does not
        // model terrain records that no unit can enter.
        let mut may_be_non_native = false;
        for terrain in self
            .rules
            .content
            .iter()
            .filter(|record| record.kind == ContentKind::Terrain)
        {
            let flags = required_content_string_values(terrain, "flags")?;
            if has_flag(&flags, TerrainFlag::NotGenerated.rule_name()) {
                continue;
            }
            let native_classes = required_content_string_list(terrain, "native_to")?;
            if !native_classes.is_empty()
                && !has_flag(&native_classes, c2c3_unit_class_rule_name(class))
            {
                may_be_non_native = true;
                break;
            }
        }
        if may_be_non_native {
            movement_cost = movement_cost.checked_mul(2).ok_or_else(|| {
                invalid_field(
                    "unknown tile movement cost",
                    "movement_cost",
                    "terrain access penalty exceeds i32 range",
                )
            })?;
        }
        Ok(movement_cost)
    }

    /// Read C2C3 unit-class flags. Civ III unit identity and class membership
    /// remain source-catalog inputs; the name conversion is deliberately
    /// explicit so unsupported source classes cannot silently use a default.
    pub(super) fn unit_class_movement_data(
        &self,
        class: UnitClass,
    ) -> Result<UnitClassMovementData, MovementRulesError> {
        let rule_name = c2c3_unit_class_rule_name(class);
        let record = unique_rule_record(self.rules, "unit_class", rule_name)?;
        let flags = required_rule_string_list(record, "flags")?;
        Ok(UnitClassMovementData {
            rule_name: record.rule_name.clone(),
            min_speed: required_nonnegative_rule_integer(record, "min_speed")?,
            build_anywhere: has_flag(&flags, "BuildAnywhere"),
            can_occupy_city: has_flag(&flags, "CanOccupyCity"),
            missile: has_flag(&flags, "Missile"),
            terrain_speed: has_flag(&flags, "TerrainSpeed"),
            zoc: has_flag(&flags, "ZOC"),
            damage_slows: has_flag(&flags, "DamageSlows"),
        })
    }

    /// Resolve the Freeciv class for a retained Civ III unit identity. Mapped
    /// C2C3 unit records supply the class; the explicit no-record ledger uses
    /// the Civ III class crosswalk.
    pub(super) fn unit_class_for_kind(
        &self,
        kind: UnitKind,
    ) -> Result<UnitClass, MovementRulesError> {
        let rule_name =
            super::action_rules::freeciv_unit_class_rule_name(kind).ok_or_else(|| {
                invalid_field(
                    &format!("unit {kind:?}"),
                    "class",
                    "no selected C2C3 class binding",
                )
            })?;
        let class = [
            (UnitClass::Land, "Land"),
            (UnitClass::SmallLand, "Small Land"),
            (UnitClass::BigLand, "Big Land"),
            (UnitClass::Sea, "Sea"),
            (UnitClass::Trireme, "Trireme"),
            (UnitClass::Air, "Air"),
            (UnitClass::Helicopter, "Helicopter"),
            (UnitClass::Missile, "Missile"),
        ]
        .into_iter()
        .find_map(|(class, name)| rule_name.eq_ignore_ascii_case(name).then_some(class));
        class.ok_or_else(|| {
            invalid_field(
                &format!("unit {kind:?}"),
                "class",
                format!("unsupported C2C3 class {rule_name:?}"),
            )
        })
    }

    pub(super) fn unit_class_movement_data_for_kind(
        &self,
        kind: UnitKind,
    ) -> Result<UnitClassMovementData, MovementRulesError> {
        self.unit_class_movement_data(self.unit_class_for_kind(kind)?)
    }

    /// Combine the selected ruleset's class behavior with an individual
    /// unit-level ZOC exemption from Civ III's source catalog. Freeciv's
    /// `IgZOC` is a unit-type flag, not a unit-class flag.
    pub(super) fn unit_movement_data(
        &self,
        kind: UnitKind,
    ) -> Result<UnitMovementData, MovementRulesError> {
        let ignores_zoc =
            super::action_rules::freeciv_unit_type_flag(kind, "IgZOC").ok_or_else(|| {
                invalid_field(
                    &format!("unit {:?}", kind),
                    "flags",
                    "no selected C2C3 unit-type IgZOC binding",
                )
            })?;
        Ok(UnitMovementData {
            class: self.unit_class_movement_data_for_kind(kind)?,
            ignores_zoc,
        })
    }

    /// Return an extra's Freeciv `native_to` class names. The argument is the
    /// selected ruleset's stable extra rule name (for example `Road`,
    /// `Railroad`, `River`, `Fort`, or `Airbase`).
    pub(super) fn extra_native_classes(
        &self,
        extra_rule_name: &str,
    ) -> Result<Vec<String>, MovementRulesError> {
        let record = unique_rule_record(self.rules, "extra", extra_rule_name)?;
        required_rule_string_list(record, "native_to")
    }

    pub(super) fn extra_is_native_to_class(
        &self,
        extra_rule_name: &str,
        class: UnitClass,
    ) -> Result<bool, MovementRulesError> {
        let classes = self.extra_native_classes(extra_rule_name)?;
        Ok(has_flag(&classes, c2c3_unit_class_rule_name(class)))
    }

    pub(super) fn road_move_cost(&self, road: RoadType) -> Result<i32, MovementRulesError> {
        let rule_name = road.rule_name();
        let record = unique_rule_record(self.rules, "road", rule_name)?;
        required_road_move_cost(record, "move_cost")
    }

    pub(super) fn road_move_mode(
        &self,
        road: RoadType,
    ) -> Result<RoadMoveMode, MovementRulesError> {
        let rule_name = road.rule_name();
        let record = unique_rule_record(self.rules, "road", rule_name)?;
        let mode =
            optional_rule_string(record, "move_mode")?.unwrap_or_else(|| "FastAlways".to_owned());
        RoadMoveMode::parse(&record.section, &mode)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TerrainMovementData {
    pub(super) rule_name: String,
    /// C2C3 whole movement points, before conversion to simulation fragments.
    pub(super) movement_cost: i32,
    pub(super) activity_times: TerrainActivityTimes,
    /// C2C3 output identity from the selected terrain record; "no" is None.
    pub(super) cultivate_result: Option<String>,
    pub(super) plant_result: Option<String>,
    pub(super) transform_result: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MovementTile {
    pub(super) terrain: Terrain,
    pub(super) road: bool,
    pub(super) railroad: bool,
    pub(super) river: bool,
    pub(super) base: Option<Base>,
}

impl MovementTile {
    pub(super) const fn new(
        terrain: Terrain,
        road: bool,
        railroad: bool,
        river: bool,
        base: Option<Base>,
    ) -> Self {
        Self {
            terrain,
            road,
            railroad,
            river,
            base,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TerrainActivityTimes {
    pub(super) base: i32,
    pub(super) road: i32,
    pub(super) irrigation: i32,
    pub(super) mining: i32,
    pub(super) cultivate: i32,
    pub(super) plant: i32,
    pub(super) transform: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TerrainFlag {
    CanHaveRiver,
    FreshWater,
    Frozen,
    NotGenerated,
    NoBarbs,
    NoCities,
    NoFortify,
    NoPollution,
    NoZoc,
    Sea,
    Starter,
    UnsafeCoast,
}

impl TerrainFlag {
    const ALL: [Self; 12] = [
        Self::CanHaveRiver,
        Self::FreshWater,
        Self::Frozen,
        Self::NotGenerated,
        Self::NoBarbs,
        Self::NoCities,
        Self::NoFortify,
        Self::NoPollution,
        Self::NoZoc,
        Self::Sea,
        Self::Starter,
        Self::UnsafeCoast,
    ];

    fn rule_name(self) -> &'static str {
        match self {
            Self::CanHaveRiver => "CanHaveRiver",
            Self::FreshWater => "FreshWater",
            Self::Frozen => "Frozen",
            Self::NotGenerated => "NotGenerated",
            Self::NoBarbs => "NoBarbs",
            Self::NoCities => "NoCities",
            Self::NoFortify => "NoFortify",
            Self::NoPollution => "NoPollution",
            Self::NoZoc => "NoZoc",
            Self::Sea => "Sea",
            Self::Starter => "Starter",
            Self::UnsafeCoast => "UnsafeCoast",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UnitClassMovementData {
    pub(super) rule_name: String,
    /// Minimum speed in whole movement points after damage and effects.
    pub(super) min_speed: i32,
    pub(super) build_anywhere: bool,
    pub(super) can_occupy_city: bool,
    pub(super) missile: bool,
    pub(super) terrain_speed: bool,
    pub(super) zoc: bool,
    pub(super) damage_slows: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UnitMovementData {
    pub(super) class: UnitClassMovementData,
    /// Selected C2C3 unit-type `IgZOC` flag.
    pub(super) ignores_zoc: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RoadType {
    Road,
    Railroad,
    Maglev,
    River,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RoadMoveMode {
    Cardinal,
    Relaxed,
    FastAlways,
}

impl RoadMoveMode {
    fn parse(section: &str, value: &str) -> Result<Self, MovementRulesError> {
        match value.to_ascii_lowercase().as_str() {
            "cardinal" => Ok(Self::Cardinal),
            "relaxed" => Ok(Self::Relaxed),
            "fastalways" => Ok(Self::FastAlways),
            _ => Err(invalid_field(
                section,
                "move_mode",
                format!("unknown road move mode {value:?}"),
            )),
        }
    }
}

impl RoadType {
    fn extra_name(self) -> &'static str {
        match self {
            Self::Road => "Road",
            Self::Railroad => "Railroad",
            Self::Maglev => "Maglev",
            Self::River => "River",
        }
    }

    fn rule_name(self) -> &'static str {
        match self {
            Self::Road => "Road",
            Self::Railroad => "Railroad",
            Self::Maglev => "Maglev",
            Self::River => "River",
        }
    }
}

fn road_move_mode_applies(mode: RoadMoveMode, cardinal: bool) -> bool {
    match mode {
        RoadMoveMode::FastAlways => true,
        // The C2C3 campaign uses Cardinal for River. Relaxed road connections
        // need an intermediate tile, which endpoint-only movement inputs omit.
        RoadMoveMode::Cardinal | RoadMoveMode::Relaxed => cardinal,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MovementRulesError {
    UnsupportedTerrain(Terrain),
    MissingRecord {
        namespace: String,
        rule_name: String,
    },
    DuplicateRecord {
        namespace: String,
        rule_name: String,
    },
    InvalidField {
        record: String,
        key: String,
        reason: String,
    },
}

/// Civ III's Sea and Ocean correspond to Freeciv's Ocean and Deep Ocean. Its
/// coast and inland-lake IDs both use C2C3's Lake movement record. Flood Plains
/// and Grassland Shield retain their Civ III terrain identity while using the
/// Desert and Grassland C2C3 base records, respectively. Volcano has no C2C3
/// terrain equivalent and remains explicitly unsupported except for the
/// movement-only rugged-terrain adaptations in cost and native-tile lookups.
pub(super) fn c2c3_terrain_rule_name(terrain: Terrain) -> Option<&'static str> {
    Some(match terrain {
        Terrain::Sea => "Ocean",
        Terrain::Ocean => "Deep Ocean",
        Terrain::Lake | Terrain::Coast => "Lake",
        Terrain::Glacier => "Glacier",
        Terrain::Jungle => "Jungle",
        Terrain::Desert | Terrain::FloodPlains => "Desert",
        Terrain::Forest => "Forest",
        Terrain::Grassland | Terrain::GrasslandShield => "Grassland",
        Terrain::Hills => "Hills",
        Terrain::Mountains => "Mountains",
        Terrain::Plains => "Plains",
        Terrain::Marsh => "Swamp",
        Terrain::Tundra => "Tundra",
        Terrain::Volcano => return None,
    })
}

pub(super) fn c2c3_unit_class_rule_name(class: UnitClass) -> &'static str {
    match class {
        UnitClass::Land => "Land",
        UnitClass::SmallLand => "Small Land",
        UnitClass::BigLand => "Big Land",
        UnitClass::Sea => "Sea",
        UnitClass::Trireme => "Trireme",
        UnitClass::Air => "Air",
        UnitClass::Helicopter => "Helicopter",
        UnitClass::Missile => "Missile",
    }
}

pub(super) fn c2c3_base_extra_rule_name(base: Base) -> &'static str {
    match base {
        Base::Fort => "Fort",
        Base::Fortress => "Fortress",
        Base::Airstrip => "Airstrip",
        Base::Airbase => "Airbase",
    }
}

fn optional_terrain_result_name(
    record: &ContentRecord,
    key: &str,
) -> Result<Option<String>, MovementRulesError> {
    let count = record
        .fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key))
        .count();
    if count > 1 {
        return Err(invalid_field(
            &record.section,
            key,
            "field occurs more than once",
        ));
    }
    let Some(value) = record
        .string_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
    else {
        return Ok(None);
    };
    if value.eq_ignore_ascii_case("no") || value.eq_ignore_ascii_case("none") {
        Ok(None)
    } else {
        Ok(Some(value.to_owned()))
    }
}

fn c2c3_terrain_result_to_civ3(name: &str) -> Result<Terrain, MovementRulesError> {
    let terrain = match name.trim().to_ascii_lowercase().as_str() {
        "ocean" => Terrain::Sea,
        "deep ocean" => Terrain::Ocean,
        "lake" => Terrain::Lake,
        "glacier" => Terrain::Glacier,
        "jungle" => Terrain::Jungle,
        "desert" => Terrain::Desert,
        "forest" => Terrain::Forest,
        "grassland" => Terrain::Grassland,
        "hills" => Terrain::Hills,
        "mountains" => Terrain::Mountains,
        "plains" => Terrain::Plains,
        "swamp" => Terrain::Marsh,
        "tundra" => Terrain::Tundra,
        _ => {
            return Err(invalid_field(
                "C2C3 terrain result crosswalk",
                "result",
                format!("unsupported C2C3 terrain result {name:?}"),
            ));
        }
    };
    Ok(terrain)
}

fn unique_content_record<'a>(
    rules: &'a RulesetData,
    kind: ContentKind,
    rule_name: &str,
    namespace: &str,
) -> Result<&'a ContentRecord, MovementRulesError> {
    let mut records = rules
        .content
        .iter()
        .filter(|record| record.kind == kind && record.rule_name.eq_ignore_ascii_case(rule_name));
    let Some(record) = records.next() else {
        return Err(MovementRulesError::MissingRecord {
            namespace: namespace.to_owned(),
            rule_name: rule_name.to_owned(),
        });
    };
    if records.next().is_some() {
        return Err(MovementRulesError::DuplicateRecord {
            namespace: namespace.to_owned(),
            rule_name: rule_name.to_owned(),
        });
    }
    Ok(record)
}

fn unique_rule_record<'a>(
    rules: &'a RulesetData,
    namespace: &str,
    rule_name: &str,
) -> Result<&'a RuleRecord, MovementRulesError> {
    let mut records = rules.rule_records.iter().filter(|record| {
        record.namespace.eq_ignore_ascii_case(namespace)
            && record.rule_name.eq_ignore_ascii_case(rule_name)
    });
    let Some(record) = records.next() else {
        return Err(MovementRulesError::MissingRecord {
            namespace: namespace.to_owned(),
            rule_name: rule_name.to_owned(),
        });
    };
    if records.next().is_some() {
        return Err(MovementRulesError::DuplicateRecord {
            namespace: namespace.to_owned(),
            rule_name: rule_name.to_owned(),
        });
    }
    Ok(record)
}

fn required_nonnegative_integer(
    record: &ContentRecord,
    key: &str,
) -> Result<i32, MovementRulesError> {
    let value = required_content_integer(record, key)?;
    if value < 0 {
        return Err(invalid_field(
            &record.section,
            key,
            format!("expected a nonnegative integer, found {value}"),
        ));
    }
    Ok(value)
}

fn optional_nonnegative_content_integer(
    record: &ContentRecord,
    key: &str,
    default: i32,
) -> Result<i32, MovementRulesError> {
    let count = record
        .fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key))
        .count();
    if count > 1 {
        return Err(invalid_field(
            &record.section,
            key,
            "field occurs more than once",
        ));
    }
    let Some(value) = record
        .integer_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
    else {
        return Ok(default);
    };
    let value = checked_i32(&record.section, key, value)?;
    if value < 0 {
        return Err(invalid_field(
            &record.section,
            key,
            format!("expected a nonnegative integer, found {value}"),
        ));
    }
    Ok(value)
}

fn optional_nonnegative_rule_integer(
    record: &RuleRecord,
    key: &str,
    default: i32,
) -> Result<i32, MovementRulesError> {
    let count = record
        .fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key))
        .count();
    if count > 1 {
        return Err(invalid_field(
            &record.section,
            key,
            "field occurs more than once",
        ));
    }
    let Some(value) = record
        .integer_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
    else {
        return Ok(default);
    };
    let value = checked_i32(&record.section, key, value)?;
    if value < 0 {
        return Err(invalid_field(
            &record.section,
            key,
            format!("expected a nonnegative integer, found {value}"),
        ));
    }
    Ok(value)
}

fn required_road_move_cost(record: &RuleRecord, key: &str) -> Result<i32, MovementRulesError> {
    let value = required_rule_integer(record, key)?;
    if value < -1 {
        return Err(invalid_field(
            &record.section,
            key,
            format!("expected -1 or a nonnegative integer, found {value}"),
        ));
    }
    Ok(value)
}

fn required_content_integer(record: &ContentRecord, key: &str) -> Result<i32, MovementRulesError> {
    let count = record
        .fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key))
        .count();
    if count != 1 {
        return Err(invalid_field(
            &record.section,
            key,
            if count == 0 {
                "required field is missing"
            } else {
                "field occurs more than once"
            },
        ));
    }
    let value = record
        .integer_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
        .ok_or_else(|| invalid_field(&record.section, key, "required field is missing"))?;
    checked_i32(&record.section, key, value)
}

fn required_rule_integer(record: &RuleRecord, key: &str) -> Result<i32, MovementRulesError> {
    let count = record
        .fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key))
        .count();
    if count != 1 {
        return Err(invalid_field(
            &record.section,
            key,
            if count == 0 {
                "required field is missing"
            } else {
                "field occurs more than once"
            },
        ));
    }
    let value = record
        .integer_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
        .ok_or_else(|| invalid_field(&record.section, key, "required field is missing"))?;
    checked_i32(&record.section, key, value)
}

fn required_nonnegative_rule_integer(
    record: &RuleRecord,
    key: &str,
) -> Result<i32, MovementRulesError> {
    let value = required_rule_integer(record, key)?;
    if value < 0 {
        return Err(invalid_field(
            &record.section,
            key,
            format!("expected a nonnegative integer, found {value}"),
        ));
    }
    Ok(value)
}

fn checked_i32(section: &str, key: &str, value: i64) -> Result<i32, MovementRulesError> {
    i32::try_from(value).map_err(|_| {
        invalid_field(
            section,
            key,
            format!("integer is outside i32 range: {value}"),
        )
    })
}

fn required_content_string_list(
    record: &ContentRecord,
    key: &str,
) -> Result<Vec<String>, MovementRulesError> {
    record
        .string_list_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
        .ok_or_else(|| invalid_field(&record.section, key, "required field is missing"))
}

fn required_content_string_values(
    record: &ContentRecord,
    key: &str,
) -> Result<Vec<String>, MovementRulesError> {
    match record.string_list_field(key) {
        Ok(Some(values)) => Ok(values),
        Ok(None) => Err(invalid_field(
            &record.section,
            key,
            "required field is missing",
        )),
        Err(list_error) => match record.string_field(key) {
            Ok(Some(value)) => Ok(vec![value.to_owned()]),
            Ok(None) => Err(invalid_field(
                &record.section,
                key,
                "required field is missing",
            )),
            Err(_) => Err(invalid_field(&record.section, key, list_error)),
        },
    }
}

fn required_rule_string_list(
    record: &RuleRecord,
    key: &str,
) -> Result<Vec<String>, MovementRulesError> {
    record
        .string_list_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))?
        .ok_or_else(|| invalid_field(&record.section, key, "required field is missing"))
}

fn optional_rule_string(
    record: &RuleRecord,
    key: &str,
) -> Result<Option<String>, MovementRulesError> {
    if record
        .fields
        .iter()
        .filter(|entry| entry.key.eq_ignore_ascii_case(key))
        .count()
        > 1
    {
        return Err(invalid_field(
            &record.section,
            key,
            "field occurs more than once",
        ));
    }
    record
        .string_field(key)
        .map_err(|reason| invalid_field(&record.section, key, reason))
        .map(|value| value.map(str::to_owned))
}

fn has_flag(flags: &[String], expected: &str) -> bool {
    flags.iter().any(|flag| flag.eq_ignore_ascii_case(expected))
}

fn invalid_field(record: &str, key: &str, reason: impl Into<String>) -> MovementRulesError {
    MovementRulesError::InvalidField {
        record: record.to_owned(),
        key: key.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERRAIN_ADAPTER: [(Terrain, Option<&str>); 17] = [
        (Terrain::Sea, Some("Ocean")),
        (Terrain::Ocean, Some("Deep Ocean")),
        (Terrain::Lake, Some("Lake")),
        (Terrain::Glacier, Some("Glacier")),
        (Terrain::Jungle, Some("Jungle")),
        (Terrain::Desert, Some("Desert")),
        (Terrain::Forest, Some("Forest")),
        (Terrain::Grassland, Some("Grassland")),
        (Terrain::Hills, Some("Hills")),
        (Terrain::Mountains, Some("Mountains")),
        (Terrain::Plains, Some("Plains")),
        (Terrain::Marsh, Some("Swamp")),
        (Terrain::Tundra, Some("Tundra")),
        (Terrain::Coast, Some("Lake")),
        (Terrain::FloodPlains, Some("Desert")),
        (Terrain::Volcano, None),
        (Terrain::GrasslandShield, Some("Grassland")),
    ];

    const UNIT_CLASS_ADAPTER: [(UnitClass, &str); 8] = [
        (UnitClass::Land, "Land"),
        (UnitClass::SmallLand, "Small Land"),
        (UnitClass::BigLand, "Big Land"),
        (UnitClass::Sea, "Sea"),
        (UnitClass::Trireme, "Trireme"),
        (UnitClass::Air, "Air"),
        (UnitClass::Helicopter, "Helicopter"),
        (UnitClass::Missile, "Missile"),
    ];

    const ACTIVITY_TIME_FIELDS: [(&str, fn(TerrainActivityTimes) -> i32); 7] = [
        ("base_time", |times| times.base),
        ("road_time", |times| times.road),
        ("irrigation_time", |times| times.irrigation),
        ("mining_time", |times| times.mining),
        ("cultivate_time", |times| times.cultivate),
        ("plant_time", |times| times.plant),
        ("transform_time", |times| times.transform),
    ];

    fn campaign_rules() -> &'static RulesetData {
        RulesetData::campaign_default()
    }

    fn terrain_record<'a>(rules: &'a RulesetData, rule_name: &str) -> &'a ContentRecord {
        rules
            .content
            .iter()
            .find(|record| {
                record.kind == ContentKind::Terrain
                    && record.rule_name.eq_ignore_ascii_case(rule_name)
            })
            .expect("C2C3 terrain record")
    }

    fn raw_integer(record: &ContentRecord, key: &str) -> i32 {
        i32::try_from(record.integer_field(key).unwrap().unwrap()).unwrap()
    }

    fn raw_rule_integer(record: &RuleRecord, key: &str) -> i32 {
        i32::try_from(record.integer_field(key).unwrap().unwrap()).unwrap()
    }

    fn raw_class_flags(record: &RuleRecord) -> Vec<String> {
        record.string_list_field("flags").unwrap().unwrap()
    }

    fn swap_content_fields(
        rules: &mut RulesetData,
        first_record: usize,
        first_field: usize,
        second_record: usize,
        second_field: usize,
    ) {
        if first_record < second_record {
            let (before, after) = rules.content.split_at_mut(second_record);
            std::mem::swap(
                &mut before[first_record].fields[first_field],
                &mut after[0].fields[second_field],
            );
        } else {
            let (before, after) = rules.content.split_at_mut(first_record);
            std::mem::swap(
                &mut after[0].fields[first_field],
                &mut before[second_record].fields[second_field],
            );
        }
    }

    fn raw_content_native_classes(record: &ContentRecord) -> Vec<String> {
        record
            .string_list_field("native_to")
            .unwrap()
            .unwrap_or_default()
    }

    fn raw_rule_native_classes(record: &RuleRecord) -> Vec<String> {
        record.string_list_field("native_to").unwrap().unwrap()
    }

    #[test]
    fn full_civ3_terrain_adapter_reads_costs_and_activity_times_from_c2c3_records() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        for (terrain, source_name) in TERRAIN_ADAPTER {
            let Some(source_name) = source_name else {
                assert_eq!(
                    adapter.terrain_data(terrain),
                    Err(MovementRulesError::UnsupportedTerrain(terrain))
                );
                continue;
            };
            assert_eq!(c2c3_terrain_rule_name(terrain), Some(source_name));
            let record = terrain_record(rules, source_name);
            let adapted = adapter.terrain_data(terrain).unwrap();
            assert_eq!(adapted.rule_name, record.rule_name, "{terrain:?}");
            assert_eq!(
                adapted.movement_cost,
                raw_integer(record, "movement_cost"),
                "{terrain:?}"
            );
            for (field, read) in ACTIVITY_TIME_FIELDS {
                assert_eq!(
                    read(adapted.activity_times),
                    raw_integer(record, field),
                    "{terrain:?}.{field}"
                );
            }
            for (field, adapted_result) in [
                ("cultivate_result", &adapted.cultivate_result),
                ("plant_result", &adapted.plant_result),
                ("transform_result", &adapted.transform_result),
            ] {
                let expected = record
                    .string_field(field)
                    .unwrap()
                    .filter(|name| {
                        !name.eq_ignore_ascii_case("no") && !name.eq_ignore_ascii_case("none")
                    })
                    .map(str::to_owned);
                assert_eq!(adapted_result, &expected, "{terrain:?}.{field}");
                if let Some(result_name) = adapted_result {
                    assert!(
                        adapter.terrain_result(result_name).is_ok(),
                        "{field}: {result_name}"
                    );
                }
            }
        }
    }

    #[test]
    fn clean_and_pillage_times_follow_selected_terrain_extra_settings() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        for (terrain, source_name) in TERRAIN_ADAPTER {
            let Some(source_name) = source_name else {
                assert!(matches!(
                    adapter.terrain_extra_removal_time(terrain, "Pollution"),
                    Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
                ));
                continue;
            };
            let terrain_record = terrain_record(rules, source_name);
            let rows = terrain_record
                .string_integer_table_field("extra_settings", "extra", "removal_time")
                .unwrap()
                .unwrap_or_default();
            let pillage_time = terrain_record
                .integer_field("pillage_time")
                .unwrap()
                .unwrap_or(1) as i32;
            for (extra_name, terrain_time) in rows {
                let extra = rules
                    .rule_records
                    .iter()
                    .find(|record| {
                        record.namespace.eq_ignore_ascii_case("extra")
                            && record.rule_name.eq_ignore_ascii_case(&extra_name)
                    })
                    .expect("C2C3 extra record");
                let direct_time = extra.integer_field("removal_time").unwrap().unwrap_or(0) as i32;
                let factor = extra
                    .integer_field("removal_time_factor")
                    .unwrap()
                    .unwrap_or(1) as i32;
                let clean_expected = if direct_time != 0 {
                    direct_time
                } else {
                    terrain_time as i32 * factor
                };
                let pillage_expected = if direct_time != 0 {
                    direct_time
                } else {
                    pillage_time * factor
                };
                assert_eq!(
                    adapter.terrain_extra_removal_time(terrain, &extra_name),
                    Ok(clean_expected),
                    "{source_name}/{extra_name}"
                );
                assert_eq!(
                    adapter.terrain_pillage_time(terrain, &extra_name),
                    Ok(pillage_expected),
                    "{source_name}/{extra_name}"
                );
            }
        }
    }

    #[test]
    fn c2c3_terrain_result_names_map_explicitly_to_civcraft_ids() {
        let adapter = MovementRules::campaign();
        for (name, expected) in [
            ("Ocean", Terrain::Sea),
            ("Deep Ocean", Terrain::Ocean),
            ("Lake", Terrain::Lake),
            ("Glacier", Terrain::Glacier),
            ("Jungle", Terrain::Jungle),
            ("Desert", Terrain::Desert),
            ("Forest", Terrain::Forest),
            ("Grassland", Terrain::Grassland),
            ("Hills", Terrain::Hills),
            ("Mountains", Terrain::Mountains),
            ("Plains", Terrain::Plains),
            ("Swamp", Terrain::Marsh),
            ("Tundra", Terrain::Tundra),
        ] {
            assert_eq!(adapter.terrain_result(name), Ok(expected));
        }
        assert!(matches!(
            adapter.terrain_result("Volcano"),
            Err(MovementRulesError::InvalidField { .. })
        ));
    }

    #[test]
    fn full_civ3_class_adapter_reads_movement_flags_from_c2c3_records() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        for (class, source_name) in UNIT_CLASS_ADAPTER {
            assert_eq!(c2c3_unit_class_rule_name(class), source_name);
            let record = rules
                .rule_records
                .iter()
                .find(|record| {
                    record.namespace == "unit_class"
                        && record.rule_name.eq_ignore_ascii_case(source_name)
                })
                .expect("mapped C2C3 unit class");
            let flags = raw_class_flags(record);
            let adapted = adapter.unit_class_movement_data(class).unwrap();
            assert_eq!(adapted.rule_name, record.rule_name);
            assert_eq!(adapted.min_speed, raw_rule_integer(record, "min_speed"));
            assert_eq!(
                adapted.build_anywhere,
                flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("BuildAnywhere"))
            );
            assert_eq!(
                adapted.can_occupy_city,
                flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("CanOccupyCity"))
            );
            assert_eq!(
                adapted.missile,
                flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("Missile"))
            );
            assert_eq!(
                adapted.terrain_speed,
                flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("TerrainSpeed"))
            );
            assert_eq!(
                adapted.zoc,
                flags.iter().any(|flag| flag.eq_ignore_ascii_case("ZOC"))
            );
            assert_eq!(
                adapted.damage_slows,
                flags
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("DamageSlows"))
            );
        }
    }

    #[test]
    fn unknown_tile_costs_match_c2c3_terrain_class_and_igter_data() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        let fragments = adapter.move_fragments().unwrap();
        let igter_cost = adapter.igter_cost().unwrap();
        for kind in UnitKind::ALL_NATIVE {
            let class = adapter.unit_class_for_kind(kind).unwrap();
            let class_name = c2c3_unit_class_rule_name(class);
            let class_record = rules
                .rule_records
                .iter()
                .find(|record| {
                    record.namespace.eq_ignore_ascii_case("unit_class")
                        && record.rule_name.eq_ignore_ascii_case(class_name)
                })
                .expect("mapped C2C3 unit class");
            let class_flags = raw_class_flags(class_record);
            let terrain_speed = class_flags
                .iter()
                .any(|flag| flag.eq_ignore_ascii_case("TerrainSpeed"));
            let ignores_terrain = super::super::action_rules::freeciv_unit_type_flag(kind, "IgTer")
                .expect("explicit C2C3 IgTer binding");

            let mut expected = if !terrain_speed {
                fragments
            } else if ignores_terrain {
                igter_cost
            } else {
                rules
                    .content
                    .iter()
                    .filter(|record| record.kind == ContentKind::Terrain)
                    .filter(|record| {
                        raw_content_native_classes(record)
                            .iter()
                            .any(|name| name.eq_ignore_ascii_case(class_name))
                    })
                    .map(|record| raw_integer(record, "movement_cost"))
                    .max()
                    .unwrap_or(1)
                    .max(1)
                    * fragments
            };
            if rules
                .content
                .iter()
                .filter(|record| record.kind == ContentKind::Terrain)
                .any(|record| {
                    let native = raw_content_native_classes(record);
                    !native.is_empty()
                        && !native
                            .iter()
                            .any(|name| name.eq_ignore_ascii_case(class_name))
                })
            {
                expected *= 2;
            }
            assert_eq!(
                adapter.unknown_tile_move_cost(kind),
                Ok(expected),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn mapped_unit_records_supply_freeciv_class_before_civ3_class_fallback() {
        let rules = MovementRules::campaign();
        assert_eq!(UnitKind::Marine.unit_class(), UnitClass::SmallLand);
        assert_eq!(
            rules.unit_class_for_kind(UnitKind::Marine).unwrap(),
            UnitClass::Land
        );
        assert_eq!(
            rules.unit_class_for_kind(UnitKind::Worker).unwrap(),
            UnitClass::SmallLand
        );
        assert_eq!(
            rules.unit_class_for_kind(UnitKind::Panzer).unwrap(),
            UnitKind::Panzer.unit_class()
        );
    }

    #[test]
    fn full_terrain_and_extra_nativity_adapters_read_c2c3_lists() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        for (terrain, source_name) in TERRAIN_ADAPTER {
            let Some(source_name) = source_name else {
                assert!(matches!(
                    adapter.terrain_native_classes(terrain),
                    Err(MovementRulesError::UnsupportedTerrain(_))
                ));
                continue;
            };
            let record = terrain_record(rules, source_name);
            let expected = raw_content_native_classes(record);
            assert_eq!(adapter.terrain_native_classes(terrain).unwrap(), expected);
            for (class, class_name) in UNIT_CLASS_ADAPTER {
                assert_eq!(
                    adapter.terrain_is_native_to_class(terrain, class).unwrap(),
                    expected
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(class_name)),
                    "{terrain:?} / {class:?}"
                );
            }
            let source_flags = required_content_string_values(record, "flags").unwrap();
            for flag in TerrainFlag::ALL {
                assert_eq!(
                    adapter.terrain_has_flag(terrain, flag).unwrap(),
                    source_flags
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(flag.rule_name())),
                    "{terrain:?} / {flag:?}"
                );
            }
        }

        let extra_records: Vec<_> = rules
            .rule_records
            .iter()
            .filter(|record| {
                record.namespace == "extra"
                    && record.fields.iter().any(|entry| entry.key == "native_to")
            })
            .collect();
        assert!(!extra_records.is_empty());
        for record in extra_records {
            let expected = raw_rule_native_classes(record);
            assert_eq!(
                adapter.extra_native_classes(&record.rule_name).unwrap(),
                expected,
                "{}",
                record.rule_name
            );
            for (class, class_name) in UNIT_CLASS_ADAPTER {
                assert_eq!(
                    adapter
                        .extra_is_native_to_class(&record.rule_name, class)
                        .unwrap(),
                    expected
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(class_name)),
                    "{} / {class:?}",
                    record.rule_name
                );
            }
        }
        assert!(
            adapter
                .terrain_native_classes(Terrain::GrasslandShield)
                .unwrap()
                .iter()
                .any(|class| class == "Merchant")
        );
    }

    #[test]
    fn tile_native_adapter_uses_terrain_and_selected_extra_class_lists() {
        let adapter = MovementRules::campaign();
        for (base, name) in [
            (Base::Fort, "Fort"),
            (Base::Fortress, "Fortress"),
            (Base::Airstrip, "Airstrip"),
            (Base::Airbase, "Airbase"),
        ] {
            assert_eq!(c2c3_base_extra_rule_name(base), name);
            assert!(adapter.extra_native_classes(name).is_ok(), "{name}");
        }
        assert!(
            adapter
                .tile_native_to_class(
                    Terrain::Grassland,
                    false,
                    false,
                    false,
                    None,
                    UnitClass::BigLand
                )
                .unwrap()
        );
        assert!(
            !adapter
                .tile_native_to_class(
                    Terrain::Jungle,
                    false,
                    false,
                    false,
                    None,
                    UnitClass::BigLand
                )
                .unwrap()
        );
        assert!(
            adapter
                .tile_native_to_class(
                    Terrain::Jungle,
                    true,
                    false,
                    false,
                    None,
                    UnitClass::BigLand
                )
                .unwrap()
        );
        assert!(
            adapter
                .tile_native_to_class(
                    Terrain::Jungle,
                    false,
                    false,
                    false,
                    Some(Base::Fortress),
                    UnitClass::BigLand
                )
                .unwrap()
        );
        assert!(
            adapter
                .tile_native_to_class(
                    Terrain::Desert,
                    false,
                    false,
                    true,
                    None,
                    UnitClass::Trireme
                )
                .unwrap()
        );
        assert!(matches!(
            adapter.tile_native_to_class(
                Terrain::Volcano,
                true,
                true,
                true,
                Some(Base::Fortress),
                UnitClass::BigLand,
            ),
            Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
        ));
        assert!(
            adapter
                .movement_tile_native_to_class(
                    Terrain::Volcano,
                    false,
                    false,
                    false,
                    None,
                    UnitClass::SmallLand
                )
                .unwrap()
        );
        assert!(
            !adapter
                .movement_tile_native_to_class(
                    Terrain::Volcano,
                    false,
                    false,
                    false,
                    None,
                    UnitClass::BigLand
                )
                .unwrap()
        );
        assert!(
            adapter
                .movement_tile_native_to_class(
                    Terrain::Volcano,
                    true,
                    false,
                    false,
                    None,
                    UnitClass::BigLand
                )
                .unwrap()
        );
    }

    #[test]
    fn unit_igzoc_comes_from_selected_c2c3_unit_type_flags() {
        let adapter = MovementRules::campaign();
        for kind in UnitKind::ALL_NATIVE {
            let adapted = adapter.unit_movement_data(kind).unwrap();
            assert_eq!(
                adapted.class,
                adapter
                    .unit_class_movement_data(adapter.unit_class_for_kind(kind).unwrap())
                    .unwrap()
            );
            assert_eq!(
                adapted.ignores_zoc,
                super::super::action_rules::freeciv_unit_type_flag(kind, "IgZOC").unwrap(),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn volcano_uses_mountains_movement_cost_without_a_terrain_identity_mapping() {
        let adapter = MovementRules::campaign();
        assert_eq!(
            adapter.terrain_movement_cost(Terrain::Volcano).unwrap(),
            adapter.terrain_movement_cost(Terrain::Mountains).unwrap()
        );
        assert!(matches!(
            adapter.terrain_data(Terrain::Volcano),
            Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
        ));
        assert!(matches!(
            adapter.terrain_native_classes(Terrain::Volcano),
            Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
        ));
        assert!(matches!(
            adapter.terrain_has_flag(Terrain::Volcano, TerrainFlag::NoZoc),
            Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
        ));
    }

    #[test]
    fn move_fragment_parameters_are_validated_from_c2c3_terrain_ruleset() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        let move_fragments = rules
            .parameters
            .iter()
            .find(|parameter| {
                parameter.file == "terrain.ruleset"
                    && parameter.section == "parameters"
                    && parameter.key == "move_fragments"
            })
            .unwrap()
            .integer_value()
            .unwrap()
            .unwrap() as i32;
        assert_eq!(adapter.move_fragments().unwrap(), move_fragments);

        let igter_cost = rules
            .parameters
            .iter()
            .find(|parameter| {
                parameter.file == "terrain.ruleset"
                    && parameter.section == "parameters"
                    && parameter.key == "igter_cost"
            })
            .unwrap()
            .integer_value()
            .unwrap()
            .unwrap() as i32;
        assert_eq!(adapter.igter_cost().unwrap(), igter_cost);

        let mut missing_parameter = rules.clone();
        missing_parameter.parameters.retain(|parameter| {
            !(parameter.file == "terrain.ruleset"
                && parameter.section == "parameters"
                && parameter.key == "move_fragments")
        });
        assert!(matches!(
            MovementRules::new(&missing_parameter).move_fragments(),
            Err(MovementRulesError::MissingRecord { .. })
        ));
    }

    #[test]
    fn road_move_costs_come_from_all_selected_c2c3_road_records() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        for road in [
            RoadType::Road,
            RoadType::Railroad,
            RoadType::Maglev,
            RoadType::River,
        ] {
            let source_name = road.rule_name();
            let record = rules
                .rule_records
                .iter()
                .find(|record| {
                    record.namespace == "road" && record.rule_name.eq_ignore_ascii_case(source_name)
                })
                .expect("mapped C2C3 road record");
            assert_eq!(
                adapter.road_move_cost(road).unwrap(),
                raw_rule_integer(record, "move_cost")
            );
        }
    }

    #[test]
    fn pure_step_cost_combines_native_tiles_terrain_cost_and_infrastructure() {
        let rules = MovementRules::campaign();
        let ground = MovementTile::new(Terrain::Grassland, false, false, false, None);
        let hills = MovementTile::new(Terrain::Hills, false, false, false, None);
        let base_cost =
            rules.terrain_movement_cost(Terrain::Hills).unwrap() * rules.move_fragments().unwrap();
        assert_eq!(
            rules
                .step_cost(ground, hills, UnitClass::SmallLand, true, false, false)
                .unwrap(),
            base_cost
        );

        let river = MovementTile::new(Terrain::Grassland, false, false, true, None);
        let river_cost = rules.road_move_cost(RoadType::River).unwrap();
        assert_eq!(
            rules
                .step_cost(river, river, UnitClass::SmallLand, true, false, false)
                .unwrap(),
            (rules.terrain_movement_cost(Terrain::Grassland).unwrap()
                * rules.move_fragments().unwrap())
            .min(river_cost)
        );
        assert_eq!(
            rules
                .step_cost(river, river, UnitClass::SmallLand, false, false, false)
                .unwrap(),
            rules.terrain_movement_cost(Terrain::Grassland).unwrap()
                * rules.move_fragments().unwrap()
        );
        assert_eq!(
            rules
                .step_cost(river, river, UnitClass::SmallLand, true, true, false)
                .unwrap(),
            rules.terrain_movement_cost(Terrain::Grassland).unwrap()
                * rules.move_fragments().unwrap()
        );
        assert_eq!(
            rules
                .step_cost(ground, hills, UnitClass::SmallLand, true, false, true)
                .unwrap(),
            rules.igter_cost().unwrap()
        );
    }

    #[test]
    fn unsupported_or_incomplete_mappings_fail_instead_of_using_local_defaults() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        assert!(matches!(
            adapter.terrain_data(Terrain::Volcano),
            Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
        ));
        assert!(matches!(
            adapter.terrain_has_flag(Terrain::Volcano, TerrainFlag::NoZoc),
            Err(MovementRulesError::UnsupportedTerrain(Terrain::Volcano))
        ));

        let mut missing_terrain = rules.clone();
        missing_terrain
            .content
            .retain(|record| !(record.kind == ContentKind::Terrain && record.rule_name == "Swamp"));
        assert!(matches!(
            MovementRules::new(&missing_terrain).terrain_movement_cost(Terrain::Marsh),
            Err(MovementRulesError::MissingRecord { .. })
        ));

        let mut missing_class = rules.clone();
        missing_class.rule_records.retain(|record| {
            !(record.namespace == "unit_class" && record.rule_name == "Small Land")
        });
        assert!(matches!(
            MovementRules::new(&missing_class).unit_class_movement_data(UnitClass::SmallLand),
            Err(MovementRulesError::MissingRecord { .. })
        ));

        let mut missing_road = rules.clone();
        missing_road.rule_records.retain(|record| {
            !(record.namespace.eq_ignore_ascii_case("road")
                && record.rule_name.eq_ignore_ascii_case("Railroad"))
        });
        assert!(matches!(
            MovementRules::new(&missing_road).road_move_cost(RoadType::Railroad),
            Err(MovementRulesError::MissingRecord { .. })
        ));

        assert!(matches!(
            adapter.extra_native_classes("No Such Extra"),
            Err(MovementRulesError::MissingRecord { .. })
        ));
    }

    #[test]
    fn adapted_terrain_values_follow_mutated_records_not_copied_constants() {
        let mut rules = campaign_rules().clone();
        let desert_index = rules
            .content
            .iter()
            .position(|record| record.kind == ContentKind::Terrain && record.rule_name == "Desert")
            .unwrap();
        let mountains_index = rules
            .content
            .iter()
            .position(|record| {
                record.kind == ContentKind::Terrain && record.rule_name == "Mountains"
            })
            .unwrap();
        let plains_index = rules
            .content
            .iter()
            .position(|record| record.kind == ContentKind::Terrain && record.rule_name == "Plains")
            .unwrap();
        for (source_index, key) in [
            (mountains_index, "movement_cost"),
            (plains_index, "road_time"),
            (plains_index, "transform_time"),
        ] {
            let destination_field = rules.content[desert_index]
                .fields
                .iter()
                .position(|entry| entry.key == key)
                .unwrap();
            let source_field = rules.content[source_index]
                .fields
                .iter()
                .position(|entry| entry.key == key)
                .unwrap();
            swap_content_fields(
                &mut rules,
                desert_index,
                destination_field,
                source_index,
                source_field,
            );
        }
        let desert = &rules.content[desert_index];
        let adapted = MovementRules::new(&rules)
            .terrain_data(Terrain::FloodPlains)
            .unwrap();
        assert_eq!(adapted.movement_cost, raw_integer(desert, "movement_cost"));
        assert_eq!(
            adapted.activity_times.road,
            raw_integer(desert, "road_time")
        );
        assert_eq!(
            adapted.activity_times.transform,
            raw_integer(desert, "transform_time")
        );
    }

    #[test]
    fn malformed_or_invalid_fields_are_reported() {
        let mut rules = campaign_rules().clone();
        let plains = rules
            .content
            .iter_mut()
            .find(|record| record.kind == ContentKind::Terrain && record.rule_name == "Plains")
            .unwrap();
        let wrong_type = plains
            .fields
            .iter()
            .find(|entry| entry.key == "name")
            .unwrap()
            .clone();
        let movement_index = plains
            .fields
            .iter()
            .position(|entry| entry.key == "movement_cost")
            .unwrap();
        let mut wrong_type = wrong_type;
        wrong_type.key = "movement_cost".to_owned();
        plains.fields[movement_index] = wrong_type;
        assert!(matches!(
            MovementRules::new(&rules).terrain_movement_cost(Terrain::Plains),
            Err(MovementRulesError::InvalidField { .. })
        ));

        let mut rules = campaign_rules().clone();
        let railroad = rules
            .rule_records
            .iter_mut()
            .find(|record| {
                record.namespace.eq_ignore_ascii_case("road")
                    && record.rule_name.eq_ignore_ascii_case("Railroad")
            })
            .unwrap();
        let wrong_type = railroad
            .fields
            .iter()
            .find(|entry| entry.key == "extra")
            .unwrap()
            .clone();
        let move_index = railroad
            .fields
            .iter()
            .position(|entry| entry.key == "move_cost")
            .unwrap();
        let mut wrong_type = wrong_type;
        wrong_type.key = "move_cost".to_owned();
        railroad.fields[move_index] = wrong_type;
        assert!(matches!(
            MovementRules::new(&rules).road_move_cost(RoadType::Railroad),
            Err(MovementRulesError::InvalidField { .. })
        ));
    }

    #[test]
    fn road_move_modes_use_rule_values_and_the_freeciv_default() {
        let adapter = MovementRules::campaign();
        assert_eq!(
            adapter.road_move_mode(RoadType::River).unwrap(),
            RoadMoveMode::Cardinal
        );
        for road in [RoadType::Road, RoadType::Railroad, RoadType::Maglev] {
            assert_eq!(
                adapter.road_move_mode(road).unwrap(),
                RoadMoveMode::FastAlways
            );
        }
    }
}
