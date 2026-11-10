//! Source-backed Civilization III: Conquests civilization catalog.
//!
//! `civilizations.tsv` contains the playable RACE records in the installed
//! Conquests BIQ order. OpenCiv3's ruleset JSON is the checked-in extraction
//! reference; the original BIQ remains the authority for names and city order.
//! City-style IDs are a renderer mapping because the BIQ stores culture groups,
//! not the five Civ III city-sheet IDs.

use crate::rules::Tech;
use std::sync::OnceLock;

#[derive(Debug)]
pub(crate) struct Civilization {
    pub(crate) id: String,
    /// Existing CivCraft display name, retained where possible for save/UI compatibility.
    pub(crate) name: String,
    pub(crate) source_name: String,
    pub(crate) noun: String,
    pub(crate) adjective: String,
    pub(crate) leader: String,
    pub(crate) cities: Vec<String>,
    pub(crate) civ3_city_style: u8,
    pub(crate) starting_techs: [Tech; 2],
    pub(crate) primary_color_index: u8,
    pub(crate) secondary_color_index: u8,
    pub(crate) legacy_names: Vec<String>,
    pub(crate) source_race_index: usize,
    /// Raw Conquests RACE CultureGroup index; separate from culture points.
    pub(crate) culture_group_index: i32,
}

fn field<'a>(fields: &'a [&str], index: usize, line: usize) -> &'a str {
    fields
        .get(index)
        .copied()
        .unwrap_or_else(|| panic!("missing civilization catalog field on row {line}"))
}

fn parse_tech(name: &str, line: usize) -> Tech {
    Tech::all()
        .find(|&tech| tech.name() == name)
        .unwrap_or_else(|| panic!("unknown starting technology {name:?} on row {line}"))
}

fn parse_catalog() -> Vec<Civilization> {
    let mut lines = include_str!("../../data/rulesets/civ3_conquests/civilizations.tsv").lines();
    let header = lines.next().expect("civilization catalog has a header");
    assert_eq!(
        header.split('\t').count(),
        15,
        "civilization catalog schema changed"
    );
    let mut result = Vec::new();
    for (zero_line, line) in lines.enumerate() {
        let line_number = zero_line + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            15,
            "invalid civilization catalog row {line_number}"
        );
        let parse_index = |index: usize| {
            field(&fields, index, line_number)
                .parse::<usize>()
                .unwrap_or_else(|error| {
                    panic!("invalid catalog index on row {line_number}: {error}")
                })
        };
        let parse_color = |index: usize| {
            u8::try_from(parse_index(index))
                .unwrap_or_else(|error| panic!("invalid color index on row {line_number}: {error}"))
        };
        let source_race_index = parse_index(12);
        assert_eq!(
            source_race_index,
            result.len() + 1,
            "catalog must preserve BIQ RACE order"
        );
        let cities = field(&fields, 13, line_number)
            .split('|')
            .filter(|city| !city.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(!cities.is_empty(), "empty city list on row {line_number}");
        result.push(Civilization {
            id: field(&fields, 0, line_number).to_owned(),
            name: field(&fields, 1, line_number).to_owned(),
            source_name: field(&fields, 2, line_number).to_owned(),
            noun: field(&fields, 3, line_number).to_owned(),
            adjective: field(&fields, 4, line_number).to_owned(),
            leader: field(&fields, 5, line_number).to_owned(),
            primary_color_index: parse_color(6),
            secondary_color_index: parse_color(7),
            civ3_city_style: parse_color(8),
            starting_techs: [
                parse_tech(field(&fields, 9, line_number), line_number),
                parse_tech(field(&fields, 10, line_number), line_number),
            ],
            legacy_names: field(&fields, 11, line_number)
                .split(';')
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect(),
            source_race_index,
            culture_group_index: field(&fields, 14, line_number)
                .parse::<i32>()
                .unwrap_or_else(|error| {
                    panic!("invalid culture group index on row {line_number}: {error}")
                }),
            cities,
        });
    }
    assert_eq!(
        result.len(),
        31,
        "Conquests has 31 playable civilization records"
    );
    result
}

pub(crate) fn all() -> &'static [Civilization] {
    static CIVILIZATIONS: OnceLock<Vec<Civilization>> = OnceLock::new();
    CIVILIZATIONS.get_or_init(parse_catalog)
}

pub(crate) fn by_id(id: &str) -> Option<&'static Civilization> {
    all().iter().find(|civilization| civilization.id == id)
}

pub(crate) fn by_name(name: &str) -> Option<&'static Civilization> {
    all().iter().find(|civilization| {
        civilization.name.eq_ignore_ascii_case(name)
            || civilization.source_name.eq_ignore_ascii_case(name)
            || civilization.noun.eq_ignore_ascii_case(name)
            || civilization
                .legacy_names
                .iter()
                .any(|legacy| legacy.eq_ignore_ascii_case(name))
    })
}

/// Keep Rome as the human civilization and vary the AI roster by world seed.
/// This uses a local SplitMix stream, so roster selection does not consume any
/// map-generation or gameplay random draws.
pub(crate) fn select_roster(seed: u64, slot_count: usize) -> Vec<&'static Civilization> {
    let catalog = all();
    assert!((1..=16).contains(&slot_count));
    let roman = catalog
        .iter()
        .position(|civilization| civilization.id == "rome")
        .expect("Conquests roster contains Rome");
    let mut candidates = (0..catalog.len())
        .filter(|&index| index != roman)
        .collect::<Vec<_>>();
    let mut state = seed ^ 0x4356_3343_4956_5331;
    for index in (1..candidates.len()).rev() {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
        candidates.swap(index, (value as usize) % (index + 1));
    }
    std::iter::once(&catalog[roman])
        .chain(candidates.into_iter().map(|index| &catalog[index]))
        .take(slot_count)
        .collect()
}

pub(crate) fn next_city_name(
    civilization_id: &str,
    occupied: impl Fn(&str) -> bool,
) -> Option<String> {
    let civilization = by_id(civilization_id)?;
    for name in &civilization.cities {
        if !occupied(name) {
            return Some(name.clone());
        }
    }
    for generation in 1.. {
        for name in &civilization.cities {
            let candidate = match generation {
                1 if name == "Tokyo" => "Neo-Tokyo".to_owned(),
                1 => format!("New {name}"),
                n => format!("New {name} {n}"),
            };
            if !occupied(&candidate) {
                return Some(candidate);
            }
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conquests_civilization_catalog_covers_31_unique_source_races() {
        assert_eq!(all().len(), 31);
        for (index, civilization) in all().iter().enumerate() {
            assert_eq!(civilization.source_race_index, index + 1);
            assert!((0..=4).contains(&civilization.culture_group_index));
            assert!(!civilization.leader.is_empty());
            assert!(civilization.civ3_city_style < 5);
            assert!((civilization.primary_color_index as usize) < 32);
            assert!((civilization.secondary_color_index as usize) < 32);
            assert_eq!(
                civilization.cities.first().map(String::as_str),
                Some(match civilization.source_name.as_str() {
                    "Rome" => "Rome",
                    "Egypt" => "Thebes",
                    "Greece" => "Athens",
                    "Babylon" => "Babylon",
                    _ => civilization.cities[0].as_str(),
                })
            );
            assert_eq!(
                civilization
                    .cities
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                civilization.cities.len()
            );
        }
        assert_eq!(by_id("rome").unwrap().culture_group_index, 2);
    }

    #[test]
    fn source_names_and_legacy_save_labels_resolve_to_the_same_civilization() {
        assert_eq!(by_name("Germany").unwrap().id, "germany");
        assert_eq!(by_name("Germans").unwrap().id, "germany");
        assert_eq!(by_name("Iron Confederation").unwrap().id, "germany");
        assert_eq!(by_name("Maya").unwrap().id, "maya");
        assert_eq!(by_name("Mayans").unwrap().id, "maya");
    }

    #[test]
    fn map_roster_keeps_rome_and_is_seeded_without_changing_civ_count() {
        for count in [4, 6, 8, 12, 16] {
            let first = select_roster(42, count);
            assert_eq!(first.len(), count);
            assert_eq!(first[0].id, "rome");
            assert_eq!(
                first
                    .iter()
                    .map(|civ| civ.id.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                count
            );
            assert_eq!(
                select_roster(42, count)
                    .iter()
                    .map(|civ| &civ.id)
                    .collect::<Vec<_>>(),
                first.iter().map(|civ| &civ.id).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn city_names_follow_source_order_and_overflow_without_duplicates() {
        assert_eq!(next_city_name("rome", |_| false).as_deref(), Some("Rome"));
        assert_eq!(
            next_city_name("rome", |name| name == "Rome" || name == "Veii").as_deref(),
            Some("Antium")
        );
        let cities = &by_id("rome").unwrap().cities;
        assert_eq!(
            next_city_name("rome", |name| cities.iter().any(|city| city == name)).as_deref(),
            Some("New Rome")
        );
        assert!(next_city_name("unknown", |_| false).is_none());
    }
}
