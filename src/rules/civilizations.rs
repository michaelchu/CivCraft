//! Native civilization catalog. Stable IDs identify civilizations; names are display content.
//! catalog construction does not depend on imported RACE records.

use crate::rules::Tech;
use std::sync::OnceLock;

#[derive(Debug)]
pub(crate) struct Civilization {
    pub(crate) id: String,
    /// Display label; gameplay identity is the stable `id`.
    pub(crate) name: String,
    pub(crate) noun: String,
    #[allow(dead_code)] // Source-backed catalog metadata for future leader and UI text.
    pub(crate) adjective: String,
    pub(crate) leader: String,
    pub(crate) cities: Vec<String>,
    pub(crate) civ3_city_style: u8,
    pub(crate) starting_techs: [Tech; 2],
    pub(crate) primary_color_index: u8,
    pub(crate) secondary_color_index: u8,
    /// Raw Conquests RACE CultureGroup index; separate from culture points.
    #[allow(dead_code)] // Kept as source metadata for city-style and culture rules.
    pub(crate) culture_group_index: i32,
}

fn parse_tech(name: &str, line: usize) -> Tech {
    Tech::all()
        .find(|&tech| tech.name() == name)
        .unwrap_or_else(|| panic!("unknown starting technology {name:?} on row {line}"))
}

fn parse_catalog() -> Vec<Civilization> {
    let mut result = Vec::new();
    for (index, native) in crate::rules::native::civilizations()
        .civilizations
        .iter()
        .enumerate()
    {
        result.push(Civilization {
            id: native.id.clone(),
            name: native.name.clone(),
            noun: native.noun.clone(),
            adjective: native.adjective.clone(),
            leader: native.leader.clone(),
            primary_color_index: native.primary_color_index,
            secondary_color_index: native.secondary_color_index,
            civ3_city_style: native.city_style,
            starting_techs: [
                parse_tech(&native.starting_techs[0], index + 1),
                parse_tech(&native.starting_techs[1], index + 1),
            ],
            culture_group_index: native.culture_group_index,
            cities: native.cities.clone(),
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
        for civilization in all() {
            assert!((0..=4).contains(&civilization.culture_group_index));
            assert!(!civilization.leader.is_empty());
            assert!(civilization.civ3_city_style < 5);
            assert!((civilization.primary_color_index as usize) < 32);
            assert!((civilization.secondary_color_index as usize) < 32);
            assert_eq!(
                civilization.cities.first().map(String::as_str),
                Some(match civilization.id.as_str() {
                    "rome" => "Rome",
                    "egypt" => "Thebes",
                    "greece" => "Athens",
                    "babylon" => "Babylon",
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
    fn native_toml_civilizations_match_the_legacy_extraction() {
        let native = crate::rules::native::civilizations();
        assert_eq!(native.civilizations.len(), all().len());
        for source in all() {
            let migrated = native
                .civilizations
                .iter()
                .find(|civilization| civilization.id == source.id)
                .unwrap_or_else(|| panic!("native TOML is missing {}", source.id));
            assert_eq!(migrated.name, source.name);
            assert_eq!(migrated.noun, source.noun);
            assert_eq!(migrated.adjective, source.adjective);
            assert_eq!(migrated.leader, source.leader);
            assert_eq!(migrated.primary_color_index, source.primary_color_index);
            assert_eq!(migrated.secondary_color_index, source.secondary_color_index);
            assert_eq!(migrated.city_style, source.civ3_city_style);
            assert_eq!(migrated.culture_group_index, source.culture_group_index);
            assert_eq!(migrated.cities, source.cities);
            assert_eq!(
                migrated.starting_techs,
                source
                    .starting_techs
                    .iter()
                    .map(|tech| tech.name().to_owned())
                    .collect::<Vec<_>>()
            );
            assert_eq!(migrated.starting_units.len(), 2);
            assert_eq!(migrated.starting_units[0].id, "Settler");
            assert_eq!(migrated.starting_units[0].count, 1);
            assert_eq!(migrated.starting_units[1].id, "Worker");
            assert_eq!(migrated.starting_units[1].count, 1);
        }
    }

    #[test]
    fn stable_ids_resolve_every_civilization_and_display_names_are_not_ids() {
        for civilization in all() {
            assert_eq!(by_id(&civilization.id).unwrap().name, civilization.name);
        }
        assert!(by_id("Crimson League").is_none());
        assert!(by_id("Iron Confederation").is_none());
        assert!(by_id("Americans").is_none());
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
