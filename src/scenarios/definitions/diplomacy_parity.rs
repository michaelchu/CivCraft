//! Treaty expiry, atomic technology exchange and an actual begin-phase war.
use super::{Check, Checks, Scenario, ScenarioRun};
use crate::scenarios::{Control, RuntimeWorld, ScenarioKind};
use crate::sim::drive::Drive;
use crate::{
    rules::Tech,
    sim::{HUMAN, Pact, Simulation, TreatyOffer, UnitKind, fixtures::WorldBuilder},
    world::GeneratedMap,
};

pub(super) const SCENARIO: Scenario = Scenario::new(
    "diplomacy-parity",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Reference,
        setup,
    },
);
fn setup(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    (
        Simulation::new(map),
        Box::new(Parity {
            pending: true,
            action_turns: 0,
            start: 0,
        }),
    )
}
struct Parity {
    pending: bool,
    action_turns: u32,
    start: i32,
}
impl ScenarioRun for Parity {
    fn has_actions(&self) -> bool {
        self.pending
    }
    fn execute_actions(&mut self, game: &mut Simulation) -> Vec<Check> {
        self.pending = false;
        let mut world = WorldBuilder::new(game);
        world.clear_campaign_entities();
        for (owner, position) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            world.founding_city(owner, position);
        }
        world.grassland((12, 23));
        let worker = world.unit(HUMAN, UnitKind::Worker, (12, 23));
        let warrior = world.unit(HUMAN, UnitKind::Warrior, (12, 23));
        game.sign_pact(HUMAN, 1, Pact::Peace);
        let turn = game.turn();
        let mut world = WorldBuilder::new(game);
        world.tile_owner((12, 23), Some(1));
        world.treaty_expiry(HUMAN, 1, turn);
        game.advance_diplomacy();
        let civilian = game.unit_by_id(worker).is_some();
        let military = game.unit_by_id(warrior).is_none();
        let mut world = WorldBuilder::new(game);
        world.known_technologies(HUMAN, &[]);
        world.known_technologies(1, &[Tech::ALPHABET, Tech::POTTERY]);
        let mut offer = TreatyOffer::default();
        offer.terms[1] = offer.terms[1]
            .add_technology(Tech::ALPHABET)
            .add_technology(Tech::POTTERY);
        let exchanged = game.propose_treaty(1, offer)
            && game.has_tech(HUMAN, Tech::ALPHABET)
            && game.has_tech(HUMAN, Tech::POTTERY);
        let mut world = WorldBuilder::new(game);
        world.ai_control(1, true);
        world.immediate_war_plan(1, HUMAN);
        self.start = turn;
        self.action_turns = 1;
        let mut checks = Checks::default();
        checks.extend([
            (
                "civilian_preserved",
                civilian,
                "Armistice expiry preserves a Worker on foreign territory".into(),
            ),
            (
                "military_removed",
                military,
                "Armistice expiry removes the Warrior on that tile".into(),
            ),
            (
                "multiple_technologies",
                exchanged,
                "One treaty transfers Alphabet and Pottery".into(),
            ),
        ]);
        checks.0
    }
    fn action_turns(&self) -> u32 {
        self.action_turns
    }
    fn complete_actions(&mut self, game: &mut Simulation) -> Vec<Check> {
        self.action_turns = 0;
        let declared = game.turn() == self.start + 1 && game.pact(HUMAN, 1) == Pact::War;
        game.enable_all_civilizations_as_ai();
        vec![Check {
            name: "begin_phase_war",
            passed: declared,
            detail: "A prepared AI declaration executes in the following begin phase".into(),
        }]
    }
    fn records_turns(&self) -> bool {
        false
    }
}
