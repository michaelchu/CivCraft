//! Native campaign unit rules, independent of imported source records.
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
    capabilities: &'static [String],
    ai_strategies: u32,
    #[allow(dead_code)]
    pub(super) zone_of_control: i32,
    pub(super) bombard_effects: i32,
    #[allow(dead_code)]
    pub(super) require_support: i32,
    #[allow(dead_code)] // Result identity retained for the unfinished enslave action.
    pub(super) enslave_result: &'static str,
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
    pub(super) fn has_capability(&self, name: &str) -> bool {
        self.capabilities
            .iter()
            .any(|capability| capability == name)
    }

    pub(super) fn has_lethal_land_bombard(&self) -> bool {
        self.has_capability("LethalLandBombard")
    }

    pub(super) fn has_lethal_sea_bombard(&self) -> bool {
        self.has_capability("LethalSeaBombard")
    }

    pub(super) fn unit_class(&self) -> crate::sim::unit_catalog::UnitClass {
        use crate::sim::unit_catalog::UnitClass as C;
        if self.has_capability("CruiseMissile")
            || self.has_capability("TacticalNuclear")
            || self.has_capability("Icbm")
        {
            return C::Missile;
        }
        match self.unit_type {
            1 if self.has_capability("SinksInSea") || self.has_capability("SinksInOcean") => {
                C::Trireme
            }
            1 => C::Sea,
            2 if self.id == "helicopter" => C::Helicopter,
            2 => C::Air,
            _ if self.has_capability("Wheeled") => C::BigLand,
            _ if self.has_capability("Foot") => C::SmallLand,
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
        let worker = self
            .capabilities
            .iter()
            .any(|capability| capability == "Worker");
        let nonmilitary = self.attack == 0
            && self.defense == 0
            && self.bombard_strength == 0
            && class != C::Missile;
        set_flag(F::Workers, worker);
        set_flag(F::NonMil, nonmilitary);
        let can_gain_ranks =
            (self.attack > 0 || self.defense > 0) && class != C::Missile && self.id != "leader";
        set_flag(F::NoVeteran, !can_gain_ranks);
        set_flag(F::CantFortify, !self.has_capability("Fortify"));
        set_flag(
            F::HasNoZOC,
            matches!(class, C::Air | C::Helicopter | C::Missile),
        );
        set_flag(
            F::NoBuild,
            self.available_to.is_empty() || self.category == "wonder_created",
        );
        set_flag(F::Unique, self.category == "unique");
        set_flag(F::Cities, self.has_capability("FoundCity"));
        set_flag(F::Marines, self.has_capability("Amphibious"));
        set_flag(F::Submarine, self.has_capability("Submarine"));
        set_flag(
            F::Nuclear,
            self.has_capability("Nuclear")
                || self.has_capability("TacticalNuclear")
                || self.has_capability("Icbm"),
        );
        set_flag(
            F::OneAttack,
            self.has_capability("CruiseMissile")
                || self.has_capability("TacticalNuclear")
                || self.has_capability("Icbm"),
        );
        set_flag(F::Bombarder, self.bombard_strength > 0);
        set_flag(F::Fortress, self.has_capability("BuildFortress"));
        // Ordinary unit capture is based on combat capability, not the PRTO
        // StealthAttack flag at Flags3[6].0 (which is a different mechanic).
        set_flag(F::Capturer, self.attack > 0);
        set_flag(
            F::Capturable,
            self.attack == 0 && self.defense == 0 && class != C::Missile,
        );
        set_flag(F::GameLoss, self.has_capability("GameLoss"));
        set_flag(F::AirAttacker, self.has_capability("AttackAir"));
        crate::sim::unit_catalog_data::UnitMetadata {
            class,
            flags,
            ai_strategies: self.ai_strategies,
            tech: self.required_tech(),
            government: None,
            capacity: self.capacity,
            veteran_levels: if can_gain_ranks { 4 } else { 1 },
            attacks_air: self.has_capability("AttackAir"),
            stealth: self.has_capability("Stealth"),
        }
    }

    fn civilization_mask_allows(&self, source_noun: &str) -> bool {
        self.available_to
            .split(';')
            .any(|available| available == source_noun)
    }

    /// Resolve a stable civilization ID to the canonical Civ III noun used by
    /// the unit catalog. Display labels never determine gameplay eligibility.
    pub(super) fn available_to(&self, civilization_id: &str) -> bool {
        let Some(civilization) = crate::rules::civilizations::by_id(civilization_id) else {
            return false;
        };
        let source_noun = civilization.noun.as_str();
        if self.category == "wonder_created"
            || (!self.unique_civilization.is_empty() && self.unique_civilization != source_noun)
            || !self.civilization_mask_allows(source_noun)
        {
            return false;
        }
        !rules().iter().any(|unique| {
            unique.category == "unique"
                && unique.unique_civilization == source_noun
                && unique.replacement_id() == Some(self.id)
                && unique.civilization_mask_allows(source_noun)
        })
    }

    pub(super) fn required_tech(&self) -> Option<crate::rules::Tech> {
        if self.required_technology.is_empty() {
            return None;
        }
        crate::rules::technologies::runtime_index(self.required_technology).map(crate::rules::Tech)
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
            .find(|candidate| candidate.id == self.upgrade_to)
            .map(|candidate| candidate.id)
    }

    pub(super) fn replacement_id(&self) -> Option<&'static str> {
        if self.replaces.is_empty() {
            return None;
        }
        rules()
            .iter()
            .find(|candidate| candidate.id == self.replaces)
            .map(|candidate| candidate.id)
    }
}

fn leak(value: &str) -> &'static str {
    Box::leak(value.to_owned().into_boxed_str())
}

fn rules() -> &'static [UnitRule] {
    static RULES: OnceLock<Vec<UnitRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut parsed = Vec::new();
        for native in &crate::rules::native::units().units {
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
                required_technology: leak(&native.required_technology),
                required_resources,
                upgrade_to: leak(&native.upgrade_to),
                cost: native.cost_shields,
                population_cost: native.population_cost,
                attack: native.attack,
                defense: native.defense,
                moves: native.moves,
                unit_type: native.domain.index(),
                capacity: native.capacity,
                operational_range: native.operational_range,
                bombard_strength: native.bombard_strength,
                bombard_range: native.bombard_range,
                rate_of_fire: native.rate_of_fire,
                hit_point_bonus: native.hit_point_bonus,
                capabilities: &native.capabilities,
                ai_strategies: native.ai_roles.iter().fold(0, |mask, role| {
                    let bit = crate::rules::unit_capabilities::AI_ROLES
                        .iter()
                        .position(|name| *name == role)
                        .expect("validated AI role");
                    mask | (1 << bit)
                }),
                zone_of_control: native.zone_of_control,
                bombard_effects: native.bombard_effects,
                require_support: native.require_support,
                enslave_result: leak(&native.enslave_result),
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
pub(super) fn all() -> &'static [UnitRule] {
    rules()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_and_ai_roles_preserve_native_baseline() {
        let expected: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../tests/fixtures/unit_capabilities_v1.json"
        ))
        .unwrap();
        assert_eq!(expected.len(), rules().len());
        for baseline in expected {
            let rule = get(baseline["id"].as_str().unwrap()).unwrap();
            let expected: Vec<String> =
                serde_json::from_value(baseline["capabilities"].clone()).unwrap();
            assert_eq!(rule.capabilities, expected);
            assert_eq!(
                rule.ai_strategies as u64,
                baseline["ai_strategy_mask"].as_u64().unwrap()
            );
            assert_eq!(rule.unit_type as i64, baseline["domain"].as_i64().unwrap());
        }
    }

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
        assert_eq!(legionary.replaces, "swordsman");
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
            let civilization = crate::rules::civilizations::all()
                .iter()
                .find(|c| c.noun == unit.unique_civilization)
                .unwrap()
                .id
                .as_str();
            let base_id = unit
                .replacement_id()
                .unwrap_or_else(|| panic!("unique unit {} has no replacement", unit.id));
            let base = get(base_id).unwrap();
            assert!(!base.available_to(civilization), "{}", unit.id);
            assert!(unit.available_to(civilization), "{}", unit.id);
        }
    }

    #[test]
    fn every_civilization_id_can_build_basic_units() {
        for civilization in crate::rules::civilizations::all() {
            for id in ["settler", "worker"] {
                assert!(
                    get(id).unwrap().available_to(&civilization.id),
                    "{} must be able to build {id}",
                    civilization.name
                );
            }
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
            assert_eq!(migrated.domain.index(), source.unit_type);
            assert_eq!(migrated.hit_point_bonus, source.hit_point_bonus);
            assert_eq!(migrated.zone_of_control, source.zone_of_control);
            assert_eq!(migrated.bombard_effects, source.bombard_effects);
            assert_eq!(migrated.require_support, source.require_support);
            assert_eq!(migrated.enslave_result, source.enslave_result);
            assert_eq!(migrated.create_craters, source.create_craters);
            assert_eq!(migrated.worker_strength, source.worker_strength);
            assert_eq!(migrated.air_defense, source.air_defense);
            assert_eq!(migrated.icon_index, source.icon_index);
            assert_eq!(migrated.civilopedia_entry, source.civilopedia_entry);
        }
    }
}
