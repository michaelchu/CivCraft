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
    Pikemen,
    Riflemen,
    AlpineTroops,
    Fanatics,
    MechInf,
    Elephants,
    Crusaders,
    Dragoons,
    Cavalry,
    Armor,
    Artillery,
    Howitzer,
    Leader,
    Awacs,
    Storm,
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
    // Source ruleset ordering is independent of enum/save discriminants.
    pub(crate) const ALL_NATIVE: [Self; 57] = [
        Self::Settlers,
        Self::Migrants,
        Self::Worker,
        Self::Engineer,
        Self::Warrior,
        Self::Phalanx,
        Self::Archer,
        Self::Legion,
        Self::Pikemen,
        Self::Musketeers,
        Self::Riflemen,
        Self::AlpineTroops,
        Self::Partisan,
        Self::Fanatics,
        Self::Marines,
        Self::Paratroopers,
        Self::MechInf,
        Self::Horseman,
        Self::Chariot,
        Self::Elephants,
        Self::Crusaders,
        Self::Knights,
        Self::Dragoons,
        Self::Cavalry,
        Self::Armor,
        Self::Catapult,
        Self::Cannon,
        Self::Artillery,
        Self::Howitzer,
        Self::Fighter,
        Self::Bomber,
        Self::Helicopter,
        Self::StealthFighter,
        Self::StealthBomber,
        Self::Trireme,
        Self::Caravel,
        Self::Galleon,
        Self::Frigate,
        Self::Ironclad,
        Self::Destroyer,
        Self::Cruiser,
        Self::AegisCruiser,
        Self::Battleship,
        Self::Submarine,
        Self::Carrier,
        Self::Transport,
        Self::CruiseMissile,
        Self::Nuclear,
        Self::Diplomat,
        Self::Spy,
        Self::Caravan,
        Self::Freight,
        Self::Explorer,
        Self::Leader,
        Self::BarbarianLeader,
        Self::Awacs,
        Self::Storm,
    ];
    #[cfg(test)]
    pub(crate) const ALL: [Self; 57] = Self::ALL_NATIVE;
    #[cfg(test)]
    pub(crate) fn all() -> impl Iterator<Item = Self> {
        Self::ALL_NATIVE.into_iter()
    }
    pub(super) const SIMPLE_MILITARY: [Self; 32] = [
        Self::Warrior,
        Self::Phalanx,
        Self::Archer,
        Self::Legion,
        Self::Pikemen,
        Self::Musketeers,
        Self::Riflemen,
        Self::AlpineTroops,
        Self::Partisan,
        Self::Fanatics,
        Self::Marines,
        Self::Paratroopers,
        Self::MechInf,
        Self::Horseman,
        Self::Chariot,
        Self::Elephants,
        Self::Crusaders,
        Self::Knights,
        Self::Dragoons,
        Self::Cavalry,
        Self::Armor,
        Self::Catapult,
        Self::Cannon,
        Self::Artillery,
        Self::Howitzer,
        Self::Helicopter,
        Self::Frigate,
        Self::Ironclad,
        Self::Destroyer,
        Self::Cruiser,
        Self::AegisCruiser,
        Self::Battleship,
    ];

    pub(super) fn hp_loss_percent(self) -> i32 {
        // Civ2Civ3 unit classes Air and Helicopter lose 10%; Missile and
        // every supported land/sea class lose none.
        if self.is_aircraft() && !self.is_missile() {
            10
        } else {
            0
        }
    }

    pub(super) fn required_tech(self) -> Option<Tech> {
        self.metadata().tech
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Pikemen => "Pikemen",
            Self::Riflemen => "Riflemen",
            Self::AlpineTroops => "Alpine Troops",
            Self::Fanatics => "Fanatics",
            Self::MechInf => "Mech. Inf.",
            Self::Elephants => "Elephants",
            Self::Crusaders => "Crusaders",
            Self::Dragoons => "Dragoons",
            Self::Cavalry => "Cavalry",
            Self::Armor => "Armor",
            Self::Artillery => "Artillery",
            Self::Howitzer => "Howitzer",
            Self::Leader => "Leader",
            Self::Awacs => "AWACS",
            Self::Storm => "Storm",
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
            Self::Pikemen => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 2,
                defense: 3,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Riflemen => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 5,
                defense: 4,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::AlpineTroops => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 7,
                defense: 4,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: true,
                can_work: false,
                can_found: false,
            },
            Self::Fanatics => UnitStats {
                cost: 20,
                pop_cost: 1,
                attack: 5,
                defense: 5,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::MechInf => UnitStats {
                cost: 70,
                pop_cost: 0,
                attack: 6,
                defense: 6,
                hp: 30,
                firepower: 1,
                moves: 3,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Elephants => UnitStats {
                cost: 30,
                pop_cost: 0,
                attack: 3,
                defense: 2,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Crusaders => UnitStats {
                cost: 40,
                pop_cost: 0,
                attack: 5,
                defense: 1,
                hp: 10,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Dragoons => UnitStats {
                cost: 50,
                pop_cost: 0,
                attack: 5,
                defense: 2,
                hp: 20,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Cavalry => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 8,
                defense: 3,
                hp: 20,
                firepower: 1,
                moves: 2,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Armor => UnitStats {
                cost: 90,
                pop_cost: 0,
                attack: 10,
                defense: 5,
                hp: 30,
                firepower: 1,
                moves: 3,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Artillery => UnitStats {
                cost: 60,
                pop_cost: 0,
                attack: 10,
                defense: 2,
                hp: 20,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Howitzer => UnitStats {
                cost: 80,
                pop_cost: 0,
                attack: 12,
                defense: 2,
                hp: 30,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Leader => UnitStats {
                cost: 10,
                pop_cost: 0,
                attack: 0,
                defense: 2,
                hp: 20,
                firepower: 1,
                moves: 2,
                vision_sq: 8,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Awacs => UnitStats {
                cost: 140,
                pop_cost: 0,
                attack: 0,
                defense: 1,
                hp: 20,
                firepower: 1,
                moves: 16,
                vision_sq: 26,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
            Self::Storm => UnitStats {
                cost: 10,
                pop_cost: 0,
                attack: 0,
                defense: 9,
                hp: 10,
                firepower: 1,
                moves: 1,
                vision_sq: 2,
                ignores_terrain: false,
                can_work: false,
                can_found: false,
            },
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
                can_work: true,
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

    pub(crate) fn is_naval(self) -> bool {
        matches!(
            self.unit_class(),
            super::unit_catalog::UnitClass::Sea | super::unit_catalog::UnitClass::Trireme
        )
    }

    pub(super) fn earns_naval_veterancy(self) -> bool {
        self.is_naval() && (self.is_military() || self == Self::Carrier)
    }

    pub(crate) fn transport_capacity(self) -> usize {
        self.metadata().capacity
    }

    pub(crate) fn is_diplomatic(self) -> bool {
        matches!(self, Self::Diplomat | Self::Spy)
    }

    pub(crate) fn can_clean(self) -> bool {
        self.is_worker()
    }

    pub(super) fn obsolete_by(self) -> &'static [Self] {
        self.metadata().obsolete
    }

    pub(crate) fn fuel_capacity(self) -> u8 {
        self.metadata().fuel
    }

    pub(super) fn is_missile(self) -> bool {
        matches!(self, Self::CruiseMissile | Self::Nuclear)
    }

    pub(crate) fn is_aircraft(self) -> bool {
        self.fuel_capacity() > 0 || self == Self::Helicopter
    }

    pub(super) fn can_attack_aircraft(self) -> bool {
        self.metadata().attacks_air
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
        self.metadata().stealth
    }

    pub(super) fn can_occupy_city(self) -> bool {
        !self.is_civilian() && self.unit_class().can_occupy_city()
    }

    pub(crate) fn can_carry(self, kind: Self) -> bool {
        self.metadata().cargo.contains(&kind.unit_class())
    }

    pub(super) fn can_capture_units(self) -> bool {
        self.has_flag(super::unit_catalog::UnitFlag::Capturer)
    }

    pub(super) fn is_capturable(self) -> bool {
        self.has_flag(super::unit_catalog::UnitFlag::Capturable)
    }

    pub(crate) fn is_military(self) -> bool {
        self.stats().attack > 0
    }

    pub(crate) fn is_civilian(self) -> bool {
        self.has_flag(super::unit_catalog::UnitFlag::NonMil)
    }

    pub(crate) fn can_fortify(self) -> bool {
        self.unit_class().can_fortify()
            && !self.has_flag(super::unit_catalog::UnitFlag::CantFortify)
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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

impl Player {
    pub(crate) fn leader_name(&self) -> &'static str {
        crate::rules::civilizations::CIVILIZATIONS
            .iter()
            .find(|civ| self.name == civ.name || self.name == civ.legacy_name)
            .map(|civ| civ.leader)
            .unwrap_or(if self.name == "Barbarians" {
                "Chieftain"
            } else {
                "Rebel Council"
            })
    }
}

#[cfg(test)]
mod native_type_order_tests {
    use super::*;
    use crate::sim::{HUMAN, Simulation};

    #[test]
    fn simple_military_cache_excludes_nonmil_fuelled_large_and_native_only_sea_types() {
        for kind in UnitKind::SIMPLE_MILITARY {
            assert!(!kind.is_civilian());
            assert_eq!(kind.fuel_capacity(), 0);
            assert!(kind.transport_capacity() < 8);
        }
        for kind in [
            UnitKind::Trireme,
            UnitKind::Caravel,
            UnitKind::Galleon,
            UnitKind::Transport,
            UnitKind::Carrier,
            UnitKind::Submarine,
            UnitKind::Fighter,
            UnitKind::Bomber,
            UnitKind::CruiseMissile,
            UnitKind::Nuclear,
            UnitKind::Diplomat,
        ] {
            assert!(!UnitKind::SIMPLE_MILITARY.contains(&kind), "{kind:?}");
        }
        assert!(UnitKind::SIMPLE_MILITARY.contains(&UnitKind::Helicopter));
        let position = |kind| {
            UnitKind::SIMPLE_MILITARY
                .iter()
                .position(|&candidate| candidate == kind)
                .unwrap()
        };
        assert!(position(UnitKind::Partisan) < position(UnitKind::Marines));
        assert!(position(UnitKind::Paratroopers) < position(UnitKind::Horseman));
        assert!(position(UnitKind::Knights) < position(UnitKind::Catapult));
        assert!(position(UnitKind::AegisCruiser) < position(UnitKind::Battleship));
    }

    #[test]
    fn tactical_equal_defense_and_cost_uses_later_native_type() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.players[1].known = vec![Tech::GUERILLA_WARFARE, Tech::AMPHIBIOUS_WARFARE];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
        }
        game.create_city(1, 10, 20);
        let attacker = game.spawn_unit(HUMAN, UnitKind::Cannon, 10, 20);
        let attacker = game.units.iter().find(|unit| unit.id == attacker).unwrap();
        let defender = game.defender_versus(&game.cities[0], attacker).unwrap();
        assert_eq!(
            UnitKind::Partisan.stats().defense,
            UnitKind::Marines.stats().defense
        );
        assert_eq!(UnitKind::Partisan.stats().hp, UnitKind::Marines.stats().hp);
        assert_eq!(
            UnitKind::Partisan.stats().cost,
            UnitKind::Marines.stats().cost
        );
        assert_eq!(defender.kind, UnitKind::Marines);
    }

    #[test]
    fn ordinary_offense_forecasts_a_counter_defender_even_for_an_empty_city() {
        use crate::sim::{Government, offset_native_by_map};
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.players[1].known = vec![Tech::WARRIOR_CODE, Tech::IRON_WORKING];
        game.players[1].government = Government::Democracy;
        game.players[1].rates = [0, 0, 100];
        game.sign_pact(1, HUMAN, crate::sim::Pact::War);
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Hills;
            tile.resource = Some(Resource::Coal);
            tile.mine = true;
            tile.road = true;
            tile.railroad = true;
            tile.river = true;
            tile.base = None;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        game.create_city(1, 10, 20);
        let worked: Vec<_> = Simulation::city_radius(&game.cities[0])
            .filter(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
            .map(|(_, _, x, y)| (x, y))
            .collect();
        game.cities[0].size = worked.len() as i32;
        game.cities[0].worked_tiles = worked;
        game.cities[0].buildings.extend([
            Building::FACTORY,
            Building::MFG_PLANT,
            Building::SOLAR_PLANT,
            Building::MARKETPLACE,
            Building::BANK,
            Building::STOCK_EXCHANGE,
        ]);
        let target = offset_native_by_map(10, 20, 4, 0);
        game.tile_mut(target.0, target.1).terrain = Terrain::Hills;
        game.tile_mut(target.0, target.1).resource = None;
        game.tile_mut(target.0, target.1).mine = false;
        game.tile_mut(target.0, target.1).railroad = false;
        game.tile_mut(target.0, target.1).river = false;
        game.create_city(HUMAN, target.0, target.1);
        game.cities.last_mut().unwrap().size = 8;
        assert!(game.city_forecast(&game.cities[0]).shield_surplus() > 45);
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Warrior));
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Archer));
        assert!(game.unit_buildable(&game.cities[0], UnitKind::Legion));
        // kill_something_with retains the buildable counter-defender TYPE
        // even for an empty nearby city. Candidate profits therefore do not
        // tie: Legion's stronger attack beats the cheaper Warrior/Archer.
        assert_eq!(
            game.offensive_production(&game.cities[0]),
            Some(UnitKind::Legion)
        );
    }
}
