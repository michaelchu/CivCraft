//! Stable building identifiers backed by the native TOML ruleset.
//!
//! Building identifiers backed by the native TOML ruleset, an exact clone of the
//! Civilization III: Conquests building roster. Constants below are positions in
//! `buildings.toml`; `index_constants_name_the_catalog_entry_at_their_index`
//! guards them. Saves are disposable during development.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Building(pub(crate) usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) enum Genus {
    Improvement,
    SmallWonder,
    GreatWonder,
    Special,
    Convert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Requirement {
    Tech(usize),
    Improvement(usize),
    River,
    Lake,
    Sea,
    Water,
    MinSize(i32),
    OwnerCityCount(usize),
    OwnerArmyCount(usize),
    OwnerBuildingCount(usize, usize),
    Resource(usize),
    /// The owner has at least one unit at the top level of its veteran system.
    OwnerEliteUnit,
}

#[derive(Clone)]
pub(crate) struct BuildingData {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) genus: Genus,
    pub(crate) cost: i32,
    pub(crate) upkeep: i32,
    pub(crate) culture_per_turn: i32,
    pub(crate) requirements: &'static [(Requirement, bool)],
    pub(crate) obsolete: &'static [(Requirement, bool)],
}

#[allow(dead_code)] // The catalog includes entries reserved for the Civ III ruleset.
impl Building {
    pub(crate) const HARBOR: Self = Self(10);
    pub(crate) const FACTORY: Self = Self(8);
    pub(crate) const HYDRO_PLANT: Self = Self(11);
    pub(crate) const MASS_TRANSIT: Self = Self(14);
    pub(crate) const MFG_PLANT: Self = Self(15);
    pub(crate) const NUCLEAR_PLANT: Self = Self(16);
    pub(crate) const OFFSHORE_PLATFORM: Self = Self(17);
    pub(crate) const RECYCLING_CENTER: Self = Self(21);
    pub(crate) const SOLAR_PLANT: Self = Self(25);
    pub(crate) const HOOVER_DAM: Self = Self(37);
    pub(crate) const LEONARDOS_WORKSHOP: Self = Self(40);
    pub(crate) const LIGHTHOUSE: Self = Self(41);
    pub(crate) const MAGELLANS_EXPEDITION: Self = Self(42);
    pub(crate) const POWER_PLANT: Self = Self(20);
    pub(crate) const AIRPORT: Self = Self(0);
    pub(crate) const PALACE: Self = Self(18);

    pub(crate) const A_SMITHS_TRADING_CO: Self = Self(30);
    pub(crate) const COPERNICUS_OBSERVATORY: Self = Self(32);
    pub(crate) const GREAT_LIBRARY: Self = Self(34);
    pub(crate) const ISAAC_NEWTONS_COLLEGE: Self = Self(38);
    pub(crate) const INTERNET: Self = Self(48);
    pub(crate) const APOLLO_PROGRAM: Self = Self(29);
    pub(crate) const AQUEDUCT: Self = Self(1);
    pub(crate) const BANK: Self = Self(2);
    pub(crate) const BARRACKS: Self = Self(3);
    pub(crate) const SUN_TZUS_WAR_ACADEMY: Self = Self(50);
    pub(crate) const CATHEDRAL: Self = Self(4);
    pub(crate) const COLOSSUS: Self = Self(31);
    pub(crate) const JS_BACHS_CATHEDRAL: Self = Self(39);
    pub(crate) const HANGING_GARDENS: Self = Self(36);
    pub(crate) const MAUSOLEUM_OF_MAUSOLOS: Self = Self(44);
    pub(crate) const TEMPLE_OF_ARTEMIS: Self = Self(46);
    pub(crate) const CITY_WALLS: Self = Self(5);
    pub(crate) const COASTAL_DEFENSE: Self = Self(6);
    pub(crate) const SAM_BATTERY: Self = Self(23);
    pub(crate) const SDI_DEFENSE: Self = Self(24);
    pub(crate) const GREAT_WALL: Self = Self(35);
    pub(crate) const COURTHOUSE: Self = Self(7);
    pub(crate) const CURE_FOR_CANCER: Self = Self(33);
    pub(crate) const GRANARY: Self = Self(9);
    pub(crate) const LIBRARY: Self = Self(12);
    pub(crate) const MANHATTAN_PROJECT: Self = Self(43);
    pub(crate) const MARKETPLACE: Self = Self(13);
    pub(crate) const POLICE_STATION: Self = Self(19);
    pub(crate) const PYRAMIDS: Self = Self(47);
    pub(crate) const RESEARCH_LAB: Self = Self(22);
    pub(crate) const SHAKESPEARES_THEATER: Self = Self(49);
    pub(crate) const STATUE_OF_ZEUS: Self = Self(45);
    pub(crate) const STOCK_EXCHANGE: Self = Self(26);
    pub(crate) const TEMPLE: Self = Self(27);
    pub(crate) const COINAGE: Self = Self(52);
    pub(crate) const UNITED_NATIONS: Self = Self(51);
    pub(crate) const UNIVERSITY: Self = Self(28);
    pub(crate) const COLOSSEUM: Self = Self(53);
    pub(crate) const HOSPITAL: Self = Self(54);
    pub(crate) const LONGEVITY: Self = Self(55);
    pub(crate) const ORACLE: Self = Self(56);
    pub(crate) const THEORY_OF_EVOLUTION: Self = Self(57);
    pub(crate) const UNIVERSAL_SUFFRAGE: Self = Self(58);
    pub(crate) const SETI_PROGRAM: Self = Self(59);
    pub(crate) const FORBIDDEN_PALACE: Self = Self(60);
    pub(crate) const HEROIC_EPIC: Self = Self(61);
    pub(crate) const IRON_WORKS: Self = Self(62);
    pub(crate) const MILITARY_ACADEMY: Self = Self(63);
    pub(crate) const PENTAGON: Self = Self(64);
    pub(crate) const WALL_STREET: Self = Self(65);
    pub(crate) const INTELLIGENCE_AGENCY: Self = Self(66);
    pub(crate) const BATTLEFIELD_MEDICINE: Self = Self(67);
    pub(crate) const SISTINE_CHAPEL: Self = Self(68);
    pub(crate) const KNIGHTS_TEMPLAR: Self = Self(69);
    // Conquests BLDG[82], included because its headquarters counts as a culture-flip capital.
    pub(crate) const SECRET_POLICE_HQ: Self = Self(70);
    pub(crate) const SS_THRUSTERS: Self = Self(71);
    pub(crate) const SS_ENGINE: Self = Self(72);
    pub(crate) const SS_DOCKING_BAY: Self = Self(73);
    pub(crate) const SS_COCKPIT: Self = Self(74);
    pub(crate) const SS_FUEL_CELLS: Self = Self(75);
    pub(crate) const SS_LIFE_SUPPORT_SYSTEM: Self = Self(76);
    pub(crate) const SS_STASIS_CHAMBER: Self = Self(77);
    pub(crate) const SS_STORAGE_SUPPLY: Self = Self(78);
    pub(crate) const SS_PLANETARY_PARTY_LOUNGE: Self = Self(79);
    pub(crate) const SS_EXTERIOR_CASING: Self = Self(80);
    pub(crate) const CIVIL_DEFENSE: Self = Self(81);
    pub(crate) const COMMERCIAL_DOCK: Self = Self(82);

    pub(crate) fn count() -> usize {
        crate::rules::native::buildings().rules.len()
    }

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..Self::count()).map(Self)
    }

    pub(crate) fn id(self) -> &'static str {
        self.data().id
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::all().find(|building| building.data().id == id)
    }

    pub(crate) fn data(self) -> &'static BuildingData {
        static DATA: OnceLock<Vec<BuildingData>> = OnceLock::new();
        DATA.get_or_init(|| {
            crate::rules::native::buildings()
                .rules
                .iter()
                .map(native_building_data)
                .collect()
        })
        .get(self.0)
        .expect("building runtime ID is present in native TOML")
    }
}

fn native_building_data(rule: &crate::rules::native::NativeBuildingRule) -> BuildingData {
    BuildingData {
        id: Box::leak(rule.id.clone().into_boxed_str()),
        name: Box::leak(rule.name.clone().into_boxed_str()),
        genus: rule.genus,
        cost: rule.cost,
        upkeep: rule.upkeep,
        culture_per_turn: rule.culture_per_turn,
        requirements: Box::leak(
            rule.requirements
                .iter()
                .map(native_building_requirement)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        obsolete: Box::leak(
            rule.obsolete
                .iter()
                .map(native_building_requirement)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    }
}

fn native_building_requirement(
    requirement: &crate::rules::native::NativeBuildingRequirement,
) -> (Requirement, bool) {
    let value = usize::try_from(requirement.value)
        .unwrap_or_else(|_| panic!("native building requirement value is negative"));
    use crate::rules::native::NativeRequirementKind as Kind;
    let parsed = match requirement.kind {
        Kind::Tech => Requirement::Tech(value),
        Kind::Improvement => Requirement::Improvement(value),
        Kind::River => Requirement::River,
        Kind::Lake => Requirement::Lake,
        Kind::Sea => Requirement::Sea,
        Kind::Water => Requirement::Water,
        Kind::MinSize => Requirement::MinSize(requirement.value),
        Kind::OwnerCityCount => Requirement::OwnerCityCount(value),
        Kind::OwnerArmyCount => Requirement::OwnerArmyCount(value),
        Kind::OwnerBuildingCount => Requirement::OwnerBuildingCount(
            value,
            usize::try_from(requirement.second_value)
                .unwrap_or_else(|_| panic!("native building count is negative")),
        ),
        Kind::Resource => Requirement::Resource(value),
        Kind::OwnerEliteUnit => Requirement::OwnerEliteUnit,
    };
    (parsed, requirement.present)
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn building_ids_and_well_known_handles_roundtrip_with_catalog() {
        assert_eq!(Building::all().count(), Building::count());
        for building in Building::all() {
            assert_eq!(Building::from_id(building.id()), Some(building));
        }
        assert_eq!(Building::from_id("building_palace"), Some(Building::PALACE));
        assert_eq!(
            Building::from_id("building_pentagon"),
            Some(Building::PENTAGON)
        );
        assert_eq!(
            Building::from_id("building_secret_police_hq"),
            Some(Building::SECRET_POLICE_HQ)
        );
        assert_eq!(Building::from_id("building_nonexistent"), None);
    }
}

#[cfg(test)]
mod constant_tests {
    use super::*;

    fn normalized(text: &str) -> String {
        text.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect()
    }

    /// Every `const NAME: Self = Self(N);` must still name the entry that the
    /// native catalog holds at index N, so reordering the TOML cannot silently
    /// retarget a hand-written constant.
    #[test]
    fn index_constants_name_the_catalog_entry_at_their_index() {
        let source = include_str!("buildings.rs");
        let mut checked = 0;
        for line in source.lines() {
            let Some(rest) = line.trim().strip_prefix("pub(crate) const ") else {
                continue;
            };
            let Some((name, tail)) = rest.split_once(": Self = Self(") else {
                continue;
            };
            let Some(index) = tail
                .strip_suffix(");")
                .and_then(|n| n.parse::<usize>().ok())
            else {
                continue;
            };
            let data = Building(index).data();
            let id = data.id.strip_prefix("building_").unwrap_or(data.id);
            let expected = normalized(name);
            assert!(
                expected == normalized(data.name) || expected == normalized(id),
                "{name} = Self({index}) but index {index} is {} ({})",
                data.name,
                data.id
            );
            checked += 1;
        }
        assert!(checked >= 80, "only checked {checked} building constants");
    }
}
