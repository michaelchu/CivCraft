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

/// Inputs to the community reverse-engineered Conquests city-flip formula.
///
/// This calculation is intentionally kept separate from turn resolution:
/// candidate tile geometry, capital selection on wrapped maps, and the exact
/// consequences of a flip still need controlled Conquests confirmation.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CityFlipFactors {
    foreign_citizens: i32,
    resisting_citizens: i32,
    foreign_radius_tiles: i32,
    challenger_city_culture: i64,
    defender_city_culture: i64,
    challenger_total_culture: i64,
    defender_total_culture: i64,
    disorder: bool,
    celebrating_wltkd: bool,
    defender_land_combat_garrison: i32,
    challenger_capital_distance: f64,
    defender_capital_distance: f64,
}

/// Return the candidate flip probability as a fraction, when its ratios exist.
///
/// CivFanatics' Conquests FAQ supplies the factors, while CivWiki's
/// reverse-engineered account supplies the `2000 * D` denominator and the
/// 0.25–4 clamp for the relative capital-distance modifier. A zero or missing
/// total-culture value and invalid distance return `None`: available sources
/// do not specify the engine's behavior for those undefined ratios.
#[allow(dead_code)] // Kept isolated until the remaining Conquests resolution rules are established.
fn city_flip_probability(factors: CityFlipFactors) -> Option<f64> {
    if factors.challenger_total_culture <= 0
        || factors.defender_total_culture <= 0
        || !factors.challenger_capital_distance.is_finite()
        || !factors.defender_capital_distance.is_finite()
        || factors.challenger_capital_distance <= 0.0
        || factors.defender_capital_distance <= 0.0
    {
        return None;
    }

    // FAQ F is all foreign nationals plus one additional count per resister.
    let foreign_pressure = factors.foreign_citizens.max(0)
        + factors.resisting_citizens.max(0)
        + factors.foreign_radius_tiles.clamp(0, 21);
    let local_culture_factor = if factors.challenger_city_culture > factors.defender_city_culture {
        2.0
    } else {
        1.0
    };
    let city_mood_factor = if factors.disorder {
        2.0
    } else if factors.celebrating_wltkd {
        0.5
    } else {
        1.0
    };
    let distance_factor =
        (factors.challenger_capital_distance / factors.defender_capital_distance).clamp(0.25, 4.0);
    let numerator = foreign_pressure as f64
        * local_culture_factor
        * city_mood_factor
        * (factors.challenger_total_culture as f64 / factors.defender_total_culture as f64)
        - factors.defender_land_combat_garrison.max(0) as f64;

    Some((numerator / (2000.0 * distance_factor)).clamp(0.0, 1.0))
}

/// Evaluate the installed Conquests Civilopedia's We Love the King Day
/// criteria once the caller has derived food and citizen moods for the city.
/// The FAQ supplies the commonly documented size-6 minimum; the Civilopedia
/// itself describes the city as "typically" reaching that size.
#[allow(dead_code)] // Used when the source-backed flip model is wired into turn resolution.
fn city_meets_wltkd_criteria(
    size: i32,
    food_surplus: i32,
    resisting_citizens: i32,
    happy: i32,
    unhappy: i32,
    angry: i32,
) -> bool {
    let content = size - happy - unhappy - angry;
    size >= 6
        && food_surplus >= 0
        && resisting_citizens == 0
        && unhappy == 0
        && angry == 0
        && happy > content
}

fn cultural_conversions_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let mut lines = include_str!("../../data/rulesets/civ3_conquests/culture_conversion_settings.tsv").lines();
        assert_eq!(
            lines.next(),
            Some("source_section\tsource_record_index\tdefault_game_rules\tcultural_conversions_flag\teffective_cultural_conversions\tbiq_sha256"),
            "invalid Conquests cultural-conversion settings header"
        );
        let fields = lines.next().expect("missing Conquests cultural-conversion setting")
            .split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 6);
        assert_eq!((fields[0], fields[1]), ("GAME", "0"));
        assert_eq!(fields[5], "1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c");
        let raw = match fields[3] { "true" => true, "false" => false, value => panic!("invalid conversion flag {value:?}") };
        let default_rules = fields[2].parse::<u8>().expect("invalid default-rules flag");
        assert!(default_rules <= 1);
        let effective = default_rules == 1 || raw;
        assert_eq!(fields[4], effective.to_string());
        assert!(lines.next().is_none(), "extra Conquests cultural-conversion settings");
        effective
    })
}

fn flip_capital(building: Building) -> bool {
    matches!(
        building,
        Building::PALACE | Building::FORBIDDEN_PALACE | Building::SECRET_POLICE_HQ
    )
}

#[allow(dead_code)] // Candidate assessment is kept separate from unverified flip resolution.
fn candidate_city_flip_probability_to(
    game: &Simulation,
    city_id: u32,
    challenger: usize,
) -> Option<f64> {
    if !cultural_conversions_enabled() || challenger >= game.civilization_count {
        return Some(0.0);
    }
    let city = game.cities.iter().find(|city| city.id == city_id)?;
    let defender = city.owner;
    if defender >= game.civilization_count || defender == challenger {
        return None;
    }
    let capital_distance = |civilization| {
        game.cities
            .iter()
            .filter(|capital| {
                capital.owner == civilization && capital.buildings.iter().copied().any(flip_capital)
            })
            .map(|capital| {
                game.map_wrap
                    .civ3_culture_distance(city.x, city.y, capital.x, capital.y)
            })
            .min_by(f64::total_cmp)
    };
    let challenger_capital_distance = capital_distance(challenger)?;
    let defender_capital_distance = capital_distance(defender)?;
    let forecast = game.city_forecast(city);
    let (happy, unhappy, angry) = game.city_feelings(city, forecast.luxury);
    // The CivFanatics Conquests FAQ defines F as all foreign citizens, with
    // resisters counted twice. Nationality is not restricted to the challenger.
    let resisting_citizens = city
        .resisting_citizens
        .iter()
        .map(|&(_, count)| count)
        .sum();
    let celebrating_wltkd = city_meets_wltkd_criteria(
        city.size,
        forecast.food_surplus(city.size, 0),
        city.resisting_citizens
            .iter()
            .map(|&(_, count)| count)
            .sum(),
        happy,
        unhappy,
        angry,
    );
    let foreign_citizens = city.foreign_citizens.iter().map(|&(_, count)| count).sum();
    let foreign_radius_tiles = game
        .map_city_radius(city)
        .filter(|&(_, _, x, y)| game.tile(x, y).owner == Some(challenger))
        .count() as i32;
    let defender_land_combat_garrison = game
        .units
        .iter()
        .filter(|unit| {
            unit.owner == defender
                && (unit.x, unit.y) == (city.x, city.y)
                && unit.kind.unit_class().is_ground()
                && unit.kind.stats().attack > 0
                && unit.kind.stats().defense > 0
        })
        .count() as i32;

    city_flip_probability(CityFlipFactors {
        foreign_citizens,
        resisting_citizens,
        foreign_radius_tiles,
        // The Conquests guide says each civilization's earned culture is
        // remembered by a city across capture. Current city culture may reset
        // under GAME.RetainCulture=0, but that reset does not erase this local
        // historical comparison used by the candidate defection formula.
        challenger_city_culture: city.historical_culture_for_civilization(challenger),
        defender_city_culture: city.historical_culture_for_civilization(defender),
        challenger_total_culture: game.civilization_culture(challenger),
        defender_total_culture: game.civilization_culture(defender),
        disorder: forecast.disorder,
        celebrating_wltkd,
        defender_land_combat_garrison,
        challenger_capital_distance,
        defender_capital_distance,
    })
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

/// Conquests Civilopedia's explicitly named city-culture border thresholds.
/// Later thresholds conflict with the editor's reverse-engineered Border
/// Factor sequence, so expansion is currently capped at the last explicit
/// Civilopedia example until the installed game can resolve the discrepancy.
const CITY_BORDER_CULTURE_THRESHOLDS: [i64; 5] = [10, 100, 1_000, 10_000, 20_000];

pub(super) fn city_cultural_border_rank(culture: i64) -> i32 {
    1 + CITY_BORDER_CULTURE_THRESHOLDS
        .iter()
        .filter(|&&threshold| culture >= threshold)
        .count() as i32
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
                "mapped-zero-output" => {
                    let _catalog_index = fields[4]
                        .parse::<usize>()
                        .expect("mapped zero-output row requires catalog ID");
                    assert!(_catalog_index < Building::all().count(), "unknown Building catalog ID");
                    assert_eq!(culture, 0, "mapped zero-output building has culture");
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
        assert!(
            result.iter().all(|threshold| threshold.one_city == 20_000),
            "Conquests one-city culture threshold must be 20,000 for every map size"
        );
        assert_eq!(result[2].one_city, 20_000);
        assert_eq!(result[2].civilization, 100_000);
        result
    })
}

fn cultural_victory_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let mut lines = include_str!(
            "../../data/rulesets/civ3_conquests/culture_victory_settings.tsv"
        )
        .lines();
        assert_eq!(
            lines.next(),
            Some("source_section\tsource_record_index\tdefault_victory_conditions\tcultural_victory_flag\teffective_cultural_victory\tbiq_sha256"),
            "invalid Conquests culture-victory settings header"
        );
        let fields = lines
            .next()
            .expect("missing Conquests culture-victory settings");
        assert!(lines.next().is_none(), "unexpected extra culture-victory settings");
        let fields = fields.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 6, "invalid Conquests culture-victory settings row");
        assert_eq!(fields[0], "GAME");
        assert_eq!(fields[1], "0");
        assert_eq!(
            fields[5],
            "1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c"
        );
        let default_victories = fields[2].parse::<u8>().expect("invalid default victory flag");
        assert!(default_victories <= 1);
        let cultural_flag = match fields[3] {
            "true" => true,
            "false" => false,
            value => panic!("invalid individual cultural-victory flag: {value:?}"),
        };
        let effective = effective_cultural_victory(default_victories == 1, cultural_flag);
        assert_eq!(fields[4], effective.to_string());
        effective
    })
}

fn effective_cultural_victory(default_conditions: bool, individual_flag: bool) -> bool {
    default_conditions || individual_flag
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
        if self.game_over.is_some() || !cultural_victory_enabled() {
            return;
        }
        let one_city_threshold = culture_victory_thresholds()[0].one_city;
        let civilization_threshold = culture_victory_thresholds_for_map(self.map_wrap)
            .map(|thresholds| thresholds.civilization);
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.player_alive(owner) {
                continue;
            }
            let city_winner = self.cities.iter().any(|city| {
                city.owner == owner && city.culture_for_civilization(owner) >= one_city_threshold
            });
            let civilization_total = self.civilization_culture(owner);
            let civilization_winner = civilization_threshold
                .is_some_and(|threshold| civilization_total >= threshold)
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

    fn candidate_flip_fixture() -> (Simulation, u32) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        for tile in &mut game.tiles {
            tile.owner = None;
            tile.border_city = None;
            tile.terrain = Terrain::Grassland;
        }
        game.create_city(HUMAN, 10, 24);
        game.create_city(HUMAN, 14, 24);
        game.create_city(1, 32, 24);
        game.create_city(1, 24, 24);
        game.cities[0].add_culture(HUMAN, 100);
        game.cities[2].add_culture(1, 100);
        let target = &mut game.cities[3];
        target.foreign_citizens = vec![(HUMAN, 1)];
        let target_id = target.id;
        let (x, y) = game
            .map_city_radius(&game.cities[3])
            .find(|&(_, _, x, y)| (x, y) != (24, 24))
            .map(|(_, _, x, y)| (x, y))
            .unwrap();
        game.tile_mut(x, y).owner = Some(HUMAN);
        (game, target_id)
    }

    #[test]
    fn reverse_engineered_city_flip_formula_matches_civwiki_example() {
        let factors = CityFlipFactors {
            foreign_citizens: 0,
            resisting_citizens: 0,
            foreign_radius_tiles: 2,
            challenger_city_culture: 0,
            defender_city_culture: 0,
            challenger_total_culture: 145,
            defender_total_culture: 176,
            disorder: false,
            celebrating_wltkd: false,
            defender_land_combat_garrison: 1,
            challenger_capital_distance: 10.0,
            defender_capital_distance: 9.0,
        };

        let probability = city_flip_probability(factors).unwrap();
        assert!((probability - 0.00029).abs() < 0.00001);
    }

    #[test]
    fn reverse_engineered_city_flip_formula_doubles_resisters_and_clamps_risk() {
        let factors = CityFlipFactors {
            foreign_citizens: 2,
            resisting_citizens: 2,
            foreign_radius_tiles: 2,
            challenger_city_culture: 1,
            defender_city_culture: 0,
            challenger_total_culture: 100_000,
            defender_total_culture: 1,
            disorder: true,
            celebrating_wltkd: false,
            defender_land_combat_garrison: 0,
            challenger_capital_distance: 1.0,
            defender_capital_distance: 100.0,
        };

        // D is clamped to 0.25, and the resulting chance cannot exceed 100%.
        assert_eq!(city_flip_probability(factors), Some(1.0));
        assert_eq!(
            city_flip_probability(CityFlipFactors {
                defender_land_combat_garrison: 4_000_000,
                ..factors
            }),
            Some(0.0)
        );
    }

    #[test]
    fn reverse_engineered_city_flip_formula_preserves_undefined_culture_ratio() {
        let factors = CityFlipFactors {
            foreign_citizens: 1,
            resisting_citizens: 0,
            foreign_radius_tiles: 0,
            challenger_city_culture: 1,
            defender_city_culture: 0,
            challenger_total_culture: 1,
            defender_total_culture: 0,
            disorder: false,
            celebrating_wltkd: false,
            defender_land_combat_garrison: 0,
            challenger_capital_distance: 1.0,
            defender_capital_distance: 1.0,
        };

        assert_eq!(city_flip_probability(factors), None);
    }

    #[test]
    fn reverse_engineered_city_flip_formula_applies_mood_and_local_culture_factors() {
        let base = CityFlipFactors {
            foreign_citizens: 3,
            resisting_citizens: 0,
            foreign_radius_tiles: 4,
            challenger_city_culture: 2,
            defender_city_culture: 1,
            challenger_total_culture: 2_000,
            defender_total_culture: 1_000,
            disorder: false,
            celebrating_wltkd: false,
            defender_land_combat_garrison: 0,
            challenger_capital_distance: 10.0,
            defender_capital_distance: 10.0,
        };

        assert!((city_flip_probability(base).unwrap() - 0.014).abs() < 1e-12);
        assert!(
            (city_flip_probability(CityFlipFactors {
                celebrating_wltkd: true,
                ..base
            })
            .unwrap()
                - 0.007)
                .abs()
                < 1e-12
        );
        assert!(
            (city_flip_probability(CityFlipFactors {
                disorder: true,
                ..base
            })
            .unwrap()
                - 0.028)
                .abs()
                < 1e-12
        );
        assert!(
            (city_flip_probability(CityFlipFactors {
                challenger_city_culture: 1,
                ..base
            })
            .unwrap()
                - 0.007)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn captured_city_keeps_per_civilization_culture_memory_for_candidate_flip_pressure() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.cities.clear();
        game.units.clear();
        for tile in &mut game.tiles {
            tile.owner = None;
            tile.border_city = None;
            tile.terrain = Terrain::Grassland;
        }
        game.create_city(1, 10, 24);
        let target_id = {
            game.create_city(1, 20, 24);
            game.cities[1].id
        };
        game.create_city(HUMAN, 30, 24);
        game.cities[0].add_culture(1, 100);
        game.cities[1].add_culture(1, 500);
        game.cities[2].add_culture(HUMAN, 100);
        game.sign_pact(HUMAN, 1, Pact::War);
        let attacker = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);

        game.capture_city_if_needed(attacker, 20, 24);
        let target_index = game.city_index_by_id(target_id).unwrap();
        assert_eq!(
            game.cities[target_index].culture_for_civilization(1),
            0,
            "the standard Conquests capture setting clears current city culture"
        );
        assert_eq!(
            game.cities[target_index].historical_culture_for_civilization(1),
            500,
            "the city remembers the former civilization's earned culture"
        );
        game.cities[target_index].add_culture(HUMAN, 100);
        game.units.clear();

        let remembered =
            candidate_city_flip_probability_to(&game, target_id, 1).unwrap();
        let mut no_memory = game.clone();
        no_memory.cities[target_index]
            .culture_memory_by_civilization
            .retain(|&(civilization, _)| civilization != 1);
        let without_former_culture =
            candidate_city_flip_probability_to(&no_memory, target_id, 1).unwrap();
        assert!(
            remembered > without_former_culture,
            "historical local culture should supply the reverse-engineered local-culture advantage after capture"
        );
    }

    #[test]
    fn conquests_wltkd_requires_food_happiness_population_and_no_resisters() {
        let qualifies = |size, food, resisters, happy, unhappy, angry| {
            city_meets_wltkd_criteria(size, food, resisters, happy, unhappy, angry)
        };
        assert!(qualifies(6, 0, 0, 4, 0, 0));
        assert!(!qualifies(5, 0, 0, 4, 0, 0));
        assert!(!qualifies(6, -1, 0, 4, 0, 0));
        assert!(!qualifies(6, 0, 1, 4, 0, 0));
        assert!(!qualifies(6, 0, 0, 3, 0, 0)); // happy must exceed content
        assert!(!qualifies(6, 0, 0, 5, 1, 0));
        assert!(!qualifies(6, 0, 0, 5, 0, 1));
    }

    #[test]
    fn candidate_flip_probability_derives_foreign_population_tiles_cultures_and_capitals() {
        let (game, city_id) = candidate_flip_fixture();
        assert!(cultural_conversions_enabled());
        assert_eq!(Building::SECRET_POLICE_HQ.data().name, "Secret Police HQ");
        let base = candidate_city_flip_probability_to(&game, city_id, HUMAN).unwrap();
        assert!(base > 0.0);

        let mut secret_police_capital = game.clone();
        secret_police_capital.cities[0]
            .buildings
            .retain(|&building| building != Building::PALACE);
        secret_police_capital.cities[0]
            .buildings
            .push(Building::SECRET_POLICE_HQ);
        assert_eq!(
            candidate_city_flip_probability_to(&secret_police_capital, city_id, HUMAN),
            Some(base)
        );

        let mut garrisoned = game.clone();
        let (x, y) = (garrisoned.cities[3].x, garrisoned.cities[3].y);
        garrisoned.spawn_unit(1, UnitKind::Warrior, x, y);
        let guarded = candidate_city_flip_probability_to(&garrisoned, city_id, HUMAN).unwrap();
        assert!(guarded < base);

        let mut stronger_local_culture = game.clone();
        stronger_local_culture.cities[3].add_culture(HUMAN, 1);
        let doubled =
            candidate_city_flip_probability_to(&stronger_local_culture, city_id, HUMAN).unwrap();
        assert!((doubled / base - 2.0).abs() < 1e-9);
    }

    #[test]
    fn candidate_flip_probability_counts_all_foreign_nationalities() {
        let (mut game, city_id) = candidate_flip_fixture();
        game.cities[3].foreign_citizens.push((2, 1));
        let with_multiple_nationalities =
            candidate_city_flip_probability_to(&game, city_id, HUMAN).unwrap();
        game.cities[3].foreign_citizens[1].1 -= 1;
        let challenger_nationality_only =
            candidate_city_flip_probability_to(&game, city_id, HUMAN).unwrap();
        assert!(with_multiple_nationalities > challenger_nationality_only);
    }

    #[test]
    fn city_border_rank_advances_at_conquests_civilopedia_thresholds() {
        let cases = [
            (0, 1),
            (9, 1),
            (10, 2),
            (99, 2),
            (100, 3),
            (999, 3),
            (1_000, 4),
            (9_999, 4),
            (10_000, 5),
            (19_999, 5),
            (20_000, 6),
            (100_000, 6),
        ];
        for (culture, expected_rank) in cases {
            assert_eq!(city_cultural_border_rank(culture), expected_rank);
        }
    }

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
    fn obsolete_conquests_wonder_keeps_its_culture_output() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        game.cities[0]
            .buildings
            .retain(|&building| building != Building::PALACE);
        game.players[HUMAN].known.push(Tech::FLIGHT);
        game.cities[0].buildings.push(Building::COLOSSUS);
        game.cities[0].record_culture_building_completion(Building::COLOSSUS, HUMAN, game.year);

        assert!(game.building_is_obsolete(&game.cities[0], Building::COLOSSUS, None));
        game.record_city_culture(0);

        assert_eq!(game.cities[0].culture_for_civilization(HUMAN), 3);
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
        assert!(cultural_victory_enabled());
        assert!(effective_cultural_victory(true, false));
        assert!(effective_cultural_victory(false, true));
        assert!(!effective_cultural_victory(false, false));
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
    fn custom_map_keeps_explicit_one_city_victory_but_no_empire_threshold_is_assumed() {
        assert!(culture_victory_thresholds_for_map(MapWrap::X).is_none());
        let mut game = Simulation::new(crate::mapgen::generate(45));
        game.cities.clear();
        for (x, y) in [(4, 4), (12, 4), (20, 4), (28, 4), (4, 20), (12, 20)] {
            game.create_city(HUMAN, x, y);
            game.cities.last_mut().unwrap().add_culture(HUMAN, 17_000);
        }

        // The empire total exceeds the Standard BIQ value, but the custom
        // campaign topology has no source-backed civilization threshold.
        game.check_cultural_victory();
        assert!(game.game_over().is_none());

        game.cities[0].add_culture(HUMAN, 3_000);
        game.check_cultural_victory();
        assert!(game.game_over().unwrap().contains("city culture"));
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
        assert!(
            at_half
                .game_over()
                .unwrap()
                .contains("civilization culture")
        );
    }
}
