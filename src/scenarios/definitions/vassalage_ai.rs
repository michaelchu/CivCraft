//! AI-to-AI agreements selected and executed by ordinary campaign turns.
use super::{Check, Scenario, ScenarioRun};
use crate::{
    rules::Tech,
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{
        Pact, Simulation, UnitKind, VassalKind,
        drive::Drive,
        fixtures::{CivPreset, WorldBuilder},
        lifecycle::CityRemoval,
    },
    world::GeneratedMap,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
enum Case {
    Capitulation,
    Voluntary,
    WarPeace,
    Independence,
}
impl Case {
    fn kind(self) -> VassalKind {
        if matches!(self, Self::Voluntary) {
            VassalKind::Voluntary
        } else {
            VassalKind::Capitulation
        }
    }
    fn check_name(self) -> &'static str {
        match self {
            Self::Capitulation => "ai_vassal_capitulation_campaign",
            Self::Voluntary => "ai_vassal_voluntary_campaign",
            Self::WarPeace => "ai_vassal_war_peace_campaign",
            Self::Independence => "ai_vassal_independence_campaign",
        }
    }
    fn turns(self) -> u32 {
        match self {
            Self::Capitulation | Self::Voluntary => 2,
            Self::WarPeace => 5,
            Self::Independence => 3,
        }
    }
}

pub(super) const CAPITULATION: Scenario = Scenario::new(
    "ai-vassal-capitulation",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: |map| setup(map, Case::Capitulation, 1, 2),
    },
);
pub(super) const VOLUNTARY: Scenario = Scenario::new(
    "ai-vassal-voluntary",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: |map| setup(map, Case::Voluntary, 1, 2),
    },
);
pub(super) const WAR_PEACE: Scenario = Scenario::new(
    "ai-vassal-war-peace",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: |map| setup(map, Case::WarPeace, 1, 2),
    },
);
pub(super) const INDEPENDENCE: Scenario = Scenario::new(
    "ai-vassal-independence",
    1,
    ScenarioKind::Runtime {
        control: Control::AllAi,
        world: RuntimeWorld::Custom,
        setup: |map| setup(map, Case::Independence, 1, 2),
    },
);

fn setup(
    map: GeneratedMap,
    case: Case,
    master: usize,
    subject: usize,
) -> (Simulation, Box<dyn ScenarioRun>) {
    let mut game = WorldBuilder::ocean_campaign(
        map,
        &[
            CivPreset {
                id: "rome",
                technologies: Some(&[]),
            },
            CivPreset {
                id: "korea",
                technologies: Some(&[]),
            },
            CivPreset {
                id: "scandinavia",
                technologies: Some(&[]),
            },
            CivPreset {
                id: "egypt",
                technologies: Some(&[]),
            },
        ],
    );
    let mut world = WorldBuilder::new(&mut game);
    for (owner, position) in [
        (0, (36, 36)),
        (master, (8, 20)),
        (subject, (12, 25)),
        (3, (18, 30)),
    ] {
        world.founding_city(owner, position);
        world.treasury(owner, 3000);
        world.ai_control(owner, true);
    }
    world.known_technologies(master, &[Tech::FEUDALISM]);
    for a in 0..4 {
        for b in a + 1..4 {
            world.diplomacy(a, b, Pact::Peace, 0, [0, 0]);
            world.embassy(a, b);
            world.embassy(b, a);
        }
    }
    world.diplomatic_state(
        master,
        subject,
        if case.kind() == VassalKind::Capitulation {
            Pact::War
        } else {
            Pact::Peace
        },
        [6000, 6000],
    );
    // Armies on visible outcrops are separated by water. Their strength can
    // inform diplomacy without a combat roll eliminating a subject first.
    world.grassland((12, 23));
    for _ in 0..3 {
        world.unit(master, UnitKind::ModernArmor, (12, 23));
    }
    world.unit(subject, UnitKind::Warrior, (12, 25));
    if case.kind() == VassalKind::Voluntary {
        world.grassland((14, 25));
        for _ in 0..3 {
            world.unit(3, UnitKind::ModernArmor, (14, 25));
        }
        world.diplomatic_state(subject, 3, Pact::War, [0, 0]);
    }
    let mut lost_cities = Vec::new();
    if matches!(case, Case::Independence) {
        for position in [(28, 12), (36, 12)] {
            for dx in -2..=2 {
                for dy in -2..=2 {
                    world.grassland((position.0 + dx, position.1 + dy));
                }
            }
            lost_cities.push(world.founding_city(subject, position));
        }
    }
    world.finish();
    let audit = Audit {
        case,
        master,
        subject,
        lost_cities,
        started_unbound: game.vassal_master(subject).is_none(),
        formed: false,
        accepted: false,
        held_turns: 0,
        all_ai: true,
        group_consistent: true,
        war: BTreeSet::new(),
        peace: BTreeSet::new(),
        departed: false,
        shock: false,
        dropped: false,
        last_sequence: None,
        last_master: None,
    };
    (game, Box::new(audit))
}

struct Audit {
    case: Case,
    master: usize,
    subject: usize,
    lost_cities: Vec<u32>,
    started_unbound: bool,
    formed: bool,
    accepted: bool,
    held_turns: u32,
    all_ai: bool,
    group_consistent: bool,
    war: BTreeSet<usize>,
    peace: BTreeSet<usize>,
    departed: bool,
    shock: bool,
    dropped: bool,
    last_sequence: Option<u64>,
    last_master: Option<usize>,
}
impl ScenarioRun for Audit {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["diplomacy.transition"]
    }
    fn before_turn(&mut self, game: &mut Simulation, completed: u32) {
        if completed == 1 && game.vassal_master(self.subject) == Some(self.master) {
            match self.case {
                Case::WarPeace => {
                    // Supply a prepared war objective as the changed condition;
                    // the normal AI phase must declare and negotiate peace.
                    WorldBuilder::new(game).immediate_war_plan(self.master, 3);
                    self.shock = true;
                }
                Case::Independence => {
                    for &city in &self.lost_cities {
                        game.remove_city_because(city, CityRemoval::Disaster, None);
                    }
                    self.shock = true;
                }
                _ => {}
            }
        }
    }
    fn observe_turn(&mut self, game: &Simulation, completed: u32) -> Value {
        self.dropped |= game.trace.dropped() > 0;
        for event in game.trace.pending_events() {
            if self
                .last_sequence
                .is_some_and(|last| event.sequence <= last)
            {
                continue;
            }
            self.last_sequence = Some(event.sequence);
            if event.kind != "diplomacy.transition" {
                continue;
            }
            let p = &event.payload;
            let actor = p["actor"].as_u64().map(|v| v as usize);
            let other = p["other"].as_u64().map(|v| v as usize);
            if p["action"] == "vassalage_started"
                && actor == Some(self.master)
                && other == Some(self.subject)
                && p["reason"] == self.case.kind().name()
            {
                self.formed = true;
            }
            if p["action"] == "offer_accepted"
                && p["offer"]["vassalage"]["master"] == self.master
                && p["offer"]["vassalage"]["kind"] == json!(self.case.kind())
                && [actor, other].contains(&Some(self.subject))
            {
                self.accepted = true;
            }
            if p["action"] == "vassalage_ended"
                && actor == Some(self.subject)
                && other == Some(self.master)
                && p["reason"] == "protection_failed"
            {
                self.departed = true;
            }
            if p["action"] == "pact_changed" {
                for member in [self.master, self.subject] {
                    if [actor, other].contains(&Some(member)) && [actor, other].contains(&Some(3)) {
                        if p["current"] == "War" {
                            self.war.insert(member);
                        }
                        if p["current"] == "Ceasefire"
                            || p["current"] == "Peace"
                            || p["current"] == "Armistice"
                        {
                            self.peace.insert(member);
                        }
                    }
                }
            }
        }
        self.all_ai &= game.players().iter().take(4).all(|p| p.ai);
        if game.vassal_master(self.subject) == Some(self.master) {
            self.held_turns += 1;
            self.group_consistent &= !game.at_war(self.master, self.subject)
                && game.pact(self.master, 3) == game.pact(self.subject, 3);
        }
        let state = json!({"completed_turns":completed,"master":self.master,"subject":self.subject,
            "subject_master":game.vassal_master(self.subject),"formed":self.formed,"accepted":self.accepted,
            "held_turns":self.held_turns,"war_members":self.war,"peace_members":self.peace,
            "shock_applied":self.shock,"departed":self.departed,
            "pacts":[game.pact(self.master,self.subject),game.pact(self.master,3),game.pact(self.subject,3)],
            "gold":[game.players()[self.master].gold,game.players()[self.subject].gold],
            "alive":[game.cities().iter().any(|c|c.owner==self.master),game.cities().iter().any(|c|c.owner==self.subject)]});
        self.last_master = game.vassal_master(self.subject);
        if self.formed && !matches!(self.case, Case::Independence) {
            self.group_consistent &= self.last_master == Some(self.master);
        }
        state
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        let held = if matches!(self.case, Case::Independence) {
            self.held_turns >= 1
        } else {
            self.held_turns >= 2
        };
        let specific = match self.case {
            Case::Capitulation => true,
            Case::Voluntary => self.war.contains(&self.master),
            Case::WarPeace => {
                self.shock
                    && [self.master, self.subject]
                        .iter()
                        .all(|m| self.war.contains(m) && self.peace.contains(m))
            }
            Case::Independence => self.shock && self.departed && self.last_master.is_none(),
        };
        vec![Check {
            name: self.case.check_name(),
            passed: completed >= self.case.turns()
                && self.started_unbound
                && self.all_ai
                && self.formed
                && self.accepted
                && held
                && self.group_consistent
                && specific
                && !self.dropped,
            detail: format!(
                "{:?}: {} normal turns; formed={}, accepted={}, held={}, consistent={}, AI={}, shock={}, departed={}, wars={:?}, peace={:?}, trace_dropped={}",
                self.case,
                completed,
                self.formed,
                self.accepted,
                self.held_turns,
                self.group_consistent,
                self.all_ai,
                self.shock,
                self.departed,
                self.war,
                self.peace,
                self.dropped
            ),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace;
    #[test]
    fn ai_vassal_campaigns_form_and_manage_agreements_through_normal_turns() {
        let mut failures = Vec::new();
        for (case, master, subject) in [
            (Case::Capitulation, 1, 2),
            (Case::Capitulation, 2, 1),
            (Case::Voluntary, 1, 2),
            (Case::WarPeace, 1, 2),
            (Case::Independence, 1, 2),
        ] {
            let (mut game, mut audit) =
                setup(crate::test_support::seeded_map(), case, master, subject);
            game.trace = trace::Collector::enabled(["*".to_owned()].into(), 10000);
            let mut observations = Vec::new();
            for completed in 0..case.turns() {
                audit.before_turn(&mut game, completed);
                game.simulate_turn();
                observations.push(audit.observe_turn(&game, completed + 1));
                game.trace.drain();
            }
            for check in audit.finish(case.turns()) {
                if !check.passed {
                    failures.push(format!(
                        "{}: {}. Observations: {}",
                        check.name,
                        check.detail,
                        json!(observations)
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
