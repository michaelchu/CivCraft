//! Normal campaign turns prove spread, competing faiths and autonomous AI.
use super::{Check, Scenario, ScenarioRun};
use crate::{
    rules::{Building, Tech},
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{
        FaithPolicy, HUMAN, Pact, Production, Simulation, UnitKind,
        drive::Drive,
        fixtures::{CivPreset, TileSurface, WorldBuilder},
    },
    world::{GeneratedMap, Terrain},
};
use serde_json::{Value, json};
#[derive(Clone, Copy)]
enum Case {
    Spread,
    Competition,
    Ai,
}
pub(super) const SPREAD: Scenario = Scenario::new(
    "religion-spread",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| setup(m, Case::Spread),
    },
);
pub(super) const COMPETITION: Scenario = Scenario::new(
    "religion-competition",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| setup(m, Case::Competition),
    },
);
pub(super) const AI: Scenario = Scenario::new(
    "religion-ai",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: |m| setup(m, Case::Ai),
    },
);
fn setup(map: GeneratedMap, case: Case) -> (Simulation, Box<dyn ScenarioRun>) {
    let techs = [Tech::MEDITATION, Tech::POLYTHEISM, Tech::CEREMONIAL_BURIAL];
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: Some(if matches!(case, Case::Ai) {
                    &[Tech::CEREMONIAL_BURIAL]
                } else {
                    &techs
                }),
            },
            CivPreset {
                id: "korea",
                technologies: Some(if matches!(case, Case::Spread) {
                    &[Tech::CEREMONIAL_BURIAL]
                } else {
                    &techs
                }),
            },
        ],
    );
    let mut w = WorldBuilder::new(&mut game);
    w.map_surface(TileSurface {
        terrain: Some(Terrain::GrasslandShield),
        road: Some(true),
        ..Default::default()
    });
    let a = w.city(HUMAN, (10, 24), 3);
    let target = w.city(HUMAN, (14, 24), 3);
    let foreign = w.city(1, (20, 24), 3);
    for (owner, id, position) in [
        (HUMAN, a, (10, 24)),
        (HUMAN, target, (14, 24)),
        (1, foreign, (20, 24)),
    ] {
        w.city_workers(id, 3);
        w.production_stock(id, Production::Building(Building::COINAGE), 0);
        w.unit(owner, UnitKind::Warrior, position);
        w.economy(owner, 200, [40, 60, 0]);
    }
    if matches!(case, Case::Spread) {
        w.ai_control(1, false);
    }
    w.city_buildings(a, &[Building::PALACE]);
    w.city_buildings(foreign, &[Building::PALACE]);
    w.diplomacy(HUMAN, 1, Pact::Peace, 1, [0; 2]);
    w.finish();
    if !matches!(case, Case::Ai) {
        assert!(game.found_religion(HUMAN, a, 1));
        assert!(game.declare_faith(HUMAN, Some(1), FaithPolicy::Missionary));
        if matches!(case, Case::Competition) {
            assert!(game.found_religion(1, foreign, 2));
            assert!(game.declare_faith(1, Some(2), FaithPolicy::Devout));
            WorldBuilder::new(&mut game).city_religion(target, &[(1, 180), (2, 180)], None);
        } else {
            WorldBuilder::new(&mut game).city_religion(target, &[(1, 90)], None);
        }
    }
    (
        game,
        Box::new(Audit {
            case,
            target,
            converted: false,
            stable: true,
            foreign_reached: false,
            ai_founded: false,
            ai_declared: false,
            attitude: false,
            peace: true,
        }),
    )
}
struct Audit {
    case: Case,
    target: u32,
    converted: bool,
    stable: bool,
    foreign_reached: bool,
    ai_founded: bool,
    ai_declared: bool,
    attitude: bool,
    peace: bool,
}
impl ScenarioRun for Audit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*"]
    }
    fn observe_turn(&mut self, g: &Simulation, _completed: u32) -> Value {
        self.converted |= g.city_dominant_faith(self.target) == Some(1);
        self.stable &= g.city_dominant_faith(self.target).is_none();
        self.foreign_reached |= g.eligible_faiths(1).contains(&1);
        self.ai_founded = g.religion_catalog().iter().any(|r| r.founder == 1);
        self.ai_declared = g.national_faith(1).declared.is_some();
        self.attitude = g.religion_attitude(1, HUMAN) == -4;
        self.peace &= g.contacted_pact(HUMAN, 1) == Some(Pact::Peace);
        json!({"religion":g.observe_religion(),"converted":self.converted,"foreign_reached":self.foreign_reached,"religion_attitude":g.religion_attitude(1,HUMAN)})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        let (name, passed) = match self.case {
            Case::Spread => (
                "religion_normal_turn_spread",
                completed >= 12 && self.converted && self.foreign_reached,
            ),
            Case::Competition => (
                "religion_competing_faiths_and_diplomacy",
                completed >= 2 && self.stable && self.attitude && self.peace,
            ),
            Case::Ai => (
                "religion_ai_foundation_and_declaration",
                completed >= 2 && self.ai_founded && self.ai_declared,
            ),
        };
        vec![Check {
            name,
            passed,
            detail: format!(
                "turns={completed}, converted={}, foreign_present={}, stable={}, AI founded={}, AI declared={}, religious hostility={}, peace={}",
                self.converted,
                self.foreign_reached,
                self.stable,
                self.ai_founded,
                self.ai_declared,
                self.attitude,
                self.peace
            ),
        }]
    }
}
pub(super) const UNLOCKS: Scenario = Scenario::new(
    "religion-tech-unlocks",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: setup_unlocks,
    },
);
fn setup_unlocks(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut game, _) = setup(map, Case::Ai);
    let a = game
        .cities()
        .iter()
        .filter(|c| c.owner == HUMAN)
        .map(|c| c.id)
        .min()
        .unwrap();
    let b = game.cities().iter().find(|c| c.owner == 1).unwrap().id;
    let mut ancient: Vec<_> = Tech::all()
        .filter(|t| t.is_civ3_advance() && t.era() == crate::rules::TechEra::Ancient)
        .collect();
    ancient.retain(|t| *t != Tech::MEDITATION);
    ancient.extend([Tech::THEOLOGY, Tech::MONOTHEISM]);
    let mut w = WorldBuilder::new(&mut game);
    w.ai_control(HUMAN, false);
    w.ai_control(1, false);
    w.known_technologies(HUMAN, &ancient);
    w.known_technologies(1, &[Tech::CEREMONIAL_BURIAL, Tech::MYSTICISM]);
    w.finish();
    let divine = game.target_cost(HUMAN, Tech::DIVINE_RIGHT);
    let meditation = game.target_cost(1, Tech::MEDITATION);
    let mut w = WorldBuilder::new(&mut game);
    w.research_project(HUMAN, Tech::DIVINE_RIGHT, divine - 1);
    w.research_project(1, Tech::MEDITATION, meditation - 1);
    w.finish();
    (
        game,
        Box::new(UnlockAudit {
            a,
            b,
            researched: false,
            founded: false,
        }),
    )
}
struct UnlockAudit {
    a: u32,
    b: u32,
    researched: bool,
    founded: bool,
}
impl ScenarioRun for UnlockAudit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*", "research.*"]
    }
    fn before_turn(&mut self, g: &mut Simulation, completed: u32) {
        if completed == 1 {
            self.founded = g.found_religion(HUMAN, self.a, 7) && g.found_religion(1, self.b, 1);
        }
    }
    fn observe_turn(&mut self, g: &Simulation, _completed: u32) -> Value {
        self.researched = g.has_tech(HUMAN, Tech::DIVINE_RIGHT) && g.has_tech(1, Tech::MEDITATION);
        json!({"researched":self.researched,"founded":self.founded,"religion":g.observe_religion()})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        vec![Check {
            name: "religion_technology_unlocks",
            passed: completed >= 2 && self.researched && self.founded,
            detail: format!(
                "turns={completed}, researched={}, founded={}",
                self.researched, self.founded
            ),
        }]
    }
}

#[cfg(test)]
mod unlock_tests {
    use super::*;
    #[test]
    fn normal_research_unlocks_predefined_founding() {
        let (mut g, mut audit) = setup_unlocks(crate::test_support::seeded_map());
        for completed in 0..2 {
            audit.before_turn(&mut g, completed);
            g.simulate_turn();
            audit.observe_turn(&g, completed + 1);
        }
        for check in audit.finish(2) {
            assert!(check.passed, "{}", check.detail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn religion_runtime_campaigns_reach_their_outcomes() {
        for (case, turns) in [(Case::Spread, 12), (Case::Competition, 2), (Case::Ai, 2)] {
            let (mut game, mut audit) = setup(crate::test_support::seeded_map(), case);
            if matches!(case, Case::Ai) {
                WorldBuilder::new(&mut game).ai_control(HUMAN, true);
            }
            for n in 1..=turns {
                game.simulate_turn();
                audit.observe_turn(&game, n);
            }
            for check in audit.finish(turns) {
                assert!(check.passed, "{}: {}", check.name, check.detail);
            }
        }
    }
}
