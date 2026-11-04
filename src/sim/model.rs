use super::{
    BASE_TECH_COST, MIN_TECH_COST, PLAYER_COUNT, SCIENCEBOX_PERCENT, TECH_COST_PERCENT, percent_of,
};
use crate::rules::{Building, Tech};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
            Self::Desert | Self::Tundra => 1,
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
            Self::Hills | Self::Mountains => 10,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone)]
pub(crate) struct Tile {
    pub(crate) terrain: Terrain,
    pub(crate) altitude: i32,
    pub(crate) hut: bool,
    pub(crate) river: bool,
    pub(crate) resource: Option<Resource>,
    pub(crate) road: bool,
    pub(crate) railroad: bool,
    pub(crate) irrigation: bool,
    pub(crate) mine: bool,
    pub(crate) owner: Option<usize>,
    pub(crate) seen: bool,
    pub(crate) visible: bool,
    pub(crate) known_by: [bool; PLAYER_COUNT],
}

impl Tile {
    pub(crate) fn yields(&self) -> (i32, i32, i32) {
        let (mut food, mut shields, mut trade) = self.terrain.base_yields();
        if let Some(resource) = self.resource {
            let (extra_food, extra_shields, extra_trade) = resource.bonus();
            food += extra_food;
            shields += extra_shields;
            trade += extra_trade;
        }
        trade += i32::from(self.river);
        if self.irrigation {
            food += self.terrain.irrigation_bonus();
        }
        if self.mine {
            shields += self.terrain.mine_bonus();
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitKind {
    Settlers,
    Worker,
    Diplomat,
    Explorer,
    Warrior,
    Phalanx,
    Archer,
    Legion,
    Horseman,
    Chariot,
    Catapult,
    Musketeers,
    Knights,
    Partisan,
    Cannon,
    Caravel,
    Galleon,
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
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Settlers => "Settlers",
            Self::Worker => "Workers",
            Self::Diplomat => "Diplomat",
            Self::Explorer => "Explorer",
            Self::Warrior => "Warriors",
            Self::Phalanx => "Phalanx",
            Self::Archer => "Archers",
            Self::Legion => "Legion",
            Self::Horseman => "Horsemen",
            Self::Chariot => "Chariot",
            Self::Catapult => "Catapult",
            Self::Musketeers => "Musketeers",
            Self::Knights => "Knights",
            Self::Galleon => "Galleon",
            Self::Caravel => "Caravel",
            Self::Cannon => "Cannon",
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

    pub(crate) fn transport_capacity(self) -> usize {
        match self {
            Self::Caravel => 3,
            Self::Galleon => 4,
            _ => 0,
        }
    }

    pub(crate) fn can_carry(self, kind: Self) -> bool {
        kind.transport_capacity() == 0
            && match self {
                Self::Caravel => !matches!(kind, Self::Cannon | Self::Catapult | Self::Chariot),
                Self::Galleon => true,
                _ => false,
            }
    }

    pub(crate) fn is_military(self) -> bool {
        self.stats().attack > 0
    }

    pub(crate) fn can_fortify(self) -> bool {
        // Every opening unit uses a land class with CanFortify, but these
        // three unit types carry the ruleset's overriding Cant_Fortify flag.
        self.transport_capacity() == 0
            && !matches!(self, Self::Settlers | Self::Worker | Self::Explorer)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitState {
    Idle,
    Fortifying,
    Fortified,
    Sentry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkKind {
    Road,
    Railroad,
    Irrigation,
    Mine,
}

impl WorkKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::Railroad => "railroad",
            Self::Irrigation => "irrigation",
            Self::Mine => "mine",
        }
    }
}

#[derive(Clone)]
pub(crate) struct Unit {
    pub(crate) id: u32,
    pub(crate) owner: usize,
    pub(crate) kind: UnitKind,
    pub(crate) transport: Option<u32>,
    pub(crate) home_city: Option<u32>,
    pub(super) home_order: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) hp: i32,
    pub(crate) moves_left: i32,
    pub(crate) state: UnitState,
    pub(crate) moved: bool,
    pub(crate) automated: bool,
    pub(crate) work_kind: Option<WorkKind>,
    pub(crate) work_remaining: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone)]
pub(crate) struct City {
    pub(crate) id: u32,
    pub(crate) owner: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) name: String,
    pub(crate) size: i32,
    pub(crate) specialists: [i32; 3],
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) production: Production,
    pub(super) production_before_switch: Option<(Production, i32)>,
    pub(crate) buildings: Vec<Building>,
}

pub(crate) struct Player {
    pub(crate) name: &'static str,
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
    pub(crate) ai: bool,
}
