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

mod action_rules;
pub(crate) mod religion;
pub(crate) use religion::FaithPolicy;
mod advisors;
mod caches;
#[cfg(any(test, feature = "scenarios"))]
pub(crate) use advisors::AiBuildChoice;
mod ai;
mod ai_economy;
mod ai_strategy;
mod ai_traits;
mod armies;
mod aviation;
mod bases;
mod borders;
mod buildings;
mod civilization_start_rules;
mod native_unit_rules;
mod nuclear;
mod unit_rules;
pub(crate) use bases::Base;
mod cities;
mod civil_war;
#[cfg(feature = "desktop")]
mod client_api;
mod client_views;
#[allow(dead_code)]
#[cfg(feature = "desktop")]
pub(crate) use client_api::{ClientSimulation, CompletedTurn};
pub(crate) use client_views::{
    AirMissionPresentationKind, AirMissionPresentationView, CombatPresentationView,
    PresentationEventView,
};
#[cfg(feature = "desktop")]
pub(crate) use client_views::{
    CityContextView, CityManagerView, DiplomacyMeetingView, DiplomacyRivalView, GameOverView,
    GovernmentView, MinimapCityView, MinimapTileView, MusicCueView, RenderTileView,
    RenderedCityView, RenderedUnitView, ResearchProgressView, SelectableUnitView,
    SpaceshipAssemblyView, TechnologyResearchView, TopBarView, UnitActionView, UnitContextView,
    UnitFidgetView,
};
#[cfg(feature = "desktop")]
pub(crate) use diplomacy::EspionageRivalView;
#[cfg(test)]
pub(crate) use notifications::NotificationCategory;
#[cfg(feature = "desktop")]
pub(crate) use notifications::{Notification, NotificationPriority};
mod combat_rules;
mod commands;
mod diplomacy;
#[cfg(any(test, feature = "scenarios"))]
pub(crate) use diplomacy::VassalKind;
#[cfg(feature = "desktop")]
pub(crate) use diplomacy::VassalTerms;
mod tech_trade;
mod trade_routes;
pub(crate) use diplomacy::{EspionageMission, EspionageRisk, Pact, TreatyMap, TreatyOffer};
mod economy;
mod exploration;
mod government;
mod guards;
mod headless;
pub(crate) mod military_observation;
pub(crate) mod observation;
pub(crate) mod research_observation;
pub(crate) mod trace;
pub(crate) use government::Government;
mod achievements;
mod citizens;
mod climate;
mod culture;
mod disasters;
mod ferries;
mod fuel_paths;
mod great_leader;
mod illness;
mod leaders;
pub(crate) mod lifecycle;
mod list_order;
mod migration;
mod movement;
#[allow(dead_code)] // Shared ruleset adapter is introduced before call-site migration.
mod movement_rules;
mod notifications;
mod presentation;
pub(crate) use presentation::VisualEvent;
#[cfg(test)]
pub(crate) use presentation::{AirMissionVisual, AirMissionVisualKind, CombatVisual};
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
pub(crate) use crate::random::GameRng;
#[cfg(feature = "desktop")]
pub(crate) use route_preview::GotoPoint;
pub(crate) use route_preview::GotoPreview;
mod save;
mod score;
mod settlement;
mod settlement_ferries;
mod spaceship;
use score::NationScore;
mod turn;
mod unit_lifecycle;
mod unit_recovery;
use climate::ClimatePressure;
pub(crate) use pillage::PillageTarget;
pub(crate) use pollution::Contamination;
use spaceship::Spaceship;
#[cfg(any(test, feature = "scenarios"))]
pub(crate) mod drive;
#[cfg(any(test, feature = "scenarios"))]
pub(crate) mod fixtures;
mod research;
#[cfg(test)]
mod research_phase;
mod terrain_work;
pub(crate) mod ui_review;
pub(crate) use commands::Command;
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

use crate::rules::engine_rules::EffectKind;
use crate::rules::{Building, Tech};
pub(crate) use model::*;
mod governor;
pub(crate) use governor::{CityGovernor, GovernorSettings};
use topology::*;

pub(crate) const PLAYER_COUNT: usize = crate::world::DEFAULT_CIVILIZATION_COUNT;
pub(crate) const MAX_CIVILIZATIONS: usize = 16;
pub(crate) const HUMAN: usize = 0;
pub(crate) const PLAYER_SLOTS: usize = 512;
pub(crate) const BARBARIANS: usize = MAX_CIVILIZATIONS;
/// Follows Freeciv's game identifier generation.
const GAME_IDENTIFIER_DRAWS: usize = 32;
/// Follows Freeciv's game identifier alphabet size.
const BASE64URL_CHARACTERS: u32 = 64;
const PERCENT_SCALE: i32 = crate::world::PERCENT_SCALE;
use movement_rules::campaign_move_fragments;
// Historical movement fixtures use the original six-fragment scale.
#[cfg(test)]
const MOVE_FRAGMENTS: i32 = 6;
const ACTIVITY_FACTOR: i32 = 10;
const POWER_FACTOR: i32 = 10;
const CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_SIZE: i32 = 9;
const FORTIFY_DEFENSE_BONUS: i32 = 50;

/// Advances the gameplay stream through `shuffle_players()` after mapgen.
fn apply_player_shuffle_randomness(rng: &mut crate::random::GameRng) {
    consume_shuffle_draws(rng, PLAYER_SLOTS);
}

/// Advances the stream through `init_new_game()`'s game identifier and start
/// assignment. Freeciv randomizes the identifier before shuffling starts.
/// CivCraft retains its Civ III roster and player identities. Freeciv's
/// `map_startpos_iterate` supplies starts in process-address-dependent hash
/// order; `GeneratedMap::starts` instead has deterministic map-generator
/// order. Apply the same Fisher-Yates shuffle to that deterministic order as
/// an explicit adaptation. Pre-map nation/leader draws remain a separate
/// parity gap.
fn apply_start_assignment_randomness(rng: &mut crate::random::GameRng, starts: &mut [(i32, i32)]) {
    // MAX_LEN_GAME_IDENTIFIER is 33, so randomize_base64url_string() draws
    // once for each of the 32 non-terminator characters from a 64-char set.
    for _ in 0..GAME_IDENTIFIER_DRAWS {
        rng.draw(BASE64URL_CHARACTERS);
    }

    // Current generated starts are all unrestricted, so Freeciv shuffles the
    // whole list even when fewer players than starts will be assigned.
    shuffle_with_map_rng(rng, starts);
}

/// Matches utility/shared.c::array_shuffle(): n - 1 calls with bounds n..2.
fn consume_shuffle_draws(rng: &mut crate::random::GameRng, item_count: usize) {
    for bound in (2..=item_count).rev() {
        rng.draw(bound as u32);
    }
}

/// Matches utility/shared.c::array_shuffle()'s Fisher-Yates swaps and bounds.
///
/// Uses Freeciv's shuffle primitive.
fn shuffle_with_map_rng<T>(rng: &mut crate::random::GameRng, items: &mut [T]) {
    for index in 0..items.len().saturating_sub(1) {
        let bound = items.len() - index;
        let swap_index = index + rng.draw(bound as u32) as usize;
        items.swap(index, swap_index);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoveResult {
    Moved,
    Fought,
    Blocked,
}

/// Complete mutable campaign state and the operations that advance its rules.
///
/// The save module defines the disk representation and initializes derived state on load.
#[derive(Clone, Default)]
pub(crate) struct Simulation {
    pub(crate) trace: trace::Collector,
    /// Full caller-provided campaign/game seed used as the reproducible world ID.
    pub(crate) world_seed: u64,
    /// 32-bit map seed drawn from the initialized Freeciv gameplay stream.
    pub(crate) map_seed: u32,
    pub(crate) civilization_count: usize,
    pub(crate) map_wrap: MapWrap,
    tiles: Vec<Tile>,
    units: Vec<Unit>,
    cities: Vec<City>,
    /// Reconstructible memoizations; private storage and key validation live in `caches`.
    caches: caches::QueryCaches,
    trade_routes: Vec<trade_routes::TradeRoute>,
    completed_wonders: Vec<Building>,
    spaceships: Vec<Spaceship>,
    nation_scores: Vec<NationScore>,
    hunter_targets: std::collections::HashMap<u32, u32>,
    previous_naval_positions: std::collections::HashMap<u32, (i32, i32)>,
    diplomacy: std::collections::BTreeMap<(usize, usize), diplomacy::Relation>,
    vassalage: diplomacy::vassalage::VassalState,
    apollo_completed: bool,
    /// Technologies ever discovered by any civilization, including advances
    /// later lost to upkeep. Freeciv's `survives=true, range=World` checks use
    /// this monotonic campaign history.
    global_advances: Vec<Tech>,
    players: Vec<Player>,
    government_adoptions: Vec<i32>,
    turn: i32,
    year: i32,
    /// Freeciv's one-turn state after skipping year zero.
    year_0_hack: bool,
    next_unit_id: u32,
    next_city_id: u32,
    rng: u64,
    native_random: Option<GameRng>,
    cargo_order: std::collections::BTreeMap<u32, Vec<u32>>,
    /// Text-only mirror retained for source-adjacent legacy assertions.
    #[cfg(test)]
    events: Vec<String>,
    notifications: Vec<notifications::Notification>,
    next_notification_id: u64,
    religion: religion::ReligionState,
    visual_events: presentation::VisualEvents,
    game_over: Option<String>,
    climate_pressure: [ClimatePressure; 2],
    climate_current: [i32; 2],
    observed_map: observed_map::ObservedMap,
    ai_purchase_reserve: std::collections::BTreeMap<usize, i32>,
    city_founders: std::collections::BTreeMap<u32, usize>,
    ai_war_plans: std::collections::BTreeMap<(usize, usize), diplomacy::WarPlan>,
    ai_space_warnings: std::collections::BTreeMap<(usize, usize), i32>,
    ai_free_bulbs: std::collections::BTreeMap<usize, i32>,
    ai_tech_wants: std::collections::BTreeMap<usize, Vec<f32>>,
    ai_ground_targets: std::collections::BTreeMap<u32, u32>,
    ai_air_raid_turn: std::collections::BTreeMap<(usize, usize), i32>,
    ai_city_bombed_turn: std::collections::BTreeMap<u32, i32>,
    recon_revealed_tiles: std::collections::BTreeMap<usize, Vec<(i32, i32)>>,
    ai_settlement_targets: std::collections::BTreeMap<u32, settlement::SettlementReservation>,
    ai_relationship_memory:
        std::collections::BTreeMap<(usize, usize), diplomacy::relationships::RelationshipMemory>,
    ai_diplomatic_memory: std::collections::BTreeMap<(usize, usize), diplomacy::DiplomaticMemory>,
    ai_diplomatic_intel: std::collections::BTreeMap<(usize, usize), diplomacy::DiplomaticIntel>,
    ai_pending_war_targets: std::collections::BTreeMap<usize, usize>,
    ai_guard_targets: std::collections::BTreeMap<u32, guards::GuardTarget>,
    ai_unit_guards: std::collections::BTreeMap<u32, guards::GuardState>,
    city_governors: std::collections::BTreeMap<u32, CityGovernor>,
    city_rapture: std::collections::BTreeMap<u32, i32>,
    unit_goto_tiles: std::collections::BTreeMap<u32, (i32, i32)>,
    list_order: list_order::ListOrder,
    ai_worker_tasks: std::collections::BTreeSet<u32>,
    ai_ferries: ferries::FerryRegistry,
    ai_government_cache: std::collections::BTreeMap<usize, government::GovernmentAdvisorCache>,
    diplomatic_contact_until: std::collections::BTreeMap<(usize, usize), i32>,
    founder_phase: bool,
    planning: ai::PlanningCaches,
    advisor_cache: advisors::AdvisorCaches,
    visibility_refresh: observed_map::VisibilityRefresh,
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
        self.unit_index_by_id(id).map(|index| &self.units[index])
    }
    /// Finds a live unit's position in the stable unit collection.
    pub(crate) fn unit_index_by_id(&self, id: u32) -> Option<usize> {
        self.caches.unit_index(&self.units, id)
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
        self.city_index_by_id(id).map(|index| &self.cities[index])
    }
    /// Finds the city occupying the given map coordinate.
    pub(crate) fn city_at(&self, x: i32, y: i32) -> Option<&City> {
        if self.list_order.planning_active() {
            return self
                .list_order
                .city_id_at(x, y)
                .and_then(|id| self.city_by_id(id));
        }
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
        self.caches.city_index(&self.cities, id)
    }
    /// Returns players in their stable slot order.
    pub(crate) fn players(&self) -> &[Player] {
        &self.players
    }
    /// Returns the current campaign turn number.
    pub(crate) fn turn(&self) -> i32 {
        self.turn
    }
    /// Returns the game-over message, if the campaign has ended.
    pub(crate) fn game_over(&self) -> Option<&str> {
        self.game_over.as_deref()
    }

    /// Builds a new campaign from a generated map and initializes player state.
    pub(crate) fn new(map: crate::world::GeneratedMap) -> Self {
        let roster = crate::rules::civilizations::select_roster(map.seed, MAX_CIVILIZATIONS);
        Self::new_with_roster(
            map,
            roster
                .into_iter()
                .map(|civ| (civ, civ.starting_techs.as_slice())),
        )
    }

    fn new_with_roster<'a>(
        map: crate::world::GeneratedMap,
        roster: impl IntoIterator<Item = (&'a crate::rules::civilizations::Civilization, &'a [Tech])>,
    ) -> Self {
        let crate::world::GeneratedMap {
            seed,
            map_seed,
            map_wrap,
            tiles: generated_tiles,
            mut starts,
            civilization_count,
            mut game_random,
        } = map;

        apply_player_shuffle_randomness(&mut game_random);

        let tiles = generated_tiles.into_iter().map(Tile::from).collect();

        let mut players = Vec::new();
        for (i, (civilization, known)) in roster.into_iter().enumerate() {
            players.push(Player {
                name: civilization.name.clone(),
                civilization_id: Some(civilization.id.clone()),
                gold: 50,
                rates: [60, 40, 0],
                government: Government::DESPOTISM,
                history: 0,
                revolution_target: None,
                revolution_turn: None,
                mobilized: false,
                known: known.to_vec(),
                researching: Tech::UNSET,
                researching_saved: None,
                research_saved: 0,
                research_goal: Tech::UNSET,
                future_techs: 0,
                research: 0,
                last_science: 0,
                ai: i != HUMAN,
            });
        }

        assert_eq!(players.len(), MAX_CIVILIZATIONS);
        players.push(Player {
            name: "Barbarians".to_owned(),
            civilization_id: None,
            gold: 0,
            rates: [60, 40, 0],
            government: Government::DESPOTISM,
            history: 0,
            revolution_target: None,
            revolution_turn: None,
            mobilized: false,
            known: Vec::new(),
            researching: Tech::FUTURE,
            researching_saved: None,
            research_saved: 0,
            research_goal: Tech::FUTURE,
            future_techs: 0,
            research: 0,
            last_science: 0,
            ai: true,
        });

        let mut global_advances = Vec::new();
        for tech in players
            .iter()
            .flat_map(|player| player.known.iter().copied())
        {
            if !global_advances.contains(&tech) {
                global_advances.push(tech);
            }
        }

        let calendar = crate::rules::engine_rules::CalendarSettings::campaign();
        let start_year = calendar.start_year;

        let mut game = Self {
            trace: Default::default(),
            world_seed: seed,
            map_seed,
            civilization_count,
            map_wrap,
            tiles,
            units: Vec::new(),
            cities: Vec::new(),
            caches: Default::default(),
            trade_routes: Vec::new(),
            completed_wonders: Vec::new(),
            spaceships: vec![Spaceship::default(); players.len()],
            nation_scores: vec![NationScore::default(); players.len()],
            hunter_targets: Default::default(),
            previous_naval_positions: Default::default(),
            diplomacy: Default::default(),
            vassalage: Default::default(),
            apollo_completed: false,
            global_advances,
            players,
            government_adoptions: vec![0; Government::count()],
            turn: 1,
            year: start_year,
            year_0_hack: false,
            next_unit_id: 1,
            next_city_id: 1,
            rng: 0x9e37_79b9_7f4a_7c15,
            native_random: Some(game_random),
            cargo_order: Default::default(),
            visual_events: Default::default(),
            #[cfg(test)]
            events: Vec::new(),
            notifications: vec![
                notifications::Notification {
                    id: 1,
                    turn: 1,
                    category: notifications::NotificationCategory::General,
                    priority: notifications::NotificationPriority::Routine,
                    text: format!(
                        "{} - Your people are ready to settle.",
                        year_label(start_year)
                    ),
                    related_city: None,
                    related_civilization: None,
                    read: false,
                },
                notifications::Notification {
                    id: 2,
                    turn: 1,
                    category: notifications::NotificationCategory::General,
                    priority: notifications::NotificationPriority::Routine,
                    text: "Choose the Settlers and press B to found a city.".to_owned(),
                    related_city: None,
                    related_civilization: None,
                    read: false,
                },
                notifications::Notification {
                    id: 3,
                    turn: 1,
                    category: notifications::NotificationCategory::General,
                    priority: notifications::NotificationPriority::Routine,
                    text: "Explore, research, grow, and unite the world.".to_owned(),
                    related_city: None,
                    related_civilization: None,
                    read: false,
                },
            ],
            next_notification_id: 4,
            religion: Default::default(),
            game_over: None,
            climate_pressure: [ClimatePressure::new(map.map_wrap.tile_count()); 2],
            climate_current: [0; 2],
            observed_map: Default::default(),
            ai_purchase_reserve: Default::default(),
            city_founders: Default::default(),
            ai_war_plans: Default::default(),
            ai_space_warnings: Default::default(),
            ai_free_bulbs: Default::default(),
            ai_tech_wants: Default::default(),
            ai_ground_targets: Default::default(),
            ai_air_raid_turn: Default::default(),
            ai_city_bombed_turn: Default::default(),
            recon_revealed_tiles: Default::default(),
            ai_settlement_targets: Default::default(),
            ai_relationship_memory: Default::default(),
            ai_diplomatic_memory: Default::default(),
            ai_diplomatic_intel: Default::default(),
            ai_pending_war_targets: Default::default(),
            ai_guard_targets: Default::default(),
            ai_unit_guards: Default::default(),
            city_governors: Default::default(),
            city_rapture: Default::default(),
            unit_goto_tiles: Default::default(),
            list_order: Default::default(),
            ai_worker_tasks: Default::default(),
            ai_ferries: Default::default(),
            ai_government_cache: Default::default(),
            diplomatic_contact_until: Default::default(),
            founder_phase: false,
            planning: Default::default(),
            advisor_cache: Default::default(),
            visibility_refresh: Default::default(),
        };
        for owner in 0..civilization_count {
            game.players[owner].researching = game.choose_research_target(owner);
        }
        let mut game_random = game
            .native_random
            .take()
            .expect("new campaign has an initialized Freeciv random stream");
        apply_start_assignment_randomness(&mut game_random, &mut starts);
        game.native_random = Some(game_random);

        for (owner, start) in starts.iter().take(civilization_count).enumerate() {
            let profile = game.players[owner]
                .civilization_id
                .as_deref()
                .and_then(civilization_start_rules::get)
                .unwrap_or_else(|| {
                    panic!(
                        "missing Civ III start profile for {}",
                        game.players[owner].name
                    )
                });
            for (name, count) in profile.units() {
                let kind = UnitKind::from_catalog_name(name).unwrap_or_else(|| {
                    panic!(
                        "unsupported Civ III starting unit {name} for {}",
                        game.players[owner].name
                    )
                });
                for _ in 0..count {
                    let mut unit = Unit::new(game.next_unit_id, owner, kind, start.0, start.1);
                    unit.moves_left = kind.stats().moves * campaign_move_fragments();
                    game.units.push(unit);
                    game.next_unit_id += 1;
                }
            }
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
        // Freeciv's initial begin_turn() calculates score before any player
        // or AI activity, just as every later turn does.
        game.refresh_score_snapshot();
        game
    }

    #[cfg(test)]
    pub(super) fn idx(x: i32, y: i32) -> usize {
        let wx = x.rem_euclid(MAP_W);
        let wy = y.rem_euclid(MAP_H);
        (wy * MAP_W + wx) as usize
    }

    /// Returns the tile at a valid map coordinate.
    pub(crate) fn tile(&self, x: i32, y: i32) -> &Tile {
        debug_assert!(
            (0..self.map_wrap.height()).contains(&y),
            "off-map cylinder tile ({x},{y})"
        );
        &self.tiles[self.map_wrap.index(x, y)]
    }

    fn tile_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        debug_assert!(
            (0..self.map_wrap.height()).contains(&y),
            "off-map cylinder tile ({x},{y})"
        );
        let idx = self.map_wrap.index(x, y);
        &mut self.tiles[idx]
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

    #[cfg(test)]
    fn set_random_seed(&mut self, seed: u32) {
        self.native_random = Some(GameRng::new(seed));
    }

    #[cfg(test)]
    fn random_snapshot(&self) -> (u64, Option<GameRng>) {
        (self.rng, self.native_random.clone())
    }

    #[cfg(test)]
    fn restore_random(&mut self, snapshot: (u64, Option<GameRng>)) {
        (self.rng, self.native_random) = snapshot;
    }

    /// Reports whether the native random stream remains usable in unit tests state.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn native_random_stream_test(&mut self) -> bool {
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
        let civilization_available = if owner >= self.civilization_count {
            true
        } else {
            kind.conquests_id()
                .and_then(unit_rules::get)
                .is_some_and(|rule| {
                    self.players[owner]
                        .civilization_id
                        .as_deref()
                        .is_some_and(|id| rule.available_to(id))
                })
        };
        !kind.has_no_build()
            && civilization_available
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
        self.map_wrap.step_distance(ax, ay, bx, by)
    }

    fn run_ai_last_activities(&mut self, owner: usize) {
        if owner != BARBARIANS && (owner >= self.civilization_count || !self.player_alive(owner)) {
            return;
        }
        // Worker movement can change threat routes between owners. Cache only
        // within this owner's read-heavy advisor pass, then discard it.
        self.begin_ai_advisor_cache();
        let profiler = self.trace.profiler();
        // ai/default/aihand.c:dai_do_last_activities runs government and
        // taxes before city management and spending, then research.
        self.clear_ai_tech_wants(owner);
        let _profile = profiler.scope("ai.last.government");
        self.choose_ai_government(owner);
        if let Some((tech, want)) = self.government_research_want(owner) {
            self.add_ai_tech_want(owner, tech, want as f32);
        }
        self.choose_ai_rates(owner);
        drop(_profile);
        let _profile = profiler.scope("ai.last.emergency_workers");
        self.arrange_ai_emergency_workers(owner);
        drop(_profile);
        let _profile = profiler.scope("ai.last.city_sales");
        self.manage_ai_city_sales(owner);
        drop(_profile);
        let _profile = profiler.scope("ai.last.domestic_tech_wants");
        self.record_ai_domestic_tech_wants(owner);
        drop(_profile);
        let _profile = profiler.scope("ai.last.city_defenses");
        let mut build_choices = self.prepare_ai_city_defenses(owner);
        self.fund_ai_upkeep_with_coinage(owner, &mut build_choices);
        drop(_profile);
        let _profile = profiler.scope("ai.last.defense_spending");
        self.spend_ai_defense_gold_with_choices(owner, &build_choices);
        drop(_profile);
        let _profile = profiler.scope("ai.last.tech_effect_wants");
        self.record_ai_tech_effect_wants(owner);
        drop(_profile);
        let _profile = profiler.scope("ai.last.research_and_spaceship");
        self.manage_ai_research(owner);
        drop(_profile);
        self.end_ai_advisor_cache();
    }
}

/// Formats an in-game year using the selected ruleset's calendar labels.
pub(crate) fn year_label(year: i32) -> String {
    let calendar = crate::rules::engine_rules::CalendarSettings::campaign();
    if year < 0 {
        format!("{} {}", -year, calendar.negative_label)
    } else {
        format!("{} {}", year, calendar.positive_label)
    }
}

#[cfg(test)]
mod tests;
