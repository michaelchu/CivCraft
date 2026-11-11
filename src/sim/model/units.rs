//! Unit kinds and their ruleset-facing identity.

use super::research_cost;
use crate::rules::Tech;
use crate::sim::unit_catalog;
use crate::sim::{Base, Terrain};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UnitKind {
    // Conquests campaign identities in source roster order.
    Settler,
    Worker,
    Scout,
    Explorer,
    Marine,
    ModernParatrooper,
    Warrior,
    Archer,
    Spearman,
    Swordsman,
    Chariot,
    Horseman,
    Pikeman,
    Longbowman,
    Musketman,
    Knight,
    Rifleman,
    Cavalry,
    Infantry,
    Tank,
    MechInfantry,
    ModernArmor,
    Catapult,
    Cannon,
    Artillery,
    RadarArtillery,
    CruiseMissile,
    TacticalNuke,
    ICBM,
    Galley,
    Caravel,
    Frigate,
    Galleon,
    Ironclad,
    Transport,
    Carrier,
    Submarine,
    Destroyer,
    Battleship,
    AegisCruiser,
    NuclearSubmarine,
    Fighter,
    Bomber,
    Helicopter,
    JetFighter,
    StealthFighter,
    StealthBomber,
    Leader,
    Army,
    JaguarWarrior,
    Bowman,
    Hoplite,
    Impi,
    Legionary,
    Immortals,
    WarChariot,
    Rider,
    MountedWarrior,
    Musketeer,
    Samurai,
    WarElephant,
    Cossack,
    Panzer,
    ManOWar,
    F15,
    Privateer,
    Keshik,
    Conquistador,
    Berserk,
    Sipahi,
    GallicSwordsman,
    AnsarWarrior,
    NumidianMercenary,
    Hwacha,
    MedievalInfantry,
    Guerilla,
    EnkiduWarrior,
    ThreeManChariot,
    Carrack,
    SwissMercenary,
    Trebuchet,
    ChasquiScout,
    JavelinThrower,
    Dromon,
    Cruiser,
    Crusader,
    AncientCavalry,
    Curragh,
    Paratrooper,
    TOWInfantry,
    Flak,
    MobileSAM,

}

impl UnitKind {
    /// Matching source identity for units shared with the Conquests roster.
    /// Legacy-only identities intentionally have no match and are not granted
    /// campaign availability by the source rules table.
    pub(crate) fn conquests_id(self) -> Option<&'static str> {
        Some(match self {
            Self::Settler => "settler",
            Self::Worker => "worker",
            Self::Explorer => "explorer",
            Self::Warrior => "warrior",
            Self::Archer => "archer",
            Self::Horseman => "horseman",
            Self::Chariot => "chariot",
            Self::Catapult => "catapult",
            Self::Musketman => "musketman",
            Self::Marine => "marine",
            Self::Knight => "knight",
            Self::Cannon => "cannon",
            Self::Bomber => "bomber",
            Self::Fighter => "fighter",
            Self::StealthFighter => "stealth_fighter",
            Self::StealthBomber => "stealth_bomber",
            Self::Helicopter => "helicopter",
            Self::Galley => "galley",
            Self::Caravel => "caravel",
            Self::Galleon => "galleon",
            Self::Transport => "transport",
            Self::Carrier => "carrier",
            Self::Frigate => "frigate",
            Self::Ironclad => "ironclad",
            Self::Destroyer => "destroyer",
            Self::Cruiser => "cruiser",
            Self::Battleship => "battleship",
            Self::Submarine => "submarine",
            Self::AegisCruiser => "aegis_cruiser",
            Self::CruiseMissile => "cruise_missile",
            Self::Paratrooper => "paratrooper",
            Self::Pikeman => "pikeman",
            Self::Rifleman => "rifleman",
            Self::Cavalry => "cavalry",
            Self::Tank => "tank",
            Self::Artillery => "artillery",
            Self::MechInfantry => "mech_infantry",
            Self::Longbowman => "longbowman",
            Self::Infantry => "infantry",
            Self::ModernArmor => "modern_armor",
            Self::Privateer => "privateer",
            Self::JetFighter => "jet_fighter",
            Self::NuclearSubmarine => "nuclear_submarine",
            Self::ICBM => "icbm",
            Self::TacticalNuke => "tactical_nuke",
            Self::Crusader => "crusader",
            Self::Leader => "leader",
            Self::Scout => "scout",
            Self::Guerilla => "guerilla",
            Self::WarElephant => "war_elephant",
            Self::ModernParatrooper => "modern_paratrooper",
            Self::Spearman => "spearman",
            Self::Swordsman => "swordsman",
            Self::RadarArtillery => "radar_artillery",
            Self::Army => "army",
            Self::JaguarWarrior => "jaguar_warrior",
            Self::Bowman => "bowman",
            Self::Hoplite => "hoplite",
            Self::Impi => "impi",
            Self::Legionary => "legionary",
            Self::Immortals => "immortals",
            Self::WarChariot => "war_chariot",
            Self::Rider => "rider",
            Self::MountedWarrior => "mounted_warrior",
            Self::Musketeer => "musketeer",
            Self::Samurai => "samurai",
            Self::Cossack => "cossack",
            Self::Panzer => "panzer",
            Self::ManOWar => "man_o_war",
            Self::F15 => "f_15",
            Self::Keshik => "keshik",
            Self::Conquistador => "conquistador",
            Self::Berserk => "berserk",
            Self::Sipahi => "sipahi",
            Self::GallicSwordsman => "gallic_swordsman",
            Self::AnsarWarrior => "ansar_warrior",
            Self::NumidianMercenary => "numidian_mercenary",
            Self::Hwacha => "hwach_a",
            Self::MedievalInfantry => "medieval_infantry",
            Self::EnkiduWarrior => "enkidu_warrior",
            Self::ThreeManChariot => "three_man_chariot",
            Self::Carrack => "carrack",
            Self::SwissMercenary => "swiss_mercenary",
            Self::Trebuchet => "trebuchet",
            Self::ChasquiScout => "chasqui_scout",
            Self::JavelinThrower => "javelin_thrower",
            Self::Dromon => "dromon",
            Self::AncientCavalry => "ancient_cavalry",
            Self::Curragh => "curragh",
            Self::TOWInfantry => "tow_infantry",
            Self::Flak => "flak",
            Self::MobileSAM => "mobile_sam",
        })
    }

    pub(crate) fn from_civ3_name(name: &str) -> Option<Self> {
        Some(match name {
            "Settler" => Self::Settler,
            "Worker" => Self::Worker,
            "Scout" => Self::Scout,
            "Chasqui Scout" => Self::ChasquiScout,
            _ => return None,
        })
    }

    pub(crate) fn from_conquests_id(id: &str) -> Option<Self> {
        Self::ALL_NATIVE
            .into_iter()
            .find(|kind| kind.conquests_id() == Some(id))
    }

    // Conquests campaign iteration follows source roster order.
    pub(crate) const ALL_NATIVE: [Self; 92] = [
        Self::Settler,
        Self::Worker,
        Self::Scout,
        Self::Explorer,
        Self::Marine,
        Self::ModernParatrooper,
        Self::Warrior,
        Self::Archer,
        Self::Spearman,
        Self::Swordsman,
        Self::Chariot,
        Self::Horseman,
        Self::Pikeman,
        Self::Longbowman,
        Self::Musketman,
        Self::Knight,
        Self::Rifleman,
        Self::Cavalry,
        Self::Infantry,
        Self::Tank,
        Self::MechInfantry,
        Self::ModernArmor,
        Self::Catapult,
        Self::Cannon,
        Self::Artillery,
        Self::RadarArtillery,
        Self::CruiseMissile,
        Self::TacticalNuke,
        Self::ICBM,
        Self::Galley,
        Self::Caravel,
        Self::Frigate,
        Self::Galleon,
        Self::Ironclad,
        Self::Transport,
        Self::Carrier,
        Self::Submarine,
        Self::Destroyer,
        Self::Battleship,
        Self::AegisCruiser,
        Self::NuclearSubmarine,
        Self::Fighter,
        Self::Bomber,
        Self::Helicopter,
        Self::JetFighter,
        Self::StealthFighter,
        Self::StealthBomber,
        Self::Leader,
        Self::Army,
        Self::JaguarWarrior,
        Self::Bowman,
        Self::Hoplite,
        Self::Impi,
        Self::Legionary,
        Self::Immortals,
        Self::WarChariot,
        Self::Rider,
        Self::MountedWarrior,
        Self::Musketeer,
        Self::Samurai,
        Self::WarElephant,
        Self::Cossack,
        Self::Panzer,
        Self::ManOWar,
        Self::F15,
        Self::Privateer,
        Self::Keshik,
        Self::Conquistador,
        Self::Berserk,
        Self::Sipahi,
        Self::GallicSwordsman,
        Self::AnsarWarrior,
        Self::NumidianMercenary,
        Self::Hwacha,
        Self::MedievalInfantry,
        Self::Guerilla,
        Self::EnkiduWarrior,
        Self::ThreeManChariot,
        Self::Carrack,
        Self::SwissMercenary,
        Self::Trebuchet,
        Self::ChasquiScout,
        Self::JavelinThrower,
        Self::Dromon,
        Self::Cruiser,
        Self::Crusader,
        Self::AncientCavalry,
        Self::Curragh,
        Self::Paratrooper,
        Self::TOWInfantry,
        Self::Flak,
        Self::MobileSAM,
    ];
    pub(in crate::sim) fn conquests_rule(self) -> &'static crate::sim::civ3_unit_rules::UnitRule {
        crate::sim::civ3_unit_rules::get(
            self.conquests_id()
                .expect("campaign unit must have a Conquests identity"),
        )
        .expect("campaign unit must have an editable Conquests rule")
    }

    pub(in crate::sim) fn simple_military() -> Vec<Self> {
        Self::ALL_NATIVE
            .into_iter()
            .filter(|kind| {
                let class = kind.unit_class();
                !kind.is_civilian()
                    && !kind.is_missile()
                    && kind.fuel_capacity() == 0
                    && kind.transport_capacity() < 8
                    && (class.is_ground()
                        || matches!(
                            class,
                            unit_catalog::UnitClass::Air
                                | unit_catalog::UnitClass::Helicopter
                                | unit_catalog::UnitClass::Trireme
                        )
                        || class == unit_catalog::UnitClass::Sea
                            && kind.stats().attack > 0
                            && !kind.has_flag(unit_catalog::UnitFlag::OnlyNativeAttack))
            })
            .collect()
    }

    pub(in crate::sim) fn hp_loss_percent(self) -> i32 {
        // Civ2Civ3 unit classes Air and Helicopter lose 10%; Missile and
        // every supported land/sea class lose none.
        if self.is_aircraft() && !self.is_missile() {
            10
        } else {
            0
        }
    }

    pub(in crate::sim) fn required_tech(self) -> Option<Tech> {
        self.conquests_rule().required_tech()
    }

    pub(in crate::sim) fn required_resources(self) -> Vec<crate::sim::Resource> {
        self.conquests_rule().resources()
    }

    pub(crate) fn name(self) -> &'static str {
        self.conquests_rule().name
    }

    pub(crate) fn is_native_on(
        self,
        terrain: Terrain,
        river: bool,
        road: bool,
        railroad: bool,
        base: Option<Base>,
    ) -> bool {
        if self.is_aircraft() {
            true
        } else if self == Self::Galley {
            matches!(terrain, Terrain::Coast | Terrain::Sea | Terrain::Lake) || river
        } else {
            terrain.is_water() == self.is_naval()
                && (!self.is_big_land()
                    || !matches!(
                        terrain,
                        Terrain::Jungle | Terrain::Mountains | Terrain::Marsh
                    )
                    || road
                    || railroad
                    || matches!(base, Some(Base::Fort | Base::Fortress)))
        }
    }

    pub(crate) fn is_naval(self) -> bool {
        matches!(
            self.unit_class(),
            unit_catalog::UnitClass::Sea | unit_catalog::UnitClass::Trireme
        )
    }

    pub(crate) fn transport_capacity(self) -> usize {
        self.metadata().capacity
    }

    pub(crate) fn can_clean(self) -> bool {
        self.is_worker()
    }

    pub(in crate::sim) fn obsolete_by(self) -> Option<Self> {
        self.conquests_rule()
            .upgrade_target_id()
            .and_then(Self::from_conquests_id)
    }

    pub(crate) fn fuel_capacity(self) -> u8 {
        self.metadata().fuel
    }

    pub(in crate::sim) fn operational_range(self) -> i32 {
        self.conquests_rule().operational_range
    }

    pub(in crate::sim) fn is_missile(self) -> bool {
        matches!(
            self,
            Self::CruiseMissile | Self::ICBM | Self::TacticalNuke
        )
    }

    pub(crate) fn is_nuclear_weapon(self) -> bool {
        matches!(self, Self::ICBM | Self::TacticalNuke)
    }

    pub(crate) fn is_aircraft(self) -> bool {
        self.fuel_capacity() > 0 || self == Self::Helicopter
    }

    pub(in crate::sim) fn can_attack_aircraft(self) -> bool {
        self.metadata().attacks_air
    }

    pub(in crate::sim) fn can_rebase(self) -> bool {
        self.conquests_rule().has_flag_3(12, 3)
    }

    pub(in crate::sim) fn bombard_rate(self) -> i32 {
        self.conquests_rule().rate_of_fire
    }

    pub(crate) fn bombard_range(self) -> i32 {
        self.conquests_rule().bombard_range
    }

    pub(in crate::sim) fn bombard_strength(self) -> i32 {
        self.conquests_rule().bombard_strength
    }

    pub(in crate::sim) fn has_lethal_bombard_against(self, target: Self) -> bool {
        if target.is_naval() {
            self.conquests_rule().has_lethal_sea_bombard()
        } else {
            self.conquests_rule().has_lethal_land_bombard()
        }
    }

    pub(in crate::sim) fn uses_stealth_vision(self) -> bool {
        self.metadata().stealth
    }

    pub(in crate::sim) fn can_occupy_city(self) -> bool {
        let class = self.unit_class();
        !self.is_civilian()
            && (class.can_occupy_city() || class == unit_catalog::UnitClass::SmallLand)
    }

    pub(crate) fn can_carry(self, kind: Self) -> bool {
        if self.conquests_rule().has_flag_1(1, 6) {
            return kind.conquests_rule().has_flag_1(0, 1);
        }
        self.metadata().cargo.contains(&kind.unit_class())
    }

    pub(in crate::sim) fn can_capture_units(self) -> bool {
        self.has_flag(unit_catalog::UnitFlag::Capturer)
    }

    pub(in crate::sim) fn is_capturable(self) -> bool {
        self.has_flag(unit_catalog::UnitFlag::Capturable)
    }

    pub(crate) fn has_hostile_action(self) -> bool {
        use unit_catalog::UnitFlag;
        !self.has_flag(UnitFlag::NonMil) || self.has_flag(UnitFlag::Capturer)
    }

    pub(crate) fn is_military(self) -> bool {
        self.stats().attack > 0
    }

    pub(crate) fn is_civilian(self) -> bool {
        self.has_flag(unit_catalog::UnitFlag::NonMil)
    }

    pub(crate) fn can_fortify(self) -> bool {
        self.unit_class().can_fortify() && !self.has_flag(unit_catalog::UnitFlag::CantFortify)
    }
}

impl Tech {
    pub(crate) fn cost(self) -> i32 {
        let requirements = self.data().map_or(1, |advance| advance.requirement_count);
        research_cost(requirements)
    }
}
