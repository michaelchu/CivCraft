//! Native TOML content for the CivCraft campaign.
//!
//! This module is the first migration boundary away from the legacy TSV
//! extraction tables.  It deliberately loads beside the current runtime
//! adapters until parity tests prove that switching consumers is safe.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnitCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) units: Vec<NativeUnit>,
}

// Catalog columns the engine does not consume yet: `expansion`,
// `ai_other_strategy`, `enslave_result_prototype_index`,
// `stealth_target_prototype_indices`, `create_craters`, `air_defense`,
// `icon_index`, `civilopedia_entry`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeUnit {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) expansion: String,
    pub(crate) source_biq: String,
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
    #[serde(default)]
    pub(crate) rules: Vec<NativeBuildingRule>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBuildingRule {
    pub(crate) legacy_runtime_index: usize,
    pub(crate) name: String,
    pub(crate) genus: String,
    pub(crate) cost: i32,
    pub(crate) upkeep: i32,
    pub(crate) requirements: Vec<NativeBuildingRequirement>,
    pub(crate) obsolete: Vec<NativeBuildingRequirement>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBuildingRequirement {
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) value: i32,
    #[serde(default)]
    pub(crate) second_value: i32,
    pub(crate) present: bool,
}

// Catalog columns the engine does not consume yet: `civilopedia_entry`,
// `doubles_happiness`, `gain_in_every_city`, `gain_in_every_city_on_continent`,
// `required_building_index`, `culture`, `bombard_defense`,
// `naval_bombard_defense`, `defense_bonus`, `naval_defense_bonus`,
// `maintenance_cost`, `content_faces_all_cities`, `content_faces`,
// `unhappy_faces_all_cities`, `unhappy_faces`, `number_of_required_buildings`,
// `air_power`, `naval_power`, `pollution`, `production`, `required_government`,
// `spaceship_part`, `required_advance_index`,
// `rendered_obsolete_by_advance_index`, `required_resource_1_index`,
// `required_resource_2_index`, `number_of_armies_required`, `flavors`,
// `unknown_tail_hex`, `unit_produced_index`, `unit_frequency`,
// `required_building_name`, `required_advance_name`,
// `rendered_obsolete_by_advance_name`, `required_resource_1_name`,
// `required_resource_2_name`, `unit_produced_name`.
#[allow(dead_code)]
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

// Catalog columns the engine does not consume yet: `source_record_index`.
#[allow(dead_code)]
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

// Catalog columns the engine does not consume yet: `assimilation`,
// `resistance`, `military_law`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct GovernmentCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) governments: Vec<NativeGovernment>,
    pub(crate) assimilation: Vec<NativeAssimilation>,
    pub(crate) resistance: Vec<NativeResistance>,
    pub(crate) military_law: Vec<NativeMilitaryLaw>,
}

impl GovernmentCatalog {
    pub(crate) fn get(&self, name: &str) -> Option<&NativeGovernment> {
        self.governments
            .iter()
            .find(|government| government.government.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeGovernment {
    pub(crate) government: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) required_technology: String,
    #[serde(default)]
    pub(crate) preferred_government: Option<String>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
    pub(crate) civil_war_chance: i32,
    pub(crate) empire_size_base: i32,
    pub(crate) max_rates: i32,
    pub(crate) town_free_units: i32,
    pub(crate) city_free_units: i32,
    pub(crate) metropolis_free_units: i32,
    pub(crate) cost_per_excess_unit_gold: i32,
}

// Catalog columns the engine does not consume yet: `source_section`,
// `source_record_index`, `government`, `assimilation_chance_percent`,
// `biq_sha256`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct NativeAssimilation {
    pub(crate) source_section: String,
    pub(crate) source_record_index: usize,
    pub(crate) government: String,
    pub(crate) assimilation_chance_percent: i32,
    pub(crate) biq_sha256: String,
}

// Catalog columns the engine does not consume yet: `captor_biq_index`,
// `captor_name`, `defender_biq_index`, `defender_name`, `modifier`,
// `biq_sha256`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct NativeResistance {
    pub(crate) captor_biq_index: usize,
    pub(crate) captor_name: String,
    pub(crate) defender_biq_index: usize,
    pub(crate) defender_name: String,
    pub(crate) modifier: i32,
    pub(crate) biq_sha256: String,
}

// Catalog columns the engine does not consume yet: `source_section`,
// `source_record_index`, `difficulty`, `military_law`, `biq_sha256`.
#[allow(dead_code)]
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
    #[serde(default)]
    pub(crate) resources: Vec<NativeResource>,
    #[serde(default)]
    pub(crate) roads: Vec<NativeRoad>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeResource {
    pub(crate) id: String,
    pub(crate) food: i32,
    pub(crate) shield: i32,
    pub(crate) trade: i32,
}

// Catalog columns the engine does not consume yet: `move_cost`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct NativeRoad {
    pub(crate) id: String,
    pub(crate) move_cost: i32,
    pub(crate) food_incr: i32,
    pub(crate) shield_incr: i32,
    pub(crate) trade_incr: i32,
    pub(crate) food_incr_const: i32,
    pub(crate) shield_incr_const: i32,
    pub(crate) trade_incr_const: i32,
    pub(crate) food_bonus: i32,
    pub(crate) shield_bonus: i32,
    pub(crate) trade_bonus: i32,
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
pub(crate) struct TerrainCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) parameters: NativeTerrainParameters,
    pub(crate) terrains: Vec<NativeTerrain>,
}

// Catalog columns the engine does not consume yet: `lake_max_size`,
// `min_start_native_area`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct NativeTerrainParameters {
    pub(crate) ocean_reclaim_requirement: i32,
    pub(crate) land_channel_requirement: i32,
    pub(crate) thaw_requirement: i32,
    pub(crate) freeze_requirement: i32,
    pub(crate) lake_max_size: i32,
    pub(crate) min_start_native_area: i32,
    pub(crate) move_fragments: i32,
    pub(crate) igter_cost: i32,
}

// Catalog columns the engine does not consume yet: `id`, `name`,
// `placing_time`, `property_ocean_depth`, `property_frozen`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct NativeTerrain {
    pub(crate) id: String,
    pub(crate) rule_name: String,
    pub(crate) name: String,
    pub(crate) class: String,
    pub(crate) movement_cost: i32,
    pub(crate) defense_bonus: i32,
    pub(crate) food: i32,
    pub(crate) shield: i32,
    pub(crate) trade: i32,
    pub(crate) road_food_incr_pct: i32,
    pub(crate) road_shield_incr_pct: i32,
    pub(crate) road_trade_incr_pct: i32,
    pub(crate) base_time: i32,
    pub(crate) road_time: i32,
    pub(crate) cultivate_time: i32,
    pub(crate) plant_time: i32,
    pub(crate) irrigation_food_incr: i32,
    pub(crate) irrigation_time: i32,
    pub(crate) mining_shield_incr: i32,
    pub(crate) mining_time: i32,
    pub(crate) transform_time: i32,
    pub(crate) placing_time: i32,
    pub(crate) pillage_time: i32,
    pub(crate) property_ocean_depth: i32,
    pub(crate) property_frozen: i32,
    pub(crate) cultivate_result: String,
    pub(crate) plant_result: String,
    pub(crate) transform_result: String,
    pub(crate) warmer_wetter_result: String,
    pub(crate) warmer_drier_result: String,
    pub(crate) cooler_wetter_result: String,
    pub(crate) cooler_drier_result: String,
    pub(crate) flags: Vec<String>,
    pub(crate) native_to: Vec<String>,
    pub(crate) extra_settings: Vec<NativeTerrainExtraSetting>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeTerrainExtraSetting {
    pub(crate) extra: String,
    pub(crate) removal_time: i32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettlementCatalog {
    pub(crate) barbarians: NativeBarbarianUnits,
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) ai: NativeSettlementAi,
    pub(crate) villages: NativeVillageRules,
    pub(crate) citizens: NativeCitizenRules,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeBarbarianUnits {
    pub(crate) basic_unit: String,
    pub(crate) advanced_unit: String,
    pub(crate) sea_unit: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeCitizenRules {
    pub(crate) partisan_unit: String,
    pub(crate) nationality: bool,
    pub(crate) partisans_pct: i32,
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
pub(crate) struct EffectCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) effects: Vec<NativeEffect>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ActionCatalog {
    pub(crate) schema_version: u32,
    pub(crate) source: String,
    pub(crate) actions: Vec<NativeAction>,
    pub(crate) enablers: Vec<NativeActionEnabler>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeAction {
    pub(crate) id: String,
    pub(crate) action: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeActionEnabler {
    pub(crate) id: String,
    pub(crate) action: String,
    #[serde(default)]
    pub(crate) actor_requirements: Vec<NativeEffectRequirement>,
    #[serde(default)]
    pub(crate) target_requirements: Vec<NativeEffectRequirement>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeEffect {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) value: i64,
    pub(crate) multiplier: Option<String>,
    #[serde(default)]
    pub(crate) requirements: Vec<NativeEffectRequirement>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeEffectRequirement {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) range: String,
    #[serde(default = "native_requirement_present")]
    pub(crate) present: bool,
    #[serde(default)]
    pub(crate) survives: bool,
    #[serde(default)]
    pub(crate) quiet: bool,
}

fn native_requirement_present() -> bool {
    true
}

// Catalog columns the engine does not consume yet: `id`.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct NativeExperienceLevel {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) base_hit_points: i32,
    pub(crate) retreat_bonus_percent: i32,
    pub(crate) promotion_chance_per_mille: i32,
    pub(crate) default_rank: bool,
}

// Catalog columns the engine does not consume yet: `civilopedia_entry`, `era`,
// `advance_icon`, `x`, `y`, `prerequisite_1_index`, `prerequisite_2_index`,
// `prerequisite_3_index`, `flags_hex`, `flavors`, `unknown_tail_hex`,
// `prerequisite_1_name`, `prerequisite_2_name`, `prerequisite_3_name`,
// `prerequisite_4_name`.
#[allow(dead_code)]
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
    #[serde(default)]
    pub(crate) flags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NativeCivilization {
    pub(crate) id: String,
    pub(crate) source_id: String,
    pub(crate) source_biq: String,
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

/// The schema version every content catalog must declare.
const SCHEMA_VERSION: u32 = 1;

/// Defines the accessor for one embedded content catalog. The catalog is
/// parsed once, must declare the supported schema version and its source, and
/// may run a cross-catalog validation before first use.
macro_rules! content_catalog {
    ($name:ident: $catalog:ty = $file:literal $(, validate = $validate:expr)?) => {
        pub(crate) fn $name() -> &'static $catalog {
            static CATALOG: OnceLock<$catalog> = OnceLock::new();
            CATALOG.get_or_init(|| {
                let catalog: $catalog = toml::from_str(include_str!(concat!(
                    "../../data/rulesets/civcraft/content/",
                    $file
                )))
                .unwrap_or_else(|error| panic!("content catalog {} is invalid: {error}", $file));
                assert_eq!(
                    catalog.schema_version, SCHEMA_VERSION,
                    "content catalog {} declares an unsupported schema version",
                    $file
                );
                assert!(
                    !catalog.source.trim().is_empty(),
                    "content catalog {} must record its source",
                    $file
                );
                $($validate(&catalog);)?
                catalog
            })
        }
    };
}

content_catalog!(units: UnitCatalog = "units.toml");
content_catalog!(civilizations: CivilizationCatalog = "civilizations.toml");
content_catalog!(buildings: BuildingCatalog = "buildings.toml");
content_catalog!(technologies: TechnologyCatalog = "technologies.toml");
content_catalog!(governments: GovernmentCatalog = "governments.toml");
content_catalog!(economy: EconomyCatalog = "economy.toml");
content_catalog!(terrain: TerrainCatalog = "terrain.toml");
content_catalog!(settlements: SettlementCatalog = "settlements.toml", validate = settlement_units_exist);
content_catalog!(experience: ExperienceCatalog = "experience_levels.toml");
content_catalog!(effects: EffectCatalog = "effects.toml");
content_catalog!(actions: ActionCatalog = "actions.toml");

fn settlement_units_exist(catalog: &SettlementCatalog) {
    for id in [
        &catalog.barbarians.basic_unit,
        &catalog.barbarians.advanced_unit,
        &catalog.barbarians.sea_unit,
        &catalog.citizens.partisan_unit,
    ] {
        assert!(
            units().units.iter().any(|unit| unit.id == *id),
            "native settlement policy references unknown unit {id:?}"
        );
    }
}

pub(crate) fn building_rule(runtime_index: usize) -> Option<&'static NativeBuildingRule> {
    buildings()
        .rules
        .iter()
        .find(|rule| rule.legacy_runtime_index == runtime_index)
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
        assert_eq!(buildings().rules.len(), 91);
        assert_eq!(technologies().schema_version, 1);
        assert_eq!(technologies().source, "Civilization III: Conquests");
        assert_eq!(technologies().technologies.len(), 83);
        assert_eq!(governments().schema_version, 1);
        assert_eq!(governments().governments.len(), 8);
        assert!(governments().governments.iter().all(|government| {
            !government.government.is_empty() && !government.name.is_empty()
        }));
        assert_eq!(
            governments()
                .governments
                .iter()
                .map(|government| government.government.as_str())
                .collect::<Vec<_>>(),
            [
                "anarchy",
                "despotism",
                "monarchy",
                "communism",
                "republic",
                "democracy",
                "fascism",
                "feudalism"
            ]
        );
        assert_eq!(
            governments().get("monarchy").unwrap().required_technology,
            "Monarchy"
        );
        assert_eq!(
            governments().get("republic").unwrap().required_technology,
            "The Republic"
        );
        assert_eq!(governments().assimilation.len(), 8);
        assert_eq!(governments().resistance.len(), 64);
        assert_eq!(governments().military_law.len(), 8);
        assert_eq!(economy().schema_version, 1);
        assert_eq!(economy().resources.len(), 20);
        assert_eq!(economy().roads.len(), 4);
        assert_eq!(terrain().schema_version, 1);
        assert_eq!(terrain().terrains.len(), 14);
        assert_eq!(settlements().schema_version, 1);
        assert_eq!(experience().schema_version, 1);
        assert_eq!(experience().levels.len(), 4);
        assert_eq!(effects().schema_version, 1);
        assert_eq!(effects().source, "CivCraft native effect graph");
        assert_eq!(effects().effects.len(), 508);
        assert_eq!(actions().schema_version, 1);
        assert_eq!(actions().source, "CivCraft native action graph");
        assert_eq!(actions().actions.len(), 19);
        assert_eq!(actions().enablers.len(), 89);
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
    fn native_buildings_and_bindings_have_stable_source_identity() {
        let catalog = buildings();
        let source_indices = catalog
            .buildings
            .iter()
            .map(|building| building.source_record_index)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(source_indices.len(), catalog.buildings.len());
        assert!(
            catalog
                .buildings
                .iter()
                .all(|building| !building.id.is_empty() && !building.name.is_empty())
        );
        assert!(catalog.bindings.iter().all(|binding| {
            catalog
                .buildings
                .iter()
                .any(|building| building.id == binding.civ3_id)
        }));
    }

    #[test]
    fn native_technologies_have_resolved_prerequisite_lists() {
        let catalog = technologies();
        assert!(catalog.technologies.iter().all(|technology| {
            !technology.id.is_empty()
                && !technology.name.is_empty()
                && technology.prerequisites.len() <= 4
                && technology
                    .prerequisites
                    .iter()
                    .all(|prerequisite| !prerequisite.is_empty())
        }));
    }

    #[test]
    fn native_units_have_art_variants() {
        assert!(units().units.iter().all(|unit| {
            !unit.id.is_empty() && !unit.art_folder.is_empty() && !unit.art_root.is_empty()
        }));
    }
}
