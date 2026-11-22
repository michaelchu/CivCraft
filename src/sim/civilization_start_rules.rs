//! Editable Conquests campaign starting-unit profiles from native TOML.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub(super) struct StartProfile {
    civilization_id: &'static str,
    starting_unit_1: &'static str,
    starting_unit_1_count: usize,
    starting_unit_2: &'static str,
    starting_unit_2_count: usize,
    expansionist: bool,
    #[allow(dead_code)] // Retained from the source profile for future leader AI behavior.
    militaristic: bool,
    expansionist_bonus_unit: &'static str,
    bonus_count: usize,
}

impl StartProfile {
    pub(super) fn units(&self) -> Vec<(&'static str, usize)> {
        let mut units = Vec::with_capacity(3);
        if self.starting_unit_1_count > 0 {
            units.push((self.starting_unit_1, self.starting_unit_1_count));
        }
        if self.starting_unit_2_count > 0 {
            units.push((self.starting_unit_2, self.starting_unit_2_count));
        }
        if self.expansionist && self.bonus_count > 0 {
            units.push((self.expansionist_bonus_unit, self.bonus_count));
        }
        units
    }
}

fn profiles() -> &'static [StartProfile] {
    static PROFILES: OnceLock<Vec<StartProfile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        let mut parsed = Vec::new();
        for civilization in &crate::rules::native::civilizations().civilizations {
            assert_eq!(civilization.starting_units.len(), 2);
            let bonus = civilization.bonus_units.first();
            parsed.push(StartProfile {
                civilization_id: leak(&civilization.id),
                starting_unit_1: leak(&civilization.starting_units[0].id),
                starting_unit_1_count: civilization.starting_units[0].count as usize,
                starting_unit_2: leak(&civilization.starting_units[1].id),
                starting_unit_2_count: civilization.starting_units[1].count as usize,
                expansionist: civilization.expansionist,
                militaristic: civilization.militaristic,
                expansionist_bonus_unit: bonus.map_or("", |unit| leak(&unit.id)),
                bonus_count: bonus.map_or(0, |unit| unit.count as usize),
            });
        }
        assert_eq!(
            parsed.len(),
            31,
            "Conquests civilization start coverage changed"
        );
        parsed
    })
}

fn leak(value: &str) -> &'static str {
    Box::leak(value.to_owned().into_boxed_str())
}

pub(super) fn get(civilization_id: &str) -> Option<&'static StartProfile> {
    profiles()
        .iter()
        .find(|profile| profile.civilization_id == civilization_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Simulation, UnitKind};

    #[test]
    fn all_conquests_start_profiles_include_settler_worker_and_expansionist_scout() {
        let profiles = profiles();
        assert_eq!(profiles.len(), 31);
        for profile in profiles {
            assert_eq!(profile.starting_unit_1, "Settler");
            assert_eq!(profile.starting_unit_1_count, 1);
            assert_eq!(profile.starting_unit_2, "Worker");
            assert_eq!(profile.starting_unit_2_count, 1);
            if profile.expansionist {
                assert_eq!(profile.bonus_count, 1);
                assert!(matches!(
                    profile.expansionist_bonus_unit,
                    "Scout" | "Chasqui Scout"
                ));
                assert_eq!(profile.units().len(), 3);
            } else {
                assert_eq!(profile.units().len(), 2);
            }
        }
    }

    #[test]
    fn every_canonical_civilization_id_has_a_start_profile() {
        for civilization in crate::rules::civilizations::all() {
            assert_eq!(
                get(&civilization.id).unwrap().civilization_id,
                civilization.id
            );
        }
        assert!(get("Crimson League").is_none());
    }

    #[test]
    fn native_toml_starting_profiles_match_the_legacy_extraction() {
        let native = crate::rules::native::civilizations();
        for profile in profiles() {
            let migrated = native
                .civilizations
                .iter()
                .find(|civilization| civilization.id == profile.civilization_id)
                .unwrap_or_else(|| panic!("native TOML is missing {}", profile.civilization_id));
            assert_eq!(migrated.expansionist, profile.expansionist);
            assert_eq!(migrated.militaristic, profile.militaristic);
            assert_eq!(
                migrated
                    .starting_units
                    .iter()
                    .map(|stack| (stack.id.as_str(), stack.count as usize))
                    .collect::<Vec<_>>(),
                vec![
                    (profile.starting_unit_1, profile.starting_unit_1_count),
                    (profile.starting_unit_2, profile.starting_unit_2_count),
                ]
            );
            let expected_bonus = if profile.expansionist && profile.bonus_count > 0 {
                vec![(profile.expansionist_bonus_unit, profile.bonus_count)]
            } else {
                Vec::new()
            };
            assert_eq!(
                migrated
                    .bonus_units
                    .iter()
                    .map(|stack| (stack.id.as_str(), stack.count as usize))
                    .collect::<Vec<_>>(),
                expected_bonus
            );
        }
    }

    #[test]
    fn new_campaign_starting_units_use_the_source_profiles() {
        let game = Simulation::new(crate::mapgen::generate(42));
        for owner in 0..game.civilization_count {
            let profile = get(game.players[owner].civilization_id.as_deref().unwrap()).unwrap();
            let owner_units = game
                .units
                .iter()
                .filter(|unit| unit.owner == owner)
                .collect::<Vec<_>>();
            assert_eq!(
                owner_units.len(),
                profile
                    .units()
                    .iter()
                    .map(|(_, count)| count)
                    .sum::<usize>()
            );
            assert_eq!(
                owner_units
                    .iter()
                    .filter(|unit| unit.kind == UnitKind::Settler)
                    .count(),
                1
            );
            assert_eq!(
                owner_units
                    .iter()
                    .filter(|unit| unit.kind == UnitKind::Worker)
                    .count(),
                1
            );
            let bonus_unit = profile
                .expansionist
                .then(|| UnitKind::from_catalog_name(profile.expansionist_bonus_unit).unwrap());
            assert_eq!(
                owner_units
                    .iter()
                    .filter(|unit| Some(unit.kind) == bonus_unit)
                    .count(),
                profile.bonus_count
            );
            assert!(
                owner_units
                    .iter()
                    .all(|unit| unit.kind != UnitKind::Explorer)
            );
        }
    }
}
