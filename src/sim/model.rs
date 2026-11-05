use super::{
    BASE_TECH_COST, MIN_TECH_COST, PLAYER_SLOTS, SCIENCEBOX_PERCENT, TECH_COST_PERCENT, percent_of,
};
use crate::rules::{Building, Tech};

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

    pub(crate) fn prefers_mine(self) -> bool {
        matches!(self, Self::Hills | Self::Mountains | Self::Tundra)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Resource {
    Gold,
    Iron,
    Game,
    Furs,
    Coal,
    Fish,
    Fruit,
    Gems,
    Buffalo,
    Wheat,
    Oasis,
    Peat,
    Pheasant,
    Resources,
    Ivory,
    Silk,
    Spice,
    Whales,
    Wine,
    Oil,
}

impl Resource {
    pub(crate) fn bonus(self) -> (i32, i32, i32) {
        match self {
            Self::Gold => (0, 0, 6),
            Self::Iron => (0, 3, 0),
            Self::Game => (2, 1, 0),
            Self::Furs => (1, 0, 3),
            Self::Coal => (0, 2, 0),
            Self::Fish => (2, 0, 0),
            Self::Fruit => (3, 0, 1),
            Self::Gems => (0, 0, 4),
            Self::Buffalo => (0, 2, 0),
            Self::Wheat => (2, 0, 0),
            Self::Oasis => (3, 0, 0),
            Self::Peat => (0, 4, 0),
            Self::Pheasant => (2, 0, 0),
            Self::Resources => (0, 1, 0),
            Self::Ivory => (1, 1, 4),
            Self::Silk => (0, 0, 3),
            Self::Spice => (1, 0, 4),
            Self::Whales => (1, 1, 0),
            Self::Wine => (0, 0, 4),
            Self::Oil => (0, 3, 0),
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
    pub(crate) base: Option<super::Base>,
    pub(crate) mine: bool,
    pub(crate) oil_well: bool,
    pub(crate) oil_platform: bool,
    pub(crate) owner: Option<usize>,
    pub(crate) border_city: Option<u32>,
    pub(crate) last_seen_owner: Option<usize>,
    pub(crate) seen: bool,
    pub(crate) visible: bool,
    pub(crate) stealth_visible: bool,
    #[serde(with = "super::save::known_by")]
    pub(crate) known_by: [bool; PLAYER_SLOTS],
}

impl Tile {
    pub(crate) fn active_resource(&self) -> Option<Resource> {
        use Resource::*;
        self.resource.filter(|resource| match self.terrain {
            Terrain::Lake => matches!(resource, Fish),
            Terrain::Ocean => matches!(resource, Fish | Whales),
            Terrain::DeepOcean => false,
            Terrain::Glacier => matches!(resource, Ivory | Oil),
            Terrain::Desert => matches!(resource, Oasis | Oil),
            Terrain::Forest => matches!(resource, Pheasant | Silk),
            Terrain::Grassland => matches!(resource, Resources),
            Terrain::Hills => matches!(resource, Coal | Wine),
            Terrain::Jungle => matches!(resource, Gems | Fruit),
            Terrain::Mountains => matches!(resource, Gold | Iron),
            Terrain::Plains => matches!(resource, Buffalo | Wheat),
            Terrain::Swamp => matches!(resource, Peat | Spice),
            Terrain::Tundra => matches!(resource, Game | Furs),
        })
    }

    pub(super) fn irrigation_food(&self) -> i32 {
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
}

#[derive(Clone, Copy)]
pub(crate) struct UnitStats {
    pub(crate) cost: i32,
    pub(crate) pop_cost: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
    pub(crate) hp: i32,
    pub(crate) firepower: i32,
    pub(crate) moves: i32,
    pub(crate) vision_sq: i32,
    pub(crate) ignores_terrain: bool,
    pub(crate) can_work: bool,
    pub(crate) can_found: bool,
}

impl UnitKind {
    pub(super) const AI_MILITARY: [Self; 7] = [
        Self::Catapult,
        Self::Chariot,
        Self::Legion,
        Self::Archer,
        Self::Horseman,
        Self::Phalanx,
        Self::Warrior,
    ];

    pub(super) fn required_tech(self) -> Option<Tech> {
        match self {
            Self::Engineer => Some(Tech::EXPLOSIVES),
            Self::Migrants => Some(Tech::POTTERY),
            Self::Phalanx => Some(Tech::BRONZE_WORKING),
            Self::Archer => Some(Tech::WARRIOR_CODE),
            Self::Legion => Some(Tech::IRON_WORKING),
            Self::Horseman => Some(Tech::HORSEBACK_RIDING),
            Self::Chariot => Some(Tech::THE_WHEEL),
            Self::Catapult => Some(Tech::MATHEMATICS),
            Self::Musketeers => Some(Tech::GUNPOWDER),
            Self::Marines => Some(Tech::AMPHIBIOUS_WARFARE),
            Self::Paratroopers => Some(Tech::COMBINED_ARMS),
            Self::Knights => Some(Tech::CHIVALRY),
            Self::Partisan => Some(Tech::GUERILLA_WARFARE),
            Self::Cannon => Some(Tech::METALLURGY),
            Self::Bomber => Some(Tech::ADVANCED_FLIGHT),
            Self::Fighter => Some(Tech::FLIGHT),
            Self::Helicopter => Some(Tech::COMBINED_ARMS),
            Self::StealthFighter | Self::StealthBomber => Some(Tech::STEALTH),
            Self::Diplomat => Some(Tech::ALPHABET),
            Self::Spy => Some(Tech::ESPIONAGE),
            Self::Trireme => Some(Tech::MAP_MAKING),
            Self::Caravel => Some(Tech::MAGNETISM),
            Self::Galleon => Some(Tech::NAVIGATION),
            Self::Transport => Some(Tech::ENGINEERING),
            Self::Carrier => Some(Tech::ADVANCED_FLIGHT),
            Self::Frigate => Some(Tech::NAVIGATION),
            Self::Ironclad => Some(Tech::STEAM_ENGINE),
            Self::Destroyer => Some(Tech::ENGINEERING),
            Self::Cruiser => Some(Tech::STEEL),
            Self::Battleship => Some(Tech::MASS_PRODUCTION),
            Self::Submarine => Some(Tech::COMBUSTION),
            Self::AegisCruiser | Self::CruiseMissile => Some(Tech::ROCKETRY),
            Self::Nuclear => Some(Tech::NUCLEAR_FISSION),
            Self::Caravan => Some(Tech::TRADE),
            Self::Freight => Some(Tech::THE_CORPORATION),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
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
        }
    }

    pub(crate) fn stats(self) -> UnitStats {
        match self {
            Self::Settlers => UnitStats {
                cost: 30,
                pop_cost: 2,
                attack: 0,
                defense: 1,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: true,
                can_found: true,
            },
            Self::Migrants => UnitStats {
                cost: 10,
                pop_cost: 1,
                attack: 0,
                defense: 1,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: true,
                can_found: false,
            },
            Self::Worker => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: true,
                can_found: false,
            },
            Self::Engineer => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 2,
                hp: 20,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: true,
                can_found: false,
            },
            Self::Diplomat => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 0,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Spy => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 0,
                hp: 10,
                firepower: 1,
                moves: 3,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Explorer => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 6,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Warrior => UnitStats {
                cost: 10,
                pop_cost: 0,
                attack: 1,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Phalanx => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 1,
                defense: 2,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Archer => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 3,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Legion => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 4,
                defense: 2,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Chariot => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 4,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Catapult => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 6,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Paratroopers => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 6,
                defense: 4,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Marines => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 8,
                defense: 5,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Musketeers => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 3,
                defense: 3,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Knights => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 4,
                defense: 3,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Partisan => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 4,
                defense: 5,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 6,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Cannon => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 8,
                defense: 1,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Trireme => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 3,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Caravel => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 0,
                defense: 2,
                hp: 10,
                firepower: 1,
                moves: 3,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Galleon => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 0,
                defense: 2,
                hp: 20,
                firepower: 1,
                moves: 4,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::StealthFighter => UnitStats {
                cost: 80,
                pop_cost: 0,
                attack: 8,
                defense: 8,
                hp: 20,
                firepower: 1,
                moves: 14,
                vision_sq: 8,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::StealthBomber => UnitStats {
                cost: 120,
                pop_cost: 0,
                attack: 9,
                defense: 5,
                hp: 30,
                firepower: 1,
                moves: 12,
                vision_sq: 8,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Helicopter => UnitStats {
                cost: 70,
                pop_cost: 0,
                attack: 5,
                defense: 3,
                hp: 30,
                firepower: 1,
                moves: 6,
                vision_sq: 8,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Fighter => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 4,
                defense: 4,
                hp: 20,
                firepower: 1,
                moves: 10,
                vision_sq: 8,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Bomber => UnitStats {
                cost: 100,
                pop_cost: 0,
                attack: 6,
                defense: 2,
                hp: 30,
                firepower: 1,
                moves: 8,
                vision_sq: 8,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Caravan | Self::Freight => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: if self == Self::Caravan { 1 } else { 2 },
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::BarbarianLeader => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 0,
                defense: 0,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Nuclear => UnitStats {
                cost: 160,
                pop_cost: 0,
                attack: 99,
                defense: 0,
                hp: 10,
                firepower: 1,
                moves: 0,
                vision_sq: 2,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::CruiseMissile => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 18,
                defense: 0,
                hp: 10,
                firepower: 1,
                moves: 16,
                vision_sq: 2,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Frigate => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 3,
                defense: 2,
                hp: 20,
                firepower: 1,
                moves: 4,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Ironclad => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 4,
                defense: 3,
                hp: 20,
                firepower: 1,
                moves: 4,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Destroyer => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 4,
                defense: 4,
                hp: 30,
                firepower: 1,
                moves: 6,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Cruiser => UnitStats {
                cost: 80,
                pop_cost: 0,
                attack: 6,
                defense: 6,
                hp: 30,
                firepower: 1,
                moves: 5,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Battleship => UnitStats {
                cost: 160,
                pop_cost: 0,
                attack: 12,
                defense: 12,
                hp: 30,
                firepower: 1,
                moves: 4,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Submarine => UnitStats {
                cost: 70,
                pop_cost: 0,
                attack: 12,
                defense: 5,
                hp: 20,
                firepower: 1,
                moves: 5,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::AegisCruiser => UnitStats {
                cost: 100,
                pop_cost: 0,
                attack: 8,
                defense: 8,
                hp: 30,
                firepower: 1,
                moves: 5,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Carrier => UnitStats {
                cost: 150,
                pop_cost: 0,
                attack: 0,
                defense: 9,
                hp: 40,
                firepower: 1,
                moves: 5,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Transport => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 0,
                defense: 3,
                hp: 30,
                firepower: 1,
                moves: 5,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Horseman => UnitStats {
                cost: 20,
                pop_cost: 0,
                attack: 2,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
        }
    }

    pub(super) fn is_naval(self) -> bool {
        matches!(
            self,
            Self::Trireme
                | Self::Caravel
                | Self::Galleon
                | Self::Transport
                | Self::Carrier
                | Self::Frigate
                | Self::Ironclad
                | Self::Destroyer
                | Self::Cruiser
                | Self::Battleship
                | Self::Submarine
                | Self::AegisCruiser
        )
    }

    pub(super) fn earns_naval_veterancy(self) -> bool {
        self.is_naval() && (self.is_military() || self == Self::Carrier)
    }

    pub(crate) fn transport_capacity(self) -> usize {
        match self {
            Self::Helicopter => 1,
            Self::Trireme => 2,
            Self::Caravel => 3,
            Self::Galleon => 4,
            Self::Transport | Self::Carrier | Self::Submarine => 8,
            _ => 0,
        }
    }

    pub(crate) fn is_diplomatic(self) -> bool {
        matches!(self, Self::Diplomat | Self::Spy)
    }

    pub(crate) fn can_clean(self) -> bool {
        self.stats().can_work || self == Self::Transport
    }

    pub(super) fn obsolete_by(self) -> &'static [Self] {
        match self {
            Self::Warrior | Self::Archer | Self::Legion | Self::Phalanx => &[Self::Musketeers],
            Self::Horseman => &[Self::Knights],
            Self::Catapult => &[Self::Cannon],
            Self::Worker => &[Self::Engineer],
            Self::Diplomat => &[Self::Spy],
            Self::Frigate => &[Self::Destroyer, Self::Ironclad],
            Self::Ironclad => &[Self::Destroyer],
            Self::Cruiser => &[Self::AegisCruiser],
            Self::Trireme => &[Self::Transport],
            Self::Caravel => &[Self::Transport, Self::Galleon],
            Self::Galleon => &[Self::Transport],
            Self::Fighter => &[Self::StealthFighter],
            Self::Bomber => &[Self::StealthBomber],
            Self::Caravan => &[Self::Freight],
            _ => &[],
        }
    }

    pub(crate) fn fuel_capacity(self) -> u8 {
        match self {
            Self::Bomber | Self::StealthBomber => 3,
            Self::Fighter | Self::StealthFighter => 2,
            Self::CruiseMissile | Self::Nuclear => 1,
            _ => 0,
        }
    }

    pub(super) fn is_missile(self) -> bool {
        matches!(self, Self::CruiseMissile | Self::Nuclear)
    }

    pub(crate) fn is_aircraft(self) -> bool {
        self.fuel_capacity() > 0 || self == Self::Helicopter
    }

    pub(super) fn can_attack_aircraft(self) -> bool {
        matches!(
            self,
            Self::Fighter | Self::StealthFighter | Self::CruiseMissile
        )
    }

    pub(super) fn bombard_rate(self) -> i32 {
        match self {
            Self::Helicopter => 2,
            Self::Bomber => 3,
            Self::StealthBomber => 4,
            _ => 0,
        }
    }

    pub(super) fn uses_stealth_vision(self) -> bool {
        self.is_diplomatic()
            || matches!(
                self,
                Self::StealthFighter | Self::StealthBomber | Self::Submarine
            )
    }

    pub(super) fn can_occupy_city(self) -> bool {
        self.is_military() && !self.is_naval() && (!self.is_aircraft() || self == Self::Helicopter)
    }

    pub(crate) fn can_carry(self, kind: Self) -> bool {
        if self == Self::Helicopter {
            return kind.is_military() && !kind.is_naval() && !kind.is_aircraft();
        }
        if self == Self::Submarine {
            return kind.is_missile();
        }
        if self == Self::Carrier {
            return kind.is_aircraft()
                || matches!(
                    kind,
                    Self::Warrior
                        | Self::Phalanx
                        | Self::Archer
                        | Self::Legion
                        | Self::Horseman
                        | Self::Musketeers
                        | Self::Marines
                        | Self::Paratroopers
                        | Self::Knights
                        | Self::Partisan
                );
        }
        !kind.is_naval()
            && !kind.is_aircraft()
            && match self {
                Self::Trireme | Self::Caravel => {
                    !matches!(kind, Self::Cannon | Self::Catapult | Self::Chariot)
                }
                Self::Galleon | Self::Transport => true,
                _ => false,
            }
    }

    pub(super) fn can_capture_units(self) -> bool {
        matches!(
            self,
            Self::Warrior
                | Self::Phalanx
                | Self::Archer
                | Self::Legion
                | Self::Musketeers
                | Self::Marines
                | Self::Paratroopers
                | Self::Partisan
                | Self::Horseman
                | Self::Chariot
                | Self::Knights
        )
    }

    pub(super) fn is_capturable(self) -> bool {
        matches!(self, Self::Caravan | Self::Freight | Self::Migrants)
    }

    pub(crate) fn is_military(self) -> bool {
        self.stats().attack > 0
    }

    pub(crate) fn can_fortify(self) -> bool {
        // Every opening unit uses a land class with CanFortify, but these
        // three unit types carry the ruleset's overriding Cant_Fortify flag.
        !self.is_naval()
            && !self.is_aircraft()
            && !matches!(
                self,
                Self::Settlers | Self::Migrants | Self::Worker | Self::Engineer | Self::Explorer
            )
    }
}

impl Tech {
    pub(crate) fn cost(self) -> i32 {
        let requirements = self.data().map_or(1, |advance| advance.requirement_count);
        research_cost(requirements)
    }
}

pub(super) fn research_cost(requirements: i32) -> i32 {
    let base = (BASE_TECH_COST * requirements).max(MIN_TECH_COST);
    percent_of(percent_of(base, TECH_COST_PERCENT), SCIENCEBOX_PERCENT).max(1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UnitState {
    Idle,
    Fortifying,
    Fortified,
    Sentry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum WorkKind {
    Road,
    Railroad,
    Irrigation,
    Farmland,
    Cultivate,
    Plant,
    Transform,
    Pillage(super::PillageTarget),
    BuildBase(super::Base),
    Clean(super::Contamination),
    OilWell,
    OilPlatform,
    Mine,
}

impl WorkKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::Railroad => "railroad",
            Self::Irrigation => "irrigation",
            Self::Farmland => "farmland",
            Self::Cultivate => "cultivate",
            Self::Plant => "plant",
            Self::Transform => "transform",
            Self::Pillage(_) => "pillage",
            Self::BuildBase(base) => base.name(),
            Self::Clean(target) => target.name(),
            Self::OilWell => "oil well",
            Self::OilPlatform => "oil platform",
            Self::Mine => "mine",
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Unit {
    pub(crate) id: u32,
    pub(super) nationality: usize,
    pub(crate) owner: usize,
    pub(crate) kind: UnitKind,
    pub(crate) transport: Option<u32>,
    pub(crate) home_city: Option<u32>,
    pub(super) home_order: u32,
    pub(crate) veteran: usize,
    pub(crate) fuel: u8,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) hp: i32,
    pub(crate) moves_left: i32,
    pub(crate) state: UnitState,
    pub(crate) moved: bool,
    pub(crate) automated: bool,
    pub(crate) work_kind: Option<WorkKind>,
    pub(crate) work_left: i32,
    pub(super) work_done: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Production {
    Unit(UnitKind),
    Building(Building),
}

impl Production {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Unit(kind) => kind.name(),
            Self::Building(building) => building.data().name,
        }
    }

    pub(crate) fn cost(self) -> i32 {
        match self {
            Self::Unit(kind) => kind.stats().cost,
            Self::Building(building) => building.data().cost,
        }
    }

    pub(super) fn class(self) -> u8 {
        match self {
            Self::Unit(_) => 0,
            Self::Building(building)
                if matches!(
                    building.data().genus,
                    crate::rules::Genus::SmallWonder | crate::rules::Genus::GreatWonder
                ) =>
            {
                1
            }
            Self::Building(_) => 2,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct City {
    pub(crate) id: u32,
    pub(crate) owner: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) name: String,
    pub(crate) size: i32,
    pub(crate) specialists: [i32; 3],
    pub(crate) worked_tiles: Vec<(i32, i32)>,
    pub(super) was_happy: bool,
    pub(super) last_plague: Option<i32>,
    pub(super) history: i32,
    pub(super) foreign_citizens: Vec<(usize, i32)>,
    pub(super) airlift_used: bool,
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) production: Production,
    pub(super) founded_turn: i32,
    pub(crate) bought_production: bool,
    pub(super) sold_building: bool,
    pub(crate) worklist: Vec<Production>,
    pub(super) technology_thefts: u32,
    pub(super) protected_shields: i32,
    pub(super) caravan_shields: i32,
    pub(super) production_before_switch: Option<(Production, i32)>,
    pub(crate) buildings: Vec<Building>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct Player {
    pub(crate) name: String,
    pub(crate) gold: i32,
    pub(crate) rates: [i32; 3],
    pub(crate) government: super::Government,
    pub(crate) revolution_target: Option<super::Government>,
    pub(crate) revolution_turn: Option<i32>,
    pub(crate) known: Vec<Tech>,
    pub(crate) researching: Tech,
    pub(crate) research_goal: Tech,
    pub(crate) future_techs: i32,
    pub(crate) research: i32,
    pub(super) last_science: i32,
    pub(crate) ai: bool,
}
