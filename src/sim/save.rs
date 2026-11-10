//! Current campaign persistence, input validation, and runtime restoration.
//!
//! Saves encode the durable part of [`Simulation`] with the current versioned
//! binary format. Older formats are not migrated or loaded.
//!
//! When adding persistent fields, decide whether they belong in the save,
//! update the round-trip and corruption checks. Runtime caches should remain excluded when
//! they can be derived from saved state.

use super::*;
use bincode::Options;
mod playtest;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

// Make save/replay comparisons independent of randomized HashMap bucket order.
pub(super) fn serialize_unit_map<S, V>(
    map: &std::collections::HashMap<u32, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    V: serde::Serialize,
{
    let ordered: std::collections::BTreeMap<_, _> = map.iter().collect();
    serde::Serialize::serialize(&ordered, serializer)
}

const HEADER: &[u8] = b"CIVCRAFT\0\x14";
const MAX_PAYLOAD: u64 = 16 * 1024 * 1024;

type PairMemory<T> = std::collections::BTreeMap<(usize, usize), T>;
type UnitTargets = std::collections::BTreeMap<u32, u32>;
type Strategy8 = (
    PairMemory<diplomacy::WarPlan>,
    PairMemory<i32>,
    std::collections::BTreeMap<usize, i32>,
    UnitTargets,
    std::collections::BTreeMap<u32, settlement::SettlementReservation>,
    PairMemory<diplomacy::DiplomaticMemory>,
);
type Strategy9 = (
    Strategy8,
    (
        ferries::FerryRegistry,
        std::collections::BTreeMap<usize, government::GovernmentAdvisorCache>,
        PairMemory<i32>,
    ),
);
type Extras11 = (
    [i32; 2],
    std::collections::BTreeMap<u32, guards::GuardTarget>,
    std::collections::BTreeMap<u32, guards::GuardState>,
    std::collections::BTreeMap<u32, i32>,
    std::collections::BTreeMap<u32, (i32, i32)>,
);
type Extras13 = (
    Option<FreecivRng>,
    std::collections::BTreeMap<u32, Vec<u32>>,
);
type CampaignPayload<A> = (
    Simulation,
    Vec<Option<movement::RouteTile>>,
    std::collections::BTreeMap<usize, i32>,
    std::collections::BTreeMap<u32, usize>,
    AiMapMemory,
    A,
);

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

impl Simulation {
    pub(crate) fn save_campaign(&self, path: &Path) -> io::Result<()> {
        let mut list_order = self.list_order.clone();
        list_order.synchronize(&self.units, &self.cities);
        let cargo_order = self.cargo_order_snapshot();
        let payload = codec()
            .serialize(&(
                (
                    self,
                    &self.route_memory,
                    &self.ai_purchase_reserve,
                    &self.city_founders,
                    &self.ai_map_memory,
                    (
                        (
                            &self.ai_war_plans,
                            &self.ai_space_warnings,
                            &self.ai_free_bulbs,
                            &self.ai_ground_targets,
                            &self.ai_settlement_targets,
                            &self.ai_diplomatic_memory,
                        ),
                        (
                            &self.ai_ferries,
                            &self.ai_government_cache,
                            &self.diplomatic_contact_until,
                        ),
                    ),
                ),
                (
                    &self.climate_current,
                    &self.ai_guard_targets,
                    &self.ai_unit_guards,
                    &self.city_rapture,
                    &self.unit_goto_tiles,
                ),
                &list_order,
                (&self.native_random, &cargo_order),
                &self.observed_map,
                self.map_wrap,
            ))
            .map_err(io::Error::other)?;
        let filename = path
            .file_name()
            .ok_or_else(|| invalid("Missing save filename"))?;
        let mut temp_name = filename.to_os_string();
        temp_name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let temp = path.with_file_name(temp_name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let result = (|| {
            file.write_all(HEADER)?;
            file.write_all(&payload)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    pub(crate) fn load_campaign(path: &Path) -> io::Result<Self> {
        let mut file = fs::File::open(path)?;
        let mut header = [0; HEADER.len()];
        file.read_exact(&mut header)?;
        if header != HEADER {
            return Err(invalid("Unsupported CivCraft save format"));
        }
        let mut payload = Vec::new();
        file.take(MAX_PAYLOAD + 1).read_to_end(&mut payload)?;
        if payload.len() as u64 > MAX_PAYLOAD {
            return Err(invalid("CivCraft save exceeds the size limit"));
        }
        let (loaded, memory, reserves, founders, observations, ai_strategy) =
            decode_current(&payload).map_err(|error| invalid(error.to_string()))?;
        let mut game = loaded;
        let (
            (
                war_plans,
                space_warnings,
                free_bulbs,
                ground_targets,
                settlement_targets,
                diplomatic_memory,
            ),
            (ferries, government_cache, contact_until),
        ): Strategy9 = ai_strategy;
        let observations: AiMapMemory = observations;
        let founders: std::collections::BTreeMap<u32, usize> = founders;
        let reserves: std::collections::BTreeMap<usize, i32> = reserves;
        let memory: Vec<Option<movement::RouteTile>> = memory;
        game.validate_campaign()?;
        if memory.len() != game.tiles.len()
            || memory.iter().flatten().any(|tile| {
                tile.city_owner
                    .is_some_and(|owner| owner >= game.players.len())
            })
        {
            return Err(invalid("Invalid remembered map"));
        }
        if reserves
            .iter()
            .any(|(&owner, &price)| owner >= game.players.len() || price <= 0)
        {
            return Err(invalid("Invalid AI purchase reserve"));
        }
        game.route_memory = memory;
        game.ai_purchase_reserve = reserves;
        if founders.iter().any(|(&city, &owner)| {
            city == 0 || city >= game.next_city_id || owner >= game.players.len()
        }) {
            return Err(invalid("Invalid city founder"));
        }
        game.city_founders = founders;
        if observations.iter().any(|(&(owner, index), tile)| {
            owner == HUMAN
                || owner == BARBARIANS
                || owner >= game.players.len()
                || index >= game.tiles.len()
                || tile.owner.is_some_and(|owner| owner >= game.players.len())
                || tile
                    .route
                    .city_owner
                    .is_some_and(|owner| owner >= game.players.len())
        }) {
            return Err(invalid("Invalid AI map observation"));
        }
        game.ai_map_memory = observations;
        if war_plans.keys().any(|&(owner, rival)| {
            owner >= game.players.len()
                || rival >= game.players.len()
                || owner == rival
                || owner == BARBARIANS
                || rival == BARBARIANS
        }) {
            return Err(invalid("Invalid AI war plan"));
        }
        game.ai_war_plans = war_plans;
        if space_warnings.iter().any(|(&(owner, rival), &timer)| {
            owner >= game.players.len()
                || rival >= game.players.len()
                || owner == rival
                || owner == BARBARIANS
                || rival == BARBARIANS
                || timer < 0
        }) {
            return Err(invalid("Invalid AI space warning"));
        }
        game.ai_space_warnings = space_warnings;
        if free_bulbs
            .keys()
            .any(|&owner| owner >= game.players.len() || owner == BARBARIANS)
        {
            return Err(invalid("Invalid AI free research bulbs"));
        }
        game.ai_free_bulbs = free_bulbs;
        if ground_targets.iter().any(|(&unit, &city)| {
            !game.units.iter().any(|actor| actor.id == unit)
                || !game.cities.iter().any(|target| target.id == city)
        }) {
            return Err(invalid("Invalid AI ground mission"));
        }
        game.ai_ground_targets = ground_targets;
        if settlement_targets.iter().any(|(&unit, reservation)| {
            let (x, y) = reservation.destination;
            !game.units.iter().any(|actor| {
                actor.id == unit
                    && actor.kind == UnitKind::Settler
                    && actor.owner == reservation.owner
            }) || !(0..game.map_wrap.width()).contains(&x)
                || !(0..game.map_wrap.height()).contains(&y)
                || reservation.worker.is_some_and(|(x, y)| {
                    !(0..game.map_wrap.width()).contains(&x)
                        || !(0..game.map_wrap.height()).contains(&y)
                })
        }) {
            return Err(invalid("Invalid AI settlement reservation"));
        }
        game.ai_settlement_targets = settlement_targets;
        if diplomatic_memory.keys().any(|&(owner, rival)| {
            owner >= game.players.len()
                || rival >= game.players.len()
                || owner == rival
                || owner == BARBARIANS
                || rival == BARBARIANS
        }) {
            return Err(invalid("Invalid AI diplomatic memory"));
        }
        if diplomatic_memory.values().any(|memory| {
            memory.spam < 0
                || memory.asked_peace < 0
                || memory.asked_alliance < 0
                || memory.asked_ceasefire < 0
        }) {
            return Err(invalid("Invalid AI diplomatic timers"));
        }
        game.ai_diplomatic_memory = diplomatic_memory;
        if !ferries.valid(&game.units) {
            return Err(invalid("Invalid AI ferry registry"));
        }
        if government_cache.iter().any(|(&owner, cache)| {
            owner >= game.players.len()
                || cache.evaluated_turn > game.turn
                || cache.next_evaluation < cache.evaluated_turn
                || cache.scores.iter().any(|score| !score.is_finite())
        }) {
            return Err(invalid("Invalid AI government cache"));
        }
        if contact_until
            .keys()
            .any(|&(a, b)| a >= b || b >= game.players.len() || a == BARBARIANS || b == BARBARIANS)
        {
            return Err(invalid("Invalid diplomatic contact"));
        }
        game.ai_ferries = ferries;
        game.ai_government_cache = government_cache;
        game.diplomatic_contact_until = contact_until;
        if !game.list_order.valid(&game.units, &game.cities) {
            return Err(invalid("Invalid native entity list order"));
        }
        if !game.valid_cargo_order() {
            return Err(invalid("Invalid transport cargo order"));
        }
        if !game.validate_ai_guards() {
            return Err(invalid("Invalid AI bodyguard assignments"));
        }
        if game
            .city_rapture
            .iter()
            .any(|(&id, &counter)| counter < 0 || !game.cities.iter().any(|city| city.id == id))
        {
            return Err(invalid("Invalid city celebration history"));
        }
        if let Some((&id, &(x, y))) = game.unit_goto_tiles.iter().find(|&(&id, &(x, y))| {
            !game.units.iter().any(|unit| unit.id == id)
                || !(0..game.map_wrap.width()).contains(&x)
                || !(0..game.map_wrap.height()).contains(&y)
        }) {
            return Err(invalid(format!(
                "Invalid unit movement destination for {id}: ({x}, {y}); live unit: {}",
                game.units.iter().any(|unit| unit.id == id)
            )));
        }
        // savegame3.c clears goto_tile after loading units with no orders
        // unless activity is ACTIVITY_GOTO. This engine has no queued native
        // GOTO activity; task destinations are reconstructed by the advisors.
        game.unit_goto_tiles.clear();
        if !game.valid_render_memory() {
            return Err(invalid("Invalid observed rendering map"));
        }
        game.refresh_diplomatic_intelligence();
        Ok(game)
    }

    fn validate_campaign(&self) -> io::Result<()> {
        let players = self.players.len();
        let position = |x, y| {
            (0..self.map_wrap.width()).contains(&x) && (0..self.map_wrap.height()).contains(&y)
        };
        let tech = |tech: Tech| tech == Tech::FUTURE || tech.data().is_some();
        let building = |building| Building::all().any(|known| known == building);
        let production = |item| match item {
            Production::Building(item) => building(item),
            Production::Unit(_) => true,
        };
        if !(PLAYER_COUNT + 1..=PLAYER_SLOTS).contains(&players)
            || self.tiles.len() != (self.map_wrap.width() * self.map_wrap.height()) as usize
            || self.nation_scores.len() != players
            || self.spaceships.len() != players
            || self.turn < 1
            || self
                .native_random
                .as_ref()
                .map_or(self.rng == 0, |random| !random.is_initialized())
            || self.climate_current.iter().any(|&count| {
                !(0..=self.map_wrap.width() * self.map_wrap.height()).contains(&count)
            })
            || self.tiles.iter().any(|tile| {
                tile.owner.is_some_and(|owner| owner >= players)
                    || tile.last_seen_owner.is_some_and(|owner| owner >= players)
            })
            || self.units.iter().any(|unit| {
                unit.owner >= players
                    || unit.nationality >= players
                    || !position(unit.x, unit.y)
                    || unit.id >= self.next_unit_id
            })
            || self.cities.iter().any(|city| {
                city.owner >= players
                    || !position(city.x, city.y)
                    || city.size <= 0
                    || city.resisting_citizens.windows(2).any(|pair| pair[0].0 >= pair[1].0)
                    || city.resisting_citizens.iter().any(|&(owner, count)| {
                        owner >= players || owner == city.owner || count <= 0
                    })
                    || city.resisting_citizens.iter().map(|&(_, count)| count).sum::<i32>()
                        > city.size
                    || city.specialists.iter().any(|&count| count < 0)
                    || city.specialists.iter().sum::<i32>()
                        + city.resisting_citizens.iter().map(|&(_, count)| count).sum::<i32>()
                        > city.size
                    || city.id >= self.next_city_id
                    || city
                        .foreign_citizens
                        .iter()
                        .any(|&(owner, count)| owner >= players || count <= 0)
                    || city
                        .culture_by_civilization
                        .windows(2)
                        .any(|pair| pair[0].0 >= pair[1].0)
                    || city
                        .culture_by_civilization
                        .iter()
                        .any(|&(owner, culture)| owner >= players || culture < 0)
                    || city
                        .culture_building_years
                        .windows(2)
                        .any(|pair| pair[0].0 >= pair[1].0)
                    || city.culture_building_years.iter().any(|&(building, year)| {
                        building >= Building::all().count() || year > self.year
                    })
                    || city
                        .culture_building_owners
                        .windows(2)
                        .any(|pair| pair[0].0 >= pair[1].0)
                    || city
                        .culture_building_owners
                        .iter()
                        .any(|&(building, owner)| building >= Building::all().count() || owner >= players)
                    || !production(city.production)
                    || city.worklist.iter().any(|&item| !production(item))
                    || city
                        .production_before_switch
                        .is_some_and(|(item, _)| !production(item))
                    || city.buildings.iter().any(|&item| !building(item))
            })
            || self.completed_wonders.iter().any(|&item| !building(item))
            || self.players.iter().any(|player| {
                !tech(player.researching)
                    || (player.research_goal != Tech::UNSET && !tech(player.research_goal))
                    || player.known.iter().any(|&item| !tech(item))
            })
            || self.diplomacy.keys().any(|&(a, b)| a >= b || b >= players)
        {
            return Err(invalid("Invalid CivCraft campaign state"));
        }
        Ok(())
    }
}

fn decode_current(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (
        mut campaign,
        (current, guard_targets, unit_guards, city_rapture, goto_tiles),
        order,
        (random, cargo),
        observed,
        wrap,
    ): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
        Extras13,
        observed_map::ObservedMap,
        MapWrap,
    ) = codec().deserialize(payload)?;
    campaign.0.climate_current = current;
    campaign.0.ai_guard_targets = guard_targets;
    campaign.0.ai_unit_guards = unit_guards;
    campaign.0.city_rapture = city_rapture;
    campaign.0.unit_goto_tiles = goto_tiles;
    campaign.0.list_order = order;
    campaign.0.native_random = random;
    campaign.0.cargo_order = cargo;
    campaign.0.observed_map = observed;
    campaign.0.map_wrap = wrap;
    Ok(campaign)
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(MAX_PAYLOAD)
        .reject_trailing_bytes()
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(super) mod known_by {
    use super::PLAYER_SLOTS;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

    pub fn serialize<S: Serializer>(
        known: &[bool; PLAYER_SLOTS],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        known.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[bool; PLAYER_SLOTS], D::Error> {
        Vec::<bool>::deserialize(deserializer)?
            .try_into()
            .map_err(|known: Vec<bool>| {
                de::Error::invalid_length(known.len(), &"the campaign player slot count")
            })
    }
}

#[cfg(test)]
#[path = "save/tests.rs"]
mod tests;
