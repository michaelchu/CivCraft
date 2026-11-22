//! Passive military, warfare, Great Leader, and Army instrumentation.
//!
//! All emitters check `trace.wants::<T>()` before computing expensive fields so
//! runs without military analytics or tracing pay only a branch check.

use super::*;
use crate::rules::Genus;
use serde::Serialize;
use std::collections::BTreeMap;

/// Classify a unit kind into a coarse military role for force-composition,
/// production, and combat reporting.
pub(crate) fn military_unit_role(kind: UnitKind) -> &'static str {
    if kind.is_nuclear_weapon() {
        "nuclear"
    } else if kind.is_missile() {
        "missile"
    } else if kind.is_aircraft() {
        if kind.bombard_strength() > 0 && kind.stats().attack == 0 {
            "bomber"
        } else {
            "fighter"
        }
    } else if kind == UnitKind::Army {
        "army"
    } else if kind == UnitKind::Leader {
        "leader"
    } else if kind == UnitKind::Paratrooper {
        "paratrooper"
    } else if kind.is_naval() {
        if kind.transport_capacity() > 0 && kind != UnitKind::Submarine {
            "transport"
        } else {
            "naval_combat"
        }
    } else if kind.bombard_strength() > 0 && kind.stats().attack == 0 {
        "bombard"
    } else if kind.is_military() {
        if kind.stats().defense > kind.stats().attack {
            "defender"
        } else {
            "attacker"
        }
    } else if kind.is_civilian() {
        "civilian"
    } else {
        "other"
    }
}

pub(crate) fn military_role_for_unit_name(name: &str) -> &'static str {
    UnitKind::all()
        .find(|kind| kind.name() == name)
        .map_or("other", military_unit_role)
}

pub(crate) fn production_role(production: Production) -> &'static str {
    match production {
        Production::Unit(kind) => military_unit_role(kind),
        Production::Building(building) => match building.data().genus {
            Genus::GreatWonder | Genus::SmallWonder => "wonder",
            _ => "building",
        },
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CombatEvent {
    pub attacker_id: u32,
    pub attacker_owner: usize,
    pub attacker_kind: &'static str,
    pub attacker_role: &'static str,
    pub attacker_elite: bool,
    pub attacker_is_army: bool,
    pub attacker_army_members: usize,
    pub attacker_hp_before: i32,
    pub attacker_hp_after: i32,
    pub attacker_cost: i32,
    pub defender_id: u32,
    pub defender_owner: usize,
    pub defender_kind: &'static str,
    pub defender_role: &'static str,
    pub defender_elite: bool,
    pub defender_is_army: bool,
    pub defender_army_members: usize,
    pub defender_hp_before: i32,
    pub defender_hp_after: i32,
    pub defender_cost: i32,
    pub tile: (i32, i32),
    pub city_id: Option<u32>,
    pub city_owner: Option<usize>,
    pub attack_power: i32,
    pub defense_power: i32,
    pub rounds: i32,
    pub winner: &'static str,
    pub attacker_occupied_tile: bool,
    pub city_captured: bool,
    pub missile_expended: bool,
    pub powerless_combat: bool,
    pub defender_was_damaged: bool,
    pub leader_eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_ineligible_reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_roll_denominator: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_created_id: Option<u32>,
}

impl trace::Payload for CombatEvent {
    const KIND: &'static str = "military.combat";
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct LeaderEvent {
    pub owner: usize,
    pub action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub army_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denominator: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heroic_epic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shields_saved: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_form_army: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members_now: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<usize>,
}

impl trace::Payload for LeaderEvent {
    const KIND: &'static str = "military.leader";
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BombardEvent {
    pub attacker_id: u32,
    pub attacker_owner: usize,
    pub attacker_kind: &'static str,
    pub attacker_role: &'static str,
    pub precision: bool,
    pub tile: (i32, i32),
    pub city_id: Option<u32>,
    pub target_owner: Option<usize>,
    pub target_kind: &'static str,
    pub intercepted: bool,
    pub hp_damage: i32,
    pub units_hit: usize,
    pub units_killed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building_destroyed: Option<&'static str>,
    pub population_lost: i32,
    pub terrain_destroyed: Vec<&'static str>,
    pub missile_expended: bool,
}

impl trace::Payload for BombardEvent {
    const KIND: &'static str = "military.bombard";
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AirMissionEvent {
    pub owner: usize,
    pub unit_id: u32,
    pub kind: &'static str,
    pub role: &'static str,
    pub mission: &'static str,
    pub from: (i32, i32),
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<(i32, i32)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_owner: Option<usize>,
    pub succeeded: bool,
}

impl trace::Payload for AirMissionEvent {
    const KIND: &'static str = "military.air_mission";
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct NuclearStrikeEvent {
    pub owner: usize,
    pub weapon_id: u32,
    pub kind: &'static str,
    pub action: &'static str,
    pub tile: (i32, i32),
    pub city_id: Option<u32>,
    pub target_owner: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intercepted_by_sdi: Option<usize>,
    pub units_killed: i32,
    pub cities_hit: usize,
    pub fallout_tiles: usize,
}

impl trace::Payload for NuclearStrikeEvent {
    const KIND: &'static str = "military.nuclear_strike";
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MilitaryProductionDecisionEvent {
    pub owner: usize,
    pub city_id: u32,
    pub at_war: bool,
    pub on_war_footing: bool,
    pub upkeep_constrained: bool,
    pub grave_danger: usize,
    pub emergency: bool,
    pub selected: String,
    pub selected_role: &'static str,
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub military_candidate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub military_candidate_role: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub military_stage: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub military_want: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressed_reason: Option<&'static str>,
}

impl trace::Payload for MilitaryProductionDecisionEvent {
    const KIND: &'static str = "military.production_decision";
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CoordinatedObjectiveStatus {
    pub city_id: u32,
    pub target_owner: usize,
    pub at_war_with_target: bool,
    pub staging_troops: usize,
    pub force_ready: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ArmyTurnStatus {
    pub id: u32,
    pub members: usize,
    pub capacity: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub tile: (i32, i32),
    pub dispatch: &'static str,
    pub moved: bool,
    pub spent_moves: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct MilitaryTurnSummaryEvent {
    pub owner: usize,
    pub at_war: bool,
    pub war_enemies: Vec<usize>,
    pub on_war_footing: bool,
    pub cities: usize,
    pub army_limit: usize,
    pub armies_owned: usize,
    pub leaders_owned: usize,
    pub elite_land_units: usize,
    pub unlocked_roles: Vec<&'static str>,
    pub units_by_role: BTreeMap<&'static str, usize>,
    pub dispatch_counts: BTreeMap<&'static str, u32>,
    pub idle_wartime_military: u32,
    pub assault_transports_dispatched: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinated_objective: Option<CoordinatedObjectiveStatus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub armies: Vec<ArmyTurnStatus>,
}

impl trace::Payload for MilitaryTurnSummaryEvent {
    const KIND: &'static str = "military.turn_summary";
}

impl Simulation {
    pub(in crate::sim) fn trace_combat(&mut self, event: CombatEvent) {
        let mut entities = vec![
            trace::Entity::Unit(event.attacker_id),
            trace::Entity::Unit(event.defender_id),
            trace::Entity::Civilization(event.attacker_owner),
        ];
        if event.defender_owner != event.attacker_owner {
            entities.push(trace::Entity::Civilization(event.defender_owner));
        }
        if let Some(city_id) = event.city_id {
            entities.push(trace::Entity::City(city_id));
        }
        if let Some(leader_id) = event.leader_created_id {
            entities.push(trace::Entity::Unit(leader_id));
        }
        self.trace.emit(self.turn, || entities, || event);
    }

    pub(in crate::sim) fn trace_leader_event(&mut self, event: LeaderEvent) {
        let mut entities = vec![trace::Entity::Civilization(event.owner)];
        if let Some(id) = event.unit_id {
            entities.push(trace::Entity::Unit(id));
        }
        if let Some(id) = event.leader_id
            && Some(id) != event.unit_id
        {
            entities.push(trace::Entity::Unit(id));
        }
        if let Some(id) = event.army_id
            && Some(id) != event.unit_id
            && Some(id) != event.leader_id
        {
            entities.push(trace::Entity::Unit(id));
        }
        if let Some(id) = event.city_id {
            entities.push(trace::Entity::City(id));
        }
        self.trace.emit(self.turn, || entities, || event);
    }

    pub(in crate::sim) fn trace_bombard(&mut self, event: BombardEvent) {
        let mut entities = vec![
            trace::Entity::Unit(event.attacker_id),
            trace::Entity::Civilization(event.attacker_owner),
        ];
        if let Some(owner) = event.target_owner
            && owner != event.attacker_owner
        {
            entities.push(trace::Entity::Civilization(owner));
        }
        if let Some(city_id) = event.city_id {
            entities.push(trace::Entity::City(city_id));
        }
        self.trace.emit(self.turn, || entities, || event);
    }

    pub(in crate::sim) fn trace_air_mission(&mut self, event: AirMissionEvent) {
        let mut entities = vec![
            trace::Entity::Civilization(event.owner),
            trace::Entity::Unit(event.unit_id),
        ];
        if let Some(target_owner) = event.target_owner
            && target_owner != event.owner
        {
            entities.push(trace::Entity::Civilization(target_owner));
        }
        self.trace.emit(self.turn, || entities, || event);
    }

    pub(in crate::sim) fn trace_nuclear_strike(&mut self, event: NuclearStrikeEvent) {
        let mut entities = vec![
            trace::Entity::Civilization(event.owner),
            trace::Entity::Unit(event.weapon_id),
        ];
        if let Some(target_owner) = event.target_owner
            && target_owner != event.owner
        {
            entities.push(trace::Entity::Civilization(target_owner));
        }
        if let Some(sdi_owner) = event.intercepted_by_sdi
            && sdi_owner != event.owner
            && Some(sdi_owner) != event.target_owner
        {
            entities.push(trace::Entity::Civilization(sdi_owner));
        }
        if let Some(city_id) = event.city_id {
            entities.push(trace::Entity::City(city_id));
        }
        self.trace.emit(self.turn, || entities, || event);
    }

    pub(in crate::sim) fn trace_military_production_decision(
        &mut self,
        event: MilitaryProductionDecisionEvent,
    ) {
        let entities = vec![
            trace::Entity::Civilization(event.owner),
            trace::Entity::City(event.city_id),
        ];
        self.trace.emit(self.turn, || entities, || event);
    }

    pub(in crate::sim) fn trace_military_turn_summary(&mut self, event: MilitaryTurnSummaryEvent) {
        let mut entities = vec![trace::Entity::Civilization(event.owner)];
        for &enemy in &event.war_enemies {
            entities.push(trace::Entity::Civilization(enemy));
        }
        for army in &event.armies {
            entities.push(trace::Entity::Unit(army.id));
        }
        self.trace.emit(self.turn, || entities, || event);
    }

    /// Military roles for which `owner` currently has at least one buildable unit
    /// in any owned city.
    pub(crate) fn unlocked_military_roles(&self, owner: usize) -> Vec<&'static str> {
        let owned_cities: Vec<&City> = self.cities_owned_by(owner).collect();
        if owned_cities.is_empty() {
            return Vec::new();
        }
        let mut roles = std::collections::BTreeSet::new();
        for kind in UnitKind::all() {
            let role = military_unit_role(kind);
            if matches!(role, "civilian" | "other" | "leader" | "army") || roles.contains(role) {
                continue;
            }
            if owned_cities
                .iter()
                .any(|city| self.unit_buildable(city, kind))
            {
                roles.insert(role);
            }
        }
        roles.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::Payload;
    use crate::sim::veterancy::selected_veteran_system;
    use std::collections::BTreeSet;

    fn traced_game() -> Simulation {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(true);
        }
        game.trace = trace::Collector::enabled(BTreeSet::from(["military.*".into()]), 512);
        game
    }

    fn make_elite(game: &mut Simulation, id: u32) {
        let kind = game.unit_by_id(id).unwrap().kind;
        let top = selected_veteran_system(kind).levels.len() - 1;
        game.units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap()
            .veteran_level = top;
    }

    #[test]
    fn combat_and_leader_events_record_eligibility_spawns_and_ineligibility_reasons() {
        let mut game = traced_game();
        game.sign_pact(HUMAN, 1, Pact::War);
        game.create_city(HUMAN, 10, 20);
        game.create_city(1, 20, 20);

        // Elite knight vs barbarian warrior: real kill, ineligible due to barbarian_loser.
        let elite = game.spawn_unit(HUMAN, UnitKind::ModernArmor, 10, 20);
        make_elite(&mut game, elite);
        let target = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        let barb = game.spawn_unit(BARBARIANS, UnitKind::Warrior, target.0, target.1);
        game.resolve_combat(elite, barb, target.0, target.1, None);

        let events = game.trace.drain();
        let combat = events
            .iter()
            .find(|e| e.kind == CombatEvent::KIND)
            .expect("military.combat emitted");
        assert_eq!(combat.payload["winner"], "attacker");
        assert_eq!(combat.payload["attacker_elite"], true);
        assert_eq!(combat.payload["leader_eligible"], false);
        assert_eq!(
            combat.payload["leader_ineligible_reason"],
            "barbarian_loser"
        );

        // Elite kill vs regular rival civ: eligible roll attempted.
        let pos = (
            game.unit_by_id(elite).unwrap().x,
            game.unit_by_id(elite).unwrap().y,
        );
        let next = game.map_wrap.civ3_neighbor(pos.0, pos.1, 2).unwrap();
        let enemy = game.spawn_unit(1, UnitKind::Warrior, next.0, next.1);
        game.units.iter_mut().find(|u| u.id == elite).unwrap().hp = 10;
        game.resolve_combat(elite, enemy, next.0, next.1, None);
        let events = game.trace.drain();
        let combat = events
            .iter()
            .find(|e| e.kind == CombatEvent::KIND)
            .expect("second combat emitted");
        assert_eq!(combat.payload["leader_eligible"], true);
        assert_eq!(combat.payload["leader_roll_denominator"], 16);
        assert!(
            events.iter().any(|e| {
                e.kind == LeaderEvent::KIND
                    && matches!(
                        e.payload["action"].as_str(),
                        Some("spawned" | "roll_missed")
                    )
            }),
            "leader roll outcome recorded"
        );
    }

    #[test]
    fn leader_army_formation_loading_and_army_combat_are_traced() {
        let mut game = traced_game();
        game.sign_pact(HUMAN, 1, Pact::War);
        for i in 0..4 {
            game.create_city(HUMAN, 10 + i * 2, 20);
        }
        game.create_city(1, 25, 20);
        let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 10, 20);
        assert!(game.form_army(leader));
        let army = game
            .units
            .iter()
            .find(|u| u.kind == UnitKind::Army)
            .unwrap()
            .id;
        let armor = game.spawn_unit(HUMAN, UnitKind::ModernArmor, 10, 20);
        assert!(game.board_specific_ship(armor, army));

        let target = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        let defender = game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.resolve_combat(army, defender, target.0, target.1, None);

        let events = game.trace.drain();
        assert!(events.iter().any(|e| {
            e.kind == LeaderEvent::KIND
                && e.payload["action"] == "formed_army"
                && e.payload["army_id"] == army
        }));
        assert!(events.iter().any(|e| {
            e.kind == LeaderEvent::KIND
                && e.payload["action"] == "army_member_loaded"
                && e.payload["army_id"] == army
                && e.payload["members_now"] == 1
        }));
        let combat = events
            .iter()
            .find(|e| e.kind == CombatEvent::KIND)
            .expect("army combat recorded");
        assert_eq!(combat.payload["attacker_is_army"], true);
        assert_eq!(combat.payload["attacker_army_members"], 1);
    }

    #[test]
    fn bombard_and_nuclear_strike_emit_military_events() {
        let mut game = traced_game();
        game.sign_pact(HUMAN, 1, Pact::War);
        game.create_city(HUMAN, 10, 20);
        let target = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        let artillery = game.spawn_unit(HUMAN, UnitKind::Artillery, 10, 20);
        assert_eq!(
            game.try_ranged_bombard(artillery, target.0, target.1),
            MoveResult::Fought
        );

        let icbm_target = (20, 20);
        game.create_city(1, icbm_target.0, icbm_target.1);
        game.spawn_unit(1, UnitKind::Warrior, icbm_target.0, icbm_target.1);
        let spotter_tile = game
            .map_wrap
            .civ3_neighbor(icbm_target.0, icbm_target.1, 2)
            .unwrap();
        game.spawn_unit(HUMAN, UnitKind::Warrior, spotter_tile.0, spotter_tile.1);
        game.refresh_visibility();
        let icbm = game.spawn_unit(HUMAN, UnitKind::ICBM, 10, 20);
        assert!(game.launch_nuclear_strike(icbm, icbm_target.0, icbm_target.1));

        let events = game.trace.drain();
        assert!(events.iter().any(|e| {
            e.kind == BombardEvent::KIND
                && e.payload["attacker_role"] == "bombard"
                && e.payload["target_kind"] == "field_units"
        }));
        assert!(events.iter().any(|e| {
            e.kind == NuclearStrikeEvent::KIND
                && e.payload["kind"] == "ICBM"
                && e.payload["cities_hit"] == 1
        }));
    }

    #[test]
    fn ai_turn_emits_military_turn_summary_with_army_and_unlocked_roles() {
        let mut game = traced_game();
        game.players[1].ai = true;
        game.players[1].known = Tech::all().collect();
        game.sign_pact(1, HUMAN, Pact::War);
        for i in 0..4 {
            game.create_city(1, 10 + i * 2, 20);
        }
        game.tile_mut(10, 20).terrain = Terrain::Desert;
        game.tile_mut(10, 20).resource = Some(Resource::Oil);
        game.tile_mut(12, 20).terrain = Terrain::Forest;
        game.tile_mut(12, 20).resource = Some(Resource::Rubber);
        game.tile_mut(14, 20).terrain = Terrain::Hills;
        game.tile_mut(14, 20).resource = Some(Resource::Aluminum);
        for x in 10..=16 {
            game.tile_mut(x, 20).road = true;
        }
        game.create_city(HUMAN, 24, 20);
        let army = game.spawn_unit(1, UnitKind::Army, 10, 20);
        let tank = game.spawn_unit(1, UnitKind::Tank, 10, 20);
        assert!(game.board_specific_ship(tank, army));
        game.run_ai_player_planning(1);

        let events = game.trace.drain();
        let summary = events
            .iter()
            .find(|e| e.kind == MilitaryTurnSummaryEvent::KIND)
            .expect("military.turn_summary emitted");
        assert_eq!(summary.payload["owner"], 1);
        assert_eq!(summary.payload["at_war"], true);
        assert_eq!(summary.payload["armies_owned"], 1);
        assert_eq!(summary.payload["armies"][0]["id"], army);
        assert_eq!(summary.payload["armies"][0]["members"], 1);
        let unlocked = summary.payload["unlocked_roles"].as_array().unwrap();
        assert!(unlocked.iter().any(|r| r == "bombard"));
        assert!(unlocked.iter().any(|r| r == "bomber"));
    }
}
