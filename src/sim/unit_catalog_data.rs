//! Unit metadata types and the Conquests-backed unit metadata accessor.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnitClass {
    Land,
    SmallLand,
    BigLand,
    Sea,
    Trireme,
    Air,
    Helicopter,
    Missile,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Preserve source unit flags even when their gameplay is not implemented yet.
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
    EvacuateFirst,
    FieldUnit,
    Fortress,
    GameLoss,
    HasNoZOC,
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
    Provoking,
    ShieldToGold,
    Submarine,
    Unbribable,
    Unique,
    Workers,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Keep the full source AI strategy vocabulary, even where simulation behavior is pending.
pub(super) enum UnitAiStrategy {
    Offense,
    Defense,
    Artillery,
    Explore,
    Army,
    CruiseMissile,
    AirBombard,
    AirDefense,
    NavalPower,
    AirTransport,
    NavalTransport,
    NavalCarrier,
    Terraform,
    Settle,
    Leader,
    TacticalNuke,
    Icbm,
    NavalMissileTransport,
    Flag,
    King,
}
#[derive(Clone, Copy)]
pub(super) struct UnitMetadata {
    pub(super) class: UnitClass,
    pub(super) flags: u64,
    pub(super) ai_strategies: u32,
    #[allow(dead_code)] // Source unit prerequisite retained for research and advisor behavior.
    pub(super) tech: Option<Tech>,
    pub(super) government: Option<Government>,
    pub(super) cargo: &'static [UnitClass],
    pub(super) capacity: usize,
    pub(super) fuel: u8,
    pub(super) veteran_levels: usize,
    pub(super) helicopter_field_load: bool,
    pub(super) attacks_air: bool,
    pub(super) stealth: bool,
}
impl UnitClass {
    pub(super) fn has_terrain_defense(self) -> bool {
        matches!(self, Self::Land | Self::SmallLand)
    }
    pub(super) fn has_barracks(self) -> bool {
        matches!(self, Self::Land | Self::SmallLand | Self::BigLand)
    }
    pub(super) fn is_ground(self) -> bool {
        matches!(self, Self::Land | Self::SmallLand | Self::BigLand)
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
            Self::Land | Self::SmallLand | Self::BigLand | Self::Missile
        )
    }
    pub(super) fn damage_slows(self) -> bool {
        matches!(
            self,
            Self::Land | Self::SmallLand | Self::BigLand | Self::Sea | Self::Trireme
        )
    }
    pub(super) fn terrain_speed(self) -> bool {
        matches!(self, Self::Land | Self::SmallLand | Self::BigLand)
    }
    pub(super) fn is_unreachable(self) -> bool {
        matches!(self, Self::Missile | Self::Helicopter | Self::Air)
    }
}
impl UnitKind {
    pub(super) fn metadata(self) -> UnitMetadata {
        crate::sim::civ3_unit_rules::get(
            self.conquests_id()
                .expect("campaign unit must have a Conquests identity"),
        )
        .expect("campaign unit must have an editable Conquests rule")
        .metadata()
    }
}
