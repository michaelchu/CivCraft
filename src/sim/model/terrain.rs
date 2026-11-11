//! Campaign tile state and its simulation-specific helpers.

use crate::rules::Tech;
use crate::sim::PLAYER_SLOTS;
use crate::world::{Resource, Terrain, TerrainBase};

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
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Tile {
    pub(crate) terrain: Terrain,
    /// Underlying Civ III ground for transition art. Feature/relief terrains
    /// use this field because the legacy `terrain` value does not retain it.
    #[serde(default)]
    pub(crate) base_terrain: TerrainBase,
    pub(crate) altitude: i32,
    pub(crate) hut: bool,
    pub(crate) river: bool,
    /// Reciprocal Civ III direction bits for river edges (N, NE, E, SE, S,
    /// SW, W, NW). `river` remains the legacy gameplay marker during migration.
    #[serde(default)]
    pub(crate) river_edges: u8,
    pub(crate) resource: Option<Resource>,
    pub(crate) road: bool,
    pub(crate) railroad: bool,
    pub(crate) irrigation: bool,
    pub(crate) farmland: bool,
    pub(crate) pollution: bool,
    pub(crate) fallout: bool,
    /// Bombardment damage that reduces this tile's food, shield, and trade
    /// output until a Worker clears it.
    #[serde(default)]
    pub(crate) craters: bool,
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
        let (mut food, mut shields, mut trade) = self
            .terrain
            .yields_with_resource(self.active_resource(), self.river);
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
