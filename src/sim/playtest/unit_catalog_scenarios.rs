use super::super::unit_catalog::UnitFlag;
use super::*;

impl Simulation {
    pub(crate) fn native_unit_catalog_playtest(&self) -> (bool, bool, bool) {
        let armor = UnitKind::Armor.stats();
        let awacs = UnitKind::Awacs.stats();
        let stats = (
            armor.cost,
            armor.attack,
            armor.defense,
            armor.hp,
            armor.moves,
        ) == (90, 10, 5, 30, 3)
            && (
                awacs.cost,
                awacs.attack,
                awacs.defense,
                awacs.hp,
                awacs.moves,
                awacs.vision_sq,
            ) == (140, 0, 1, 20, 16, 26)
            && UnitKind::Awacs.fuel_capacity() == 3
            && UnitKind::Fanatics.stats().pop_cost == 1
            && UnitKind::Fanatics.base_upkeep() == (1, 0, 0)
            && UnitKind::Pikemen
                .required_tech()
                .unwrap()
                .data()
                .unwrap()
                .name
                == "Feudalism"
            && UnitKind::MechInf
                .required_tech()
                .unwrap()
                .data()
                .unwrap()
                .name
                == "Labor Union";
        let flags = UnitKind::Riflemen.has_marine_attack()
            && UnitKind::AlpineTroops.can_helicopter_field_load()
            && !UnitKind::AlpineTroops.has_flag(UnitFlag::IgZOC)
            && !UnitKind::Riflemen.can_helicopter_field_load()
            && UnitKind::Fanatics.required_government() == Some(Government::Fundamentalism)
            && UnitKind::Artillery.is_city_buster()
            && UnitKind::Howitzer.is_city_buster()
            && UnitKind::Leader.has_no_build()
            && UnitKind::Leader.is_unbribable()
            && UnitKind::Storm.has_flag(UnitFlag::RandomMovement)
            && UnitKind::Storm.has_flag(UnitFlag::Flagless)
            && !UnitKind::Awacs.is_civilian()
            && !UnitKind::Storm.is_civilian()
            && UnitKind::MechInf.is_big_land()
            && !UnitKind::Armor.has_terrain_defense();
        let position = |kind| {
            UnitKind::ALL_NATIVE
                .iter()
                .position(|&item| item == kind)
                .unwrap()
        };
        let order = UnitKind::ALL_NATIVE.len() == 57
            && UnitKind::SIMPLE_MILITARY.len() == 32
            && UnitKind::Warrior as u32 == 7
            && position(UnitKind::Warrior) == 4
            && UnitKind::Paratroopers as u32 == 41
            && UnitKind::Pikemen as u32 == 42
            && UnitKind::Storm as u32 == 56
            && position(UnitKind::Pikemen) < position(UnitKind::Musketeers)
            && position(UnitKind::MechInf) < position(UnitKind::Horseman)
            && position(UnitKind::AegisCruiser) < position(UnitKind::Battleship);
        (stats, flags, order)
    }
}
