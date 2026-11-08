//! Turn resolution and campaign rules over the shared [`Simulation`] state.
//!
//! `Simulation` is the authority for gameplay: modules implement focused
//! systems for movement, cities, diplomacy, research, economy, AI, and other
//! rules, while [`Command`] is the player-action boundary used by the client.
//! Most systems mutate the same state directly so a completed action is visible
//! to later systems in the current phase.
//!
//! [`model`] contains the persistent campaign entities and their rule data;
//! [`topology`] defines map geometry shared with map generation and rendering.
//! [`turn`] coordinates phase order. [`save`] serializes persistent state and
//! rebuilds transient caches after loading. Per-player observations and AI
//! memories are kept distinct from live map state to prevent hidden information
//! from leaking into presentation or decisions.
//!
//! Preserve deterministic ordering and random-stream boundaries when changing
//! rules: tests and reference fixtures depend on both, and several derived
//! fields are deliberately skipped during save serialization.

mod advisors;
mod ai;
mod ai_economy;
mod aviation;
mod bases;
mod borders;
mod bribery;
mod buildings;
mod nuclear;
pub(crate) use bases::Base;
mod cities;
mod civil_war;
mod commands;
mod diplomacy;
mod tech_trade;
pub(crate) use diplomacy::{Pact, TreatyMap, TreatyOffer};
mod economy;
mod espionage;
mod exploration;
pub(crate) use espionage::SabotageTarget;
mod government;
mod guards;
mod investigation;
pub(crate) use government::Government;
pub(crate) use investigation::CityInvestigation;
mod achievements;
mod citizens;
mod climate;
mod disasters;
mod ferries;
mod fuel_paths;
mod illness;
mod leaders;
mod list_order;
mod movement;
mod presentation;
pub(crate) use presentation::VisualEvent;
mod normal_paths;
mod observed_map;
mod pathfinding;
pub(crate) use observed_map::ObservedCity;
mod output_effects;
mod partisans;
mod pf_actions;
mod pillage;
mod pollution;
mod route_preview;
pub(crate) use route_preview::{GotoPoint, GotoPreview};
mod random;
pub(crate) use random::FreecivRng;
mod save;
mod score;
mod settlement;
mod settlement_ferries;
mod spaceship;
use score::NationScore;
mod trade;
mod turn;
mod unit_lifecycle;
use climate::ClimatePressure;
pub(crate) use pillage::PillageTarget;
pub(crate) use pollution::Contamination;
use spaceship::Spaceship;
pub(crate) use spaceship::{STRUCTURE_PARENT, ShipPart};
mod playtest;
mod research;
#[cfg(test)]
mod research_phase;
mod terrain_work;
mod ui_review;
pub(crate) use commands::Command;
use model::research_cost;
mod military_research;
mod model;
mod offense;
mod roster_actions;
pub(crate) mod topology;
mod unit_catalog;
mod unit_catalog_data;
mod upgrades;
mod upkeep;
mod uprisings;
mod veterancy;
mod villages;
mod workers;
mod worklist;
pub(crate) use worklist::{WORKLIST_LIMIT, WorklistEdit};

use crate::rules::{Building, Tech};
pub(crate) use model::*;
use topology::*;

pub(crate) const PLAYER_COUNT: usize = 6;
pub(crate) const HUMAN: usize = 0;
pub(crate) const PLAYER_SLOTS: usize = 512;
pub(crate) const BARBARIANS: usize = PLAYER_COUNT;
const PERCENT_SCALE: i32 = 100;
const CITY_MIN_DISTANCE: i32 = 3;
pub(crate) const MOVE_FRAGMENTS: i32 = 6;
const ACTIVITY_FACTOR: i32 = 10;
const POWER_FACTOR: i32 = 10;
const BASE_TECH_COST: i32 = 10;
const MIN_TECH_COST: i32 = 10;
const TECH_COST_PERCENT: i32 = 300;
const SCIENCEBOX_PERCENT: i32 = 100;
const FOODBOX_PERCENT: i32 = 100;
pub(crate) const CITIZEN_FOOD_COST: i32 = 2;
const GRANARY_FOOD_INITIAL: [i32; 8] = [20, 20, 20, 20, 20, 30, 30, 40];
const GRANARY_FOOD_INCREMENT: i32 = 0;
const CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_SIZE: i32 = 9;
const FORTIFY_DEFENSE_BONUS: i32 = 50;
const CITY_MIN_HP_PERCENT: i32 = 33;
const BASE_HP_REGEN_2: i32 = 10;
const FORTIFIED_HP_REGEN_2: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoveResult {
    Moved,
    Fought,
    Blocked,
}

type AiMapMemory = std::collections::BTreeMap<(usize, usize), MapObservation>;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct MapObservation {
    route: movement::RouteTile,
    owner: Option<usize>,
    ocean: bool,
}

/// Complete mutable campaign state and the operations that advance its rules.
///
/// Serialized fields form the saved campaign. Runtime-only indexes, visibility
/// snapshots, and event presentation caches are rebuilt or reset as needed.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Simulation {
    pub(crate) world_seed: u64,
    #[serde(skip, default = "topology::legacy_wrap")]
    pub(crate) map_wrap: MapWrap,
    tiles: Vec<Tile>,
    units: Vec<Unit>,
    cities: Vec<City>,
    completed_wonders: Vec<Building>,
    spaceships: Vec<Spaceship>,
    nation_scores: Vec<NationScore>,
    #[serde(serialize_with = "save::serialize_unit_map")]
    hunter_targets: std::collections::HashMap<u32, u32>,
    #[serde(serialize_with = "save::serialize_unit_map")]
    previous_naval_positions: std::collections::HashMap<u32, (i32, i32)>,
    diplomacy: std::collections::BTreeMap<(usize, usize), diplomacy::Relation>,
    apollo_completed: bool,
    players: Vec<Player>,
    government_adoptions: [i32; 9],
    turn: i32,
    year: i32,
    next_unit_id: u32,
    next_city_id: u32,
    rng: u64,
    #[serde(skip)]
    native_random: Option<FreecivRng>,
    #[serde(skip)]
    cargo_order: std::collections::BTreeMap<u32, Vec<u32>>,
    events: Vec<String>,
    #[serde(skip)]
    visual_events: presentation::VisualEvents,
    game_over: Option<String>,
    climate_pressure: [ClimatePressure; 2],
    #[serde(skip)]
    climate_current: [i32; 2],
    trade_routes: Vec<(u32, u32)>,
    #[serde(skip)]
    route_memory: Vec<Option<movement::RouteTile>>,
    #[serde(skip)]
    observed_map: observed_map::ObservedMap,
    #[serde(skip)]
    ai_map_memory: AiMapMemory,
    #[serde(skip)]
    ai_purchase_reserve: std::collections::BTreeMap<usize, i32>,
    #[serde(skip)]
    agent_city_targets: std::collections::BTreeMap<u32, u32>,
    #[serde(skip)]
    city_founders: std::collections::BTreeMap<u32, usize>,
    #[serde(skip)]
    ai_war_plans: std::collections::BTreeMap<(usize, usize), diplomacy::WarPlan>,
    #[serde(skip)]
    ai_space_warnings: std::collections::BTreeMap<(usize, usize), i32>,
    #[serde(skip)]
    ai_free_bulbs: std::collections::BTreeMap<usize, i32>,
    #[serde(skip)]
    ai_tech_wants: std::collections::BTreeMap<usize, Vec<f32>>,
    #[serde(skip)]
    ai_ground_targets: std::collections::BTreeMap<u32, u32>,
    #[serde(skip)]
    ai_merchant_missions: std::collections::BTreeMap<u32, trade::MerchantMission>,
    #[serde(skip)]
    merchant_mission_attempts: std::collections::HashSet<u32>,
    #[serde(skip)]
    merchant_birth_turns: std::collections::BTreeMap<u32, i32>,
    #[serde(skip)]
    ai_settlement_targets: std::collections::BTreeMap<u32, settlement::SettlementReservation>,
    #[serde(skip)]
    ai_diplomatic_memory: std::collections::BTreeMap<(usize, usize), diplomacy::DiplomaticMemory>,
    #[serde(skip)]
    ai_diplomatic_intel: std::collections::BTreeMap<(usize, usize), diplomacy::DiplomaticIntel>,
    #[serde(skip)]
    ai_pending_war_targets: std::collections::BTreeMap<usize, usize>,
    #[serde(skip)]
    ai_guard_targets: std::collections::BTreeMap<u32, guards::GuardTarget>,
    #[serde(skip)]
    ai_unit_guards: std::collections::BTreeMap<u32, guards::GuardState>,
    #[serde(skip)]
    city_rapture: std::collections::BTreeMap<u32, i32>,
    #[serde(skip)]
    unit_goto_tiles: std::collections::BTreeMap<u32, (i32, i32)>,
    #[serde(skip)]
    list_order: list_order::ListOrder,
    #[serde(skip)]
    ai_worker_tasks: std::collections::BTreeSet<u32>,
    #[serde(skip)]
    ai_ferries: ferries::FerryRegistry,
    #[serde(skip)]
    ai_government_cache: std::collections::BTreeMap<usize, government::GovernmentAdvisorCache>,
    #[serde(skip)]
    diplomatic_contact_until: std::collections::BTreeMap<(usize, usize), i32>,
}

fn percent_of(value: i32, percent: i32) -> i32 {
    value * percent / PERCENT_SCALE
}

fn apply_percent_bonus(value: i32, bonus: i32) -> i32 {
    percent_of(value, PERCENT_SCALE + bonus)
}

// common/combat.c:win_chance. This is the probability that the attacker
// scores every required hit before the defender does, including unequal HP
// and firepower. The native defender-selection path uses this exact result.
fn combat_win_chance(
    attack_strength: i32,
    attacker_hp: i32,
    attacker_firepower: i32,
    defense_strength: i32,
    defender_hp: i32,
    defender_firepower: i32,
) -> f64 {
    let attacker_rounds_to_lose = (attacker_hp + defender_firepower - 1) / defender_firepower;
    let defender_rounds_to_lose = (defender_hp + attacker_firepower - 1) / attacker_firepower;
    let total_strength = attack_strength + defense_strength;
    let attacker_round_loss = if total_strength == 0 {
        0.5
    } else {
        defense_strength as f64 / total_strength as f64
    };
    let defender_round_loss = 1.0 - attacker_round_loss;

    let mut binomial_term = defender_round_loss.powi(defender_rounds_to_lose.saturating_sub(1));
    let mut accumulated = binomial_term;
    for lost_rounds in 1..attacker_rounds_to_lose {
        let series_length = lost_rounds + defender_rounds_to_lose - 1;
        binomial_term *= series_length as f64;
        binomial_term /= lost_rounds as f64;
        binomial_term *= attacker_round_loss;
        accumulated += binomial_term;
    }
    (accumulated * defender_round_loss).clamp(0.0, 1.0)
}

impl Simulation {
    /// Returns all live units in their stable simulation order.
    pub(crate) fn units(&self) -> &[Unit] {
        &self.units
    }
    /// Iterates live units owned by one player.
    pub(crate) fn units_owned_by(&self, owner: usize) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(move |unit| unit.owner == owner)
    }
    /// Finds a live unit by its persistent identifier.
    pub(crate) fn unit_by_id(&self, id: u32) -> Option<&Unit> {
        self.units.iter().find(|unit| unit.id == id)
    }
    /// Finds a live unit's position in the stable unit collection.
    pub(crate) fn unit_index_by_id(&self, id: u32) -> Option<usize> {
        self.units.iter().position(|unit| unit.id == id)
    }
    /// Returns all cities in their stable simulation order.
    pub(crate) fn cities(&self) -> &[City] {
        &self.cities
    }
    /// Iterates cities owned by one player.
    pub(crate) fn cities_owned_by(&self, owner: usize) -> impl Iterator<Item = &City> {
        self.cities.iter().filter(move |city| city.owner == owner)
    }
    /// Finds a city by its persistent identifier.
    pub(crate) fn city_by_id(&self, id: u32) -> Option<&City> {
        self.cities.iter().find(|city| city.id == id)
    }
    /// Finds the city occupying the given map coordinate.
    pub(crate) fn city_at(&self, x: i32, y: i32) -> Option<&City> {
        self.cities.iter().find(|city| (city.x, city.y) == (x, y))
    }
    /// Finds the index of the city occupying the given map coordinate.
    pub(crate) fn city_index_at(&self, x: i32, y: i32) -> Option<usize> {
        self.cities
            .iter()
            .position(|city| (city.x, city.y) == (x, y))
    }
    /// Finds a city's index by its persistent identifier.
    pub(crate) fn city_index_by_id(&self, id: u32) -> Option<usize> {
        self.cities.iter().position(|city| city.id == id)
    }
    /// Returns players in their stable slot order.
    pub(crate) fn players(&self) -> &[Player] {
        &self.players
    }
    /// Returns the accumulated player-facing event log.
    pub(crate) fn events(&self) -> &[String] {
        &self.events
    }
    /// Returns the current campaign turn number.
    pub(crate) fn turn(&self) -> i32 {
        self.turn
    }
    /// Returns the current in-game year.
    pub(crate) fn year(&self) -> i32 {
        self.year
    }
    /// Returns the game-over message, if the campaign has ended.
    pub(crate) fn game_over(&self) -> Option<&str> {
        self.game_over.as_deref()
    }

    /// Builds a new campaign from a generated map and initializes player state.
    pub(crate) fn new(map: crate::mapgen::GeneratedMap) -> Self {
        let crate::mapgen::GeneratedMap {
            seed,
            map_wrap,
            tiles,
            starts,
            game_random,
        } = map;

        let free_techs = [
            Tech::BRONZE_WORKING,
            Tech::ALPHABET,
            Tech::HORSEBACK_RIDING,
            Tech::POTTERY,
            Tech::WARRIOR_CODE,
            Tech::BRONZE_WORKING,
        ];
        let research_targets = [
            Tech::WARRIOR_CODE,
            Tech::BRONZE_WORKING,
            Tech::POTTERY,
            Tech::HORSEBACK_RIDING,
            Tech::ALPHABET,
            Tech::WARRIOR_CODE,
        ];
        let mut players = Vec::new();
        for i in 0..PLAYER_COUNT {
            players.push(Player {
                name: crate::rules::civilizations::CIVILIZATIONS[i]
                    .name
                    .to_owned(),
                gold: 50,
                rates: [60, 40, 0],
                government: Government::Despotism,
                revolution_target: None,
                revolution_turn: None,
                known: vec![free_techs[i]],
                researching: research_targets[i],
                research_goal: [
                    research_targets[i],
                    Tech::IRON_WORKING,
                    Tech::THE_WHEEL,
                    Tech::MATHEMATICS,
                    Tech::IRON_WORKING,
                    Tech::THE_WHEEL,
                ][i],
                future_techs: 0,
                research: 0,
                last_science: 0,
                ai: i != HUMAN,
            });
        }

        players.push(Player {
            name: "Barbarians".to_owned(),
            gold: 0,
            rates: [60, 40, 0],
            government: Government::Despotism,
            revolution_target: None,
            revolution_turn: None,
            known: Vec::new(),
            researching: Tech::FUTURE,
            research_goal: Tech::FUTURE,
            future_techs: 0,
            research: 0,
            last_science: 0,
            ai: true,
        });

        let mut units = Vec::new();
        let mut next_unit_id = 1;
        for (owner, start) in starts.iter().take(PLAYER_COUNT).enumerate() {
            for kind in [
                UnitKind::Settlers,
                UnitKind::Worker,
                UnitKind::Explorer,
                UnitKind::Diplomat,
            ] {
                let mut unit = Unit::new(next_unit_id, owner, kind, start.0, start.1);
                unit.moves_left = kind.stats().moves * MOVE_FRAGMENTS;
                units.push(unit);
                next_unit_id += 1;
            }
        }

        let mut game = Self {
            world_seed: seed,
            map_wrap,
            tiles,
            units,
            cities: Vec::new(),
            completed_wonders: Vec::new(),
            spaceships: vec![Spaceship::default(); players.len()],
            nation_scores: vec![NationScore::default(); players.len()],
            hunter_targets: Default::default(),
            previous_naval_positions: Default::default(),
            diplomacy: Default::default(),
            apollo_completed: false,
            players,
            government_adoptions: [0; 9],
            turn: 1,
            year: -4000,
            next_unit_id,
            next_city_id: 1,
            rng: 0x9e37_79b9_7f4a_7c15,
            native_random: Some(game_random),
            cargo_order: Default::default(),
            visual_events: Default::default(),
            events: vec![
                "4000 BCE - Your people are ready to settle.".to_owned(),
                "Choose the Settlers and press B to found a city.".to_owned(),
                "Explore, research, grow, and unite the world.".to_owned(),
            ],
            game_over: None,
            climate_pressure: [ClimatePressure::new(map.map_wrap.tile_count()); 2],
            climate_current: [0; 2],
            trade_routes: Vec::new(),
            route_memory: vec![None; map.map_wrap.tile_count()],
            observed_map: Default::default(),
            ai_map_memory: Default::default(),
            ai_purchase_reserve: Default::default(),
            agent_city_targets: Default::default(),
            city_founders: Default::default(),
            ai_war_plans: Default::default(),
            ai_space_warnings: Default::default(),
            ai_free_bulbs: Default::default(),
            ai_tech_wants: Default::default(),
            ai_ground_targets: Default::default(),
            ai_merchant_missions: Default::default(),
            merchant_mission_attempts: Default::default(),
            merchant_birth_turns: Default::default(),
            ai_settlement_targets: Default::default(),
            ai_diplomatic_memory: Default::default(),
            ai_diplomatic_intel: Default::default(),
            ai_pending_war_targets: Default::default(),
            ai_guard_targets: Default::default(),
            ai_unit_guards: Default::default(),
            city_rapture: Default::default(),
            unit_goto_tiles: Default::default(),
            list_order: Default::default(),
            ai_worker_tasks: Default::default(),
            ai_ferries: Default::default(),
            ai_government_cache: Default::default(),
            diplomatic_contact_until: Default::default(),
        };
        for owner in 1..PLAYER_COUNT {
            game.players[owner].researching = game.research_target(owner);
        }
        let initial_contacts: Vec<_> = game
            .units
            .iter()
            .map(|unit| (unit.owner, unit.x, unit.y))
            .collect();
        for (owner, x, y) in initial_contacts {
            game.refresh_diplomatic_contact_at(owner, x, y);
        }
        game.sync_list_order();
        game.refresh_visibility();
        game
    }

    #[cfg(test)]
    fn idx(x: i32, y: i32) -> usize {
        let wx = x.rem_euclid(MAP_W);
        let wy = y.rem_euclid(MAP_H);
        (wy * MAP_W + wx) as usize
    }

    /// Returns the tile at a valid map coordinate.
    pub(crate) fn tile(&self, x: i32, y: i32) -> &Tile {
        debug_assert!(
            self.map_wrap.wraps_y() || (0..self.map_wrap.height()).contains(&y),
            "off-map cylinder tile ({x},{y})"
        );
        &self.tiles[self.map_wrap.index(x, y)]
    }

    fn tile_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        debug_assert!(
            self.map_wrap.wraps_y() || (0..self.map_wrap.height()).contains(&y),
            "off-map cylinder tile ({x},{y})"
        );
        let idx = self.map_wrap.index(x, y);
        &mut self.tiles[idx]
    }

    /// Appends a message to the event log shown to the player.
    pub(crate) fn add_event(&mut self, text: impl Into<String>) {
        self.events.push(text.into());
        if self.events.len() > 10 {
            self.events.remove(0);
        }
    }

    fn rand_below(&mut self, max: i32) -> i32 {
        if max <= 1 {
            return 0;
        }
        if let Some(random) = &mut self.native_random {
            return random.draw(max as u32) as i32;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % max as u64) as i32
    }

    fn set_random_seed(&mut self, seed: u32) {
        self.native_random = Some(FreecivRng::new(seed));
    }

    fn random_snapshot(&self) -> (u64, Option<FreecivRng>) {
        (self.rng, self.native_random.clone())
    }

    fn restore_random(&mut self, snapshot: (u64, Option<FreecivRng>)) {
        (self.rng, self.native_random) = snapshot;
    }

    /// Reports whether the native random stream remains usable in playtest state.
    pub(crate) fn native_random_stream_playtest(&mut self) -> bool {
        let same = Self::new(crate::mapgen::generate_with_wrap(
            self.world_seed,
            self.map_wrap,
        ));
        let reproducible =
            self.native_random.is_some() && self.random_snapshot() == same.random_snapshot();
        let other = Self::new(crate::mapgen::generate_with_wrap(
            self.world_seed.wrapping_add(1),
            self.map_wrap,
        ));
        let seed_matters = self.random_snapshot() != other.random_snapshot();
        self.set_random_seed(42);
        // Literal fc_srand/fc_rand oracle values, independent of this wrapper.
        let oracle = [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 2),
            (7, 4),
            (56, 41),
            (100, 4),
            (1000, 186),
        ];
        let equal = oracle
            .into_iter()
            .all(|(bound, value)| self.rand_below(bound) == value);
        let before = self.random_snapshot();
        let trivial =
            self.rand_below(0) == 0 && self.rand_below(1) == 0 && self.random_snapshot() == before;
        reproducible && seed_matters && equal && trivial
    }

    /// Checks whether a player has unlocked a unit kind.
    pub(crate) fn unit_available(&self, owner: usize, kind: UnitKind) -> bool {
        !kind.has_no_build()
            && kind
                .required_government()
                .is_none_or(|government| self.players[owner].government == government)
            && (!kind.is_nuclear_weapon() || self.manhattan_project_completed())
            && kind
                .required_tech()
                .is_none_or(|tech| self.has_tech(owner, tech))
    }

    /// Measures Civ III graph distance between wrapped map coordinates.
    pub(crate) fn map_distance(&self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        self.map_wrap.civ3_distance(ax, ay, bx, by)
    }

    fn run_ai_last_activities(&mut self, owner: usize) {
        // ai/default/aihand.c:dai_do_last_activities runs government and
        // taxes before city management and spending, then research.
        self.clear_ai_tech_wants(owner);
        self.choose_ai_government(owner);
        if let Some((tech, want)) = self.government_research_want(owner) {
            self.add_ai_tech_want(owner, tech, want as f32);
        }
        self.choose_ai_rates(owner);
        self.arrange_ai_emergency_workers(owner);
        self.record_ai_domestic_tech_wants(owner);
        self.prepare_ai_city_defenses(owner);
        self.spend_ai_defense_gold(owner);
        self.record_ai_tech_effect_wants(owner);
        self.manage_ai_research(owner);
    }
}

/// Formats an in-game year using the campaign calendar's BC/AD convention.
pub(crate) fn year_label(year: i32) -> String {
    if year < 0 {
        format!("{} BCE", -year)
    } else {
        format!("{} CE", year)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
