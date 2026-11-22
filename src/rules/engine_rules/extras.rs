//! The campaign's terrain extras and roads.
//!
//! An extra is a tile feature: a road, a mine, a river, a base, a resource.
//! `requirements` say where it can be built, `native_to` which unit classes
//! can enter or defend on it, and `conflicts` which extras it replaces.

use super::Requirement;
use super::RequirementKind::{
    CityTile, Extra, MaxLatitude, MinLatitude, Tech, Terrain, TerrainClass, TerrainFlag, UnitClass,
    UnitTypeFlag,
};
use super::RequirementRange::{Local, Player, Tile};
use super::action_enablers::{Req, is, not, requirements};
use super::loader::resolve_requirements;
use std::sync::OnceLock;

/// An extra as written in the table. Omitted fields take the values in `BASE`.
struct ExtraSpec {
    /// Section suffix in the original ruleset, such as `oil_well`.
    id: &'static str,
    /// The rule name saves and requirements use, such as `Oil Well`.
    name: &'static str,
    /// A different player-facing name, when there is one.
    display_name: Option<&'static str>,
    flags: &'static [&'static str],
    /// Unit classes that can enter and defend on it. `None` for extras
    /// that are not terrain a unit can stand on.
    native_to: Option<&'static [&'static str]>,
    conflicts: &'static [&'static str],
    /// Extras this one can be built across, such as a road over a river.
    bridged_over: &'static [&'static str],
    /// Turns to build, overriding the terrain's. Zero defers to the terrain.
    build_time: i32,
    build_time_factor: i32,
    removal_time: i32,
    removal_time_factor: i32,
    /// Percent defense bonus for native units.
    defense_bonus: i32,
    requirements: &'static [Req],
}

const BASE: ExtraSpec = ExtraSpec {
    id: "",
    name: "",
    display_name: None,
    flags: &[],
    native_to: None,
    conflicts: &[],
    bridged_over: &[],
    build_time: 0,
    build_time_factor: 1,
    removal_time: 0,
    removal_time_factor: 1,
    defense_bonus: 0,
    requirements: &[],
};

pub(crate) struct ExtraRule {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) flags: &'static [&'static str],
    pub(crate) native_to: Option<&'static [&'static str]>,
    pub(crate) conflicts: &'static [&'static str],
    pub(crate) bridged_over: &'static [&'static str],
    pub(crate) build_time: i32,
    pub(crate) build_time_factor: i32,
    pub(crate) removal_time: i32,
    pub(crate) removal_time_factor: i32,
    pub(crate) defense_bonus: i32,
    pub(crate) requirements: Vec<Requirement>,
}

impl ExtraRule {
    /// The section name this extra had in the original ruleset, used in
    /// diagnostics.
    pub(crate) fn section(&self) -> String {
        format!("extra_{}", self.id)
    }

    pub(crate) fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f.eq_ignore_ascii_case(flag))
    }

    /// Whether `name` is this extra's rule name, id or display name.
    fn is_called(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
            || self.id.eq_ignore_ascii_case(name)
            || self.display_name.eq_ignore_ascii_case(name)
    }
}

/// A road-like extra: its movement cost and how units move along it.
pub(crate) struct RoadRule {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    /// Movement fragments to move along it, or -1 for none.
    pub(crate) move_cost: i32,
    /// `Cardinal`, `Relaxed` or `FastAlways` when set.
    pub(crate) move_mode: Option<&'static str>,
}

#[rustfmt::skip]
static EXTRAS: &[ExtraSpec] = &[
    ExtraSpec { id: "irrigation", name: "Irrigation", flags: &["IrrigationSource"], conflicts: &["Mine", "Oil Well"], ..BASE },
    ExtraSpec { id: "mine", name: "Mine", conflicts: &["Irrigation", "Farmland"], requirements: &[not(TerrainClass, "Oceanic", Tile).quietly(), not(UnitClass, "Sea", Local).quietly()], ..BASE },
    ExtraSpec { id: "oil_well", name: "Oil Well", conflicts: &["Irrigation", "Farmland"], requirements: &[is(Tech, "Refining", Player), is(TerrainFlag, "Oil", Tile).quietly(), is(Extra, "Mine", Tile), not(UnitClass, "Sea", Local).quietly()], ..BASE },
    ExtraSpec { id: "oil_platform", name: "Oil Platform", conflicts: &["Buoy"], requirements: &[is(Tech, "Miniaturization", Player), is(Terrain, "Deep Ocean", Tile)], ..BASE },
    ExtraSpec { id: "pollution", name: "Pollution", flags: &["GlobalWarming"], requirements: &[not(TerrainFlag, "NoPollution", Tile).quietly()], ..BASE },
    ExtraSpec { id: "hut", name: "Hut", display_name: Some("Minor Tribe Village"), requirements: &[is(TerrainClass, "Land", Tile), is(MaxLatitude, "980", Tile), is(MinLatitude, "-980", Tile)], ..BASE },
    ExtraSpec { id: "farmland", name: "Farmland", conflicts: &["Mine", "Oil Well"], requirements: &[is(Tech, "Refrigeration", Player), is(Extra, "Irrigation", Tile)], ..BASE },
    ExtraSpec { id: "fallout", name: "Fallout", flags: &["NuclearWinter"], requirements: &[not(TerrainFlag, "NoPollution", Tile).quietly()], ..BASE },
    ExtraSpec { id: "fort", name: "Fort", flags: &["NativeTile", "DiplomatDefense"], native_to: Some(&["Land", "Small Land", "Merchant", "Big Land"]), conflicts: &["Airstrip", "Airbase"], build_time_factor: 2, requirements: &[is(Tech, "Construction", Player), is(TerrainClass, "Land", Tile), is(UnitTypeFlag, "Fortress", Local), not(CityTile, "Center", Tile).quietly()], ..BASE },
    ExtraSpec { id: "fortress", name: "Fortress", flags: &["NativeTile", "DiplomatDefense"], native_to: Some(&["Land", "Small Land", "Merchant", "Big Land"]), conflicts: &["Airstrip", "Airbase"], build_time_factor: 2, requirements: &[is(Extra, "Fort", Tile), is(Tech, "Construction", Player), is(TerrainClass, "Land", Tile), is(UnitTypeFlag, "Fortress", Local), not(CityTile, "Center", Tile).quietly(), not(Extra, "River", Tile)], ..BASE },
    ExtraSpec { id: "airstrip", name: "Airstrip", flags: &["Refuel", "ParadropFrom"], native_to: Some(&["Air", "Helicopter", "Missile"]), conflicts: &["Fort", "Fortress"], build_time_factor: 2, requirements: &[is(Tech, "Radio", Player), is(TerrainClass, "Land", Tile), is(UnitTypeFlag, "Workers", Local), not(CityTile, "Center", Tile).quietly()], ..BASE },
    ExtraSpec { id: "airbase", name: "Airbase", flags: &["Refuel", "ParadropFrom"], native_to: Some(&["Air", "Helicopter", "Missile"]), conflicts: &["Fort", "Fortress"], build_time_factor: 2, requirements: &[is(Extra, "Airstrip", Tile), is(Tech, "Radio", Player), is(TerrainClass, "Land", Tile), is(UnitTypeFlag, "Workers", Local), not(CityTile, "Center", Tile).quietly(), not(Extra, "River", Tile)], ..BASE },
    ExtraSpec { id: "buoy", name: "Buoy", flags: &["ShowFlag"], conflicts: &["Oil Platform"], build_time: 3, requirements: &[is(Tech, "Radio", Player), is(TerrainClass, "Oceanic", Tile), is(UnitTypeFlag, "Workers", Local), not(CityTile, "Center", Tile).quietly()], ..BASE },
    ExtraSpec { id: "ruins", name: "Ruins", flags: &["TerrChangeRemoves"], requirements: &[is(TerrainClass, "Land", Tile), not(CityTile, "Center", Tile).quietly()], ..BASE },
    ExtraSpec { id: "road", name: "Road", flags: &["NativeTile", "AlwaysOnCityCenter"], native_to: Some(&["Land", "Small Land", "Big Land", "Merchant"]), bridged_over: &["River"], requirements: &[is(UnitTypeFlag, "Workers", Local), not(UnitClass, "Sea", Local).quietly(), is(TerrainClass, "Land", Tile)], ..BASE },
    ExtraSpec { id: "railroad", name: "Railroad", flags: &["NativeTile", "AutoOnCityCenter"], native_to: Some(&["Land", "Small Land", "Big Land", "Merchant"]), bridged_over: &["River"], build_time: 3, requirements: &[is(Tech, "Railroad", Player), is(Extra, "Road", Tile), is(UnitTypeFlag, "Workers", Local), not(UnitClass, "Sea", Local).quietly(), is(TerrainClass, "Land", Tile)], ..BASE },
    ExtraSpec { id: "maglev", name: "Maglev", flags: &["NativeTile", "AutoOnCityCenter"], native_to: Some(&["Land", "Small Land"]), bridged_over: &["River"], build_time: 3, requirements: &[is(Tech, "Superconductors", Player), is(Extra, "Railroad", Tile), is(UnitTypeFlag, "Workers", Local), not(UnitClass, "Sea", Local).quietly(), is(TerrainClass, "Land", Tile)], ..BASE },
    ExtraSpec { id: "river", name: "River", flags: &["NativeTile", "IrrigationSource", "AllowsFarmlandOnDesert"], native_to: Some(&["Land", "Small Land", "Merchant", "Trireme"]), defense_bonus: 25, ..BASE },
    ExtraSpec { id: "gold", name: "Gold", ..BASE },
    ExtraSpec { id: "iron", name: "Iron", ..BASE },
    ExtraSpec { id: "game", name: "Game", ..BASE },
    ExtraSpec { id: "furs", name: "Furs", ..BASE },
    ExtraSpec { id: "coal", name: "Coal", ..BASE },
    ExtraSpec { id: "fish", name: "Fish", ..BASE },
    ExtraSpec { id: "fruit", name: "Fruit", ..BASE },
    ExtraSpec { id: "gems", name: "Gems", ..BASE },
    ExtraSpec { id: "buffalo", name: "Buffalo", ..BASE },
    ExtraSpec { id: "wheat", name: "Wheat", ..BASE },
    ExtraSpec { id: "oasis", name: "Oasis", flags: &["AllowsFarmlandOnDesert"], ..BASE },
    ExtraSpec { id: "peat", name: "Peat", ..BASE },
    ExtraSpec { id: "pheasant", name: "Pheasant", ..BASE },
    ExtraSpec { id: "resources", name: "Resources", ..BASE },
    ExtraSpec { id: "ivory", name: "Ivory", ..BASE },
    ExtraSpec { id: "silk", name: "Silk", ..BASE },
    ExtraSpec { id: "spice", name: "Spice", ..BASE },
    ExtraSpec { id: "whales", name: "Whales", ..BASE },
    ExtraSpec { id: "wine", name: "Wine", ..BASE },
    ExtraSpec { id: "oil", name: "Oil", ..BASE },
];

#[rustfmt::skip]
pub(crate) static ROADS: &[RoadRule] = &[
    RoadRule { id: "road", name: "road", move_cost: 2, move_mode: None },
    RoadRule { id: "railroad", name: "railroad", move_cost: 1, move_mode: None },
    RoadRule { id: "maglev", name: "maglev", move_cost: 0, move_mode: None },
    RoadRule { id: "river", name: "river", move_cost: 2, move_mode: Some("Cardinal") },
];

/// Every extra, in the order Freeciv lists them.
pub(crate) fn campaign_extras() -> &'static [ExtraRule] {
    static BUILT: OnceLock<Vec<ExtraRule>> = OnceLock::new();
    BUILT.get_or_init(|| {
        EXTRAS
            .iter()
            .map(|spec| {
                let mut reqs =
                    requirements(&format!("extra_{}", spec.id), "reqs", spec.requirements);
                resolve_requirements(&mut reqs);
                ExtraRule {
                    id: spec.id,
                    name: spec.name,
                    display_name: spec.display_name.unwrap_or(spec.name),
                    flags: spec.flags,
                    native_to: spec.native_to,
                    conflicts: spec.conflicts,
                    bridged_over: spec.bridged_over,
                    build_time: spec.build_time,
                    build_time_factor: spec.build_time_factor,
                    removal_time: spec.removal_time,
                    removal_time_factor: spec.removal_time_factor,
                    defense_bonus: spec.defense_bonus,
                    requirements: reqs,
                }
            })
            .collect()
    })
}

/// The extra with this rule name.
pub(crate) fn extra_by_name(name: &str) -> Option<&'static ExtraRule> {
    campaign_extras()
        .iter()
        .find(|extra| extra.name.eq_ignore_ascii_case(name))
}

/// The first extra called `name` by its rule name, id or display name.
pub(crate) fn extra_by_any_name(name: &str) -> Option<&'static ExtraRule> {
    campaign_extras().iter().find(|extra| extra.is_called(name))
}

pub(crate) fn road_by_name(name: &str) -> Option<&'static RoadRule> {
    ROADS
        .iter()
        .find(|road| road.name.eq_ignore_ascii_case(name))
}
