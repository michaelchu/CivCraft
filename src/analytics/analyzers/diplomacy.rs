//! Diplomatic activity counts and end-of-turn relationship context.
use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{
    observation::{DiplomacySnapshot, Snapshot},
    trace::Event,
};
use serde_json::json;
use std::collections::BTreeMap;
#[derive(Default)]
pub(crate) struct Diplomacy {
    counts: BTreeMap<String, u64>,
    dropped: u64,
}
impl Analyzer for Diplomacy {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "diplomacy",
            version: 1,
            events: vec!["diplomacy.transition"],
            providers: vec!["diplomacy"],
            configuration: json!({"scope":"activity_and_outcomes"}),
        }
    }
    fn on_event(&mut self, event: &Event, out: &mut AnalysisOutput) {
        if let Some(action) = event.payload["action"].as_str() {
            *self.counts.entry(action.to_owned()).or_default() += 1;
            out.metric("transitions", json!(self.counts));
        }
    }
    fn on_boundary(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        if let Some(pairs) = snapshot.get::<Vec<DiplomacySnapshot>>("diplomacy") {
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
