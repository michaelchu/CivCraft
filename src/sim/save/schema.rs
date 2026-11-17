//! Explicit positional schemas for the campaign save formats.
use super::super::*;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap};

mod records;

type PairMemory<T> = BTreeMap<(usize, usize), T>;

// Only the legacy readers consume the AI's former reduced tile memory.
#[derive(Clone, Copy)]
pub(super) struct MapObservation {
    pub(super) route: movement::RouteTile,
    pub(super) owner: Option<usize>,
    pub(super) ocean: bool,
}

#[derive(Default, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
#[serde(transparent)]
pub(super) struct LegacyAiMap(
    #[serde(with = "records::map")] pub(super) BTreeMap<(usize, usize), MapObservation>,
);

// Field order is part of the 0x26 through 0x2A wire formats. Make a
// version-specific type if a future version changes any existing payload.
#[derive(Serialize, Deserialize)]
struct SavedCore<'a> {
    world_seed: u64,
    map_seed: u32,
    civilization_count: usize,
    map_wrap: MapWrap,
    #[serde(
        serialize_with = "records::sequence::serialize",
        deserialize_with = "records::sequence::deserialize_cow"
    )]
    tiles: Cow<'a, [Tile]>,
    #[serde(
        serialize_with = "records::sequence::serialize",
        deserialize_with = "records::sequence::deserialize_cow"
    )]
    units: Cow<'a, [Unit]>,
    #[serde(
        serialize_with = "records::sequence::serialize",
        deserialize_with = "records::sequence::deserialize_cow"
    )]
    cities: Cow<'a, [City]>,
    trade_routes: Cow<'a, [trade_routes::TradeRoute]>,
    completed_wonders: Cow<'a, [Building]>,
    spaceships: Cow<'a, [Spaceship]>,
    nation_scores: Cow<'a, [NationScore]>,
    hunter_targets: BTreeMap<u32, u32>,
    previous_naval_positions: BTreeMap<u32, (i32, i32)>,
    diplomacy: Cow<'a, BTreeMap<(usize, usize), diplomacy::Relation>>,
    apollo_completed: bool,
    global_advances: Cow<'a, [Tech]>,
    #[serde(
        serialize_with = "records::sequence::serialize",
        deserialize_with = "records::sequence::deserialize_cow"
    )]
    players: Cow<'a, [Player]>,
    government_adoptions: [i32; 11],
    turn: i32,
    year: i32,
    year_0_hack: bool,
    next_unit_id: u32,
    next_city_id: u32,
    rng: u64,
    events: Cow<'a, [String]>,
    game_over: Cow<'a, Option<String>>,
    climate_pressure: Cow<'a, [ClimatePressure; 2]>,
}

#[derive(Serialize, Deserialize)]
struct SavedAi<'a, A> {
    ai_purchase_reserve: Cow<'a, BTreeMap<usize, i32>>,
    city_founders: Cow<'a, BTreeMap<u32, usize>>,
    // A unit value occupies zero bytes in the current bincode layout.
    legacy_ai_map: A,
    ai_war_plans: Cow<'a, PairMemory<diplomacy::WarPlan>>,
    ai_space_warnings: Cow<'a, PairMemory<i32>>,
    ai_free_bulbs: Cow<'a, BTreeMap<usize, i32>>,
    ai_ground_targets: Cow<'a, BTreeMap<u32, u32>>,
    ai_settlement_targets: Cow<'a, BTreeMap<u32, settlement::SettlementReservation>>,
    ai_diplomatic_memory: Cow<'a, PairMemory<diplomacy::DiplomaticMemory>>,
    ai_ferries: Cow<'a, ferries::FerryRegistry>,
    ai_government_cache: Cow<'a, BTreeMap<usize, government::GovernmentAdvisorCache>>,
    diplomatic_contact_until: Cow<'a, PairMemory<i32>>,
}

#[derive(Serialize, Deserialize)]
struct SavedWorld<'a> {
    climate_current: [i32; 2],
    ai_guard_targets: Cow<'a, BTreeMap<u32, guards::GuardTarget>>,
    ai_unit_guards: Cow<'a, BTreeMap<u32, guards::GuardState>>,
    city_rapture: Cow<'a, BTreeMap<u32, i32>>,
    unit_goto_tiles: Cow<'a, BTreeMap<u32, (i32, i32)>>,
    list_order: list_order::ListOrder,
    native_random: Option<FreecivRng>,
    cargo_order: BTreeMap<u32, Vec<u32>>,
    #[serde(with = "records::observation")]
    observed_map: Cow<'a, observed_map::ObservedMap>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct SavedCampaign<'a, A, S> {
    core: SavedCore<'a>,
    ai: SavedAi<'a, A>,
    world: SavedWorld<'a>,
    partial_sites: S,
}

/// Format 0x2A: the 0x29 payload followed by the ids of units that have
/// already created a Great Leader. Appending keeps every earlier layout valid.
#[derive(Serialize, Deserialize)]
pub(super) struct SavedCampaignV2A<'a, S> {
    campaign: SavedCampaign<'a, (), S>,
    created_leader: Vec<u32>,
}

impl<'a, S> SavedCampaignV2A<'a, S> {
    pub(super) fn from_game(game: &'a Simulation, partial_sites: S) -> Self {
        Self {
            campaign: SavedCampaign::from_game(game, (), partial_sites),
            created_leader: game
                .units
                .iter()
                .filter(|unit| unit.created_leader)
                .map(|unit| unit.id)
                .collect(),
        }
    }

    pub(super) fn into_game(self) -> (Simulation, S) {
        let (mut game, (), partial_sites) = self.campaign.into_game();
        for unit in &mut game.units {
            unit.created_leader = self.created_leader.contains(&unit.id);
        }
        (game, partial_sites)
    }
}

#[derive(Deserialize)]
pub(super) struct LegacyCampaign<'a> {
    core: SavedCore<'a>,
    #[serde(deserialize_with = "records::sequence::deserialize")]
    route_memory: Vec<Option<movement::RouteTile>>,
    ai: SavedAi<'a, LegacyAiMap>,
    world: SavedWorld<'a>,
}

impl LegacyCampaign<'_> {
    pub(super) fn topology(&self) -> MapWrap {
        self.core.map_wrap
    }

    pub(super) fn into_game(self) -> (Simulation, Vec<Option<movement::RouteTile>>, LegacyAiMap) {
        let Self {
            core,
            route_memory,
            ai,
            world,
        } = self;
        let (game, legacy_ai_map, ()) = SavedCampaign {
            core,
            ai,
            world,
            partial_sites: (),
        }
        .into_game();
        (game, route_memory, legacy_ai_map)
    }
}

impl<'a, A, S> SavedCampaign<'a, A, S> {
    pub(super) fn from_game(game: &'a Simulation, legacy_ai_map: A, partial_sites: S) -> Self {
        let mut list_order = game.list_order.clone();
        list_order.synchronize(&game.units, &game.cities);
        let cargo_order = game.cargo_order_snapshot();
        Self {
            core: SavedCore {
                world_seed: game.world_seed,
                map_seed: game.map_seed,
                civilization_count: game.civilization_count,
                map_wrap: game.map_wrap,
                tiles: Cow::Borrowed(&game.tiles),
                units: Cow::Borrowed(&game.units),
                cities: Cow::Borrowed(&game.cities),
                trade_routes: Cow::Borrowed(&game.trade_routes),
                completed_wonders: Cow::Borrowed(&game.completed_wonders),
                spaceships: Cow::Borrowed(&game.spaceships),
                nation_scores: Cow::Borrowed(&game.nation_scores),
                hunter_targets: game
                    .hunter_targets
                    .iter()
                    .map(|(&id, &value)| (id, value))
                    .collect(),
                previous_naval_positions: game
                    .previous_naval_positions
                    .iter()
                    .map(|(&id, &value)| (id, value))
                    .collect(),
                diplomacy: Cow::Borrowed(&game.diplomacy),
                apollo_completed: game.apollo_completed,
                global_advances: Cow::Borrowed(&game.global_advances),
                players: Cow::Borrowed(&game.players),
                government_adoptions: game.government_adoptions,
                turn: game.turn,
                year: game.year,
                year_0_hack: game.year_0_hack,
                next_unit_id: game.next_unit_id,
                next_city_id: game.next_city_id,
                rng: game.rng,
                events: Cow::Borrowed(&game.events),
                game_over: Cow::Borrowed(&game.game_over),
                climate_pressure: Cow::Borrowed(&game.climate_pressure),
            },
            ai: SavedAi {
                ai_purchase_reserve: Cow::Borrowed(&game.ai_purchase_reserve),
                city_founders: Cow::Borrowed(&game.city_founders),
                legacy_ai_map,
                ai_war_plans: Cow::Borrowed(&game.ai_war_plans),
                ai_space_warnings: Cow::Borrowed(&game.ai_space_warnings),
                ai_free_bulbs: Cow::Borrowed(&game.ai_free_bulbs),
                ai_ground_targets: Cow::Borrowed(&game.ai_ground_targets),
                ai_settlement_targets: Cow::Borrowed(&game.ai_settlement_targets),
                ai_diplomatic_memory: Cow::Borrowed(&game.ai_diplomatic_memory),
                ai_ferries: Cow::Borrowed(&game.ai_ferries),
                ai_government_cache: Cow::Borrowed(&game.ai_government_cache),
                diplomatic_contact_until: Cow::Borrowed(&game.diplomatic_contact_until),
            },
            world: SavedWorld {
                climate_current: game.climate_current,
                ai_guard_targets: Cow::Borrowed(&game.ai_guard_targets),
                ai_unit_guards: Cow::Borrowed(&game.ai_unit_guards),
                city_rapture: Cow::Borrowed(&game.city_rapture),
                unit_goto_tiles: Cow::Borrowed(&game.unit_goto_tiles),
                list_order,
                native_random: game.native_random.clone(),
                cargo_order,
                observed_map: Cow::Borrowed(&game.observed_map),
            },
            partial_sites,
        }
    }

    pub(super) fn into_game(self) -> (Simulation, A, S) {
        let Self {
            core,
            ai,
            world,
            partial_sites,
        } = self;
        let game = Simulation {
            world_seed: core.world_seed,
            map_seed: core.map_seed,
            civilization_count: core.civilization_count,
            map_wrap: core.map_wrap,
            tiles: core.tiles.into_owned(),
            units: core.units.into_owned(),
            cities: core.cities.into_owned(),
            trade_routes: core.trade_routes.into_owned(),
            completed_wonders: core.completed_wonders.into_owned(),
            spaceships: core.spaceships.into_owned(),
            nation_scores: core.nation_scores.into_owned(),
            hunter_targets: core.hunter_targets.into_iter().collect(),
            previous_naval_positions: core.previous_naval_positions.into_iter().collect(),
            diplomacy: core.diplomacy.into_owned(),
            apollo_completed: core.apollo_completed,
            global_advances: core.global_advances.into_owned(),
            players: core.players.into_owned(),
            government_adoptions: core.government_adoptions,
            turn: core.turn,
            year: core.year,
            year_0_hack: core.year_0_hack,
            next_unit_id: core.next_unit_id,
            next_city_id: core.next_city_id,
            rng: core.rng,
            events: core.events.into_owned(),
            game_over: core.game_over.into_owned(),
            climate_pressure: core.climate_pressure.into_owned(),
            ai_purchase_reserve: ai.ai_purchase_reserve.into_owned(),
            city_founders: ai.city_founders.into_owned(),
            ai_war_plans: ai.ai_war_plans.into_owned(),
            ai_space_warnings: ai.ai_space_warnings.into_owned(),
            ai_free_bulbs: ai.ai_free_bulbs.into_owned(),
            ai_ground_targets: ai.ai_ground_targets.into_owned(),
            ai_settlement_targets: ai.ai_settlement_targets.into_owned(),
            ai_diplomatic_memory: ai.ai_diplomatic_memory.into_owned(),
            ai_ferries: ai.ai_ferries.into_owned(),
            ai_government_cache: ai.ai_government_cache.into_owned(),
            diplomatic_contact_until: ai.diplomatic_contact_until.into_owned(),
            climate_current: world.climate_current,
            ai_guard_targets: world.ai_guard_targets.into_owned(),
            ai_unit_guards: world.ai_unit_guards.into_owned(),
            city_rapture: world.city_rapture.into_owned(),
            unit_goto_tiles: world.unit_goto_tiles.into_owned(),
            list_order: world.list_order,
            native_random: world.native_random,
            cargo_order: world.cargo_order,
            observed_map: world.observed_map.into_owned(),
            ..Simulation::default()
        };
        (game, ai.legacy_ai_map, partial_sites)
    }
}
