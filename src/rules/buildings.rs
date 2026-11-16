//! Stable building identifiers backed by the native TOML ruleset.
//!
//! The runtime IDs remain stable for saves and UI code; editable names, costs,
//! genera, and requirements are loaded from the CivCraft campaign package.
//!
//! Keep catalog indices and native ordering stable because saves and several
//! deterministic selection paths refer to these entries.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Building(pub(crate) usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Genus {
    Improvement,
    SmallWonder,
    GreatWonder,
    Special,
    Convert,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Requirement {
    Tech(usize),
    Improvement(usize),
    River,
    Lake,
    Sea,
    Water,
    MinSize(i32),
    OwnerCityCount(usize),
    OwnerBuildingCount(usize, usize),
    Resource(usize),
}

#[derive(Clone)]
pub(crate) struct BuildingData {
    pub(crate) name: &'static str,
    pub(crate) genus: Genus,
    pub(crate) cost: i32,
    pub(crate) upkeep: i32,
    pub(crate) requirements: &'static [(Requirement, bool)],
    pub(crate) obsolete: &'static [(Requirement, bool)],
}

const BUILDING_COUNT: usize = 91;

#[allow(dead_code)] // The catalog includes entries reserved for the Civ III ruleset.
impl Building {
    pub(crate) const SUPER_HIGHWAYS: Self = Self(38);
    pub(crate) const SUPERMARKET: Self = Self(39);
    pub(crate) const HARBOR: Self = Self(15);
    pub(crate) const FACTORY: Self = Self(13);
    pub(crate) const HYDRO_PLANT: Self = Self(16);
    pub(crate) const MASS_TRANSIT: Self = Self(19);
    pub(crate) const MFG_PLANT: Self = Self(20);
    pub(crate) const NUCLEAR_PLANT: Self = Self(21);
    pub(crate) const OFFSHORE_PLATFORM: Self = Self(22);
    pub(crate) const RECYCLING_CENTER: Self = Self(28);
    pub(crate) const SOLAR_PLANT: Self = Self(33);
    pub(crate) const EIFFEL_TOWER: Self = Self(48);
    pub(crate) const HOOVER_DAM: Self = Self(52);
    pub(crate) const MARCO_POLOS_EMBASSY: Self = Self(60);
    pub(crate) const LEONARDOS_WORKSHOP: Self = Self(56);
    pub(crate) const LIGHTHOUSE: Self = Self(57);
    pub(crate) const MAGELLANS_EXPEDITION: Self = Self(58);
    pub(crate) const PORT_FACILITY: Self = Self(26);
    pub(crate) const POWER_PLANT: Self = Self(27);
    pub(crate) const AIRPORT: Self = Self(0);
    pub(crate) const PALACE: Self = Self(23);
    pub(crate) const SPACE_STRUCTURAL: Self = Self(36);
    pub(crate) const SPACE_COMPONENT: Self = Self(34);
    pub(crate) const SPACE_MODULE: Self = Self(35);

    pub(crate) const A_SMITHS_TRADING_CO: Self = Self(43);
    pub(crate) const COPERNICUS_OBSERVATORY: Self = Self(45);
    pub(crate) const GREAT_LIBRARY: Self = Self(49);
    pub(crate) const ISAAC_NEWTONS_COLLEGE: Self = Self(53);
    pub(crate) const INTERNET: Self = Self(66);
    pub(crate) const APOLLO_PROGRAM: Self = Self(42);
    pub(crate) const AQUEDUCT: Self = Self(1);
    pub(crate) const AQUEDUCT_LAKE: Self = Self(2);
    pub(crate) const AQUEDUCT_RIVER: Self = Self(3);
    pub(crate) const BANK: Self = Self(4);
    pub(crate) const BARRACKS: Self = Self(5);
    pub(crate) const BARRACKS_II: Self = Self(6);
    pub(crate) const BARRACKS_III: Self = Self(7);
    pub(crate) const SUN_TZUS_WAR_ACADEMY: Self = Self(69);
    pub(crate) const AMPHITHEATER: Self = Self(11);
    pub(crate) const CATHEDRAL: Self = Self(8);
    pub(crate) const COLOSSUS: Self = Self(44);
    pub(crate) const JS_BACHS_CATHEDRAL: Self = Self(54);
    pub(crate) const HANGING_GARDENS: Self = Self(51);
    pub(crate) const MICHELANGELOS_CHAPEL: Self = Self(61);
    pub(crate) const MAUSOLEUM_OF_MAUSOLOS: Self = Self(62);
    pub(crate) const TEMPLE_OF_ARTEMIS: Self = Self(64);
    pub(crate) const CITY_WALLS: Self = Self(9);
    pub(crate) const COASTAL_DEFENSE: Self = Self(10);
    pub(crate) const SAM_BATTERY: Self = Self(30);
    pub(crate) const SDI_DEFENSE: Self = Self(31);
    pub(crate) const GREAT_WALL: Self = Self(50);
    pub(crate) const COURTHOUSE: Self = Self(12);
    pub(crate) const CURE_FOR_CANCER: Self = Self(46);
    pub(crate) const DARWINS_VOYAGE: Self = Self(47);
    pub(crate) const ECCLESIASTICAL_PALACE: Self = Self(24);
    pub(crate) const GRANARY: Self = Self(14);
    pub(crate) const KING_RICHARDS_CRUSADE: Self = Self(55);
    pub(crate) const LIBRARY: Self = Self(17);
    pub(crate) const MANHATTAN_PROJECT: Self = Self(59);
    pub(crate) const MARKETPLACE: Self = Self(18);
    pub(crate) const POLICE_STATION: Self = Self(25);
    pub(crate) const PYRAMIDS: Self = Self(65);
    pub(crate) const RESEARCH_LAB: Self = Self(29);
    pub(crate) const SEWER_SYSTEM: Self = Self(32);
    pub(crate) const SHAKESPEARES_THEATER: Self = Self(67);
    pub(crate) const STATUE_OF_LIBERTY: Self = Self(68);
    pub(crate) const STATUE_OF_ZEUS: Self = Self(63);
    pub(crate) const STOCK_EXCHANGE: Self = Self(37);
    pub(crate) const TEMPLE: Self = Self(40);
    pub(crate) const UNITED_NATIONS: Self = Self(70);
    pub(crate) const UNIVERSITY: Self = Self(41);
    pub(crate) const WOMENS_SUFFRAGE: Self = Self(71);
    pub(crate) const COLOSSEUM: Self = Self(73);
    pub(crate) const HOSPITAL: Self = Self(74);
    pub(crate) const LONGEVITY: Self = Self(75);
    pub(crate) const ORACLE: Self = Self(76);
    pub(crate) const THEORY_OF_EVOLUTION: Self = Self(77);
    pub(crate) const UNIVERSAL_SUFFRAGE: Self = Self(78);
    pub(crate) const SETI_PROGRAM: Self = Self(79);
    pub(crate) const FORBIDDEN_PALACE: Self = Self(80);
    pub(crate) const HEROIC_EPIC: Self = Self(81);
    pub(crate) const IRON_WORKS: Self = Self(82);
    pub(crate) const MILITARY_ACADEMY: Self = Self(83);
    pub(crate) const PENTAGON: Self = Self(84);
    pub(crate) const WALL_STREET: Self = Self(85);
    pub(crate) const INTELLIGENCE_AGENCY: Self = Self(86);
    pub(crate) const BATTLEFIELD_MEDICINE: Self = Self(87);
    pub(crate) const SISTINE_CHAPEL: Self = Self(88);
    pub(crate) const KNIGHTS_TEMPLAR: Self = Self(89);
    // Conquests BLDG[82], included because its headquarters counts as a culture-flip capital.
    pub(crate) const SECRET_POLICE_HQ: Self = Self(90);

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..BUILDING_COUNT).map(Self)
    }

    pub(crate) fn data(self) -> &'static BuildingData {
        static DATA: OnceLock<Vec<BuildingData>> = OnceLock::new();
        DATA.get_or_init(|| {
            let rules = crate::rules::native::buildings().rules.as_slice();
            assert_eq!(rules.len(), BUILDING_COUNT);
            rules.iter().map(native_building_data).collect()
        })
        .get(self.0)
        .expect("building runtime ID is present in native TOML")
    }
}

fn native_building_data(rule: &crate::rules::native::NativeBuildingRule) -> BuildingData {
    BuildingData {
        name: Box::leak(rule.name.clone().into_boxed_str()),
        genus: match rule.genus.as_str() {
            "Improvement" => Genus::Improvement,
            "SmallWonder" => Genus::SmallWonder,
            "GreatWonder" => Genus::GreatWonder,
            "Special" => Genus::Special,
            "Convert" => Genus::Convert,
            other => panic!("unknown native building genus {other:?}"),
        },
        cost: rule.cost,
        upkeep: rule.upkeep,
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
    let parsed = match requirement.kind.as_str() {
        "Tech" => Requirement::Tech(value),
        "Improvement" => Requirement::Improvement(value),
        "River" => Requirement::River,
        "Lake" => Requirement::Lake,
        "Sea" => Requirement::Sea,
        "Water" => Requirement::Water,
        "MinSize" => Requirement::MinSize(requirement.value),
        "OwnerCityCount" => Requirement::OwnerCityCount(value),
        "OwnerBuildingCount" => Requirement::OwnerBuildingCount(
            value,
            usize::try_from(requirement.second_value)
                .unwrap_or_else(|_| panic!("native building count is negative")),
        ),
        "Resource" => Requirement::Resource(value),
        other => panic!("unknown native building requirement {other:?}"),
    };
    (parsed, requirement.present)
}
