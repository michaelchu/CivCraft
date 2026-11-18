//! Passive government advice and adoption evidence. Never refresh the advisor here.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Candidate {
    pub government: Government,
    pub available: bool,
    pub score: Option<f64>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct GovernmentSnapshot {
    pub owner: usize,
    #[serde(default)]
    pub name: String,
    pub ai: bool,
    pub alive: bool,
    pub cities: usize,
    pub current: Government,
    pub revolution_target: Option<Government>,
    pub revolution_turn: Option<i32>,
    pub evaluated_turn: Option<i32>,
    pub next_evaluation: Option<i32>,
    pub best_available: Option<Government>,
    pub research_goal: Option<Government>,
    pub candidates: Vec<Candidate>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct GovernmentDecision {
    pub advice: GovernmentSnapshot,
    pub selected: Government,
    pub outcome: String,
}
impl trace::Payload for GovernmentDecision {
    const KIND: &'static str = "government.decision";
}
#[derive(Serialize)]
pub(crate) struct GovernmentChange {
    pub owner: usize,
    pub previous: Government,
    pub current: Government,
    pub target: Option<Government>,
    pub reason: &'static str,
}
impl trace::Payload for GovernmentChange {
    const KIND: &'static str = "government.changed";
}
impl Simulation {
    pub(crate) fn observe_governments(&self) -> Vec<GovernmentSnapshot> {
        (0..self.civilization_count)
            .map(|owner| self.observe_government(owner))
            .collect()
    }
    pub(super) fn observe_government(&self, owner: usize) -> GovernmentSnapshot {
        let player = &self.players[owner];
        let cache = self.ai_government_cache.get(&owner);
        GovernmentSnapshot {
            owner,
            name: player.name.clone(),
            ai: player.ai,
            alive: self.player_alive(owner),
            cities: self.cities.iter().filter(|c| c.owner == owner).count(),
            current: player.government,
            revolution_target: player.revolution_target,
            revolution_turn: player.revolution_turn,
            evaluated_turn: cache.map(|c| c.evaluated_turn),
            next_evaluation: cache.map(|c| c.next_evaluation),
            best_available: cache.map(|c| self.government_goals(owner, &c.scores).1),
            research_goal: cache.map(|c| self.government_goals(owner, &c.scores).0),
            candidates: Government::targets()
                .iter()
                .map(|&government| Candidate {
                    government,
                    available: self.government_available(owner, government),
                    score: cache.map(|c| c.scores[government as usize]),
                })
                .collect(),
        }
    }
    pub(super) fn trace_government_change(
        &mut self,
        owner: usize,
        previous: Government,
        reason: &'static str,
    ) {
        self.trace.emit(
            self.turn,
            || vec![trace::Entity::Civilization(owner)],
            || GovernmentChange {
                owner,
                previous,
                current: self.players[owner].government,
                target: self.players[owner].revolution_target,
                reason,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn political_observation_and_tracing_preserve_simulation_results() {
        let mut plain = Simulation::new(crate::mapgen::generate(42));
        plain.enable_all_civilizations_as_ai();
        let mut observed = plain.clone();
        observed.trace = trace::Collector::enabled(["*".to_owned()].into(), 10000);
        for _ in 0..2 {
            observed.observe_governments();
            observed.observe_diplomacy();
            plain.simulate_turn();
            observed.simulate_turn();
        }
        assert_eq!(
            plain.encode_campaign().unwrap(),
            observed.encode_campaign().unwrap()
        );
    }
    #[test]
    fn observing_does_not_populate_or_refresh_advice() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.ai_government_cache.clear();
        let rng = game.rng;
        let before = game.encode_campaign().unwrap();
        let observations = game.observe_governments();
        assert!(
            observations
                .iter()
                .all(|p| p.evaluated_turn.is_none()
                    && p.candidates.iter().all(|c| c.score.is_none()))
        );
        assert!(game.ai_government_cache.is_empty());
        assert_eq!(rng, game.rng);
        game.observe_diplomacy();
        assert_eq!(before, game.encode_campaign().unwrap());
        game.refresh_government_advisor(1);
        let evaluated = game.ai_government_cache[&1].evaluated_turn;
        game.turn += 30;
        let observation = game.observe_government(1);
        assert_eq!(observation.evaluated_turn, Some(evaluated));
        assert_eq!(game.ai_government_cache[&1].evaluated_turn, evaluated);
    }
    #[test]
    fn decisions_include_scores_and_actual_adoption_is_a_separate_event() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.trace = trace::Collector::enabled(["*".to_owned()].into(), 1000);
        game.create_city(1, 10, 20);
        game.choose_ai_government(1);
        let events = game.trace.drain();
        let decision = events
            .iter()
            .find(|e| e.kind == "government.decision")
            .unwrap();
        let decision: GovernmentDecision =
            serde_json::from_value(decision.payload.clone()).unwrap();
        assert_eq!(Some(decision.selected), decision.advice.best_available);
        assert!(decision.advice.candidates.iter().all(|c| c.score.is_some()));
        game.players[1].government = Government::Despotism;
        game.players[1].known.push(Tech::MONARCHY);
        assert!(game.change_government(1, Government::Monarchy));
        game.finish_revolutions();
        let events = game.trace.drain();
        assert!(
            events
                .iter()
                .any(|e| e.payload["reason"] == "revolution_started")
        );
        assert!(
            events
                .iter()
                .any(|e| e.payload["reason"] == "adopted" && e.payload["current"] == "Monarchy")
        );
    }
}
