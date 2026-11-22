//! Action enablers for the campaign's Freeciv-style action rules.
//!
//! Each enabler allows one action when every actor requirement and every
//! target requirement holds. Several enablers for one action are
//! alternatives. The table is in the order the evaluator visits them.

use super::{ActionEnabler, Requirement, RequirementKind, RequirementRange, RequirementValue};
use super::{ResolvedRequirementValue, RuleSourceLocation};
use RequirementKind::{
    Activity, Building, BuildingGenus, CityTile, DiplRel, DiplRelTileOther, Extra, ExtraFlag,
    MaxUnitsOnTile, MinHitPoints, MinMoveFrags, MinSize, Nation, NationGroup, PlayerState, Tech,
    Terrain, TerrainAlter, TerrainClass, TerrainFlag, UnitClass, UnitClassFlag, UnitState,
    UnitType, UnitTypeFlag,
};
use RequirementRange::{CAdjacent, City, Local, Player, Tile};

/// A requirement as written in the table.
#[derive(Clone, Copy)]
pub(super) struct Req {
    pub(super) kind: RequirementKind,
    pub(super) name: &'static str,
    pub(super) range: RequirementRange,
    pub(super) present: bool,
    /// The requirement is still met after the source (a building or
    /// technology) is gone.
    pub(super) survives: bool,
    /// The requirement is omitted from help text.
    pub(super) quiet: bool,
}

/// The requirement holds.
pub(super) const fn is(kind: RequirementKind, name: &'static str, range: RequirementRange) -> Req {
    Req {
        kind,
        name,
        range,
        present: true,
        survives: false,
        quiet: false,
    }
}

/// The requirement must not hold.
pub(super) const fn not(kind: RequirementKind, name: &'static str, range: RequirementRange) -> Req {
    Req {
        kind,
        name,
        range,
        present: false,
        survives: false,
        quiet: false,
    }
}

impl Req {
    pub(super) const fn surviving(mut self) -> Self {
        self.survives = true;
        self
    }

    pub(super) const fn quietly(mut self) -> Self {
        self.quiet = true;
        self
    }
}

struct Enabler {
    rule_name: &'static str,
    action: &'static str,
    actor: &'static [Req],
    target: &'static [Req],
}

const fn enabler(
    rule_name: &'static str,
    action: &'static str,
    actor: &'static [Req],
    target: &'static [Req],
) -> Enabler {
    Enabler {
        rule_name,
        action,
        actor,
        target,
    }
}

static ENABLERS: &[Enabler] = &[
    enabler(
        "sabotage_city",
        "Sabotage City",
        &[
            is(UnitTypeFlag, "Diplomat", Local),
            is(DiplRel, "War", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            not(UnitTypeFlag, "Spy", Local),
        ],
        &[],
    ),
    enabler(
        "sabotage_city_spy",
        "Sabotage City Escape",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(DiplRel, "War", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[],
    ),
    enabler(
        "sabotage_city_target",
        "Targeted Sabotage City Escape",
        &[
            is(DiplRel, "War", Local),
            is(UnitTypeFlag, "Spy", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[],
    ),
    enabler(
        "sabotage_city_production",
        "Sabotage City Production Escape",
        &[
            is(DiplRel, "War", Local),
            is(UnitTypeFlag, "Spy", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[],
    ),
    enabler(
        "establish_embassy",
        "Establish Embassy Stay",
        &[
            is(UnitTypeFlag, "Diplomat", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
            not(DiplRel, "Has real embassy", Local),
            not(UnitTypeFlag, "Spy", Local),
        ],
        &[not(NationGroup, "Barbarian", Player)],
    ),
    enabler(
        "establish_embassy_spy",
        "Establish Embassy",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
            not(DiplRel, "Has real embassy", Local),
        ],
        &[not(NationGroup, "Barbarian", Player)],
    ),
    enabler(
        "establish_embassy_explorer",
        "Establish Embassy Stay",
        &[
            is(UnitType, "Explorer", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
            not(DiplRel, "Has real embassy", Local),
        ],
        &[not(NationGroup, "Barbarian", Player)],
    ),
    enabler(
        "investigate_city",
        "Investigate City",
        &[
            is(UnitTypeFlag, "Diplomat", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
            not(UnitTypeFlag, "Spy", Local),
        ],
        &[],
    ),
    enabler(
        "investigate_city_spy",
        "Investigate City",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[],
    ),
    enabler(
        "investigate_city_explorer",
        "Investigate City",
        &[
            is(UnitType, "Explorer", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[],
    ),
    enabler(
        "steal_tech_random",
        "Steal Tech",
        &[
            not(UnitTypeFlag, "Spy", Local),
            is(UnitTypeFlag, "Diplomat", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[not(NationGroup, "Barbarian", Player)],
    ),
    enabler(
        "steal_tech_random_esc",
        "Steal Tech Escape Expected",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[not(NationGroup, "Barbarian", Player)],
    ),
    enabler(
        "steal_tech_target",
        "Targeted Steal Tech Escape Expected",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[not(NationGroup, "Barbarian", Player)],
    ),
    enabler(
        "poison_city",
        "Poison City Escape",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(DiplRel, "War", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[is(MinSize, "2", City)],
    ),
    enabler(
        "incite_city",
        "Incite City",
        &[
            is(UnitTypeFlag, "Diplomat", Local),
            not(DiplRel, "Alliance", Local),
            not(DiplRel, "Team", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
            not(UnitTypeFlag, "Spy", Local),
        ],
        &[
            not(Building, "Courthouse", City),
            not(Building, "Palace", City),
            not(Building, "Ecclesiastical Palace", City),
            not(Building, "Mausoleum of Mausolos", Player),
        ],
    ),
    enabler(
        "incite_city_spy",
        "Incite City Escape",
        &[
            is(UnitTypeFlag, "Spy", Local),
            not(DiplRel, "Alliance", Local),
            not(DiplRel, "Team", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[
            not(Building, "Courthouse", City),
            not(Building, "Palace", City),
            not(Building, "Ecclesiastical Palace", City),
            not(Building, "Mausoleum of Mausolos", Player),
        ],
    ),
    enabler(
        "bribe_unit",
        "Bribe Unit",
        &[
            is(UnitTypeFlag, "Diplomat", Local),
            is(UnitState, "HasHomeCity", Local),
            not(DiplRel, "Alliance", Local),
            not(DiplRel, "Team", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[
            not(UnitTypeFlag, "Unbribable", Local),
            not(CityTile, "Center", Tile),
            is(MaxUnitsOnTile, "1", Tile),
        ],
    ),
    enabler(
        "sabotage_unit",
        "Sabotage Unit Escape",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(DiplRel, "War", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[
            not(CityTile, "Center", Tile),
            is(MinHitPoints, "2", Local),
            is(MaxUnitsOnTile, "1", Tile),
        ],
    ),
    enabler(
        "eliminate_spy",
        "Spy Attack",
        &[
            is(UnitTypeFlag, "Spy", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            not(DiplRel, "Alliance", Local),
            not(DiplRel, "Team", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[not(CityTile, "Center", Tile)],
    ),
    enabler(
        "trade_route",
        "Establish Trade Route",
        &[
            is(UnitTypeFlag, "TradeRoute", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitState, "HasHomeCity", Local),
        ],
        &[],
    ),
    enabler(
        "help_build_wonder",
        "Help Wonder",
        &[
            is(UnitTypeFlag, "HelpWonder", Local),
            not(DiplRel, "Armistice", Local),
            not(DiplRel, "War", Local),
            not(DiplRel, "Cease-fire", Local),
            not(DiplRel, "Peace", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[is(BuildingGenus, "GreatWonder", Local)],
    ),
    enabler(
        "disband_unit_recover",
        "Disband Unit Recover",
        &[
            not(UnitTypeFlag, "EvacuateFirst", Local),
            not(DiplRel, "War", Local),
            not(DiplRel, "Cease-fire", Local),
            not(DiplRel, "Armistice", Local),
            not(DiplRel, "Peace", Local),
        ],
        &[],
    ),
    enabler(
        "disband_unit",
        "Disband Unit",
        &[not(UnitTypeFlag, "EvacuateFirst", Local)],
        &[],
    ),
    enabler(
        "capture",
        "Capture Units",
        &[
            is(UnitTypeFlag, "Capturer", Local),
            is(DiplRel, "War", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "Foreign", Local),
        ],
        &[
            is(UnitTypeFlag, "Capturable", Local),
            not(UnitState, "Transporting", Local),
            not(CityTile, "Center", Tile),
        ],
    ),
    enabler(
        "bombard_city",
        "Bombard",
        &[
            is(UnitTypeFlag, "Bombarder", Local),
            not(UnitState, "Transported", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "War", Local),
        ],
        &[
            not(TerrainClass, "Oceanic", Tile),
            is(DiplRelTileOther, "War", Local),
        ],
    ),
    enabler(
        "bombard_no_city",
        "Bombard",
        &[
            is(UnitTypeFlag, "Bombarder", Local),
            not(UnitState, "Transported", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "War", Local),
        ],
        &[
            not(TerrainClass, "Oceanic", Tile),
            not(CityTile, "Center", Tile),
        ],
    ),
    enabler(
        "build_city_pioneer",
        "Found City",
        &[
            is(UnitTypeFlag, "Cities", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[
            not(CityTile, "Claimed", Tile),
            not(TerrainFlag, "NoCities", Tile),
        ],
    ),
    enabler(
        "build_city_domestic",
        "Found City",
        &[
            is(UnitTypeFlag, "Cities", Local),
            is(UnitState, "OnLivableTile", Local),
            is(MinMoveFrags, "1", Local),
            not(DiplRel, "Foreign", Local),
        ],
        &[
            is(CityTile, "Claimed", Tile),
            not(TerrainFlag, "NoCities", Tile),
        ],
    ),
    enabler(
        "join_city",
        "Join City",
        &[
            is(UnitTypeFlag, "AddToCity", Local),
            not(DiplRel, "Foreign", Local),
            is(MinMoveFrags, "1", Local),
        ],
        &[],
    ),
    enabler(
        "nuke",
        "Explode Nuclear",
        &[is(UnitTypeFlag, "Nuclear", Local)],
        &[],
    ),
    enabler(
        "nuke_city",
        "Nuke City",
        &[
            is(UnitTypeFlag, "Nuclear", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "War", Local),
        ],
        &[],
    ),
    enabler(
        "nuke_units",
        "Nuke Units",
        &[
            is(UnitTypeFlag, "Nuclear", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "War", Local),
        ],
        &[not(CityTile, "Center", Tile)],
    ),
    enabler(
        "attack_from_native",
        "Attack",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitState, "OnNativeTile", Local),
            is(DiplRel, "War", Local),
        ],
        &[],
    ),
    enabler(
        "attack_marines",
        "Attack",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitTypeFlag, "Marines", Local),
            is(DiplRel, "War", Local),
        ],
        &[],
    ),
    enabler(
        "attack_att_from_non_native",
        "Attack",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitClassFlag, "AttFromNonNative", Local),
            is(DiplRel, "War", Local),
        ],
        &[],
    ),
    enabler(
        "ransom_from_native",
        "Collect Ransom",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitState, "OnNativeTile", Local),
            is(DiplRel, "War", Local),
        ],
        &[is(PlayerState, "Barbarian", Player)],
    ),
    enabler(
        "ransom_marines",
        "Collect Ransom",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitTypeFlag, "Marines", Local),
            is(DiplRel, "War", Local),
        ],
        &[is(PlayerState, "Barbarian", Player)],
    ),
    enabler(
        "ransom_att_from_non_native",
        "Collect Ransom",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitClassFlag, "AttFromNonNative", Local),
            is(DiplRel, "War", Local),
        ],
        &[is(PlayerState, "Barbarian", Player)],
    ),
    enabler(
        "explode_missile",
        "Suicide Attack",
        &[
            not(UnitTypeFlag, "NonMil", Local),
            is(UnitClassFlag, "Missile", Local),
            is(MinMoveFrags, "1", Local),
            is(DiplRel, "War", Local),
        ],
        &[],
    ),
    enabler(
        "conquer_city_native",
        "Conquer City Shrink",
        &[
            is(UnitClassFlag, "CanOccupyCity", Local),
            not(UnitTypeFlag, "NonMil", Local),
            is(DiplRel, "War", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitState, "OnLivableTile", Local),
            not(Nation, "Animal Kingdom", Player),
        ],
        &[is(MaxUnitsOnTile, "0", Tile)],
    ),
    enabler(
        "conquer_city_marines",
        "Conquer City Shrink 2",
        &[
            is(UnitClassFlag, "CanOccupyCity", Local),
            not(UnitTypeFlag, "NonMil", Local),
            is(DiplRel, "War", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitTypeFlag, "Marines", Local),
            not(UnitState, "OnLivableTile", Local),
            not(Nation, "Animal Kingdom", Player),
        ],
        &[is(MaxUnitsOnTile, "0", Tile)],
    ),
    enabler(
        "change_home_city",
        "Home City",
        &[
            not(UnitTypeFlag, "NoHome", Local),
            is(UnitState, "HasHomeCity", Local),
            not(DiplRel, "Foreign", Local),
        ],
        &[],
    ),
    enabler(
        "change_home_city_diplo",
        "Home City",
        &[
            is(UnitTypeFlag, "Diplomat", Local),
            not(DiplRel, "Foreign", Local),
        ],
        &[],
    ),
    enabler(
        "paradrop_base_to_claimed",
        "Paradrop Unit Enter",
        &[
            is(UnitTypeFlag, "Paratroopers", Local),
            is(MinMoveFrags, "6", Local),
            not(UnitState, "Transporting", Local),
            is(ExtraFlag, "ParadropFrom", Tile),
            not(DiplRel, "Peace", Local),
            not(DiplRel, "War", Local),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[is(CityTile, "Claimed", Tile)],
    ),
    enabler(
        "paradrop_base_to_unclaimed",
        "Paradrop Unit Enter",
        &[
            is(UnitTypeFlag, "Paratroopers", Local),
            is(MinMoveFrags, "6", Local),
            not(UnitState, "Transporting", Local),
            is(ExtraFlag, "ParadropFrom", Tile),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[not(CityTile, "Claimed", Tile)],
    ),
    enabler(
        "paradrop_base_to_war_conquer",
        "Paradrop Unit Enter Conquer",
        &[
            is(UnitTypeFlag, "Paratroopers", Local),
            is(MinMoveFrags, "6", Local),
            not(UnitState, "Transporting", Local),
            is(ExtraFlag, "ParadropFrom", Tile),
            is(DiplRel, "War", Local),
            is(UnitClassFlag, "CanOccupyCity", Local),
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "HutFrighten", Local),
            not(Nation, "Animal Kingdom", Player),
        ],
        &[is(CityTile, "Claimed", Tile)],
    ),
    enabler(
        "paradrop_city_to_claimed",
        "Paradrop Unit Enter",
        &[
            is(UnitTypeFlag, "Paratroopers", Local),
            is(MinMoveFrags, "6", Local),
            not(UnitState, "Transporting", Local),
            is(CityTile, "Center", Tile),
            not(DiplRel, "Peace", Local),
            not(DiplRel, "War", Local),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[is(CityTile, "Claimed", Tile)],
    ),
    enabler(
        "paradrop_city_to_unclaimed",
        "Paradrop Unit Enter",
        &[
            is(UnitTypeFlag, "Paratroopers", Local),
            is(MinMoveFrags, "6", Local),
            not(UnitState, "Transporting", Local),
            is(CityTile, "Center", Tile),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[not(CityTile, "Claimed", Tile)],
    ),
    enabler(
        "paradrop_city_to_war_conquer",
        "Paradrop Unit Enter Conquer",
        &[
            is(UnitTypeFlag, "Paratroopers", Local),
            is(MinMoveFrags, "6", Local),
            not(UnitState, "Transporting", Local),
            is(CityTile, "Center", Tile),
            is(DiplRel, "War", Local),
            is(UnitClassFlag, "CanOccupyCity", Local),
            not(UnitTypeFlag, "NonMil", Local),
            not(UnitClassFlag, "HutFrighten", Local),
            not(Nation, "Animal Kingdom", Player),
        ],
        &[is(CityTile, "Claimed", Tile)],
    ),
    enabler(
        "upgrade_unit",
        "Upgrade Unit",
        &[not(DiplRel, "Foreign", Local)],
        &[],
    ),
    enabler(
        "airlift_unit",
        "Airlift Unit",
        &[
            is(UnitClassFlag, "Airliftable", Local),
            not(UnitState, "Transporting", Local),
            is(MinMoveFrags, "1", Local),
            is(CityTile, "Center", Tile),
        ],
        &[],
    ),
    enabler(
        "transform_terrain",
        "Transform Terrain",
        &[
            is(Tech, "Fusion Power", Player),
            is(UnitTypeFlag, "Transform", Local),
            is(UnitTypeFlag, "Workers", Local),
        ],
        &[],
    ),
    enabler(
        "cultivate",
        "Cultivate",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[],
    ),
    enabler(
        "plant",
        "Plant",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[],
    ),
    enabler(
        "pillage",
        "Pillage",
        &[is(UnitClassFlag, "CanPillage", Local)],
        &[],
    ),
    enabler("clean", "Clean", &[is(UnitTypeFlag, "Workers", Local)], &[]),
    enabler(
        "fortify_terrain",
        "Fortify",
        &[
            is(UnitClassFlag, "CanFortify", Local),
            not(UnitTypeFlag, "Cant_Fortify", Local),
            not(TerrainFlag, "NoFortify", Tile),
            not(Activity, "Fortified", Local),
        ],
        &[],
    ),
    enabler(
        "fortify_city",
        "Fortify",
        &[
            is(UnitClassFlag, "CanFortify", Local),
            not(UnitTypeFlag, "Cant_Fortify", Local),
            is(CityTile, "Center", Tile),
            not(Activity, "Fortified", Local),
        ],
        &[],
    ),
    enabler(
        "road",
        "Build Road",
        &[is(UnitTypeFlag, "Workers", Local)],
        &[is(TerrainAlter, "CanRoad", Tile)],
    ),
    enabler(
        "base",
        "Build Base",
        &[],
        &[is(TerrainAlter, "CanBase", Tile)],
    ),
    enabler(
        "mining",
        "Build Mine",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            not(Terrain, "Deep Ocean", Tile),
            is(TerrainAlter, "CanMine", Tile),
        ],
    ),
    enabler(
        "mining_ocean",
        "Build Mine",
        &[
            is(UnitTypeFlag, "Workers", Local),
            is(Tech, "Miniaturization", Player),
        ],
        &[is(Terrain, "Deep Ocean", Tile)],
    ),
    enabler(
        "irrigate_src_tech_basic_irrigation",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            is(Tech, "Electricity", Player),
            not(UnitClass, "Sea", Local),
        ],
        &[
            not(Extra, "Irrigation", Tile),
            is(TerrainAlter, "CanIrrigate", Tile),
        ],
    ),
    enabler(
        "irrigate_src_ocean_basic_irrigation",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(TerrainClass, "Oceanic", CAdjacent),
            not(Extra, "Irrigation", Tile),
            is(TerrainAlter, "CanIrrigate", Tile),
        ],
    ),
    enabler(
        "irrigate_src_extra_basic_irrigation",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(ExtraFlag, "IrrigationSource", CAdjacent),
            not(Extra, "Irrigation", Tile),
            is(TerrainAlter, "CanIrrigate", Tile),
        ],
    ),
    enabler(
        "irrigate_src_tech_not_in_desert",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            is(Tech, "Electricity", Player),
            not(UnitClass, "Sea", Local),
        ],
        &[
            not(Terrain, "Desert", Tile),
            is(TerrainAlter, "CanIrrigate", Tile),
        ],
    ),
    enabler(
        "irrigate_src_ocean_not_in_desert",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(TerrainClass, "Oceanic", CAdjacent),
            not(Terrain, "Desert", Tile),
            is(TerrainAlter, "CanIrrigate", Tile),
        ],
    ),
    enabler(
        "irrigate_src_extra_not_in_desert",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(ExtraFlag, "IrrigationSource", CAdjacent),
            not(Terrain, "Desert", Tile),
            is(TerrainAlter, "CanIrrigate", Tile),
        ],
    ),
    enabler(
        "irrigate_src_tech_desert_but_allowed",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            is(Tech, "Electricity", Player),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(Terrain, "Desert", Tile),
            is(ExtraFlag, "AllowsFarmlandOnDesert", Tile),
        ],
    ),
    enabler(
        "irrigate_src_ocean_desert_but_allowed",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(TerrainClass, "Oceanic", CAdjacent),
            is(Terrain, "Desert", Tile),
            is(ExtraFlag, "AllowsFarmlandOnDesert", Tile),
        ],
    ),
    enabler(
        "irrigate_src_extra_desert_but_allowed",
        "Build Irrigation",
        &[
            is(UnitTypeFlag, "Workers", Local),
            not(UnitClass, "Sea", Local),
        ],
        &[
            is(ExtraFlag, "IrrigationSource", CAdjacent),
            is(Terrain, "Desert", Tile),
            is(ExtraFlag, "AllowsFarmlandOnDesert", Tile),
        ],
    ),
    enabler(
        "deboard",
        "Transport Deboard",
        &[
            is(UnitState, "OnLivableTile", Local),
            is(UnitState, "Transported", Local),
        ],
        &[is(UnitState, "Transporting", Local)],
    ),
    enabler(
        "board",
        "Transport Board",
        &[
            not(DiplRel, "Armistice", Local),
            not(DiplRel, "War", Local),
            not(DiplRel, "Cease-fire", Local),
            not(DiplRel, "Peace", Local),
            not(DiplRel, "Never met", Local),
        ],
        &[],
    ),
    enabler(
        "unload",
        "Transport Unload",
        &[is(UnitState, "Transporting", Local)],
        &[
            is(UnitState, "OnLivableTile", Local),
            is(UnitState, "Transported", Local),
        ],
    ),
    enabler(
        "disembark_from_native_tile",
        "Transport Disembark",
        &[
            is(UnitState, "Transported", Local),
            is(MinMoveFrags, "1", Local),
            is(UnitState, "OnNativeTile", Local),
        ],
        &[],
    ),
    enabler(
        "disembark_from_native_city",
        "Transport Disembark",
        &[
            is(UnitState, "Transported", Local),
            is(MinMoveFrags, "1", Local),
            is(CityTile, "Center", Tile),
        ],
        &[],
    ),
    enabler(
        "disembark_from_non_native",
        "Transport Disembark 2",
        &[
            is(UnitState, "Transported", Local),
            is(MinMoveFrags, "1", Local),
            not(UnitState, "OnNativeTile", Local),
            not(CityTile, "Center", Tile),
        ],
        &[],
    ),
    enabler(
        "embark",
        "Transport Embark",
        &[
            is(MinMoveFrags, "1", Local),
            not(DiplRel, "Armistice", Local),
            not(DiplRel, "War", Local),
            not(DiplRel, "Cease-fire", Local),
            not(DiplRel, "Peace", Local),
            not(DiplRel, "Never met", Local),
        ],
        &[],
    ),
    enabler(
        "regular_move",
        "Unit Move",
        &[
            is(MinMoveFrags, "1", Local),
            not(UnitState, "Transported", Local),
        ],
        &[],
    ),
    enabler(
        "hut_enter_from_native_tile",
        "Enter Hut",
        &[
            is(MinMoveFrags, "1", Local),
            is(UnitState, "OnNativeTile", Local),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[],
    ),
    enabler(
        "hut_enter_from_native_city",
        "Enter Hut",
        &[
            is(MinMoveFrags, "1", Local),
            is(CityTile, "Center", Tile),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[],
    ),
    enabler(
        "hut_enter_from_non_native",
        "Enter Hut 2",
        &[
            is(MinMoveFrags, "1", Local),
            not(UnitState, "OnNativeTile", Local),
            not(CityTile, "Center", Tile),
            not(UnitClassFlag, "HutFrighten", Local),
        ],
        &[],
    ),
    enabler(
        "hut_frighten_from_native_tile",
        "Frighten Hut",
        &[
            is(MinMoveFrags, "1", Local),
            is(UnitState, "OnNativeTile", Local),
            is(UnitClassFlag, "HutFrighten", Local),
        ],
        &[],
    ),
    enabler(
        "hut_frighten_from_native_city",
        "Frighten Hut",
        &[
            is(MinMoveFrags, "1", Local),
            is(CityTile, "Center", Tile),
            is(UnitClassFlag, "HutFrighten", Local),
        ],
        &[],
    ),
    enabler(
        "hut_frighten_from_non_native",
        "Frighten Hut 2",
        &[
            is(MinMoveFrags, "1", Local),
            not(UnitState, "OnNativeTile", Local),
            not(CityTile, "Center", Tile),
            is(UnitClassFlag, "HutFrighten", Local),
        ],
        &[],
    ),
    enabler(
        "gain_veterancy",
        "Gain Veterancy",
        &[not(UnitTypeFlag, "NoVeteran", Local)],
        &[],
    ),
    enabler("civil_war", "Civil War", &[], &[]),
    enabler("finish_unit", "Finish Unit", &[], &[]),
    enabler("finish_building", "Finish Building", &[], &[]),
];

pub(super) fn requirements(label: &str, key: &str, reqs: &[Req]) -> Vec<Requirement> {
    reqs.iter()
        .map(|req| Requirement {
            kind: req.kind,
            name: RequirementValue::String(req.name.to_owned()),
            resolved: None::<ResolvedRequirementValue>,
            range: req.range,
            present: req.present,
            survives: req.survives,
            quiet: req.quiet,
            source: RuleSourceLocation {
                file: "action_enablers.rs".into(),
                section: label.to_owned(),
                key: key.to_owned(),
                line: 0,
            },
        })
        .collect()
}

/// The campaign's action enablers, in evaluation order.
pub(super) fn campaign_action_enablers() -> Vec<ActionEnabler> {
    ENABLERS
        .iter()
        .map(|enabler| ActionEnabler {
            rule_name: enabler.rule_name.to_owned(),
            action: enabler.action.to_owned(),
            actor_requirements: requirements(
                &format!("enabler_{}", enabler.rule_name),
                "actor_reqs",
                enabler.actor,
            ),
            target_requirements: requirements(
                &format!("enabler_{}", enabler.rule_name),
                "target_reqs",
                enabler.target,
            ),
        })
        .collect()
}
