use super::*;
use bincode::Options;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

// Preserve the existing map wire format while making save/replay comparisons
// independent of each HashMap's randomized bucket order.
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

const HEADER_V1: &[u8] = b"CIVCRAFT\0\x01";
const HEADER_V2: &[u8] = b"CIVCRAFT\0\x02";
const HEADER_V3: &[u8] = b"CIVCRAFT\0\x03";
const HEADER_V4: &[u8] = b"CIVCRAFT\0\x04";
const HEADER_V5: &[u8] = b"CIVCRAFT\0\x05";
const HEADER_V6: &[u8] = b"CIVCRAFT\0\x06";
const HEADER_V7: &[u8] = b"CIVCRAFT\0\x07";
const HEADER_V8: &[u8] = b"CIVCRAFT\0\x08";
const HEADER_V9: &[u8] = b"CIVCRAFT\0\x09";
const HEADER_V10: &[u8] = b"CIVCRAFT\0\x0a";
const HEADER_V11: &[u8] = b"CIVCRAFT\0\x0b";
const HEADER_V12: &[u8] = b"CIVCRAFT\0\x0c";
const HEADER_V13: &[u8] = b"CIVCRAFT\0\x0d";
const HEADER_V14: &[u8] = b"CIVCRAFT\0\x0e";
const HEADER: &[u8] = b"CIVCRAFT\0\x10";
const MAX_PAYLOAD: u64 = 16 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

type PairMemory<T> = std::collections::BTreeMap<(usize, usize), T>;
type UnitTargets = std::collections::BTreeMap<u32, u32>;
type LegacyStrategy7 = (
    PairMemory<diplomacy::WarPlan>,
    PairMemory<i32>,
    std::collections::BTreeMap<usize, i32>,
    UnitTargets,
);
type LegacyStrategy7Early = (
    PairMemory<diplomacy::WarPlan>,
    PairMemory<i32>,
    std::collections::BTreeMap<usize, i32>,
);
type Strategy8 = (
    PairMemory<diplomacy::WarPlan>,
    PairMemory<i32>,
    std::collections::BTreeMap<usize, i32>,
    UnitTargets,
    std::collections::BTreeMap<u32, trade::MerchantMission>,
    std::collections::BTreeMap<u32, settlement::SettlementReservation>,
    PairMemory<diplomacy::DiplomaticMemory>,
    std::collections::BTreeMap<u32, i32>,
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
    UnitTargets,
    std::collections::BTreeMap<u32, usize>,
    AiMapMemory,
    A,
);

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
                    &self.agent_city_targets,
                    &self.city_founders,
                    &self.ai_map_memory,
                    (
                        (
                            &self.ai_war_plans,
                            &self.ai_space_warnings,
                            &self.ai_free_bulbs,
                            &self.ai_ground_targets,
                            &self.ai_merchant_missions,
                            &self.ai_settlement_targets,
                            &self.ai_diplomatic_memory,
                            &self.merchant_birth_turns,
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
        if header != HEADER
            && header != HEADER_V14
            && header != HEADER_V13
            && header != HEADER_V12
            && header != HEADER_V11
            && header != HEADER_V10
            && header != HEADER_V9
            && header != HEADER_V8
            && header != HEADER_V7
            && header != HEADER_V6
            && header != HEADER_V5
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
            let (loaded, memory, reserves, missions, founders, observations, ai_strategy) =
                match header.as_slice() {
                    HEADER_V2 => {
                        let (loaded, memory) =
                            codec().deserialize(&payload).map_err(decode_error)?;
                        (
                            loaded,
                            memory,
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )
                    }
                    HEADER_V3 => {
                        let (loaded, memory, reserves) =
                            codec().deserialize(&payload).map_err(decode_error)?;
                        (
                            loaded,
                            memory,
                            reserves,
                            Default::default(),
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )
                    }
                    HEADER_V4 => {
                        let (loaded, memory, reserves, missions) =
                            codec().deserialize(&payload).map_err(decode_error)?;
                        (
                            loaded,
                            memory,
                            reserves,
                            missions,
                            Default::default(),
                            Default::default(),
                            Default::default(),
                        )
                    }
                    HEADER_V5 => {
                        let (loaded, memory, reserves, missions, founders) =
                            codec().deserialize(&payload).map_err(decode_error)?;
                        (
                            loaded,
                            memory,
                            reserves,
                            missions,
                            founders,
                            Default::default(),
                            Default::default(),
                        )
                    }
                    HEADER_V6 => {
                        let (loaded, memory, reserves, missions, founders, observations) =
                            codec().deserialize(&payload).map_err(decode_error)?;
                        (
                            loaded,
                            memory,
                            reserves,
                            missions,
                            founders,
                            observations,
                            Default::default(),
                        )
                    }
                    HEADER_V7 => upgrade_strategy(decode_v7(&payload).map_err(decode_error)?),
                    HEADER_V8 => upgrade_strategy(
                        codec()
                            .deserialize::<CampaignPayload<Strategy8>>(&payload)
                            .map_err(decode_error)?,
                    ),
                    HEADER_V9 => codec().deserialize(&payload).map_err(decode_error)?,
                    HEADER_V10 => decode_v10(&payload).map_err(decode_error)?,
                    HEADER_V11 => decode_v11(&payload).map_err(decode_error)?,
                    HEADER_V12 => decode_v12(&payload).map_err(decode_error)?,
                    HEADER_V13 => decode_v13(&payload).map_err(decode_error)?,
                    HEADER_V14 => decode_v14(&payload).map_err(decode_error)?,
                    _ => decode_v15(&payload).map_err(decode_error)?,
                };
            let (
                (
                    war_plans,
                    space_warnings,
                    free_bulbs,
                    ground_targets,
                    merchant_missions,
                    settlement_targets,
                    diplomatic_memory,
                    merchant_birth_turns,
                ),
                (ferries, government_cache, contact_until),
            ): Strategy9 = ai_strategy;
            let observations: AiMapMemory = observations;
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
            if merchant_missions.iter().any(|(&unit, mission)| {
                !game.units.iter().any(|actor| {
                    actor.id == unit
                        && matches!(actor.kind, UnitKind::Caravan | UnitKind::Freight)
                        && actor.owner == mission.owner
                }) || !game.cities.iter().any(|city| city.id == mission.city)
            }) {
                return Err(invalid("Invalid AI merchant mission"));
            }
            game.ai_merchant_missions = merchant_missions;
            if settlement_targets.iter().any(|(&unit, reservation)| {
                let (x, y) = reservation.destination;
                !game.units.iter().any(|actor| {
                    actor.id == unit
                        && actor.kind == UnitKind::Settlers
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
            if merchant_birth_turns.iter().any(|(&id, &born)| {
                born > game.turn
                    || !game.units.iter().any(|unit| {
                        unit.id == id && matches!(unit.kind, UnitKind::Caravan | UnitKind::Freight)
                    })
            }) {
                return Err(invalid("Invalid merchant birth turn"));
            }
            game.merchant_birth_turns = merchant_birth_turns;
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
            if contact_until.keys().any(|&(a, b)| {
                a >= b || b >= game.players.len() || a == BARBARIANS || b == BARBARIANS
            }) {
                return Err(invalid("Invalid diplomatic contact"));
            }
            game.ai_ferries = ferries;
            game.ai_government_cache = government_cache;
            game.diplomatic_contact_until = contact_until;
        }
        if matches!(
            header.as_slice(),
            HEADER | HEADER_V14 | HEADER_V13 | HEADER_V12
        ) && !game.list_order.valid(&game.units, &game.cities)
        {
            return Err(invalid("Invalid native entity list order"));
        }
        if !matches!(
            header.as_slice(),
            HEADER | HEADER_V14 | HEADER_V13 | HEADER_V12
        ) {
            // Legacy saves did not record arrival/transfer history. Infer
            // creation order; exact historical tile order is unrecoverable.
            game.sync_list_order();
        }
        if matches!(header.as_slice(), HEADER | HEADER_V14 | HEADER_V13) {
            if !game.valid_cargo_order() {
                return Err(invalid("Invalid transport cargo order"));
            }
        } else {
            game.sync_cargo_order();
        }
        if !matches!(
            header.as_slice(),
            HEADER | HEADER_V14 | HEADER_V13 | HEADER_V12 | HEADER_V11 | HEADER_V10 | HEADER_V9
        ) {
            game.seed_legacy_diplomatic_contacts();
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
        // Preserve the save layout and custom city names. Only migrate the six
        // exact former faction labels, leaving rebels and custom names alone.
        for (player, civ) in game
            .players
            .iter_mut()
            .zip(crate::rules::civilizations::CIVILIZATIONS.iter())
        {
            if player.name == civ.legacy_name {
                player.name = civ.name.to_owned();
            }
        }
        if header != HEADER && header != HEADER_V14 {
            game.migrate_render_memory();
        }
        if !game.valid_render_memory() {
            return Err(invalid("Invalid observed rendering map"));
        }
        game.refresh_diplomatic_intelligence();
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
            self.unit_goto_tiles.insert(agent, (12, 24));
            let guard = self.spawn_unit(1, UnitKind::Phalanx, 18, 32);
            self.ai_guard_targets
                .insert(guard, guards::GuardTarget::Unit(agent));
            self.ai_unit_guards
                .insert(agent, guards::GuardState::Assigned(guard));
            let city_guard = self.spawn_unit(1, UnitKind::Musketeers, 18, 32);
            let home = self.cities.iter().find(|city| city.owner == 1).unwrap().id;
            self.ai_guard_targets
                .insert(city_guard, guards::GuardTarget::City(home));
            self.ai_war_plans.insert(
                (1, HUMAN),
                diplomacy::WarPlan {
                    countdown: -3,
                    reason: diplomacy::WarReason::Excuse,
                },
            );
            self.ai_space_warnings.insert((1, HUMAN), 7);
            self.ai_free_bulbs.insert(1, 3);
            let infantry = self.spawn_unit(1, UnitKind::Warrior, 18, 32);
            self.ai_ground_targets.insert(infantry, target);
            let founder = self.spawn_unit(1, UnitKind::Settlers, 18, 32);
            self.ai_settlement_targets.insert(
                founder,
                settlement::SettlementReservation {
                    owner: 1,
                    destination: (22, 40),
                    worker: Some((22, 42)),
                },
            );
            let merchant = self.spawn_unit(1, UnitKind::Freight, 18, 32);
            self.units.last_mut().unwrap().home_city = self
                .cities
                .iter()
                .find(|city| city.owner == 1)
                .map(|city| city.id);
            self.ai_merchant_missions.insert(
                merchant,
                trade::MerchantMission {
                    owner: 1,
                    city: target,
                    wonder: false,
                },
            );
            self.ai_diplomatic_memory.insert(
                (1, HUMAN),
                diplomacy::DiplomaticMemory {
                    spam: 3,
                    asked_peace: 4,
                    asked_alliance: 5,
                    asked_ceasefire: 6,
                    strongest_pact: Pact::Peace,
                },
            );
            self.tile_mut(19, 33).terrain = Terrain::Ocean;
            let boat = self.spawn_unit(1, UnitKind::Transport, 19, 33);
            self.assign_ai_ferry(boat, merchant);
            self.ai_government_cache.insert(
                1,
                government::GovernmentAdvisorCache {
                    evaluated_turn: self.turn,
                    next_evaluation: self.turn + 5,
                    scores: [1.0; 9],
                },
            );
            self.diplomatic_contact_until
                .insert(Self::relation_key(1, HUMAN), self.turn + 20);
            self.climate_current = [7, 4];
            let celebrating = self
                .cities
                .iter_mut()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            celebrating.size = 3;
            celebrating.was_happy = true;
            self.city_rapture.insert(celebrating.id, 4);
            self.save_campaign(&path)?;
            let mut resumed = Self::load_campaign(&path)?;
            let bytes = |sim: &Self| {
                let mut order = sim.list_order.clone();
                order.synchronize(&sim.units, &sim.cities);
                let cargo_order = sim.cargo_order_snapshot();
                codec()
                    .serialize(&(
                        (
                            sim,
                            &sim.route_memory,
                            &sim.ai_purchase_reserve,
                            &sim.agent_city_targets,
                            &sim.city_founders,
                            &sim.ai_map_memory,
                            (
                                (
                                    &sim.ai_war_plans,
                                    &sim.ai_space_warnings,
                                    &sim.ai_free_bulbs,
                                    &sim.ai_ground_targets,
                                    &sim.ai_merchant_missions,
                                    &sim.ai_settlement_targets,
                                    &sim.ai_diplomatic_memory,
                                    &sim.merchant_birth_turns,
                                ),
                                (
                                    &sim.ai_ferries,
                                    &sim.ai_government_cache,
                                    &sim.diplomatic_contact_until,
                                ),
                            ),
                        ),
                        (
                            &sim.climate_current,
                            &sim.ai_guard_targets,
                            &sim.ai_unit_guards,
                            &sim.city_rapture,
                            &sim.unit_goto_tiles,
                        ),
                        &order,
                        (&sim.native_random, &cargo_order),
                    ))
                    .map_err(io::Error::other)
            };
            let mut expected = self.clone();
            expected.unit_goto_tiles.clear();
            let restored = bytes(&expected)? == bytes(&resumed)?
                && resumed.unit_goto_tiles.is_empty()
                && resumed.ai_worker_tasks.is_empty();
            let mut replay = Self::load_campaign(&path)?;
            replay.apply(Command::EndTurn);
            resumed.apply(Command::EndTurn);
            let deterministic = bytes(&replay)? == bytes(&resumed)?;
            *self = resumed;
            self.save_campaign(&path)?;
            let replaced = Self::load_campaign(&path)?.turn == self.turn;
            let current_file = fs::read(&path)?;
            let (mut previous_state, extra, order, _, _, _): (
                CampaignPayload<Strategy9>,
                Extras11,
                list_order::ListOrder,
                Extras13,
                observed_map::ObservedMap,
                MapWrap,
            ) = codec()
                .deserialize(&current_file[HEADER.len()..])
                .map_err(io::Error::other)?;
            let mut previous_v12 = HEADER_V12.to_vec();
            codec()
                .serialize_into(&mut previous_v12, &(&previous_state, &extra, &order))
                .map_err(io::Error::other)?;
            fs::write(&path, previous_v12)?;
            let loaded_v12 = Self::load_campaign(&path)?;
            let migrated_v12 = loaded_v12.native_random.is_none()
                && loaded_v12.rng == self.rng
                && loaded_v12.list_order == order;
            let mut previous_v11 = HEADER_V11.to_vec();
            codec()
                .serialize_into(&mut previous_v11, &(&previous_state, &extra))
                .map_err(io::Error::other)?;
            fs::write(&path, previous_v11)?;
            let loaded_v11 = Self::load_campaign(&path)?;
            let migrated_v11 = loaded_v11
                .list_order
                .valid(&loaded_v11.units, &loaded_v11.cities)
                && loaded_v11.diplomatic_contact_until == self.diplomatic_contact_until
                && loaded_v11.ai_guard_targets == self.ai_guard_targets;
            let mut previous_v10 = HEADER_V10.to_vec();
            codec()
                .serialize_into(&mut previous_v10, &(&previous_state, extra.0))
                .map_err(io::Error::other)?;
            fs::write(&path, previous_v10)?;
            let loaded_v10 = Self::load_campaign(&path)?;
            let migrated_v10 = loaded_v10.climate_current == self.climate_current
                && loaded_v10.diplomatic_contact_until == self.diplomatic_contact_until
                && loaded_v10.ai_guard_targets.is_empty()
                && loaded_v10.ai_unit_guards.is_empty()
                && loaded_v10.city_rapture.is_empty()
                && loaded_v10.unit_goto_tiles.is_empty();
            let expired = self.turn - 1;
            previous_state.6.1.2.insert((HUMAN, 1), expired);
            let mut previous_v9 = HEADER_V9.to_vec();
            codec()
                .serialize_into(&mut previous_v9, &previous_state)
                .map_err(io::Error::other)?;
            fs::write(&path, previous_v9)?;
            let loaded_v9 = Self::load_campaign(&path)?;
            let migrated_v9 = loaded_v9.climate_current == [0; 2]
                && loaded_v9.diplomatic_contact_until.get(&(HUMAN, 1)) == Some(&expired);
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
            let mut previous = HEADER_V5.to_vec();
            codec()
                .serialize_into(
                    &mut previous,
                    &(
                        &*self,
                        &self.route_memory,
                        &self.ai_purchase_reserve,
                        &self.agent_city_targets,
                        &self.city_founders,
                    ),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, previous)?;
            let migrated_v5 = Self::load_campaign(&path)?.ai_map_memory.is_empty();
            let mut previous = HEADER_V6.to_vec();
            codec()
                .serialize_into(
                    &mut previous,
                    &(
                        &*self,
                        &self.route_memory,
                        &self.ai_purchase_reserve,
                        &self.agent_city_targets,
                        &self.city_founders,
                        &self.ai_map_memory,
                    ),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, previous)?;
            let migrated_game = Self::load_campaign(&path)?;
            let migrated_v6 = migrated_game.ai_war_plans.is_empty()
                && migrated_game.ai_space_warnings.is_empty()
                && migrated_game.ai_free_bulbs.is_empty()
                && migrated_game.ai_ground_targets.is_empty()
                && migrated_game.ai_merchant_missions.is_empty()
                && migrated_game.ai_settlement_targets.is_empty()
                && migrated_game.ai_diplomatic_memory.is_empty()
                && migrated_game.merchant_birth_turns.is_empty();
            let mut previous_v8 = HEADER_V8.to_vec();
            codec()
                .serialize_into(
                    &mut previous_v8,
                    &(
                        &*self,
                        &self.route_memory,
                        &self.ai_purchase_reserve,
                        &self.agent_city_targets,
                        &self.city_founders,
                        &self.ai_map_memory,
                        (
                            &self.ai_war_plans,
                            &self.ai_space_warnings,
                            &self.ai_free_bulbs,
                            &self.ai_ground_targets,
                            &self.ai_merchant_missions,
                            &self.ai_settlement_targets,
                            &self.ai_diplomatic_memory,
                            &self.merchant_birth_turns,
                        ),
                    ),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, previous_v8)?;
            let loaded_v8 = Self::load_campaign(&path)?;
            let migrated_v8 = loaded_v8.ai_government_cache.is_empty()
                && loaded_v8.ai_ferries.boats.is_empty()
                && loaded_v8.ai_merchant_missions.len() == self.ai_merchant_missions.len();
            let mut previous = HEADER_V7.to_vec();
            codec()
                .serialize_into(
                    &mut previous,
                    &(
                        &*self,
                        &self.route_memory,
                        &self.ai_purchase_reserve,
                        &self.agent_city_targets,
                        &self.city_founders,
                        &self.ai_map_memory,
                        (
                            &self.ai_war_plans,
                            &self.ai_space_warnings,
                            &self.ai_free_bulbs,
                            &self.ai_ground_targets,
                        ),
                    ),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, previous)?;
            let loaded_v7 = Self::load_campaign(&path)?;
            let migrated_v7 = loaded_v7.ai_merchant_missions.is_empty()
                && loaded_v7.ai_settlement_targets.is_empty()
                && loaded_v7.ai_diplomatic_memory.is_empty()
                && loaded_v7.ai_ground_targets == self.ai_ground_targets;
            let mut early = HEADER_V7.to_vec();
            codec()
                .serialize_into(
                    &mut early,
                    &(
                        &*self,
                        &self.route_memory,
                        &self.ai_purchase_reserve,
                        &self.agent_city_targets,
                        &self.city_founders,
                        &self.ai_map_memory,
                        (
                            &self.ai_war_plans,
                            &self.ai_space_warnings,
                            &self.ai_free_bulbs,
                        ),
                    ),
                )
                .map_err(io::Error::other)?;
            fs::write(&path, early)?;
            let loaded_early = Self::load_campaign(&path)?;
            let migrated_v7_early = loaded_early.ai_ground_targets.is_empty()
                && loaded_early.ai_space_warnings == self.ai_space_warnings
                && loaded_early.ai_free_bulbs == self.ai_free_bulbs;
            fs::write(&path, b"CIVCRAFT\0\xffbad save")?;
            let rejected = Self::load_campaign(&path).is_err();
            Ok((
                restored,
                deterministic,
                replaced
                    && migrated
                    && migrated_v2
                    && migrated_v3
                    && migrated_v4
                    && migrated_v5
                    && migrated_v6
                    && migrated_v7
                    && migrated_v7_early
                    && migrated_v8
                    && migrated_v9
                    && migrated_v10
                    && migrated_v11
                    && migrated_v12
                    && rejected,
            ))
        })();
        let cleanup = fs::remove_dir_all(dir);
        result.and_then(|outcome| cleanup.map(|()| outcome))
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

fn decode_v11(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (mut campaign, (current, guard_targets, unit_guards, city_rapture, goto_tiles)): (
        CampaignPayload<Strategy9>,
        Extras11,
    ) = codec().deserialize(payload)?;
    campaign.0.climate_current = current;
    campaign.0.ai_guard_targets = guard_targets;
    campaign.0.ai_unit_guards = unit_guards;
    campaign.0.city_rapture = city_rapture;
    campaign.0.unit_goto_tiles = goto_tiles;
    Ok(campaign)
}

fn decode_v12(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (mut campaign, (current, guard_targets, unit_guards, city_rapture, goto_tiles), order): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
    ) = codec().deserialize(payload)?;
    campaign.0.climate_current = current;
    campaign.0.ai_guard_targets = guard_targets;
    campaign.0.ai_unit_guards = unit_guards;
    campaign.0.city_rapture = city_rapture;
    campaign.0.unit_goto_tiles = goto_tiles;
    campaign.0.list_order = order;
    Ok(campaign)
}

fn decode_v13(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (
        mut campaign,
        (current, guard_targets, unit_guards, city_rapture, goto_tiles),
        order,
        (random, cargo),
    ): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
        Extras13,
    ) = codec().deserialize(payload)?;
    campaign.0.climate_current = current;
    campaign.0.ai_guard_targets = guard_targets;
    campaign.0.ai_unit_guards = unit_guards;
    campaign.0.city_rapture = city_rapture;
    campaign.0.unit_goto_tiles = goto_tiles;
    campaign.0.list_order = order;
    campaign.0.native_random = random;
    campaign.0.cargo_order = cargo;
    Ok(campaign)
}

fn decode_v14(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (
        mut campaign,
        (current, guard_targets, unit_guards, city_rapture, goto_tiles),
        order,
        (random, cargo),
        observed,
    ): (
        CampaignPayload<Strategy9>,
        Extras11,
        list_order::ListOrder,
        Extras13,
        observed_map::ObservedMap,
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
    Ok(campaign)
}

fn decode_v15(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
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

fn decode_v10(payload: &[u8]) -> Result<CampaignPayload<Strategy9>, bincode::Error> {
    let (mut campaign, current): (CampaignPayload<Strategy9>, [i32; 2]) =
        codec().deserialize(payload)?;
    campaign.0.climate_current = current;
    Ok(campaign)
}

fn upgrade_strategy(payload: CampaignPayload<Strategy8>) -> CampaignPayload<Strategy9> {
    let (game, routes, reserves, missions, founders, observations, strategy) = payload;
    (
        game,
        routes,
        reserves,
        missions,
        founders,
        observations,
        (strategy, Default::default()),
    )
}

fn decode_v7(payload: &[u8]) -> Result<CampaignPayload<Strategy8>, bincode::Error> {
    // Development V7 files existed with three, then four, strategy maps.
    // Accept the expanded development layout too; V8 fixes the schema going forward.
    if let Ok(current) = codec().deserialize::<CampaignPayload<Strategy8>>(payload) {
        return Ok(current);
    }
    if let Ok((
        game,
        routes,
        reserves,
        missions,
        founders,
        observations,
        (war, warnings, bulbs, ground),
    )) = codec().deserialize::<CampaignPayload<LegacyStrategy7>>(payload)
    {
        return Ok((
            game,
            routes,
            reserves,
            missions,
            founders,
            observations,
            (
                war,
                warnings,
                bulbs,
                ground,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        ));
    }
    let (game, routes, reserves, missions, founders, observations, (war, warnings, bulbs)) =
        codec().deserialize::<CampaignPayload<LegacyStrategy7Early>>(payload)?;
    Ok((
        game,
        routes,
        reserves,
        missions,
        founders,
        observations,
        (
            war,
            warnings,
            bulbs,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ),
    ))
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
mod ai_state_tests {
    use super::*;

    #[test]
    fn old_faction_labels_migrate_without_renaming_existing_cities_or_changing_rng() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-roster-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for (player, civ) in game
            .players
            .iter_mut()
            .zip(crate::rules::civilizations::CIVILIZATIONS.iter())
        {
            player.name = civ.legacy_name.to_owned();
        }
        game.create_city(HUMAN, 10, 20);
        game.cities[0].name = "My Custom Capital".to_owned();
        let random = game.random_snapshot();
        game.save_campaign(&path).unwrap();
        let loaded = Simulation::load_campaign(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        for (player, civ) in loaded
            .players
            .iter()
            .zip(crate::rules::civilizations::CIVILIZATIONS.iter())
        {
            assert_eq!(player.name, civ.name);
            assert_eq!(player.leader_name(), civ.leader);
        }
        assert_eq!(loaded.cities[0].name, "My Custom Capital");
        assert_eq!(loaded.random_snapshot(), random);
    }

    #[test]
    fn v13_preserves_native_rng_and_cargo_and_legacy_campaign_streams() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-rng-cargo-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.native_cargo_order_playtest());
        game.set_random_seed(42);
        for bound in [2, 3, i32::MAX, 100, 1000] {
            game.rand_below(bound);
        }
        game.save_campaign(&path).unwrap();
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        assert_eq!(game.random_snapshot(), loaded.random_snapshot());
        assert_eq!(game.cargo_order, loaded.cargo_order);
        for bound in [0, 1, 1000, 3, i32::MAX, 2] {
            assert_eq!(game.rand_below(bound), loaded.rand_below(bound));
        }
        let data = fs::read(&path).unwrap();
        let mut payload: (
            CampaignPayload<Strategy9>,
            Extras11,
            list_order::ListOrder,
            Extras13,
            observed_map::ObservedMap,
            MapWrap,
        ) = codec().deserialize(&data[HEADER.len()..]).unwrap();
        // V13 remains readable, including its full native RNG and cargo state.
        let saved_random = Simulation::load_campaign(&path).unwrap().random_snapshot();
        let mut previous_v13 = HEADER_V13.to_vec();
        codec()
            .serialize_into(
                &mut previous_v13,
                &(&payload.0, &payload.1, &payload.2, &payload.3),
            )
            .unwrap();
        fs::write(&path, previous_v13).unwrap();
        let migrated_v13 = Simulation::load_campaign(&path).unwrap();
        assert_eq!(migrated_v13.random_snapshot(), saved_random);
        assert_eq!(migrated_v13.cargo_order, loaded.cargo_order);
        assert!(migrated_v13.valid_render_memory());
        let valid_cargo = payload.3.1.clone();
        let ids = payload.3.1.values_mut().next().unwrap();
        ids.push(ids[0]);
        let mut corrupt = HEADER.to_vec();
        codec().serialize_into(&mut corrupt, &payload).unwrap();
        fs::write(&path, corrupt).unwrap();
        assert!(
            Simulation::load_campaign(&path)
                .err()
                .unwrap()
                .to_string()
                .contains("cargo order")
        );
        payload.3.1 = valid_cargo;
        payload.3.0 = Some(FreecivRng::default());
        let mut corrupt = HEADER.to_vec();
        codec().serialize_into(&mut corrupt, &payload).unwrap();
        fs::write(&path, corrupt).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        payload.0.0.rng = 1;
        let mut legacy = HEADER_V12.to_vec();
        codec()
            .serialize_into(&mut legacy, &(payload.0, payload.1, payload.2))
            .unwrap();
        fs::write(&path, legacy).unwrap();
        let mut loaded = Simulation::load_campaign(&path).unwrap();
        assert!(loaded.native_random.is_none());
        assert_eq!(loaded.rand_below(100), 61); // Frozen scalar generator's first result.
        let state = loaded.random_snapshot();
        loaded.save_campaign(&path).unwrap();
        let mut upgraded = Simulation::load_campaign(&path).unwrap();
        assert_eq!(state, upgraded.random_snapshot());
        assert_eq!(loaded.rand_below(1000), upgraded.rand_below(1000));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn v12_preserves_independent_orders_and_rejects_invalid_stamps() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-list-order-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.native_list_order_playtest(), (true, true, true));
        game.sync_list_order();
        game.save_campaign(&path).unwrap();
        let loaded = Simulation::load_campaign(&path).unwrap();
        assert_eq!(loaded.list_order, game.list_order);
        assert_eq!(
            loaded
                .player_units_ordered(1)
                .map(|u| u.id)
                .collect::<Vec<_>>(),
            game.player_units_ordered(1)
                .map(|u| u.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            loaded
                .tile_units_ordered(10, 24)
                .map(|u| u.id)
                .collect::<Vec<_>>(),
            game.tile_units_ordered(10, 24)
                .map(|u| u.id)
                .collect::<Vec<_>>()
        );
        let data = fs::read(&path).unwrap();
        let mut payload: (
            CampaignPayload<Strategy9>,
            Extras11,
            list_order::ListOrder,
            Extras13,
            observed_map::ObservedMap,
            MapWrap,
        ) = codec().deserialize(&data[HEADER.len()..]).unwrap();
        payload.2.next = 0;
        let mut data = HEADER.to_vec();
        codec().serialize_into(&mut data, &payload).unwrap();
        fs::write(&path, data).unwrap();
        assert!(
            Simulation::load_campaign(&path)
                .err()
                .unwrap()
                .to_string()
                .contains("list order")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn v11_rejects_nonreciprocal_guards_and_unknown_city_celebration_history() {
        let path = std::env::temp_dir().join(format!(
            "civcraft-invalid-guard-history-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let guard = game.spawn_unit(1, UnitKind::Phalanx, 10, 24);
        let charge = game.spawn_unit(1, UnitKind::Settlers, 10, 24);
        game.ai_guard_targets
            .insert(guard, guards::GuardTarget::Unit(charge));
        game.save_campaign(&path).unwrap();
        let error = Simulation::load_campaign(&path)
            .err()
            .expect("one-sided guard link accepted");
        assert!(error.to_string().contains("bodyguard"));
        game.ai_unit_guards
            .insert(charge, guards::GuardState::Assigned(guard));
        game.city_rapture.insert(u32::MAX, 3);
        game.save_campaign(&path).unwrap();
        let error = Simulation::load_campaign(&path)
            .err()
            .expect("unknown city's history accepted");
        assert!(error.to_string().contains("celebration"));
        game.city_rapture.clear();
        game.create_city(HUMAN, 10, 24);
        game.city_rapture.insert(game.cities.last().unwrap().id, -1);
        game.save_campaign(&path).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_cached_environment_count_is_rejected_on_load() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.climate_current = [MAP_W * MAP_H + 1, 0];
        let path = std::env::temp_dir().join(format!(
            "civcraft-invalid-climate-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        game.save_campaign(&path).unwrap();
        assert!(Simulation::load_campaign(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn ordered_hunter_maps_keep_legacy_map_wire_format() {
        #[derive(serde::Serialize)]
        struct Canonical {
            #[serde(serialize_with = "serialize_unit_map")]
            positions: std::collections::HashMap<u32, (i32, i32)>,
        }
        let entries = [(17, (4, 8)), (3, (10, 12)), (51, (16, 20))];
        let original: std::collections::HashMap<_, _> = entries.into_iter().collect();
        let legacy_bytes = codec().serialize(&original).unwrap();
        let legacy_decoded: std::collections::HashMap<u32, (i32, i32)> =
            codec().deserialize(&legacy_bytes).unwrap();
        let reversed = entries.into_iter().rev().collect();
        let canonical = codec()
            .serialize(&Canonical {
                positions: original.clone(),
            })
            .unwrap();
        assert_eq!(
            canonical,
            codec()
                .serialize(&Canonical {
                    positions: reversed
                })
                .unwrap()
        );
        // A one-field struct has the exact historical map encoding; no new
        // save header, sequence tag or per-entry metadata is introduced.
        let decoded: std::collections::HashMap<u32, (i32, i32)> =
            codec().deserialize(&canonical).unwrap();
        assert_eq!(decoded, legacy_decoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn strategic_state_loaded_replays_and_native_task_resets_migrate_legacy_saves() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.save_campaign_playtest().unwrap(), (true, true, true));
    }

    #[test]
    fn invalid_space_countdown_is_rejected_before_advancing() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let path = std::env::temp_dir().join(format!(
            "civcraft-invalid-ai-timer-{}-{}.save",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        game.ai_space_warnings.insert((1, HUMAN), i32::MIN);
        game.save_campaign(&path).unwrap();
        let error = match Simulation::load_campaign(&path) {
            Ok(_) => panic!("invalid space warning was accepted"),
            Err(error) => error,
        };
        fs::remove_file(path).unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("space warning"));
    }
}

#[cfg(test)]
mod wrap_save_tests {
    use super::*;
    #[test]
    fn new_saves_preserve_the_campaign_wrap_setting() {
        let path = std::env::temp_dir().join(format!("civcraft-wrap-{}.save", std::process::id()));
        for wrap in [MapWrap::X, MapWrap::XY] {
            let game = Simulation::new(crate::mapgen::generate_with_wrap(42, wrap));
            game.save_campaign(&path).unwrap();
            let loaded = Simulation::load_campaign(&path).unwrap();
            assert_eq!(loaded.map_wrap, wrap);
            assert_eq!(loaded.random_snapshot(), game.random_snapshot());
            assert_eq!(
                bincode::serialize(&loaded.tiles).unwrap(),
                bincode::serialize(&game.tiles).unwrap()
            );
        }
        fs::remove_file(path).unwrap();
    }
}
