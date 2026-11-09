//! Tactical and strategic decisions for computer-controlled units and players.
//!
//! The AI evaluates the same map topology and rule outcomes as the human
//! player, but uses per-player observations and remembered routes where the
//! rules limit knowledge. Specialized child modules handle city defense,
//! invasion planning, transport assaults, hunting, and unit automation.
//! Economy and advisor decisions live in sibling modules and feed production
//! and research priorities back into the AI turn.
//!
//! Preserve stable candidate ordering and random draw behavior when adjusting
//! scores: ties can determine unit targets, production, and reference parity.

use super::normal_paths::GotoTask;
use super::*;

mod assault_transports;
mod city_defense;
mod ground_invasion;
mod hunter;
mod unit_automation;

impl Simulation {
    /// Our ferry itinerary stores a sea waypoint separately from the final
    /// passenger destination. Native amphibious PF fills the ferry parameter
    /// with that final land tile, permanently retaining it as goto_tile.
    fn clean_current_tile(&mut self, unit: &Unit) -> bool {
        unit.kind.can_clean()
            && [Contamination::Fallout, Contamination::Pollution]
                .into_iter()
                .any(|target| {
                    self.tile(unit.x, unit.y).has_contamination(target)
                        && self.begin_work(unit.id, WorkKind::Clean(target))
                })
    }

    fn assault_capture_chance(&self, city: &City, troops: &[&Unit]) -> f64 {
        let Some(first) = troops.first() else {
            return 0.0;
        };
        let defenders: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                (unit.x, unit.y) == (city.x, city.y)
                    && unit.transport.is_none()
                    && self.at_war(first.owner, unit.owner)
            })
            .collect();
        if !troops.iter().any(|unit| unit.kind.can_occupy_city()) || defenders.len() > troops.len()
        {
            return 0.0;
        }
        if defenders.is_empty() {
            return 1.0;
        }
        // Estimate successive victories conservatively: each attacker faces
        // the hardest remaining type at its current health.
        let needed = defenders.len();
        let mut victories = vec![0.0; needed + 1];
        victories[0] = 1.0;
        for attacker in troops {
            let chance = defenders
                .iter()
                .map(|defender| {
                    let (attack_fp, defense_fp) = self.combat_firepower(attacker, defender);
                    let attack = percent_of(
                        attacker.kind.stats().attack * POWER_FACTOR,
                        attacker.veteran_power(),
                    );
                    combat_win_chance(
                        attack,
                        attacker.hp,
                        attack_fp,
                        self.defense_power_against(defender, city.x, city.y, attacker.kind),
                        defender.hp,
                        defense_fp,
                    )
                })
                .fold(1.0_f64, f64::min);
            victories[needed] += victories[needed - 1] * chance;
            for wins in (1..needed).rev() {
                victories[wins] = victories[wins] * (1.0 - chance) + victories[wins - 1] * chance;
            }
            victories[0] *= 1.0 - chance;
        }
        victories[needed]
    }

    pub(super) fn run_ai_player(&mut self, owner: usize) {
        if owner != BARBARIANS && (owner >= self.civilization_count || !self.player_alive(owner)) {
            return;
        }
        // dai_do_first_activities assesses danger (and its research feedback)
        // before moving units; do_last_activities clears these wants later.
        self.record_ai_first_activity_tech_wants(owner);
        self.refresh_ai_ferries();
        self.refresh_ai_guards();
        let boats: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.owner == owner && Self::is_ai_ferry(unit.kind))
            .map(|unit| unit.id)
            .collect();
        for boat in boats {
            self.refresh_ferry_boss(boat);
        }
        self.agent_city_targets.retain(|id, city| {
            self.units
                .iter()
                .any(|unit| unit.id == *id && unit.kind.is_diplomatic())
                && self.cities.iter().any(|target| target.id == *city)
        });
        let counterattacks = self.counterattack_from_ai_cities(owner);
        let reinforcements = self.reinforce_ai_cities(owner);
        let paratroopers: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == owner
                    && unit.kind == UnitKind::Paratroopers
                    && !counterattacks.contains(&unit.id)
                    && !reinforcements.contains(&unit.id)
            })
            .cloned()
            .collect();
        let drops: std::collections::HashSet<_> = paratroopers
            .iter()
            .filter_map(|unit| self.run_ai_paratrooper(unit).then_some(unit.id))
            .collect();
        let ferry_dispatch = self.load_ai_merchant_ferries(owner);
        self.prune_settlement_reservations(owner);
        let settlement_ferries = self.dispatch_settlement_ferries(owner, &ferry_dispatch);
        let fleet = self.coordinate_assault_fleets(
            owner,
            |id| ferry_dispatch.contains_key(&id) || settlement_ferries.contains_key(&id),
            |id| {
                counterattacks.contains(&id) || reinforcements.contains(&id) || drops.contains(&id)
            },
        );
        let assault = self.dispatch_assault_transports(
            owner,
            |id| {
                ferry_dispatch.contains_key(&id)
                    || settlement_ferries.contains_key(&id)
                    || fleet.contains(&id)
            },
            |id| {
                counterattacks.contains(&id)
                    || reinforcements.contains(&id)
                    || drops.contains(&id)
                    || fleet.contains(&id)
            },
        );
        let reserved = counterattacks
            .iter()
            .chain(&reinforcements)
            .chain(&drops)
            .chain(&fleet)
            .chain(&assault)
            .copied()
            .collect();
        self.refresh_ground_invasions(owner, &reserved);
        let mut claimed_targets = std::collections::HashSet::new();
        let mut ferry_managed = std::collections::HashSet::new();
        let unit_ids: Vec<u32> = self.player_units_ordered(owner).map(|u| u.id).collect();
        for id in unit_ids {
            let Some(unit) = self.unit_by_id(id).cloned() else {
                continue;
            };
            if unit.kind == UnitKind::BarbarianLeader {
                self.run_barbarian_leader(id);
                continue;
            }
            if self.ai_guard_targets.contains_key(&id) && self.run_ai_guard(id) {
                continue;
            }
            if ferry_managed.contains(&id)
                || unit
                    .transport
                    .and_then(|boat| self.unit_by_id(boat))
                    .is_some_and(|boat| boat.owner == owner)
            {
                // Native owner iteration delegates own passengers through
                // their transport. Foreign allied transports are different.
                continue;
            }
            if unit.kind.fuel_capacity() > 0 {
                self.run_ai_strike_aircraft(&unit);
                continue;
            }
            if assault.contains(&id)
                || fleet.contains(&id)
                || drops.contains(&id)
                || counterattacks.contains(&id)
                || reinforcements.contains(&id)
                || ferry_dispatch.values().any(|&passenger| passenger == id)
                || settlement_ferries.values().any(|plan| plan.founder == id)
            {
                continue;
            }
            // Native worker/founder dispatch precedes standalone ferry AI.
            if unit.kind.is_worker() || unit.kind.stats().can_found {
                continue;
            }
            if unit.kind.is_naval()
                && unit.kind != UnitKind::Submarine
                && unit.kind.transport_capacity() > 0
                && !self.hunter_targets.contains_key(&id)
            {
                let (handled, done) = self.run_source_air_ferry(&unit);
                ferry_managed.extend(done);
                if handled {
                    continue;
                }
                if let Some(plan) = settlement_ferries.get(&id) {
                    self.run_settlement_ferry(id, plan);
                    continue;
                }
                if !self.run_merchant_ferry(id, ferry_dispatch.get(&id).copied())
                    && !self.clean_current_tile(&unit)
                {
                    self.run_raider_transport(&unit);
                }
                continue;
            }
            if owner != BARBARIANS
                && unit.kind.has_role(super::unit_catalog::UnitRole::Hunter)
                && self.run_existing_hunter(&unit, &mut claimed_targets)
            {
                continue;
            }
            if unit.kind.is_military()
                && !unit.kind.is_aircraft()
                && !Self::is_ai_ferry(unit.kind)
                && self.run_ai_guard(id)
            {
                continue;
            }
            match unit.kind {
                // Founders and workers share the late-phase workermap.
                UnitKind::Settlers | UnitKind::Worker | UnitKind::Engineer | UnitKind::Migrants => {
                }
                UnitKind::Diplomat | UnitKind::Spy => {
                    self.ai_bribe_nearby(&unit);
                    let Some(agent) = self.unit_by_id(id).cloned() else {
                        continue;
                    };
                    if agent.moves_left <= 0 {
                        continue;
                    }
                    // Freeciv keeps a sole agent in an exposed city. Otherwise
                    // it resumes/acquires an offensive mission before defending
                    // another city (ai/default/daidiplomat.c:dai_manage_diplomat).
                    if self.holds_counterintelligence(&agent) {
                        self.agent_city_targets.remove(&id);
                    } else if !self.ai_city_espionage(&agent)
                        && !self.assign_counterintelligence(&agent)
                    {
                        self.wander_unit(id);
                    }
                }
                UnitKind::Explorer => {
                    self.run_auto_explorer(id);
                }
                UnitKind::BarbarianLeader => unreachable!("leader handled before cargo dispatch"),
                UnitKind::Caravan | UnitKind::Freight => self.run_ai_merchant(&unit),
                _ => {
                    let on_city = self
                        .cities
                        .iter()
                        .find(|c| c.owner == owner && c.x == unit.x && c.y == unit.y)
                        .cloned();
                    if let Some(city) = on_city {
                        let defenders = self
                            .units
                            .iter()
                            .filter(|u| {
                                u.owner == owner
                                    && u.x == city.x
                                    && u.y == city.y
                                    && u.kind.is_military()
                            })
                            .count();
                        if defenders <= 1 || self.city_threats(&city).next().is_some() {
                            self.unit_goto_tiles.insert(id, (city.x, city.y));
                            self.fortify_unit(id);
                            continue;
                        }
                    }
                    if unit.kind.is_military()
                        && !unit.kind.is_aircraft()
                        && !unit.kind.is_naval()
                        && owner != BARBARIANS
                    {
                        let mission = self
                            .ai_ground_targets
                            .get(&id)
                            .and_then(|target| self.city_by_id(*target))
                            .filter(|city| self.at_war(owner, city.owner))
                            .and_then(|city| {
                                self.ai_goto_path(&unit, (city.x, city.y), GotoTask::Attack)
                            })
                            .map(|(path, _)| path);
                        if let Some(path) = mission {
                            self.follow_ground_invasion_path(id, path);
                        } else {
                            self.unit_goto_tiles.remove(&id);
                            self.run_auto_explorer(id);
                        }
                        continue;
                    }
                    if let Some(target) = self
                        .cities
                        .iter()
                        .filter(|c| self.at_war(c.owner, owner))
                        .min_by_key(|c| self.map_distance(unit.x, unit.y, c.x, c.y))
                        .cloned()
                    {
                        self.unit_goto_tiles.insert(id, (target.x, target.y));
                        self.ai_goto_unit(id, (target.x, target.y), GotoTask::Attack);
                    } else if owner == BARBARIANS {
                        self.wander_unit(id);
                    } else {
                        self.unit_goto_tiles.remove(&id);
                        self.run_auto_explorer(id);
                    }
                }
            }
        }
        self.run_ai_worker_phase(owner);
        let alive: std::collections::HashSet<_> = self.units.iter().map(|u| u.id).collect();
        self.ai_ground_targets.retain(|id, _| alive.contains(id));
    }
}

#[cfg(test)]
#[path = "ai/hunter_tests.rs"]
mod hunter_tests;
