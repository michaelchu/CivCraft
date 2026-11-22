//! Military effectiveness, force composition, combined-arms, Great Leader, and Army analytics.

use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{
    Pact,
    military_observation::military_role_for_unit_name,
    observation::{DiplomacySnapshot, Snapshot},
    trace::{Entity, Event},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const STRATEGIC_ROLES: &[&str] = &["bombard", "bomber", "fighter", "missile", "nuclear"];
const IDLE_WAR_STREAK_THRESHOLD: u32 = 5;
const OBJECTIVE_STALL_THRESHOLD: u32 = 10;
const UNUSED_ROLE_WAR_TURNS_THRESHOLD: u32 = 8;
const ARMY_UNDERUTILIZED_STREAK_THRESHOLD: u32 = 5;
const LEADER_SHORTFALL_EXPECTED_THRESHOLD: f64 = 3.0;
const RECENT_LIMIT: usize = 24;

#[derive(Default)]
struct NationMilitary {
    name: String,
    alive: bool,
    war_turns: u32,
    war_footing_turns: u32,
    peak_military_units: usize,
    peak_elite_land_units: usize,
    unlocked_roles: BTreeSet<String>,
    unlocked_war_turns: BTreeMap<String, u32>,
    roles_built: BTreeMap<String, u64>,
    roles_fielded_peak: BTreeMap<String, usize>,
    production_selected_by_role: BTreeMap<String, u64>,
    production_stage_wins: BTreeMap<String, u64>,
    production_suppressed_by_reason: BTreeMap<String, u64>,
    dispatch_unit_turns: BTreeMap<String, u64>,
    idle_wartime_unit_turns: u64,
    attacks: u64,
    attack_wins: u64,
    defenses: u64,
    defense_wins: u64,
    city_attacks: u64,
    city_captures: u64,
    softened_city_attacks: u64,
    shields_destroyed: i64,
    shields_lost: i64,
    bombard_shots: u64,
    bombard_aircraft_shots: u64,
    bombard_ground_naval_shots: u64,
    bombard_precision_shots: u64,
    bombard_intercepted: u64,
    bombard_hp_damage: i64,
    bombard_units_killed: u64,
    bombard_buildings_destroyed: u64,
    bombard_population_lost: i64,
    bombard_terrain_destroyed: u64,
    bombard_targets_by_kind: BTreeMap<String, u64>,
    air_missions_by_type: BTreeMap<String, u64>,
    air_interceptions: u64,
    air_interception_kills: u64,
    paradrops: u64,
    paradrops_succeeded: u64,
    cruise_missile_launches: u64,
    cruise_missiles_expended: u64,
    nuclear_strikes: u64,
    nuclear_sdi_intercepts: u64,
    nuclear_units_killed: i64,
    nuclear_cities_hit: u64,
    elite_kills: u64,
    eligible_leader_kills: u64,
    leader_rolls_attempted: u64,
    expected_leaders: f64,
    leaders_spawned: u64,
    ineligible_elite_kills_by_reason: BTreeMap<String, u64>,
    leader_actions: BTreeMap<String, u64>,
    leader_rushed_when_army_legal: u64,
    leader_shields_saved: i64,
    armies_formed: u64,
    army_members_loaded: u64,
    army_unit_turns: u64,
    army_empty_turns: u64,
    army_active_turns: u64,
    army_moved_turns: u64,
    army_dispatch_counts: BTreeMap<String, u64>,
    army_attacks: u64,
    army_attack_wins: u64,
    army_defenses: u64,
    army_defense_wins: u64,
    army_losses: u64,
    objective_turns: u32,
    objective_ready_turns: u32,
    objective_stalled_streak: u32,
    objective_max_stalled_streak: u32,
    assault_transports_dispatched: u64,
    idle_war_streak: u32,
    army_underutilized_streak: u32,
    last_combat_action_turn: Option<i32>,
    last_army_combat_turn: Option<i32>,
}

#[derive(Clone)]
struct WarEpisode {
    players: [usize; 2],
    names: [String; 2],
    start_turn: i32,
    end_turn: Option<i32>,
    active: bool,
    combats: u64,
    bombards: u64,
    air_strikes: u64,
    nuclear_strikes: u64,
    cities_captured_by: [u64; 2],
    shields_destroyed_by: [i64; 2],
    armies_engaged: u64,
}

#[derive(Default)]
pub(crate) struct Military {
    nations: BTreeMap<usize, NationMilitary>,
    wars: Vec<WarEpisode>,
    active_wars: BTreeMap<(usize, usize), usize>,
    combats_total: u64,
    bombards_total: u64,
    air_missions_total: u64,
    missile_launches_total: u64,
    nuclear_strikes_total: u64,
    eligible_leader_kills_total: u64,
    leaders_spawned_total: u64,
    armies_formed_total: u64,
    army_combats_total: u64,
    dropped: u64,
    recent_combats: VecDeque<Value>,
    recent_strikes: VecDeque<Value>,
    recent_leader_events: VecDeque<Value>,
}

impl Military {
    fn nation(&mut self, id: usize) -> &mut NationMilitary {
        self.nations.entry(id).or_insert_with(|| NationMilitary {
            name: format!("Civilization {id}"),
            alive: true,
            ..Default::default()
        })
    }

    fn canonical_pair(a: usize, b: usize) -> (usize, usize) {
        if a <= b { (a, b) } else { (b, a) }
    }

    fn active_war_mut(&mut self, a: usize, b: usize) -> Option<(&mut WarEpisode, usize)> {
        let pair = Self::canonical_pair(a, b);
        let &index = self.active_wars.get(&pair)?;
        let slot = usize::from(a != pair.0);
        Some((&mut self.wars[index], slot))
    }

    fn push_recent(queue: &mut VecDeque<Value>, entry: Value) {
        queue.push_back(entry);
        if queue.len() > RECENT_LIMIT {
            queue.pop_front();
        }
    }

    fn publish_metrics(&self, out: &mut AnalysisOutput) {
        let nations: Vec<Value> = self
            .nations
            .iter()
            .map(|(&id, n)| {
                let exchange_ratio = if n.shields_lost > 0 {
                    Some(
                        ((n.shields_destroyed as f64 / n.shields_lost as f64) * 100.0).round()
                            / 100.0,
                    )
                } else if n.shields_destroyed > 0 {
                    Some(n.shields_destroyed as f64)
                } else {
                    None
                };
                let unused_unlocked_roles: Vec<&String> = n
                    .unlocked_war_turns
                    .iter()
                    .filter(|(role, turns)| {
                        **turns > 0
                            && n.roles_built.get(*role).copied().unwrap_or(0) == 0
                            && n.roles_fielded_peak.get(*role).copied().unwrap_or(0) == 0
                    })
                    .map(|(role, _)| role)
                    .collect();
                json!({
                    "id": id,
                    "name": n.name,
                    "alive": n.alive,
                    "war_turns": n.war_turns,
                    "war_footing_turns": n.war_footing_turns,
                    "peak_military_units": n.peak_military_units,
                    "peak_elite_land_units": n.peak_elite_land_units,
                    "unlocked_roles": n.unlocked_roles,
                    "unlocked_war_turns": n.unlocked_war_turns,
                    "roles_built": n.roles_built,
                    "roles_fielded_peak": n.roles_fielded_peak,
                    "unlocked_but_never_used_in_war": unused_unlocked_roles,
                    "production_decisions": {
                        "selected_by_role": n.production_selected_by_role,
                        "military_stage_wins": n.production_stage_wins,
                        "suppressed_by_reason": n.production_suppressed_by_reason,
                    },
                    "dispatch_unit_turns": n.dispatch_unit_turns,
                    "idle_wartime_unit_turns": n.idle_wartime_unit_turns,
                    "combat": {
                        "attacks": n.attacks,
                        "attack_wins": n.attack_wins,
                        "defenses": n.defenses,
                        "defense_wins": n.defense_wins,
                        "city_attacks": n.city_attacks,
                        "city_captures": n.city_captures,
                        "softened_city_attacks": n.softened_city_attacks,
                        "shields_destroyed": n.shields_destroyed,
                        "shields_lost": n.shields_lost,
                        "exchange_ratio": exchange_ratio,
                    },
                    "bombard": {
                        "shots": n.bombard_shots,
                        "aircraft_shots": n.bombard_aircraft_shots,
                        "ground_naval_shots": n.bombard_ground_naval_shots,
                        "precision_shots": n.bombard_precision_shots,
                        "intercepted": n.bombard_intercepted,
                        "hp_damage": n.bombard_hp_damage,
                        "units_killed": n.bombard_units_killed,
                        "buildings_destroyed": n.bombard_buildings_destroyed,
                        "population_lost": n.bombard_population_lost,
                        "terrain_destroyed": n.bombard_terrain_destroyed,
                        "targets_by_kind": n.bombard_targets_by_kind,
                    },
                    "air_and_missiles": {
                        "missions_by_type": n.air_missions_by_type,
                        "interceptions": n.air_interceptions,
                        "interception_kills": n.air_interception_kills,
                        "paradrops": n.paradrops,
                        "paradrops_succeeded": n.paradrops_succeeded,
                        "cruise_missile_launches": n.cruise_missile_launches,
                        "cruise_missiles_expended": n.cruise_missiles_expended,
                        "nuclear_strikes": n.nuclear_strikes,
                        "nuclear_sdi_intercepts": n.nuclear_sdi_intercepts,
                        "nuclear_units_killed": n.nuclear_units_killed,
                        "nuclear_cities_hit": n.nuclear_cities_hit,
                    },
                    "leaders_and_armies": {
                        "elite_kills": n.elite_kills,
                        "eligible_leader_kills": n.eligible_leader_kills,
                        "leader_rolls_attempted": n.leader_rolls_attempted,
                        "expected_leaders": (n.expected_leaders * 1000.0).round() / 1000.0,
                        "leaders_spawned": n.leaders_spawned,
                        "ineligible_elite_kills_by_reason": n.ineligible_elite_kills_by_reason,
                        "leader_actions": n.leader_actions,
                        "leader_rushed_when_army_legal": n.leader_rushed_when_army_legal,
                        "leader_shields_saved": n.leader_shields_saved,
                        "armies_formed": n.armies_formed,
                        "army_members_loaded": n.army_members_loaded,
                        "army_unit_turns": n.army_unit_turns,
                        "army_empty_turns": n.army_empty_turns,
                        "army_active_turns": n.army_active_turns,
                        "army_moved_turns": n.army_moved_turns,
                        "army_dispatch_counts": n.army_dispatch_counts,
                        "army_attacks": n.army_attacks,
                        "army_attack_wins": n.army_attack_wins,
                        "army_defenses": n.army_defenses,
                        "army_defense_wins": n.army_defense_wins,
                        "army_losses": n.army_losses,
                    },
                    "coordinated_wars": {
                        "objective_turns": n.objective_turns,
                        "ready_turns": n.objective_ready_turns,
                        "max_stalled_streak": n.objective_max_stalled_streak,
                        "assault_transports_dispatched": n.assault_transports_dispatched,
                    },
                })
            })
            .collect();
        let wars: Vec<Value> = self
            .wars
            .iter()
            .map(|w| {
                json!({
                    "players": w.players,
                    "names": w.names,
                    "start_turn": w.start_turn,
                    "end_turn": w.end_turn,
                    "active": w.active,
                    "combats": w.combats,
                    "bombards": w.bombards,
                    "air_strikes": w.air_strikes,
                    "nuclear_strikes": w.nuclear_strikes,
                    "cities_captured_by": w.cities_captured_by,
                    "shields_destroyed_by": w.shields_destroyed_by,
                    "armies_engaged": w.armies_engaged,
                })
            })
            .collect();
        out.metric("nations", json!(nations));
        out.metric("wars", json!(wars));
        out.metric(
            "totals",
            json!({
                "combats": self.combats_total,
                "bombards": self.bombards_total,
                "air_missions": self.air_missions_total,
                "missile_launches": self.missile_launches_total,
                "nuclear_strikes": self.nuclear_strikes_total,
                "eligible_leader_kills": self.eligible_leader_kills_total,
                "leaders_spawned": self.leaders_spawned_total,
                "armies_formed": self.armies_formed_total,
                "army_combats": self.army_combats_total,
                "event_counts_complete": self.dropped == 0,
            }),
        );
        out.metric("recent_combats", json!(self.recent_combats));
        out.metric("recent_strikes", json!(self.recent_strikes));
        out.metric("recent_leader_events", json!(self.recent_leader_events));
        out.metric("event_counts_complete", self.dropped == 0);
    }

    fn handle_combat(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let p = &event.payload;
        let Some(attacker_owner) = p["attacker_owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let Some(defender_owner) = p["defender_owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let winner = p["winner"].as_str().unwrap_or("none");
        let attacker_cost = p["attacker_cost"].as_i64().unwrap_or(0);
        let defender_cost = p["defender_cost"].as_i64().unwrap_or(0);
        let attacker_elite = p["attacker_elite"].as_bool().unwrap_or(false);
        let defender_elite = p["defender_elite"].as_bool().unwrap_or(false);
        let attacker_is_army = p["attacker_is_army"].as_bool().unwrap_or(false);
        let defender_is_army = p["defender_is_army"].as_bool().unwrap_or(false);
        let city_attack = p["city_id"].is_number();
        let city_captured = p["city_captured"].as_bool().unwrap_or(false);
        let defender_was_damaged = p["defender_was_damaged"].as_bool().unwrap_or(false);
        let missile_expended = p["missile_expended"].as_bool().unwrap_or(false);
        let powerless = p["powerless_combat"].as_bool().unwrap_or(false);
        let leader_eligible = p["leader_eligible"].as_bool().unwrap_or(false);
        let leader_reason = p["leader_ineligible_reason"].as_str();
        let leader_denom = p["leader_roll_denominator"].as_i64();

        self.combats_total += 1;
        if attacker_is_army || defender_is_army {
            self.army_combats_total += 1;
        }
        if leader_eligible {
            self.eligible_leader_kills_total += 1;
        }

        {
            let a = self.nation(attacker_owner);
            a.attacks += 1;
            a.last_combat_action_turn = Some(event.turn);
            if city_attack {
                a.city_attacks += 1;
                if defender_was_damaged {
                    a.softened_city_attacks += 1;
                }
            }
            if city_captured {
                a.city_captures += 1;
            }
            if missile_expended {
                a.cruise_missiles_expended += 1;
            }
            if attacker_is_army {
                a.army_attacks += 1;
                a.last_army_combat_turn = Some(event.turn);
            }
            if winner == "attacker" {
                a.attack_wins += 1;
                a.shields_destroyed += defender_cost;
                if attacker_is_army {
                    a.army_attack_wins += 1;
                }
                if attacker_elite && !powerless {
                    a.elite_kills += 1;
                    if leader_eligible {
                        a.eligible_leader_kills += 1;
                        if let Some(denom) = leader_denom.filter(|&d| d > 0) {
                            a.expected_leaders += 1.0 / denom as f64;
                        }
                    } else if let Some(reason) = leader_reason {
                        *a.ineligible_elite_kills_by_reason
                            .entry(reason.to_owned())
                            .or_default() += 1;
                    }
                }
            } else if winner == "defender" {
                a.shields_lost += attacker_cost;
                if attacker_is_army {
                    a.army_losses += 1;
                }
            }
        }
        {
            let d = self.nation(defender_owner);
            d.defenses += 1;
            d.last_combat_action_turn = Some(event.turn);
            if defender_is_army {
                d.army_defenses += 1;
                d.last_army_combat_turn = Some(event.turn);
            }
            if winner == "defender" {
                d.defense_wins += 1;
                d.shields_destroyed += attacker_cost;
                if defender_is_army {
                    d.army_defense_wins += 1;
                }
                if defender_elite && !powerless {
                    d.elite_kills += 1;
                    if leader_eligible {
                        d.eligible_leader_kills += 1;
                        if let Some(denom) = leader_denom.filter(|&d| d > 0) {
                            d.expected_leaders += 1.0 / denom as f64;
                        }
                    } else if let Some(reason) = leader_reason {
                        *d.ineligible_elite_kills_by_reason
                            .entry(reason.to_owned())
                            .or_default() += 1;
                    }
                }
            } else if winner == "attacker" {
                d.shields_lost += defender_cost;
                if defender_is_army {
                    d.army_losses += 1;
                }
            }
        }

        if let Some((war, att_slot)) = self.active_war_mut(attacker_owner, defender_owner) {
            let def_slot = 1 - att_slot;
            war.combats += 1;
            if attacker_is_army || defender_is_army {
                war.armies_engaged += 1;
            }
            if winner == "attacker" {
                war.shields_destroyed_by[att_slot] += defender_cost;
            } else if winner == "defender" {
                war.shields_destroyed_by[def_slot] += attacker_cost;
            }
            if city_captured {
                war.cities_captured_by[att_slot] += 1;
            }
        }

        Self::push_recent(
            &mut self.recent_combats,
            json!({
                "turn": event.turn,
                "evidence": out.evidence(),
                "attacker_owner": attacker_owner,
                "attacker_kind": p["attacker_kind"],
                "defender_owner": defender_owner,
                "defender_kind": p["defender_kind"],
                "winner": winner,
                "city_captured": city_captured,
                "army_battle": attacker_is_army || defender_is_army,
                "leader_eligible": leader_eligible,
                "leader_created_id": p["leader_created_id"],
            }),
        );
    }

    fn handle_leader(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let p = &event.payload;
        let Some(owner) = p["owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let Some(action) = p["action"].as_str() else {
            return;
        };
        let n = self.nation(owner);
        *n.leader_actions.entry(action.to_owned()).or_default() += 1;
        match action {
            "spawned" => {
                n.leader_rolls_attempted += 1;
                n.leaders_spawned += 1;
                self.leaders_spawned_total += 1;
            }
            "roll_missed" => {
                n.leader_rolls_attempted += 1;
            }
            "rushed_building" => {
                n.leader_shields_saved += p["shields_saved"].as_i64().unwrap_or(0);
                if p["can_form_army"].as_bool().unwrap_or(false) {
                    n.leader_rushed_when_army_legal += 1;
                }
            }
            "formed_army" => {
                n.armies_formed += 1;
                self.armies_formed_total += 1;
            }
            "army_member_loaded" => {
                n.army_members_loaded += 1;
            }
            _ => {}
        }
        if matches!(
            action,
            "spawned" | "rushed_building" | "formed_army" | "army_member_loaded"
        ) {
            Self::push_recent(
                &mut self.recent_leader_events,
                json!({
                    "turn": event.turn,
                    "evidence": out.evidence(),
                    "event": p,
                }),
            );
        }
    }

    fn handle_bombard(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let p = &event.payload;
        let Some(owner) = p["attacker_owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let role = p["attacker_role"].as_str().unwrap_or("other");
        let target_kind = p["target_kind"].as_str().unwrap_or("other");
        let precision = p["precision"].as_bool().unwrap_or(false);
        let intercepted = p["intercepted"].as_bool().unwrap_or(false);
        let hp_damage = p["hp_damage"].as_i64().unwrap_or(0);
        let units_killed = p["units_killed"].as_u64().unwrap_or(0);
        let population_lost = p["population_lost"].as_i64().unwrap_or(0);
        let terrain_destroyed = p["terrain_destroyed"]
            .as_array()
            .map_or(0, |items| items.len() as u64);
        let missile_expended = p["missile_expended"].as_bool().unwrap_or(false);
        let target_owner = p["target_owner"].as_u64().map(|v| v as usize);

        self.bombards_total += 1;
        {
            let n = self.nation(owner);
            n.bombard_shots += 1;
            n.last_combat_action_turn = Some(event.turn);
            if matches!(role, "bomber" | "fighter") {
                n.bombard_aircraft_shots += 1;
            } else {
                n.bombard_ground_naval_shots += 1;
            }
            if precision {
                n.bombard_precision_shots += 1;
            }
            if intercepted {
                n.bombard_intercepted += 1;
            }
            n.bombard_hp_damage += hp_damage;
            n.bombard_units_killed += units_killed;
            if p["building_destroyed"].is_string() {
                n.bombard_buildings_destroyed += 1;
            }
            n.bombard_population_lost += population_lost;
            n.bombard_terrain_destroyed += terrain_destroyed;
            if missile_expended {
                n.cruise_missiles_expended += 1;
            }
            *n.bombard_targets_by_kind
                .entry(target_kind.to_owned())
                .or_default() += 1;
        }
        if let Some(other) = target_owner
            && let Some((war, _)) = self.active_war_mut(owner, other)
        {
            war.bombards += 1;
            if matches!(role, "bomber" | "fighter") {
                war.air_strikes += 1;
            }
        }
        Self::push_recent(
            &mut self.recent_strikes,
            json!({
                "turn": event.turn,
                "kind": "bombard",
                "evidence": out.evidence(),
                "owner": owner,
                "unit_kind": p["attacker_kind"],
                "target_kind": target_kind,
                "hp_damage": hp_damage,
                "units_killed": units_killed,
                "building_destroyed": p["building_destroyed"],
                "intercepted": intercepted,
            }),
        );
    }

    fn handle_air_mission(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let p = &event.payload;
        let Some(owner) = p["owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let Some(mission) = p["mission"].as_str() else {
            return;
        };
        let succeeded = p["succeeded"].as_bool().unwrap_or(false);
        let target_owner = p["target_owner"].as_u64().map(|v| v as usize);

        self.air_missions_total += 1;
        if mission == "missile_launch" {
            self.missile_launches_total += 1;
        }
        {
            let n = self.nation(owner);
            *n.air_missions_by_type
                .entry(mission.to_owned())
                .or_default() += 1;
            match mission {
                "interception" => {
                    n.air_interceptions += 1;
                    if succeeded {
                        n.air_interception_kills += 1;
                    }
                    n.last_combat_action_turn = Some(event.turn);
                }
                "paradrop" => {
                    n.paradrops += 1;
                    if succeeded {
                        n.paradrops_succeeded += 1;
                    }
                    n.last_combat_action_turn = Some(event.turn);
                }
                "missile_launch" => {
                    n.cruise_missile_launches += 1;
                    n.last_combat_action_turn = Some(event.turn);
                }
                "air_strike" | "bombard" => {
                    n.last_combat_action_turn = Some(event.turn);
                }
                _ => {}
            }
        }
        if mission == "air_strike"
            && let Some(other) = target_owner
            && let Some((war, _)) = self.active_war_mut(owner, other)
        {
            war.air_strikes += 1;
        }
        if matches!(
            mission,
            "interception" | "paradrop" | "missile_launch" | "air_strike"
        ) {
            Self::push_recent(
                &mut self.recent_strikes,
                json!({
                    "turn": event.turn,
                    "kind": mission,
                    "evidence": out.evidence(),
                    "owner": owner,
                    "unit_kind": p["kind"],
                    "to": p["to"],
                    "succeeded": succeeded,
                }),
            );
        }
    }

    fn handle_nuclear(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let p = &event.payload;
        let Some(owner) = p["owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let intercepted = p["intercepted_by_sdi"].is_number();
        let units_killed = p["units_killed"].as_i64().unwrap_or(0);
        let cities_hit = p["cities_hit"].as_u64().unwrap_or(0);
        let target_owner = p["target_owner"].as_u64().map(|v| v as usize);

        self.nuclear_strikes_total += 1;
        {
            let n = self.nation(owner);
            n.nuclear_strikes += 1;
            n.last_combat_action_turn = Some(event.turn);
            if intercepted {
                n.nuclear_sdi_intercepts += 1;
            } else {
                n.nuclear_units_killed += units_killed;
                n.nuclear_cities_hit += cities_hit;
            }
        }
        if let Some(other) = target_owner
            && let Some((war, _)) = self.active_war_mut(owner, other)
        {
            war.nuclear_strikes += 1;
        }
        Self::push_recent(
            &mut self.recent_strikes,
            json!({
                "turn": event.turn,
                "kind": "nuclear_strike",
                "evidence": out.evidence(),
                "owner": owner,
                "unit_kind": p["kind"],
                "action": p["action"],
                "intercepted_by_sdi": p["intercepted_by_sdi"],
                "units_killed": units_killed,
                "cities_hit": cities_hit,
            }),
        );
    }

    fn handle_production_decision(&mut self, event: &Event) {
        let p = &event.payload;
        let Some(owner) = p["owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let changed = p["changed"].as_bool().unwrap_or(false);
        let n = self.nation(owner);
        if changed && let Some(role) = p["selected_role"].as_str() {
            *n.production_selected_by_role
                .entry(role.to_owned())
                .or_default() += 1;
        }
        if changed
            && let Some(stage) = p["military_stage"].as_str()
            && p["suppressed_reason"].is_null()
        {
            *n.production_stage_wins.entry(stage.to_owned()).or_default() += 1;
        }
        if let Some(reason) = p["suppressed_reason"].as_str() {
            *n.production_suppressed_by_reason
                .entry(reason.to_owned())
                .or_default() += 1;
        }
    }

    fn handle_turn_summary(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let p = &event.payload;
        let Some(owner) = p["owner"].as_u64().map(|v| v as usize) else {
            return;
        };
        let at_war = p["at_war"].as_bool().unwrap_or(false);
        let on_war_footing = p["on_war_footing"].as_bool().unwrap_or(false);
        let elite_land_units = p["elite_land_units"].as_u64().unwrap_or(0) as usize;
        let idle_wartime = p["idle_wartime_military"].as_u64().unwrap_or(0);
        let assault_dispatched = p["assault_transports_dispatched"].as_u64().unwrap_or(0);

        let mut total_military = 0_usize;
        let mut army_underutilized_this_turn = false;
        let mut has_army = false;
        let mut objective_stalled = false;
        let mut objective_evidence = Value::Null;
        let idle_streak;
        let army_streak;
        let stalled_streak;
        let mut unused_role_alerts: Vec<(String, u32)> = Vec::new();
        let mut leader_shortfall = None;

        {
            let n = self.nation(owner);
            if at_war {
                n.war_turns += 1;
            }
            if on_war_footing {
                n.war_footing_turns += 1;
            }
            n.peak_elite_land_units = n.peak_elite_land_units.max(elite_land_units);
            n.idle_wartime_unit_turns += idle_wartime;
            n.assault_transports_dispatched += assault_dispatched;

            if let Some(roles) = p["unlocked_roles"].as_array() {
                for role in roles.iter().filter_map(Value::as_str) {
                    n.unlocked_roles.insert(role.to_owned());
                    if at_war {
                        *n.unlocked_war_turns.entry(role.to_owned()).or_default() += 1;
                    }
                }
            }
            if let Some(units_by_role) = p["units_by_role"].as_object() {
                for (role, count) in units_by_role {
                    let count = count.as_u64().unwrap_or(0) as usize;
                    if role != "leader" {
                        total_military += count;
                    }
                    let peak = n.roles_fielded_peak.entry(role.clone()).or_default();
                    *peak = (*peak).max(count);
                }
            }
            n.peak_military_units = n.peak_military_units.max(total_military);

            if let Some(dispatches) = p["dispatch_counts"].as_object() {
                for (branch, count) in dispatches {
                    *n.dispatch_unit_turns.entry(branch.clone()).or_default() +=
                        count.as_u64().unwrap_or(0);
                }
            }

            if let Some(armies) = p["armies"].as_array()
                && !armies.is_empty()
            {
                has_army = true;
                let mut any_empty_or_idle = false;
                for army in armies {
                    n.army_unit_turns += 1;
                    let members = army["members"].as_u64().unwrap_or(0);
                    let moved = army["moved"].as_bool().unwrap_or(false);
                    let spent_moves = army["spent_moves"].as_bool().unwrap_or(false);
                    if let Some(branch) = army["dispatch"].as_str() {
                        *n.army_dispatch_counts.entry(branch.to_owned()).or_default() += 1;
                    }
                    if members == 0 {
                        n.army_empty_turns += 1;
                        any_empty_or_idle = true;
                    } else if moved || spent_moves || n.last_army_combat_turn == Some(event.turn) {
                        n.army_active_turns += 1;
                        if moved {
                            n.army_moved_turns += 1;
                        }
                    } else if at_war {
                        any_empty_or_idle = true;
                    }
                }
                army_underutilized_this_turn = any_empty_or_idle;
            }
            if has_army && army_underutilized_this_turn {
                n.army_underutilized_streak += 1;
            } else {
                n.army_underutilized_streak = 0;
            }
            army_streak = n.army_underutilized_streak;

            if let Some(obj) = p["coordinated_objective"].as_object() {
                n.objective_turns += 1;
                let ready = obj
                    .get("force_ready")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if ready {
                    n.objective_ready_turns += 1;
                    n.objective_stalled_streak = 0;
                } else {
                    n.objective_stalled_streak += 1;
                    n.objective_max_stalled_streak = n
                        .objective_max_stalled_streak
                        .max(n.objective_stalled_streak);
                    objective_stalled = n.objective_stalled_streak >= OBJECTIVE_STALL_THRESHOLD;
                    objective_evidence = json!({
                        "stalled_streak": n.objective_stalled_streak,
                        "objective": obj,
                    });
                }
            } else {
                n.objective_stalled_streak = 0;
            }
            stalled_streak = n.objective_stalled_streak;

            let had_action_this_turn = n.last_combat_action_turn == Some(event.turn);
            if at_war
                && total_military >= 4
                && !had_action_this_turn
                && (idle_wartime as usize) * 2 >= total_military
            {
                n.idle_war_streak += 1;
            } else {
                n.idle_war_streak = 0;
            }
            idle_streak = n.idle_war_streak;

            for &role in STRATEGIC_ROLES {
                let turns = n.unlocked_war_turns.get(role).copied().unwrap_or(0);
                let built = n.roles_built.get(role).copied().unwrap_or(0);
                let peak = n.roles_fielded_peak.get(role).copied().unwrap_or(0);
                if turns >= UNUSED_ROLE_WAR_TURNS_THRESHOLD && built == 0 && peak == 0 {
                    unused_role_alerts.push((role.to_owned(), turns));
                }
            }

            if n.expected_leaders >= LEADER_SHORTFALL_EXPECTED_THRESHOLD && n.leaders_spawned == 0 {
                leader_shortfall = Some((n.eligible_leader_kills, n.expected_leaders));
            }
        }

        out.check(
            format!("war_army_idle:{owner}"),
            "war_army_idle",
            vec![Entity::Civilization(owner)],
            idle_streak >= IDLE_WAR_STREAK_THRESHOLD,
            "AI military stayed mostly idle without combat or bombardment during an active war",
            json!({
                "owner": owner,
                "idle_war_streak": idle_streak,
                "idle_wartime_military": idle_wartime,
                "total_military": total_military,
            }),
        );
        out.check(
            format!("war_objective_stalled:{owner}"),
            "war_objective_stalled",
            vec![Entity::Civilization(owner)],
            objective_stalled,
            "Coordinated war objective remained unready for an extended period",
            json!({
                "owner": owner,
                "stalled_streak": stalled_streak,
                "detail": objective_evidence,
            }),
        );
        out.check(
            format!("army_underutilized:{owner}"),
            "army_underutilized",
            vec![Entity::Civilization(owner)],
            army_streak >= ARMY_UNDERUTILIZED_STREAK_THRESHOLD,
            "Owned Army remained empty or inactive during wartime",
            json!({
                "owner": owner,
                "streak": army_streak,
                "armies": p["armies"],
            }),
        );
        for (role, turns) in unused_role_alerts {
            out.check(
                format!("unlocked_role_unused_in_war:{owner}:{role}"),
                "unlocked_role_unused_in_war",
                vec![Entity::Civilization(owner)],
                true,
                "Civilization had a strategic military unit role unlocked during war but never built or fielded one",
                json!({
                    "owner": owner,
                    "role": role,
                    "unlocked_war_turns": turns,
                }),
            );
        }
        if let Some((eligible, expected)) = leader_shortfall {
            out.check(
                format!("leader_spawn_shortfall:{owner}"),
                "leader_spawn_shortfall",
                vec![Entity::Civilization(owner)],
                true,
                "Eligible elite kills accumulated high expected Great Leader spawns without producing one",
                json!({
                    "owner": owner,
                    "eligible_leader_kills": eligible,
                    "expected_leaders": (expected * 1000.0).round() / 1000.0,
                }),
            );
        }
    }
}

impl Analyzer for Military {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "military",
            version: 1,
            events: vec![
                "military.combat",
                "military.leader",
                "military.bombard",
                "military.air_mission",
                "military.nuclear_strike",
                "military.production_decision",
                "military.turn_summary",
            ],
            providers: vec!["civilizations", "diplomacy", "lifecycle"],
            configuration: json!({
                "scope": "warfare_production_combined_arms_leaders_armies",
                "idle_war_streak_threshold": IDLE_WAR_STREAK_THRESHOLD,
                "objective_stall_threshold": OBJECTIVE_STALL_THRESHOLD,
                "unused_role_war_turns_threshold": UNUSED_ROLE_WAR_TURNS_THRESHOLD,
                "army_underutilized_streak_threshold": ARMY_UNDERUTILIZED_STREAK_THRESHOLD,
            }),
        }
    }

    fn on_start(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        self.on_boundary(snapshot, out);
    }

    fn on_event(&mut self, event: &Event, out: &mut AnalysisOutput) {
        match event.kind.as_str() {
            "military.combat" => self.handle_combat(event, out),
            "military.leader" => self.handle_leader(event, out),
            "military.bombard" => self.handle_bombard(event, out),
            "military.air_mission" => self.handle_air_mission(event, out),
            "military.nuclear_strike" => self.handle_nuclear(event, out),
            "military.production_decision" => self.handle_production_decision(event),
            "military.turn_summary" => self.handle_turn_summary(event, out),
            _ => return,
        }
        self.publish_metrics(out);
    }

    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        if let Some(civs) = snapshot.civilizations() {
            for civ in civs {
                let n = self.nation(civ.id);
                n.name = civ.name.clone();
                n.alive = civ.alive;
            }
        }
        if let Some(lifecycle) = snapshot.lifecycle() {
            for nation in self.nations.values_mut() {
                nation.roles_built.clear();
            }
            for row in &lifecycle.unit_totals {
                if row.built == 0 {
                    continue;
                }
                let role = military_role_for_unit_name(&row.kind);
                if role == "civilian" {
                    continue;
                }
                let n = self.nation(row.owner);
                *n.roles_built.entry(role.to_owned()).or_default() += row.built;
            }
        }
        if let Some(pairs) = snapshot.get::<Vec<DiplomacySnapshot>>("diplomacy") {
            let mut currently_at_war = BTreeSet::new();
            for pair in pairs {
                let key = Self::canonical_pair(pair.players[0], pair.players[1]);
                let at_war = pair.pact == Some(Pact::War) && pair.alive[0] && pair.alive[1];
                if at_war {
                    currently_at_war.insert(key);
                    if !self.active_wars.contains_key(&key) {
                        let idx = self.wars.len();
                        self.wars.push(WarEpisode {
                            players: [key.0, key.1],
                            names: if key.0 == pair.players[0] {
                                pair.names.clone()
                            } else {
                                [pair.names[1].clone(), pair.names[0].clone()]
                            },
                            start_turn: snapshot.turn,
                            end_turn: None,
                            active: true,
                            combats: 0,
                            bombards: 0,
                            air_strikes: 0,
                            nuclear_strikes: 0,
                            cities_captured_by: [0, 0],
                            shields_destroyed_by: [0, 0],
                            armies_engaged: 0,
                        });
                        self.active_wars.insert(key, idx);
                    }
                }
            }
            let ended: Vec<_> = self
                .active_wars
                .keys()
                .copied()
                .filter(|pair| !currently_at_war.contains(pair))
                .collect();
            for pair in ended {
                if let Some(idx) = self.active_wars.remove(&pair) {
                    self.wars[idx].active = false;
                    self.wars[idx].end_turn = Some(snapshot.turn);
                }
            }
        }
        self.publish_metrics(out);
    }

    fn on_gap(&mut self, dropped: u64, out: &mut AnalysisOutput) {
        self.dropped += dropped;
        self.publish_metrics(out);
    }

    fn finish(&mut self, out: &mut AnalysisOutput) {
        self.publish_metrics(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::Phase;

    fn make_event(turn: i32, sequence: u64, kind: &str, payload: Value) -> Event {
        Event {
            sequence,
            turn,
            phase: Phase::AiPlanning,
            source: "military".into(),
            kind: kind.into(),
            schema_version: 1,
            entities: vec![],
            cause: None,
            payload,
        }
    }

    #[test]
    fn tracks_combat_leaders_armies_bombard_air_and_nukes() {
        let mut analyzer = Military::default();
        let mut out = AnalysisOutput::default();
        out.context("military", 10, None, Some((1, true)));

        analyzer.on_event(
            &make_event(
                10,
                1,
                "military.combat",
                json!({
                    "attacker_id": 1, "attacker_owner": 1, "attacker_kind": "Knight",
                    "attacker_role": "attacker", "attacker_elite": true,
                    "attacker_is_army": false, "attacker_army_members": 0,
                    "attacker_hp_before": 5, "attacker_hp_after": 3, "attacker_cost": 70,
                    "defender_id": 2, "defender_owner": 2, "defender_kind": "Spearman",
                    "defender_role": "defender", "defender_elite": false,
                    "defender_is_army": false, "defender_army_members": 0,
                    "defender_hp_before": 2, "defender_hp_after": 0, "defender_cost": 20,
                    "tile": [5, 5], "city_id": 10, "city_owner": 2,
                    "attack_power": 40, "defense_power": 20, "rounds": 3,
                    "winner": "attacker", "attacker_occupied_tile": true,
                    "city_captured": true, "missile_expended": false,
                    "powerless_combat": false, "defender_was_damaged": true,
                    "leader_eligible": true, "leader_roll_denominator": 16,
                    "leader_created_id": 99
                }),
            ),
            &mut out,
        );
        analyzer.on_event(
            &make_event(
                10,
                2,
                "military.leader",
                json!({
                    "owner": 1, "action": "spawned", "unit_id": 1, "leader_id": 99,
                    "kind": "Knight", "attacked": true, "denominator": 16, "heroic_epic": false
                }),
            ),
            &mut out,
        );
        analyzer.on_event(
            &make_event(
                10,
                3,
                "military.leader",
                json!({
                    "owner": 1, "action": "formed_army", "unit_id": 99, "leader_id": 99,
                    "army_id": 100, "city_id": 1, "can_form_army": true, "members_now": 0, "capacity": 3
                }),
            ),
            &mut out,
        );
        analyzer.on_event(
            &make_event(
                10,
                4,
                "military.leader",
                json!({
                    "owner": 1, "action": "army_member_loaded", "unit_id": 1, "army_id": 100,
                    "kind": "Knight", "members_now": 1, "capacity": 3
                }),
            ),
            &mut out,
        );
        analyzer.on_event(
            &make_event(
                10,
                5,
                "military.bombard",
                json!({
                    "attacker_id": 3, "attacker_owner": 1, "attacker_kind": "Artillery",
                    "attacker_role": "bombard", "precision": false, "tile": [5, 5],
                    "city_id": 10, "target_owner": 2, "target_kind": "city_unit",
                    "intercepted": false, "hp_damage": 2, "units_hit": 1, "units_killed": 0,
                    "population_lost": 0, "terrain_destroyed": [], "missile_expended": false
                }),
            ),
            &mut out,
        );
        analyzer.on_event(
            &make_event(
                10,
                6,
                "military.nuclear_strike",
                json!({
                    "owner": 1, "weapon_id": 50, "kind": "ICBM", "action": "nuke_city",
                    "tile": [8, 8], "city_id": 11, "target_owner": 2,
                    "units_killed": 4, "cities_hit": 1, "fallout_tiles": 3
                }),
            ),
            &mut out,
        );

        let totals = &out.metrics["military.totals"];
        assert_eq!(totals["combats"], 1);
        assert_eq!(totals["bombards"], 1);
        assert_eq!(totals["nuclear_strikes"], 1);
        assert_eq!(totals["eligible_leader_kills"], 1);
        assert_eq!(totals["leaders_spawned"], 1);
        assert_eq!(totals["armies_formed"], 1);

        let nations = out.metrics["military.nations"].as_array().unwrap();
        let civ1 = nations.iter().find(|n| n["id"] == 1).unwrap();
        assert_eq!(civ1["combat"]["city_captures"], 1);
        assert_eq!(civ1["combat"]["softened_city_attacks"], 1);
        assert_eq!(civ1["leaders_and_armies"]["leaders_spawned"], 1);
        assert_eq!(civ1["leaders_and_armies"]["armies_formed"], 1);
        assert_eq!(civ1["leaders_and_armies"]["army_members_loaded"], 1);
        assert_eq!(civ1["bombard"]["hp_damage"], 2);
        assert_eq!(civ1["air_and_missiles"]["nuclear_units_killed"], 4);
    }

    #[test]
    fn raises_findings_for_unused_unlocked_role_and_empty_army() {
        let mut analyzer = Military::default();
        let mut out = AnalysisOutput::default();
        for turn in 1..=8 {
            out.context("military", turn, None, Some((turn as u64, true)));
            analyzer.on_event(
                &make_event(
                    turn,
                    turn as u64,
                    "military.turn_summary",
                    json!({
                        "owner": 1,
                        "at_war": true,
                        "war_enemies": [2],
                        "on_war_footing": true,
                        "cities": 4,
                        "army_limit": 1,
                        "armies_owned": 1,
                        "leaders_owned": 0,
                        "elite_land_units": 1,
                        "unlocked_roles": ["defender", "attacker", "bombard"],
                        "units_by_role": {"attacker": 4, "army": 1},
                        "dispatch_counts": {"fallback_explore": 4, "city_defender": 1},
                        "idle_wartime_military": 4,
                        "assault_transports_dispatched": 0,
                        "armies": [{
                            "id": 77, "members": 0, "capacity": 3, "hp": 1, "max_hp": 1,
                            "tile": [1, 1], "dispatch": "fallback_explore",
                            "moved": false, "spent_moves": false
                        }]
                    }),
                ),
                &mut out,
            );
        }
        assert!(out.findings.iter().any(|f| f.kind == "war_army_idle"));
        assert!(out.findings.iter().any(|f| f.kind == "army_underutilized"));
        assert!(
            out.findings
                .iter()
                .any(|f| f.kind == "unlocked_role_unused_in_war")
        );
    }
}
