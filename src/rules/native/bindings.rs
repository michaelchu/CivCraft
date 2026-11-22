//! Engine-owned bindings that content authors should not edit.
//!
//! These values tie catalog records to Civ III artwork, to saved-game
//! identities and to presentation behavior. They are not balance knobs, so
//! they live in Rust rather than in `data/rulesets/civcraft/content/`. A record
//! without a binding here is rejected when its catalog loads.

/// Art folder and installation label for a unit. Folders default to the unit
/// name; a `|`-separated list gives one variant per era (Leader, Army).
pub(super) fn unit_art(id: &str, name: &str) -> (String, String) {
    let folder = match id {
        "modern_paratrooper" => "Paratrooper",
        "aegis_cruiser" => "Aegis Cruiser",
        "leader" => {
            "Leader Ancient Times|Leader Middle Ages|Leader Industrial Ages|Leader Modern Times"
        }
        "army" => "Army Ancient Times|Army Middle Ages|Army Industrial Ages|Army Modern Times",
        "numidian_mercenary" => "Libyan Mercenary",
        "hwach_a" => "Hwacha",
        "three_man_chariot" => "Three Man Chariot",
        "chasqui_scout" => "Chasquis Scout",
        _ => name,
    };
    let root = match id {
        "leader" | "army" => "base|base|base|base",
        "keshik" | "conquistador" | "berserk" | "sipahi" | "gallic_swordsman" | "ansar_warrior"
        | "numidian_mercenary" | "hwach_a" | "medieval_infantry" | "guerilla" => "ptw",
        "enkidu_warrior" | "three_man_chariot" | "carrack" | "swiss_mercenary" | "trebuchet"
        | "chasqui_scout" | "javelin_thrower" | "dromon" | "cruiser" | "crusader"
        | "ancient_cavalry" | "curragh" | "paratrooper" | "tow_infantry" | "flak"
        | "mobile_sam" => "conquests",
        _ => "base",
    };
    (folder.to_owned(), root.to_owned())
}

/// Units whose ranged attacks play bombardment impact effects. Presentation only.
pub(super) fn has_bombard_effects(id: &str) -> bool {
    matches!(
        id,
        "catapult"
            | "cannon"
            | "artillery"
            | "radar_artillery"
            | "cruise_missile"
            | "frigate"
            | "ironclad"
            | "destroyer"
            | "battleship"
            | "aegis_cruiser"
            | "man_o_war"
            | "hwach_a"
            | "trebuchet"
            | "dromon"
            | "cruiser"
    )
}

/// Persistent `Tech` index for each technology ID. Saved campaigns store these
/// numbers, so an entry must never change or be reused; new technologies append.
pub(super) fn technology_save_index(id: &str) -> Option<usize> {
    Some(match id {
        "TECH_Bronze_Working" => 8,
        "TECH_Masonry" => 45,
        "TECH_Alphabet" => 1,
        "TECH_Pottery" => 62,
        "TECH_The_Wheel" => 80,
        "TECH_Warrior_Code" => 85,
        "TECH_Ceremonial_Burial" => 9,
        "TECH_Iron_Working" => 37,
        "TECH_Writing" => 86,
        "TECH_Mysticism" => 54,
        "TECH_Mathematics" => 47,
        "TECH_Philosophy" => 58,
        "TECH_Code_of_Laws" => 12,
        "TECH_Literature" => 90,
        "TECH_Map_Making" => 44,
        "TECH_Horseback_Riding" => 34,
        "TECH_Polytheism" => 61,
        "TECH_Currency" => 19,
        "TECH_The_Republic" => 79,
        "TECH_Monarchy" => 52,
        "TECH_Construction" => 18,
        "TECH_Monotheism" => 53,
        "TECH_Feudalism" => 28,
        "TECH_Engineering" => 24,
        "TECH_Theology" => 81,
        "TECH_Chivalry" => 11,
        "TECH_Invention" => 36,
        "TECH_Printing_Press" => 93,
        "TECH_Music_Theory" => 92,
        "TECH_Education" => 84,
        "TECH_Gunpowder" => 33,
        "TECH_Banking" => 6,
        "TECH_Astronomy" => 3,
        "TECH_Chemistry" => 10,
        "TECH_Democracy" => 20,
        "TECH_Economics" => 21,
        "TECH_Navigation" => 55,
        "TECH_Physics" => 59,
        "TECH_Metallurgy" => 49,
        "TECH_Free_Artistry" => 87,
        "TECH_Theory_of_Gravity" => 82,
        "TECH_Magnetism" => 43,
        "TECH_Military_Tradition" => 91,
        "TECH_Nationalism" => 17,
        "TECH_Steam_Power" => 97,
        "TECH_Medicine" => 48,
        "TECH_Communism" => 15,
        "TECH_Industrialization" => 35,
        "TECH_Electricity" => 22,
        "TECH_Scientific_Method" => 96,
        "TECH_Sanitation" => 70,
        "TECH_Espionage" => 26,
        "TECH_The_Corporation" => 78,
        "TECH_Refining" => 66,
        "TECH_Steel" => 75,
        "TECH_Atomic_Theory" => 4,
        "TECH_Combustion" => 14,
        "TECH_Replaceable_Parts" => 94,
        "TECH_Flight" => 29,
        "TECH_Amphibious_Warfare" => 2,
        "TECH_Mass_Production" => 46,
        "TECH_Electronics" => 23,
        "TECH_Motorized_Transportation" => 51,
        "TECH_Advanced_Flight" => 0,
        "TECH_Rocketry" => 69,
        "TECH_Fission" => 56,
        "TECH_Computers" => 16,
        "TECH_Recycling" => 65,
        "TECH_Space_Flight" => 72,
        "TECH_Nuclear_Power" => 57,
        "TECH_Superconductor" => 76,
        "TECH_Miniaturization" => 50,
        "TECH_Ecology" => 25,
        "TECH_Synthetic_Fibers" => 60,
        "TECH_Satellites" => 95,
        "TECH_The_Laser" => 39,
        "TECH_Genetics" => 88,
        "TECH_Stealth" => 73,
        "TECH_Smart_Weapons" => 98,
        "TECH_Robotics" => 68,
        "TECH_Integrated_Defense" => 89,
        "TECH_Ironclads" => 100,
        "TECH_Fascism" => 99,
        "TECH_Meditation" => 103,
        "TECH_Divine_Right" => 104,
        _ => return None,
    })
}

/// Palette, city-art and culture-group bindings for a civilization.
#[derive(Clone, Copy)]
pub(super) struct CivilizationArt {
    pub(super) primary_color_index: u8,
    pub(super) secondary_color_index: u8,
    pub(super) city_style: u8,
    /// Raw Conquests RACE CultureGroup index; separate from culture points.
    pub(super) culture_group_index: i32,
}

pub(super) fn civilization_art(id: &str) -> Option<CivilizationArt> {
    let (primary_color_index, secondary_color_index, city_style, culture_group_index) = match id {
        "rome" => (1, 1, 4, 2),
        "egypt" => (3, 3, 3, 2),
        "greece" => (10, 10, 2, 2),
        "babylon" => (1, 13, 3, 3),
        "germany" => (6, 6, 2, 1),
        "russia" => (9, 9, 2, 1),
        "china" => (5, 15, 0, 4),
        "america" => (5, 5, 1, 0),
        "japan" => (4, 11, 0, 4),
        "france" => (7, 7, 2, 1),
        "india" => (8, 12, 0, 4),
        "persia" => (10, 14, 3, 3),
        "aztecs" => (4, 4, 1, 0),
        "zululand" => (3, 16, 3, 3),
        "iroquois" => (8, 8, 1, 0),
        "england" => (2, 2, 2, 1),
        "mongols" => (3, 26, 0, 4),
        "spain" => (5, 20, 2, 1),
        "scandinavia" => (8, 31, 2, 1),
        "ottomans" => (2, 18, 3, 3),
        "celts" => (4, 25, 2, 1),
        "arabia" => (7, 30, 3, 3),
        "carthage" => (9, 11, 3, 2),
        "korea" => (6, 19, 0, 4),
        "sumeria" => (5, 20, 3, 3),
        "hittites" => (14, 27, 3, 3),
        "netherlands" => (2, 18, 2, 1),
        "portugal" => (8, 17, 2, 1),
        "byzantines" => (11, 1, 2, 2),
        "inca" => (7, 8, 1, 0),
        "maya" => (6, 5, 1, 0),
        _ => return None,
    };
    Some(CivilizationArt {
        primary_color_index,
        secondary_color_index,
        city_style,
        culture_group_index,
    })
}
