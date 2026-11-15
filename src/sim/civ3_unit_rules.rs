//! Editable source rules imported from the installed Conquests campaign BIQ.
//!
//! This is the runtime reader for the native TOML unit catalog.
//! Keep simulation code independent of individual unit values so rebalance
//! edits to the table do not require changes to rule logic.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub(super) struct UnitRule {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) category: &'static str,
    /// All original PRTO prototype indices represented by this campaign identity.
    /// Multiple indices occur when the same unit identity appears in more than
    /// one installed BIQ.
    #[allow(dead_code)] // Retained from the source catalog for prototype-index lookups.
    pub(super) source_prototypes: &'static str,
    pub(super) required_technology: &'static str,
    pub(super) required_resources: [&'static str; 3],
    pub(super) upgrade_to: &'static str,
    pub(super) cost: i32,
    pub(super) population_cost: i32,
    pub(super) attack: i32,
    pub(super) defense: i32,
    pub(super) moves: i32,
    pub(super) unit_type: i32,
    pub(super) capacity: usize,
    pub(super) operational_range: i32,
    pub(super) bombard_strength: i32,
    pub(super) bombard_range: i32,
    pub(super) rate_of_fire: i32,
    pub(super) hit_point_bonus: i32,
    flags_1: [u8; 8],
    flags_2: [u8; 8],
    flags_3: [u8; 20],
    ai_strategies: u32,
    #[allow(dead_code)] // Retained source terrain flags for movement rule integration.
    ignore_movement_cost: [u8; 14],
    #[allow(dead_code)]
    pub(super) zone_of_control: i32,
    pub(super) bombard_effects: i32,
    #[allow(dead_code)]
    pub(super) require_support: i32,
    #[allow(dead_code)] // Retained source result for the unfinished enslave action path.
    pub(super) enslave_result_prototype_index: i32,
    #[allow(dead_code)] // Retained source capability for the unfinished crater action path.
    pub(super) create_craters: i32,
    pub(super) worker_strength: f32,
    #[allow(dead_code)]
    pub(super) air_defense: i32,
    #[allow(dead_code)]
    pub(super) icon_index: i32,
    #[allow(dead_code)]
    pub(super) civilopedia_entry: &'static str,
    pub(super) available_to: &'static str,
    #[allow(dead_code)]
    pub(super) unique_civilization: &'static str,
    #[allow(dead_code)]
    pub(super) replaces: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ExperienceRule {
    pub(super) name: &'static str,
    pub(super) base_hit_points: i32,
    #[allow(dead_code)]
    pub(super) retreat_bonus_percent: i32,
    #[allow(dead_code)] // Retained source experience value for veteran promotion behavior.
    pub(super) promotion_chance_per_mille: i32,
    pub(super) is_default: bool,
}

fn experience_levels() -> &'static [ExperienceRule] {
    static LEVELS: OnceLock<Vec<ExperienceRule>> = OnceLock::new();
    LEVELS.get_or_init(|| {
        let parsed = crate::rules::native::experience()
            .levels
            .iter()
            .map(|level| ExperienceRule {
                name: leak(&level.name),
                base_hit_points: level.base_hit_points,
                retreat_bonus_percent: level.retreat_bonus_percent,
                promotion_chance_per_mille: level.promotion_chance_per_mille,
                is_default: level.default_rank,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            parsed.len(),
            4,
            "Conquests experience-level coverage changed"
        );
        assert_eq!(
            parsed.iter().filter(|level| level.is_default).count(),
            1,
            "Conquests must have one default experience level"
        );
        let default_index = parsed
            .iter()
            .position(|level| level.is_default)
            .expect("Conquests default experience level");
        assert_eq!(parsed[default_index].name, "Regular");
        parsed
    })
}

pub(super) fn default_experience_rank() -> usize {
    experience_levels()
        .iter()
        .position(|level| level.is_default)
        .expect("Conquests default experience level")
}

pub(super) fn experience_rank(rank: usize) -> &'static ExperienceRule {
    &experience_levels()[rank.min(experience_levels().len() - 1)]
}

impl UnitRule {
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

    #[allow(dead_code)]
    pub(super) fn has_flag_2(&self, byte: usize, bit: u8) -> bool {
        self.flags_2
            .get(byte)
            .is_some_and(|value| value & (1 << bit) != 0)
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
        let nonmilitary = self.attack == 0 && self.defense == 0;
        set_flag(F::Workers, worker);
        set_flag(F::NonMil, nonmilitary);
        let can_gain_ranks = !nonmilitary && class != C::Missile && self.id != "leader";
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
        let cargo = if self.id == "nuclear_submarine" {
            // Conquests grants this unit one tactical-missile slot. Regular
            // submarines have no transport capacity, and nuclear submarines
            // do not carry land or air units.
            &[C::Missile][..]
        } else if self.has_flag_1(1, 0) {
            &[C::Air, C::Helicopter][..]
        } else if self.capacity > 0 && matches!(class, C::Sea | C::Trireme) {
            &[C::Land, C::SmallLand, C::BigLand, C::Missile][..]
        } else if self.capacity > 0 && class == C::Air {
            &[C::Missile][..]
        } else if self.capacity > 0 && self.id == "army" {
            &[C::Land, C::SmallLand, C::BigLand][..]
        } else {
            &[]
        };
        crate::sim::unit_catalog_data::UnitMetadata {
            class,
            flags,
            ai_strategies: self.ai_strategies,
            tech: self.required_tech(),
            government: None,
            cargo,
            capacity: self.capacity,
            fuel: if matches!(class, C::Air | C::Helicopter) {
                self.operational_range.min(u8::MAX as i32) as u8
            } else {
                0
            },
            veteran_levels: if can_gain_ranks { 4 } else { 1 },
            attacks_air: self.has_flag_3(12, 2),
            stealth: self.has_flag_1(2, 5),
        }
    }

    fn civilization_mask_allows(&self, civilization: &str) -> bool {
        self.available_to
            .split(';')
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
                && unique.replacement_id() == Some(self.id)
                && unique.civilization_mask_allows(civilization)
        })
    }

    pub(super) fn required_tech(&self) -> Option<crate::rules::Tech> {
        if self.required_technology.is_empty() {
            return None;
        }
        let alias = match self.required_technology {
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
            .filter(|name| !name.is_empty())
            .map(|name| match *name {
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
            .map(|candidate| candidate.id)
    }

    pub(super) fn replacement_id(&self) -> Option<&'static str> {
        if self.replaces.is_empty() {
            return None;
        }
        rules()
            .iter()
            .find(|candidate| candidate.name == self.replaces)
            .map(|candidate| candidate.id)
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

fn leak(value: &str) -> &'static str {
    Box::leak(value.to_owned().into_boxed_str())
}

fn join_indices(values: &[i32]) -> &'static str {
    leak(
        &values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(";"),
    )
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
            let mut required_resources = [""; 3];
            for (slot, resource) in native.required_resources.iter().take(3).enumerate() {
                required_resources[slot] = leak(resource);
            }
            let available_to = leak(&native.available_to.join(";"));
            let unique_civilization = leak(&native.unique_civilization);
            let replaces = leak(&native.replaces);
            parsed.push(UnitRule {
                id: leak(&native.id),
                name: leak(&native.name),
                category: leak(&native.category),
                source_prototypes: join_indices(&native.source_prototypes),
                required_technology: leak(&native.required_technology),
                required_resources,
                upgrade_to: leak(&native.upgrade_to),
                cost: native.cost_shields,
                population_cost: native.population_cost,
                attack: native.attack,
                defense: native.defense,
                moves: native.moves,
                unit_type: native.unit_type,
                capacity: native.capacity,
                operational_range: native.operational_range,
                bombard_strength: native.bombard_strength,
                bombard_range: native.bombard_range,
                rate_of_fire: native.rate_of_fire,
                hit_point_bonus: native.hit_point_bonus,
                flags_1: parse_hex(&native.flags_1_hex, row, "flags_1_hex"),
                flags_2: parse_hex(&native.flags_2_hex, row, "flags_2_hex"),
                flags_3: parse_hex(&native.flags_3_hex, row, "flags_3_hex"),
                ai_strategies: parse_ai_strategy_mask(&native.source_ai_profiles, row),
                ignore_movement_cost: parse_hex(
                    &native.ignore_movement_cost_hex,
                    row,
                    "ignore_movement_cost_hex",
                ),
                zone_of_control: native.zone_of_control,
                bombard_effects: native.bombard_effects,
                require_support: native.require_support,
                enslave_result_prototype_index: native.enslave_result_prototype_index,
                create_craters: native.create_craters,
                worker_strength: native.worker_strength,
                air_defense: native.air_defense,
                icon_index: native.icon_index,
                civilopedia_entry: leak(&native.civilopedia_entry),
                available_to,
                unique_civilization,
                replaces,
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
    rules().iter().find(|rule| {
        rule.source_prototypes
            .split(';')
            .any(|source_index| source_index.parse::<i32>().ok() == Some(index))
    })
}

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
            (warrior.cost, warrior.attack, warrior.defense, warrior.moves),
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
                .map(|level| (level.name, level.base_hit_points))
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
            let civilization = unit.unique_civilization;
            let base_id = unit
                .replacement_id()
                .unwrap_or_else(|| panic!("unique unit {} has no replacement", unit.id));
            let base = get(base_id).unwrap();
            assert!(!base.available_to(civilization), "{}", unit.id);
            assert!(unit.available_to(civilization), "{}", unit.id);
        }
    }

    #[test]
    fn native_toml_unit_catalog_matches_the_legacy_extraction() {
        let native = crate::rules::native::units();
        assert_eq!(native.units.len(), rules().len());
        for source in rules() {
            let migrated = native
                .units
                .iter()
                .find(|unit| unit.id == source.id)
                .unwrap_or_else(|| panic!("native TOML is missing {}", source.id));
            assert_eq!(migrated.name, source.name);
            assert_eq!(migrated.category, source.category);
            assert_eq!(migrated.cost_shields, source.cost);
            assert_eq!(migrated.population_cost, source.population_cost);
            assert_eq!(migrated.attack, source.attack);
            assert_eq!(migrated.defense, source.defense);
            assert_eq!(migrated.moves, source.moves);
            assert_eq!(migrated.bombard_strength, source.bombard_strength);
            assert_eq!(migrated.bombard_range, source.bombard_range);
            assert_eq!(migrated.rate_of_fire, source.rate_of_fire);
            assert_eq!(migrated.operational_range, source.operational_range);
            assert_eq!(migrated.capacity, source.capacity);
            assert_eq!(migrated.required_technology, source.required_technology);
            assert_eq!(
                migrated.required_resources,
                source
                    .required_resources
                    .iter()
                    .filter(|resource| !resource.is_empty())
                    .map(|resource| (*resource).to_owned())
                    .collect::<Vec<_>>()
            );
            assert_eq!(migrated.upgrade_to, source.upgrade_to);
            assert_eq!(
                migrated.available_to,
                source
                    .available_to
                    .split(';')
                    .filter(|civilization| !civilization.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            );
            assert_eq!(migrated.unique_civilization, source.unique_civilization);
            assert_eq!(migrated.replaces, source.replaces);
            assert_eq!(migrated.unit_type, source.unit_type);
            assert_eq!(migrated.flags_1_hex, hex(source.flags_1));
            assert_eq!(migrated.flags_2_hex, hex(source.flags_2));
            assert_eq!(migrated.flags_3_hex, hex(source.flags_3));
            assert_eq!(migrated.hit_point_bonus, source.hit_point_bonus);
            assert_eq!(
                migrated.ignore_movement_cost_hex,
                hex(source.ignore_movement_cost)
            );
            assert_eq!(migrated.zone_of_control, source.zone_of_control);
            assert_eq!(migrated.bombard_effects, source.bombard_effects);
            assert_eq!(migrated.require_support, source.require_support);
            assert_eq!(
                migrated.enslave_result_prototype_index,
                source.enslave_result_prototype_index
            );
            assert_eq!(migrated.create_craters, source.create_craters);
            assert_eq!(migrated.worker_strength, source.worker_strength);
            assert_eq!(migrated.air_defense, source.air_defense);
            assert_eq!(migrated.icon_index, source.icon_index);
            assert_eq!(migrated.civilopedia_entry, source.civilopedia_entry);
        }
    }

    fn hex<const N: usize>(bytes: [u8; N]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
