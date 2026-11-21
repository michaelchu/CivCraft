//! Unit kinds and their ruleset-facing identity.

use crate::rules::Tech;
use crate::sim::unit_catalog;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct UnitKind(pub(crate) usize);

#[allow(dead_code, non_upper_case_globals)]
impl UnitKind {
    // Conquests campaign identities in `content/units.toml` order.
    pub(crate) const Settler: Self = Self(0);
    pub(crate) const Worker: Self = Self(1);
    pub(crate) const Scout: Self = Self(2);
    pub(crate) const Explorer: Self = Self(3);
    pub(crate) const Marine: Self = Self(4);
    pub(crate) const ModernParatrooper: Self = Self(5);
    pub(crate) const Warrior: Self = Self(6);
    pub(crate) const Archer: Self = Self(7);
    pub(crate) const Spearman: Self = Self(8);
    pub(crate) const Swordsman: Self = Self(9);
    pub(crate) const Chariot: Self = Self(10);
    pub(crate) const Horseman: Self = Self(11);
    pub(crate) const Pikeman: Self = Self(12);
    pub(crate) const Longbowman: Self = Self(13);
    pub(crate) const Musketman: Self = Self(14);
    pub(crate) const Knight: Self = Self(15);
    pub(crate) const Rifleman: Self = Self(16);
    pub(crate) const Cavalry: Self = Self(17);
    pub(crate) const Infantry: Self = Self(18);
    pub(crate) const Tank: Self = Self(19);
    pub(crate) const MechInfantry: Self = Self(20);
    pub(crate) const ModernArmor: Self = Self(21);
    pub(crate) const Catapult: Self = Self(22);
    pub(crate) const Cannon: Self = Self(23);
    pub(crate) const Artillery: Self = Self(24);
    pub(crate) const RadarArtillery: Self = Self(25);
    pub(crate) const CruiseMissile: Self = Self(26);
    pub(crate) const TacticalNuke: Self = Self(27);
    pub(crate) const ICBM: Self = Self(28);
    pub(crate) const Galley: Self = Self(29);
    pub(crate) const Caravel: Self = Self(30);
    pub(crate) const Frigate: Self = Self(31);
    pub(crate) const Galleon: Self = Self(32);
    pub(crate) const Ironclad: Self = Self(33);
    pub(crate) const Transport: Self = Self(34);
    pub(crate) const Carrier: Self = Self(35);
    pub(crate) const Submarine: Self = Self(36);
    pub(crate) const Destroyer: Self = Self(37);
    pub(crate) const Battleship: Self = Self(38);
    pub(crate) const AegisCruiser: Self = Self(39);
    pub(crate) const NuclearSubmarine: Self = Self(40);
    pub(crate) const Fighter: Self = Self(41);
    pub(crate) const Bomber: Self = Self(42);
    pub(crate) const Helicopter: Self = Self(43);
    pub(crate) const JetFighter: Self = Self(44);
    pub(crate) const StealthFighter: Self = Self(45);
    pub(crate) const StealthBomber: Self = Self(46);
    pub(crate) const Leader: Self = Self(47);
    pub(crate) const Army: Self = Self(48);
    pub(crate) const JaguarWarrior: Self = Self(49);
    pub(crate) const Bowman: Self = Self(50);
    pub(crate) const Hoplite: Self = Self(51);
    pub(crate) const Impi: Self = Self(52);
    pub(crate) const Legionary: Self = Self(53);
    pub(crate) const Immortals: Self = Self(54);
    pub(crate) const WarChariot: Self = Self(55);
    pub(crate) const Rider: Self = Self(56);
    pub(crate) const MountedWarrior: Self = Self(57);
    pub(crate) const Musketeer: Self = Self(58);
    pub(crate) const Samurai: Self = Self(59);
    pub(crate) const WarElephant: Self = Self(60);
    pub(crate) const Cossack: Self = Self(61);
    pub(crate) const Panzer: Self = Self(62);
    pub(crate) const ManOWar: Self = Self(63);
    pub(crate) const F15: Self = Self(64);
    pub(crate) const Privateer: Self = Self(65);
    pub(crate) const Keshik: Self = Self(66);
    pub(crate) const Conquistador: Self = Self(67);
    pub(crate) const Berserk: Self = Self(68);
    pub(crate) const Sipahi: Self = Self(69);
    pub(crate) const GallicSwordsman: Self = Self(70);
    pub(crate) const AnsarWarrior: Self = Self(71);
    pub(crate) const NumidianMercenary: Self = Self(72);
    pub(crate) const Hwacha: Self = Self(73);
    pub(crate) const MedievalInfantry: Self = Self(74);
    pub(crate) const Guerilla: Self = Self(75);
    pub(crate) const EnkiduWarrior: Self = Self(76);
    pub(crate) const ThreeManChariot: Self = Self(77);
    pub(crate) const Carrack: Self = Self(78);
    pub(crate) const SwissMercenary: Self = Self(79);
    pub(crate) const Trebuchet: Self = Self(80);
    pub(crate) const ChasquiScout: Self = Self(81);
    pub(crate) const JavelinThrower: Self = Self(82);
    pub(crate) const Dromon: Self = Self(83);
    pub(crate) const Cruiser: Self = Self(84);
    pub(crate) const Crusader: Self = Self(85);
    pub(crate) const AncientCavalry: Self = Self(86);
    pub(crate) const Curragh: Self = Self(87);
    pub(crate) const Paratrooper: Self = Self(88);
    pub(crate) const TOWInfantry: Self = Self(89);
    pub(crate) const Flak: Self = Self(90);
    pub(crate) const MobileSAM: Self = Self(91);

    pub(crate) const fn index(self) -> usize {
        self.0
    }

    pub(crate) fn count() -> usize {
        crate::rules::native::units().units.len()
    }

    pub(crate) fn all() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        (0..Self::count()).map(Self)
    }

    pub(crate) fn all_slice() -> &'static [Self] {
        static ALL: std::sync::OnceLock<Vec<UnitKind>> = std::sync::OnceLock::new();
        ALL.get_or_init(|| Self::all().collect())
    }

    /// Matching source identity in `content/units.toml`.
    pub(crate) fn conquests_id(self) -> Option<&'static str> {
        crate::rules::native::units()
            .units
            .get(self.0)
            .map(|unit| unit.id.as_str())
    }

    pub(crate) fn from_civ3_name(name: &str) -> Option<Self> {
        crate::rules::native::units()
            .units
            .iter()
            .position(|unit| unit.name == name)
            .map(Self)
    }

    pub(crate) fn from_conquests_id(id: &str) -> Option<Self> {
        crate::rules::native::units()
            .units
            .iter()
            .position(|unit| unit.id == id)
            .map(Self)
    }

    pub(in crate::sim) fn simple_military() -> Vec<Self> {
        Self::all()
            .filter(|kind| {
                let class = kind.unit_class();
                *kind != Self::Army
                    && !kind.is_civilian()
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
        self.metadata().tech
    }

    pub(in crate::sim) fn required_resources(self) -> Vec<crate::sim::Resource> {
        self.conquests_rule().resources()
    }

    pub(crate) fn name(self) -> &'static str {
        self.conquests_rule().name
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
        crate::sim::action_rules::freeciv_unit_type_fuel_capacity(self)
            .expect("native unit mission budget")
    }

    pub(in crate::sim) fn operational_range(self) -> i32 {
        self.conquests_rule().operational_range
    }

    pub(in crate::sim) fn is_missile(self) -> bool {
        self.unit_class() == unit_catalog::UnitClass::Missile
    }

    pub(crate) fn is_nuclear_weapon(self) -> bool {
        self.has_flag(unit_catalog::UnitFlag::Nuclear)
    }

    pub(crate) fn is_aircraft(self) -> bool {
        self.fuel_capacity() > 0 || self.unit_class() == unit_catalog::UnitClass::Helicopter
    }

    pub(in crate::sim) fn can_attack_aircraft(self) -> bool {
        self.metadata().attacks_air
    }

    pub(in crate::sim) fn bombard_rate(self) -> i32 {
        self.conquests_rule().rate_of_fire
    }

    pub(in crate::sim) fn bombard_range(self) -> i32 {
        self.conquests_rule().bombard_range
    }

    pub(in crate::sim) fn bombard_strength(self) -> i32 {
        self.conquests_rule().bombard_strength
    }

    pub(in crate::sim) fn has_bombard_effects(self) -> bool {
        self.conquests_rule().bombard_effects > 0
    }

    pub(in crate::sim) fn has_lethal_bombard_against(self, target: Self) -> bool {
        if target.is_naval() {
            self.conquests_rule().has_lethal_sea_bombard()
        } else {
            self.conquests_rule().has_lethal_land_bombard()
        }
    }

    pub(in crate::sim) fn can_occupy_city(self) -> bool {
        let class = self.unit_class();
        !self.is_civilian()
            && (class.can_occupy_city() || class == unit_catalog::UnitClass::SmallLand)
    }

    pub(crate) fn can_carry(self, kind: Self) -> bool {
        crate::sim::native_unit_rules::can_carry(self, kind)
    }

    pub(crate) fn has_hostile_action(self) -> bool {
        use unit_catalog::UnitFlag;
        !self.has_flag(UnitFlag::NonMil) || self.has_flag(UnitFlag::Capturer)
    }

    pub(crate) fn is_military(self) -> bool {
        self == Self::Army || self.attack_strength() > 0
    }

    /// Strength consumed by the retained combat-action engine. Ordinary attacks
    /// use PRTO attack; its Cruise Missile action consumes PRTO bombard strength.
    pub(crate) fn attack_strength(self) -> i32 {
        crate::sim::action_rules::freeciv_missile_attack_strength(self)
            .unwrap_or_else(|| self.stats().attack)
    }

    pub(crate) fn is_civilian(self) -> bool {
        self.has_flag(unit_catalog::UnitFlag::NonMil)
    }

    pub(crate) fn can_fortify(self) -> bool {
        self.unit_class().can_fortify() && !self.has_flag(unit_catalog::UnitFlag::CantFortify)
    }
}
