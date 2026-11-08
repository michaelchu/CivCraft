//! Focused gameplay scenarios for the parent subsystem.

use super::*;
use std::sync::atomic::Ordering;

impl Simulation {
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
            let home = self.cities_owned_by(1).next().unwrap().id;
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
            self.tile_mut(19, 33).terrain = Terrain::Sea;
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
}
