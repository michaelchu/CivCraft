//! Static Civ2Civ3 unit classes, flags, roles and per-unit ruleset data.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnitClass {
    Land,
    SmallLand,
    BigLand,
    Merchant,
    Sea,
    Trireme,
    Air,
    Helicopter,
    Missile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnitFlag {
    AddToCity,
    AirAttacker,
    BadCityDefender,
    Bombarder,
    CantFortify,
    Capturable,
    Capturer,
    Cities,
    CityBuster,
    Diplomat,
    EvacuateFirst,
    Fanatic,
    FieldUnit,
    Flagless,
    Fortress,
    GameLoss,
    HasNoZOC,
    HelpWonder,
    IgTer,
    IgZOC,
    Marines,
    NoBuild,
    NoHome,
    NoVeteran,
    NonMil,
    Nuclear,
    OneAttack,
    OnlyNativeAttack,
    Paratroopers,
    ProvidesRansom,
    Provoking,
    RandomMovement,
    ShieldToGold,
    Spy,
    Submarine,
    SuperSpy,
    TradeRoute,
    Transform,
    Unbribable,
    Unique,
    Workers,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnitRole {
    AttackFastStartUnit,
    AttackStrongStartUnit,
    Barbarian,
    BarbarianBoat,
    BarbarianBuild,
    BarbarianBuildTech,
    BarbarianLeader,
    BarbarianSea,
    BarbarianSeaTech,
    BarbarianTech,
    CitiesStartUnit,
    DefendGood,
    DefendGoodStartUnit,
    DefendOk,
    DefendOkStartUnit,
    DiplomatStartUnit,
    Explorer,
    ExplorerStartUnit,
    FerryStartUnit,
    Ferryboat,
    FirstBuild,
    Hunter,
    Hut,
    HutTech,
    KingStartUnit,
    Partisan,
    Settlers,
    WorkerStartUnit,
}
#[derive(Clone, Copy)]
pub(super) struct UnitMetadata {
    pub(super) class: UnitClass,
    pub(super) flags: u64,
    pub(super) roles: u64,
    pub(super) tech: Option<Tech>,
    pub(super) government: Option<Government>,
    pub(super) obsolete: &'static [UnitKind],
    pub(super) cargo: &'static [UnitClass],
    pub(super) capacity: usize,
    pub(super) fuel: u8,
    pub(super) upkeep: (i32, i32, i32),
    pub(super) veteran_levels: usize,
    pub(super) helicopter_field_load: bool,
    pub(super) attacks_air: bool,
    pub(super) stealth: bool,
}
impl UnitClass {
    pub(super) fn has_terrain_defense(self) -> bool {
        matches!(self, Self::Land | Self::SmallLand | Self::Merchant)
    }
    pub(super) fn has_barracks(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Merchant
        )
    }
    #[cfg(test)]
    pub(super) fn is_ground(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Merchant
        )
    }
    pub(super) fn has_zoc(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Merchant | Self::Trireme
        )
    }
    pub(super) fn can_fortify(self) -> bool {
        matches!(self, Self::Land | Self::SmallLand | Self::BigLand)
    }
    pub(super) fn can_occupy_city(self) -> bool {
        matches!(self, Self::Land | Self::BigLand | Self::Helicopter)
    }
    pub(super) fn build_anywhere(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Merchant
        )
    }
    pub(super) fn damage_slows(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Sea | Self::Trireme
        )
    }
    pub(super) fn terrain_speed(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Merchant
        )
    }
    pub(super) fn is_unreachable(self) -> bool {
        matches!(self, Self::Missile | Self::Helicopter | Self::Air)
    }
}
impl UnitKind {
    pub(super) fn metadata(self) -> UnitMetadata {
        use UnitClass as C;
        use UnitFlag as F;
        use UnitRole as R;
        match self {
            Self::Settlers => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::Workers as u32)
                    | (1u64 << F::CantFortify as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::AddToCity as u32)
                    | (1u64 << F::Cities as u32)
                    | (1u64 << F::Fortress as u32),
                roles: (1u64 << R::CitiesStartUnit as u32),
                tech: None,
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 0, 0),
                veteran_levels: 1,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Migrants => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::Workers as u32)
                    | (1u64 << F::CantFortify as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::AddToCity as u32)
                    | (1u64 << F::Capturable as u32)
                    | (1u64 << F::Fortress as u32),
                roles: (1u64 << R::Settlers as u32),
                tech: Some(Tech::POTTERY), /* Pottery */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 0, 0),
                veteran_levels: 1,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Worker => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::Workers as u32)
                    | (1u64 << F::CantFortify as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Settlers as u32) | (1u64 << R::WorkerStartUnit as u32),
                tech: None,
                government: None,
                obsolete: &[Self::Engineer],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 1,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Engineer => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::Workers as u32)
                    | (1u64 << F::CantFortify as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::Transform as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Settlers as u32) | (1u64 << R::WorkerStartUnit as u32),
                tech: Some(Tech::ENGINEERING), /* Engineering */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Warrior => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32)
                    | (1u64 << R::DefendOkStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32),
                tech: None,
                government: None,
                obsolete: &[Self::Musketeers],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Phalanx => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendGoodStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32),
                tech: Some(Tech::BRONZE_WORKING), /* Bronze Working */
                government: None,
                obsolete: &[Self::Pikemen],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Archer => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32) | (1u64 << R::Hut as u32),
                tech: Some(Tech::WARRIOR_CODE), /* Warrior Code */
                government: None,
                obsolete: &[Self::Musketeers],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Legion => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Marines as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32)
                    | (1u64 << R::BarbarianBuild as u32)
                    | (1u64 << R::BarbarianSea as u32),
                tech: Some(Tech::IRON_WORKING), /* Iron Working */
                government: None,
                obsolete: &[Self::Musketeers],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Pikemen => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendGoodStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32),
                tech: Some(Tech(28)), /* Feudalism */
                government: None,
                obsolete: &[Self::Musketeers],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Musketeers => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Marines as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendGoodStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32)
                    | (1u64 << R::HutTech as u32)
                    | (1u64 << R::BarbarianTech as u32)
                    | (1u64 << R::BarbarianBuildTech as u32)
                    | (1u64 << R::BarbarianSeaTech as u32),
                tech: Some(Tech::GUNPOWDER), /* Gunpowder */
                government: None,
                obsolete: &[Self::Riflemen],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Riflemen => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Marines as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendGoodStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32),
                tech: Some(Tech(17)), /* Conscription */
                government: None,
                obsolete: &[Self::Marines],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::AlpineTroops => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::IgTer as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32) | (1u64 << R::DefendOkStartUnit as u32),
                tech: Some(Tech::MILITARY_TRADITION), /* Military Tradition */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: true,
                attacks_air: false,
                stealth: false,
            },
            Self::Partisan => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::IgTer as u32)
                    | (1u64 << F::IgZOC as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendGoodStartUnit as u32)
                    | (1u64 << R::Partisan as u32)
                    | (1u64 << R::BarbarianTech as u32),
                tech: Some(Tech::NATIONALISM), /* Nationalism */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: true,
                attacks_air: false,
                stealth: false,
            },
            Self::Fanatics => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Fanatic as u32)
                    | (1u64 << F::AddToCity as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32),
                roles: (1u64 << R::DefendGood as u32) | (1u64 << R::DefendGoodStartUnit as u32),
                tech: Some(Tech::NATIONALISM), /* Nationalism */
                government: Some(Government::Fundamentalism),
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 0, 0),
                veteran_levels: 4,
                helicopter_field_load: true,
                attacks_air: false,
                stealth: false,
            },
            Self::Marines => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Marines as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32),
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendOkStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32)
                    | (1u64 << R::BarbarianSeaTech as u32),
                tech: Some(Tech::AMPHIBIOUS_WARFARE), /* Amphibious Warfare */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: true,
                attacks_air: false,
                stealth: false,
            },
            Self::Paratroopers => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Paratroopers as u32)
                    | (1u64 << F::Capturer as u32)
                    | (1u64 << F::Fortress as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32),
                tech: Some(Tech::ADVANCED_FLIGHT), /* Advanced Flight */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: true,
                attacks_air: false,
                stealth: false,
            },
            Self::MechInf => UnitMetadata {
                class: C::BigLand,
                flags: 0,
                roles: (1u64 << R::DefendGood as u32)
                    | (1u64 << R::DefendGoodStartUnit as u32)
                    | (1u64 << R::FirstBuild as u32),
                tech: Some(Tech::COMPUTERS), /* Computers */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Horseman => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackFastStartUnit as u32)
                    | (1u64 << R::Hut as u32)
                    | (1u64 << R::Barbarian as u32)
                    | (1u64 << R::Hunter as u32),
                tech: Some(Tech::HORSEBACK_RIDING), /* Horseback Riding */
                government: None,
                obsolete: &[Self::Knights],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Chariot => UnitMetadata {
                class: C::BigLand,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackFastStartUnit as u32),
                tech: Some(Tech::THE_WHEEL), /* The Wheel */
                government: None,
                obsolete: &[Self::Dragoons],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Elephants => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32)
                    | (1u64 << R::AttackFastStartUnit as u32)
                    | (1u64 << R::Hunter as u32),
                tech: Some(Tech::CHIVALRY), /* Chivalry */
                government: None,
                obsolete: &[Self::Dragoons],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Crusaders => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackFastStartUnit as u32) | (1u64 << R::Hunter as u32),
                tech: Some(Tech(53)), /* Monotheism */
                government: None,
                obsolete: &[Self::Dragoons],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Knights => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32)
                    | (1u64 << R::AttackFastStartUnit as u32)
                    | (1u64 << R::HutTech as u32)
                    | (1u64 << R::BarbarianTech as u32)
                    | (1u64 << R::BarbarianBuildTech as u32)
                    | (1u64 << R::Hunter as u32),
                tech: Some(Tech::CHIVALRY), /* Chivalry */
                government: None,
                obsolete: &[Self::Dragoons],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Dragoons => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackFastStartUnit as u32)
                    | (1u64 << R::BarbarianBuildTech as u32)
                    | (1u64 << R::Hunter as u32),
                tech: Some(Tech::MILITARY_TRADITION), /* Military Tradition */
                government: None,
                obsolete: &[Self::Cavalry],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Cavalry => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::Capturer as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackFastStartUnit as u32) | (1u64 << R::Hunter as u32),
                tech: Some(Tech::MILITARY_TRADITION), /* Military Tradition */
                government: None,
                obsolete: &[Self::Armor],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Armor => UnitMetadata {
                class: C::BigLand,
                flags: (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32)
                    | (1u64 << R::AttackFastStartUnit as u32)
                    | (1u64 << R::Hunter as u32),
                tech: Some(Tech::MOTORIZED_TRANSPORTATION), /* Motorized Transportation */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Catapult => UnitMetadata {
                class: C::BigLand,
                flags: (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackStrongStartUnit as u32),
                tech: Some(Tech::MATHEMATICS), /* Mathematics */
                government: None,
                obsolete: &[Self::Cannon],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Cannon => UnitMetadata {
                class: C::BigLand,
                flags: (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackStrongStartUnit as u32)
                    | (1u64 << R::BarbarianTech as u32)
                    | (1u64 << R::BarbarianBuildTech as u32),
                tech: Some(Tech::METALLURGY), /* Metallurgy */
                government: None,
                obsolete: &[Self::Artillery],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Artillery => UnitMetadata {
                class: C::BigLand,
                flags: (1u64 << F::CityBuster as u32),
                roles: (1u64 << R::AttackStrongStartUnit as u32),
                tech: Some(Tech::REPLACEABLE_PARTS), /* Replaceable Parts */
                government: None,
                obsolete: &[Self::Howitzer],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Howitzer => UnitMetadata {
                class: C::BigLand,
                flags: (1u64 << F::CityBuster as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::AttackStrongStartUnit as u32),
                tech: Some(Tech(68)), /* Robotics */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Fighter => UnitMetadata {
                class: C::Air,
                flags: (1u64 << F::AirAttacker as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32) | (1u64 << R::DefendOkStartUnit as u32),
                tech: Some(Tech::FLIGHT), /* Flight */
                government: None,
                obsolete: &[Self::StealthFighter],
                cargo: &[],
                capacity: 0,
                fuel: 2,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: true,
                stealth: false,
            },
            Self::Bomber => UnitMetadata {
                class: C::Air,
                flags: (1u64 << F::FieldUnit as u32)
                    | (1u64 << F::OneAttack as u32)
                    | (1u64 << F::AirAttacker as u32)
                    | (1u64 << F::Bombarder as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::FLIGHT), /* Flight */
                government: None,
                obsolete: &[Self::StealthBomber],
                cargo: &[],
                capacity: 0,
                fuel: 3,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Helicopter => UnitMetadata {
                class: C::Helicopter,
                flags: (1u64 << F::OneAttack as u32)
                    | (1u64 << F::AirAttacker as u32)
                    | (1u64 << F::Bombarder as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::ADVANCED_FLIGHT), /* Advanced Flight */
                government: None,
                obsolete: &[],
                cargo: &[C::Land],
                capacity: 1,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::StealthFighter => UnitMetadata {
                class: C::Air,
                flags: (1u64 << F::AirAttacker as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32) | (1u64 << R::DefendGoodStartUnit as u32),
                tech: Some(Tech::STEALTH), /* Stealth */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 2,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: true,
                stealth: true,
            },
            Self::StealthBomber => UnitMetadata {
                class: C::Air,
                flags: (1u64 << F::FieldUnit as u32)
                    | (1u64 << F::OneAttack as u32)
                    | (1u64 << F::AirAttacker as u32)
                    | (1u64 << F::Bombarder as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32) | (1u64 << R::DefendOkStartUnit as u32),
                tech: Some(Tech::STEALTH), /* Stealth */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 3,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: true,
            },
            Self::Trireme => UnitMetadata {
                class: C::Trireme,
                flags: (1u64 << F::BadCityDefender as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Ferryboat as u32) | (1u64 << R::FerryStartUnit as u32),
                tech: Some(Tech::MAP_MAKING), /* Map Making */
                government: None,
                obsolete: &[Self::Transport],
                cargo: &[C::Land, C::SmallLand, C::Merchant],
                capacity: 2,
                fuel: 0,
                upkeep: (1, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Caravel => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32)
                    | (1u64 << F::OnlyNativeAttack as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::NonMil as u32),
                roles: (1u64 << R::Ferryboat as u32)
                    | (1u64 << R::FerryStartUnit as u32)
                    | (1u64 << R::BarbarianBoat as u32),
                tech: Some(Tech::ASTRONOMY), /* Astronomy */
                government: None,
                obsolete: &[Self::Galleon],
                cargo: &[C::Land, C::SmallLand, C::Merchant],
                capacity: 3,
                fuel: 0,
                upkeep: (1, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Galleon => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Ferryboat as u32)
                    | (1u64 << R::FerryStartUnit as u32)
                    | (1u64 << R::BarbarianBoat as u32),
                tech: Some(Tech::MAGNETISM), /* Magnetism */
                government: None,
                obsolete: &[Self::Transport],
                cargo: &[C::Land, C::SmallLand, C::BigLand, C::Merchant],
                capacity: 4,
                fuel: 0,
                upkeep: (1, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Frigate => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Hunter as u32),
                tech: Some(Tech::MAGNETISM), /* Magnetism */
                government: None,
                obsolete: &[Self::Ironclad],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Ironclad => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Hunter as u32),
                tech: Some(Tech::STEAM_POWER), /* Steam Power */
                government: None,
                obsolete: &[Self::Destroyer],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Destroyer => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Hunter as u32),
                tech: Some(Tech::COMBUSTION), /* Combustion */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Cruiser => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32) | (1u64 << R::DefendOkStartUnit as u32),
                tech: Some(Tech::COMBUSTION), /* Combustion */
                government: None,
                obsolete: &[Self::AegisCruiser],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::AegisCruiser => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32) | (1u64 << R::DefendGoodStartUnit as u32),
                tech: Some(Tech::ROBOTICS), /* Robotics */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Battleship => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32) | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendOk as u32) | (1u64 << R::DefendOkStartUnit as u32),
                tech: Some(Tech::MASS_PRODUCTION), /* Mass Production */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Submarine => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32)
                    | (1u64 << F::OnlyNativeAttack as u32)
                    | (1u64 << F::Submarine as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Hunter as u32),
                tech: Some(Tech::MASS_PRODUCTION), /* Mass Production */
                government: None,
                obsolete: &[],
                cargo: &[C::Missile],
                capacity: 8,
                fuel: 0,
                upkeep: (1, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Carrier => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::BadCityDefender as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DefendGood as u32),
                tech: Some(Tech::MASS_PRODUCTION), /* Mass Production */
                government: None,
                obsolete: &[],
                cargo: &[C::Land, C::Air, C::Missile, C::Helicopter],
                capacity: 8,
                fuel: 0,
                upkeep: (1, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Transport => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::Workers as u32)
                    | (1u64 << F::BadCityDefender as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Ferryboat as u32) | (1u64 << R::FerryStartUnit as u32),
                tech: Some(Tech::COMBUSTION), /* Combustion */
                government: None,
                obsolete: &[],
                cargo: &[C::Land, C::SmallLand, C::BigLand, C::Merchant],
                capacity: 8,
                fuel: 0,
                upkeep: (1, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::CruiseMissile => UnitMetadata {
                class: C::Missile,
                flags: (1u64 << F::OneAttack as u32)
                    | (1u64 << F::AirAttacker as u32)
                    | (1u64 << F::CityBuster as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::ROCKETRY), /* Rocketry */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 1,
                upkeep: (0, 1, 1),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: true,
                stealth: false,
            },
            Self::Nuclear => UnitMetadata {
                class: C::Missile,
                flags: (1u64 << F::FieldUnit as u32)
                    | (1u64 << F::OneAttack as u32)
                    | (1u64 << F::Nuclear as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::SPACE_FLIGHT), /* Space Flight */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 1,
                upkeep: (0, 1, 1),
                veteran_levels: 1,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Diplomat => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::Diplomat as u32)
                    | (1u64 << F::IgZOC as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DiplomatStartUnit as u32),
                tech: Some(Tech::ALPHABET), /* Alphabet */
                government: None,
                obsolete: &[Self::Spy],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: true,
            },
            Self::Spy => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::Diplomat as u32)
                    | (1u64 << F::IgZOC as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Spy as u32)
                    | (1u64 << F::Provoking as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::DiplomatStartUnit as u32),
                tech: Some(Tech::ESPIONAGE), /* Espionage */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: true,
            },
            Self::Caravan => UnitMetadata {
                class: C::Merchant,
                flags: (1u64 << F::TradeRoute as u32)
                    | (1u64 << F::HelpWonder as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Capturable as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::ECONOMICS), /* Economics */
                government: None,
                obsolete: &[Self::Freight],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Freight => UnitMetadata {
                class: C::Merchant,
                flags: (1u64 << F::TradeRoute as u32)
                    | (1u64 << F::HelpWonder as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::Capturable as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::THE_CORPORATION), /* The Corporation */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Explorer => UnitMetadata {
                class: C::SmallLand,
                flags: (1u64 << F::IgTer as u32)
                    | (1u64 << F::IgZOC as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::CantFortify as u32)
                    | (1u64 << F::ShieldToGold as u32),
                roles: (1u64 << R::Explorer as u32) | (1u64 << R::ExplorerStartUnit as u32),
                tech: Some(Tech(71)), /* Seafaring */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Leader => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::IgZOC as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::NoBuild as u32)
                    | (1u64 << F::NoHome as u32)
                    | (1u64 << F::SuperSpy as u32)
                    | (1u64 << F::EvacuateFirst as u32)
                    | (1u64 << F::Unbribable as u32)
                    | (1u64 << F::GameLoss as u32)
                    | (1u64 << F::Unique as u32)
                    | (1u64 << F::Provoking as u32),
                roles: (1u64 << R::KingStartUnit as u32),
                tech: None,
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 0, 0),
                veteran_levels: 1,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::BarbarianLeader => UnitMetadata {
                class: C::Land,
                flags: (1u64 << F::IgZOC as u32)
                    | (1u64 << F::NonMil as u32)
                    | (1u64 << F::Unbribable as u32)
                    | (1u64 << F::ProvidesRansom as u32)
                    | (1u64 << F::EvacuateFirst as u32)
                    | (1u64 << F::SuperSpy as u32)
                    | (1u64 << F::NoHome as u32)
                    | (1u64 << F::NoBuild as u32),
                roles: (1u64 << R::BarbarianLeader as u32),
                tech: None,
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 0, 0),
                veteran_levels: 1,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Awacs => UnitMetadata {
                class: C::Air,
                flags: (1u64 << F::HasNoZOC as u32) | (1u64 << F::ShieldToGold as u32),
                roles: 0,
                tech: Some(Tech::ADVANCED_FLIGHT), /* Advanced Flight */
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 3,
                upkeep: (0, 1, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
            Self::Storm => UnitMetadata {
                class: C::Sea,
                flags: (1u64 << F::NoBuild as u32)
                    | (1u64 << F::CantFortify as u32)
                    | (1u64 << F::Unbribable as u32)
                    | (1u64 << F::HasNoZOC as u32)
                    | (1u64 << F::NoVeteran as u32)
                    | (1u64 << F::RandomMovement as u32)
                    | (1u64 << F::Flagless as u32),
                roles: 0,
                tech: None,
                government: None,
                obsolete: &[],
                cargo: &[],
                capacity: 0,
                fuel: 0,
                upkeep: (0, 0, 0),
                veteran_levels: 4,
                helicopter_field_load: false,
                attacks_air: false,
                stealth: false,
            },
        }
    }
}
