//! Editable source rules imported from the installed Conquests campaign BIQ.
//!
//! This is the runtime reader for `data/rulesets/civ3_conquests/units.tsv`.
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
    #[allow(dead_code)] // Retained source rule for the unfinished bombard action path.
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
    #[allow(dead_code)] // Retained source effects for the unfinished bombard action path.
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
        let mut lines =
            include_str!("../../data/rulesets/civ3_conquests/experience_levels.tsv").lines();
        let header = lines.next().expect("Civ III experience table has a header");
        let expected_columns = header.split('\t').count();
        let mut parsed = Vec::new();
        for (line_number, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                expected_columns,
                "invalid Civ III experience table row {}",
                line_number + 2
            );
            let number = |column: usize| {
                fields[column].parse::<i32>().unwrap_or_else(|error| {
                    panic!(
                        "invalid experience value on row {}: {error}",
                        line_number + 2
                    )
                })
            };
            parsed.push(ExperienceRule {
                name: fields[1],
                base_hit_points: number(2),
                retreat_bonus_percent: number(3),
                promotion_chance_per_mille: number(4),
                is_default: fields[5] == "true",
            });
        }
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

    #[cfg(test)]
    pub(super) fn has_lethal_land_bombard(&self) -> bool {
        self.has_flag_1(3, 3)
    }

    #[cfg(test)]
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

fn validate_prototype_indices(value: &str, row: usize, field: &str) {
    if value.is_empty() {
        return;
    }
    for index in value.split(';') {
        index.parse::<u16>().unwrap_or_else(|error| {
            panic!("invalid {field} PRTO index on Civ III unit table row {row}: {error}")
        });
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
            .chunks_exact(2)
            .map(|pair| {
                let pair = std::str::from_utf8(pair).expect("hex profile bytes");
                u8::from_str_radix(pair, 16)
                    .unwrap_or_else(|error| panic!("invalid source AI flags on row {row}: {error}"))
            })
            .collect::<Vec<_>>();
        strategies | u32::from(bytes[4]) | (u32::from(bytes[5]) << 8) | (u32::from(bytes[6]) << 16)
    })
}

fn rules() -> &'static [UnitRule] {
    static RULES: OnceLock<Vec<UnitRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut lines = include_str!("../../data/rulesets/civ3_conquests/units.tsv").lines();
        let header = lines.next().expect("Civ III unit table has a header");
        let expected_columns = header.split('\t').count();
        let mut parsed = Vec::new();
        let parse_bytes = |value: &str, row: usize| -> Vec<u8> {
            assert_eq!(value.len() % 2, 0, "invalid flag bytes on row {row}");
            value
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    let pair = std::str::from_utf8(pair).unwrap();
                    u8::from_str_radix(pair, 16)
                        .unwrap_or_else(|error| panic!("invalid flag bytes on row {row}: {error}"))
                })
                .collect()
        };
        for (line_number, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                expected_columns,
                "invalid Civ III unit table row {}",
                line_number + 2
            );
            let number = |column: usize| {
                fields[column].parse::<i32>().unwrap_or_else(|error| {
                    panic!(
                        "invalid numeric field on Civ III unit table row {}: {error}",
                        line_number + 2
                    )
                })
            };
            validate_prototype_indices(fields[4], line_number + 2, "source prototype");
            validate_prototype_indices(fields[38], line_number + 2, "stealth target");
            let flags_1 = parse_bytes(fields[25], line_number + 2);
            let flags_2 = parse_bytes(fields[26], line_number + 2);
            let flags_3 = parse_bytes(fields[27], line_number + 2);
            let flags_1: [u8; 8] = flags_1.try_into().unwrap_or_else(|_| {
                panic!(
                    "invalid flags_1 length on Civ III unit table row {}",
                    line_number + 2
                )
            });
            let flags_2: [u8; 8] = flags_2.try_into().unwrap_or_else(|_| {
                panic!(
                    "invalid flags_2 length on Civ III unit table row {}",
                    line_number + 2
                )
            });
            let flags_3: [u8; 20] = flags_3.try_into().unwrap_or_else(|_| {
                panic!(
                    "invalid flags_3 length on Civ III unit table row {}",
                    line_number + 2
                )
            });
            let ignore_movement_cost: [u8; 14] = parse_bytes(fields[33], line_number + 2)
                .try_into()
                .unwrap_or_else(|_| {
                    panic!(
                        "invalid ignore_movement_cost length on Civ III unit table row {}",
                        line_number + 2
                    )
                });
            let ai_strategies = parse_ai_strategy_mask(fields[30], line_number + 2);
            parsed.push(UnitRule {
                id: fields[0],
                name: fields[1],
                category: fields[2],
                source_prototypes: fields[4],
                required_technology: fields[16],
                required_resources: [fields[17], fields[18], fields[19]],
                upgrade_to: fields[20],
                cost: number(6),
                population_cost: number(7),
                attack: number(8),
                defense: number(9),
                moves: number(10),
                unit_type: number(24),
                capacity: number(15) as usize,
                operational_range: number(14),
                bombard_strength: number(11),
                bombard_range: number(12),
                rate_of_fire: number(13),
                hit_point_bonus: number(28),
                flags_1,
                flags_2,
                flags_3,
                ai_strategies,
                ignore_movement_cost,
                zone_of_control: number(34),
                bombard_effects: number(35),
                require_support: number(36),
                enslave_result_prototype_index: number(37),
                create_craters: number(39),
                worker_strength: fields[40].parse::<f32>().unwrap_or_else(|error| {
                    panic!(
                        "invalid worker strength on Civ III unit table row {}: {error}",
                        line_number + 2
                    )
                }),
                air_defense: number(41),
                icon_index: number(42),
                civilopedia_entry: fields[43],
                available_to: fields[21],
                unique_civilization: fields[22],
                replaces: fields[23],
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
}
