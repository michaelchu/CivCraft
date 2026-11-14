//! Editable Conquests campaign starting-unit profiles.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub(super) struct StartProfile {
    civilization: &'static str,
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
        let mut lines =
            include_str!("../../data/rulesets/civ3_conquests/civilization_starts.tsv").lines();
        let header = lines.next().expect("Civ III start table has a header");
        let expected_columns = header.split('\t').count();
        let mut parsed = Vec::new();
        for (line_number, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                expected_columns,
                "invalid Civ III start profile row {}",
                line_number + 2
            );
            let count = |column: usize| {
                fields[column].parse::<usize>().unwrap_or_else(|error| {
                    panic!("invalid start count on row {}: {error}", line_number + 2)
                })
            };
            parsed.push(StartProfile {
                civilization: fields[1],
                starting_unit_1: fields[5],
                starting_unit_1_count: count(6),
                starting_unit_2: fields[7],
                starting_unit_2_count: count(8),
                expansionist: fields[3] == "true",
                militaristic: fields[4] == "true",
                expansionist_bonus_unit: fields[9],
                bonus_count: count(10),
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

pub(super) fn get(civilization: &str) -> Option<&'static StartProfile> {
    let civilization = crate::rules::civilizations::by_name(civilization)
        .map_or(civilization, |civ| civ.noun.as_str());
    let source_name = match civilization {
        // The runtime's display labels are singular; Conquests BIQ Race names
        // (and the profile table) use these source plural/demonym forms.
        "Germany" => "Germans",
        "Russia" => "Russians",
        "America" => "Americans",
        "France" => "French",
        "India" => "Indians",
        "Persia" => "Persians",
        "Zululand" => "Zulus",
        "England" => "English",
        "Zulu" => "Zulus",
        "Maya" => "Mayans",
        name => name,
    };
    profiles()
        .iter()
        .find(|profile| profile.civilization == source_name)
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
    fn display_civilization_labels_resolve_to_conquests_race_names() {
        for (display, source) in [
            ("Germany", "Germans"),
            ("Russia", "Russians"),
            ("America", "Americans"),
            ("France", "French"),
            ("India", "Indians"),
            ("Persia", "Persians"),
            ("Zululand", "Zulus"),
            ("England", "English"),
        ] {
            assert_eq!(get(display).unwrap().civilization, source);
        }
    }

    #[test]
    fn new_campaign_starting_units_use_the_source_profiles() {
        let game = Simulation::new(crate::mapgen::generate(42));
        for owner in 0..game.civilization_count {
            let profile = get(&game.players[owner].name).unwrap();
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
                .then(|| UnitKind::from_civ3_name(profile.expansionist_bonus_unit).unwrap());
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
