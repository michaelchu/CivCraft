//! Current campaign persistence, input validation, and runtime restoration.
//!
//! Saves encode the durable part of [`Simulation`] with the current versioned
//! binary format. Formats 0x26 through 0x2F are read and upgraded on save.
//!
//! When adding persistent fields, decide whether they belong in the save,
//! update the round-trip and corruption checks. Runtime caches should remain excluded when
//! they can be derived from saved state.

use super::*;
use bincode::Options;
use std::{
    borrow::Cow,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

mod schema;
use schema::{
    LegacyAiMap, LegacyCampaign, SavedCampaign, SavedCampaignV2A, SavedCampaignV2B,
    SavedCampaignV2C, SavedCampaignV2D, SavedCampaignV2F,
};

const HEADER: &[u8] = b"CIVCRAFT\0\x2F";
const V2E_HEADER: &[u8] = b"CIVCRAFT\0\x2E";
const V2D_HEADER: &[u8] = b"CIVCRAFT\0\x2D";
const V2C_HEADER: &[u8] = b"CIVCRAFT\0\x2C";
const V2B_HEADER: &[u8] = b"CIVCRAFT\0\x2B";
const V2A_HEADER: &[u8] = b"CIVCRAFT\0\x2A";
const V29_HEADER: &[u8] = b"CIVCRAFT\0\x29";
const V28_HEADER: &[u8] = b"CIVCRAFT\0\x28";
const PACKED_LEGACY_HEADER: &[u8] = b"CIVCRAFT\0\x27";
const LEGACY_HEADER: &[u8] = b"CIVCRAFT\0\x26";
const LEGACY_MAX_PAYLOAD: u64 = 16 * 1024 * 1024;
// Allow space for terrain and a remembered tile for each active civilization,
// in addition to the campaign-wide state budget.
const MAX_PAYLOAD: u64 =
    LEGACY_MAX_PAYLOAD + MapWrap::MAX_TILES * (256 + MAX_CIVILIZATIONS as u64 * 128);

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
type SavedWarObjectives = Vec<((usize, usize), diplomacy::WarObjective)>;

impl Simulation {
    pub(crate) fn save_campaign(&self, path: &Path) -> io::Result<()> {
        let payload = self.encode_campaign()?;
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

    pub(crate) fn encode_campaign(&self) -> io::Result<Vec<u8>> {
        codec(MAX_PAYLOAD)
            .serialize(&SavedCampaignV2F::from_game(
                self,
                Cow::Borrowed(&self.observed_map.partial_sites),
            ))
            .map_err(io::Error::other)
    }

    pub(crate) fn load_campaign(path: &Path) -> io::Result<Self> {
        let mut file = fs::File::open(path)?;
        let mut header = [0; HEADER.len()];
        file.read_exact(&mut header)?;
        let max_payload = match header.as_slice() {
            HEADER | V2E_HEADER | V2D_HEADER | V2C_HEADER | V2B_HEADER | V2A_HEADER
            | V29_HEADER | V28_HEADER | PACKED_LEGACY_HEADER => MAX_PAYLOAD,
            LEGACY_HEADER => LEGACY_MAX_PAYLOAD,
            _ => return Err(invalid("Unsupported CivCraft save format")),
        };
        let mut payload = Vec::new();
        file.take(max_payload + 1).read_to_end(&mut payload)?;
        if payload.len() as u64 > max_payload {
            return Err(invalid("CivCraft save exceeds the size limit"));
        }
        let (mut game, routes, legacy_ai_map, partial_sites) = match header.as_slice() {
            LEGACY_HEADER => {
                let (legacy, duplicate_wrap): (LegacyCampaign<'_>, MapWrap) = codec(max_payload)
                    .deserialize(&payload)
                    .map_err(|error| invalid(error.to_string()))?;
                if legacy.topology() != duplicate_wrap {
                    return Err(invalid("Conflicting legacy map dimensions"));
                }
                let (game, routes, legacy_ai_map) = legacy.into_game();
                (game, Some(routes), legacy_ai_map, Default::default())
            }
            PACKED_LEGACY_HEADER => {
                let legacy: LegacyCampaign<'_> = codec(max_payload)
                    .deserialize(&payload)
                    .map_err(|error| invalid(error.to_string()))?;
                let (game, routes, legacy_ai_map) = legacy.into_game();
                (game, Some(routes), legacy_ai_map, Default::default())
            }
            V28_HEADER => {
                let saved: SavedCampaign<'_, LegacyAiMap, ()> = codec(max_payload)
                    .deserialize(&payload)
                    .map_err(|error| invalid(error.to_string()))?;
                let (game, legacy_ai_map, ()) = saved.into_game();
                (game, None, legacy_ai_map, Default::default())
            }
            V29_HEADER => {
                let saved: SavedCampaign<'_, (), observed_map::PartialCitySites> =
                    codec(max_payload)
                        .deserialize(&payload)
                        .map_err(|error| invalid(error.to_string()))?;
                let (game, (), partial_sites) = saved.into_game();
                (game, None, LegacyAiMap::default(), partial_sites)
            }
            V2A_HEADER => {
                let saved: SavedCampaignV2A<'_, observed_map::PartialCitySites> =
                    codec(max_payload)
                        .deserialize(&payload)
                        .map_err(|error| invalid(error.to_string()))?;
                let (game, partial_sites) = saved.into_game();
                (game, None, LegacyAiMap::default(), partial_sites)
            }
            V2B_HEADER => {
                let saved: SavedCampaignV2B<'_, observed_map::PartialCitySites> =
                    codec(max_payload)
                        .deserialize(&payload)
                        .map_err(|error| invalid(error.to_string()))?;
                let (game, partial_sites) = saved.into_game().map_err(invalid)?;
                (game, None, LegacyAiMap::default(), partial_sites)
            }
            V2C_HEADER => {
                let saved: SavedCampaignV2C<'_, observed_map::PartialCitySites> =
                    codec(max_payload)
                        .deserialize(&payload)
                        .map_err(|error| invalid(error.to_string()))?;
                let (game, partial_sites) = saved.into_game().map_err(invalid)?;
                (game, None, LegacyAiMap::default(), partial_sites)
            }
            V2E_HEADER => {
                let (saved, objectives): (
                    SavedCampaignV2D<'_, observed_map::PartialCitySites>,
                    SavedWarObjectives,
                ) = codec(max_payload)
                    .deserialize(&payload)
                    .map_err(|error| invalid(error.to_string()))?;
                let (mut game, partial_sites) = saved.into_game().map_err(invalid)?;
                for (pair, objective) in objectives {
                    let Some(plan) = game.ai_war_plans.get_mut(&pair) else {
                        return Err(invalid("War objective has no plan"));
                    };
                    if plan.objective.is_some()
                        || !(0..game.map_wrap.width()).contains(&objective.staging.0)
                        || !(0..game.map_wrap.height()).contains(&objective.staging.1)
                    {
                        return Err(invalid("Invalid war objective"));
                    }
                    plan.objective = Some(objective);
                }
                (game, None, LegacyAiMap::default(), partial_sites)
            }
            V2D_HEADER => {
                let saved: SavedCampaignV2D<'_, observed_map::PartialCitySites> =
                    codec(max_payload)
                        .deserialize(&payload)
                        .map_err(|error| invalid(error.to_string()))?;
                let (game, partial_sites) = saved.into_game().map_err(invalid)?;
                (game, None, LegacyAiMap::default(), partial_sites)
            }
            _ => {
                let saved: SavedCampaignV2F<'_, observed_map::PartialCitySites> =
                    codec(max_payload)
                        .deserialize(&payload)
                        .map_err(|error| invalid(error.to_string()))?;
                let (game, partial_sites) = saved.into_game().map_err(invalid)?;
                (game, None, LegacyAiMap::default(), partial_sites)
            }
        };
        game.initialize_missing_culture_building_ages();
        game.validate_campaign()?;
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
        if partial_sites.iter().any(|(&(observer, index), &owner)| {
            owner >= game.players.len() || !game.observed_map.tiles.contains_key(&(observer, index))
        }) {
            return Err(invalid("Invalid remembered city site"));
        }
        game.observed_map.partial_sites = partial_sites;
        if let Some(routes) = routes {
            if routes.len() != game.tiles.len()
                || routes.iter().flatten().any(|tile| {
                    tile.city_owner
                        .is_some_and(|owner| owner >= game.players.len())
                })
            {
                return Err(invalid("Invalid remembered map"));
            }
            game.import_legacy_routes(routes);
        }
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
        if legacy_ai_map.0.iter().any(|(&(owner, index), tile)| {
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
        for ((observer, index), tile) in legacy_ai_map.0 {
            if game.tiles[index].known_by[observer] {
                game.import_partial_tile(observer, index, tile.route, tile.owner);
            }
        }
        if game.ai_war_plans.keys().any(|&(owner, rival)| {
            owner >= game.players.len()
                || rival >= game.players.len()
                || owner == rival
                || owner == BARBARIANS
                || rival == BARBARIANS
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
        // savegame3.c clears goto_tile after loading units with no orders
        // unless activity is ACTIVITY_GOTO. This engine has no queued native
        // GOTO activity; task destinations are reconstructed by the advisors.
        game.unit_goto_tiles.clear();
        game.rebuild_observed_city_index();
        game.refresh_diplomatic_intelligence();
        // Restoration may query entities while validating and repairing state.
        // Return with reconstructible memoizations cold; remembered knowledge stays intact.
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
        let production = |item| match item {
            Production::Building(item) => building(item),
            Production::Unit(_) => true,
        };
        if !(PLAYER_COUNT + 1..=PLAYER_SLOTS).contains(&players)
            || self.tiles.len() != topology.tile_count()
            || self.nation_scores.len() != players
            || self.spaceships.len() != players
            || self.turn < 1
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
                city.owner >= players
                    || !position(city.x, city.y)
                    || city.size <= 0
                    || city.history < 0
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

pub(super) mod known_by {
    use super::PLAYER_SLOTS;
    use crate::sim::KnownBy;
    use serde::{Deserializer, Serializer, de};
    use std::fmt;

    const PACKED_BYTES: usize = PLAYER_SLOTS.div_ceil(8);

    pub fn serialize<S: Serializer>(known: &KnownBy, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&known.0)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<KnownBy, D::Error> {
        struct Visibility;

        impl de::Visitor<'_> for Visibility {
            type Value = KnownBy;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("packed campaign visibility or the legacy boolean array")
            }

            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
                match bytes.len() {
                    PACKED_BYTES => Ok(KnownBy(
                        bytes.try_into().expect("checked visibility length"),
                    )),
                    // Bincode encoded the old sequence with the same length
                    // prefix and a single 0 or 1 for each player slot.
                    PLAYER_SLOTS if bytes.iter().all(|&byte| byte <= 1) => {
                        Ok(KnownBy::from(std::array::from_fn(|index| {
                            bytes[index] == 1
                        })))
                    }
                    PLAYER_SLOTS => Err(E::custom("invalid legacy visibility value")),
                    length => Err(E::invalid_length(length, &self)),
                }
            }
        }

        deserializer.deserialize_bytes(Visibility)
    }
}

#[cfg(test)]
mod packed_visibility_tests {
    use super::*;
    use crate::sim::KnownBy;
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Record {
        #[serde(with = "super::known_by")]
        known: KnownBy,
    }
    #[test]
    fn visibility_encoding_preserves_packed_bytes_and_legacy_slots() {
        let legacy: Vec<u8> = (0..PLAYER_SLOTS)
            .map(|player| u8::from(player % 7 == 0 || player == PLAYER_SLOTS - 1))
            .collect();
        let loaded: Record = bincode::deserialize(&bincode::serialize(&legacy).unwrap()).unwrap();
        for (player, &flag) in legacy.iter().enumerate() {
            assert_eq!(loaded.known[player], flag == 1);
        }
        let mut packed = vec![0_u8; PLAYER_SLOTS.div_ceil(8)];
        for (player, &flag) in legacy.iter().enumerate() {
            packed[player / 8] |= flag << (player % 8);
        }
        assert_eq!(
            bincode::serialize(&loaded).unwrap(),
            bincode::serialize(&packed).unwrap()
        );
        let roundtrip: Record =
            bincode::deserialize(&bincode::serialize(&loaded).unwrap()).unwrap();
        assert_eq!(roundtrip.known, loaded.known);
        for invalid in [vec![2_u8; PLAYER_SLOTS], vec![0; 63]] {
            assert!(
                bincode::deserialize::<Record>(&bincode::serialize(&invalid).unwrap()).is_err()
            );
        }
    }
}

#[cfg(test)]
#[path = "save/tests.rs"]
pub(in crate::sim) mod tests;
