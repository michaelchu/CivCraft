//! Rendering of cities, units, and other world entities.

use super::units::UnitKind;
use crate::rules::{Building, Tech};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UnitState {
    Idle,
    Fortifying,
    Fortified,
    Sentry,
    AirSuperiority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum WorkKind {
    Road,
    Railroad,
    Irrigation,
    Farmland,
    Cultivate,
    Plant,
    Pillage(crate::sim::PillageTarget),
    BuildBase(crate::sim::Base),
    Clean(crate::sim::Contamination),
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
    pub(in crate::sim) nationality: usize,
    pub(crate) owner: usize,
    pub(crate) kind: UnitKind,
    pub(crate) transport: Option<u32>,
    pub(crate) home_city: Option<u32>,
    pub(in crate::sim) home_order: u32,
    pub(crate) veteran: usize,
    pub(crate) fuel: u8,
    /// OpenCiv3 FLC direction row: SW, S, SE, E, NE, N, NW, W.
    pub(crate) facing: u8,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) hp: i32,
    pub(crate) moves_left: i32,
    pub(crate) state: UnitState,
    pub(crate) moved: bool,
    pub(crate) automated: bool,
    pub(crate) work_kind: Option<WorkKind>,
    pub(crate) work_left: i32,
    pub(in crate::sim) work_done: i32,
}

impl Unit {
    pub(in crate::sim) fn new(id: u32, owner: usize, kind: UnitKind, x: i32, y: i32) -> Self {
        let veteran = if kind.conquests_id().is_some() {
            crate::sim::civ3_unit_rules::default_experience_rank()
        } else {
            0
        };
        Self {
            id,
            nationality: owner,
            owner,
            kind,
            transport: None,
            home_city: None,
            home_order: 0,
            veteran,
            fuel: kind.fuel_capacity(),
            facing: 2,
            x,
            y,
            hp: kind.hit_points_at_rank(veteran),
            moves_left: 0,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_left: 0,
            work_done: 0,
        }
    }

    pub(crate) fn max_hit_points(&self) -> i32 {
        self.kind.hit_points_at_rank(self.veteran)
    }
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

    pub(in crate::sim) fn class(self) -> u8 {
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
    pub(in crate::sim) was_happy: bool,
    pub(in crate::sim) last_plague: Option<i32>,
    pub(in crate::sim) history: i32,
    /// Accumulated gameplay culture, retained by its original civilization
    /// when this city changes hands. Sorted by civilization slot for stable saves.
    pub(in crate::sim) culture_by_civilization: Vec<(usize, i64)>,
    /// BIQ building identity to completion year, used for the documented
    /// age-based doubling of each building's culture contribution.
    pub(in crate::sim) culture_building_years: Vec<(usize, i32)>,
    pub(in crate::sim) foreign_citizens: Vec<(usize, i32)>,
    pub(in crate::sim) airlift_used: bool,
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) production: Production,
    pub(in crate::sim) founded_turn: i32,
    pub(crate) bought_production: bool,
    pub(in crate::sim) sold_building: bool,
    pub(crate) worklist: Vec<Production>,
    pub(in crate::sim) protected_shields: i32,
    pub(in crate::sim) production_before_switch: Option<(Production, i32)>,
    pub(crate) buildings: Vec<Building>,
}

impl City {
    pub(in crate::sim) fn new(
        id: u32,
        owner: usize,
        x: i32,
        y: i32,
        name: String,
        founded_turn: i32,
    ) -> Self {
        Self {
            id,
            owner,
            x,
            y,
            name,
            size: 1,
            specialists: [0; 3],
            worked_tiles: Vec::new(),
            was_happy: false,
            last_plague: None,
            history: 0,
            culture_by_civilization: vec![(owner, 0)],
            culture_building_years: Vec::new(),
            foreign_citizens: Vec::new(),
            airlift_used: false,
            food: 0,
            shields: 0,
            production: Production::Unit(UnitKind::Warrior),
            founded_turn,
            bought_production: false,
            sold_building: false,
            worklist: Vec::new(),
            protected_shields: 0,
            production_before_switch: None,
            buildings: Vec::new(),
        }
    }

    pub(crate) fn culture_for_civilization(&self, civilization: usize) -> i64 {
        self.culture_by_civilization
            .binary_search_by_key(&civilization, |&(slot, _)| slot)
            .map_or(0, |index| self.culture_by_civilization[index].1)
    }

    pub(crate) fn total_culture(&self) -> i64 {
        self.culture_by_civilization
            .iter()
            .map(|&(_, culture)| culture)
            .sum()
    }

    pub(crate) fn add_culture(&mut self, civilization: usize, amount: i64) {
        assert!(amount >= 0, "culture accumulation must be nonnegative");
        match self
            .culture_by_civilization
            .binary_search_by_key(&civilization, |&(slot, _)| slot)
        {
            Ok(index) => {
                self.culture_by_civilization[index].1 =
                    self.culture_by_civilization[index].1.saturating_add(amount);
            }
            Err(index) => self
                .culture_by_civilization
                .insert(index, (civilization, amount)),
        }
    }

    pub(in crate::sim) fn record_culture_building_completion(
        &mut self,
        building: Building,
        year: i32,
    ) {
        match self
            .culture_building_years
            .binary_search_by_key(&building.0, |&(id, _)| id)
        {
            Ok(index) => self.culture_building_years[index].1 = year,
            Err(index) => self
                .culture_building_years
                .insert(index, (building.0, year)),
        }
    }

    pub(in crate::sim) fn culture_building_year(
        &mut self,
        building: Building,
        fallback_year: i32,
    ) -> i32 {
        match self
            .culture_building_years
            .binary_search_by_key(&building.0, |&(id, _)| id)
        {
            Ok(index) => self.culture_building_years[index].1,
            Err(index) => {
                self.culture_building_years
                    .insert(index, (building.0, fallback_year));
                fallback_year
            }
        }
    }

    pub(in crate::sim) fn ensure_culture_civilization(&mut self, civilization: usize) {
        if let Err(index) = self
            .culture_by_civilization
            .binary_search_by_key(&civilization, |&(slot, _)| slot)
        {
            self.culture_by_civilization
                .insert(index, (civilization, 0));
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Player {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) civilization_id: Option<String>,
    pub(crate) gold: i32,
    pub(crate) rates: [i32; 3],
    pub(crate) government: crate::sim::Government,
    pub(crate) revolution_target: Option<crate::sim::Government>,
    pub(crate) revolution_turn: Option<i32>,
    pub(crate) known: Vec<Tech>,
    pub(crate) researching: Tech,
    pub(crate) research_goal: Tech,
    pub(crate) future_techs: i32,
    pub(crate) research: i32,
    pub(in crate::sim) last_science: i32,
    pub(crate) ai: bool,
}

impl Player {
    pub(crate) fn leader_name(&self) -> String {
        self.civilization_id
            .as_deref()
            .and_then(crate::rules::civilizations::by_id)
            .or_else(|| crate::rules::civilizations::by_name(&self.name))
            .map(|civilization| civilization.leader.clone())
            .unwrap_or_else(|| {
                if self.name == "Barbarians" {
                    "Chieftain".to_owned()
                } else {
                    "Rebel Council".to_owned()
                }
            })
    }
}
