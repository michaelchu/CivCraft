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

pub(crate) fn governments() -> &'static GovernmentCatalog {
    static CATALOG: OnceLock<GovernmentCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let gov = |government: &str,
                   name: &str,
                   required_technology: &str,
                   technology: Option<crate::rules::Tech>,
                   preferred_government: Option<&str>,
                   flags: &[&str],
                   (
            town_free_units,
            city_free_units,
            metropolis_free_units,
            cost_per_excess_unit_gold,
        )| {
            NativeGovernment {
                government: government.to_owned(),
                name: name.to_owned(),
                required_technology: required_technology.to_owned(),
                technology,
                preferred_government: preferred_government.map(str::to_owned),
                flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
                town_free_units,
                city_free_units,
                metropolis_free_units,
                cost_per_excess_unit_gold,
            }
        };
        GovernmentCatalog {
            governments: vec![
                gov("anarchy", "Anarchy", "", None, None, &[], (0, 0, 0, 1)),
                gov(
                    "despotism",
                    "Despotism",
                    "",
                    None,
                    Some("monarchy"),
                    &["ShieldToGold"],
                    (4, 4, 4, 1),
                ),
                gov(
                    "monarchy",
                    "Monarchy",
                    "TECH_Monarchy",
                    Some(crate::rules::Tech::MONARCHY),
                    None,
                    &["ShieldToGold"],
                    (2, 4, 8, 1),
                ),
                gov(
                    "communism",
                    "Communism",
                    "TECH_Communism",
                    Some(crate::rules::Tech::COMMUNISM),
                    None,
                    &[],
                    (6, 6, 6, 1),
                ),
                gov(
                    "republic",
                    "Republic",
                    "TECH_The_Republic",
                    Some(crate::rules::Tech::THE_REPUBLIC),
                    None,
                    &[],
                    (1, 3, 4, 2),
                ),
                gov(
                    "democracy",
                    "Democracy",
                    "TECH_Democracy",
                    Some(crate::rules::Tech::DEMOCRACY),
                    None,
                    &["ShieldToGold"],
                    (0, 0, 0, 1),
                ),
                gov(
                    "fascism",
                    "Fascism",
                    "TECH_Fascism",
                    Some(crate::rules::Tech::FASCISM),
                    None,
                    &[],
                    (4, 7, 10, 1),
                ),
                gov(
                    "feudalism",
                    "Feudalism",
                    "TECH_Feudalism",
                    Some(crate::rules::Tech::FEUDALISM),
                    None,
                    &[],
                    (5, 2, 1, 3),
                ),
            ],
        }
    })
}
