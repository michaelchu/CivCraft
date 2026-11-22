//! Catalog schema, coverage and parity tests.

#[test]
fn parse_buildings_and_units_reject_unknown_technology_ids() {
    let buildings_src = include_str!("../../../data/rulesets/civcraft/content/buildings.toml");
    let bad_buildings = buildings_src.replace("TECH_Flight", "TECH_Missing_Flight");
    let err = super::parse_buildings(&bad_buildings, super::technologies()).unwrap_err();
    assert!(err.contains("building_airport"));

    let units_src = include_str!("../../../data/rulesets/civcraft/content/units.toml");
    let bad_units = units_src.replace("TECH_Bronze_Working", "TECH_Missing_Bronze");
    let err = super::parse_units(&bad_units, super::technologies()).unwrap_err();
    assert!(err.contains("unknown technology ID"));
}

use super::*;

#[test]
fn village_weights_preserve_original_rolls_and_reject_invalid_tables() {
    let villages = &settlements().villages;
    assert_eq!(villages.total_weight(), 14);
    let labels: Vec<_> = (0..14)
        .map(|roll| match villages.reward_at(roll) {
            NativeVillageReward::Gold { amount } => format!("gold:{amount}"),
            NativeVillageReward::Technology => "technology".into(),
            NativeVillageReward::Reinforcements => "reinforcements".into(),
            NativeVillageReward::Barbarians => "barbarians".into(),
            NativeVillageReward::Settlement => "settlement".into(),
            NativeVillageReward::Map => "map".into(),
        })
        .collect();
    assert_eq!(
        labels,
        [
            "gold:25",
            "gold:50",
            "gold:50",
            "gold:50",
            "gold:100",
            "technology",
            "technology",
            "technology",
            "reinforcements",
            "reinforcements",
            "barbarians",
            "settlement",
            "map",
            "map"
        ]
    );
    for weights in [vec![], vec![0], vec![-1], vec![i32::MAX, 1]] {
        let invalid = NativeVillageRules {
            barbarian_onset_turn: 60,
            map_reveal_radius_squared: 30,
            outcomes: weights
                .into_iter()
                .map(|weight| NativeVillageOutcome {
                    weight,
                    reward: NativeVillageReward::Map,
                })
                .collect(),
        };
        assert!(invalid.validate().is_err());
    }
}

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
    assert_eq!(buildings().rules.len(), 91);
    assert_eq!(technologies().schema_version, 1);
    assert_eq!(technologies().source, "Civilization III: Conquests");
    assert_eq!(technologies().technologies.len(), 85);
    assert_eq!(governments().governments.len(), 8);
    assert!(
        governments()
            .governments
            .iter()
            .all(|government| { !government.government.is_empty() && !government.name.is_empty() })
    );
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
        "TECH_Monarchy"
    );
    assert_eq!(
        governments().get("republic").unwrap().required_technology,
        "TECH_The_Republic"
    );
    assert_eq!(economy().resources.len(), 20);
    assert_eq!(experience().schema_version, 1);
    assert_eq!(experience().levels.len(), 4);
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
fn selected_obsolescence_policy_preserves_pre_removal_behavior() {
    let expected: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/building_obsolescence_policy_v1.json"
    ))
    .unwrap();
    let catalog = buildings();
    let actual: Vec<_> = catalog
        .rules
        .iter()
        .enumerate()
        .filter(|(index, rule)| {
            assert_eq!(*index, rule.legacy_runtime_index);
            rule.selected_obsolescence
        })
        .map(|(_, rule)| rule.id.as_str())
        .collect();
    assert_eq!(actual, expected);
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

#[test]
fn unified_content_ruleset_loads_all_campaign_toml_files_from_directory() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/rulesets/civcraft/content");
    let loaded = ContentRuleset::load_from_directory(&dir)
        .expect("all 6 campaign content TOML files must load and validate");
    assert_eq!(loaded.units.units.len(), units().units.len());
    assert_eq!(loaded.buildings.rules.len(), buildings().rules.len());
    assert_eq!(
        loaded.technologies.technologies.len(),
        technologies().technologies.len()
    );
    assert_eq!(loaded.religion.faiths.len(), religion().faiths.len());
}

#[test]
fn indexed_lookups_match_linear_scans() {
    use crate::rules::Tech;
    for index in 0..Tech::storage_len() + 4 {
        assert_eq!(
            Tech::from_index(index),
            Tech::all().find(|tech| tech.0 == index),
            "tech index {index}"
        );
    }
    let rules = &buildings().rules;
    for index in 0..rules.len() + 2 {
        let linear = rules.iter().find(|rule| rule.legacy_runtime_index == index);
        assert_eq!(
            building_rule(index).map(|rule| &rule.id),
            linear.map(|rule| &rule.id)
        );
    }
}
