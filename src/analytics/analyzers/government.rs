//! Compares decisions with their published advisor assessment, not long-term optimality.
use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{
    observation::{GovernmentDecision, GovernmentSnapshot, Snapshot},
    trace::Event,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
#[derive(Default)]
pub(crate) struct Government {
    decisions: u64,
    changes: u64,
    gaps: u64,
    outcomes: BTreeMap<String, u64>,
    recent_changes: VecDeque<Value>,
    /// Latest revolution per owner: (turn, previous, target).
    last_revolution: BTreeMap<u64, (i64, String, String)>,
}

/// A return to the government just left within this many turns is churn.
const FLIP_FLOP_WINDOW_TURNS: i64 = 30;

impl Government {
    /// Flags a revolution into the current government (a disorder loop) and a
    /// quick return to the government just left (a flip-flop).
    fn check_revolution(&mut self, event: &Event, out: &mut AnalysisOutput) {
        let payload = &event.payload;
        if payload["reason"] != "revolution_started" {
            return;
        }
        let (Some(owner), Some(previous), Some(target)) = (
            payload["owner"].as_u64(),
            payload["previous"].as_str(),
            payload["target"].as_str(),
        ) else {
            return;
        };
        let turn = i64::from(event.turn);
        let subject = format!("civilization:{owner}");
        out.check(
            subject.clone(),
            "government_revolution_repeats_current",
            event.entities.clone(),
            previous == target,
            "A revolution started into the government the nation already has (for example a disorder loop under Republic or Democracy).",
            payload.clone(),
        );
        let flip = self
            .last_revolution
            .get(&owner)
            .is_some_and(|(at, from, to)| {
                turn - at <= FLIP_FLOP_WINDOW_TURNS && from == target && to == previous
            });
        out.check(
            subject,
            "government_flip_flop",
            event.entities.clone(),
            flip,
            "A nation returned to the government it left within a few turns; check the biased advisor scores and the retention margin.",
            json!({"previous_revolution": self.last_revolution.get(&owner), "revolution": payload}),
        );
        self.last_revolution
            .insert(owner, (turn, previous.to_owned(), target.to_owned()));
    }

    fn overview(snapshot: &Snapshot, out: &mut AnalysisOutput) {
        let Some(players) = snapshot.get::<Vec<GovernmentSnapshot>>("governments") else {
            return;
        };
        out.metric(
            "ai_nations_observed",
            players.iter().filter(|p| p.ai && p.alive).count(),
        );
        out.metric(
            "nations",
            json!(players.iter().filter(|p| p.alive).map(|p| json!({
            "owner":p.owner,"name":p.name,"ai":p.ai,"cities":p.cities,
            "current":p.current,"best_available":p.best_available,"research_goal":p.research_goal,
            "revolution_target":p.revolution_target,
            "advice_age_turns":p.evaluated_turn.map(|turn| snapshot.turn-turn),
            "evaluated_turn":p.evaluated_turn,"next_evaluation":p.next_evaluation,
            "evidence":{"snapshot_turn":snapshot.turn}
        })).collect::<Vec<_>>()),
        );
    }
}
impl Analyzer for Government {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "government",
            version: 3,
            events: vec!["government.decision", "government.changed"],
            providers: vec!["governments"],
            configuration: json!({"scope":"consistency_with_cached_advisor", "recent_changes_limit":32}),
        }
    }
    fn on_start(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        Self::overview(snapshot, out);
    }
    fn on_event(&mut self, event: &Event, out: &mut AnalysisOutput) {
        if event.kind == "government.decision" {
            self.decisions += 1;
        } else {
            self.changes += 1;
            self.check_revolution(event, out);
            self.recent_changes
                .push_back(json!({"turn":event.turn, "phase":event.phase,
                "evidence":out.evidence(),"change":event.payload}));
            if self.recent_changes.len() > 32 {
                self.recent_changes.pop_front();
            }
            out.metric("recent_changes", json!(self.recent_changes));
        }
        if let Some(outcome) = event.payload.get("outcome").and_then(Value::as_str) {
            *self.outcomes.entry(outcome.to_owned()).or_default() += 1;
            out.metric("decision_outcomes", json!(self.outcomes));
        }
        out.metric("decisions", self.decisions);
        out.metric("changes", self.changes);
        if event.kind == "government.decision"
            && let Ok(decision) =
                serde_json::from_value::<GovernmentDecision>(event.payload.clone())
            && decision.outcome != "deferred"
        {
            let mismatch = decision
                .advice
                .best_available
                .as_ref()
                .is_some_and(|best| *best != decision.selected);
            out.check(format!("civilization:{}", decision.advice.owner), "selection_differs_from_advisor",
                        event.entities.clone(), mismatch,
                        "Selected government differs from the cached best available government; this checks advisor consistency, not long-term optimality.", event.payload.clone());
        }
    }
    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        let Some(players) = snapshot.get::<Vec<GovernmentSnapshot>>("governments") else {
            return;
        };
        for player in players.iter().filter(|p| p.ai && p.alive && p.cities > 0) {
            let mismatch = player.revolution_target.is_none()
                && player
                    .best_available
                    .is_some_and(|best| best != player.current);
            out.check(format!("adoption:{}", player.owner), "government_differs_from_advisor",
                vec![crate::sim::trace::Entity::Civilization(player.owner)], mismatch,
                "AI government differs from its cached best available choice with no pending revolution. The assessment may be stale; inspect evaluated_turn and candidate scores.",
                json!(player));
        }
        Self::overview(snapshot, out);
    }
    fn on_gap(&mut self, dropped: u64, out: &mut AnalysisOutput) {
        self.gaps += dropped;
        out.metric("dropped_events", self.gaps);
        out.metric("event_counts_complete", false);
    }
    fn finish(&mut self, out: &mut AnalysisOutput) {
        out.metric("decisions", self.decisions);
        out.metric("changes", self.changes);
        out.metric("event_counts_complete", self.gaps == 0);
        out.metric("decision_outcomes", json!(self.outcomes));
        out.metric("recent_changes", json!(self.recent_changes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::{Entity, Phase};
    #[test]
    fn flip_flops_and_repeat_revolutions_are_findings() {
        let mut analyzer = Government::default();
        let mut out = AnalysisOutput::default();
        let revolution = |turn: i32, previous: &str, target: &str| Event {
            sequence: 1,
            turn,
            phase: Phase::AiPlanning,
            source: "government".into(),
            kind: "government.changed".into(),
            schema_version: 1,
            entities: vec![Entity::Civilization(1)],
            cause: None,
            payload: json!({"owner":1,"previous":previous,"target":target,
                "current":"Anarchy","reason":"revolution_started"}),
        };
        out.context("government", 1, None, Some((1, true)));
        analyzer.on_event(&revolution(10, "Monarchy", "Republic"), &mut out);
        analyzer.on_event(&revolution(30, "Republic", "Monarchy"), &mut out);
        analyzer.on_event(&revolution(90, "Monarchy", "Republic"), &mut out);
        analyzer.on_event(&revolution(100, "Republic", "Republic"), &mut out);
        let kinds: Vec<_> = out.findings.iter().map(|f| f.kind.as_str()).collect();
        assert!(kinds.contains(&"government_flip_flop"), "{kinds:?}");
        assert!(
            kinds.contains(&"government_revolution_repeats_current"),
            "{kinds:?}"
        );
        // The turn-90 change is 60 turns after the last one: no second flip.
        assert_eq!(
            kinds
                .iter()
                .filter(|k| **k == "government_flip_flop")
                .count(),
            1
        );
    }

    #[test]
    fn mismatch_resolves_and_deferred_decisions_do_not_claim_failure() {
        let mut analyzer = Government::default();
        let mut out = AnalysisOutput::default();
        let mut event = Event {
            sequence: 1,
            turn: 1,
            phase: Phase::AiPlanning,
            source: "government".into(),
            kind: "government.decision".into(),
            schema_version: 1,
            entities: vec![Entity::Civilization(1)],
            cause: None,
            payload: json!({"advice": {"owner":1,"ai":true,"alive":true,"cities":1,
                "current":"Despotism","revolution_target":null,"revolution_turn":null,
                "evaluated_turn":1,"next_evaluation":6,"best_available":"Monarchy",
                "research_goal":"Monarchy","candidates":[]}, "selected":"Despotism","outcome":"retained"}),
        };
        out.context("government", 1, None, Some((1, true)));
        analyzer.on_event(&event, &mut out);
        assert_eq!(out.findings.len(), 1);
        event.payload["outcome"] = json!("deferred");
        analyzer.on_event(&event, &mut out);
        assert_eq!(out.findings[0].status, "open");
        event.payload["outcome"] = json!("retained");
        event.payload["selected"] = json!("Monarchy");
        out.context("government", 2, None, Some((2, true)));
        analyzer.on_event(&event, &mut out);
        assert_eq!(out.findings[0].resolved_turn, Some(2));
        analyzer.on_gap(3, &mut out);
        analyzer.finish(&mut out);
        assert_eq!(out.metrics["government.event_counts_complete"], false);
    }
}
