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
            fs::write(&path, b"CIVCRAFT\0\xffbad save")?;
            let rejected = Self::load_campaign(&path).is_err();
            Ok((restored, deterministic, replaced && rejected))
        })();
        let cleanup = fs::remove_dir_all(dir);
        result.and_then(|outcome| cleanup.map(|()| outcome))
    }
}
