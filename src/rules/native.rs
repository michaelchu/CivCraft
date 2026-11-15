//! Native TOML content for the CivCraft campaign.
//!
//! This module is the first migration boundary away from the legacy TSV
//! extraction tables.  It deliberately loads beside the current runtime
//! adapters until parity tests prove that switching consumers is safe.

#![allow(dead_code)]

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub(crate) struct UnitCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) units: Vec<NativeUnit>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeUnit {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) expansion: String,
    pub(crate) source_prototypes: Vec<i32>,
    pub(crate) cost_shields: i32,
    pub(crate) population_cost: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
    pub(crate) moves: i32,
    pub(crate) bombard_strength: i32,
    pub(crate) bombard_range: i32,
    pub(crate) rate_of_fire: i32,
    pub(crate) operational_range: i32,
    pub(crate) capacity: usize,
    pub(crate) required_technology: String,
    pub(crate) required_resources: Vec<String>,
    pub(crate) upgrade_to: String,
    pub(crate) available_to: Vec<String>,
    pub(crate) unique_civilization: String,
    pub(crate) replaces: String,
    pub(crate) unit_type: i32,
    pub(crate) flags_1_hex: String,
    pub(crate) flags_2_hex: String,
    pub(crate) flags_3_hex: String,
    pub(crate) hit_point_bonus: i32,
    pub(crate) ai_other_strategy: i32,
    pub(crate) source_ai_profiles: String,
    pub(crate) art_folder: String,
    pub(crate) art_root: String,
    pub(crate) ignore_movement_cost_hex: String,
    pub(crate) zone_of_control: i32,
    pub(crate) bombard_effects: i32,
    pub(crate) require_support: i32,
    pub(crate) enslave_result_prototype_index: i32,
    pub(crate) stealth_target_prototype_indices: Vec<i32>,
    pub(crate) create_craters: i32,
    pub(crate) worker_strength: f32,
    pub(crate) air_defense: i32,
    pub(crate) icon_index: i32,
    pub(crate) civilopedia_entry: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CivilizationCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) civilizations: Vec<NativeCivilization>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BuildingCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) buildings: Vec<NativeBuilding>,
    #[serde(default)]
    pub(crate) bindings: Vec<NativeBuildingBinding>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBuilding {
    pub(crate) id: String,
    pub(crate) source_record_index: usize,
    pub(crate) source_biq: String,
    pub(crate) name: String,
    pub(crate) civilopedia_entry: String,
    pub(crate) doubles_happiness: i32,
    pub(crate) gain_in_every_city: i32,
    pub(crate) gain_in_every_city_on_continent: i32,
    pub(crate) required_building_index: i32,
    pub(crate) cost: i32,
    pub(crate) culture: i32,
    pub(crate) bombard_defense: i32,
    pub(crate) naval_bombard_defense: i32,
    pub(crate) defense_bonus: i32,
    pub(crate) naval_defense_bonus: i32,
    pub(crate) maintenance_cost: i32,
    pub(crate) content_faces_all_cities: i32,
    pub(crate) content_faces: i32,
    pub(crate) unhappy_faces_all_cities: i32,
    pub(crate) unhappy_faces: i32,
    pub(crate) number_of_required_buildings: i32,
    pub(crate) air_power: i32,
    pub(crate) naval_power: i32,
    pub(crate) pollution: i32,
    pub(crate) production: i32,
    pub(crate) required_government: i32,
    pub(crate) spaceship_part: i32,
    pub(crate) required_advance_index: i32,
    pub(crate) rendered_obsolete_by_advance_index: i32,
    pub(crate) required_resource_1_index: i32,
    pub(crate) required_resource_2_index: i32,
    pub(crate) flags_hex: String,
    pub(crate) number_of_armies_required: i32,
    pub(crate) flavors: i32,
    pub(crate) unknown_tail_hex: String,
    pub(crate) unit_produced_index: i32,
    pub(crate) unit_frequency: i32,
    pub(crate) required_building_name: String,
    pub(crate) required_advance_name: String,
    pub(crate) rendered_obsolete_by_advance_name: String,
    pub(crate) required_resource_1_name: String,
    pub(crate) required_resource_2_name: String,
    pub(crate) unit_produced_name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBuildingBinding {
    pub(crate) civ3_id: String,
    pub(crate) source_record_index: usize,
    pub(crate) legacy_runtime_index: i32,
    pub(crate) mapping_status: String,
    pub(crate) mapping_note: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TechnologyCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) technologies: Vec<NativeTechnology>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GovernmentCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) governments: Vec<NativeGovernment>,
    pub(crate) assimilation: Vec<NativeAssimilation>,
    pub(crate) resistance: Vec<NativeResistance>,
    pub(crate) military_law: Vec<NativeMilitaryLaw>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeGovernment {
    pub(crate) government: String,
    pub(crate) town_free_units: i32,
    pub(crate) city_free_units: i32,
    pub(crate) metropolis_free_units: i32,
    pub(crate) cost_per_excess_unit_gold: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeAssimilation {
    pub(crate) source_section: String,
    pub(crate) source_record_index: usize,
    pub(crate) government: String,
    pub(crate) assimilation_chance_percent: i32,
    pub(crate) biq_sha256: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeResistance {
    pub(crate) captor_biq_index: usize,
    pub(crate) captor_name: String,
    pub(crate) defender_biq_index: usize,
    pub(crate) defender_name: String,
    pub(crate) modifier: i32,
    pub(crate) biq_sha256: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeMilitaryLaw {
    pub(crate) source_section: String,
    pub(crate) source_record_index: usize,
    pub(crate) difficulty: String,
    pub(crate) military_law: i32,
    pub(crate) biq_sha256: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EconomyCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) city: NativeCityEconomy,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeCityEconomy {
    pub(crate) city_min_distance: i32,
    pub(crate) food_cost: i32,
    pub(crate) happy_cost: i32,
    pub(crate) granary_food_initial: Vec<i32>,
    pub(crate) granary_food_increment: i32,
    pub(crate) foodbox_percent: i32,
    pub(crate) output_granularity: i32,
    pub(crate) city_center_minimum: [i32; 3],
    pub(crate) no_trade_size: i32,
    pub(crate) full_trade_size: i32,
    pub(crate) gold_upkeep_style: String,
    pub(crate) celebrate_size_limit: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettlementCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) ai: NativeSettlementAi,
    pub(crate) villages: NativeVillageRules,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeSettlementAi {
    pub(crate) food_priority: i32,
    pub(crate) shield_priority: i32,
    pub(crate) trade_priority: i32,
    pub(crate) research_priority_percent: i32,
    pub(crate) no_research_priority: i32,
    pub(crate) growth_priority: i32,
    pub(crate) growth_potential_deemphasis: i32,
    pub(crate) growth_food_threshold: i32,
    pub(crate) perfection: i32,
    pub(crate) good_enough: i32,
    pub(crate) discount_period: i32,
    pub(crate) defense_emphasis: i32,
    pub(crate) naval_emphasis: i32,
    pub(crate) wonder_city_pop_founder_divisor: i32,
    pub(crate) wonder_city_other_founder_divisor: i32,
    pub(crate) expansionist_trait_baseline: i32,
    pub(crate) ferry_cost_divisor: i32,
    pub(crate) ferry_tech_want: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeVillageRules {
    pub(crate) barbarian_onset_turn: i32,
    pub(crate) outcome_rolls: i32,
    pub(crate) map_reveal_radius_squared: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExperienceCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) levels: Vec<NativeExperienceLevel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UnitEconomicsCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) bindings: Vec<NativeUnitEconomicsBinding>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeUnitEconomicsBinding {
    pub(crate) civ3_unit_id: String,
    pub(crate) profile: String,
    pub(crate) mapping_kind: String,
    pub(crate) basis: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeExperienceLevel {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_hit_points: i32,
    pub(crate) retreat_bonus_percent: i32,
    pub(crate) promotion_chance_per_mille: i32,
    pub(crate) default_rank: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeTechnology {
    pub(crate) id: String,
    pub(crate) source_record_index: usize,
    pub(crate) source_biq: String,
    pub(crate) name: String,
    pub(crate) civilopedia_entry: String,
    pub(crate) cost: i32,
    pub(crate) era: i32,
    pub(crate) advance_icon: i32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) prerequisite_1_index: i32,
    pub(crate) prerequisite_2_index: i32,
    pub(crate) prerequisite_3_index: i32,
    pub(crate) prerequisite_4_index: i32,
    pub(crate) flags_hex: String,
    pub(crate) flavors: i32,
    pub(crate) unknown_tail_hex: String,
    pub(crate) prerequisite_1_name: String,
    pub(crate) prerequisite_2_name: String,
    pub(crate) prerequisite_3_name: String,
    pub(crate) prerequisite_4_name: String,
    pub(crate) prerequisites: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeCivilization {
    pub(crate) id: String,
    pub(crate) source_id: String,
    pub(crate) name: String,
    pub(crate) source_name: String,
    pub(crate) noun: String,
    pub(crate) adjective: String,
    pub(crate) leader: String,
    pub(crate) primary_color_index: u8,
    pub(crate) secondary_color_index: u8,
    pub(crate) city_style: u8,
    pub(crate) starting_techs: Vec<String>,
    pub(crate) legacy_names: Vec<String>,
    pub(crate) source_race_index: usize,
    pub(crate) cities: Vec<String>,
    pub(crate) culture_group_index: i32,
    pub(crate) expansionist: bool,
    pub(crate) militaristic: bool,
    pub(crate) starting_units: Vec<NativeUnitStack>,
    #[serde(default)]
    pub(crate) bonus_units: Vec<NativeUnitStack>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeUnitStack {
    pub(crate) id: String,
    pub(crate) count: i32,
}

pub(crate) fn units() -> &'static UnitCatalog {
    static CATALOG: OnceLock<UnitCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/units.toml"
        ))
        .expect("native unit TOML must be valid")
    })
}

pub(crate) fn civilizations() -> &'static CivilizationCatalog {
    static CATALOG: OnceLock<CivilizationCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/civilizations.toml"
        ))
        .expect("native civilization TOML must be valid")
    })
}

pub(crate) fn buildings() -> &'static BuildingCatalog {
    static CATALOG: OnceLock<BuildingCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/buildings.toml"
        ))
        .expect("native building TOML must be valid")
    })
}

pub(crate) fn technologies() -> &'static TechnologyCatalog {
    static CATALOG: OnceLock<TechnologyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/technologies.toml"
        ))
        .expect("native technology TOML must be valid")
    })
}

pub(crate) fn governments() -> &'static GovernmentCatalog {
    static CATALOG: OnceLock<GovernmentCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/governments.toml"
        ))
        .expect("native government TOML must be valid")
    })
}

pub(crate) fn economy() -> &'static EconomyCatalog {
    static CATALOG: OnceLock<EconomyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/economy.toml"
        ))
        .expect("native economy TOML must be valid")
    })
}

pub(crate) fn settlements() -> &'static SettlementCatalog {
    static CATALOG: OnceLock<SettlementCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/settlements.toml"
        ))
        .expect("native settlement TOML must be valid")
    })
}

pub(crate) fn experience() -> &'static ExperienceCatalog {
    static CATALOG: OnceLock<ExperienceCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/experience_levels.toml"
        ))
        .expect("native experience TOML must be valid")
    })
}

pub(crate) fn unit_economics() -> &'static UnitEconomicsCatalog {
    static CATALOG: OnceLock<UnitEconomicsCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str(include_str!(
            "../../data/rulesets/civcraft_native/unit_economics.toml"
        ))
        .expect("native unit economics TOML must be valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_catalogs_have_expected_schema_and_campaign_coverage() {
        assert_eq!(units().schema_version, 1);
        assert_eq!(units().source, "Civilization III: Conquests");
        assert_eq!(units().units.len(), 92);
        assert_eq!(civilizations().schema_version, 1);
        assert_eq!(civilizations().source, "Civilization III: Conquests");
        assert_eq!(civilizations().civilizations.len(), 31);
        assert_eq!(buildings().schema_version, 1);
        assert_eq!(buildings().source, "Civilization III: Conquests");
        assert_eq!(buildings().buildings.len(), 83);
        assert_eq!(buildings().bindings.len(), 83);
        assert_eq!(technologies().schema_version, 1);
        assert_eq!(technologies().source, "Civilization III: Conquests");
        assert_eq!(technologies().technologies.len(), 83);
        assert_eq!(governments().schema_version, 1);
        assert_eq!(governments().governments.len(), 8);
        assert_eq!(governments().assimilation.len(), 8);
        assert_eq!(governments().resistance.len(), 64);
        assert_eq!(governments().military_law.len(), 8);
        assert_eq!(economy().schema_version, 1);
        assert_eq!(settlements().schema_version, 1);
        assert_eq!(experience().schema_version, 1);
        assert_eq!(experience().levels.len(), 4);
        assert_eq!(unit_economics().schema_version, 1);
        assert_eq!(unit_economics().bindings.len(), 92);
        assert_eq!(
            experience()
                .levels
                .iter()
                .map(|level| (
                    level.name.as_str(),
                    level.base_hit_points,
                    level.retreat_bonus_percent,
                    level.promotion_chance_per_mille,
                    level.default_rank,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("Conscript", 2, 34, 500, false),
                ("Regular", 3, 50, 250, true),
                ("Veteran", 4, 58, 125, false),
                ("Elite", 5, 66, 0, false),
            ]
        );
    }

    #[test]
    fn native_civilization_ids_and_starts_are_unique_and_resolved() {
        let catalog = civilizations();
        let ids = catalog
            .civilizations
            .iter()
            .map(|civilization| civilization.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), catalog.civilizations.len());
        for civilization in &catalog.civilizations {
            assert_eq!(civilization.starting_techs.len(), 2);
            assert_eq!(civilization.starting_units.len(), 2);
            assert!(
                civilization
                    .starting_units
                    .iter()
                    .all(|stack| stack.count > 0)
            );
        }
    }

    #[test]
    fn native_buildings_and_bindings_match_the_extracted_tables() {
        let rows = tsv_rows(include_str!(
            "../../data/rulesets/civ3_conquests/buildings.tsv"
        ));
        assert_eq!(rows.len(), buildings().buildings.len());
        for (native, row) in buildings().buildings.iter().zip(rows) {
            assert_eq!(native.id, row[0]);
            assert_eq!(native.source_record_index.to_string(), row[1]);
            assert_eq!(native.source_biq, row[2]);
            assert_eq!(native.name, row[3]);
            assert_eq!(native.cost.to_string(), row[9]);
            assert_eq!(native.culture.to_string(), row[10]);
            assert_eq!(native.maintenance_cost.to_string(), row[15]);
            assert_eq!(native.required_advance_index.to_string(), row[27]);
            assert_eq!(native.flags_hex, row[31]);
            assert_eq!(native.required_building_name, row[37]);
            assert_eq!(native.required_advance_name, row[38]);
            assert_eq!(native.unit_produced_name, row[42]);
        }

        let bindings = tsv_rows(include_str!(
            "../../data/rulesets/civ3_conquests/building_catalog_bindings.tsv"
        ));
        assert_eq!(buildings().bindings.len(), bindings.len());
        for (native, row) in buildings().bindings.iter().zip(bindings) {
            assert_eq!(native.civ3_id, row[0]);
            assert_eq!(native.source_record_index.to_string(), row[1]);
            let expected_legacy_index = row[2].parse::<i32>().unwrap_or(-1);
            assert_eq!(native.legacy_runtime_index, expected_legacy_index);
            assert_eq!(native.mapping_status, row[3]);
            assert_eq!(native.mapping_note, row[4]);
        }
    }

    #[test]
    fn native_technologies_match_the_extracted_technology_table() {
        let rows = tsv_rows(include_str!(
            "../../data/rulesets/civ3_conquests/technologies.tsv"
        ));
        assert_eq!(technologies().technologies.len(), rows.len());
        for (native, row) in technologies().technologies.iter().zip(rows) {
            assert_eq!(native.id, row[0]);
            assert_eq!(native.source_record_index.to_string(), row[1]);
            assert_eq!(native.source_biq, row[2]);
            assert_eq!(native.name, row[3]);
            assert_eq!(native.cost.to_string(), row[5]);
            assert_eq!(native.era.to_string(), row[6]);
            assert_eq!(native.advance_icon.to_string(), row[7]);
            assert_eq!(native.x.to_string(), row[8]);
            assert_eq!(native.y.to_string(), row[9]);
            assert_eq!(native.flags_hex, row[14]);
            assert_eq!(native.prerequisite_1_name, row[17]);
            assert_eq!(native.prerequisite_2_name, row[18]);
            assert_eq!(native.prerequisite_3_name, row[19]);
            assert_eq!(native.prerequisite_4_name, row[20]);
            assert_eq!(
                native.prerequisites,
                row[17..21]
                    .iter()
                    .filter(|name| !name.is_empty())
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn native_unit_art_variants_match_the_extracted_unit_table() {
        let rows = tsv_rows(include_str!("../../data/rulesets/civ3_conquests/units.tsv"));
        assert_eq!(units().units.len(), rows.len());
        for (native, row) in units().units.iter().zip(rows) {
            assert_eq!(native.art_folder, row[31]);
            assert_eq!(native.art_root, row[32]);
        }
    }

    fn tsv_rows(source: &'static str) -> Vec<Vec<&'static str>> {
        source
            .lines()
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.split('\t').collect())
            .collect()
    }
}
