//! Static Civ2Civ3 unit classes, flags and roles. Enum wire order is kept
//! separately in model.rs; these tables follow the supplied ruleset.
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
    flags: u64,
    roles: u64,
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
                tech: Some(Tech::EXPLOSIVES), /* Explosives */
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
                tech: Some(Tech(77)), /* Tactics */
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
                tech: Some(Tech::GUERILLA_WARFARE), /* Guerilla Warfare */
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
                tech: Some(Tech::GUERILLA_WARFARE), /* Guerilla Warfare */
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
                tech: Some(Tech::COMBINED_ARMS), /* Combined Arms */
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
                tech: Some(Tech(38)), /* Labor Union */
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
                tech: Some(Tech(61)), /* Polytheism */
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
                tech: Some(Tech(40)), /* Leadership */
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
                tech: Some(Tech(77)), /* Tactics */
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
                tech: Some(Tech::MOBILE_WARFARE), /* Mobile Warfare */
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
                tech: Some(Tech::MACHINE_TOOLS), /* Machine Tools */
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
                tech: Some(Tech::ADVANCED_FLIGHT), /* Advanced Flight */
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
                tech: Some(Tech::COMBINED_ARMS), /* Combined Arms */
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
                tech: Some(Tech::MAGNETISM), /* Magnetism */
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
                tech: Some(Tech::NAVIGATION), /* Navigation */
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
                tech: Some(Tech::NAVIGATION), /* Navigation */
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
                tech: Some(Tech::STEAM_ENGINE), /* Steam Engine */
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
                tech: Some(Tech::ENGINEERING), /* Engineering */
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
                tech: Some(Tech::STEEL), /* Steel */
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
                tech: Some(Tech::ROCKETRY), /* Rocketry */
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
                tech: Some(Tech::COMBUSTION), /* Combustion */
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
                tech: Some(Tech::ADVANCED_FLIGHT), /* Advanced Flight */
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
                tech: Some(Tech::ENGINEERING), /* Engineering */
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
                tech: Some(Tech::NUCLEAR_FISSION), /* Nuclear Fission */
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
                tech: Some(Tech::TRADE), /* Trade */
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
    pub(super) fn unit_class(self) -> UnitClass {
        self.metadata().class
    }
    pub(super) fn has_flag(self, flag: UnitFlag) -> bool {
        self.metadata().flags & (1u64 << flag as u32) != 0
    }
    pub(super) fn has_role(self, role: UnitRole) -> bool {
        self.metadata().roles & (1u64 << role as u32) != 0
    }
    pub(super) fn is_worker(self) -> bool {
        self.has_flag(UnitFlag::Workers)
    }
    pub(super) fn has_marine_attack(self) -> bool {
        self.has_flag(UnitFlag::Marines)
    }
    pub(super) fn is_unbribable(self) -> bool {
        self.has_flag(UnitFlag::Unbribable)
    }
    pub(super) fn is_city_buster(self) -> bool {
        self.has_flag(UnitFlag::CityBuster)
    }
    pub(super) fn has_no_build(self) -> bool {
        self.has_flag(UnitFlag::NoBuild)
    }
    pub(super) fn has_shield_to_gold(self) -> bool {
        self.has_flag(UnitFlag::ShieldToGold)
    }
    pub(super) fn has_no_zoc(self) -> bool {
        self.has_flag(UnitFlag::HasNoZOC)
    }
    pub(super) fn has_no_home(self) -> bool {
        self.has_flag(UnitFlag::NoHome)
    }
    pub(super) fn no_veteran(self) -> bool {
        self.has_flag(UnitFlag::NoVeteran)
    }
    pub(super) fn is_big_land(self) -> bool {
        self.unit_class() == UnitClass::BigLand
    }
    pub(super) fn has_terrain_defense(self) -> bool {
        self.unit_class().has_terrain_defense()
    }
    pub(super) fn can_helicopter_field_load(self) -> bool {
        self.metadata().helicopter_field_load
    }
    pub(super) fn can_build_fortress(self) -> bool {
        self.is_worker() || self.has_flag(UnitFlag::Fortress)
    }
    pub(super) fn required_government(self) -> Option<Government> {
        self.metadata().government
    }
    pub(super) fn base_upkeep(self) -> (i32, i32, i32) {
        self.metadata().upkeep
    }
    pub(super) fn max_veteran_levels(self) -> usize {
        self.metadata().veteran_levels
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_fixture_checks_source_stats_capabilities_and_independent_native_order() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.native_unit_catalog_playtest(), (true, true, true));
    }

    #[test]
    fn existing_42_unit_discriminants_preserve_saved_games() {
        use UnitKind::*;
        let legacy = [
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
        ];
        for (index, kind) in legacy.into_iter().enumerate() {
            assert_eq!(
                bincode::serialize(&kind).unwrap(),
                (index as u32).to_le_bytes()
            );
            assert_eq!(
                bincode::deserialize::<UnitKind>(&(index as u32).to_le_bytes()).unwrap(),
                kind
            );
        }
        for kind in UnitKind::ALL_NATIVE {
            assert_eq!(
                bincode::deserialize::<UnitKind>(&bincode::serialize(&kind).unwrap()).unwrap(),
                kind
            );
        }
    }

    #[test]
    fn native_cache_predicate_and_direct_obsolescence_graph_match_rules() {
        let derived: Vec<_> = UnitKind::all()
            .filter(|kind| {
                let class = kind.unit_class();
                !kind.is_civilian()
                    && !kind.is_missile()
                    && kind.fuel_capacity() == 0
                    && kind.transport_capacity() < 8
                    && (class.is_ground()
                        || matches!(
                            class,
                            UnitClass::Air | UnitClass::Helicopter | UnitClass::Trireme
                        )
                        || class == UnitClass::Sea
                            && kind.stats().attack > 0
                            && !kind.has_flag(UnitFlag::OnlyNativeAttack))
            })
            .collect();
        assert_eq!(derived, UnitKind::SIMPLE_MILITARY);
        assert_eq!(UnitKind::Phalanx.obsolete_by(), &[UnitKind::Pikemen]);
        assert_eq!(UnitKind::Musketeers.obsolete_by(), &[UnitKind::Riflemen]);
        assert_eq!(UnitKind::Knights.obsolete_by(), &[UnitKind::Dragoons]);
        assert_eq!(UnitKind::Cannon.obsolete_by(), &[UnitKind::Artillery]);
        assert_eq!(UnitKind::Artillery.obsolete_by(), &[UnitKind::Howitzer]);
        assert_eq!(UnitKind::Frigate.obsolete_by(), &[UnitKind::Ironclad]);
        assert_eq!(UnitKind::Caravel.obsolete_by(), &[UnitKind::Galleon]);
    }

    #[test]
    fn class_and_flags_keep_marines_field_loading_upkeep_and_veterancy_distinct() {
        for kind in [
            UnitKind::MechInf,
            UnitKind::Armor,
            UnitKind::Artillery,
            UnitKind::Howitzer,
        ] {
            assert!(kind.is_big_land());
            assert!(!kind.has_terrain_defense());
            assert!(!UnitKind::Carrier.can_carry(kind));
            assert!(!UnitKind::Trireme.can_carry(kind));
            assert!(UnitKind::Transport.can_carry(kind));
        }
        for kind in [
            UnitKind::Legion,
            UnitKind::Musketeers,
            UnitKind::Riflemen,
            UnitKind::Marines,
        ] {
            assert!(kind.has_marine_attack());
        }
        assert!(UnitKind::AlpineTroops.stats().ignores_terrain);
        assert!(!UnitKind::AlpineTroops.has_flag(UnitFlag::IgZOC));
        assert!(UnitKind::Fanatics.can_helicopter_field_load());
        assert!(!UnitKind::Riflemen.can_helicopter_field_load());
        assert!(UnitKind::Leader.has_no_home());
        assert!(!UnitKind::Leader.has_no_zoc());
        assert_eq!(UnitKind::Leader.max_veteran_levels(), 1);
        assert_eq!(UnitKind::BarbarianLeader.max_veteran_levels(), 1);
        assert!(UnitKind::Storm.no_veteran());
        assert_eq!(UnitKind::Storm.max_veteran_levels(), 4);
        assert_eq!(UnitKind::Crusaders.base_upkeep(), (1, 1, 0));
        assert!(!UnitKind::Fanatics.has_shield_to_gold());
        assert_eq!(UnitKind::Awacs.base_upkeep(), (0, 1, 0));
        assert!(UnitKind::Explorer.has_role(UnitRole::Explorer));
        assert!(!UnitKind::Explorer.has_no_build());
        assert_eq!(
            UnitKind::Explorer
                .required_tech()
                .unwrap()
                .data()
                .unwrap()
                .name,
            "Seafaring"
        );
    }
}
