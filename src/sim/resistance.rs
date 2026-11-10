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
    continued_chance: i32,
}

struct ResistanceRules {
    bands: [ResistanceBand; 6],
    government_names: Vec<String>,
    modifiers: Vec<Vec<i32>>,
    military_law: i32,
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
                continued_chance: fields[6].parse().unwrap(),
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

        let mut lines = include_str!("../../data/rulesets/civ3_conquests/resistance_military_law.tsv").lines();
        assert_eq!(
            lines.next(),
            Some("source_section\tsource_record_index\tdifficulty\tmilitary_law\tbiq_sha256")
        );
        let military_law_values = lines
            .enumerate()
            .map(|(index, line)| {
                let fields = line.split('\t').collect::<Vec<_>>();
                assert_eq!(fields.len(), 5);
                assert_eq!(fields[0], "DIFF");
                assert_eq!(fields[1].parse::<usize>().unwrap(), index);
                assert!(!fields[2].is_empty());
                assert!(hash.replace(fields[4]).is_none_or(|old| old == fields[4]));
                fields[3].parse::<i32>().unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(military_law_values.len(), 8);
        assert!(military_law_values[0] > 0);
        assert!(military_law_values.iter().all(|&value| value == military_law_values[0]));
        ResistanceRules {
            bands: bands.try_into().unwrap(),
            government_names: names,
            modifiers: matrix,
            military_law: military_law_values[0],
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
    resistance_chance(
        captor_culture,
        citizen_culture,
        captor_government,
        citizen_government,
        false,
    )
}

fn continued_resistance_chance(
    captor_culture: i64,
    citizen_culture: i64,
    captor_government: Government,
    citizen_government: Government,
) -> i32 {
    resistance_chance(
        captor_culture,
        citizen_culture,
        captor_government,
        citizen_government,
        true,
    )
}

fn resistance_chance(
    captor_culture: i64,
    citizen_culture: i64,
    captor_government: Government,
    citizen_government: Government,
    continued: bool,
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
    let base_chance = if continued {
        band.continued_chance
    } else {
        band.initial_chance
    };
    (base_chance + rules.modifiers[captor_index][citizen_index]).clamp(0, 100)
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

    pub(super) fn resolve_resistance_for_owner(&mut self, owner: usize) {
        let culture_totals = (0..self.players.len())
            .map(|civilization| self.civilization_culture(civilization))
            .collect::<Vec<_>>();
        let captor_culture = culture_totals.get(owner).copied().unwrap_or(0);
        let captor_government = self.players[owner].government;
        let city_ids = self
            .cities
            .iter()
            .filter(|city| city.owner == owner && !city.resisting_citizens.is_empty())
            .map(|city| city.id)
            .collect::<Vec<_>>();
        for city_id in city_ids {
            let Some(city_index) = self.city_index_by_id(city_id) else {
                continue;
            };
            let city = &self.cities[city_index];
            let resisting_count = city.resisting_citizens.iter().map(|&(_, n)| n).sum::<i32>();
            let eligible_units = self
                .units
                .iter()
                .filter(|unit| {
                    unit.owner == owner
                        && unit.transport.is_none()
                        && (unit.x, unit.y) == (city.x, city.y)
                        && unit.kind.unit_class().is_ground()
                        && unit.kind.is_military()
                        && !unit.kind.is_civilian()
                })
                .count() as i32;
            let mut quelling_capacity =
                (eligible_units * resistance_rules().military_law).min(resisting_count);
            // The source gives a per-turn cap but no nationality tie-breaker;
            // stored order keeps this inferred selection stable.
            let defenders = city.resisting_citizens.clone();
            let mut remaining = Vec::new();
            for (nationality, population) in defenders {
                let chance = continued_resistance_chance(
                    captor_culture,
                    culture_totals.get(nationality).copied().unwrap_or(0),
                    captor_government,
                    self.players[nationality].government,
                );
                let would_continue = (0..population)
                    .filter(|_| self.rand_below(100) < chance)
                    .count() as i32;
                let eligible_to_quell = population - would_continue;
                let quelled = eligible_to_quell.min(quelling_capacity);
                quelling_capacity -= quelled;
                let still_resisting = population - quelled;
                if still_resisting > 0 {
                    remaining.push((nationality, still_resisting));
                }
            }
            self.cities[city_index].resisting_citizens = remaining;
        }
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
        assert_eq!(rules.military_law, 1);
        assert_eq!(
            rules.bands.map(|band| band.continued_chance),
            [30, 40, 50, 60, 70, 80]
        );
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

    #[test]
    fn continued_resistance_uses_biq_chances_and_the_same_government_modifier() {
        assert_eq!(
            continued_resistance_chance(300, 100, Government::Republic, Government::Republic),
            30
        );
        assert_eq!(
            continued_resistance_chance(100, 100, Government::Republic, Government::Monarchy),
            55
        );
    }

    #[test]
    fn resistance_cannot_end_without_a_qualifying_garrison() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(HUMAN, 10, 24);
        game.cities[0].size = 4;
        game.cities[0].foreign_citizens = vec![(1, 3)];
        game.cities[0].resisting_citizens = vec![(1, 3)];

        game.resolve_resistance_for_owner(HUMAN);

        assert_eq!(game.cities[0].resisting_citizens, [(1, 3)]);
    }

    #[test]
    fn one_ground_combat_unit_quells_at_most_one_resistor_per_turn() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(HUMAN, 10, 24);
        game.create_city(1, 30, 24);
        game.cities[0].size = 11;
        game.cities[0].foreign_citizens = vec![(1, 10)];
        game.cities[0].resisting_citizens = vec![(1, 10)];
        game.cities[0].add_culture(HUMAN, 300);
        game.cities[1].add_culture(1, 1);
        game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 24);
        game.spawn_unit(HUMAN, UnitKind::Worker, 10, 24);
        game.spawn_unit(HUMAN, UnitKind::Artillery, 10, 24);
        game.spawn_unit(HUMAN, UnitKind::Fighter, 10, 24);
        game.spawn_unit(HUMAN, UnitKind::Transport, 10, 24);
        game.set_random_seed(42);

        game.process_player_cities(HUMAN);

        let remaining: i32 = game.cities[0]
            .resisting_citizens
            .iter()
            .map(|&(_, count)| count)
            .sum();
        assert_eq!(remaining, 9);
    }
}
