// Catalog transcribed from Freeciv data/civ2civ3/techs.ruleset.
// Reference unlock labels come from the unit and building rulesets.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tech(pub(crate) usize);

pub(crate) struct Advance {
    pub(crate) name: &'static str,
    pub(crate) prerequisites: &'static [Tech],
    pub(crate) requirement_count: i32,
    pub(crate) first_discovery_bonus: bool,
    pub(crate) reference_unlocks: &'static str,
}

impl Tech {
    pub(crate) const ALPHABET: Self = Self(1);
    pub(crate) const BRIDGE_BUILDING: Self = Self(7);
    pub(crate) const BRONZE_WORKING: Self = Self(8);
    pub(crate) const WARRIOR_CODE: Self = Self(85);
    pub(crate) const HORSEBACK_RIDING: Self = Self(34);
    pub(crate) const POTTERY: Self = Self(62);
    pub(crate) const IRON_WORKING: Self = Self(37);
    pub(crate) const THE_WHEEL: Self = Self(80);
    pub(crate) const MATHEMATICS: Self = Self(47);
    pub(crate) const UNIVERSITY: Self = Self(84);
    pub(crate) const RAILROAD: Self = Self(64);
    pub(crate) const AUTOMOBILE: Self = Self(5);
    pub(crate) const ROCKETRY: Self = Self(69);
    pub(crate) const SUPERCONDUCTORS: Self = Self(76);
    pub(crate) const CHIVALRY: Self = Self(11);
    pub(crate) const GUNPOWDER: Self = Self(33);
    pub(crate) const GUERILLA_WARFARE: Self = Self(32);
    pub(crate) const METALLURGY: Self = Self(49);
    pub(crate) const FUTURE: Self = Self(ADVANCES.len());

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..ADVANCES.len()).map(Self)
    }

    pub(crate) fn data(self) -> Option<&'static Advance> {
        ADVANCES.get(self.0)
    }

    pub(crate) fn name(self) -> &'static str {
        self.data().map_or("Future Tech", |advance| advance.name)
    }

    pub(crate) fn prerequisites(self) -> &'static [Self] {
        self.data().map_or(&[], |advance| advance.prerequisites)
    }
}

const ADVANCES: &[Advance] = &[
    Advance {
        name: "Advanced Flight",
        prerequisites: &[Tech(63), Tech(42)],
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
        prerequisites: &[Tech(24), Tech(77)],
        requirement_count: 45,
        first_discovery_bonus: false,
        reference_unlocks: "Marines, Port Facility",
    },
    Advance {
        name: "Astronomy",
        prerequisites: &[Tech(54), Tech(47)],
        requirement_count: 6,
        first_discovery_bonus: false,
        reference_unlocks: "Copernicus' Observatory",
    },
    Advance {
        name: "Atomic Theory",
        prerequisites: &[Tech(10), Tech(67)],
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
        prerequisites: &[Tech(83), Tech(79)],
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
        prerequisites: &[Tech(84), Tech(48)],
        requirement_count: 16,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Chivalry",
        prerequisites: &[Tech(28), Tech(34)],
        requirement_count: 10,
        first_discovery_bonus: false,
        reference_unlocks: "Knights, King Richard's Crusade",
    },
    Advance {
        name: "Code of Laws",
        prerequisites: &[Tech(1)],
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
        prerequisites: &[Tech(66), Tech(24)],
        requirement_count: 44,
        first_discovery_bonus: false,
        reference_unlocks: "Submarine",
    },
    Advance {
        name: "Communism",
        prerequisites: &[Tech(81), Tech(35)],
        requirement_count: 42,
        first_discovery_bonus: false,
        reference_unlocks: "Police Station, United Nations",
    },
    Advance {
        name: "Computers",
        prerequisites: &[Tech(50), Tech(63)],
        requirement_count: 56,
        first_discovery_bonus: false,
        reference_unlocks: "Research Lab, Internet",
    },
    Advance {
        name: "Conscription",
        prerequisites: &[Tech(20), Tech(49)],
        requirement_count: 26,
        first_discovery_bonus: false,
        reference_unlocks: "Riflemen, Women's Suffrage",
    },
    Advance {
        name: "Construction",
        prerequisites: &[Tech(45), Tech(37)],
        requirement_count: 5,
        first_discovery_bonus: false,
        reference_unlocks: "Aqueduct, Amphitheater, Great Wall",
    },
    Advance {
        name: "Currency",
        prerequisites: &[Tech(8)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Marketplace",
    },
    Advance {
        name: "Democracy",
        prerequisites: &[Tech(6), Tech(36)],
        requirement_count: 19,
        first_discovery_bonus: false,
        reference_unlocks: "Statue of Liberty",
    },
    Advance {
        name: "Economics",
        prerequisites: &[Tech(6), Tech(84)],
        requirement_count: 17,
        first_discovery_bonus: false,
        reference_unlocks: "Stock Exchange, A.Smith's Trading Co.",
    },
    Advance {
        name: "Electricity",
        prerequisites: &[Tech(49), Tech(82)],
        requirement_count: 30,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Electronics",
        prerequisites: &[Tech(78), Tech(22)],
        requirement_count: 43,
        first_discovery_bonus: false,
        reference_unlocks: "Hydro Plant, Hoover Dam",
    },
    Advance {
        name: "Engineering",
        prerequisites: &[Tech(22), Tech(74)],
        requirement_count: 37,
        first_discovery_bonus: false,
        reference_unlocks: "Destroyer, Transport",
    },
    Advance {
        name: "Environmentalism",
        prerequisites: &[Tech(65), Tech(72)],
        requirement_count: 66,
        first_discovery_bonus: false,
        reference_unlocks: "Solar Plant, Space Module",
    },
    Advance {
        name: "Espionage",
        prerequisites: &[Tech(15), Tech(20)],
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
        prerequisites: &[Tech(37), Tech(52)],
        requirement_count: 8,
        first_discovery_bonus: false,
        reference_unlocks: "Pikemen, Sun Tzu's War Academy",
    },
    Advance {
        name: "Flight",
        prerequisites: &[Tech(14), Tech(77)],
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
        prerequisites: &[Tech(36), Tech(28)],
        requirement_count: 17,
        first_discovery_bonus: false,
        reference_unlocks: "Musketeers, Barracks II",
    },
    Advance {
        name: "Horseback Riding",
        prerequisites: &[],
        requirement_count: 1,
        first_discovery_bonus: false,
        reference_unlocks: "Horsemen",
    },
    Advance {
        name: "Industrialization",
        prerequisites: &[Tech(64), Tech(6)],
        requirement_count: 38,
        first_discovery_bonus: false,
        reference_unlocks: "Factory",
    },
    Advance {
        name: "Invention",
        prerequisites: &[Tech(7), Tech(41)],
        requirement_count: 13,
        first_discovery_bonus: false,
        reference_unlocks: "Leonardo's Workshop",
    },
    Advance {
        name: "Iron Working",
        prerequisites: &[Tech(8), Tech(85)],
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
        name: "Laser",
        prerequisites: &[Tech(16), Tech(57)],
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
        prerequisites: &[Tech(71), Tech(3)],
        requirement_count: 10,
        first_discovery_bonus: false,
        reference_unlocks: "Caravel, Magellan's Expedition",
    },
    Advance {
        name: "Map Making",
        prerequisites: &[Tech(1)],
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
        prerequisites: &[Tech(5), Tech(78)],
        requirement_count: 49,
        first_discovery_bonus: false,
        reference_unlocks: "Battleship, Mass Transit",
    },
    Advance {
        name: "Mathematics",
        prerequisites: &[Tech(1), Tech(45)],
        requirement_count: 3,
        first_discovery_bonus: false,
        reference_unlocks: "Catapult",
    },
    Advance {
        name: "Medicine",
        prerequisites: &[Tech(58), Tech(83)],
        requirement_count: 12,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Metallurgy",
        prerequisites: &[Tech(33), Tech(47)],
        requirement_count: 19,
        first_discovery_bonus: false,
        reference_unlocks: "Cannon",
    },
    Advance {
        name: "Miniaturization",
        prerequisites: &[Tech(14), Tech(23)],
        requirement_count: 48,
        first_discovery_bonus: false,
        reference_unlocks: "Offshore Platform",
    },
    Advance {
        name: "Mobile Warfare",
        prerequisites: &[Tech(5), Tech(42)],
        requirement_count: 53,
        first_discovery_bonus: false,
        reference_unlocks: "Armor, Barracks III",
    },
    Advance {
        name: "Monarchy",
        prerequisites: &[Tech(9), Tech(12)],
        requirement_count: 4,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Monotheism",
        prerequisites: &[Tech(3), Tech(61)],
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
        prerequisites: &[Tech(36), Tech(59)],
        requirement_count: 23,
        first_discovery_bonus: false,
        reference_unlocks: "Galleon, Frigate, Coastal Defense",
    },
    Advance {
        name: "Nuclear Fission",
        prerequisites: &[Tech(46), Tech(4)],
        requirement_count: 53,
        first_discovery_bonus: false,
        reference_unlocks: "Nuclear, Manhattan Project",
    },
    Advance {
        name: "Nuclear Power",
        prerequisites: &[Tech(56), Tech(50)],
        requirement_count: 56,
        first_discovery_bonus: false,
        reference_unlocks: "Nuclear Plant",
    },
    Advance {
        name: "Philosophy",
        prerequisites: &[Tech(54), Tech(41)],
        requirement_count: 7,
        first_discovery_bonus: true,
        reference_unlocks: "",
    },
    Advance {
        name: "Physics",
        prerequisites: &[Tech(80), Tech(43)],
        requirement_count: 13,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "Plastics",
        prerequisites: &[Tech(46), Tech(68)],
        requirement_count: 63,
        first_discovery_bonus: false,
        reference_unlocks: "Mfg. Plant",
    },
    Advance {
        name: "Polytheism",
        prerequisites: &[Tech(34), Tech(9)],
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
        prerequisites: &[Tech(29), Tech(23)],
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
        prerequisites: &[Tech(46), Tech(70)],
        requirement_count: 51,
        first_discovery_bonus: false,
        reference_unlocks: "Recycling Center",
    },
    Advance {
        name: "Refining",
        prerequisites: &[Tech(27), Tech(35)],
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
        prerequisites: &[Tech(51), Tech(16)],
        requirement_count: 61,
        first_discovery_bonus: false,
        reference_unlocks: "Howitzer",
    },
    Advance {
        name: "Rocketry",
        prerequisites: &[Tech(0), Tech(50)],
        requirement_count: 59,
        first_discovery_bonus: false,
        reference_unlocks: "AEGIS Cruiser, Cruise Missile, SAM Battery",
    },
    Advance {
        name: "Sanitation",
        prerequisites: &[Tech(7), Tech(48)],
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
        prerequisites: &[Tech(16), Tech(69)],
        requirement_count: 61,
        first_discovery_bonus: false,
        reference_unlocks: "Space Structural, Apollo Program",
    },
    Advance {
        name: "Stealth",
        prerequisites: &[Tech(60), Tech(72)],
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
        prerequisites: &[Tech(24), Tech(35)],
        requirement_count: 42,
        first_discovery_bonus: false,
        reference_unlocks: "Cruiser",
    },
    Advance {
        name: "Superconductors",
        prerequisites: &[Tech(72), Tech(39)],
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
        prerequisites: &[Tech(21), Tech(35)],
        requirement_count: 40,
        first_discovery_bonus: false,
        reference_unlocks: "Freight",
    },
    Advance {
        name: "The Republic",
        prerequisites: &[Tech(12), Tech(41)],
        requirement_count: 5,
        first_discovery_bonus: false,
        reference_unlocks: "",
    },
    Advance {
        name: "The Wheel",
        prerequisites: &[Tech(34)],
        requirement_count: 2,
        first_discovery_bonus: false,
        reference_unlocks: "Chariot",
    },
    Advance {
        name: "Theology",
        prerequisites: &[Tech(58), Tech(53)],
        requirement_count: 14,
        first_discovery_bonus: false,
        reference_unlocks: "Ecclesiastical Palace, J.S. Bach's Cathedral",
    },
    Advance {
        name: "Theory of Gravity",
        prerequisites: &[Tech(59), Tech(84)],
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
        name: "University",
        prerequisites: &[Tech(47), Tech(58)],
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
];
