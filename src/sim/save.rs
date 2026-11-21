//! Current campaign persistence, input validation, and runtime restoration.
//!
//! Saves encode the durable part of [`Simulation`] with the current binary
//! format (`CIVCRAFT\0\x33`). Runtime caches are excluded and reconstructed on load.

use super::*;
use bincode::Options;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

mod schema;
use schema::SavedCampaign;

const HEADER: &[u8] = b"CIVCRAFT\0\x33";
const MAX_PAYLOAD: u64 =
    16 * 1024 * 1024 + MapWrap::MAX_TILES * (256 + MAX_CIVILIZATIONS as u64 * 128);

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

impl Simulation {
    pub(crate) fn save_campaign(&self, path: &Path) -> io::Result<()> {
        let payload = self.encode_campaign()?;
        Self::write_campaign_payload(path, &payload)
    }

    /// Write an immutable encoded snapshot; safe to call from a save worker.
    pub(crate) fn write_campaign_payload(path: &Path, payload: &[u8]) -> io::Result<()> {
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
            file.write_all(payload)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    pub(crate) fn encode_campaign(&self) -> io::Result<Vec<u8>> {
        codec(MAX_PAYLOAD)
            .serialize(&SavedCampaign::from_game(self))
            .map_err(io::Error::other)
    }

    pub(crate) fn load_campaign(path: &Path) -> io::Result<Self> {
        let mut file = fs::File::open(path)?;
        let mut header = [0; HEADER.len()];
        file.read_exact(&mut header)?;
        if header.as_slice() != HEADER {
            return Err(invalid("Unsupported CivCraft save format"));
        }
        let mut payload = Vec::new();
        file.take(MAX_PAYLOAD + 1).read_to_end(&mut payload)?;
        if payload.len() as u64 > MAX_PAYLOAD {
            return Err(invalid("CivCraft save exceeds the size limit"));
        }
        let saved: SavedCampaign<'_> = codec(MAX_PAYLOAD)
            .deserialize(&payload)
            .map_err(|error| invalid(error.to_string()))?;
        let mut game = saved.into_game().map_err(invalid)?;

        game.validate_campaign()?;
        if !game.valid_religion() {
            return Err(invalid("Invalid religion state"));
        }
        if !game.valid_vassalage() {
            return Err(invalid("Invalid vassalage state"));
        }
        if game.city_governors.iter().any(|(&id, state)| {
            !state.settings.valid()
                || !game
                    .cities
                    .iter()
                    .any(|city| city.id == id && city.owner == HUMAN)
        }) {
            return Err(invalid("Invalid city governor settings"));
        }
        if !game.valid_render_memory() {
            return Err(invalid("Invalid observed rendering map"));
        }
        game.rebuild_observed_city_index();
        if game
            .ai_purchase_reserve
            .iter()
            .any(|(&owner, &price)| owner >= game.players.len() || price <= 0)
        {
            return Err(invalid("Invalid AI purchase reserve"));
        }
        if game.city_founders.iter().any(|(&city, &owner)| {
            city == 0 || city >= game.next_city_id || owner >= game.players.len()
        }) {
            return Err(invalid("Invalid city founder"));
        }
        if game.ai_war_plans.iter().any(|(&(owner, rival), plan)| {
            owner >= game.players.len()
                || rival >= game.players.len()
                || owner == rival
                || owner == BARBARIANS
                || rival == BARBARIANS
                || plan.objective.is_some_and(|objective| {
                    !(0..game.map_wrap.width()).contains(&objective.staging.0)
                        || !(0..game.map_wrap.height()).contains(&objective.staging.1)
                })
        }) {
            return Err(invalid("Invalid AI war plan"));
        }
        if game
            .ai_space_warnings
            .iter()
            .any(|(&(owner, rival), &timer)| {
                owner >= game.players.len()
                    || rival >= game.players.len()
                    || owner == rival
                    || owner == BARBARIANS
                    || rival == BARBARIANS
                    || timer < 0
            })
        {
            return Err(invalid("Invalid AI space warning"));
        }
        if game
            .ai_free_bulbs
            .keys()
            .any(|&owner| owner >= game.players.len() || owner == BARBARIANS)
        {
            return Err(invalid("Invalid AI free research bulbs"));
        }
        if game.ai_ground_targets.iter().any(|(&unit, &city)| {
            !game.units.iter().any(|actor| actor.id == unit)
                || !game.cities.iter().any(|target| target.id == city)
        }) {
            return Err(invalid("Invalid AI ground mission"));
        }
        if game
            .ai_settlement_targets
            .iter()
            .any(|(&unit, reservation)| {
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
            })
        {
            return Err(invalid("Invalid AI settlement reservation"));
        }
        if !game.valid_relationship_memory() {
            return Err(invalid("Invalid relationship history"));
        }
        if game.ai_diplomatic_memory.keys().any(|&(owner, rival)| {
            owner >= game.players.len()
                || rival >= game.players.len()
                || owner == rival
                || owner == BARBARIANS
                || rival == BARBARIANS
        }) {
            return Err(invalid("Invalid AI diplomatic memory"));
        }
        if game.ai_diplomatic_memory.values().any(|memory| {
            memory.spam < 0
                || memory.asked_peace < 0
                || memory.asked_alliance < 0
                || memory.asked_ceasefire < 0
        }) {
            return Err(invalid("Invalid AI diplomatic timers"));
        }
        if !game.ai_ferries.valid(&game.units) {
            return Err(invalid("Invalid AI ferry registry"));
        }
        if game.ai_government_cache.iter().any(|(&owner, cache)| {
            owner >= game.players.len()
                || cache.evaluated_turn > game.turn
                || cache.next_evaluation < cache.evaluated_turn
                || cache.scores.iter().any(|score| !score.is_finite())
        }) {
            return Err(invalid("Invalid AI government cache"));
        }
        if game
            .diplomatic_contact_until
            .keys()
            .any(|&(a, b)| a >= b || b >= game.players.len() || a == BARBARIANS || b == BARBARIANS)
        {
            return Err(invalid("Invalid diplomatic contact"));
        }
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
        game.unit_goto_tiles.clear();
        game.rebuild_armies();
        game.rebuild_city_ownership();
        game.rebuild_observed_city_index();
        game.refresh_diplomatic_intelligence();
        game.caches.clear();
        Ok(game)
    }

    fn validate_campaign(&self) -> io::Result<()> {
        let players = self.players.len();
        let topology = MapWrap::with_dimensions(self.map_wrap.width(), self.map_wrap.height())
            .map_err(invalid)?;
        let trade_route_settings = crate::rules::engine_rules::RulesetData::campaign_default()
            .trade_route_settings()
            .map_err(|error| invalid(format!("Invalid C2C3 trade-route rules: {error}")))?;
        let position = |x, y| {
            (0..self.map_wrap.width()).contains(&x) && (0..self.map_wrap.height()).contains(&y)
        };
        let tech = |tech: Tech| tech == Tech::FUTURE || tech.data().is_some();
        let building = |building| Building::all().any(|known| known == building);
        let mut trade_route_pairs = std::collections::BTreeSet::new();
        let notification_ids: std::collections::BTreeSet<_> =
            self.notifications.iter().map(|item| item.id).collect();
        let max_notification_id = notification_ids.iter().next_back().copied().unwrap_or(0);
        let production = |item| match item {
            Production::Building(item) => building(item),
            Production::Unit(_) => true,
        };
        if !(PLAYER_COUNT + 1..=PLAYER_SLOTS).contains(&players)
            || self.tiles.len() != topology.tile_count()
            || self.nation_scores.len() != players
            || self.spaceships.len() != players
            || self.turn < 1
            || notification_ids.len() != self.notifications.len()
            || notification_ids.contains(&0)
            || self.next_notification_id <= max_notification_id
            || self.notifications.len() > 500
            || (self.year_0_hack && self.year != 1)
            || self
                .global_advances
                .iter()
                .enumerate()
                .any(|(index, tech)| {
                    tech.data().is_none() || self.global_advances[..index].contains(tech)
                })
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
                let mut seen_culture = Vec::new();
                let invalid_culture = city.culture_buildings.iter().any(|record| {
                    record.builder >= players
                        || record.active_since_year > self.year
                        || !city.buildings.contains(&record.building)
                        || seen_culture.contains(&record.building)
                        || {
                            seen_culture.push(record.building);
                            false
                        }
                });
                city.owner >= players
                    || !position(city.x, city.y)
                    || city.size <= 0
                    || city.history < 0
                    || invalid_culture
                    || city.specialists.iter().any(|&count| count < 0)
                    || city.specialists.iter().sum::<i32>() > city.size
                    || city
                        .foreign_citizens
                        .iter()
                        .map(|&(_, count)| count)
                        .sum::<i32>()
                        > city.size
                    || city.id >= self.next_city_id
                    || city
                        .foreign_citizens
                        .iter()
                        .any(|&(owner, count)| owner >= players || count <= 0)
                    || !production(city.production)
                    || city.worklist.iter().any(|&item| !production(item))
                    || city
                        .production_before_switch
                        .is_some_and(|(item, _)| !production(item))
                    || city.buildings.iter().any(|&item| !building(item))
            })
            || self.trade_routes.iter().any(|route| {
                route.from_city_id == route.to_city_id
                    || self.city_by_id(route.from_city_id).is_none()
                    || self.city_by_id(route.to_city_id).is_none()
                    || route.goods_rule_name.trim().is_empty()
                    || !trade_route_settings
                        .goods
                        .iter()
                        .any(|good| good.id.eq_ignore_ascii_case(&route.goods_rule_name))
                    || !trade_route_pairs.insert(route.unordered_city_pair())
            })
            || self.completed_wonders.iter().any(|&item| !building(item))
            || self.players.iter().any(|player| {
                (player.researching != Tech::UNSET && !tech(player.researching))
                    || (player.research_goal != Tech::UNSET && !tech(player.research_goal))
                    || player.known.iter().any(|&item| !tech(item))
                    || player.history < 0
            })
            || self.diplomacy.keys().any(|&(a, b)| a >= b || b >= players)
            || self
                .diplomacy
                .values()
                .filter_map(|r| r.offer.as_ref())
                .any(|offer| {
                    offer.terms.iter().any(|terms| {
                        !terms.technologies_are_unique()
                            || terms.technologies().any(|technology| !tech(technology))
                    })
                })
        {
            return Err(invalid("Invalid CivCraft campaign state"));
        }
        Ok(())
    }
}

fn codec(max_payload: u64) -> impl Options {
    bincode::DefaultOptions::new()
        .with_limit(max_payload)
        .reject_trailing_bytes()
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "save/tests.rs"]
pub(in crate::sim) mod tests;
