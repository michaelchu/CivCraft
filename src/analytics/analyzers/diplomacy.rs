//! Diplomatic activity counts and end-of-turn relationship context.
use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::{
    Pact,
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
    trade_counts: BTreeMap<String, u64>,
    recent_trades: VecDeque<Value>,
    memory_counts: BTreeMap<String, u64>,
    recent_memories: VecDeque<Value>,
    negotiation_counts: BTreeMap<String, u64>,
    recent_negotiations: VecDeque<Value>,
}
impl Analyzer for Diplomacy {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "diplomacy",
            version: 7,
            events: vec![
                "diplomacy.transition",
                "diplomacy.trade_decision",
                "diplomacy.relationship_memory",
                "diplomacy.negotiation_decision",
            ],
            providers: vec!["diplomacy"],
            configuration: json!({"scope":"activity_and_outcomes", "recent_activity_limit":32}),
        }
    }
    fn on_start(&mut self, snapshot: &Snapshot, out: &mut AnalysisOutput) {
        self.on_boundary(snapshot, out);
    }
    fn on_event(&mut self, event: &Event, out: &mut AnalysisOutput) {
        if event.kind == "diplomacy.negotiation_decision" {
            if let Some(reason) = event.payload["reason"].as_str() {
                *self
                    .negotiation_counts
                    .entry(reason.to_owned())
                    .or_default() += 1;
                self.recent_negotiations.push_back(json!({"turn":event.turn,
                    "evidence":out.evidence(),"decision":event.payload}));
                if self.recent_negotiations.len() > 8 {
                    self.recent_negotiations.pop_front();
                }
                out.metric("negotiation_decisions", json!(self.negotiation_counts));
                out.metric(
                    "recent_negotiation_decisions",
                    json!(self.recent_negotiations),
                );
            }
            return;
        }
        if event.kind == "diplomacy.relationship_memory" {
            if let Some(reason) = event.payload["reason"].as_str() {
                *self.memory_counts.entry(reason.to_owned()).or_default() += 1;
                self.recent_memories.push_back(
                    json!({"turn":event.turn,"evidence":out.evidence(),"memory":event.payload}),
                );
                if self.recent_memories.len() > 8 {
                    self.recent_memories.pop_front();
                }
                out.metric("relationship_memories", json!(self.memory_counts));
                out.metric("recent_relationship_memories", json!(self.recent_memories));
            }
            return;
        }
        if event.kind == "diplomacy.trade_decision" {
            if let Some(reason) = event.payload["reason"].as_str() {
                *self.trade_counts.entry(reason.to_owned()).or_default() += 1;
                self.recent_trades.push_back(json!({"turn":event.turn,
                    "evidence":out.evidence(),"decision":event.payload}));
                if self.recent_trades.len() > 8 {
                    self.recent_trades.pop_front();
                }
                out.metric("trade_decisions", json!(self.trade_counts));
                out.metric("recent_trade_decisions", json!(self.recent_trades));
            }
            return;
        }
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
                    "pact":p.pact,"vassalage":p.vassalage,"independence_reason":p.independence_reason,"until":p.until,"trust":p.trust,"relationship_memory":p.relationship_memory,"war_plans":p.war_plans,
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
                    .filter(|p| {
                        p.first_contact.is_some()
                            && p.alive.iter().all(|&alive| alive)
                            && p.pact.is_some_and(|pact| pact != Pact::War)
                    })
                    .flat_map(|p| p.war_plans.iter())
                    .flatten()
                    .filter(|plan| plan.countdown >= 0)
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

    #[test]
    fn relationship_incidents_have_separate_counts_and_bounded_evidence() {
        let mut analyzer = Diplomacy::default();
        let mut out = AnalysisOutput::default();
        out.context("diplomacy", 1, None, Some((3, true)));
        for turn in 1..=12 {
            analyzer.on_event(
                &Event {
                    turn,
                    sequence: turn as u64,
                    phase: Phase::AiPlanning,
                    source: "diplomacy".into(),
                    kind: "diplomacy.relationship_memory".into(),
                    schema_version: 1,
                    cause: None,
                    entities: Vec::new(),
                    payload: json!({"reason":"completed_trade","observer":1,"other":2}),
                },
                &mut out,
            );
        }
        assert_eq!(
            out.metrics["diplomacy.relationship_memories"]["completed_trade"],
            12
        );
        assert_eq!(
            out.metrics["diplomacy.recent_relationship_memories"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            out.metrics["diplomacy.recent_relationship_memories"][0]["turn"],
            5
        );
        assert!(analyzer.counts.is_empty());
    }
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

    #[test]
    fn trade_decisions_have_separate_counts_and_bounded_evidence() {
        let mut analyzer = Diplomacy::default();
        let mut out = AnalysisOutput::default();
        out.context("diplomacy", 1, None, Some((3, true)));
        for turn in 1..=12 {
            analyzer.on_event(&Event {
                turn, sequence: turn as u64, phase: Phase::AiPlanning,
                source: "diplomacy".into(), kind: "diplomacy.trade_decision".into(),
                schema_version: 1, entities: vec![], cause: None,
                payload: json!({"actor":1,"other":2,"reason":"no_viable_trade","selected":null}),
            }, &mut out);
        }
        assert_eq!(
            out.metrics["diplomacy.trade_decisions"]["no_viable_trade"],
            12
        );
        assert_eq!(
            out.metrics["diplomacy.recent_trade_decisions"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert!(!out.metrics.contains_key("diplomacy.transitions"));
    }
}
