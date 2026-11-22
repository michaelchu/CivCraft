//! Ruleset-backed movement data adapted to Civ III runtime identities.
//!
//! CivCraft keeps its Civ III terrain and unit enums as stable simulation IDs.
//! This module is the explicit crosswalk from those IDs to the selected C2C3
//! ruleset records. It only reads data; movement callers can migrate to these
//! validated accessors independently.

use std::sync::OnceLock;

use crate::rules::engine_rules::{ExtraRule, RoadRule, RulesetData, extra_by_name, road_by_name};

use super::unit_catalog::UnitClass;
use super::{Base, Terrain, UnitKind};

/// `move_fragments`: fragments in one movement point.
const MOVE_FRAGMENTS: i32 = 6;

/// `igter_cost`: the cost of a step for a unit that ignores terrain.
const IGTER_COST: i32 = 2;

pub(super) const fn campaign_move_fragments() -> i32 {
    MOVE_FRAGMENTS
}

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

    pub(super) fn campaign_class_data(class: UnitClass) -> &'static UnitClassMovementData {
        static CLASSES: [OnceLock<UnitClassMovementData>; UnitClass::Missile as usize + 1] =
            [const { OnceLock::new() }; UnitClass::Missile as usize + 1];
        CLASSES[class as usize].get_or_init(|| Self::campaign().unit_class_movement_data(class))
    }

    pub(super) fn campaign_unit_class(kind: UnitKind) -> UnitClass {
        Self::campaign_unit_movement_data(kind).0
    }

    pub(super) fn campaign_unit_movement_data(
        kind: UnitKind,
    ) -> &'static (UnitClass, UnitClassMovementData) {
        type UnitMovement = (UnitClass, UnitClassMovementData);
        static UNITS: OnceLock<Vec<OnceLock<UnitMovement>>> = OnceLock::new();
        let slots = UNITS.get_or_init(|| (0..UnitKind::count()).map(|_| OnceLock::new()).collect());
        slots[kind.index()].get_or_init(|| {
            let class = Self::campaign()
                .unit_class_for_kind(kind)
                .expect("campaign C2C3 unit class binding");
            let data = Self::campaign_class_data(class).clone().with_unit(kind);
            (class, data)
        })
    }

    /// A lower bound for any allowed step, including moves that use only the
    /// remaining fragments this turn. Zero-cost terrain or roads disable it.
    pub(super) fn campaign_steps_cost_at_least_one() -> bool {
        static POSITIVE: OnceLock<bool> = OnceLock::new();
        *POSITIVE.get_or_init(|| {
            let rules = Self::campaign();
            Terrain::ALL.into_iter().all(|terrain| {
                rules
                    .terrain_movement_cost(terrain)
                    .expect("campaign terrain movement cost")
                    > 0
            }) && [RoadType::Road, RoadType::Railroad, RoadType::River]
                .into_iter()
                .all(|road| {
                    rules
                        .road_move_cost(road)
                        .expect("campaign road movement cost")
                        != 0
                })
        })
    }

    /// Resolve all movement and worker activity times for the C2C3 terrain
    /// corresponding to a Civ III terrain identity.
    pub(super) fn terrain_data(
        &self,
        terrain: Terrain,
    ) -> Result<TerrainMovementData, MovementRulesError> {
        let rule_name = terrain
            .engine_rule_name()
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let spec = crate::rules::terrain_catalog::terrain_spec(rule_name).ok_or_else(|| {
            MovementRulesError::MissingRecord {
                namespace: "terrain".to_owned(),
                rule_name: rule_name.to_owned(),
            }
        })?;
        Ok(TerrainMovementData {
            rule_name: spec.rule_name.to_owned(),
            movement_cost: spec.movement_cost,
            activity_times: TerrainActivityTimes {
                base: spec.base_time,
                road: spec.road_time,
                irrigation: spec.irrigation_time,
                mining: spec.mining_time,
                cultivate: spec.cultivate_time,
                plant: spec.plant_time,
                transform: spec.transform_time,
            },
            cultivate_result: spec.cultivate_result.map(str::to_owned),
            plant_result: spec.plant_result.map(str::to_owned),
            transform_result: spec.transform_result.map(str::to_owned),
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

    /// Movement-point fragments per whole movement point; road `move_cost`
    /// values are already expressed in these fragments.
    pub(super) const fn move_fragments(&self) -> i32 {
        MOVE_FRAGMENTS
    }

    /// Cost applied to unit types that ignore terrain movement costs.
    pub(super) const fn igter_cost(&self) -> i32 {
        IGTER_COST
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
        let terrain_rule_name = terrain
            .engine_rule_name()
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let spec =
            crate::rules::terrain_catalog::terrain_spec(terrain_rule_name).ok_or_else(|| {
                MovementRulesError::MissingRecord {
                    namespace: "terrain".to_owned(),
                    rule_name: terrain_rule_name.to_owned(),
                }
            })?;
        let time = spec.extra_removal_time(extra_rule_name);
        time.checked_mul(factor).ok_or_else(|| {
            invalid_field(
                spec.rule_name,
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
        let terrain_rule_name = terrain
            .engine_rule_name()
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let spec =
            crate::rules::terrain_catalog::terrain_spec(terrain_rule_name).ok_or_else(|| {
                MovementRulesError::MissingRecord {
                    namespace: "terrain".to_owned(),
                    rule_name: terrain_rule_name.to_owned(),
                }
            })?;
        spec.pillage_time().checked_mul(factor).ok_or_else(|| {
            invalid_field(
                spec.rule_name,
                "pillage_time",
                "pillage time exceeds i32 range after extra factor scaling",
            )
        })
    }

    fn extra_removal_config(
        &self,
        extra_rule_name: &str,
    ) -> Result<(&'static ExtraRule, i32, i32), MovementRulesError> {
        let extra = campaign_extra(extra_rule_name)?;
        Ok((extra, extra.removal_time, extra.removal_time_factor))
    }

    /// Return the selected terrain record's Freeciv `native_to` unit-class
    /// names, preserving entries such as `Merchant` that have no Civ III
    /// runtime class equivalent.
    pub(super) fn terrain_native_classes(
        &self,
        terrain: Terrain,
    ) -> Result<Vec<String>, MovementRulesError> {
        let rule_name = terrain
            .engine_rule_name()
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let spec = crate::rules::terrain_catalog::terrain_spec(rule_name).ok_or_else(|| {
            MovementRulesError::MissingRecord {
                namespace: "terrain".to_owned(),
                rule_name: rule_name.to_owned(),
            }
        })?;
        Ok(spec.native_to.iter().map(|s| (*s).to_owned()).collect())
    }

    pub(super) fn terrain_has_flag(
        &self,
        terrain: Terrain,
        flag: TerrainFlag,
    ) -> Result<bool, MovementRulesError> {
        self.terrain_has_named_flag(terrain, flag.rule_name())
    }

    pub(super) fn terrain_has_named_flag(
        &self,
        terrain: Terrain,
        flag: &str,
    ) -> Result<bool, MovementRulesError> {
        let rule_name = terrain
            .engine_rule_name()
            .ok_or(MovementRulesError::UnsupportedTerrain(terrain))?;
        let spec = crate::rules::terrain_catalog::terrain_spec(rule_name).ok_or_else(|| {
            MovementRulesError::MissingRecord {
                namespace: "terrain".to_owned(),
                rule_name: rule_name.to_owned(),
            }
        })?;
        Ok(spec.has_flag(flag))
    }

    pub(super) fn terrain_class(&self, terrain: Terrain) -> Result<&'a str, MovementRulesError> {
        let rule_name = terrain.engine_rugged_rule_name();
        let spec = crate::rules::terrain_catalog::terrain_spec(rule_name).ok_or_else(|| {
            MovementRulesError::MissingRecord {
                namespace: "terrain".to_owned(),
                rule_name: rule_name.to_owned(),
            }
        })?;
        Ok(spec.class_name())
    }

    /// Freeciv `common/combat.c::get_defense_power` terrain stage. Volcano has
    /// no direct terrain catalog identity; its combat terrain bonus is explicitly
    /// mapped to Mountains.
    pub(super) fn terrain_defense_bonus(
        &self,
        terrain: Terrain,
    ) -> Result<i32, MovementRulesError> {
        let rule_name = terrain.engine_rugged_rule_name();
        let spec = crate::rules::terrain_catalog::terrain_spec(rule_name).ok_or_else(|| {
            MovementRulesError::MissingRecord {
                namespace: "terrain".to_owned(),
                rule_name: rule_name.to_owned(),
            }
        })?;
        Ok(spec.defense_bonus)
    }

    pub(super) fn terrain_is_native_to_class(
        &self,
        terrain: Terrain,
        class: UnitClass,
    ) -> Result<bool, MovementRulesError> {
        let classes = self.terrain_native_classes(terrain)?;
        Ok(has_flag(&classes, class.rule_name()))
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
        if let Some(base) = base
            && self.extra_is_native_to_class(base.extra_rule_name(), class)?
        {
            return Ok(true);
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
        let move_fragments = self.move_fragments();
        let igter_cost = self.igter_cost();
        if !self.unit_class_movement_data(class).terrain_speed {
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
        let class_data = self.unit_class_movement_data(class);
        let fragments = self.move_fragments();
        let ignores_terrain =
            super::action_rules::unit_type_flag(kind, "IgTer").ok_or_else(|| {
                invalid_field(
                    &format!("unit {kind:?}"),
                    "flags",
                    "no selected C2C3 unit-type IgTer binding",
                )
            })?;

        let mut movement_cost = if !class_data.terrain_speed {
            fragments
        } else if ignores_terrain {
            self.igter_cost()
        } else {
            let mut worst_terrain_cost = 1;
            for terrain in crate::rules::terrain_catalog::TERRAIN_SPECS {
                if terrain.has_flag(TerrainFlag::NotGenerated.rule_name()) {
                    continue;
                }
                if terrain
                    .native_to
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(class.rule_name()))
                {
                    worst_terrain_cost = worst_terrain_cost.max(terrain.movement_cost);
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
        for terrain in crate::rules::terrain_catalog::TERRAIN_SPECS {
            if terrain.has_flag(TerrainFlag::NotGenerated.rule_name()) {
                continue;
            }
            if !terrain.native_to.is_empty()
                && !terrain
                    .native_to
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(class.rule_name()))
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

    /// The movement-relevant behavior of a Freeciv unit class. Civ III unit
    /// identity and class membership remain source-catalog inputs.
    pub(super) fn unit_class_movement_data(&self, class: UnitClass) -> UnitClassMovementData {
        UnitClassMovementData::campaign(class)
    }

    /// Resolve the Freeciv class for a retained Civ III unit identity. Mapped
    /// C2C3 unit records supply the class; the explicit no-record ledger uses
    /// the Civ III class crosswalk.
    pub(super) fn unit_class_for_kind(
        &self,
        kind: UnitKind,
    ) -> Result<UnitClass, MovementRulesError> {
        let rule_name = super::action_rules::unit_class_rule_name(kind).ok_or_else(|| {
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
        Ok(self
            .unit_class_movement_data(self.unit_class_for_kind(kind)?)
            .with_unit(kind))
    }

    /// Combine the selected ruleset's class behavior with an individual
    /// unit-level ZOC exemption from Civ III's source catalog. Freeciv's
    /// `IgZOC` is a unit-type flag, not a unit-class flag.
    pub(super) fn unit_movement_data(
        &self,
        kind: UnitKind,
    ) -> Result<UnitMovementData, MovementRulesError> {
        let ignores_zoc = super::action_rules::unit_type_flag(kind, "IgZOC").ok_or_else(|| {
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
        let extra = campaign_extra(extra_rule_name)?;
        let native_to = extra.native_to.ok_or_else(|| {
            invalid_field(&extra.section(), "native_to", "required field is missing")
        })?;
        Ok(native_to.iter().map(|class| (*class).to_owned()).collect())
    }

    pub(super) fn extra_is_native_to_class(
        &self,
        extra_rule_name: &str,
        class: UnitClass,
    ) -> Result<bool, MovementRulesError> {
        let classes = self.extra_native_classes(extra_rule_name)?;
        Ok(has_flag(&classes, class.rule_name()))
    }

    /// Read a selected extra's defender bonus after checking its native unit
    /// classes, as `tile_extras_class_defense_bonus` does for natural extras.
    pub(super) fn extra_class_defense_bonus(
        &self,
        extra_rule_name: &str,
        class: UnitClass,
    ) -> Result<i32, MovementRulesError> {
        let extra = campaign_extra(extra_rule_name)?;
        if !self.extra_is_native_to_class(extra_rule_name, class)? {
            return Ok(0);
        }
        Ok(extra.defense_bonus)
    }

    pub(super) fn road_move_cost(&self, road: RoadType) -> Result<i32, MovementRulesError> {
        Ok(campaign_road(road.rule_name())?.move_cost)
    }

    pub(super) fn road_move_mode(
        &self,
        road: RoadType,
    ) -> Result<RoadMoveMode, MovementRulesError> {
        let road = campaign_road(road.rule_name())?;
        RoadMoveMode::parse(
            &format!("road_{}", road.id),
            road.move_mode.unwrap_or("FastAlways"),
        )
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct MovementTile {
    pub(super) terrain: Terrain,
    pub(super) road: bool,
    pub(super) railroad: bool,
    pub(super) river: bool,
    pub(super) base: Option<Base>,
}

impl From<&super::Tile> for MovementTile {
    fn from(tile: &super::Tile) -> Self {
        Self::new(
            tile.terrain,
            tile.road,
            tile.railroad,
            tile.river,
            tile.base,
        )
    }
}

impl MovementTile {
    pub(super) const CACHE_BITS: u32 = 11;

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

    pub(super) fn cache_index(self) -> u32 {
        const TERRAIN_BITS: u32 = 5;
        const BASE_BITS: u32 = 3;
        const EXTRA_BITS: u32 = 3;
        const { assert!((Terrain::GrasslandShield as u32) < 1 << TERRAIN_BITS) };
        let base = match self.base {
            None => 0,
            Some(Base::Fort) => 1,
            Some(Base::Fortress) => 2,
            Some(Base::Airstrip) => 3,
            Some(Base::Airbase) => 4,
        };
        let extras =
            u32::from(self.road) | (u32::from(self.railroad) << 1) | (u32::from(self.river) << 2);
        ((self.terrain as u32) << (BASE_BITS + EXTRA_BITS)) | (base << EXTRA_BITS) | extras
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
    /// Selected class defense percentage while defending off native terrain.
    pub(super) non_native_def_pct: i32,
    pub(super) build_anywhere: bool,
    pub(super) can_occupy_city: bool,
    pub(super) kills_citizens: bool,
    pub(super) missile: bool,
    pub(super) terrain_defense: bool,
    pub(super) terrain_speed: bool,
    pub(super) zoc: bool,
    pub(super) damage_slows: bool,
}

impl UnitClassMovementData {
    /// The pinned `civ2civ3` class records. Every class defends at full
    /// strength off native terrain (`non_native_def_pct = 100`).
    fn campaign(class: UnitClass) -> Self {
        let none = Self {
            rule_name: class.rule_name().to_owned(),
            min_speed: 1,
            non_native_def_pct: 100,
            build_anywhere: false,
            can_occupy_city: false,
            kills_citizens: false,
            missile: false,
            terrain_defense: false,
            terrain_speed: false,
            zoc: false,
            damage_slows: false,
        };
        match class {
            UnitClass::Land => Self {
                build_anywhere: true,
                can_occupy_city: true,
                terrain_defense: true,
                terrain_speed: true,
                zoc: true,
                damage_slows: true,
                ..none
            },
            UnitClass::SmallLand => Self {
                build_anywhere: true,
                terrain_defense: true,
                terrain_speed: true,
                zoc: true,
                damage_slows: true,
                ..none
            },
            UnitClass::BigLand => Self {
                build_anywhere: true,
                can_occupy_city: true,
                kills_citizens: true,
                terrain_speed: true,
                zoc: true,
                damage_slows: true,
                ..none
            },
            UnitClass::Sea => Self {
                min_speed: 2,
                damage_slows: true,
                ..none
            },
            UnitClass::Trireme => Self {
                min_speed: 2,
                zoc: true,
                damage_slows: true,
                ..none
            },
            UnitClass::Helicopter => Self {
                can_occupy_city: true,
                ..none
            },
            UnitClass::Air => none,
            UnitClass::Missile => Self {
                min_speed: 0,
                missile: true,
                ..none
            },
        }
    }

    fn with_unit(mut self, kind: UnitKind) -> Self {
        let source = kind.conquests_rule();
        if source.unit_type == 0 {
            // Foot/wheeled flags restrict transport/terrain; they do not remove
            // Civ III ground-unit pillage, occupation, or terrain defense.
            self.can_occupy_city = source.attack > 0 || kind == UnitKind::Army;
        }
        self
    }
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
impl Terrain {
    /// See the crosswalk notes above; `None` only for Volcano.
    pub(crate) fn engine_rule_name(self) -> Option<&'static str> {
        Some(match self {
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

    /// [`Self::engine_rule_name`] with CivCraft's rugged-terrain adaptation:
    /// Volcano borrows the Mountains record for defense, output and terrain
    /// class. Its exact identity, terrain flags and worker actions stay distinct.
    pub(in crate::sim) fn engine_rugged_rule_name(self) -> &'static str {
        match self {
            Terrain::Volcano => "Mountains",
            terrain => terrain
                .engine_rule_name()
                .expect("every terrain except Volcano has a C2C3 record"),
        }
    }
}

impl UnitClass {
    /// The selected C2C3 unit-class rule name.
    pub(in crate::sim) fn rule_name(self) -> &'static str {
        match self {
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
}

impl Base {
    /// The selected C2C3 extra rule name for this base.
    pub(in crate::sim) fn extra_rule_name(self) -> &'static str {
        match self {
            Base::Fort => "Fort",
            Base::Fortress => "Fortress",
            Base::Airstrip => "Airstrip",
            Base::Airbase => "Airbase",
        }
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

fn campaign_extra(name: &str) -> Result<&'static ExtraRule, MovementRulesError> {
    extra_by_name(name).ok_or_else(|| MovementRulesError::MissingRecord {
        namespace: "extra".to_owned(),
        rule_name: name.to_owned(),
    })
}

fn campaign_road(name: &str) -> Result<&'static RoadRule, MovementRulesError> {
    road_by_name(name).ok_or_else(|| MovementRulesError::MissingRecord {
        namespace: "road".to_owned(),
        rule_name: name.to_owned(),
    })
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

    type ActivityTimeField = (&'static str, fn(TerrainActivityTimes) -> i32);
    const ACTIVITY_TIME_FIELDS: [ActivityTimeField; 7] = [
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
            assert_eq!(terrain.engine_rule_name(), Some(source_name));
            let spec = crate::rules::terrain_catalog::terrain_spec(source_name).unwrap();
            let adapted = adapter.terrain_data(terrain).unwrap();
            assert_eq!(adapted.rule_name, spec.rule_name, "{terrain:?}");
            assert_eq!(adapted.movement_cost, spec.movement_cost, "{terrain:?}");
            assert_eq!(
                adapter.terrain_defense_bonus(terrain).unwrap(),
                spec.defense_bonus,
                "{terrain:?}.defense_bonus"
            );
            assert_eq!(adapted.activity_times.base, spec.base_time);
            assert_eq!(adapted.activity_times.road, spec.road_time);
            assert_eq!(adapted.activity_times.irrigation, spec.irrigation_time);
            assert_eq!(adapted.activity_times.mining, spec.mining_time);
            assert_eq!(adapted.activity_times.cultivate, spec.cultivate_time);
            assert_eq!(adapted.activity_times.plant, spec.plant_time);
            assert_eq!(adapted.activity_times.transform, spec.transform_time);
            assert_eq!(
                adapted.cultivate_result.as_deref(),
                spec.cultivate_result,
                "{terrain:?}.cultivate_result"
            );
            assert_eq!(
                adapted.plant_result.as_deref(),
                spec.plant_result,
                "{terrain:?}.plant_result"
            );
            assert_eq!(
                adapted.transform_result.as_deref(),
                spec.transform_result,
                "{terrain:?}.transform_result"
            );
            for result_name in [
                &adapted.cultivate_result,
                &adapted.plant_result,
                &adapted.transform_result,
            ]
            .into_iter()
            .flatten()
            {
                assert!(
                    adapter.terrain_result(result_name).is_ok(),
                    "{terrain:?}: {result_name}"
                );
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
            let spec = crate::rules::terrain_catalog::terrain_spec(source_name).unwrap();
            let pillage_time = spec.pillage_time();
            for extra_name in ["Pollution", "Fallout"] {
                let terrain_time = spec.extra_removal_time(extra_name);
                let extra = extra_by_name(extra_name).expect("campaign extra");
                let direct_time = extra.removal_time;
                let factor = extra.removal_time_factor;
                let clean_expected = if direct_time != 0 {
                    direct_time
                } else {
                    terrain_time * factor
                };
                let pillage_expected = if direct_time != 0 {
                    direct_time
                } else {
                    pillage_time * factor
                };
                assert_eq!(
                    adapter.terrain_extra_removal_time(terrain, extra_name),
                    Ok(clean_expected),
                    "{source_name}/{extra_name}"
                );
                assert_eq!(
                    adapter.terrain_pillage_time(terrain, extra_name),
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
    fn unit_class_movement_data_matches_the_pinned_class_records() {
        let adapter = MovementRules::new(campaign_rules());
        // class, min_speed, build_anywhere, occupy, kills_citizens, missile,
        // terrain_defense, terrain_speed, zoc, damage_slows
        let expected = [
            (
                UnitClass::Land,
                1,
                true,
                true,
                false,
                false,
                true,
                true,
                true,
                true,
            ),
            (
                UnitClass::SmallLand,
                1,
                true,
                false,
                false,
                false,
                true,
                true,
                true,
                true,
            ),
            (
                UnitClass::BigLand,
                1,
                true,
                true,
                true,
                false,
                false,
                true,
                true,
                true,
            ),
            (
                UnitClass::Sea,
                2,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
            ),
            (
                UnitClass::Trireme,
                2,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                true,
            ),
            (
                UnitClass::Air,
                1,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
            ),
            (
                UnitClass::Helicopter,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
            ),
            (
                UnitClass::Missile,
                0,
                false,
                false,
                false,
                true,
                false,
                false,
                false,
                false,
            ),
        ];
        for (class, speed, anywhere, occupy, kills, missile, defense, terrain, zoc, slows) in
            expected
        {
            let data = adapter.unit_class_movement_data(class);
            assert_eq!(data.rule_name, class.rule_name());
            assert_eq!(data.non_native_def_pct, 100, "{class:?}");
            assert_eq!(
                (
                    data.min_speed,
                    data.build_anywhere,
                    data.can_occupy_city,
                    data.kills_citizens,
                    data.missile,
                    data.terrain_defense,
                    data.terrain_speed,
                    data.zoc,
                    data.damage_slows,
                ),
                (
                    speed, anywhere, occupy, kills, missile, defense, terrain, zoc, slows
                ),
                "{class:?}"
            );
        }
    }

    #[test]
    fn river_combat_bonus_uses_the_selected_extra_and_unit_class() {
        let adapter = MovementRules::campaign();
        for (class, expected) in [
            (UnitClass::Land, 25),
            (UnitClass::SmallLand, 25),
            (UnitClass::BigLand, 0),
            (UnitClass::Trireme, 25),
            (UnitClass::Sea, 0),
            (UnitClass::Air, 0),
            (UnitClass::Helicopter, 0),
            (UnitClass::Missile, 0),
        ] {
            assert_eq!(
                adapter.extra_class_defense_bonus("River", class).unwrap(),
                expected,
                "{class:?}"
            );
        }
    }

    #[test]
    fn unknown_tile_costs_match_c2c3_terrain_class_and_igter_data() {
        let rules = campaign_rules();
        let adapter = MovementRules::new(rules);
        let fragments = adapter.move_fragments();
        let igter_cost = adapter.igter_cost();
        for kind in UnitKind::all() {
            let class = adapter.unit_class_for_kind(kind).unwrap();
            let class_name = class.rule_name();
            let terrain_speed = adapter.unit_class_movement_data(class).terrain_speed;
            let ignores_terrain = super::super::action_rules::unit_type_flag(kind, "IgTer")
                .expect("explicit C2C3 IgTer binding");

            let mut expected = if !terrain_speed {
                fragments
            } else if ignores_terrain {
                igter_cost
            } else {
                crate::rules::terrain_catalog::TERRAIN_SPECS
                    .iter()
                    .filter(|spec| {
                        spec.native_to
                            .iter()
                            .any(|name| name.eq_ignore_ascii_case(class_name))
                    })
                    .map(|spec| spec.movement_cost)
                    .max()
                    .unwrap_or(1)
                    .max(1)
                    * fragments
            };
            if crate::rules::terrain_catalog::TERRAIN_SPECS
                .iter()
                .any(|spec| {
                    !spec.native_to.is_empty()
                        && !spec
                            .native_to
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
    fn unit_classes_come_directly_from_native_prto() {
        let rules = MovementRules::campaign();
        for kind in UnitKind::all() {
            assert_eq!(rules.unit_class_for_kind(kind).unwrap(), kind.unit_class());
        }
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
            let spec = crate::rules::terrain_catalog::terrain_spec(source_name).unwrap();
            let expected: Vec<String> = spec.native_to.iter().map(|s| (*s).to_owned()).collect();
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
            for flag in TerrainFlag::ALL {
                assert_eq!(
                    adapter.terrain_has_flag(terrain, flag).unwrap(),
                    spec.has_flag(flag.rule_name()),
                    "{terrain:?} / {flag:?}"
                );
            }
        }

        let extra_records: Vec<_> = crate::rules::engine_rules::campaign_extras()
            .iter()
            .filter(|extra| extra.native_to.is_some())
            .collect();
        assert!(!extra_records.is_empty());
        for record in extra_records {
            let expected = record.native_to.unwrap();
            assert_eq!(
                adapter.extra_native_classes(record.name).unwrap(),
                expected,
                "{}",
                record.name
            );
            for (class, class_name) in UNIT_CLASS_ADAPTER {
                assert_eq!(
                    adapter
                        .extra_is_native_to_class(record.name, class)
                        .unwrap(),
                    expected
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(class_name)),
                    "{} / {class:?}",
                    record.name
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
            assert_eq!(base.extra_rule_name(), name);
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
        for kind in UnitKind::all() {
            let adapted = adapter.unit_movement_data(kind).unwrap();
            assert_eq!(
                adapted.class,
                adapter.unit_class_movement_data_for_kind(kind).unwrap()
            );
            assert_eq!(
                adapted.ignores_zoc,
                super::super::action_rules::unit_type_flag(kind, "IgZOC").unwrap(),
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
        assert_eq!(
            adapter.terrain_defense_bonus(Terrain::Volcano).unwrap(),
            adapter.terrain_defense_bonus(Terrain::Mountains).unwrap()
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
    fn movement_scale_and_igter_cost_match_the_pinned_ruleset() {
        let adapter = MovementRules::campaign();
        assert_eq!(adapter.move_fragments(), 6);
        assert_eq!(adapter.igter_cost(), 2);
    }

    #[test]
    fn configured_movement_scale_reaches_starting_units_path_cost_and_tired_attack() {
        use crate::sim::{HUMAN, Simulation, Unit};

        let rules = MovementRules::campaign();
        let scale = rules.move_fragments();
        let game = Simulation::new(crate::test_support::seeded_map());
        assert!(!game.units().is_empty());
        assert!(
            game.units()
                .iter()
                .all(|unit| { unit.moves_left == unit.kind.stats().moves * scale })
        );

        let kind = UnitKind::Warrior;
        let class = rules.unit_class_for_kind(kind).unwrap();
        let tile = MovementTile::new(Terrain::Hills, false, false, false, None);
        assert_eq!(
            rules
                .step_cost(tile, tile, class, true, false, false)
                .unwrap(),
            scale * rules.terrain_movement_cost(Terrain::Hills).unwrap()
        );
        let mut unit = Unit::new(1, HUMAN, kind, 0, 0);
        unit.moves_left = scale;
        let full_power = Simulation::attack_power(&unit);
        unit.moves_left = scale - 1;
        assert_eq!(
            Simulation::attack_power(&unit),
            full_power * (scale - 1) / scale
        );
    }

    #[test]
    fn road_move_costs_come_from_all_selected_c2c3_road_records() {
        let adapter = MovementRules::campaign();
        for (road, expected) in [
            (RoadType::Road, 2),
            (RoadType::Railroad, 1),
            (RoadType::Maglev, 0),
            (RoadType::River, 2),
        ] {
            assert_eq!(adapter.road_move_cost(road).unwrap(), expected, "{road:?}");
        }
    }

    #[test]
    fn pure_step_cost_combines_native_tiles_terrain_cost_and_infrastructure() {
        let rules = MovementRules::campaign();
        let ground = MovementTile::new(Terrain::Grassland, false, false, false, None);
        let hills = MovementTile::new(Terrain::Hills, false, false, false, None);
        let base_cost =
            rules.terrain_movement_cost(Terrain::Hills).unwrap() * rules.move_fragments();
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
            (rules.terrain_movement_cost(Terrain::Grassland).unwrap() * rules.move_fragments())
                .min(river_cost)
        );
        assert_eq!(
            rules
                .step_cost(river, river, UnitClass::SmallLand, false, false, false)
                .unwrap(),
            rules.terrain_movement_cost(Terrain::Grassland).unwrap() * rules.move_fragments()
        );
        assert_eq!(
            rules
                .step_cost(river, river, UnitClass::SmallLand, true, true, false)
                .unwrap(),
            rules.terrain_movement_cost(Terrain::Grassland).unwrap() * rules.move_fragments()
        );
        assert_eq!(
            rules
                .step_cost(ground, hills, UnitClass::SmallLand, true, false, true)
                .unwrap(),
            rules.igter_cost()
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

        assert!(matches!(
            adapter.extra_native_classes("No Such Extra"),
            Err(MovementRulesError::MissingRecord { .. })
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
