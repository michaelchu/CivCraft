//! Technology identifiers and the ordered technology catalog.
//!
//! Each advance records its name, prerequisites, cost inputs, and unlock
//! metadata. Research eligibility and progress are resolved in `sim::research`
//! and `sim::research_phase`; this module supplies the static graph those
//! systems query. Prerequisites may be alternative paths, so callers should
//! use the catalog's requirement helpers instead of assuming every listed
//! prerequisite is mandatory.
//!
//! Catalog identity and order are used by saved player knowledge and UI/AI
//! iteration. Preserve them when editing the ruleset data.

// Catalog transcribed from Freeciv data/civ2civ3/techs.ruleset.
// Reference unlock labels come from the unit and building rulesets.

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Tech(pub(crate) usize);

pub(crate) struct Advance {
    pub(crate) name: &'static str,
    pub(crate) prerequisites: &'static [Tech],
    pub(crate) requirement_count: i32,
    pub(crate) first_discovery_bonus: bool,
    pub(crate) reference_unlocks: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TechEra {
    Ancient,
    MiddleAges,
    Industrial,
    Modern,
}

impl TechEra {
    pub(crate) const ALL: [Self; 4] = [
        Self::Ancient,
        Self::MiddleAges,
        Self::Industrial,
        Self::Modern,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ancient => "Ancient Times",
            Self::MiddleAges => "Middle Ages",
            Self::Industrial => "Industrial Age",
            Self::Modern => "Modern Times",
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Self::Ancient => 0,
            Self::MiddleAges => 1,
            Self::Industrial => 2,
            Self::Modern => 3,
        }
    }
}

impl Tech {
    pub(crate) const FREE_ARTISTRY: Self = Self(87);
    pub(crate) const GENETICS: Self = Self(88);
    pub(crate) const INTEGRATED_DEFENSE: Self = Self(89);
    pub(crate) const LITERATURE: Self = Self(90);
    pub(crate) const MILITARY_TRADITION: Self = Self(91);
    pub(crate) const ROBOTICS: Self = Self(68);
    pub(crate) const SPACE_FLIGHT: Self = Self(72);
    pub(crate) const MUSIC_THEORY: Self = Self(92);
    pub(crate) const PRINTING_PRESS: Self = Self(93);
    pub(crate) const REPLACEABLE_PARTS: Self = Self(94);
    pub(crate) const SATELLITES: Self = Self(95);
    pub(crate) const SCIENTIFIC_METHOD: Self = Self(96);
    pub(crate) const STEAM_POWER: Self = Self(97);
    pub(crate) const SMART_WEAPONS: Self = Self(98);

    // Civ III names for existing CivCraft advances with close equivalents.
    pub(crate) const EDUCATION: Self = Self::UNIVERSITY;
    pub(crate) const NATIONALISM: Self = Self(17);
    pub(crate) const MOTORIZED_TRANSPORTATION: Self = Self::MOBILE_WARFARE;
    pub(crate) const ECOLOGY: Self = Self(25);
    pub(crate) const FISSION: Self = Self::NUCLEAR_FISSION;
    pub(crate) const THE_LASER: Self = Self(39);
    pub(crate) const SUPERCONDUCTOR: Self = Self::SUPERCONDUCTORS;
    pub(crate) const SYNTHETIC_FIBERS: Self = Self(60);

    pub(crate) const STEALTH: Self = Self(73);
    pub(crate) const COMBUSTION: Self = Self(14);
    pub(crate) const COMBINED_ARMS: Self = Self(13);
    pub(crate) const MASS_PRODUCTION: Self = Self(46);
    pub(crate) const STEEL: Self = Self(75);
    pub(crate) const FLIGHT: Self = Self(29);
    pub(crate) const ADVANCED_FLIGHT: Self = Self(0);
    pub(crate) const ASTRONOMY: Self = Self(3);
    pub(crate) const CONSTRUCTION: Self = Self(18);
    pub(crate) const RADIO: Self = Self(63);
    pub(crate) const AMPHIBIOUS_WARFARE: Self = Self(2);
    pub(crate) const ALPHABET: Self = Self(1);
    pub(crate) const BRIDGE_BUILDING: Self = Self(7);
    pub(crate) const BRONZE_WORKING: Self = Self(8);
    pub(crate) const WARRIOR_CODE: Self = Self(85);
    pub(crate) const HORSEBACK_RIDING: Self = Self(34);
    pub(crate) const INDUSTRIALIZATION: Self = Self(35);
    pub(crate) const POTTERY: Self = Self(62);
    pub(crate) const IRON_WORKING: Self = Self(37);
    pub(crate) const THE_WHEEL: Self = Self(80);
    pub(crate) const MAP_MAKING: Self = Self(44);
    pub(crate) const MASONRY: Self = Self(45);
    pub(crate) const MAGNETISM: Self = Self(43);
    pub(crate) const NAVIGATION: Self = Self(55);
    pub(crate) const MEDICINE: Self = Self(48);
    pub(crate) const MATHEMATICS: Self = Self(47);
    pub(crate) const UNIVERSITY: Self = Self(84);
    pub(crate) const RAILROAD: Self = Self(64);
    pub(crate) const AUTOMOBILE: Self = Self(5);
    pub(crate) const ROCKETRY: Self = Self(69);
    pub(crate) const SUPERCONDUCTORS: Self = Self(76);
    pub(crate) const CHIVALRY: Self = Self(11);
    pub(crate) const GUNPOWDER: Self = Self(33);
    pub(crate) const GUERILLA_WARFARE: Self = Self(32);
    pub(crate) const MACHINE_TOOLS: Self = Self(42);
    pub(crate) const METALLURGY: Self = Self(49);
    pub(crate) const FUTURE: Self = Self(ADVANCES.len());
    // Keep all persisted real/Future indices stable. A_UNSET is a goal state,
    // never a researchable/discoverable advance.
    pub(crate) const UNSET: Self = Self(ADVANCES.len() + 1);

    pub(crate) const COMPUTERS: Self = Self(16);
    pub(crate) const COMMUNISM: Self = Self(15);
    pub(crate) const DEMOCRACY: Self = Self(20);
    pub(crate) const MOBILE_WARFARE: Self = Self(51);
    pub(crate) const MINIATURIZATION: Self = Self(50);
    pub(crate) const REFINING: Self = Self(66);
    pub(crate) const STEAM_ENGINE: Self = Self(74);
    pub(crate) const ENGINEERING: Self = Self(24);
    pub(crate) const INVENTION: Self = Self(36);
    pub(crate) const ESPIONAGE: Self = Self(26);
    pub(crate) const EXPLOSIVES: Self = Self(27);
    pub(crate) const FUSION_POWER: Self = Self(30);
    pub(crate) const ELECTRICITY: Self = Self(22);
    pub(crate) const REFRIGERATION: Self = Self(67);
    pub(crate) const ECONOMICS: Self = Self(21);
    pub(crate) const MONARCHY: Self = Self(52);
    pub(crate) const MYSTICISM: Self = Self(54);
    pub(crate) const NUCLEAR_POWER: Self = Self(57);
    pub(crate) const SANITATION: Self = Self(70);
    pub(crate) const NUCLEAR_FISSION: Self = Self(56);
    pub(crate) const THE_CORPORATION: Self = Self(78);
    pub(crate) const THE_REPUBLIC: Self = Self(79);
    pub(crate) const THEOLOGY: Self = Self(81);
    pub(crate) const TRADE: Self = Self(83);

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..ADVANCES.len()).map(Self)
    }

    pub(crate) fn data(self) -> Option<&'static Advance> {
        ADVANCES.get(self.0)
    }

    pub(crate) fn name(self) -> &'static str {
        if self == Self::UNSET {
            "No goal"
        } else {
            self.data().map_or("Future Tech", |advance| advance.name)
        }
    }

    pub(crate) fn prerequisites(self) -> &'static [Self] {
        self.data().map_or(&[], |advance| advance.prerequisites)
    }

    pub(crate) fn era(self) -> TechEra {
        use TechEra::*;
        if self == Self::FUTURE {
            return Modern;
        }
        match self.name() {
            "Alphabet" | "Bronze Working" | "Ceremonial Burial" | "Code of Laws"
            | "Construction" | "Currency" | "Horseback Riding" | "Iron Working" | "Literature"
            | "Map Making" | "Masonry" | "Mathematics" | "Monarchy" | "Mysticism"
            | "Philosophy" | "Polytheism" | "Pottery" | "The Republic" | "Warrior Code"
            | "The Wheel" | "Writing" | "Bridge Building" | "Literacy" | "Trade" => Ancient,
            "Astronomy" | "Banking" | "Chemistry" | "Chivalry" | "Democracy" | "Economics"
            | "Education" | "Engineering" | "Feudalism" | "Free Artistry" | "Gunpowder"
            | "Invention" | "Magnetism" | "Metallurgy" | "Military Tradition" | "Monotheism"
            | "Music Theory" | "Navigation" | "Physics" | "Printing Press" | "Theology"
            | "Theory of Gravity" | "Explosives" | "Leadership" => MiddleAges,
            "Advanced Flight"
            | "Amphibious Warfare"
            | "Atomic Theory"
            | "Combustion"
            | "Communism"
            | "The Corporation"
            | "Electricity"
            | "Electronics"
            | "Espionage"
            | "Flight"
            | "Industrialization"
            | "Mass Production"
            | "Medicine"
            | "Motorized Transportation"
            | "Nationalism"
            | "Radio"
            | "Refining"
            | "Replaceable Parts"
            | "Sanitation"
            | "Scientific Method"
            | "Steam Power"
            | "Steel"
            | "Automobile"
            | "Combined Arms"
            | "Guerilla Warfare"
            | "Machine Tools"
            | "Railroad"
            | "Refrigeration"
            | "Seafaring"
            | "Steam Engine"
            | "Tactics" => Industrial,
            "Computers"
            | "Ecology"
            | "Fission"
            | "Genetics"
            | "Integrated Defense"
            | "The Laser"
            | "Miniaturization"
            | "Nuclear Power"
            | "Recycling"
            | "Robotics"
            | "Rocketry"
            | "Satellites"
            | "Smart Weapons"
            | "Space Flight"
            | "Stealth"
            | "Superconductor"
            | "Synthetic Fibers"
            | "Fusion Power"
            | "Genetic Engineering"
            | "Labor Union" => Modern,
            _ => Ancient,
        }
    }

    pub(crate) fn is_civ3_advance(self) -> bool {
        matches!(
            self.era(),
            TechEra::Ancient | TechEra::MiddleAges | TechEra::Industrial | TechEra::Modern
        ) && matches!(
            self.name(),
            "Advanced Flight"
                | "Alphabet"
                | "Amphibious Warfare"
                | "Astronomy"
                | "Atomic Theory"
                | "Banking"
                | "Bronze Working"
                | "Ceremonial Burial"
                | "Chemistry"
                | "Chivalry"
                | "Code of Laws"
                | "Combustion"
                | "Communism"
                | "Computers"
                | "Construction"
                | "Currency"
                | "Democracy"
                | "Economics"
                | "Ecology"
                | "Education"
                | "Electricity"
                | "Electronics"
                | "Engineering"
                | "Espionage"
                | "Feudalism"
                | "Fission"
                | "Flight"
                | "Free Artistry"
                | "Genetics"
                | "Gunpowder"
                | "Horseback Riding"
                | "Industrialization"
                | "Invention"
                | "Integrated Defense"
                | "Iron Working"
                | "Literature"
                | "Magnetism"
                | "Map Making"
                | "Masonry"
                | "Mass Production"
                | "Mathematics"
                | "Medicine"
                | "Metallurgy"
                | "Military Tradition"
                | "Miniaturization"
                | "Monarchy"
                | "Monotheism"
                | "Music Theory"
                | "Motorized Transportation"
                | "Mysticism"
                | "Nationalism"
                | "Navigation"
                | "Nuclear Power"
                | "Physics"
                | "Philosophy"
                | "Polytheism"
                | "Pottery"
                | "Printing Press"
                | "Radio"
                | "Recycling"
                | "Refining"
                | "Replaceable Parts"
                | "Robotics"
                | "Rocketry"
                | "Sanitation"
                | "Satellites"
                | "Scientific Method"
                | "Smart Weapons"
                | "Space Flight"
                | "Stealth"
                | "Steel"
                | "Steam Power"
                | "Superconductor"
                | "Synthetic Fibers"
                | "The Corporation"
                | "The Laser"
                | "The Republic"
                | "The Wheel"
                | "Theology"
                | "Theory of Gravity"
                | "Warrior Code"
                | "Writing"
        )
    }

    /// CivCraft-only advances retained for save compatibility and legacy
    /// effects. Their effects now ride on the closest Civ III advance and
    /// they are not separate research choices.
    pub(crate) fn civ3_equivalent(self) -> Option<Self> {
        match self {
            Self::AUTOMOBILE => Some(Self::MOTORIZED_TRANSPORTATION),
            Self::BRIDGE_BUILDING | Self::EXPLOSIVES => Some(Self::ENGINEERING),
            Self::COMBINED_ARMS => Some(Self::ADVANCED_FLIGHT),
            Self::FUSION_POWER => Some(Self::NUCLEAR_POWER),
            Self(31) => Some(Self::GENETICS), // Genetic Engineering
            Self::GUERILLA_WARFARE => Some(Self::NATIONALISM),
            Self(38) => Some(Self::REPLACEABLE_PARTS), // Labor Union
            Self(40) => Some(Self::MILITARY_TRADITION), // Leadership
            Self(41) => Some(Self::LITERATURE),        // Literacy
            Self::MACHINE_TOOLS => Some(Self::ROBOTICS),
            Self::RAILROAD | Self::STEAM_ENGINE => Some(Self::STEAM_POWER),
            Self::REFRIGERATION => Some(Self::SANITATION),
            Self(71) => Some(Self::NAVIGATION), // Seafaring
            Self(77) => Some(Self::MILITARY_TRADITION), // Tactics
            Self::TRADE => Some(Self::ECONOMICS),
            _ => None,
        }
    }

    pub(crate) fn optional_in_civ3(self) -> bool {
        matches!(
            self.name(),
            "Horseback Riding"
                | "Literature"
                | "Monarchy"
                | "The Republic"
                | "Chivalry"
                | "Democracy"
                | "Economics"
                | "Free Artistry"
                | "Military Tradition"
                | "Music Theory"
                | "Navigation"
                | "Printing Press"
                | "Advanced Flight"
                | "Amphibious Warfare"
                | "Communism"
                | "Espionage"
                | "Nationalism"
                | "Sanitation"
        )
    }

    pub(crate) fn required_for_civ3_era(self) -> bool {
        self.is_civ3_advance() && !self.optional_in_civ3()
    }

    pub(crate) fn era_gate(self) -> Option<TechEra> {
        match self.name() {
            "Engineering" | "Feudalism" | "Monotheism" => Some(TechEra::Ancient),
            "Medicine" | "Nationalism" | "Steam Power" => Some(TechEra::MiddleAges),
            "Computers" | "Ecology" | "Fission" | "Rocketry" => Some(TechEra::Industrial),
            _ => None,
        }
    }
}

const ADVANCES: &[Advance] = &[
    Advance {
        name: "Advanced Flight",
        prerequisites: &[Tech(29), Tech(63), Tech(51)],
        requirement_count: 57,
        first_discovery_bonus: false,
        reference_unlocks: "Bomber, Carrier, AWACS",
    },
    Advance {
        name: "Alphabet",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Diplomat",
    },
    Advance {
        name: "Amphibious Warfare",
        prerequisites: &[Tech(46)],
        requirement_count: 45,
        first_discovery_bonus: false,
        reference_unlocks: "Marines, Port Facility",
    },
    Advance {
        name: "Astronomy",
        prerequisites: &[Tech(84)],
        requirement_count: 6,
        first_discovery_bonus: false,
        reference_unlocks: "Copernicus' Observatory",
    },
    Advance {
        name: "Atomic Theory",
        prerequisites: &[Tech(96)],
        requirement_count: 37,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Automobile",
        prerequisites: &[Tech(14), Tech(75)],
        requirement_count: 46,
        first_discovery_bonus: false,
        reference_unlocks: "Super Highways",
    },
    Advance {
        name: "Banking",
        prerequisites: &[Tech(84)],
        requirement_count: 10,
        first_discovery_bonus: false,
        reference_unlocks: "Bank",
    },
    Advance {
        name: "Bridge Building",
        prerequisites: &[Tech(80), Tech(18)],
        requirement_count: 8,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Bronze Working",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Phalanx, Colossus",
    },
    Advance {
        name: "Ceremonial Burial",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Temple, Mausoleum of Mausolos",
    },
    Advance {
        name: "Chemistry",
        prerequisites: &[Tech(33)],
        requirement_count: 16,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Chivalry",
        prerequisites: &[Tech(53), Tech(28)],
        requirement_count: 10,
        first_discovery_bonus: false,
        reference_unlocks: "Knights, King Richard's Crusade",
    },
    Advance {
        name: "Code of Laws",
        prerequisites: &[Tech(86)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Courthouse",
    },
    Advance {
        name: "Combined Arms",
        prerequisites: &[Tech(51), Tech(0)],
        requirement_count: 60,
        first_discovery_bonus: false,
        reference_unlocks: "Paratroopers, Helicopter",
    },
    Advance {
        name: "Combustion",
        prerequisites: &[Tech(66), Tech(75)],
        requirement_count: 44,
        first_discovery_bonus: false,
        reference_unlocks: "Submarine",
    },
    Advance {
        name: "Communism",
        prerequisites: &[Tech(17)],
        requirement_count: 42,
        first_discovery_bonus: false,
        reference_unlocks: "Police Station, United Nations",
    },
    Advance {
        name: "Computers",
        prerequisites: &[],
        requirement_count: 56,
        first_discovery_bonus: false,
        reference_unlocks: "Research Lab, Internet",
    },
    Advance {
        name: "Nationalism",
        prerequisites: &[],
        requirement_count: 26,
        first_discovery_bonus: false,
        reference_unlocks: "Riflemen, Women's Suffrage",
    },
    Advance {
        name: "Construction",
        prerequisites: &[Tech(37), Tech(47)],
        requirement_count: 5,
        first_discovery_bonus: false,
        reference_unlocks: "Aqueduct, Amphitheater, Great Wall",
    },
    Advance {
        name: "Currency",
        prerequisites: &[Tech(47)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Marketplace",
    },
    Advance {
        name: "Democracy",
        prerequisites: &[Tech(93), Tech(6)],
        requirement_count: 19,
        first_discovery_bonus: false,
        reference_unlocks: "Statue of Liberty",
    },
    Advance {
        name: "Economics",
        prerequisites: &[Tech(6)],
        requirement_count: 17,
        first_discovery_bonus: false,
        reference_unlocks: "Stock Exchange, A.Smith's Trading Co.",
    },
    Advance {
        name: "Electricity",
        prerequisites: &[Tech(97)],
        requirement_count: 30,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Electronics",
        prerequisites: &[Tech(4)],
        requirement_count: 43,
        first_discovery_bonus: false,
        reference_unlocks: "Hydro Plant, Hoover Dam",
    },
    Advance {
        name: "Engineering",
        prerequisites: &[],
        requirement_count: 37,
        first_discovery_bonus: false,
        reference_unlocks: "Destroyer, Transport",
    },
    Advance {
        name: "Ecology",
        prerequisites: &[],
        requirement_count: 66,
        first_discovery_bonus: false,
        reference_unlocks: "Solar Plant, Space Module",
    },
    Advance {
        name: "Espionage",
        prerequisites: &[Tech(17), Tech(35)],
        requirement_count: 44,
        first_discovery_bonus: false,
        reference_unlocks: "Spy",
    },
    Advance {
        name: "Explosives",
        prerequisites: &[Tech(33), Tech(10)],
        requirement_count: 27,
        first_discovery_bonus: false,
        reference_unlocks: "Engineers",
    },
    Advance {
        name: "Feudalism",
        prerequisites: &[],
        requirement_count: 8,
        first_discovery_bonus: false,
        reference_unlocks: "Pikemen, Sun Tzu's War Academy",
    },
    Advance {
        name: "Flight",
        prerequisites: &[Tech(14)],
        requirement_count: 50,
        first_discovery_bonus: false,
        reference_unlocks: "Fighter",
    },
    Advance {
        name: "Fusion Power",
        prerequisites: &[Tech(76), Tech(38)],
        requirement_count: 77,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Genetic Engineering",
        prerequisites: &[Tech(67), Tech(78)],
        requirement_count: 45,
        first_discovery_bonus: false,
        reference_unlocks: "Cure For Cancer",
    },
    Advance {
        name: "Guerilla Warfare",
        prerequisites: &[Tech(15), Tech(77)],
        requirement_count: 48,
        first_discovery_bonus: false,
        reference_unlocks: "Partisan, Fanatics",
    },
    Advance {
        name: "Gunpowder",
        prerequisites: &[Tech(36)],
        requirement_count: 17,
        first_discovery_bonus: false,
        reference_unlocks: "Musketeers, Barracks II",
    },
    Advance {
        name: "Horseback Riding",
        prerequisites: &[Tech(80), Tech(85)],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Horsemen",
    },
    Advance {
        name: "Industrialization",
        prerequisites: &[Tech(97)],
        requirement_count: 38,
        first_discovery_bonus: false,
        reference_unlocks: "Factory",
    },
    Advance {
        name: "Invention",
        prerequisites: &[Tech(28), Tech(24)],
        requirement_count: 13,
        first_discovery_bonus: false,
        reference_unlocks: "Leonardo's Workshop",
    },
    Advance {
        name: "Iron Working",
        prerequisites: &[Tech(8)],
        requirement_count: 3,
        first_discovery_bonus: false,
        reference_unlocks: "Legion",
    },
    Advance {
        name: "Labor Union",
        prerequisites: &[Tech(15), Tech(51)],
        requirement_count: 58,
        first_discovery_bonus: false,
        reference_unlocks: "Mech. Inf.",
    },
    Advance {
        name: "The Laser",
        prerequisites: &[Tech(57), Tech(16)],
        requirement_count: 65,
        first_discovery_bonus: false,
        reference_unlocks: "SDI Defense",
    },
    Advance {
        name: "Leadership",
        prerequisites: &[Tech(11), Tech(33)],
        requirement_count: 19,
        first_discovery_bonus: false,
        reference_unlocks: "Dragoons",
    },
    Advance {
        name: "Literacy",
        prerequisites: &[Tech(86), Tech(12)],
        requirement_count: 4,
        first_discovery_bonus: false,
        reference_unlocks: "Great Library",
    },
    Advance {
        name: "Machine Tools",
        prerequisites: &[Tech(75), Tech(77)],
        requirement_count: 48,
        first_discovery_bonus: false,
        reference_unlocks: "Artillery",
    },
    Advance {
        name: "Magnetism",
        prerequisites: &[Tech(59)],
        requirement_count: 10,
        first_discovery_bonus: false,
        reference_unlocks: "Caravel, Magellan's Expedition",
    },
    Advance {
        name: "Map Making",
        prerequisites: &[Tech(86), Tech(62)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Trireme, Lighthouse",
    },
    Advance {
        name: "Masonry",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "City Walls, Pyramids",
    },
    Advance {
        name: "Mass Production",
        prerequisites: &[Tech(14), Tech(94)],
        requirement_count: 49,
        first_discovery_bonus: false,
        reference_unlocks: "Battleship, Mass Transit",
    },
    Advance {
        name: "Mathematics",
        prerequisites: &[Tech(45), Tech(1)],
        requirement_count: 3,
        first_discovery_bonus: false,
        reference_unlocks: "Catapult",
    },
    Advance {
        name: "Medicine",
        prerequisites: &[],
        requirement_count: 12,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Metallurgy",
        prerequisites: &[Tech(10)],
        requirement_count: 19,
        first_discovery_bonus: false,
        reference_unlocks: "Cannon",
    },
    Advance {
        name: "Miniaturization",
        prerequisites: &[Tech(16)],
        requirement_count: 48,
        first_discovery_bonus: false,
        reference_unlocks: "Offshore Platform",
    },
    Advance {
        name: "Motorized Transportation",
        prerequisites: &[Tech(46), Tech(23)],
        requirement_count: 53,
        first_discovery_bonus: false,
        reference_unlocks: "Armor, Barracks III",
    },
    Advance {
        name: "Monarchy",
        prerequisites: &[Tech(85), Tech(61)],
        requirement_count: 4,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Monotheism",
        prerequisites: &[],
        requirement_count: 9,
        first_discovery_bonus: false,
        reference_unlocks: "Crusaders, Cathedral, Michelangelo's Chapel",
    },
    Advance {
        name: "Mysticism",
        prerequisites: &[Tech(9)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Temple of Artemis",
    },
    Advance {
        name: "Navigation",
        prerequisites: &[Tech(3)],
        requirement_count: 23,
        first_discovery_bonus: false,
        reference_unlocks: "Galleon, Frigate, Coastal Defense",
    },
    Advance {
        name: "Fission",
        prerequisites: &[],
        requirement_count: 53,
        first_discovery_bonus: false,
        reference_unlocks: "Nuclear, Manhattan Project",
    },
    Advance {
        name: "Nuclear Power",
        prerequisites: &[Tech(56)],
        requirement_count: 56,
        first_discovery_bonus: false,
        reference_unlocks: "Nuclear Plant",
    },
    Advance {
        name: "Philosophy",
        prerequisites: &[Tech(86)],
        requirement_count: 7,
        first_discovery_bonus: true,
        reference_unlocks: "",
    },
    Advance {
        name: "Physics",
        prerequisites: &[Tech(3), Tech(10)],
        requirement_count: 13,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Synthetic Fibers",
        prerequisites: &[Tech(65)],
        requirement_count: 63,
        first_discovery_bonus: false,
        reference_unlocks: "Mfg. Plant",
    },
    Advance {
        name: "Polytheism",
        prerequisites: &[Tech(54)],
        requirement_count: 3,
        first_discovery_bonus: false,
        reference_unlocks: "Elephants, Statue of Zeus",
    },
    Advance {
        name: "Pottery",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Migrants, Granary, Hanging Gardens",
    },
    Advance {
        name: "Radio",
        prerequisites: &[Tech(23)],
        requirement_count: 54,
        first_discovery_bonus: false,
        reference_unlocks: "Airport",
    },
    Advance {
        name: "Railroad",
        prerequisites: &[Tech(74), Tech(49)],
        requirement_count: 35,
        first_discovery_bonus: false,
        reference_unlocks: "Eiffel Tower",
    },
    Advance {
        name: "Recycling",
        prerequisites: &[Tech(25)],
        requirement_count: 51,
        first_discovery_bonus: false,
        reference_unlocks: "Recycling Center",
    },
    Advance {
        name: "Refining",
        prerequisites: &[Tech(78)],
        requirement_count: 40,
        first_discovery_bonus: false,
        reference_unlocks: "Power Plant",
    },
    Advance {
        name: "Refrigeration",
        prerequisites: &[Tech(70), Tech(22)],
        requirement_count: 35,
        first_discovery_bonus: false,
        reference_unlocks: "Supermarket",
    },
    Advance {
        name: "Robotics",
        prerequisites: &[Tech(39), Tech(50)],
        requirement_count: 61,
        first_discovery_bonus: false,
        reference_unlocks: "Howitzer",
    },
    Advance {
        name: "Rocketry",
        prerequisites: &[],
        requirement_count: 59,
        first_discovery_bonus: false,
        reference_unlocks: "AEGIS Cruiser, Cruise Missile, SAM Battery",
    },
    Advance {
        name: "Sanitation",
        prerequisites: &[Tech(48)],
        requirement_count: 20,
        first_discovery_bonus: false,
        reference_unlocks: "Sewer System, Shakespeare's Theater",
    },
    Advance {
        name: "Seafaring",
        prerequisites: &[Tech(62), Tech(44)],
        requirement_count: 4,
        first_discovery_bonus: false,
        reference_unlocks: "Explorer, Harbor",
    },
    Advance {
        name: "Space Flight",
        prerequisites: &[Tech(69)],
        requirement_count: 61,
        first_discovery_bonus: false,
        reference_unlocks: "Space Structural, Apollo Program",
    },
    Advance {
        name: "Stealth",
        prerequisites: &[Tech(60)],
        requirement_count: 67,
        first_discovery_bonus: false,
        reference_unlocks: "Stealth Fighter, Stealth Bomber",
    },
    Advance {
        name: "Steam Engine",
        prerequisites: &[Tech(10), Tech(55)],
        requirement_count: 30,
        first_discovery_bonus: false,
        reference_unlocks: "Ironclad, Darwin's Voyage",
    },
    Advance {
        name: "Steel",
        prerequisites: &[Tech(78)],
        requirement_count: 42,
        first_discovery_bonus: false,
        reference_unlocks: "Cruiser",
    },
    Advance {
        name: "Superconductor",
        prerequisites: &[Tech(56), Tech(72)],
        requirement_count: 70,
        first_discovery_bonus: false,
        reference_unlocks: "Space Component",
    },
    Advance {
        name: "Tactics",
        prerequisites: &[Tech(17), Tech(40)],
        requirement_count: 29,
        first_discovery_bonus: false,
        reference_unlocks: "Alpine Troops, Cavalry",
    },
    Advance {
        name: "The Corporation",
        prerequisites: &[Tech(35)],
        requirement_count: 40,
        first_discovery_bonus: false,
        reference_unlocks: "Freight",
    },
    Advance {
        name: "The Republic",
        prerequisites: &[Tech(58), Tech(12)],
        requirement_count: 5,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "The Wheel",
        prerequisites: &[],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Chariot",
    },
    Advance {
        name: "Theology",
        prerequisites: &[Tech(53)],
        requirement_count: 14,
        first_discovery_bonus: false,
        reference_unlocks: "Ecclesiastical Palace, J.S. Bach's Cathedral",
    },
    Advance {
        name: "Theory of Gravity",
        prerequisites: &[Tech(59)],
        requirement_count: 19,
        first_discovery_bonus: false,
        reference_unlocks: "Isaac Newton's College",
    },
    Advance {
        name: "Trade",
        prerequisites: &[Tech(19), Tech(62)],
        requirement_count: 4,
        first_discovery_bonus: false,
        reference_unlocks: "Caravan, Marco Polo's Embassy",
    },
    Advance {
        name: "Education",
        prerequisites: &[Tech(81)],
        requirement_count: 10,
        first_discovery_bonus: false,
        reference_unlocks: "University",
    },
    Advance {
        name: "Warrior Code",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Archers",
    },
    Advance {
        name: "Writing",
        prerequisites: &[Tech(1)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Library",
    },
    // Civ III advances absent from the original Freeciv Civ2Civ3 catalog.
    // IDs are appended so existing serialized technology IDs remain stable.
    Advance {
        name: "Free Artistry",
        prerequisites: &[Tech(20)],
        requirement_count: 30,
        first_discovery_bonus: false,
        reference_unlocks: "Shakespeare's Theater",
    },
    Advance {
        name: "Genetics",
        prerequisites: &[Tech(50)],
        requirement_count: 60,
        first_discovery_bonus: false,
        reference_unlocks: "Cure For Cancer, Longevity",
    },
    Advance {
        name: "Integrated Defense",
        prerequisites: &[Tech(76), Tech(95), Tech(98)],
        requirement_count: 70,
        first_discovery_bonus: false,
        reference_unlocks: "Strategic Missile Defense",
    },
    Advance {
        name: "Literature",
        prerequisites: &[Tech(86)],
        requirement_count: 4,
        first_discovery_bonus: false,
        reference_unlocks: "Library, Great Library",
    },
    Advance {
        name: "Military Tradition",
        prerequisites: &[Tech(49)],
        requirement_count: 38,
        first_discovery_bonus: false,
        reference_unlocks: "Cavalry, Military Academy",
    },
    Advance {
        name: "Music Theory",
        prerequisites: &[Tech(84)],
        requirement_count: 30,
        first_discovery_bonus: false,
        reference_unlocks: "J.S. Bach's Cathedral",
    },
    Advance {
        name: "Printing Press",
        prerequisites: &[Tech(81)],
        requirement_count: 30,
        first_discovery_bonus: false,
        reference_unlocks: "Communications trading",
    },
    Advance {
        name: "Replaceable Parts",
        prerequisites: &[Tech(22)],
        requirement_count: 50,
        first_discovery_bonus: false,
        reference_unlocks: "Artillery, Infantry, Rubber, faster workers",
    },
    Advance {
        name: "Satellites",
        prerequisites: &[Tech(72)],
        requirement_count: 70,
        first_discovery_bonus: false,
        reference_unlocks: "ICBM, spaceship thrusters, map reveal",
    },
    Advance {
        name: "Scientific Method",
        prerequisites: &[Tech(48), Tech(22)],
        requirement_count: 50,
        first_discovery_bonus: false,
        reference_unlocks: "Theory of Evolution",
    },
    Advance {
        name: "Steam Power",
        prerequisites: &[],
        requirement_count: 40,
        first_discovery_bonus: false,
        reference_unlocks: "Ironclad, railroads, Coal",
    },
    Advance {
        name: "Smart Weapons",
        prerequisites: &[Tech(95), Tech(39)],
        requirement_count: 65,
        first_discovery_bonus: false,
        reference_unlocks: "Integrated Defense",
    },
];

#[cfg(test)]
mod sentinel_tests {
    use super::*;
    #[test]
    fn adding_unset_preserves_every_existing_persisted_advance_index() {
        assert_eq!(Tech::all().count(), 99);
        assert_eq!(Tech::FUTURE.0, 99);
        assert_eq!(Tech::UNSET.0, 100);
        assert_eq!(Tech::POTTERY.0, 62);
        assert_eq!(
            bincode::serialize(&Tech::FUTURE).unwrap(),
            99_u64.to_le_bytes()
        );
        assert_eq!(
            bincode::deserialize::<Tech>(&100_u64.to_le_bytes()).unwrap(),
            Tech::UNSET
        );
        assert_eq!(Tech::UNSET.name(), "No goal");
        assert!(Tech::UNSET.data().is_none());
        assert!(Tech::UNSET.prerequisites().is_empty());
    }
}
