//! An optional war against a defended city, using only units built in play.

use super::*;
use crate::{
    rules::{Building, Tech},
    sim::{
        HUMAN, Pact, Production, Simulation, UnitKind,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
    },
    world::{GeneratedMap, Resource, Terrain, topology::offset_native_by_map},
};
use serde_json::{Value, json};

const ATTACKER: usize = 1;
const DEADLINE: u32 = 50;

pub(super) const SCENARIO: Scenario = Scenario::new(
    "border-offensive",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup,
    },
);

fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
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
        Box::new(BorderOffensive {
            home,
            target,
            defender,
            staging: None,
            first_assembly: None,
            first_war: None,
            captured: None,
            guarded_while_preparing: true,
            defender_seen_during_peace: false,
        }),
    )
}

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
        if target_owner == Some(ATTACKER) && !defender_present && turn <= DEADLINE {
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
