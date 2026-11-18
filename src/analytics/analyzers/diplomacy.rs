//! Diplomatic activity counts and end-of-turn relationship context.
use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{
    observation::{DiplomacySnapshot, Snapshot},
    trace::Event,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
#[derive(Default)]
pub(crate) struct Diplomacy {
    counts: BTreeMap<String, u64>,
    dropped: u64,
    recent_activity: VecDeque<Value>,
}
impl Analyzer for Diplomacy {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "diplomacy",
            version: 2,
            events: vec!["diplomacy.transition"],
            providers: vec!["diplomacy"],
            configuration: json!({"scope":"activity_and_outcomes", "recent_activity_limit":32}),
        }
    }
    fn on_start(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        self.on_boundary(snapshot, out);
    }
    fn on_event(&mut self, event: &Event, out: &mut AnalysisOutput) {
        if let Some(action) = event.payload["action"].as_str() {
            *self.counts.entry(action.to_owned()).or_default() += 1;
            out.metric("transitions", json!(self.counts));
            self.recent_activity
                .push_back(json!({"turn":event.turn,"evidence":out.evidence(),
                "action":action,"actor":event.payload["actor"],"other":event.payload["other"],
                "previous":event.payload["previous"],"current":event.payload["current"],
                "reason":event.payload["reason"]}));
            if self.recent_activity.len() > 32 {
                self.recent_activity.pop_front();
            }
            out.metric("recent_activity", json!(self.recent_activity));
        }
    }
    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        if let Some(pairs) = snapshot.get::<Vec<DiplomacySnapshot>>("diplomacy") {
            out.metric(
                "relationships",
                json!(
                    pairs
                        .iter()
                        .filter(|p| p.first_contact.is_some())
                        .map(
                            |p| json!({"players":p.players,"names":p.names,"alive":p.alive,
                    "pact":p.pact,"until":p.until,"trust":p.trust,"war_plans":p.war_plans,
                    "pending_offer":p.offer,"evidence":{"snapshot_turn":snapshot.turn}})
                        )
                        .collect::<Vec<_>>()
                ),
            );
            out.metric(
                "contacted_pairs",
                pairs.iter().filter(|p| p.first_contact.is_some()).count(),
            );
            out.metric(
                "active_war_plans",
                pairs
                    .iter()
                    .flat_map(|p| p.war_plans.iter())
                    .flatten()
                    .count(),
            );
        }
    }
    fn on_gap(&mut self, dropped: u64, out: &mut AnalysisOutput) {
        self.dropped += dropped;
        out.metric("dropped_events", self.dropped);
        out.metric("event_counts_complete", false);
    }
    fn finish(&mut self, out: &mut AnalysisOutput) {
        out.metric("transitions", json!(self.counts));
        out.metric("event_counts_complete", self.dropped == 0);
        out.metric("recent_activity", json!(self.recent_activity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::Phase;
    #[test]
    fn counts_actions_and_marks_overflow_incomplete() {
        let mut analyzer = Diplomacy::default();
        let mut out = AnalysisOutput::default();
        out.context("diplomacy", 1, None, Some((1, true)));
        for action in [
            "offer_proposed",
            "offer_rejected",
            "offer_proposed",
            "offer_accepted",
        ] {
            analyzer.on_event(
                &Event {
                    sequence: 1,
                    turn: 1,
                    phase: Phase::AiPlanning,
                    source: "diplomacy".into(),
                    kind: "diplomacy.transition".into(),
                    schema_version: 1,
                    entities: vec![],
                    cause: None,
                    payload: json!({"action":action}),
                },
                &mut out,
            );
        }
        analyzer.on_gap(2, &mut out);
        analyzer.finish(&mut out);
        assert_eq!(out.metrics["diplomacy.transitions"]["offer_proposed"], 2);
        assert_eq!(out.metrics["diplomacy.transitions"]["offer_accepted"], 1);
        assert_eq!(out.metrics["diplomacy.event_counts_complete"], false);
    }
}
