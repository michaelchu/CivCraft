use super::*;
use bincode::Options;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const HEADER_V1: &[u8] = b"CIVCRAFT\0\x01";
const HEADER_V2: &[u8] = b"CIVCRAFT\0\x02";
const HEADER_V3: &[u8] = b"CIVCRAFT\0\x03";
const HEADER_V4: &[u8] = b"CIVCRAFT\0\x04";
const HEADER: &[u8] = b"CIVCRAFT\0\x05";
const MAX_PAYLOAD: u64 = 16 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

impl Simulation {
    pub(crate) fn save_campaign(&self, path: &Path) -> io::Result<()> {
        let payload = codec()
            .serialize(&(
                self,
                &self.route_memory,
                &self.ai_purchase_reserve,
                &self.agent_city_targets,
                &self.city_founders,
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
        if header != HEADER
            && header != HEADER_V4
            && header != HEADER_V3
            && header != HEADER_V2
            && header != HEADER_V1
        {
            return Err(invalid("Unsupported CivCraft save format"));
        }
        let mut payload = Vec::new();
        file.take(MAX_PAYLOAD + 1).read_to_end(&mut payload)?;
        if payload.len() as u64 > MAX_PAYLOAD {
            return Err(invalid("CivCraft save exceeds the size limit"));
        }
        let mut game: Self;
        if header == HEADER_V1 {
            game = codec()
                .deserialize(&payload)
                .map_err(|error| invalid(error.to_string()))?;
            game.validate_campaign()?;
            game.remember_route_tiles(true);
        } else {
            let decode_error = |error: bincode::Error| invalid(error.to_string());
            let (loaded, memory, reserves, missions, founders) = if header == HEADER_V2 {
                let (loaded, memory) = codec().deserialize(&payload).map_err(decode_error)?;
                (
                    loaded,
                    memory,
                    Default::default(),
                    Default::default(),
                    Default::default(),
                )
            } else if header == HEADER_V3 {
                let (loaded, memory, reserves) =
                    codec().deserialize(&payload).map_err(decode_error)?;
                (
                    loaded,
                    memory,
                    reserves,
                    Default::default(),
                    Default::default(),
                )
            } else if header == HEADER_V4 {
                let (loaded, memory, reserves, missions) =
                    codec().deserialize(&payload).map_err(decode_error)?;
                (loaded, memory, reserves, missions, Default::default())
            } else {
                codec().deserialize(&payload).map_err(decode_error)?
            };
            let founders: std::collections::BTreeMap<u32, usize> = founders;
            let missions: std::collections::BTreeMap<u32, u32> = missions;
            let reserves: std::collections::BTreeMap<usize, i32> = reserves;
            let memory: Vec<Option<movement::RouteTile>> = memory;
            game = loaded;
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
            if missions.iter().any(|(&unit, &city)| {
                unit == 0 || unit >= game.next_unit_id || city == 0 || city >= game.next_city_id
            }) {
                return Err(invalid("Invalid AI city mission"));
            }
            game.ai_purchase_reserve = reserves;
            if founders.iter().any(|(&city, &owner)| {
                city == 0 || city >= game.next_city_id || owner >= game.players.len()
            }) {
                return Err(invalid("Invalid city founder"));
            }
            game.agent_city_targets = missions;
            game.city_founders = founders;
        }
        Ok(game)
    }

    pub(crate) fn save_campaign_playtest(&mut self) -> io::Result<(bool, bool, bool)> {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "civcraft-save-test-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&dir)?;
        let path = dir.join("campaign.save");
        let result = (|| {
            self.create_city(HUMAN, 12, 24);
            self.create_city(1, 18, 32);
            self.advance_diplomacy();
            self.apply(Command::EndTurn);
            self.ai_purchase_reserve.insert(1, 120);
            let agent = self.spawn_unit(1, UnitKind::Diplomat, 18, 32);
            let target = self
                .cities
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap()
                .id;
            self.agent_city_targets.insert(agent, target);
            self.save_campaign(&path)?;
            let mut resumed = Self::load_campaign(&path)?;
            let bytes = |sim: &Self| {
                codec()
                    .serialize(&(
                        sim,
                        &sim.route_memory,
                        &sim.ai_purchase_reserve,
                        &sim.agent_city_targets,
                        &sim.city_founders,
                    ))
                    .map_err(io::Error::other)
            };
            let restored = bytes(self)? == bytes(&resumed)?;
            self.apply(Command::EndTurn);
            resumed.apply(Command::EndTurn);
            let deterministic = bytes(self)? == bytes(&resumed)?;
            self.save_campaign(&path)?;
            let replaced = Self::load_campaign(&path)?.turn == self.turn;
            let mut legacy = HEADER_V1.to_vec();
            codec()
                .serialize_into(&mut legacy, self)
                .map_err(io::Error::other)?;
            fs::write(&path, legacy)?;
            let migrated = Self::load_campaign(&path)?.route_memory.len() == self.tiles.len();
            let mut previous = HEADER_V2.to_vec();
            codec()
                .serialize_into(&mut previous, &(&*self, &self.route_memory))
                .map_err(io::Error::other)?;
            fs::write(&path, previous)?;
            let migrated_v2 = Self::load_campaign(&path)?.ai_purchase_reserve.is_empty();
            let mut previous = HEADER_V3.to_vec();
            codec()
                .serialize_into(
                    &mut previous,
                    &(&*self, &self.route_memory, &self.ai_purchase_reserve),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, previous)?;
            let migrated_v3 = Self::load_campaign(&path)?.agent_city_targets.is_empty();
            let mut previous = HEADER_V4.to_vec();
            codec()
                .serialize_into(
                    &mut previous,
                    &(
                        &*self,
                        &self.route_memory,
                        &self.ai_purchase_reserve,
                        &self.agent_city_targets,
                    ),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, previous)?;
            let migrated_v4 = Self::load_campaign(&path)?.city_founders.is_empty();
            fs::write(&path, b"CIVCRAFT\0\xffbad save")?;
            let rejected = Self::load_campaign(&path).is_err();
            Ok((
                restored,
                deterministic,
                replaced && migrated && migrated_v2 && migrated_v3 && migrated_v4 && rejected,
            ))
        })();
        let cleanup = fs::remove_dir_all(dir);
        result.and_then(|outcome| cleanup.map(|()| outcome))
    }

    fn validate_campaign(&self) -> io::Result<()> {
        let players = self.players.len();
        let position = |x, y| (0..MAP_W).contains(&x) && (0..MAP_H).contains(&y);
        let tech = |tech: Tech| tech == Tech::FUTURE || tech.data().is_some();
        let building = |building| Building::all().any(|known| known == building);
        let production = |item| match item {
            Production::Building(item) => building(item),
            Production::Unit(_) => true,
        };
        if !(PLAYER_COUNT + 1..=PLAYER_SLOTS).contains(&players)
            || self.tiles.len() != (MAP_W * MAP_H) as usize
            || self.nation_scores.len() != players
            || self.spaceships.len() != players
            || self.turn < 1
            || self.rng == 0
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
            || self.completed_wonders.iter().any(|&item| !building(item))
            || self.players.iter().any(|player| {
                !tech(player.researching)
                    || !tech(player.research_goal)
                    || player.known.iter().any(|&item| !tech(item))
            })
            || self.diplomacy.keys().any(|&(a, b)| a >= b || b >= players)
        {
            return Err(invalid("Invalid CivCraft campaign state"));
        }
        Ok(())
    }
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
