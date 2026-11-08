//! Terrain sprite selection, blending, and tile surface rendering.

use crate::rules::Tech;
use crate::sim::PLAYER_SLOTS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Terrain {
    Ocean,
    DeepOcean,
    Lake,
    Glacier,
    Jungle,
    Desert,
    Forest,
    Grassland,
    Hills,
    Mountains,
    Plains,
    Swamp,
    Tundra,
}

impl Terrain {
    pub(crate) fn resource_options(self) -> &'static [Resource] {
        use Resource::*;
        match self {
            Self::Lake => &[Fish],
            Self::Ocean => &[Fish, Whales],
            Self::DeepOcean => &[],
            Self::Glacier => &[Oil, Uranium, Ivory],
            Self::Desert => &[Oasis, Oil, Incense, Saltpeter],
            Self::Forest => &[Furs, Silk, Horses, Coal, Rubber],
            Self::Grassland => &[Cattle, Horses, Wine, Tobacco],
            Self::Hills => &[Coal, Wine, Iron, Aluminum, Saltpeter],
            Self::Jungle => &[Gems, TropicalFruit, Dyes, Rubber, Sugar],
            Self::Mountains => &[Gold, Iron, Aluminum, Uranium],
            Self::Plains => &[Cattle, Wheat, Horses, Iron, Oil, Ivory],
            Self::Swamp => &[Spice, Oil, Sugar],
            Self::Tundra => &[Game, Furs, Oil],
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::DeepOcean => "Deep Ocean",
            Self::Lake => "Lake",
            Self::Glacier => "Glacier",
            Self::Jungle => "Jungle",
            Self::Desert => "Desert",
            Self::Forest => "Forest",
            Self::Grassland => "Grassland",
            Self::Hills => "Hills",
            Self::Mountains => "Mountains",
            Self::Plains => "Plains",
            Self::Swamp => "Swamp",
            Self::Tundra => "Tundra",
        }
    }

    pub(crate) fn is_water(self) -> bool {
        matches!(self, Self::Ocean | Self::DeepOcean | Self::Lake)
    }

    pub(crate) fn movement_cost(self) -> i32 {
        match self {
            Self::Forest | Self::Hills | Self::Swamp | Self::Jungle | Self::Glacier => 2,
            Self::Mountains => 3,
            _ => 1,
        }
    }

    pub(crate) fn defense_bonus(self) -> i32 {
        match self {
            Self::Forest | Self::Swamp | Self::Jungle => 25,
            Self::Hills => 50,
            Self::Mountains => 100,
            _ => 0,
        }
    }

    pub(crate) fn base_yields(self) -> (i32, i32, i32) {
        match self {
            Self::Ocean | Self::DeepOcean => (1, 0, 2),
            Self::Lake => (2, 0, 2),
            Self::Glacier => (0, 0, 0),
            Self::Jungle => (1, 1, 0),
            Self::Desert => (0, 1, 0),
            Self::Forest => (1, 2, 0),
            Self::Grassland => (2, 0, 0),
            Self::Hills => (1, 1, 0),
            Self::Mountains => (0, 1, 0),
            Self::Plains => (1, 1, 0),
            Self::Swamp => (1, 0, 0),
            Self::Tundra => (1, 0, 0),
        }
    }

    pub(crate) fn irrigation_bonus(self) -> i32 {
        match self {
            Self::Desert | Self::Grassland | Self::Hills | Self::Plains | Self::Swamp => 1,
            _ => 0,
        }
    }

    pub(crate) fn mine_bonus(self) -> i32 {
        match self {
            Self::Desert | Self::Glacier | Self::Tundra => 1,
            Self::Hills | Self::Mountains => 2,
            _ => 0,
        }
    }

    pub(crate) fn road_trade_bonus(self) -> i32 {
        match self {
            Self::Desert | Self::Grassland | Self::Plains | Self::Tundra => 1,
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
            Self::Desert | Self::Grassland | Self::Plains | Self::Swamp => 5,
            Self::Hills => 10,
            _ => 0,
        }
    }

    pub(crate) fn mine_time(self) -> i32 {
        match self {
            Self::Desert | Self::Tundra => 5,
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
            Self::Aluminum => Some(Tech::FLIGHT),
            Self::Coal => Some(Tech::STEAM_ENGINE),
            Self::Horses => Some(Tech::HORSEBACK_RIDING),
            Self::Iron => Some(Tech::IRON_WORKING),
            Self::Oil => Some(Tech::REFINING),
            Self::Rubber => Some(Tech::INDUSTRIALIZATION),
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
