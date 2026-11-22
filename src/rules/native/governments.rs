//! Campaign government table.

use super::*;

pub(crate) struct GovernmentCatalog {
    pub(crate) governments: Vec<NativeGovernment>,
}

impl GovernmentCatalog {
    pub(crate) fn get(&self, name: &str) -> Option<&NativeGovernment> {
        self.governments
            .iter()
            .find(|government| government.government.eq_ignore_ascii_case(name))
    }
}

pub(crate) struct NativeGovernment {
    pub(crate) government: String,
    pub(crate) name: String,
    pub(crate) required_technology: String,
    pub(crate) technology: Option<crate::rules::Tech>,
    pub(crate) preferred_government: Option<String>,
    pub(crate) flags: Vec<String>,
    pub(crate) town_free_units: i32,
    pub(crate) city_free_units: i32,
    pub(crate) metropolis_free_units: i32,
    pub(crate) cost_per_excess_unit_gold: i32,
}

/// One government as written in source, with named fields instead of the
/// positional tuple the table used to take.
struct GovernmentSpec {
    government: &'static str,
    name: &'static str,
    required_technology: &'static str,
    technology: Option<crate::rules::Tech>,
    preferred_government: Option<&'static str>,
    flags: &'static [&'static str],
    town_free_units: i32,
    city_free_units: i32,
    metropolis_free_units: i32,
    cost_per_excess_unit_gold: i32,
}

/// Table order is the catalog order and is observable (UI lists, AI ties).
const GOVERNMENTS: &[GovernmentSpec] = &[
    GovernmentSpec {
        government: "anarchy",
        name: "Anarchy",
        required_technology: "",
        technology: None,
        preferred_government: None,
        flags: &[],
        town_free_units: 0,
        city_free_units: 0,
        metropolis_free_units: 0,
        cost_per_excess_unit_gold: 1,
    },
    GovernmentSpec {
        government: "despotism",
        name: "Despotism",
        required_technology: "",
        technology: None,
        preferred_government: Some("monarchy"),
        flags: &["ShieldToGold"],
        town_free_units: 4,
        city_free_units: 4,
        metropolis_free_units: 4,
        cost_per_excess_unit_gold: 1,
    },
    GovernmentSpec {
        government: "monarchy",
        name: "Monarchy",
        required_technology: "TECH_Monarchy",
        technology: Some(crate::rules::Tech::MONARCHY),
        preferred_government: None,
        flags: &["ShieldToGold"],
        town_free_units: 2,
        city_free_units: 4,
        metropolis_free_units: 8,
        cost_per_excess_unit_gold: 1,
    },
    GovernmentSpec {
        government: "communism",
        name: "Communism",
        required_technology: "TECH_Communism",
        technology: Some(crate::rules::Tech::COMMUNISM),
        preferred_government: None,
        flags: &[],
        town_free_units: 6,
        city_free_units: 6,
        metropolis_free_units: 6,
        cost_per_excess_unit_gold: 1,
    },
    GovernmentSpec {
        government: "republic",
        name: "Republic",
        required_technology: "TECH_The_Republic",
        technology: Some(crate::rules::Tech::THE_REPUBLIC),
        preferred_government: None,
        flags: &[],
        town_free_units: 1,
        city_free_units: 3,
        metropolis_free_units: 4,
        cost_per_excess_unit_gold: 2,
    },
    GovernmentSpec {
        government: "democracy",
        name: "Democracy",
        required_technology: "TECH_Democracy",
        technology: Some(crate::rules::Tech::DEMOCRACY),
        preferred_government: None,
        flags: &["ShieldToGold"],
        town_free_units: 0,
        city_free_units: 0,
        metropolis_free_units: 0,
        cost_per_excess_unit_gold: 1,
    },
    GovernmentSpec {
        government: "fascism",
        name: "Fascism",
        required_technology: "TECH_Fascism",
        technology: Some(crate::rules::Tech::FASCISM),
        preferred_government: None,
        flags: &[],
        town_free_units: 4,
        city_free_units: 7,
        metropolis_free_units: 10,
        cost_per_excess_unit_gold: 1,
    },
    GovernmentSpec {
        government: "feudalism",
        name: "Feudalism",
        required_technology: "TECH_Feudalism",
        technology: Some(crate::rules::Tech::FEUDALISM),
        preferred_government: None,
        flags: &[],
        town_free_units: 5,
        city_free_units: 2,
        metropolis_free_units: 1,
        cost_per_excess_unit_gold: 3,
    },
];

impl From<&GovernmentSpec> for NativeGovernment {
    fn from(spec: &GovernmentSpec) -> Self {
        Self {
            government: spec.government.to_owned(),
            name: spec.name.to_owned(),
            required_technology: spec.required_technology.to_owned(),
            technology: spec.technology,
            preferred_government: spec.preferred_government.map(str::to_owned),
            flags: spec.flags.iter().map(|flag| (*flag).to_owned()).collect(),
            town_free_units: spec.town_free_units,
            city_free_units: spec.city_free_units,
            metropolis_free_units: spec.metropolis_free_units,
            cost_per_excess_unit_gold: spec.cost_per_excess_unit_gold,
        }
    }
}

pub(crate) fn governments() -> &'static GovernmentCatalog {
    static CATALOG: OnceLock<GovernmentCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| GovernmentCatalog {
        governments: GOVERNMENTS.iter().map(NativeGovernment::from).collect(),
    })
}
