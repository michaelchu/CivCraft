//! Terrain sprite selection, blending, and tile surface rendering.

use crate::rules::Tech;
use crate::sim::PLAYER_SLOTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Terrain {
    /// Civ III water categories, kept in the old wire slots: old shallow Ocean
    /// became Sea, and old Deep Ocean became Ocean.
    Sea,
    Ocean,
    Lake,
    Glacier,
    Jungle,
    Desert,
    Forest,
    Grassland,
    Hills,
    Mountains,
    Plains,
    Marsh,
    Tundra,
    // Appended so existing bincode terrain discriminants remain stable.
    Coast,
    FloodPlains,
    Volcano,
    GrasslandShield,
}

impl Terrain {
    pub(crate) fn resource_options(self) -> &'static [Resource] {
        use Resource::*;
        match self {
            Self::Lake => &[Fish],
            Self::Coast => &[Fish],
            Self::Sea | Self::Ocean => &[Fish, Whales],
            Self::Glacier => &[], // legacy polar marker; not a Civ III terrain
            Self::FloodPlains => &[Wheat],
            Self::GrasslandShield => &[Cattle, Horses, Wine, Tobacco],
            Self::Volcano => &[],
            Self::Desert => &[Oasis, Oil, Incense, Saltpeter],
            Self::Forest => &[Furs, Silk, Horses, Coal, Rubber],
            Self::Grassland => &[Cattle, Horses, Wine, Tobacco],
            Self::Hills => &[Coal, Wine, Iron, Aluminum, Saltpeter],
            Self::Jungle => &[Gems, TropicalFruit, Dyes, Rubber, Sugar],
            Self::Mountains => &[Gold, Iron, Aluminum, Uranium],
            Self::Plains => &[Cattle, Wheat, Horses, Iron, Oil, Ivory],
            Self::Marsh => &[Spice, Oil, Sugar],
            Self::Tundra => &[Game, Furs, Oil],
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Sea => "Sea",
            Self::Ocean => "Ocean",
            Self::Coast => "Coast",
            Self::Lake => "Fresh Water Lake",
            Self::Glacier => "Glacier",
            Self::Jungle => "Jungle",
            Self::Desert => "Desert",
            Self::Forest => "Forest",
            Self::Grassland => "Grassland",
            Self::Hills => "Hills",
            Self::Mountains => "Mountains",
            Self::Plains => "Plains",
            Self::Marsh => "Marsh",
            Self::Tundra => "Tundra",
            Self::FloodPlains => "Flood Plains",
            Self::Volcano => "Volcano",
            Self::GrasslandShield => "Grassland (Shield)",
        }
    }

    pub(crate) fn is_water(self) -> bool {
        matches!(self, Self::Coast | Self::Sea | Self::Ocean | Self::Lake)
    }

    pub(crate) fn movement_cost(self) -> i32 {
        match self {
            Self::Forest | Self::Hills | Self::Marsh | Self::Jungle => 2,
            Self::Mountains | Self::Volcano => 3,
            _ => 1,
        }
    }

    pub(crate) fn defense_bonus(self) -> i32 {
        match self {
            Self::Forest | Self::Jungle => 25,
            Self::Hills => 50,
            Self::Mountains => 100,
            Self::Marsh => 20,
            Self::Volcano => 80,
            _ => 0,
        }
    }

    pub(crate) fn base_yields(self) -> (i32, i32, i32) {
        match self {
            Self::Coast => (1, 0, 2),
            Self::Sea => (1, 0, 1),
            Self::Ocean => (1, 0, 0),
            Self::Lake => (2, 0, 2),
            Self::Glacier => (0, 0, 0),
            Self::FloodPlains => (3, 0, 0),
            Self::Volcano => (0, 3, 0),
            Self::GrasslandShield => (2, 1, 0),
            Self::Jungle => (1, 1, 0),
            Self::Desert => (0, 1, 0),
            Self::Forest => (1, 2, 0),
            Self::Grassland => (2, 0, 0),
            Self::Hills => (1, 1, 0),
            Self::Mountains => (0, 1, 0),
            Self::Plains => (1, 1, 0),
            Self::Marsh => (1, 0, 0),
            Self::Tundra => (1, 0, 0),
        }
    }

    pub(crate) fn irrigation_bonus(self) -> i32 {
        match self {
            Self::Desert
            | Self::Grassland
            | Self::GrasslandShield
            | Self::Hills
            | Self::Plains
            | Self::Marsh
            | Self::FloodPlains => 1,
            _ => 0,
        }
    }

    pub(crate) fn mine_bonus(self) -> i32 {
        match self {
            Self::Desert
            | Self::Grassland
            | Self::GrasslandShield
            | Self::Plains
            | Self::Tundra => 1,
            Self::Hills | Self::Mountains => 2,
            _ => 0,
        }
    }

    pub(crate) fn road_trade_bonus(self) -> i32 {
        match self {
            Self::Desert
            | Self::Grassland
            | Self::GrasslandShield
            | Self::Plains
            | Self::Tundra
            | Self::FloodPlains => 1,
            _ => 0,
        }
    }

    pub(crate) fn road_time(self) -> i32 {
        if self.is_water() {
            0
        } else {
            self.movement_cost() * 2
        }
    }

    pub(crate) fn irrigation_time(self) -> i32 {
        match self {
            Self::Desert
            | Self::Grassland
            | Self::GrasslandShield
            | Self::Plains
            | Self::Marsh
            | Self::FloodPlains => 5,
            Self::Hills => 10,
            _ => 0,
        }
    }

    pub(crate) fn mine_time(self) -> i32 {
        match self {
            Self::Desert
            | Self::Grassland
            | Self::GrasslandShield
            | Self::Plains
            | Self::Tundra => 5,
            Self::Glacier | Self::Hills | Self::Mountains => 10,
            _ => 0,
        }
    }

    pub(crate) fn allows_city(self) -> bool {
        !self.is_water() && !matches!(self, Self::Mountains | Self::Glacier)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Resource {
    // Civilization III: Conquests catalog; luxury happiness and strategic
    // supply effects are not implemented yet.
    Aluminum,
    Coal,
    Horses,
    Iron,
    Oil,
    Rubber,
    Saltpeter,
    Uranium,
    Dyes,
    Furs,
    Gems,
    Incense,
    Ivory,
    Silk,
    Spice,
    Wine,
    Cattle,
    Fish,
    Gold,
    Game,
    Oasis,
    Sugar,
    Tobacco,
    TropicalFruit,
    Whales,
    Wheat,
}

impl Resource {
    /// Strategic resources are present from map generation, but their map icon
    /// stays hidden until the civilization discovers the revealing technology.
    /// Bonus and luxury resources are visible from the start.
    pub(crate) fn visibility_tech(self) -> Option<Tech> {
        match self {
            Self::Aluminum => Some(Tech::ROCKETRY),
            Self::Coal => Some(Tech::STEAM_POWER),
            Self::Horses => Some(Tech::THE_WHEEL),
            Self::Iron => Some(Tech::IRON_WORKING),
            Self::Oil => Some(Tech::REFINING),
            Self::Rubber => Some(Tech::REPLACEABLE_PARTS),
            Self::Saltpeter => Some(Tech::GUNPOWDER),
            Self::Uranium => Some(Tech::NUCLEAR_FISSION),
            Self::Dyes
            | Self::Furs
            | Self::Gems
            | Self::Incense
            | Self::Ivory
            | Self::Silk
            | Self::Spice
            | Self::Wine
            | Self::Cattle
            | Self::Fish
            | Self::Gold
            | Self::Game
            | Self::Oasis
            | Self::Sugar
            | Self::Tobacco
            | Self::TropicalFruit
            | Self::Whales
            | Self::Wheat => None,
        }
    }

    pub(crate) fn bonus(self) -> (i32, i32, i32) {
        match self {
            Self::Aluminum => (0, 2, 0),
            Self::Coal => (0, 2, 0),
            Self::Horses => (0, 1, 0),
            Self::Iron => (0, 3, 0),
            Self::Oil => (0, 3, 0),
            Self::Rubber => (0, 1, 0),
            Self::Saltpeter => (0, 1, 0),
            Self::Uranium => (0, 2, 0),
            Self::Dyes => (0, 0, 3),
            Self::Furs => (1, 0, 3),
            Self::Gems => (0, 0, 4),
            Self::Incense => (0, 0, 3),
            Self::Ivory => (1, 1, 4),
            Self::Silk => (0, 0, 3),
            Self::Spice => (1, 0, 4),
            Self::Wine => (0, 0, 4),
            Self::Cattle => (2, 1, 0),
            Self::Fish => (2, 0, 0),
            Self::Gold => (0, 0, 6),
            Self::Game => (2, 1, 0),
            Self::Oasis => (3, 0, 0),
            Self::Sugar => (2, 0, 1),
            Self::Tobacco => (0, 0, 3),
            Self::TropicalFruit => (3, 0, 1),
            Self::Whales => (1, 1, 0),
            Self::Wheat => (2, 0, 0),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Tile {
    pub(crate) terrain: Terrain,
    pub(crate) altitude: i32,
    pub(crate) hut: bool,
    pub(crate) river: bool,
    pub(crate) resource: Option<Resource>,
    pub(crate) road: bool,
    pub(crate) railroad: bool,
    pub(crate) irrigation: bool,
    pub(crate) farmland: bool,
    pub(crate) pollution: bool,
    pub(crate) fallout: bool,
    pub(crate) base: Option<crate::sim::Base>,
    pub(crate) mine: bool,
    pub(crate) oil_well: bool,
    pub(crate) oil_platform: bool,
    pub(crate) owner: Option<usize>,
    pub(crate) border_city: Option<u32>,
    pub(crate) last_seen_owner: Option<usize>,
    pub(crate) seen: bool,
    pub(crate) visible: bool,
    pub(crate) stealth_visible: bool,
    #[serde(with = "crate::sim::save::known_by")]
    pub(crate) known_by: [bool; PLAYER_SLOTS],
}

impl Tile {
    pub(crate) fn active_resource(&self) -> Option<Resource> {
        self.resource
            .filter(|resource| self.terrain.resource_options().contains(resource))
    }

    pub(in crate::sim) fn irrigation_food(&self) -> i32 {
        let bonus = self.terrain.irrigation_bonus();
        if self.terrain == Terrain::Desert && self.river && self.resource != Some(Resource::Oasis) {
            bonus * 2
        } else {
            bonus
        }
    }

    pub(crate) fn yields(&self) -> (i32, i32, i32) {
        let (mut food, mut shields, mut trade) = self.terrain.base_yields();
        if let Some(resource) = self.active_resource() {
            let (extra_food, extra_shields, extra_trade) = resource.bonus();
            food += extra_food;
            shields += extra_shields;
            trade += extra_trade;
        }
        trade += i32::from(self.river);
        if self.irrigation {
            food += self.irrigation_food();
        }
        if self.mine {
            shields += self.terrain.mine_bonus() + i32::from(self.oil_well);
        }
        if self.road {
            trade += self.terrain.road_trade_bonus();
        }
        if self.railroad {
            shields += shields / 2;
        }
        (food, shields, trade)
    }
}

#[cfg(test)]
mod civ3_terrain_tests {
    use super::*;

    #[test]
    fn water_yields_and_resource_habitats_match_the_civ3_manual() {
        assert_eq!(Terrain::Coast.base_yields(), (1, 0, 2));
        assert_eq!(Terrain::Sea.base_yields(), (1, 0, 1));
        assert_eq!(Terrain::Ocean.base_yields(), (1, 0, 0));
        assert_eq!(Terrain::Coast.resource_options(), &[Resource::Fish]);
        for water in [Terrain::Sea, Terrain::Ocean] {
            assert_eq!(
                water.resource_options(),
                &[Resource::Fish, Resource::Whales]
            );
        }
    }

    #[test]
    fn civ3_special_land_terrains_keep_their_manual_yields() {
        assert_eq!(Terrain::FloodPlains.base_yields(), (3, 0, 0));
        assert_eq!(Terrain::GrasslandShield.base_yields(), (2, 1, 0));
        assert_eq!(Terrain::GrasslandShield.irrigation_bonus(), 1);
        assert_eq!(Terrain::GrasslandShield.mine_bonus(), 1);
        assert_eq!(Terrain::GrasslandShield.road_trade_bonus(), 1);
        assert!(Terrain::GrasslandShield.irrigation_time() > 0);
        assert!(Terrain::GrasslandShield.mine_time() > 0);
        assert_eq!(Terrain::FloodPlains.irrigation_bonus(), 1);
        assert!(Terrain::FloodPlains.irrigation_time() > 0);
        assert_eq!(Terrain::Marsh.name(), "Marsh");
    }
}
