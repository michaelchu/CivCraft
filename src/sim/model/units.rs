//! Unit kinds and their ruleset-facing identity.

use super::research_cost;
use crate::rules::Tech;
use crate::sim::unit_catalog;
use crate::sim::{Base, Terrain};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UnitKind {
    Settlers,
    Worker,
    Migrants,
    Engineer,
    Diplomat,
    Spy,
    Explorer,
    Warrior,
    Phalanx,
    Archer,
    Legion,
    Horseman,
    Chariot,
    Catapult,
    Musketeers,
    Marines,
    Knights,
    Partisan,
    Cannon,
    Bomber,
    Fighter,
    StealthFighter,
    StealthBomber,
    Helicopter,
    Trireme,
    Caravel,
    Galleon,
    Transport,
    Carrier,
    Frigate,
    Ironclad,
    Destroyer,
    Cruiser,
    Battleship,
    Submarine,
    AegisCruiser,
    CruiseMissile,
    Caravan,
    Freight,
    Nuclear,
    BarbarianLeader,
    Paratroopers,
    Pikemen,
    Riflemen,
    AlpineTroops,
    Fanatics,
    MechInf,
    Elephants,
    Crusaders,
    Dragoons,
    Cavalry,
    Armor,
    Artillery,
    Howitzer,
    Leader,
    Awacs,
    Storm,
    // Appended to preserve existing serialized discriminants.
    Scout,
    Longbowman,
    Infantry,
    ModernArmor,
    Privateer,
    JetFighter,
    NuclearSubmarine,
    Icbm,
    TacticalNuke,
}

impl UnitKind {
    // Source ruleset ordering is independent of enum/save discriminants.
    pub(crate) const ALL_NATIVE: [Self; 66] = [
        Self::Settlers,
        Self::Migrants,
        Self::Worker,
        Self::Engineer,
        Self::Warrior,
        Self::Phalanx,
        Self::Archer,
        Self::Legion,
        Self::Pikemen,
        Self::Musketeers,
        Self::Riflemen,
        Self::AlpineTroops,
        Self::Partisan,
        Self::Fanatics,
        Self::Marines,
        Self::Paratroopers,
        Self::MechInf,
        Self::Horseman,
        Self::Chariot,
        Self::Elephants,
        Self::Crusaders,
        Self::Knights,
        Self::Dragoons,
        Self::Cavalry,
        Self::Armor,
        Self::Catapult,
        Self::Cannon,
        Self::Artillery,
        Self::Howitzer,
        Self::Fighter,
        Self::Bomber,
        Self::Helicopter,
        Self::StealthFighter,
        Self::StealthBomber,
        Self::Trireme,
        Self::Caravel,
        Self::Galleon,
        Self::Frigate,
        Self::Ironclad,
        Self::Destroyer,
        Self::Cruiser,
        Self::AegisCruiser,
        Self::Battleship,
        Self::Submarine,
        Self::Carrier,
        Self::Transport,
        Self::CruiseMissile,
        Self::Nuclear,
        Self::Diplomat,
        Self::Spy,
        Self::Caravan,
        Self::Freight,
        Self::Explorer,
        Self::Leader,
        Self::BarbarianLeader,
        Self::Awacs,
        Self::Storm,
        Self::Scout,
        Self::Longbowman,
        Self::Infantry,
        Self::ModernArmor,
        Self::Privateer,
        Self::JetFighter,
        Self::NuclearSubmarine,
        Self::Icbm,
        Self::TacticalNuke,
    ];
    #[cfg(test)]
    pub(crate) const ALL: [Self; 66] = Self::ALL_NATIVE;
    #[cfg(test)]
    pub(crate) fn all() -> impl Iterator<Item = Self> {
        Self::ALL_NATIVE.into_iter()
    }
    pub(in crate::sim) const SIMPLE_MILITARY: [Self; 35] = [
        Self::Warrior,
        Self::Phalanx,
        Self::Archer,
        Self::Legion,
        Self::Pikemen,
        Self::Musketeers,
        Self::Riflemen,
        Self::AlpineTroops,
        Self::Partisan,
        Self::Fanatics,
        Self::Marines,
        Self::Paratroopers,
        Self::MechInf,
        Self::Horseman,
        Self::Chariot,
        Self::Elephants,
        Self::Crusaders,
        Self::Knights,
        Self::Dragoons,
        Self::Cavalry,
        Self::Armor,
        Self::Catapult,
        Self::Cannon,
        Self::Artillery,
        Self::Howitzer,
        Self::Helicopter,
        Self::Frigate,
        Self::Ironclad,
        Self::Destroyer,
        Self::Cruiser,
        Self::AegisCruiser,
        Self::Battleship,
        Self::Longbowman,
        Self::Infantry,
        Self::ModernArmor,
    ];

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

    pub(in crate::sim) fn required_resources(self) -> &'static [crate::sim::Resource] {
        use crate::sim::Resource as R;
        match self {
            Self::Privateer => &[R::Saltpeter, R::Iron],
            Self::Infantry => &[R::Rubber],
            Self::ModernArmor => &[R::Oil, R::Rubber, R::Aluminum],
            Self::JetFighter => &[R::Oil],
            Self::NuclearSubmarine => &[R::Uranium],
            Self::Icbm | Self::TacticalNuke => &[R::Aluminum, R::Uranium],
            _ => &[],
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Pikemen => "Pikemen",
            Self::Riflemen => "Riflemen",
            Self::AlpineTroops => "Alpine Troops",
            Self::Fanatics => "Fanatics",
            Self::MechInf => "Mech. Inf.",
            Self::Elephants => "Elephants",
            Self::Crusaders => "Crusaders",
            Self::Dragoons => "Dragoons",
            Self::Cavalry => "Cavalry",
            Self::Armor => "Armor",
            Self::Artillery => "Artillery",
            Self::Howitzer => "Howitzer",
            Self::Leader => "Leader",
            Self::Awacs => "AWACS",
            Self::Storm => "Storm",
            Self::Settlers => "Settlers",
            Self::Worker => "Workers",
            Self::Migrants => "Migrants",
            Self::Engineer => "Engineers",
            Self::Diplomat => "Diplomat",
            Self::Spy => "Spy",
            Self::Explorer => "Explorer",
            Self::Warrior => "Warriors",
            Self::Phalanx => "Phalanx",
            Self::Archer => "Archers",
            Self::Legion => "Legion",
            Self::Horseman => "Horsemen",
            Self::Chariot => "Chariot",
            Self::Catapult => "Catapult",
            Self::Musketeers => "Musketeers",
            Self::Marines => "Marines",
            Self::Paratroopers => "Paratroopers",
            Self::Knights => "Knights",
            Self::Galleon => "Galleon",
            Self::Transport => "Transport",
            Self::Carrier => "Carrier",
            Self::Frigate => "Frigate",
            Self::Ironclad => "Ironclad",
            Self::Destroyer => "Destroyer",
            Self::Cruiser => "Cruiser",
            Self::Battleship => "Battleship",
            Self::AegisCruiser => "AEGIS Cruiser",
            Self::Submarine => "Submarine",
            Self::CruiseMissile => "Cruise Missile",
            Self::Nuclear => "Nuclear",
            Self::BarbarianLeader => "Barbarian Leader",
            Self::Caravan => "Caravan",
            Self::Freight => "Freight",
            Self::Trireme => "Trireme",
            Self::Caravel => "Caravel",
            Self::Cannon => "Cannon",
            Self::Bomber => "Bomber",
            Self::Fighter => "Fighter",
            Self::Helicopter => "Helicopter",
            Self::StealthFighter => "Stealth Fighter",
            Self::StealthBomber => "Stealth Bomber",
            Self::Partisan => "Partisan",
            Self::Scout => "Scout",
            Self::Longbowman => "Longbowman",
            Self::Infantry => "Infantry",
            Self::ModernArmor => "Modern Armor",
            Self::Privateer => "Privateer",
            Self::JetFighter => "Jet Fighter",
            Self::NuclearSubmarine => "Nuclear Submarine",
            Self::Icbm => "ICBM",
            Self::TacticalNuke => "Tactical Nuke",
        }
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
        } else if self == Self::Trireme {
            matches!(terrain, Terrain::Ocean | Terrain::Lake) || river
        } else {
            terrain.is_water() == self.is_naval()
                && (!self.is_big_land()
                    || !matches!(
                        terrain,
                        Terrain::Jungle | Terrain::Mountains | Terrain::Swamp
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

    pub(in crate::sim) fn earns_naval_veterancy(self) -> bool {
        self.is_naval() && (self.is_military() || self == Self::Carrier)
    }

    pub(crate) fn transport_capacity(self) -> usize {
        self.metadata().capacity
    }

    pub(crate) fn is_diplomatic(self) -> bool {
        matches!(self, Self::Diplomat | Self::Spy)
    }

    pub(crate) fn can_clean(self) -> bool {
        self.is_worker()
    }

    pub(in crate::sim) fn obsolete_by(self) -> &'static [Self] {
        self.metadata().obsolete
    }

    pub(crate) fn fuel_capacity(self) -> u8 {
        self.metadata().fuel
    }

    pub(in crate::sim) fn is_missile(self) -> bool {
        matches!(
            self,
            Self::CruiseMissile | Self::Nuclear | Self::Icbm | Self::TacticalNuke
        )
    }

    pub(crate) fn is_nuclear_weapon(self) -> bool {
        matches!(self, Self::Nuclear | Self::Icbm | Self::TacticalNuke)
    }

    pub(crate) fn is_aircraft(self) -> bool {
        self.fuel_capacity() > 0 || self == Self::Helicopter
    }

    pub(in crate::sim) fn can_attack_aircraft(self) -> bool {
        self.metadata().attacks_air
    }

    pub(in crate::sim) fn bombard_rate(self) -> i32 {
        match self {
            Self::Helicopter => 2,
            Self::Bomber => 3,
            Self::StealthBomber => 4,
            _ => 0,
        }
    }

    pub(in crate::sim) fn uses_stealth_vision(self) -> bool {
        self.metadata().stealth
    }

    pub(in crate::sim) fn can_occupy_city(self) -> bool {
        !self.is_civilian() && self.unit_class().can_occupy_city()
    }

    pub(crate) fn can_carry(self, kind: Self) -> bool {
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
        !self.has_flag(UnitFlag::NonMil)
            || self.has_flag(UnitFlag::Diplomat)
            || self.has_flag(UnitFlag::Spy)
            || self.has_flag(UnitFlag::Capturer)
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
