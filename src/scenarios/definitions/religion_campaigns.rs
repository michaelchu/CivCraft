//! Religion research choices, changing opportunities and controlled spread.
use super::{Check, Scenario, ScenarioRun};
use crate::{
    rules::{Building, Tech},
    scenarios::{Control, RuntimeWorld, ScenarioKind},
    sim::{FaithPolicy, Government, HUMAN, Simulation, drive::Drive, fixtures::WorldBuilder},
    world::GeneratedMap,
};
use serde_json::{Value, json};

const FAITH_TECHS: [Tech; 7] = [
    Tech::MEDITATION,
    Tech::POLYTHEISM,
    Tech::MONOTHEISM,
    Tech(12),
    Tech(58),
    Tech::THEOLOGY,
    Tech::DIVINE_RIGHT,
];
#[derive(Clone, Copy, Debug)]
enum ResearchCase {
    Race,
    Claimed,
    Recovery,
}
pub(super) const RACE: Scenario = Scenario::new(
    "religion-ai-research-race",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| research(m, ResearchCase::Race),
    },
);
pub(super) const CLAIMED: Scenario = Scenario::new(
    "religion-ai-rival-claim",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| research(m, ResearchCase::Claimed),
    },
);
pub(super) const RECOVERY: Scenario = Scenario::new(
    "religion-ai-recovery",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: |m| research(m, ResearchCase::Recovery),
    },
);
pub(super) const SPREAD: Scenario = Scenario::new(
    "religion-ai-missionary",
    1,
    ScenarioKind::Runtime {
        control: Control::Human,
        world: RuntimeWorld::Custom,
        setup: spread,
    },
);

fn research(map: GeneratedMap, case: ResearchCase) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut g, ids) = super::religion_strategy::base(map);
    let known: Vec<_> = Tech::all()
        .filter(|t| t.is_researchable_advance() && !FAITH_TECHS.contains(t))
        .collect();
    let mut w = WorldBuilder::new(&mut g);
    w.ai_control(2, false);
    w.ai_control(3, false);
    w.known_technologies(1, &known);
    // Isolate the research race from an unrelated opening revolution/unrest.
    w.government(1, Government::DEMOCRACY);
    w.city_buildings(ids[1], &[Building::PALACE, Building::TEMPLE]);
    w.research_wants(1, &[]);
    w.research_goal(1, Tech::UNSET);
    w.finish();
    (
        g,
        Box::new(Research {
            case,
            capital: ids[1],
            rival_city: ids[0],
            initial: None,
            chosen: None,
            initial_goal: None,
            chosen_goal: None,
            original_interest: 0.0,
            changed_interest: 0.0,
            crisis: false,
            selected_legally: false,
            researched: false,
            founded: false,
            claim_unique: false,
            dropped: false,
            last_sequence: None,
            discovery_event: false,
            founding_event: false,
        }),
    )
}
struct Research {
    case: ResearchCase,
    capital: u32,
    rival_city: u32,
    initial: Option<Tech>,
    chosen: Option<Tech>,
    initial_goal: Option<usize>,
    chosen_goal: Option<usize>,
    original_interest: f32,
    changed_interest: f32,
    crisis: bool,
    selected_legally: bool,
    researched: bool,
    founded: bool,
    claim_unique: bool,
    dropped: bool,
    last_sequence: Option<u64>,
    discovery_event: bool,
    founding_event: bool,
}
impl ScenarioRun for Research {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*", "research.*"]
    }
    fn before_turn(&mut self, g: &mut Simulation, completed: u32) {
        if completed != 0 {
            return;
        }
        g.ensure_research_observation();
        let selected = g.select_ai_research(1).0;
        self.initial = Some(selected);
        self.initial_goal = g
            .observe_research()
            .civilizations
            .iter()
            .find(|r| r.owner == 1)
            .and_then(|r| r.goal_technology_id);
        let Some(index) = FAITH_TECHS.iter().position(|t| *t == selected) else {
            return;
        };
        self.original_interest =
            g.religion_research_interest(1, selected, &g.strategic_priorities(1));
        let mut chosen = selected;
        match self.case {
            ResearchCase::Race => {}
            ResearchCase::Claimed => {
                if !g.found_religion(HUMAN, self.rival_city, index as u32 + 1) {
                    return;
                }
                self.changed_interest =
                    g.religion_research_interest(1, selected, &g.strategic_priorities(1));
                chosen = g.select_ai_research(1).0;
            }
            ResearchCase::Recovery => {
                let mut w = WorldBuilder::new(g);
                let mut buildings = vec![Building::LIBRARY; 20];
                buildings.push(Building::PALACE);
                w.city_buildings(self.capital, &buildings);
                w.treasury(1, 0);
                let p = g.strategic_priorities(1);
                self.crisis = p.economic_crisis;
                self.changed_interest = g.religion_research_interest(1, selected, &p);
            }
        }
        self.chosen = Some(chosen);
        self.chosen_goal = g
            .observe_research()
            .civilizations
            .iter()
            .find(|r| r.owner == 1)
            .and_then(|r| r.goal_technology_id);
        self.selected_legally = g.tech_available(1, chosen)
            && FAITH_TECHS.contains(&chosen)
            && self.chosen_goal == Some(chosen.0);
        if !self.selected_legally {
            return;
        }
        // Prepare the final beaker only after recording the AI's own choice.
        // Discovery and founding still happen in the ordinary turn pipeline.
        let progress = if matches!(self.case, ResearchCase::Recovery) {
            0
        } else {
            g.target_cost(1, chosen) + g.technology_upkeep(1) - 1
        };
        WorldBuilder::new(g).research_project(1, chosen, progress);
    }
    fn observe_turn(&mut self, g: &Simulation, completed: u32) -> Value {
        self.dropped |= g.trace.dropped() > 0;
        for event in g.trace.pending_events() {
            if self.last_sequence.is_some_and(|n| event.sequence <= n) {
                continue;
            }
            self.last_sequence = Some(event.sequence);
            if event.kind == "research.technology_changed"
                && event.payload["owner"] == 1
                && event.payload["action"] == "acquired"
                && event.payload["source"] == "research"
                && self
                    .chosen
                    .is_some_and(|t| event.payload["technology_id"] == t.0)
            {
                self.discovery_event = true;
            }
            if event.kind == "religion.change"
                && event.payload["owner"] == 1
                && event.payload["action"] == "founded"
            {
                self.founding_event = true;
            }
        }
        if let Some(chosen) = self.chosen {
            self.researched = g.has_tech(1, chosen);
            if let Some(index) = FAITH_TECHS.iter().position(|t| *t == chosen) {
                self.founded = g
                    .religion_catalog()
                    .iter()
                    .any(|f| f.founder == 1 && f.id == index as u32 + 1);
            }
        }
        if let Some(initial) = self.initial
            && let Some(index) = FAITH_TECHS.iter().position(|t| *t == initial)
        {
            self.claim_unique = g
                .religion_catalog()
                .iter()
                .filter(|f| f.id == index as u32 + 1)
                .count()
                == 1
                && g.religion_catalog()
                    .iter()
                    .any(|f| f.id == index as u32 + 1 && f.founder == HUMAN);
        }
        json!({"completed":completed,"case":format!("{:?}",self.case),"initial":self.initial.map(|t|t.name()),"chosen":self.chosen.map(|t|t.name()),"initial_goal":self.initial_goal,"chosen_goal":self.chosen_goal,"initial_interest":self.original_interest,"changed_interest":self.changed_interest,"crisis_before_turn":self.crisis,"researched":self.researched,"founded":self.founded,"research":g.observe_research(),"government":format!("{:?}",g.players()[1].government),"rates":g.players()[1].rates,"cities":g.observe_cities(),"religion":g.observe_religion()})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        let common = completed >= 1
            && self.selected_legally
            && self.original_interest > 0.0
            && !self.dropped;
        let (name, specific) = match self.case {
            ResearchCase::Race => (
                "religion_ai_research_and_found",
                self.researched && self.founded && self.discovery_event && self.founding_event,
            ),
            ResearchCase::Claimed => (
                "religion_ai_retargets_claimed_faith",
                self.changed_interest == 0.0
                    && self.initial != self.chosen
                    && self.initial_goal != self.chosen_goal
                    && self.claim_unique
                    && self.researched
                    && self.founded
                    && self.founding_event,
            ),
            ResearchCase::Recovery => (
                "religion_ai_pauses_for_recovery",
                self.crisis && self.changed_interest == 0.0,
            ),
        };
        vec![Check {
            name,
            passed: common && specific,
            detail: format!(
                "{:?}: chosen={:?}->{:?}, goals={:?}->{:?}, interest={}->{}, legal={}, researched={}, founded={}, crisis={}, unique={}, trace_dropped={}",
                self.case,
                self.initial,
                self.chosen,
                self.initial_goal,
                self.chosen_goal,
                self.original_interest,
                self.changed_interest,
                self.selected_legally,
                self.researched,
                self.founded,
                self.crisis,
                self.claim_unique,
                self.dropped
            ),
        }]
    }
}

fn spread(map: GeneratedMap) -> (Simulation, Box<dyn ScenarioRun>) {
    let (mut g, ids) = super::religion_strategy::base(map);
    let mut w = WorldBuilder::new(&mut g);
    w.ai_control(2, false);
    w.ai_control(3, false);
    w.known_technologies(1, &[Tech::POLYTHEISM]);
    let a = w.founding_city(1, (14, 24));
    let b = w.founding_city(1, (16, 24));
    w.finish();
    assert!(g.found_religion(1, ids[1], 2));
    let mut w = WorldBuilder::new(&mut g);
    w.city_religion(a, &[(2, 90)], None);
    w.city_religion(b, &[(2, 90)], None);
    w.finish();
    let mut control = g.clone();
    assert!(control.declare_faith(1, Some(2), FaithPolicy::Tolerant));
    (
        g,
        Box::new(Spread {
            control,
            targets: [a, b],
            missionary: false,
            converted: false,
            control_unconverted: false,
            conversion_events: 0,
            last_sequence: None,
            dropped: false,
        }),
    )
}
struct Spread {
    control: Simulation,
    targets: [u32; 2],
    missionary: bool,
    converted: bool,
    control_unconverted: bool,
    conversion_events: usize,
    last_sequence: Option<u64>,
    dropped: bool,
}
impl ScenarioRun for Spread {
    fn trace_kinds(&self) -> &'static [&'static str] {
        &["religion.*"]
    }
    fn comparison_game(&mut self) -> Option<&mut Simulation> {
        Some(&mut self.control)
    }
    fn observe_turn(&mut self, g: &Simulation, completed: u32) -> Value {
        self.missionary |= g.national_faith(1).policy == FaithPolicy::Missionary;
        self.converted = self
            .targets
            .iter()
            .all(|id| g.city_dominant_faith(*id) == Some(2));
        self.control_unconverted = self
            .targets
            .iter()
            .all(|id| self.control.city_dominant_faith(*id).is_none());
        self.dropped |= g.trace.dropped() > 0;
        for event in g.trace.pending_events() {
            if self.last_sequence.is_some_and(|n| event.sequence <= n) {
                continue;
            }
            self.last_sequence = Some(event.sequence);
            if event.kind == "religion.change"
                && event.payload["action"] == "converted"
                && event.payload["faith"] == 2
                && event.payload["city"]
                    .as_u64()
                    .is_some_and(|id| self.targets.contains(&(id as u32)))
            {
                self.conversion_events += 1;
            }
        }
        json!({"completed":completed,"missionary":self.missionary,"converted":self.converted,"control_unconverted":self.control_unconverted,"conversion_events":self.conversion_events,"religion":g.observe_religion(),"control":self.control.observe_religion()})
    }
    fn finish(self: Box<Self>, completed: u32) -> Vec<Check> {
        vec![Check {
            name: "religion_ai_missionary_accelerates_conversion",
            passed: completed >= 5
                && self.missionary
                && self.converted
                && self.control_unconverted
                && self.conversion_events >= 2
                && self.control.national_faith(1).policy == FaithPolicy::Tolerant
                && !self.dropped,
            detail: format!(
                "turns={completed}, Missionary={}, targets converted={}, control unconverted={}, conversion events={}, dropped={}",
                self.missionary,
                self.converted,
                self.control_unconverted,
                self.conversion_events,
                self.dropped
            ),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn religion_campaigns_cover_research_rivals_recovery_and_spread() {
        let mut failures = Vec::new();
        for seed in [42, 7, 99] {
            for (scenario, turns) in [(RACE, 1), (CLAIMED, 1), (RECOVERY, 1), (SPREAD, 5)] {
                let ScenarioKind::Runtime { setup, .. } = scenario.kind else {
                    unreachable!()
                };
                let (mut g, mut audit) = setup(crate::mapgen::generate(seed));
                g.trace = crate::sim::trace::Collector::enabled(["*".into()].into(), 10000);
                for n in 0..turns {
                    audit.before_turn(&mut g, n);
                    g.simulate_turn();
                    if let Some(c) = audit.comparison_game() {
                        c.simulate_turn();
                    }
                    audit.observe_turn(&g, n + 1);
                    g.trace.drain();
                }
                for check in audit.finish(turns) {
                    if !check.passed {
                        failures.push(format!("seed {seed}, {}: {}", scenario.id, check.detail));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
