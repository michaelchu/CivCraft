//! Editable source rules imported from the installed Conquests campaign BIQ.
//!
//! This is the runtime reader for the native TOML unit catalog.
//! Keep simulation code independent of individual unit values so rebalance
//! edits to the table do not require changes to rule logic.

use crate::rules::native::{NativeExperienceLevel, NativeUnit};
use std::sync::OnceLock;

/// One campaign unit: its catalog record plus the bit fields and AI
/// strategies decoded from that record's packed columns. Scalar values are
/// read straight from the catalog record through `Deref`, so there is a
/// single copy of every statistic.
#[derive(Clone, Copy, Debug)]
pub(super) struct UnitRule {
    native: &'static NativeUnit,
    flags_1: [u8; 8],
    flags_3: [u8; 20],
    ai_strategies: u32,
}

impl std::ops::Deref for UnitRule {
    type Target = NativeUnit;

    fn deref(&self) -> &NativeUnit {
        self.native
    }
}

/// The catalog's experience levels, validated once on first use.
fn experience_levels() -> &'static [NativeExperienceLevel] {
    static LEVELS: OnceLock<&'static [NativeExperienceLevel]> = OnceLock::new();
    LEVELS.get_or_init(|| {
        let levels = crate::rules::native::experience().levels.as_slice();
        assert_eq!(
            levels.len(),
            4,
            "Conquests experience-level coverage changed"
        );
        assert_eq!(
            levels.iter().filter(|level| level.default_rank).count(),
            1,
            "Conquests must have one default experience level"
        );
        let default_index = levels
            .iter()
            .position(|level| level.default_rank)
            .expect("Conquests default experience level");
        assert_eq!(levels[default_index].name, "Regular");
        levels
    })
}

pub(super) fn default_experience_rank() -> usize {
    experience_levels()
        .iter()
        .position(|level| level.default_rank)
        .expect("Conquests default experience level")
}

pub(super) fn experience_rank(rank: usize) -> &'static NativeExperienceLevel {
    &experience_levels()[rank.min(experience_levels().len() - 1)]
}

impl UnitRule {
    /// The stable catalog identifier.
    pub(super) fn id(&self) -> &'static str {
        &self.native.id
    }

    /// The display name from the catalog.
    pub(super) fn name(&self) -> &'static str {
        &self.native.name
    }

    pub(super) fn has_flag_1(&self, byte: usize, bit: u8) -> bool {
        self.flags_1
            .get(byte)
            .is_some_and(|value| value & (1 << bit) != 0)
    }

    pub(super) fn has_flag_3(&self, byte: usize, bit: u8) -> bool {
        self.flags_3
            .get(byte)
            .is_some_and(|value| value & (1 << bit) != 0)
    }

    pub(super) fn has_lethal_land_bombard(&self) -> bool {
        self.has_flag_1(3, 3)
    }

    pub(super) fn has_lethal_sea_bombard(&self) -> bool {
        self.has_flag_1(3, 4)
    }

    pub(super) fn unit_class(&self) -> crate::sim::unit_catalog::UnitClass {
        use crate::sim::unit_catalog::UnitClass as C;
        if self.has_flag_1(0, 3) || self.has_flag_1(2, 4) || self.has_flag_1(2, 7) {
            return C::Missile;
        }
        match self.unit_type {
            1 if self.has_flag_1(1, 3) || self.has_flag_1(1, 4) => C::Trireme,
            1 => C::Sea,
            2 if self.id == "helicopter" => C::Helicopter,
            2 => C::Air,
            _ if self.has_flag_1(0, 0) => C::BigLand,
            _ if self.has_flag_1(0, 1) => C::SmallLand,
            _ => C::Land,
        }
    }

    pub(super) fn metadata(&self) -> crate::sim::unit_catalog_data::UnitMetadata {
        use crate::sim::unit_catalog::UnitClass as C;
        use crate::sim::unit_catalog::UnitFlag as F;
        let class = self.unit_class();
        let mut flags = 0u64;
        let mut set_flag = |flag: F, active: bool| {
            if active {
                flags |= 1u64 << flag as u32;
            }
        };
        let worker = self.flags_3[8] & 0xfc != 0 || self.flags_3[9] != 0 || self.flags_3[10] != 0;
        let nonmilitary = self.attack == 0
            && self.defense == 0
            && self.bombard_strength == 0
            && class != C::Missile;
        set_flag(F::Workers, worker);
        set_flag(F::NonMil, nonmilitary);
        let can_gain_ranks =
            (self.attack > 0 || self.defense > 0) && class != C::Missile && self.id != "leader";
        set_flag(F::NoVeteran, !can_gain_ranks);
        set_flag(F::CantFortify, !self.has_flag_3(0, 2));
        set_flag(
            F::HasNoZOC,
            matches!(class, C::Air | C::Helicopter | C::Missile),
        );
        set_flag(
            F::NoBuild,
            self.available_to.is_empty() || self.category == "wonder_created",
        );
        set_flag(F::Unique, self.category == "unique");
        set_flag(F::Cities, self.has_flag_3(8, 1));
        set_flag(F::Marines, self.has_flag_1(0, 6));
        set_flag(F::Submarine, self.has_flag_1(0, 7));
        set_flag(
            F::Nuclear,
            self.has_flag_1(2, 0) || self.has_flag_1(2, 4) || self.has_flag_1(2, 7),
        );
        set_flag(
            F::OneAttack,
            self.has_flag_1(0, 3) || self.has_flag_1(2, 4) || self.has_flag_1(2, 7),
        );
        set_flag(F::Bombarder, self.bombard_strength > 0);
        set_flag(F::Fortress, self.has_flag_3(8, 4));
        // Ordinary unit capture is based on combat capability, not the PRTO
        // StealthAttack flag at Flags3[6].0 (which is a different mechanic).
        set_flag(F::Capturer, self.attack > 0);
        set_flag(
            F::Capturable,
            self.attack == 0 && self.defense == 0 && class != C::Missile,
        );
        set_flag(F::GameLoss, self.has_flag_1(3, 5));
        set_flag(F::AirAttacker, self.has_flag_3(12, 2));
        crate::sim::unit_catalog_data::UnitMetadata {
            class,
            flags,
            ai_strategies: self.ai_strategies,
            tech: self.required_tech(),
            government: None,
            capacity: self.capacity,
            veteran_levels: if can_gain_ranks { 4 } else { 1 },
            attacks_air: self.has_flag_3(12, 2),
            stealth: self.has_flag_1(2, 5),
        }
    }

    fn civilization_mask_allows(&self, civilization: &str) -> bool {
        self.available_to
            .iter()
            .any(|available| available == civilization)
    }

    pub(super) fn available_to(&self, civilization: &str) -> bool {
        if self.category == "wonder_created"
            || (!self.unique_civilization.is_empty() && self.unique_civilization != civilization)
            || !self.civilization_mask_allows(civilization)
        {
            return false;
        }
        // Civ III's RACE mask alone can still include the replaced base unit.
        // The unique-unit relationship takes precedence for production.
        !rules().iter().any(|unique| {
            unique.category == "unique"
                && unique.unique_civilization == civilization
                && unique.replacement_id() == Some(self.id.as_str())
                && unique.civilization_mask_allows(civilization)
        })
    }

    pub(super) fn required_tech(&self) -> Option<crate::rules::Tech> {
        if self.required_technology.is_empty() {
            return None;
        }
        let alias = match self.required_technology.as_str() {
            "Amphibious War" => Some(crate::rules::Tech::AMPHIBIOUS_WARFARE),
            "Ironclads" => Some(crate::rules::Tech::STEAM_ENGINE),
            _ => None,
        };
        if alias.is_some() {
            return alias;
        }
        crate::rules::Tech::all().find(|tech| {
            tech.data()
                .is_some_and(|advance| advance.name == self.required_technology)
        })
    }

    pub(super) fn resources(&self) -> Vec<crate::sim::Resource> {
        self.required_resources
            .iter()
            .take(3)
            .filter(|name| !name.is_empty())
            .map(|name| match name.as_str() {
                "Aluminum" => crate::sim::Resource::Aluminum,
                "Coal" => crate::sim::Resource::Coal,
                "Horses" => crate::sim::Resource::Horses,
                "Iron" => crate::sim::Resource::Iron,
                "Oil" => crate::sim::Resource::Oil,
                "Rubber" => crate::sim::Resource::Rubber,
                "Saltpeter" => crate::sim::Resource::Saltpeter,
                "Uranium" => crate::sim::Resource::Uranium,
                other => panic!("unknown Conquests unit resource {other}"),
            })
            .collect()
    }

    pub(super) fn upgrade_target_id(&self) -> Option<&'static str> {
        if self.upgrade_to.is_empty() {
            return None;
        }
        rules()
            .iter()
            .find(|candidate| candidate.name == self.upgrade_to)
            .map(UnitRule::id)
    }

    pub(super) fn replacement_id(&self) -> Option<&'static str> {
        if self.replaces.is_empty() {
            return None;
        }
        rules()
            .iter()
            .find(|candidate| candidate.name == self.replaces)
            .map(UnitRule::id)
    }
}

fn parse_ai_strategy_mask(profiles: &str, row: usize) -> u32 {
    profiles.split('|').fold(0, |strategies, profile| {
        let mut parts = profile.split(':');
        let _prototype_index = parts.next();
        let _other_strategy = parts.next();
        let raw_flags = parts
            .next()
            .unwrap_or_else(|| panic!("missing source AI flags on Civ III unit table row {row}"));
        assert_eq!(raw_flags.len(), 16, "invalid source AI flags on row {row}");
        let bytes = raw_flags
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let pair = std::str::from_utf8(pair).expect("hex profile bytes");
                u8::from_str_radix(pair, 16)
                    .unwrap_or_else(|error| panic!("invalid source AI flags on row {row}: {error}"))
            })
            .collect::<Vec<_>>();
        strategies | u32::from(bytes[4]) | (u32::from(bytes[5]) << 8) | (u32::from(bytes[6]) << 16)
    })
}

fn parse_hex<const N: usize>(value: &str, row: usize, field: &str) -> [u8; N] {
    assert_eq!(
        value.len(),
        N * 2,
        "invalid {field} length on native unit row {row}"
    );
    let bytes = value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("native unit hex bytes");
            u8::from_str_radix(pair, 16)
                .unwrap_or_else(|error| panic!("invalid {field} on native unit row {row}: {error}"))
        })
        .collect::<Vec<_>>();
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("invalid {field} length on native unit row {row}"))
}

fn rules() -> &'static [UnitRule] {
    static RULES: OnceLock<Vec<UnitRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut parsed = Vec::new();
        for (index, native) in crate::rules::native::units().units.iter().enumerate() {
            let row = index + 1;
            // Decoding every packed column validates the record at load, even
            // where the engine does not consume the decoded value yet.
            let _: [u8; 8] = parse_hex(&native.flags_2_hex, row, "flags_2_hex");
            let _: [u8; 14] = parse_hex(
                &native.ignore_movement_cost_hex,
                row,
                "ignore_movement_cost_hex",
            );
            parsed.push(UnitRule {
                native,
                flags_1: parse_hex(&native.flags_1_hex, row, "flags_1_hex"),
                flags_3: parse_hex(&native.flags_3_hex, row, "flags_3_hex"),
                ai_strategies: parse_ai_strategy_mask(&native.source_ai_profiles, row),
            });
        }
        assert_eq!(
            parsed.len(),
            92,
            "Conquests campaign roster coverage changed"
        );
        parsed
    })
}

pub(super) fn get(id: &str) -> Option<&'static UnitRule> {
    rules().iter().find(|rule| rule.id == id)
}

#[cfg(test)]
pub(super) fn get_by_prototype_index(index: i32) -> Option<&'static UnitRule> {
    rules()
        .iter()
        .find(|rule| rule.source_prototypes.contains(&index))
}

#[cfg(feature = "scenarios")]
pub(super) fn all() -> &'static [UnitRule] {
    rules()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_runtime_table_loads_all_campaign_identities_and_source_values() {
        let units = rules();
        assert_eq!(units.len(), 92);
        assert_eq!(
            units
                .iter()
                .filter(|unit| unit.category == "unique")
                .count(),
            31
        );
        assert_eq!(
            units
                .iter()
                .filter(|unit| unit.category == "wonder_created")
                .count(),
            2
        );
        let warrior = get("warrior").unwrap();
        assert_eq!(
            (
                warrior.cost_shields,
                warrior.attack,
                warrior.defense,
                warrior.moves
            ),
            (10, 1, 1, 1)
        );
        let legionary = get("legionary").unwrap();
        assert_eq!(legionary.unique_civilization, "Romans");
        assert_eq!(legionary.replaces, "Swordsman");
        assert!(get("ancient_cavalry").is_some());
        assert!(get("crusader").is_some());
        assert_eq!(get("artillery").unwrap().bombard_range, 2);
        assert_eq!(get("tactical_nuke").unwrap().bombard_range, 6);
        assert_eq!(get("artillery").unwrap().bombard_strength, 12);
        assert_eq!(get("artillery").unwrap().rate_of_fire, 2);
        assert!(
            crate::sim::UnitKind::Bomber.has_lethal_bombard_against(crate::sim::UnitKind::Warrior)
        );
        assert!(
            crate::sim::UnitKind::Hwacha.has_lethal_bombard_against(crate::sim::UnitKind::Warrior)
        );
        assert!(
            crate::sim::UnitKind::Dromon
                .has_lethal_bombard_against(crate::sim::UnitKind::Battleship)
        );
        assert!(
            !crate::sim::UnitKind::Dromon.has_lethal_bombard_against(crate::sim::UnitKind::Warrior)
        );
        assert_eq!(get("war_elephant").unwrap().hit_point_bonus, 1);
        assert_eq!(get("ancient_cavalry").unwrap().hit_point_bonus, 1);
        assert_eq!(get("warrior").unwrap().hit_point_bonus, 0);
        assert_eq!(
            experience_levels()
                .iter()
                .map(|level| (level.name.as_str(), level.base_hit_points))
                .collect::<Vec<_>>(),
            vec![
                ("Conscript", 2),
                ("Regular", 3),
                ("Veteran", 4),
                ("Elite", 5)
            ]
        );
        assert_eq!(default_experience_rank(), 1);
        assert_eq!(crate::sim::UnitKind::Warrior.hit_points_at_rank(0), 2);
        assert_eq!(crate::sim::UnitKind::Warrior.hit_points_at_rank(3), 5);
        assert_eq!(crate::sim::UnitKind::WarElephant.hit_points_at_rank(1), 4);
        assert_eq!(crate::sim::UnitKind::WarElephant.hit_points_at_rank(3), 6);
        for unit in units {
            if !unit.required_technology.is_empty() {
                assert!(
                    unit.required_tech().is_some(),
                    "unresolved source technology {} for {}",
                    unit.required_technology,
                    unit.id
                );
            }
            let _ = unit.resources();
        }
        let unique = units
            .iter()
            .filter(|unit| unit.category == "unique")
            .collect::<Vec<_>>();
        assert_eq!(unique.len(), 31);
        for unit in unique {
            let civilization = unit.unique_civilization.as_str();
            let base_id = unit
                .replacement_id()
                .unwrap_or_else(|| panic!("unique unit {} has no replacement", unit.id));
            let base = get(base_id).unwrap();
            assert!(!base.available_to(civilization), "{}", unit.id);
            assert!(unit.available_to(civilization), "{}", unit.id);
        }
    }

    #[test]
    fn decoded_flag_bytes_round_trip_to_the_catalog_columns() {
        for rule in rules() {
            assert_eq!(rule.flags_1_hex, hex(rule.flags_1), "{}", rule.id);
            assert_eq!(rule.flags_3_hex, hex(rule.flags_3), "{}", rule.id);
        }
    }

    fn hex<const N: usize>(bytes: [u8; N]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
