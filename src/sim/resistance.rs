//! Captured-city resistance initiation from Conquests CULT/GOVT data.
//!
//! Culture-band and base-government formulas are reverse-engineered from
//! CivFanatics' account credited to Firaxis developer Mike B. Conquests BIQ
//! values supply the inputs; applying its added Fascism/Feudalism matrix
//! entries follows the same inferred percentage-point interpretation.

use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
struct ResistanceBand {
    numerator: i64,
    denominator: i64,
    initial_chance: i32,
}

struct ResistanceRules {
    bands: [ResistanceBand; 6],
    government_names: Vec<String>,
    modifiers: Vec<Vec<i32>>,
}

fn resistance_rules() -> &'static ResistanceRules {
    static RULES: OnceLock<ResistanceRules> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut lines = include_str!("../../data/rulesets/civ3_conquests/culture_resistance_bands.tsv").lines();
        assert_eq!(
            lines.next(),
            Some("source_section\tsource_record_index\tname\tculture_ratio_numerator\tculture_ratio_denominator\tinitial_chance\tcontinued_chance\tbiq_sha256")
        );
        let mut bands = Vec::new();
        let mut hash = None;
        for (index, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 8);
            assert_eq!(fields[0], "CULT");
            assert_eq!(fields[1].parse::<usize>().unwrap(), index);
            let numerator = fields[3].parse::<i64>().unwrap();
            let denominator = fields[4].parse::<i64>().unwrap();
            assert!(numerator > 0 && denominator > 0);
            bands.push(ResistanceBand {
                numerator,
                denominator,
                initial_chance: fields[5].parse().unwrap(),
            });
            assert!(hash.replace(fields[7]).is_none_or(|old| old == fields[7]));
        }
        assert_eq!(bands.len(), 6);

        let mut lines = include_str!("../../data/rulesets/civ3_conquests/government_resistance.tsv").lines();
        assert_eq!(
            lines.next(),
            Some("captor_biq_index\tcaptor_name\tdefender_biq_index\tdefender_name\tmodifier\tbiq_sha256")
        );
        let mut names = Vec::new();
        let mut matrix = vec![vec![0; 8]; 8];
        let mut seen = std::collections::BTreeSet::new();
        for line in lines {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 6);
            let captor = fields[0].parse::<usize>().unwrap();
            let defender = fields[2].parse::<usize>().unwrap();
            assert!(captor < 8 && defender < 8);
            assert!(seen.insert((captor, defender)));
            if defender == 0 {
                assert_eq!(captor, names.len());
                names.push(fields[1].to_owned());
            }
            assert_eq!(fields[3], names.get(defender).map(String::as_str).unwrap_or(fields[3]));
            matrix[captor][defender] = fields[4].parse().unwrap();
            assert!(hash.replace(fields[5]).is_none_or(|old| old == fields[5]));
        }
        assert_eq!(seen.len(), 64);
        assert_eq!(names.len(), 8);
        assert_eq!(hash.as_deref(), Some("1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c"));
        ResistanceRules {
            bands: bands.try_into().unwrap(),
            government_names: names,
            modifiers: matrix,
        }
    })
}

fn government_biq_index(government: Government) -> usize {
    resistance_rules()
        .government_names
        .iter()
        .position(|name| name == government.name())
        .unwrap_or_else(|| {
            panic!(
                "government missing from Conquests BIQ: {}",
                government.name()
            )
        })
}

fn initial_resistance_chance(
    captor_culture: i64,
    citizen_culture: i64,
    captor_government: Government,
    citizen_government: Government,
) -> i32 {
    assert!(captor_culture >= 0 && citizen_culture >= 0);
    let rules = resistance_rules();
    // The community reverse-engineering account specifies this zero-culture
    // edge case: a cultureless captor is treated as disdainful of, even at 0:0.
    let band = if captor_culture == 0 {
        // The article explicitly calls out this edge case, including 0:0.
        rules.bands[5]
    } else {
        rules
            .bands
            .iter()
            .find(|band| {
                (captor_culture as i128) * band.denominator as i128
                    >= (citizen_culture as i128) * band.numerator as i128
            })
            .copied()
            .unwrap_or(rules.bands[5])
    };
    let captor_index = government_biq_index(captor_government);
    let citizen_index = government_biq_index(citizen_government);
    let _source_ratio = (band.numerator, band.denominator);
    (band.initial_chance + rules.modifiers[captor_index][citizen_index]).clamp(0, 100)
}

impl Simulation {
    pub(super) fn initialize_capture_resistance(
        &mut self,
        city_index: usize,
        culture_at_capture: &[i64],
    ) {
        let captor = self.cities[city_index].owner;
        let captor_culture = culture_at_capture.get(captor).copied().unwrap_or(0);
        let captor_government = self.players[captor].government;
        let foreign = self.cities[city_index].foreign_citizens.clone();
        let mut resisting = Vec::new();
        for (nationality, population) in foreign {
            let chance = initial_resistance_chance(
                captor_culture,
                culture_at_capture.get(nationality).copied().unwrap_or(0),
                captor_government,
                self.players[nationality].government,
            );
            let count = (0..population)
                .filter(|_| self.rand_below(100) < chance)
                .count() as i32;
            if count > 0 {
                resisting.push((nationality, count));
            }
        }
        self.cities[city_index].resisting_citizens = resisting;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biq_resistance_inputs_cover_all_ratio_bands_and_government_pairs() {
        let rules = resistance_rules();
        assert_eq!(
            rules.bands.map(|band| band.initial_chance),
            [40, 50, 60, 70, 80, 90]
        );
        assert_eq!(
            rules.bands.map(|band| (band.numerator, band.denominator)),
            [(3, 1), (2, 1), (1, 1), (3, 4), (1, 2), (1, 3)]
        );
        assert_eq!(rules.modifiers.len(), 8);
        assert_eq!(
            rules
                .government_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "Anarchy",
                "Despotism",
                "Monarchy",
                "Communism",
                "Republic",
                "Democracy",
                "Fascism",
                "Feudalism",
            ]
        );
        assert_eq!(rules.government_names.len(), 8);
    }

    #[test]
    fn culture_ratio_edges_select_the_reverse_engineered_resistance_bands() {
        let chance = |captor, defender| {
            initial_resistance_chance(captor, defender, Government::Republic, Government::Republic)
        };
        assert_eq!(chance(300, 100), 40);
        assert_eq!(chance(200, 100), 50);
        assert_eq!(chance(100, 100), 60);
        assert_eq!(chance(75, 100), 70);
        assert_eq!(chance(50, 100), 80);
        assert_eq!(chance(40, 100), 90);
        assert_eq!(chance(33, 100), 90);
        assert_eq!(chance(0, 0), 90);
        assert_eq!(chance(100, 0), 40);
    }

    #[test]
    fn biq_government_matrix_supplies_percentage_point_adjustments() {
        assert_eq!(
            initial_resistance_chance(100, 100, Government::Republic, Government::Monarchy),
            65
        );
        assert_eq!(
            initial_resistance_chance(100, 100, Government::Democracy, Government::Communism),
            65
        );
        assert_eq!(
            initial_resistance_chance(100, 100, Government::Fascism, Government::Communism),
            50
        );
        assert_eq!(
            initial_resistance_chance(100, 100, Government::Feudalism, Government::Fascism),
            65
        );
    }
}
