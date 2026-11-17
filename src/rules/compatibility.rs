//! Stable building save identities and retained rule-selection policy.
//! Array positions are serialized identities. Never reorder existing entries.
//! Some inherited buildings use the legacy obsolescence fallback; preserve that
//! policy independently of the removed import catalog.
pub(crate) const BUILDINGS: &[(&str, bool)] = &[
    ("building_airport", false),
    ("building_aqueduct", false),
    ("building_aqueduct_lake", false),
    ("building_aqueduct_river", false),
    ("building_bank", false),
    ("building_barracks", false),
    ("building_barracks_ii", false),
    ("building_barracks_iii", false),
    ("building_cathedral", true),
    ("building_city_walls", false),
    ("building_coastal_defense", false),
    ("building_amphitheater", false),
    ("building_courthouse", false),
    ("building_factory", false),
    ("building_granary", false),
    ("building_harbor", false),
    ("building_hydro_plant", false),
    ("building_library", true),
    ("building_marketplace", false),
    ("building_mass_transit", false),
    ("building_mfg_plant", false),
    ("building_nuclear_plant", false),
    ("building_offshore_platform", false),
    ("building_palace", true),
    ("building_ecclesiastical_palace", false),
    ("building_police_station", false),
    ("building_port_facility", false),
    ("building_power_plant", false),
    ("building_recycling_center", false),
    ("building_research_lab", true),
    ("building_sam_battery", false),
    ("building_sdi_defense", true),
    ("building_sewer_system", false),
    ("building_solar_plant", false),
    ("building_space_component", false),
    ("building_space_module", false),
    ("building_space_structural", false),
    ("building_stock_exchange", false),
    ("building_super_highways", false),
    ("building_supermarket", false),
    ("building_temple", true),
    ("building_university", true),
    ("building_apollo_program", true),
    ("building_a_smith_s_trading_co", true),
    ("building_colossus", true),
    ("building_copernicus_observatory", true),
    ("building_cure_for_cancer", true),
    ("building_darwin_s_voyage", false),
    ("building_eiffel_tower", false),
    ("building_great_library", true),
    ("building_great_wall", true),
    ("building_hanging_gardens", true),
    ("building_hoover_dam", true),
    ("building_isaac_newton_s_college", true),
    ("building_j_s_bach_s_cathedral", true),
    ("building_king_richard_s_crusade", false),
    ("building_leonardo_s_workshop", true),
    ("building_lighthouse", true),
    ("building_magellan_s_expedition", true),
    ("building_manhattan_project", true),
    ("building_marco_polo_s_embassy", false),
    ("building_michelangelo_s_chapel", false),
    ("building_mausoleum_of_mausolos", true),
    ("building_statue_of_zeus", true),
    ("building_temple_of_artemis", true),
    ("building_pyramids", true),
    ("building_internet", true),
    ("building_shakespeare_s_theater", true),
    ("building_statue_of_liberty", false),
    ("building_sun_tzu_s_war_academy", true),
    ("building_united_nations", true),
    ("building_women_s_suffrage", false),
    ("building_coinage", false),
    ("building_colosseum", true),
    ("building_hospital", false),
    ("building_longevity", true),
    ("building_oracle", true),
    ("building_theory_of_evolution", true),
    ("building_universal_suffrage", true),
    ("building_seti_program", true),
    ("building_forbidden_palace", true),
    ("building_heroic_epic", true),
    ("building_iron_works", true),
    ("building_military_academy", true),
    ("building_pentagon", true),
    ("building_wall_street", true),
    ("building_intelligence_agency", true),
    ("building_battlefield_medicine", true),
    ("building_sistine_chapel", true),
    ("building_knights_templar", true),
    ("building_secret_police_hq", true),
];

pub(crate) fn building_index(id: &str) -> Option<usize> {
    BUILDINGS.iter().position(|(candidate, _)| *candidate == id)
}

pub(crate) fn uses_selected_obsolescence(index: usize) -> bool {
    BUILDINGS.get(index).is_some_and(|(_, selected)| *selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_obsolescence_policy_preserves_pre_removal_behavior() {
        let expected: Vec<String> = serde_json::from_str(include_str!(
            "../../tests/fixtures/building_obsolescence_policy_v1.json"
        ))
        .unwrap();
        let actual: Vec<_> = BUILDINGS
            .iter()
            .enumerate()
            .filter(|(index, _)| uses_selected_obsolescence(*index))
            .map(|(_, (id, _))| *id)
            .collect();
        assert_eq!(actual, expected);
    }
}
