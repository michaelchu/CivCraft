//! Compares decisions with their published advisor assessment, not long-term optimality.
use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{
    observation::{GovernmentDecision, GovernmentSnapshot, Snapshot},
    trace::Event,
};
use serde_json::json;
#[derive(Default)]
pub(crate) struct Government {
    decisions: u64,
    changes: u64,
    gaps: u64,
}
impl Analyzer for Government {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "government",
            version: 1,
            events: vec!["government.decision", "government.changed"],
            providers: vec!["governments"],
            configuration: json!({"scope":"consistency_with_cached_advisor"}),
        }
    }
    fn on_event(&mut self, event: &Event, out: &mut AnalysisOutput) {
        if event.kind == "government.decision" {
            self.decisions += 1;
        } else {
            self.changes += 1;
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
        out.metric(
            "ai_nations_observed",
            players.iter().filter(|p| p.ai).count(),
        );
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::{Entity, Phase};
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
