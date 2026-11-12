//! Sight-only binding from stable Civ III Conquests unit IDs to Freeciv C2C3 unit records.
//!
//! These rows choose vision-radius data only. They do not bind full unit content or
//! establish interchangeable unit identities. Exact rows preserve direct identity
//! matches; `SightAlias` rows are explicit sight-only crosswalks. `Unresolved`
//! rows retain the C2C3 baseline unit radius of 2 and do not infer a counterpart
//! from similar names or roles.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SightBinding {
    Exact(&'static str),
    SightAlias(&'static str),
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Civ3UnitSightBinding {
    conquests_id: &'static str,
    binding: SightBinding,
}

// Complete Conquests runtime roster: 33 exact, 27 sight-only aliases (18 source-linked), 32 unresolved.
const CIV3_UNIT_SIGHT_BINDINGS: &[Civ3UnitSightBinding] = &[
    Civ3UnitSightBinding {
        conquests_id: "settler",
        binding: SightBinding::Exact("unit_settlers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "worker",
        binding: SightBinding::Exact("unit_worker"),
    },
    Civ3UnitSightBinding {
        conquests_id: "explorer",
        binding: SightBinding::Exact("unit_explorer"),
    },
    Civ3UnitSightBinding {
        conquests_id: "warrior",
        binding: SightBinding::Exact("unit_warriors"),
    },
    Civ3UnitSightBinding {
        conquests_id: "archer",
        binding: SightBinding::Exact("unit_archers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "horseman",
        binding: SightBinding::SightAlias("unit_horsemen"),
    },
    Civ3UnitSightBinding {
        conquests_id: "chariot",
        binding: SightBinding::Exact("unit_chariot"),
    },
    Civ3UnitSightBinding {
        conquests_id: "catapult",
        binding: SightBinding::Exact("unit_catapult"),
    },
    Civ3UnitSightBinding {
        conquests_id: "musketman",
        binding: SightBinding::SightAlias("unit_musketeers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "marine",
        binding: SightBinding::Exact("unit_marines"),
    },
    Civ3UnitSightBinding {
        conquests_id: "knight",
        binding: SightBinding::Exact("unit_knights"),
    },
    Civ3UnitSightBinding {
        conquests_id: "cannon",
        binding: SightBinding::Exact("unit_cannon"),
    },
    Civ3UnitSightBinding {
        conquests_id: "bomber",
        binding: SightBinding::Exact("unit_bomber"),
    },
    Civ3UnitSightBinding {
        conquests_id: "fighter",
        binding: SightBinding::Exact("unit_fighter"),
    },
    Civ3UnitSightBinding {
        conquests_id: "stealth_fighter",
        binding: SightBinding::Exact("unit_stealth_fighter"),
    },
    Civ3UnitSightBinding {
        conquests_id: "stealth_bomber",
        binding: SightBinding::Exact("unit_stealth_bomber"),
    },
    Civ3UnitSightBinding {
        conquests_id: "helicopter",
        binding: SightBinding::Exact("unit_helicopter"),
    },
    Civ3UnitSightBinding {
        conquests_id: "galley",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "caravel",
        binding: SightBinding::Exact("unit_caravel"),
    },
    Civ3UnitSightBinding {
        conquests_id: "galleon",
        binding: SightBinding::Exact("unit_galleon"),
    },
    Civ3UnitSightBinding {
        conquests_id: "transport",
        binding: SightBinding::Exact("unit_transport"),
    },
    Civ3UnitSightBinding {
        conquests_id: "carrier",
        binding: SightBinding::Exact("unit_carrier"),
    },
    Civ3UnitSightBinding {
        conquests_id: "frigate",
        binding: SightBinding::Exact("unit_frigate"),
    },
    Civ3UnitSightBinding {
        conquests_id: "ironclad",
        binding: SightBinding::Exact("unit_ironclad"),
    },
    Civ3UnitSightBinding {
        conquests_id: "destroyer",
        binding: SightBinding::Exact("unit_destroyer"),
    },
    Civ3UnitSightBinding {
        conquests_id: "cruiser",
        binding: SightBinding::Exact("unit_cruiser"),
    },
    Civ3UnitSightBinding {
        conquests_id: "battleship",
        binding: SightBinding::Exact("unit_battleship"),
    },
    Civ3UnitSightBinding {
        conquests_id: "submarine",
        binding: SightBinding::Exact("unit_submarine"),
    },
    Civ3UnitSightBinding {
        conquests_id: "aegis_cruiser",
        binding: SightBinding::Exact("unit_aegis_cruiser"),
    },
    Civ3UnitSightBinding {
        conquests_id: "cruise_missile",
        binding: SightBinding::Exact("unit_cruise_missile"),
    },
    Civ3UnitSightBinding {
        conquests_id: "paratrooper",
        binding: SightBinding::Exact("unit_paratroopers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "pikeman",
        binding: SightBinding::SightAlias("unit_pikemen"),
    },
    Civ3UnitSightBinding {
        conquests_id: "rifleman",
        binding: SightBinding::SightAlias("unit_riflemen"),
    },
    Civ3UnitSightBinding {
        conquests_id: "cavalry",
        binding: SightBinding::Exact("unit_cavalry"),
    },
    Civ3UnitSightBinding {
        conquests_id: "tank",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "artillery",
        binding: SightBinding::Exact("unit_artillery"),
    },
    Civ3UnitSightBinding {
        conquests_id: "mech_infantry",
        binding: SightBinding::SightAlias("unit_mech_inf"),
    },
    Civ3UnitSightBinding {
        conquests_id: "longbowman",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "infantry",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "modern_armor",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "privateer",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "jet_fighter",
        binding: SightBinding::SightAlias("unit_fighter"),
    },
    Civ3UnitSightBinding {
        conquests_id: "nuclear_submarine",
        binding: SightBinding::SightAlias("unit_submarine"),
    },
    Civ3UnitSightBinding {
        conquests_id: "icbm",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "tactical_nuke",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "crusader",
        binding: SightBinding::Exact("unit_crusaders"),
    },
    Civ3UnitSightBinding {
        conquests_id: "leader",
        binding: SightBinding::Exact("unit_leader"),
    },
    Civ3UnitSightBinding {
        conquests_id: "scout",
        binding: SightBinding::SightAlias("unit_explorer"),
    },
    Civ3UnitSightBinding {
        conquests_id: "guerilla",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "war_elephant",
        binding: SightBinding::SightAlias("unit_knights"),
    },
    Civ3UnitSightBinding {
        conquests_id: "modern_paratrooper",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "spearman",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "swordsman",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "radar_artillery",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "army",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "jaguar_warrior",
        binding: SightBinding::SightAlias("unit_warriors"),
    },
    Civ3UnitSightBinding {
        conquests_id: "bowman",
        binding: SightBinding::SightAlias("unit_archers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "hoplite",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "impi",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "legionary",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "immortals",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "war_chariot",
        binding: SightBinding::SightAlias("unit_chariot"),
    },
    Civ3UnitSightBinding {
        conquests_id: "rider",
        binding: SightBinding::SightAlias("unit_knights"),
    },
    Civ3UnitSightBinding {
        conquests_id: "mounted_warrior",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "musketeer",
        binding: SightBinding::Exact("unit_musketeers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "samurai",
        binding: SightBinding::SightAlias("unit_knights"),
    },
    Civ3UnitSightBinding {
        conquests_id: "cossack",
        binding: SightBinding::SightAlias("unit_cavalry"),
    },
    Civ3UnitSightBinding {
        conquests_id: "panzer",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "man_o_war",
        binding: SightBinding::SightAlias("unit_frigate"),
    },
    Civ3UnitSightBinding {
        conquests_id: "f_15",
        binding: SightBinding::SightAlias("unit_fighter"),
    },
    Civ3UnitSightBinding {
        conquests_id: "keshik",
        binding: SightBinding::SightAlias("unit_knights"),
    },
    Civ3UnitSightBinding {
        conquests_id: "conquistador",
        binding: SightBinding::SightAlias("unit_explorer"),
    },
    Civ3UnitSightBinding {
        conquests_id: "berserk",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "sipahi",
        binding: SightBinding::SightAlias("unit_cavalry"),
    },
    Civ3UnitSightBinding {
        conquests_id: "gallic_swordsman",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "ansar_warrior",
        binding: SightBinding::SightAlias("unit_knights"),
    },
    Civ3UnitSightBinding {
        conquests_id: "numidian_mercenary",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "hwach_a",
        binding: SightBinding::SightAlias("unit_cannon"),
    },
    Civ3UnitSightBinding {
        conquests_id: "medieval_infantry",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "enkidu_warrior",
        binding: SightBinding::SightAlias("unit_warriors"),
    },
    Civ3UnitSightBinding {
        conquests_id: "three_man_chariot",
        binding: SightBinding::SightAlias("unit_chariot"),
    },
    Civ3UnitSightBinding {
        conquests_id: "carrack",
        binding: SightBinding::SightAlias("unit_caravel"),
    },
    Civ3UnitSightBinding {
        conquests_id: "swiss_mercenary",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "trebuchet",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "chasqui_scout",
        binding: SightBinding::SightAlias("unit_explorer"),
    },
    Civ3UnitSightBinding {
        conquests_id: "javelin_thrower",
        binding: SightBinding::SightAlias("unit_archers"),
    },
    Civ3UnitSightBinding {
        conquests_id: "dromon",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "ancient_cavalry",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "curragh",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "tow_infantry",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "flak",
        binding: SightBinding::Unresolved,
    },
    Civ3UnitSightBinding {
        conquests_id: "mobile_sam",
        binding: SightBinding::Unresolved,
    },
];

const _: [(); crate::sim::UnitKind::ALL_NATIVE.len()] = [(); CIV3_UNIT_SIGHT_BINDINGS.len()];

pub(super) fn binding_for(conquests_id: &str) -> Option<SightBinding> {
    CIV3_UNIT_SIGHT_BINDINGS
        .iter()
        .find(|entry| entry.conquests_id == conquests_id)
        .map(|entry| entry.binding)
}
