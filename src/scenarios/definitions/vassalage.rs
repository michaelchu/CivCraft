//! Submission through ordinary diplomacy, followed by a normal campaign turn.
use super::{Check, Checks, Scenario, ScenarioRun};
use crate::{
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{Command, HUMAN, Pact, Simulation, VassalKind, drive::Drive, fixtures::WorldBuilder},
    world::GeneratedMap,
};

pub(super) const CAPITULATION: Scenario = Scenario::new(
    "vassal-capitulation",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: capitulation,
    },
);
pub(super) const VOLUNTARY: Scenario = Scenario::new(
    "vassal-voluntary",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: voluntary,
    },
);
fn capitulation(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    setup(map, VassalKind::Capitulation)
}
fn voluntary(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    setup(map, VassalKind::Voluntary)
}
fn setup(map: GeneratedMap, kind: VassalKind) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = Simulation::new(map);
    let mut world = WorldBuilder::new(&mut game);
    world.vassalage_campaign(kind);
    world.finish();
    (
        game,
        Box::new(VassalRun {
            kind,
            pending: true,
            observed: false,
            protected: true,
        }),
    )
}
struct VassalRun {
    kind: VassalKind,
    pending: bool,
    observed: bool,
    protected: bool,
}
impl ScenarioRun for VassalRun {
    fn has_actions(&self) -> bool {
        self.pending
    }
    fn execute_actions(&mut self, game: &mut Simulation) -> Vec<Check> {
        self.pending = false;
        game.advance_diplomatic_offers_for(1);
        let offer = game.treaty_offer(1);
        let awaiting = game.vassal_master(1).is_none()
            && offer
                .as_ref()
                .is_some_and(|o| o.vassalage.is_some_and(|t| t.kind == self.kind));
        let accepted = offer.is_some_and(|o| game.apply(Command::AcceptOffer(1, o)));
        let aligned = match self.kind {
            VassalKind::Capitulation => !game.at_war(0, 1),
            VassalKind::Voluntary => game.at_war(0, 2) && game.at_war(1, 2),
        };
        let subject_blocked = !game.cancel_pact(1, 2);
        let names = match self.kind {
            VassalKind::Capitulation => [
                "vassal_capitulation_human_approval",
                "vassal_capitulation_submission",
                "vassal_capitulation_war_alignment",
                "vassal_capitulation_subject_diplomacy",
            ],
            VassalKind::Voluntary => [
                "vassal_voluntary_human_approval",
                "vassal_voluntary_submission",
                "vassal_voluntary_war_alignment",
                "vassal_voluntary_subject_diplomacy",
            ],
        };
        let mut checks = Checks::default();
        checks.extend([
            (
                names[0],
                awaiting,
                "AI submission remains an offer until the human accepts".into(),
            ),
            (
                names[1],
                accepted && game.vassal_master(1) == Some(HUMAN),
                "Existing treaty command establishes vassalage".into(),
            ),
            (
                names[2],
                aligned,
                "Submission aligns wars for the entire group".into(),
            ),
            (
                names[3],
                subject_blocked,
                "A subject cannot independently change war or peace".into(),
            ),
        ]);
        checks.0
    }
    fn observe_turn(&mut self, game: &Simulation, _completed: u32) -> serde_json::Value {
        self.observed = true;
        self.protected &= game.vassal_master(1) == Some(HUMAN) && game.pact(0, 1) == Pact::Peace;
        serde_json::json!({"master":game.vassal_master(1),"relations":game.observe_diplomacy()})
    }
    fn finish(self: Box<Self>, _completed: u32) -> Vec<Check> {
        vec![Check {
            name: match self.kind {
                VassalKind::Capitulation => "vassal_capitulation_normal_turn",
                VassalKind::Voluntary => "vassal_voluntary_normal_turn",
            },
            passed: self.observed && self.protected,
            detail: "Ordinary AI and treaty processing preserve the protected relationship".into(),
        }]
    }
}
