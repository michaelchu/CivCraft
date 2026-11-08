//! Fixed six-civilization roster. Identity is presentation data: gameplay keeps
//! the Civ2Civ3 rules, existing player slots, colors, starts and random stream.
//! City lists use original Civ III names (including its historical spellings).
//! Sources: <https://civilization.fandom.com/wiki/Category:Cities_(Civ3)>
//! <https://civfanatics.com/civ3/infocenter/>
pub(crate) struct Civilization {
    pub(crate) name: &'static str,
    pub(crate) leader: &'static str,
    pub(crate) cities: &'static [&'static str], // Capital first.
    pub(crate) city_style: &'static str,
    pub(crate) legacy_name: &'static str,
}

pub(crate) const CIVILIZATIONS: [Civilization; 6] = [
    Civilization {
        name: "Romans",
        leader: "Julius Caesar",
        city_style: "Classical",
        legacy_name: "Crimson League",
        cities: &[
            "Rome",
            "Veii",
            "Antium",
            "Cumae",
            "Neapolis",
            "Pompeii",
            "Pisae",
            "Ravenna",
            "Hispalis",
            "Viroconium",
            "Lugdunum",
            "Lutetia",
            "Byzantium",
            "Brundisium",
            "Syracuse",
            "Caesaraugusta",
            "Palmyra",
            "Jerusalem",
            "Caesarea",
            "Tarentum",
            "Nicomedia",
            "Seleucia",
            "Artaxata",
            "Aurelianorum",
            "Hippo Regius",
            "Nicopolis",
            "Londinium",
            "Eburacum",
            "Arretium",
            "Gordion",
            "Agrippina",
            "Cyrene",
            "Tyrus",
            "Verona",
            "Corfinium",
            "Mediolanum",
            "Treveri",
            "Sirmium",
            "Augustadorum",
            "Trapezus",
            "Bagacum",
            "Lauriacum",
            "Teurnia",
            "Curia",
        ],
    },
    Civilization {
        name: "Greeks",
        leader: "Alexander",
        city_style: "Classical",
        legacy_name: "Golden Compact",
        cities: &[
            "Athens",
            "Sparta",
            "Thermopylae",
            "Corinth",
            "Delphi",
            "Pharsalos",
            "Knossos",
            "Argos",
            "Mycenae",
            "Herakleia",
            "Ephesus",
            "Thessalonica",
            "Rhodes",
            "Eretria",
            "Troy",
            "Marathon",
            "Halicarnassus",
            "Pergamon",
            "Miletos",
            "Artemisium",
            "Megara",
            "Phocaea",
            "Sicyon",
            "Gortyn",
            "Mytilene",
            "Tegea",
            "Syracuse",
            "Apollonia",
            "Apolyton",
        ],
    },
    Civilization {
        name: "Egyptians",
        leader: "Cleopatra",
        city_style: "Babylonian",
        legacy_name: "Azure Assembly",
        cities: &[
            "Thebes",
            "Memphis",
            "Heliopolis",
            "Elephantine",
            "Alexandria",
            "Pi-Ramesses",
            "Giza",
            "Byblos",
            "El-Amarna",
            "Hieraconpolis",
            "Abydos",
            "Asyut",
            "Avaris",
            "Lisht",
            "Buto",
            "Edfu",
            "Pithom",
            "Busiris",
            "Kahun",
            "Athribis",
            "Mendes",
            "Elashmunein",
            "Tanis",
            "Bubastis",
            "Oryx",
            "Sebennytus",
            "Akhmin",
            "Khaty",
            "Tehneh",
            "Siut",
            "Damanhur",
            "Telloh",
            "Dendera",
            "Iseum",
        ],
    },
    Civilization {
        name: "Babylonians",
        leader: "Hammurabi",
        city_style: "Babylonian",
        legacy_name: "Violet Court",
        // Original release roster, before Conquests moved Sumerian names.
        cities: &[
            "Babylon",
            "Ur",
            "Nineveh",
            "Ashur",
            "Ellipi",
            "Akkad",
            "Uruk",
            "Eridu",
            "Samarra",
            "Lagash",
            "Kish",
            "Nippur",
            "Shuruppak",
            "Zariqum",
            "Sippar",
            "Izibia",
            "Larsa",
            "Nimrud",
            "Zamua",
            "Khorsabad",
            "Hindana",
            "Tell Wilaya",
            "Umma",
            "Adab",
            "Telloh",
            "Nina",
            "Ebla",
        ],
    },
    Civilization {
        name: "Chinese",
        leader: "Mao Zedong",
        city_style: "Asian",
        legacy_name: "Amber Union",
        cities: &[
            "Beijing", "Shanghai", "Canton", "Nanking", "Tsingtao", "Xinjian", "Chengdu",
            "Hangchow", "Tientsin", "Tatung", "Macao", "Anyang", "Shantung", "Chinan", "Kaifeng",
            "Ningpo", "Paoting", "Yangchow",
        ],
    },
    Civilization {
        name: "Japanese",
        leader: "Tokugawa",
        city_style: "Asian",
        legacy_name: "Rose Confederacy",
        cities: &[
            "Kyoto",
            "Osaka",
            "Tokyo",
            "Edo",
            "Satsuma",
            "Kagoshima",
            "Nara",
            "Nagoya",
            "Izumo",
            "Nagasaki",
            "Yokohama",
            "Shimonoseki",
            "Matsuyama",
            "Sapporo",
            "Hakodate",
            "Ise",
            "Toyama",
            "Fukushima",
            "Suo",
            "Bizen",
            "Echizen",
            "Izumi",
            "Omi",
            "Echigo",
            "Kozuke",
            "Sado",
        ],
    },
];

pub(crate) fn next_city_name(owner: usize, occupied: impl Fn(&str) -> bool) -> Option<String> {
    let civ = CIVILIZATIONS.get(owner)?;
    for &name in civ.cities {
        if !occupied(name) {
            return Some(name.to_owned());
        }
    }
    // Deterministic unlimited overflow, also checking captured/renamed cities.
    for generation in 1.. {
        for &name in civ.cities {
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
    fn roster_has_six_distinct_leaders_and_capitals() {
        for (index, civ) in CIVILIZATIONS.iter().enumerate() {
            assert!(!civ.leader.is_empty());
            assert_eq!(
                next_city_name(index, |_| false).as_deref(),
                Some(civ.cities[0])
            );
            for prior in &CIVILIZATIONS[..index] {
                assert_ne!(civ.name, prior.name);
                assert_ne!(civ.leader, prior.leader);
                assert_ne!(civ.cities[0], prior.cities[0]);
            }
            let unique: std::collections::HashSet<_> = civ.cities.iter().collect();
            assert_eq!(unique.len(), civ.cities.len());
        }
    }
    #[test]
    fn city_lists_skip_used_names_and_overflow_without_duplicates() {
        assert_eq!(
            next_city_name(0, |n| n == "Rome" || n == "Antium").as_deref(),
            Some("Veii")
        );
        let names = CIVILIZATIONS[0].cities;
        assert_eq!(
            next_city_name(0, |n| names.contains(&n)).as_deref(),
            Some("New Rome")
        );
        assert_eq!(
            next_city_name(0, |n| names.contains(&n) || n == "New Rome").as_deref(),
            Some("New Veii")
        );
        assert!(next_city_name(6, |_| false).is_none());
    }
}
