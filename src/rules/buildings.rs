//! Building identifiers and catalog data for the Civ2Civ3 ruleset.
//!
//! This module stores building names, costs, obsolescence, genera, and
//! requirement descriptions. It is data, not the resolver for construction or
//! effects: city production and simulation effect systems consume these
//! definitions. Requirements are represented explicitly so availability can
//! be checked against player technologies, government, and other rule state.
//!
//! Keep catalog indices and native ordering stable because saves and several
//! deterministic selection paths refer to these entries.

// Catalog from Freeciv data/civ2civ3/buildings.ruleset, revision eb8c7033.
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

pub(crate) struct BuildingData {
    pub(crate) name: &'static str,
    pub(crate) genus: Genus,
    pub(crate) cost: i32,
    pub(crate) upkeep: i32,
    pub(crate) requirements: &'static [(Requirement, bool)],
    pub(crate) obsolete: &'static [(Requirement, bool)],
}

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

    pub(crate) fn history_per_turn(self) -> i32 {
        match self.0 {
            8 => 3,  // Cathedral
            11 => 2, // Amphitheater (Colosseum)
            17 => 1, // Library
            23 => 1, // Palace
            24 => 1, // Ecclesiastical Palace
            29 => 3, // Research Lab
            40 => 1, // Temple
            41 => 2, // University
            42 => 6, // Apollo Program
            43 => 4, // A.Smith's Trading Co.
            44 => 2, // Colossus
            45 => 3, // Copernicus' Observatory
            46 => 6, // Cure For Cancer
            47 => 4, // Darwin's Voyage
            48 => 3, // Eiffel Tower
            49 => 3, // Great Library
            50 => 3, // Great Wall
            51 => 2, // Hanging Gardens
            52 => 6, // Hoover Dam
            53 => 4, // Isaac Newton's College
            54 => 4, // J.S. Bach's Cathedral
            55 => 3, // King Richard's Crusade
            56 => 4, // Leonardo's Workshop
            57 => 2, // Lighthouse
            58 => 4, // Magellan's Expedition
            59 => 6, // Manhattan Project
            60 => 2, // Marco Polo's Embassy
            61 => 4, // Michelangelo's Chapel
            62 => 2, // Mausoleum of Mausolos
            63 => 2, // Statue of Zeus
            64 => 2, // Temple of Artemis
            65 => 2, // Pyramids
            66 => 6, // Internet
            67 => 3, // Shakespeare's Theater
            68 => 4, // Statue of Liberty
            69 => 3, // Sun Tzu's War Academy
            70 => 6, // United Nations
            71 => 6, // Women's Suffrage
            75 => 3, // Longevity
            76 => 4, // Oracle
            77 => 3, // Theory of Evolution
            78 => 4, // Universal Suffrage
            79 => 3, // SETI Program
            80 => 3, // Forbidden Palace
            81 => 4, // Heroic Epic
            82 => 2, // Iron Works
            83 => 1, // Military Academy
            84 => 1, // Pentagon
            85 => 2, // Wall Street
            86 => 1, // Intelligence Agency
            87 => 1, // Battlefield Medicine
            _ => 0,
        }
    }

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..BUILDINGS.len()).map(Self)
    }

    pub(crate) fn data(self) -> &'static BuildingData {
        &BUILDINGS[self.0]
    }
}

use Requirement::*;
const BUILDINGS: &[BuildingData] = &[
    BuildingData {
        name: "Airport",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 3,
        requirements: &[(Tech(29), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Aqueduct",
        genus: Genus::Improvement,
        cost: 60,
        upkeep: 1,
        requirements: &[(Tech(18), true), (River, false), (Lake, false)],
        obsolete: &[],
    },
    BuildingData {
        name: "Aqueduct, Lake",
        genus: Genus::Improvement,
        cost: 20,
        upkeep: 0,
        requirements: &[(Lake, true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Aqueduct, River",
        genus: Genus::Improvement,
        cost: 20,
        upkeep: 0,
        requirements: &[(River, true), (Lake, false)],
        obsolete: &[],
    },
    BuildingData {
        name: "Bank",
        genus: Genus::Improvement,
        cost: 80,
        upkeep: 1,
        requirements: &[(Tech(6), true), (Improvement(18), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Barracks",
        genus: Genus::Improvement,
        cost: 30,
        upkeep: 1,
        requirements: &[],
        obsolete: &[
            (Tech(33), true),
            (Improvement(6), true),
            (Improvement(7), true),
        ],
    },
    BuildingData {
        name: "Barracks II",
        genus: Genus::Improvement,
        cost: 30,
        upkeep: 1,
        requirements: &[(Tech(33), true)],
        obsolete: &[(Tech(51), true), (Improvement(7), true)],
    },
    BuildingData {
        name: "Barracks III",
        genus: Genus::Improvement,
        cost: 30,
        upkeep: 1,
        requirements: &[(Tech(51), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Cathedral",
        genus: Genus::Improvement,
        cost: 80,
        upkeep: 3,
        requirements: &[(Tech(53), true), (Improvement(40), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "City Walls",
        genus: Genus::Improvement,
        cost: 30,
        upkeep: 0,
        requirements: &[(Tech(45), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Coastal Defense",
        genus: Genus::Improvement,
        cost: 60,
        upkeep: 1,
        requirements: &[(Tech(49), true), (Water, true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Amphitheater",
        genus: Genus::Improvement,
        cost: 70,
        upkeep: 3,
        requirements: &[(Tech(18), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Courthouse",
        genus: Genus::Improvement,
        cost: 60,
        upkeep: 1,
        requirements: &[(Tech(12), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Factory",
        genus: Genus::Improvement,
        cost: 140,
        upkeep: 3,
        requirements: &[(Tech(35), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Granary",
        genus: Genus::Improvement,
        cost: 40,
        upkeep: 1,
        requirements: &[(Tech(62), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Harbor",
        genus: Genus::Improvement,
        cost: 40,
        upkeep: 1,
        requirements: &[(Tech(44), true), (Sea, true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Hydro Plant",
        genus: Genus::Improvement,
        cost: 180,
        upkeep: 4,
        requirements: &[(Tech(23), true), (Improvement(13), true), (River, true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Library",
        genus: Genus::Improvement,
        cost: 60,
        upkeep: 1,
        requirements: &[(Tech(90), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Marketplace",
        genus: Genus::Improvement,
        cost: 60,
        upkeep: 0,
        requirements: &[(Tech(19), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Mass Transit",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 2,
        requirements: &[(Tech(25), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Mfg. Plant",
        genus: Genus::Improvement,
        cost: 220,
        upkeep: 4,
        requirements: &[(Tech(68), true), (Improvement(13), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Nuclear Plant",
        genus: Genus::Improvement,
        cost: 240,
        upkeep: 2,
        requirements: &[(Tech(57), true), (Improvement(13), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Offshore Platform",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 3,
        requirements: &[(Tech(50), true), (Water, true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Palace",
        genus: Genus::SmallWonder,
        cost: 70,
        upkeep: 0,
        requirements: &[],
        obsolete: &[],
    },
    BuildingData {
        name: "Ecclesiastical Palace",
        genus: Genus::SmallWonder,
        cost: 140,
        upkeep: 0,
        requirements: &[(Tech(81), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Police Station",
        genus: Genus::Improvement,
        cost: 50,
        upkeep: 2,
        requirements: &[(Tech(15), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Port Facility",
        genus: Genus::Improvement,
        cost: 60,
        upkeep: 2,
        requirements: &[(Tech(2), true), (Water, true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Power Plant",
        genus: Genus::Improvement,
        cost: 130,
        upkeep: 4,
        requirements: &[(Tech(35), true), (Improvement(13), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Recycling Center",
        genus: Genus::Improvement,
        cost: 140,
        upkeep: 2,
        requirements: &[(Tech(65), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Research Lab",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 3,
        requirements: &[(Tech(16), true), (Improvement(41), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "SAM Battery",
        genus: Genus::Improvement,
        cost: 70,
        upkeep: 2,
        requirements: &[(Tech(69), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "SDI Defense",
        genus: Genus::SmallWonder,
        cost: 140,
        upkeep: 3,
        requirements: &[(Tech(89), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Sewer System",
        genus: Genus::Improvement,
        cost: 80,
        upkeep: 2,
        requirements: &[(Tech(70), true), (MinSize(9), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Solar Plant",
        genus: Genus::Improvement,
        cost: 320,
        upkeep: 4,
        requirements: &[(Tech(25), true), (Improvement(13), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Space Component",
        genus: Genus::Special,
        cost: 300,
        upkeep: 0,
        requirements: &[
            (Tech(76), true),
            (Improvement(13), true),
            (Improvement(17), true),
            (Improvement(41), true),
            (Improvement(29), true),
        ],
        obsolete: &[],
    },
    BuildingData {
        name: "Space Module",
        genus: Genus::Special,
        cost: 600,
        upkeep: 0,
        requirements: &[
            (Tech(25), true),
            (Improvement(13), true),
            (Improvement(17), true),
            (Improvement(41), true),
            (Improvement(29), true),
        ],
        obsolete: &[],
    },
    BuildingData {
        name: "Space Structural",
        genus: Genus::Special,
        cost: 150,
        upkeep: 0,
        requirements: &[
            (Tech(72), true),
            (Improvement(13), true),
            (Improvement(17), true),
            (Improvement(41), true),
            (Improvement(29), true),
        ],
        obsolete: &[],
    },
    BuildingData {
        name: "Stock Exchange",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 2,
        requirements: &[(Tech(21), true), (Improvement(4), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Super Highways",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 3,
        requirements: &[(Tech(5), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Supermarket",
        genus: Genus::Improvement,
        cost: 80,
        upkeep: 3,
        requirements: &[(Tech(70), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Temple",
        genus: Genus::Improvement,
        cost: 30,
        upkeep: 1,
        requirements: &[(Tech(9), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "University",
        genus: Genus::Improvement,
        cost: 120,
        upkeep: 2,
        requirements: &[(Tech(84), true), (Improvement(17), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Apollo Program",
        genus: Genus::SmallWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(72), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "A.Smith's Trading Co.",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(21), true), (Improvement(37), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Colossus",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(8), true)],
        obsolete: &[(Tech(29), true)],
    },
    BuildingData {
        name: "Copernicus' Observatory",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(3), true)],
        obsolete: &[(Tech(16), true)],
    },
    BuildingData {
        name: "Cure For Cancer",
        genus: Genus::GreatWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(88), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Darwin's Voyage",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(74), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Eiffel Tower",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(64), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Great Library",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(90), true), (Improvement(17), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Great Wall",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(18), true)],
        obsolete: &[(Tech(49), true)],
    },
    BuildingData {
        name: "Hanging Gardens",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(52), true)],
        obsolete: &[(Tech(97), true)],
    },
    BuildingData {
        name: "Hoover Dam",
        genus: Genus::GreatWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(23), true), (River, true), (Improvement(13), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Isaac Newton's College",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(82), true), (Improvement(41), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "J.S. Bach's Cathedral",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(92), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "King Richard's Crusade",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(11), true)],
        obsolete: &[(Tech(15), true)],
    },
    BuildingData {
        name: "Leonardo's Workshop",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(36), true)],
        obsolete: &[(Tech(14), true)],
    },
    BuildingData {
        name: "Lighthouse",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(44), true), (Sea, true)],
        obsolete: &[(Tech(43), true)],
    },
    BuildingData {
        name: "Magellan's Expedition",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(55), true), (Sea, true)],
        obsolete: &[(Tech(57), true)],
    },
    BuildingData {
        name: "Manhattan Project",
        genus: Genus::GreatWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(56), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Marco Polo's Embassy",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(83), true)],
        obsolete: &[(Tech(20), true)],
    },
    BuildingData {
        name: "Michelangelo's Chapel",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(81), true), (Improvement(8), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Mausoleum of Mausolos",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(58), true)],
        obsolete: &[(Tech(70), true)],
    },
    BuildingData {
        name: "Statue of Zeus",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(47), true)],
        obsolete: &[(Tech(49), true)],
    },
    BuildingData {
        name: "Temple of Artemis",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(61), true), (Improvement(40), true)],
        obsolete: &[(Tech(84), true)],
    },
    BuildingData {
        name: "Pyramids",
        genus: Genus::GreatWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[(Tech(45), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Internet",
        genus: Genus::GreatWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(16), true), (Improvement(29), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Shakespeare's Theater",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(87), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Statue of Liberty",
        genus: Genus::GreatWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(20), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Sun Tzu's War Academy",
        genus: Genus::GreatWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Tech(28), true)],
        obsolete: &[(Tech(51), true)],
    },
    BuildingData {
        name: "United Nations",
        genus: Genus::GreatWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(56), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Women's Suffrage",
        genus: Genus::GreatWonder,
        cost: 600,
        upkeep: 0,
        requirements: &[(Tech(17), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Coinage",
        genus: Genus::Convert,
        cost: 999,
        upkeep: 0,
        requirements: &[],
        obsolete: &[],
    },
    BuildingData {
        name: "Colosseum",
        genus: Genus::Improvement,
        cost: 100,
        upkeep: 2,
        requirements: &[(Tech(18), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Hospital",
        genus: Genus::Improvement,
        cost: 160,
        upkeep: 2,
        requirements: &[(Tech(70), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Longevity",
        genus: Genus::GreatWonder,
        cost: 700,
        upkeep: 0,
        requirements: &[(Tech(88), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Oracle",
        genus: Genus::GreatWonder,
        cost: 260,
        upkeep: 0,
        requirements: &[(Tech(54), true)],
        obsolete: &[(Tech(81), true)],
    },
    BuildingData {
        name: "Theory of Evolution",
        genus: Genus::GreatWonder,
        cost: 550,
        upkeep: 0,
        requirements: &[(Tech(96), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Universal Suffrage",
        genus: Genus::GreatWonder,
        cost: 580,
        upkeep: 0,
        requirements: &[(Tech(35), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "SETI Program",
        genus: Genus::GreatWonder,
        cost: 700,
        upkeep: 0,
        requirements: &[(Tech(16), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Forbidden Palace",
        genus: Genus::SmallWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(OwnerCityCount(8), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Heroic Epic",
        genus: Genus::SmallWonder,
        cost: 200,
        upkeep: 0,
        requirements: &[],
        obsolete: &[],
    },
    BuildingData {
        name: "Iron Works",
        genus: Genus::SmallWonder,
        cost: 300,
        upkeep: 0,
        requirements: &[(Resource(1), true), (Resource(3), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Military Academy",
        genus: Genus::SmallWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(91), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Pentagon",
        genus: Genus::SmallWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[],
        obsolete: &[],
    },
    BuildingData {
        name: "Wall Street",
        genus: Genus::SmallWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(OwnerBuildingCount(4, 5), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Intelligence Agency",
        genus: Genus::SmallWonder,
        cost: 400,
        upkeep: 0,
        requirements: &[(Tech(26), true)],
        obsolete: &[],
    },
    BuildingData {
        name: "Battlefield Medicine",
        genus: Genus::SmallWonder,
        cost: 500,
        upkeep: 0,
        requirements: &[(OwnerBuildingCount(74, 5), true)],
        obsolete: &[],
    },
];
