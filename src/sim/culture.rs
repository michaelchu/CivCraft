//! Conquests city culture accumulation from the installed BIQ's BLDG records.

use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
struct BuildingCulture {
    catalog_index: usize,
    culture_per_turn: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CultureVictoryThresholds {
    map_size: &'static str,
    one_city: i64,
    civilization: i64,
}

pub(super) fn retain_culture_on_capture() -> bool {
    static RETAIN: OnceLock<bool> = OnceLock::new();
    *RETAIN.get_or_init(|| {
        let mut lines = include_str!(
            "../../data/rulesets/civ3_conquests/culture_capture_settings.tsv"
        )
        .lines();
        assert_eq!(
            lines.next(),
            Some("biq_path\tsource_section\tsource_record_index\tretain_culture_on_capture_raw\tretain_culture_on_capture\tbiq_sha256"),
            "invalid Conquests capture-culture table header"
        );
        let mut matches = lines.filter_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 6, "invalid Conquests capture-culture row");
            (fields[0] == "conquests.biq").then_some(fields)
        });
        let fields = matches
            .next()
            .expect("missing main Conquests BIQ capture-culture setting");
        assert!(
            matches.next().is_none(),
            "duplicate main Conquests BIQ capture-culture setting"
        );
        assert_eq!(fields[1], "GAME");
        assert_eq!(fields[2], "0");
        assert_eq!(
            fields[5],
            "1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c"
        );
        let raw = fields[3].parse::<u8>().expect("invalid RetainCulture byte");
        assert!(raw <= 1, "unexpected RetainCulture value");
        assert_eq!(fields[4], if raw == 1 { "true" } else { "false" });
        raw == 1
    })
}

fn building_culture_rules() -> &'static [BuildingCulture] {
    static RULES: OnceLock<Vec<BuildingCulture>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut lines = include_str!("../../data/rulesets/civ3_conquests/culture_buildings.tsv").lines();
        assert_eq!(
            lines.next(),
            Some("biq_section\tsource_record_index\tbiq_name\tculture_per_turn\tcivcraft_catalog_index\tmapping_status\tbiq_sha256"),
            "invalid Conquests culture building table header"
        );
        let mut rules = Vec::new();
        let mut indices = std::collections::BTreeSet::new();
        let mut hashes = std::collections::BTreeSet::new();
        let mut unresolved_nonzero = Vec::new();
        for (offset, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 7, "invalid Conquests culture row {}", offset + 2);
            assert_eq!(fields[0], "BLDG", "unexpected culture source section");
            let source_index = fields[1].parse::<usize>().expect("invalid BLDG index");
            assert!(indices.insert(source_index), "duplicate BLDG index {source_index}");
            assert!(!fields[2].is_empty(), "empty BIQ building name");
            let culture = fields[3].parse::<i32>().expect("invalid culture rate");
            assert!(culture >= 0, "negative BIQ culture rate");
            assert_eq!(fields[6].len(), 64, "invalid BIQ hash on row {}", offset + 2);
            hashes.insert(fields[6]);
            match fields[5] {
                "mapped" => {
                    let catalog_index = fields[4]
                        .parse::<usize>()
                        .expect("mapped culture row requires catalog ID");
                    assert!(catalog_index < Building::all().count(), "unknown Building catalog ID");
                    assert!(culture > 0, "mapped culture row must contribute culture");
                    rules.push(BuildingCulture { catalog_index, culture_per_turn: culture });
                }
                "unresolved" => {
                    assert_eq!(fields[4], "-", "unresolved culture row has catalog ID");
                    assert!(culture > 0, "zero culture row cannot be unresolved");
                    unresolved_nonzero.push(fields[2]);
                }
                "zero-output; catalog mapping not needed" => {
                    assert_eq!(culture, 0, "zero-output status has positive culture");
                    assert_eq!(fields[4], "-", "zero-output row should not need catalog ID");
                }
                status => panic!("unknown culture mapping status {status:?}"),
            }
        }
        assert_eq!(indices, (0..83).collect(), "culture table must cover every Conquests BLDG record");
        assert_eq!(hashes.len(), 1, "culture table mixes BIQ versions");
        assert!(unresolved_nonzero.is_empty(), "unmapped culture buildings: {unresolved_nonzero:?}");
        assert_eq!(rules.len(), 46, "unexpected mapped culture building count");
        rules
    })
}

pub(super) fn building_culture_per_turn(building: Building) -> Option<i32> {
    building_culture_rules()
        .iter()
        .find(|rule| rule.catalog_index == building.0)
        .map(|rule| rule.culture_per_turn)
}

pub(super) fn prepare_culture_buildings_for_capture(city: &mut City, old_owner: usize, year: i32) {
    for building in city.buildings.clone() {
        if building_culture_per_turn(building).is_none() {
            continue;
        }
        city.ensure_culture_building_owner(building, old_owner);
        city.reset_culture_building_age(building, year);
    }
}

fn culture_victory_thresholds() -> &'static [CultureVictoryThresholds] {
    static THRESHOLDS: OnceLock<Vec<CultureVictoryThresholds>> = OnceLock::new();
    THRESHOLDS.get_or_init(|| {
        let mut lines =
            include_str!("../../data/rulesets/civ3_conquests/culture_victory.tsv").lines();
        assert_eq!(
            lines.next(),
            Some(
                "map_size\tone_city_culture\tcivilization_culture\tcivilopedia_source\tbiq_sha256"
            ),
            "invalid Conquests culture victory table header"
        );
        let expected = ["Tiny", "Small", "Standard", "Large", "Huge"];
        let mut result = Vec::new();
        let mut hashes = std::collections::BTreeSet::new();
        for (index, line) in lines.enumerate() {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 5, "invalid culture victory row {}", index + 2);
            assert!(index < expected.len(), "too many culture victory rows");
            assert_eq!(fields[0], expected[index], "unexpected map-size row order");
            assert_eq!(fields[3], "Conquests/Text/Civilopedia.txt#GCON_Victory");
            let one_city = fields[1]
                .parse::<i64>()
                .expect("invalid one-city threshold");
            let civilization = fields[2]
                .parse::<i64>()
                .expect("invalid civilization threshold");
            assert!(one_city > 0 && civilization >= one_city);
            assert_eq!(
                fields[4].len(),
                64,
                "invalid BIQ hash in culture victory row"
            );
            hashes.insert(fields[4]);
            result.push(CultureVictoryThresholds {
                map_size: expected[index],
                one_city,
                civilization,
            });
        }
        assert_eq!(
            result.len(),
            expected.len(),
            "missing map-size victory thresholds"
        );
        assert_eq!(hashes.len(), 1, "culture victory table mixes BIQ versions");
        assert_eq!(result[2].one_city, 20_000);
        assert_eq!(result[2].civilization, 100_000);
        result
    })
}

fn culture_victory_thresholds_for_map(map_wrap: MapWrap) -> Option<CultureVictoryThresholds> {
    let name = match (map_wrap.width(), map_wrap.height()) {
        (60, 60) => "Tiny",
        (80, 80) => "Small",
        (100, 100) => "Standard",
        (130, 130) => "Large",
        (160, 160) => "Huge",
        _ => return None,
    };
    culture_victory_thresholds()
        .iter()
        .copied()
        .find(|threshold| threshold.map_size == name)
}

impl Simulation {
    pub(crate) fn civilization_culture(&self, civilization: usize) -> i64 {
        self.cities
            .iter()
            .filter(|city| city.owner == civilization)
            .map(|city| city.culture_for_civilization(civilization))
            .sum()
    }

    pub(super) fn record_city_culture(&mut self, city_index: usize) {
        let current_year = self.year;
        let owner = self.cities[city_index].owner;
        let domestic = self.cities[city_index].domestic_citizens();
        if self.cities[city_index].culture_suppressed_until_majority_assimilated
            && domestic * 2 > self.cities[city_index].size
        {
            self.cities[city_index].culture_suppressed_until_majority_assimilated = false;
        }
        if self.players[owner].government == Government::Fascism
            && self.cities[city_index].culture_suppressed_until_majority_assimilated
        {
            return;
        }
        let buildings = self.cities[city_index].buildings.clone();
        let mut culture_this_turn = 0_i64;
        for building in buildings {
            let Some(rate) = building_culture_per_turn(building) else {
                continue;
            };
            if self.cities[city_index].ensure_culture_building_owner(building, owner) != owner {
                continue;
            }
            let built_year = self.cities[city_index].culture_building_year(building, current_year);
            let age_multiplier = if current_year.saturating_sub(built_year) >= 1000 {
                2
            } else {
                1
            };
            culture_this_turn += i64::from(rate) * age_multiplier;
        }
        self.cities[city_index].add_culture(owner, culture_this_turn);
    }

    pub(super) fn check_cultural_victory(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        let Some(thresholds) = culture_victory_thresholds_for_map(self.map_wrap) else {
            return;
        };
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.player_alive(owner) {
                continue;
            }
            let city_winner = self.cities.iter().any(|city| {
                city.owner == owner && city.culture_for_civilization(owner) >= thresholds.one_city
            });
            let civilization_total = self.civilization_culture(owner);
            let civilization_winner = civilization_total >= thresholds.civilization
                && (0..self.players.len()).all(|rival| {
                    rival == owner
                        || rival == BARBARIANS
                        || !self.player_alive(rival)
                        || self.civilization_culture(rival) <= civilization_total / 2
                });
            if city_winner || civilization_winner {
                let kind = if city_winner { "city" } else { "civilization" };
                self.game_over = Some(format!(
                    "{} wins a cultural victory through {kind} culture.",
                    self.players[owner].name
                ));
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_conquests_building_culture_rates_are_loaded() {
        assert_eq!(building_culture_per_turn(Building::PALACE), Some(1));
        assert_eq!(building_culture_per_turn(Building::TEMPLE), Some(2));
        assert_eq!(building_culture_per_turn(Building::LIBRARY), Some(3));
        assert_eq!(building_culture_per_turn(Building::CATHEDRAL), Some(3));
        assert_eq!(building_culture_per_turn(Building::UNIVERSITY), Some(4));
        assert_eq!(building_culture_per_turn(Building::GREAT_LIBRARY), Some(6));
        assert_eq!(building_culture_per_turn(Building::SISTINE_CHAPEL), Some(6));
        assert_eq!(
            building_culture_per_turn(Building::KNIGHTS_TEMPLAR),
            Some(2)
        );
        assert_eq!(
            building_culture_per_turn(Building::MICHELANGELOS_CHAPEL),
            None
        );
    }

    #[test]
    fn city_culture_accumulates_by_owner_and_doubles_at_one_thousand_years() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.year = 999;
        game.cities[0].record_culture_building_completion(Building::PALACE, HUMAN, 999);
        game.cities[0].buildings.push(Building::TEMPLE);
        game.cities[0].record_culture_building_completion(Building::TEMPLE, HUMAN, 0);
        game.record_city_culture(0);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 3);

        game.year = 1000;
        game.record_city_culture(0);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 8);

        game.cities[0].transfer_citizens(1);
        game.record_city_culture(0);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 8);
        assert_eq!(game.cities[0].culture_for_civilization(1), 0);
        assert_eq!(game.civilization_culture(HUMAN), 0);
        assert_eq!(game.civilization_culture(1), 0);
    }

    #[test]
    fn city_turn_process_accrues_culture_from_installed_buildings() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.process_player_cities(HUMAN);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 1);
    }

    #[test]
    fn fascist_city_culture_waits_for_an_assimilated_population_majority() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0].size = 4;
        game.cities[0].foreign_citizens = vec![(1, 2)];
        game.cities[0].culture_suppressed_until_majority_assimilated = true;
        game.players[HUMAN].government = Government::Fascism;

        game.record_city_culture(0);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 0);

        game.players[HUMAN].government = Government::Despotism;
        game.record_city_culture(0);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 1);

        game.players[HUMAN].government = Government::Fascism;
        game.cities[0].foreign_citizens[0].1 = 1;
        game.record_city_culture(0);
        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 2);
    }

    #[test]
    fn source_map_size_thresholds_drive_cultural_victory() {
        let thresholds = culture_victory_thresholds();
        assert_eq!(
            thresholds
                .iter()
                .map(|item| item.civilization)
                .collect::<Vec<_>>(),
            [60_000, 80_000, 100_000, 130_000, 160_000]
        );
        for (size, expected) in [
            (crate::mapgen::Civ3MapSize::Tiny, 60_000),
            (crate::mapgen::Civ3MapSize::Small, 80_000),
            (crate::mapgen::Civ3MapSize::Standard, 100_000),
            (crate::mapgen::Civ3MapSize::Large, 130_000),
            (crate::mapgen::Civ3MapSize::Huge, 160_000),
        ] {
            let map = size.topology().unwrap();
            assert_eq!(
                culture_victory_thresholds_for_map(map)
                    .unwrap()
                    .civilization,
                expected
            );
        }
        assert!(culture_victory_thresholds_for_map(MapWrap::X).is_none());
    }

    #[test]
    fn one_city_and_civilization_culture_thresholds_end_the_game() {
        let mut city_victory = Simulation::new(crate::mapgen::generate_for_civ3_size(
            42,
            crate::mapgen::Civ3MapSize::Standard,
            crate::mapgen::Civ3WorldSettings::default(),
        ));
        city_victory.create_city(HUMAN, 10, 20);
        city_victory.cities[0].add_culture(HUMAN, 19_999);
        city_victory.check_cultural_victory();
        assert!(city_victory.game_over().is_none());
        city_victory.cities[0].add_culture(HUMAN, 1);
        city_victory.check_cultural_victory();
        assert!(city_victory.game_over().unwrap().contains("city culture"));

        let mut civilization_victory = Simulation::new(crate::mapgen::generate_for_civ3_size(
            43,
            crate::mapgen::Civ3MapSize::Tiny,
            crate::mapgen::Civ3WorldSettings::default(),
        ));
        for (x, y) in [(8, 10), (18, 10), (28, 10), (38, 10)] {
            civilization_victory.create_city(HUMAN, x, y);
        }
        for city in civilization_victory.cities.iter_mut().take(3) {
            city.add_culture(HUMAN, 15_000);
        }
        civilization_victory.cities[3].add_culture(HUMAN, 14_999);
        civilization_victory.check_cultural_victory();
        assert!(civilization_victory.game_over().is_none());
        civilization_victory.cities[3].add_culture(HUMAN, 1);
        civilization_victory.check_cultural_victory();
        assert!(
            civilization_victory
                .game_over()
                .unwrap()
                .contains("civilization culture")
        );
    }

    #[test]
    fn civilization_culture_victory_requires_at_least_twice_each_living_rivals_culture() {
        let game_with_rival_culture = |rival_culture: i64| {
            let mut game = Simulation::new(crate::mapgen::generate_for_civ3_size(
                44,
                crate::mapgen::Civ3MapSize::Tiny,
                crate::mapgen::Civ3WorldSettings::default(),
            ));
            for (x, y) in [(8, 10), (18, 10), (28, 10), (38, 10)] {
                game.create_city(HUMAN, x, y);
            }
            game.create_city(1, 8, 30);
            game.create_city(1, 28, 30);
            for city in game.cities.iter_mut().take(4) {
                city.add_culture(HUMAN, 15_000);
            }
            game.cities[4].add_culture(1, rival_culture / 2);
            game.cities[5].add_culture(1, rival_culture - rival_culture / 2);
            game
        };

        let mut blocked = game_with_rival_culture(30_001);
        blocked.check_cultural_victory();
        assert!(blocked.game_over().is_none());

        let mut at_half = game_with_rival_culture(30_000);
        at_half.check_cultural_victory();
        assert!(at_half.game_over().unwrap().contains("civilization culture"));
    }
}
