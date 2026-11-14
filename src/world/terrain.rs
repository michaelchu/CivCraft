//! Shared terrain and resource values used by generation and campaign state.

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

/// Base terrain used by Civ III's transition sheets. This is separate from
/// `Terrain`, which currently also carries feature and landform state for the
/// legacy simulation model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TerrainBase {
    #[default]
    Grassland,
    Plains,
    Desert,
    Tundra,
    Coast,
    Sea,
    Ocean,
    Ice,
}

impl TerrainBase {
    pub(crate) fn is_feature_terrain(terrain: Terrain) -> bool {
        matches!(
            terrain,
            Terrain::Forest
                | Terrain::Jungle
                | Terrain::Hills
                | Terrain::Mountains
                | Terrain::Marsh
                | Terrain::Volcano
        )
    }
}

impl Terrain {
    /// Fresh-tile yield before worker improvements, shared by generation's
    /// start-site scoring and campaign tile calculations.
    pub(crate) fn yields_with_resource(
        self,
        resource: Option<Resource>,
        river: bool,
    ) -> (i32, i32, i32) {
        let (mut food, mut shields, mut trade) = self.base_yields();
        if let Some(resource) = resource.filter(|r| self.resource_options().contains(r)) {
            let (extra_food, extra_shields, extra_trade) = resource.bonus();
            food += extra_food;
            shields += extra_shields;
            trade += extra_trade;
        }
        trade += i32::from(river);
        (food, shields, trade)
    }

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
