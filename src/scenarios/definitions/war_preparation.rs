//! End-to-end headless scenarios proving AI war preparation across distant
//! land fronts, overseas naval invasions, and provoked revenge wars.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        HUMAN, Pact, Production, Simulation, UnitKind, WarObjectiveKind,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
    },
    world::{
        GeneratedMap, Resource, Terrain,
        topology::{EIGHT_WAY_DIRS, offset_native_by_map},
    },
};
use serde_json::{Value, json};

const ATTACKER: usize = 1;
const DISTANT_DEADLINE: u32 = 60;
const OVERSEAS_DEADLINE: u32 = 75;
const REVENGE_DEADLINE: u32 = 20;

pub(super) const DISTANT_OFFENSIVE: Scenario = Scenario::new(
    "distant-offensive",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_distant,
    },
);

pub(super) const OVERSEAS_OFFENSIVE: Scenario = Scenario::new(
    "overseas-offensive",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_overseas,
    },
);

pub(super) const PROVOKED_REVENGE: Scenario = Scenario::new(
    "provoked-revenge-prep",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_revenge,
    },
);

fn setup_distant(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "inca",
                technologies: None,
            },
            CivPreset {
                id: "france",
                technologies: Some(&[Tech::WARRIOR_CODE, Tech::BRONZE_WORKING, Tech::IRON_WORKING]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(40, -2000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    let home = (6, 24);
    let front = offset_native_by_map(home.0, home.1, 14, 0);
    world.tile_surface(
        home,
        TileSurface {
            terrain: Some(Terrain::Hills),
            resource: Some(Some(Resource::Iron)),
            ..Default::default()
        },
    );
    let home_city = world.city(ATTACKER, home, 4);
    let target = world.city(HUMAN, front, 2);
    world.city_workers(home_city, 4);
    world.city_workers(target, 2);
    world.production_stock(target, Production::Building(Building::COINAGE), 0);
    world.unit(ATTACKER, UnitKind::Spearman, home);
    let defender = world.unit(HUMAN, UnitKind::Spearman, front);
    world.treasury(ATTACKER, 120);
    world.diplomacy(ATTACKER, HUMAN, Pact::Peace, 1, [-5000, -5000]);
    for dx in 0..=15 {
        for dy in -3..=3 {
            world.mark_known(ATTACKER, offset_native_by_map(home.0, home.1, dx, dy));
        }
    }
    world.finish();
    (
        game,
        Box::new(DistantOffensive {
            home,
            target,
            defender,
            staging: None,
            land_objective_seen: false,
            first_assembly: None,
            first_war: None,
            captured: None,
            guarded_while_preparing: true,
            defender_seen_during_peace: false,
        }),
    )
}

struct DistantOffensive {
    home: (i32, i32),
    target: u32,
    defender: u32,
    staging: Option<(i32, i32)>,
    land_objective_seen: bool,
    first_assembly: Option<u32>,
    first_war: Option<u32>,
    captured: Option<u32>,
    guarded_while_preparing: bool,
    defender_seen_during_peace: bool,
}

impl ScenarioRun for DistantOffensive {
    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let at_war = game.at_war(ATTACKER, HUMAN);
        let defender_present = game.unit_by_id(self.defender).is_some();
        let target_owner = game.city_by_id(self.target).map(|city| city.owner);
        let objective = game.coordinated_war_objective(ATTACKER);
        let staged: Vec<_> = game
            .units()
            .iter()
            .filter(|unit| {
                unit.owner == ATTACKER
                    && matches!(unit.kind, UnitKind::Archer | UnitKind::Swordsman)
                    && objective.is_some_and(|goal| (unit.x, unit.y) == goal.staging)
            })
            .map(|unit| unit.id)
            .collect();
        let garrison = game.units().iter().any(|unit| {
            unit.owner == ATTACKER
                && unit.kind.is_military()
                && unit.kind.stats().defense > 0
                && (unit.x, unit.y) == self.home
        });
        if !at_war && self.first_war.is_none() {
            self.defender_seen_during_peace |= defender_present;
            if let Some(goal) = objective {
                self.guarded_while_preparing &= garrison;
                if goal.city == self.target && goal.kind == WarObjectiveKind::Land {
                    self.land_objective_seen = true;
                    if staged.len() >= 2 {
                        self.staging = Some(goal.staging);
                        self.first_assembly.get_or_insert(turn);
                    }
                }
            }
        }
        if at_war {
            self.first_war.get_or_insert(turn);
        }
        if target_owner == Some(ATTACKER) && !defender_present && turn <= DISTANT_DEADLINE {
            self.captured.get_or_insert(turn);
        }
        json!({
            "objective": objective,
            "staged_attackers": staged,
            "garrison": garrison,
            "at_war": at_war,
            "target_owner": target_owner,
        })
    }

    fn finish(self: Box<Self>, _: u32) -> Vec<Check> {
        let assembled_before_war = self
            .first_assembly
            .zip(self.first_war)
            .is_some_and(|(assembled, declared)| assembled < declared);
        vec![
            Check {
                name: "distant_objective_selected",
                passed: self.land_objective_seen,
                detail: format!("Selected distant Land objective; staging {:?}", self.staging),
            },
            Check {
                name: "distant_offensive_built_and_staged_before_declaration",
                passed: assembled_before_war,
                detail: format!(
                    "Staging {:?}, built force assembled on {:?}, war on {:?}",
                    self.staging, self.first_assembly, self.first_war
                ),
            },
            Check {
                name: "distant_offensive_preserved_home_garrison",
                passed: self.guarded_while_preparing && self.first_assembly.is_some(),
                detail: "Home city retained a garrison throughout distant preparation".into(),
            },
            Check {
                name: "distant_offensive_captured_defended_city",
                passed: self.defender_seen_during_peace && self.captured.is_some(),
                detail: format!(
                    "Defender seen during peace; captured on {:?} within {DISTANT_DEADLINE} turns",
                    self.captured
                ),
            },
        ]
    }
}

fn setup_overseas(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "inca",
                technologies: None,
            },
            CivPreset {
                id: "france",
                technologies: Some(&[
                    Tech::WARRIOR_CODE,
                    Tech::BRONZE_WORKING,
                    Tech::IRON_WORKING,
                    Tech::MAP_MAKING,
                ]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(40, -2000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::Coast),
        ..Default::default()
    });
    let home = (10, 24);
    let target_pos = offset_native_by_map(home.0, home.1, 8, 0);
    // Home island: 5 productive land tiles with Iron, leaving east/south coastal access.
    let home_island = [
        home,
        offset_native_by_map(home.0, home.1, -1, 0),
        offset_native_by_map(home.0, home.1, -1, -1),
        offset_native_by_map(home.0, home.1, 0, -1),
        offset_native_by_map(home.0, home.1, -2, 0),
    ];
    for tile in home_island {
        world.tile_surface(
            tile,
            TileSurface {
                terrain: Some(Terrain::GrasslandShield),
                road: Some(true),
                ..Default::default()
            },
        );
    }
    world.tile_surface(
        home,
        TileSurface {
            terrain: Some(Terrain::Hills),
            resource: Some(Some(Resource::Iron)),
            road: Some(true),
            ..Default::default()
        },
    );
    // Target island: 3 land tiles separated by sea from home island.
    let target_island = [
        target_pos,
        offset_native_by_map(target_pos.0, target_pos.1, 1, 0),
        offset_native_by_map(target_pos.0, target_pos.1, 0, 1),
    ];
    for tile in target_island {
        world.tile_surface(
            tile,
            TileSurface {
                terrain: Some(Terrain::Grassland),
                ..Default::default()
            },
        );
    }

    let home_city = world.city(ATTACKER, home, 5);
    let target = world.city(HUMAN, target_pos, 2);
    world.city_workers(home_city, 5);
    world.city_workers(target, 2);
    world.production_stock(target, Production::Building(Building::COINAGE), 0);
    world.unit(ATTACKER, UnitKind::Spearman, home);
    let defender = world.unit(HUMAN, UnitKind::Spearman, target_pos);
    world.treasury(ATTACKER, 200);
    world.diplomacy(ATTACKER, HUMAN, Pact::Peace, 1, [-5000, -5000]);
    for dx in -3..=11 {
        for dy in -4..=4 {
            world.mark_known(ATTACKER, offset_native_by_map(home.0, home.1, dx, dy));
        }
    }
    world.finish();
    (
        game,
        Box::new(OverseasOffensive {
            home,
            target,
            defender,
            staging: None,
            naval_objective_seen: false,
            built_transport: false,
            first_staged_fleet: None,
            first_war: None,
            captured: None,
            guarded_while_preparing: true,
            defender_seen_during_peace: false,
        }),
    )
}

struct OverseasOffensive {
    home: (i32, i32),
    target: u32,
    defender: u32,
    staging: Option<(i32, i32)>,
    naval_objective_seen: bool,
    built_transport: bool,
    first_staged_fleet: Option<u32>,
    first_war: Option<u32>,
    captured: Option<u32>,
    guarded_while_preparing: bool,
    defender_seen_during_peace: bool,
}

impl ScenarioRun for OverseasOffensive {
    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        let at_war = game.at_war(ATTACKER, HUMAN);
        let defender_present = game.unit_by_id(self.defender).is_some();
        let target_owner = game.city_by_id(self.target).map(|city| city.owner);
        let objective = game.coordinated_war_objective(ATTACKER);
        let transports: Vec<_> = game
            .units()
            .iter()
            .filter(|unit| {
                unit.owner == ATTACKER
                    && matches!(
                        unit.kind,
                        UnitKind::Galley
                            | UnitKind::Caravel
                            | UnitKind::Galleon
                            | UnitKind::Transport
                    )
            })
            .map(|unit| (unit.id, unit.x, unit.y))
            .collect();
        if !transports.is_empty() {
            self.built_transport = true;
        }
        let staged_cargo: usize = objective.map_or(0, |goal| {
            let staged_boats: Vec<u32> = transports
                .iter()
                .filter(|&&(_, x, y)| (x, y) == goal.staging)
                .map(|&(id, _, _)| id)
                .collect();
            game.units()
                .iter()
                .filter(|unit| {
                    unit.owner == ATTACKER
                        && unit
                            .transport
                            .is_some_and(|boat| staged_boats.contains(&boat))
                        && matches!(unit.kind, UnitKind::Archer | UnitKind::Swordsman)
                })
                .count()
        });
        let garrison = game.units().iter().any(|unit| {
            unit.owner == ATTACKER
                && unit.kind.is_military()
                && !unit.kind.is_naval()
                && unit.kind.stats().defense > 0
                && (unit.x, unit.y) == self.home
        });
        if !at_war && self.first_war.is_none() {
            self.defender_seen_during_peace |= defender_present;
            if let Some(goal) = objective {
                self.guarded_while_preparing &= garrison;
                if goal.city == self.target && goal.kind == WarObjectiveKind::Naval {
                    self.naval_objective_seen = true;
                    if staged_cargo >= 2 {
                        self.staging = Some(goal.staging);
                        self.first_staged_fleet.get_or_insert(turn);
                    }
                }
            }
        }
        if at_war {
            self.first_war.get_or_insert(turn);
        }
        if target_owner == Some(ATTACKER) && !defender_present && turn <= OVERSEAS_DEADLINE {
            self.captured.get_or_insert(turn);
        }
        json!({
            "objective": objective,
            "transports": transports.len(),
            "staged_cargo": staged_cargo,
            "garrison": garrison,
            "at_war": at_war,
            "target_owner": target_owner,
        })
    }

    fn finish(self: Box<Self>, _: u32) -> Vec<Check> {
        let staged_before_war = self
            .first_staged_fleet
            .zip(self.first_war)
            .is_some_and(|(staged, declared)| staged < declared);
        vec![
            Check {
                name: "overseas_naval_objective_selected",
                passed: self.naval_objective_seen,
                detail: format!("Selected Naval objective; staging {:?}", self.staging),
            },
            Check {
                name: "overseas_built_transports_and_staged_before_declaration",
                passed: self.built_transport && staged_before_war,
                detail: format!(
                    "Built transport: {}, fleet staged on {:?}, war on {:?}",
                    self.built_transport, self.first_staged_fleet, self.first_war
                ),
            },
            Check {
                name: "overseas_preserved_home_garrison",
                passed: self.guarded_while_preparing && self.first_staged_fleet.is_some(),
                detail: "Muster port retained a defender throughout naval preparation".into(),
            },
            Check {
                name: "overseas_landed_and_captured_defended_city",
                passed: self.defender_seen_during_peace && self.captured.is_some(),
                detail: format!(
                    "Defender seen during peace; overseas capture on {:?} within {OVERSEAS_DEADLINE} turns",
                    self.captured
                ),
            },
        ]
    }
}

fn setup_revenge(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "inca",
                technologies: None,
            },
            CivPreset {
                id: "france",
                technologies: Some(&[Tech::WARRIOR_CODE, Tech::BRONZE_WORKING, Tech::IRON_WORKING]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    world.start_at(40, -2000);
    world.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    let home = (10, 24);
    let front = offset_native_by_map(home.0, home.1, 6, 0);
    world.tile_surface(
        home,
        TileSurface {
            terrain: Some(Terrain::Hills),
            resource: Some(Some(Resource::Iron)),
            road: Some(true),
            ..Default::default()
        },
    );
    let home_city = world.city(ATTACKER, home, 4);
    let target = world.city(HUMAN, front, 2);
    world.city_workers(home_city, 4);
    world.city_workers(target, 2);
    world.production_stock(target, Production::Building(Building::COINAGE), 0);
    world.unit(ATTACKER, UnitKind::Spearman, home);
    world.unit(HUMAN, UnitKind::Spearman, front);
    world.treasury(ATTACKER, 100);
    // Negative trust + fresh provocation triggers WarReason::Behavior (not Hatred).
    world.diplomacy(ATTACKER, HUMAN, Pact::Peace, 1, [-500, -500]);
    world.provocation(ATTACKER, HUMAN);
    for dx in 0..=8 {
        for dy in -3..=3 {
            world.mark_known(ATTACKER, offset_native_by_map(home.0, home.1, dx, dy));
        }
    }
    world.finish();
    (
        game,
        Box::new(ProvokedRevenge {
            behavior_started: None,
            objective_seen_before_war: false,
            deferral_turns: 0,
            first_war: None,
            offensive_built: false,
        }),
    )
}

struct ProvokedRevenge {
    behavior_started: Option<u32>,
    objective_seen_before_war: bool,
    deferral_turns: u32,
    first_war: Option<u32>,
    offensive_built: bool,
}

impl ScenarioRun for ProvokedRevenge {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["diplomacy.transition"]
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        for event in game.trace.pending_events() {
            if event.kind != "diplomacy.transition"
                || event.payload["actor"] != ATTACKER
                || event.payload["other"] != HUMAN
            {
                continue;
            }
            if event.payload["action"] == "war_plan_started"
                && event.payload["reason"] == "behavior"
            {
                self.behavior_started.get_or_insert(turn);
            }
            if event.payload["action"] == "war_deferred"
                && event.payload["reason"] == "assembling_force"
            {
                self.deferral_turns += 1;
            }
        }
        let at_war = game.at_war(ATTACKER, HUMAN);
        let objective = game.coordinated_war_objective(ATTACKER);
        if !at_war && objective.is_some() {
            self.objective_seen_before_war = true;
        }
        if at_war {
            self.first_war.get_or_insert(turn);
        }
        let attackers = game
            .units()
            .iter()
            .filter(|unit| {
                unit.owner == ATTACKER
                    && matches!(unit.kind, UnitKind::Archer | UnitKind::Swordsman)
            })
            .count();
        if attackers > 0 {
            self.offensive_built = true;
        }
        let _ = EIGHT_WAY_DIRS;
        json!({
            "behavior_started": self.behavior_started,
            "objective": objective,
            "deferral_turns": self.deferral_turns,
            "attackers": attackers,
            "at_war": at_war,
        })
    }

    fn finish(self: Box<Self>, _: u32) -> Vec<Check> {
        vec![
            Check {
                name: "behavior_plan_started_with_objective",
                passed: self.behavior_started.is_some() && self.objective_seen_before_war,
                detail: format!(
                    "Behavior plan started on {:?}, objective staged before war: {}",
                    self.behavior_started, self.objective_seen_before_war
                ),
            },
            Check {
                name: "behavior_deferred_to_assemble_force",
                passed: self.deferral_turns > 0 && self.deferral_turns <= 4,
                detail: format!(
                    "Deferred {} turn(s) for force assembly before declaring",
                    self.deferral_turns
                ),
            },
            Check {
                name: "behavior_declared_within_bounded_timeout",
                passed: self
                    .first_war
                    .is_some_and(|war| war <= REVENGE_DEADLINE),
                detail: format!(
                    "Declared revenge war on {:?} within {REVENGE_DEADLINE} turns",
                    self.first_war
                ),
            },
            Check {
                name: "behavior_built_troops_during_preparation",
                passed: self.offensive_built,
                detail: format!(
                    "Built offensive land units during preparation: {}",
                    self.offensive_built
                ),
            },
        ]
    }
}
