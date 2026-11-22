//! An optional war against a defended city, using only units built in play.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        HUMAN, Pact, Production, Simulation, UnitKind,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
    },
    world::{
        GeneratedMap, Resource, Terrain,
        topology::{EIGHT_WAY_DIRS, offset_native_by_map},
    },
};
use serde_json::{Value, json};

const ATTACKER: usize = 1;
const DEADLINE: u32 = 50;
const REINFORCED_DEADLINE: u32 = 75;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "border-offensive",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

pub(super) const REINFORCED: Scenario = Scenario::new(
    "border-reinforcements",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_reinforced,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (game, audit) = campaign(map);
    (game, Box::new(audit))
}

fn campaign(map: GeneratedMap) -> (Simulation, BorderOffensive) {
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
    let front = offset_native_by_map(home.0, home.1, 8, 0);
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
    world.treasury(ATTACKER, 80);
    world.diplomacy(ATTACKER, HUMAN, Pact::Peace, 1, [-5000, -5000]);
    for dx in 0..=8 {
        for dy in -3..=3 {
            world.mark_known(ATTACKER, offset_native_by_map(home.0, home.1, dx, dy));
        }
    }
    world.finish();
    (
        game,
        BorderOffensive {
            home,
            target,
            defender,
            staging: None,
            first_assembly: None,
            first_war: None,
            captured: None,
            guarded_while_preparing: true,
            defender_seen_during_peace: false,
            capture_deadline: DEADLINE,
        },
    )
}

#[derive(Clone)]
struct BorderOffensive {
    home: (i32, i32),
    target: u32,
    defender: u32,
    staging: Option<(i32, i32)>,
    first_assembly: Option<u32>,
    first_war: Option<u32>,
    captured: Option<u32>,
    guarded_while_preparing: bool,
    defender_seen_during_peace: bool,
    capture_deadline: u32,
}

impl ScenarioRun for BorderOffensive {
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
                if goal.city == self.target && staged.len() >= 2 {
                    self.staging = Some(goal.staging);
                    self.first_assembly.get_or_insert(turn);
                }
            }
        }
        if at_war {
            self.first_war.get_or_insert(turn);
        }
        if target_owner == Some(ATTACKER) && !defender_present && turn <= self.capture_deadline {
            self.captured.get_or_insert(turn);
        }
        json!({
            "objective": objective,
            "staged_attackers": staged,
            "garrison": garrison,
            "at_war": at_war,
            "defender_present": defender_present,
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
                name: "border_offensive_built_and_staged_before_declaration",
                passed: assembled_before_war,
                detail: format!(
                    "Staging {:?}, built force assembled on {:?}, war on {:?}",
                    self.staging, self.first_assembly, self.first_war
                ),
            },
            Check {
                name: "border_offensive_preserved_home_garrison",
                passed: self.guarded_while_preparing && self.first_assembly.is_some(),
                detail: "Home retained a defending military unit throughout observed preparation"
                    .into(),
            },
            Check {
                name: "border_offensive_captured_defended_city",
                passed: self.defender_seen_during_peace && self.captured.is_some(),
                detail: format!(
                    "Starting defender was present in peacetime; capture on {:?} within {DEADLINE} turns",
                    self.captured
                ),
            },
        ]
    }
}

fn setup_reinforced(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (control, control_audit) = campaign(map);
    let game = control.clone();
    let mut audit = control_audit.clone();
    audit.capture_deadline = REINFORCED_DEADLINE;
    (
        game,
        Box::new(ReinforcementAudit {
            control,
            control_audit,
            audit,
            reinforcement_turn: None,
            defenders: Vec::new(),
            staged_at_shock: 0,
            first_readiness_deferral: None,
            spotted_reinforcements: false,
        }),
    )
}

struct ReinforcementAudit {
    control: Simulation,
    control_audit: BorderOffensive,
    audit: BorderOffensive,
    reinforcement_turn: Option<u32>,
    defenders: Vec<u32>,
    staged_at_shock: usize,
    first_readiness_deferral: Option<u32>,
    spotted_reinforcements: bool,
}

impl ScenarioRun for ReinforcementAudit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["diplomacy.transition"]
    }

    fn comparison_game(&mut self) -> Option<&mut Simulation> {
        Some(&mut self.control)
    }

    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        if self.reinforcement_turn.is_some() {
            if game.at_war(ATTACKER, HUMAN)
                && let Some(city) = game
                    .city_by_id(self.audit.target)
                    .filter(|city| city.owner == HUMAN)
            {
                let target = (city.x, city.y);
                for &id in &self.defenders {
                    let Some(unit) = game.unit_by_id(id) else {
                        continue;
                    };
                    let distance = game.map_distance(unit.x, unit.y, target.0, target.1);
                    if let Some(direction) = EIGHT_WAY_DIRS.iter().position(|&(dx, dy)| {
                        game.map_wrap
                            .eight_way_offset(unit.x, unit.y, dx, dy)
                            .is_some_and(|(x, y)| {
                                game.map_distance(x, y, target.0, target.1) < distance
                            })
                    }) {
                        game.try_move_unit(id, direction);
                    }
                }
            }
            return;
        }
        if game.at_war(ATTACKER, HUMAN) {
            return;
        }
        let Some(goal) = game.coordinated_war_objective(ATTACKER) else {
            return;
        };
        self.staged_at_shock = game
            .units()
            .iter()
            .filter(|unit| {
                unit.owner == ATTACKER
                    && matches!(unit.kind, UnitKind::Archer | UnitKind::Swordsman)
                    && (unit.x, unit.y) == goal.staging
            })
            .count();
        if self.staged_at_shock == 0 {
            return;
        }
        let city = game.city_by_id(self.audit.target).unwrap();
        let position = game
            .map_wrap
            .tiles_within_steps(city.x, city.y, 3)
            .into_iter()
            .filter_map(|(x, y, distance)| (distance > 0).then_some((x, y)))
            .filter(|&(x, y)| {
                !game.tile(x, y).terrain.is_water()
                    && game.tile(x, y).owner != Some(ATTACKER)
                    && (x, y) != goal.staging
                    && game
                        .units()
                        .iter()
                        .all(|unit| (unit.x, unit.y) != (x, y) || unit.owner == HUMAN)
            })
            .min_by_key(|&(x, y)| {
                (
                    game.map_distance(x, y, goal.staging.0, goal.staging.1),
                    x,
                    y,
                )
            })
            .expect("the fortified border has a visible land approach");
        let mut world = WorldBuilder::new(game);
        for _ in 0..2 {
            self.defenders
                .push(world.unit(HUMAN, UnitKind::Pikeman, position));
        }
        world.finish();
        self.reinforcement_turn = Some(completed + 1);
    }

    fn observe_turn(&mut self, game: &Simulation, turn: u32) -> Value {
        if self.reinforcement_turn.is_some()
            && game.trace.pending_events().iter().any(|event| {
                event.kind == "diplomacy.transition"
                    && event.payload["action"] == "war_deferred"
                    && event.payload["reason"] == "assembling_force"
                    && event.payload["actor"] == ATTACKER
                    && event.payload["other"] == HUMAN
            })
        {
            self.first_readiness_deferral.get_or_insert(turn);
        }
        let visible = self
            .defenders
            .iter()
            .filter_map(|&id| game.unit_by_id(id))
            .filter(|unit| game.unit_visible_to(ATTACKER, unit))
            .count();
        self.spotted_reinforcements |= visible > 0 && !game.at_war(ATTACKER, HUMAN);
        json!({
            "first_readiness_deferral": self.first_readiness_deferral,
            "control": self.control_audit.observe_turn(&self.control, turn),
            "reinforced": self.audit.observe_turn(game, turn),
            "reinforced_on": self.reinforcement_turn,
            "reinforcements_alive": self.defenders.iter().filter(|&&id| game.unit_by_id(id).is_some()).count(),
            "visible_reinforcements": visible,
            "staged_at_reinforcement": self.staged_at_shock,
        })
    }

    fn finish(self: Box<Self>, _: u32) -> Vec<Check> {
        let after_control = self
            .control_audit
            .first_war
            .is_some_and(|control| self.audit.first_war.is_none_or(|war| war > control));
        vec![
            Check {
                name: "reinforced_after_assembly_started",
                passed: self.reinforcement_turn.is_some()
                    && self.spotted_reinforcements
                    && self.control_audit.captured.is_some(),
                detail: format!(
                    "Reinforced on {:?}, control captured on {:?}",
                    self.reinforcement_turn, self.control_audit.captured
                ),
            },
            Check {
                name: "reinforcements_delay_declaration",
                passed: self.first_readiness_deferral.is_some() && after_control,
                detail: format!(
                    "Control declared on {:?}, reinforced lane deferred for readiness on {:?} and declared on {:?}",
                    self.control_audit.first_war,
                    self.first_readiness_deferral,
                    self.audit.first_war
                ),
            },
            Check {
                name: "reinforced_offensive_keeps_home_guarded",
                passed: self.reinforcement_turn.is_some() && self.audit.guarded_while_preparing,
                detail: "The staging city retains a defender during preparation".into(),
            },
            Check {
                name: "reinforced_offensive_recovers_and_captures",
                passed: after_control && self.audit.captured.is_some(),
                detail: format!("Capture after reassessment on {:?}", self.audit.captured),
            },
        ]
    }
}
