//! Presentation-only event compaction. Analyzer input and simulation state are untouched.
use crate::sim::{observation::GovernmentDecision, trace::Event};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct EventCompaction {
    decisions: BTreeMap<usize, GovernmentDecision>,
    funding: BTreeMap<u64, serde_json::Value>,
    trades: BTreeMap<(u64, u64), serde_json::Value>,
    pub counts: BTreeMap<String, usize>,
}
#[derive(serde::Deserialize)]
struct Growth {
    outcome: String,
    food_before: i32,
    food_after: i32,
    population_before: i32,
    population_after: i32,
    applied_surplus: i32,
}
#[derive(serde::Deserialize)]
struct Search {
    selected: bool,
}
impl EventCompaction {
    fn funding_signature(event: &Event) -> Option<(u64, serde_json::Value)> {
        if event.kind != "economy.funding_decision" {
            return None;
        }
        Some((
            event.payload["owner"].as_u64()?,
            serde_json::json!({
            "rates": event.payload.get("rates")?, "reason": event.payload.get("reason")?,
            "target_met": event.payload.get("target_met")?}),
        ))
    }

    fn decision(event: &Event) -> Option<GovernmentDecision> {
        (event.kind == "government.decision")
            .then(|| serde_json::from_value(event.payload.clone()).ok())
            .flatten()
    }
    pub fn redundant(&mut self, event: &Event) -> bool {
        if event.schema_version != 1 || event.cause.is_some() {
            return false;
        }
        let routine = match event.kind.as_str() {
            "economy.funding_decision" => Self::funding_signature(event)
                .is_some_and(|(owner, signature)| self.funding.get(&owner) == Some(&signature)),
            "diplomacy.trade_decision" => {
                event.payload.get("selected") == Some(&serde_json::Value::Null)
                    && event.payload["actor"]
                        .as_u64()
                        .zip(event.payload["other"].as_u64())
                        .is_some_and(|pair| self.trades.get(&pair) == Some(&event.payload))
            }
            "cities.growth_applied" => serde_json::from_value::<Growth>(event.payload.clone())
                .ok()
                .is_some_and(|g| {
                    g.outcome == "unchanged"
                        && g.population_before == g.population_after
                        && g.applied_surplus >= 0
                        && i64::from(g.food_after) - i64::from(g.food_before)
                            == i64::from(g.applied_surplus)
                }),
            "settlement.site_search_completed" => {
                serde_json::from_value::<Search>(event.payload.clone())
                    .ok()
                    .is_some_and(|s| s.selected)
            }
            _ => false,
        };
        if routine {
            *self.counts.entry(event.kind.clone()).or_default() += 1;
            return true;
        }
        let Some(decision) = Self::decision(event) else {
            return false;
        };
        // Always persist exceptional decisions, even if the exception repeats.
        let redundant = decision.outcome == "retained"
            && decision.advice.best_available == Some(decision.selected)
            && self.decisions.get(&decision.advice.owner) == Some(&decision);
        if redundant {
            *self.counts.entry(event.kind.clone()).or_default() += 1;
        }
        redundant
    }
    /// Only successfully persisted events can serve as a compacted event's baseline.
    pub fn retained(&mut self, event: &Event) {
        if let Some((owner, signature)) = Self::funding_signature(event) {
            self.funding.insert(owner, signature);
        }
        if event.kind == "diplomacy.trade_decision"
            && let Some(pair) = event.payload["actor"]
                .as_u64()
                .zip(event.payload["other"].as_u64())
        {
            self.trades.insert(pair, event.payload.clone());
        }
        if let Some(decision) = Self::decision(event) {
            self.decisions.insert(decision.advice.owner, decision);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::Phase;
    use serde_json::json;
    pub(super) fn decision() -> Event {
        Event {
            sequence: 1,
            turn: 1,
            phase: Phase::AiPlanning,
            source: "government".into(),
            kind: "government.decision".into(),
            schema_version: 1,
            entities: vec![],
            cause: None,
            payload: json!({"advice":{"owner":1,"ai":true,"alive":true,"cities":1,
                "current":"Despotism","revolution_target":null,"revolution_turn":null,
                "evaluated_turn":1,"next_evaluation":6,"best_available":"Despotism",
                "research_goal":"Monarchy","candidates":[{"government":"Monarchy","available":false,"score":100.0}]},
                "selected":"Despotism","outcome":"retained"}),
        }
    }

    #[test]
    fn unsuccessful_trade_decisions_compact_only_against_retained_identical_evidence() {
        let mut compaction = EventCompaction::default();
        let mut event = decision();
        event.kind = "diplomacy.trade_decision".into();
        event.payload = json!({"actor":1,"other":2,"reason":"no_viable_trade",
            "candidates":2,"rejected":{"price_or_reserve":2},"reserve":10,"selected":null});
        assert!(!compaction.redundant(&event));
        assert!(!compaction.redundant(&event));
        compaction.retained(&event);
        assert!(compaction.redundant(&event));
        event.payload["reserve"] = json!(20);
        assert!(!compaction.redundant(&event));
        event.payload["selected"] = json!({"actor_gain":10,"partner_gain":10});
        compaction.retained(&event);
        assert!(!compaction.redundant(&event));
    }
    #[test]
    fn compacts_only_successfully_persisted_unchanged_decisions() {
        let mut policy = EventCompaction::default();
        let event = decision();
        assert!(!policy.redundant(&event));
        // A filtered or byte-limited event must not become a baseline.
        assert!(!policy.redundant(&event));
        policy.retained(&event);
        let mut next = event.clone();
        next.turn += 1;
        next.sequence += 1;
        assert!(policy.redundant(&next));
        assert_eq!(policy.counts["government.decision"], 1);
        next.payload["advice"]["evaluated_turn"] = json!(2);
        assert!(!policy.redundant(&next));
        next = event.clone();
        next.payload["advice"]["candidates"][0]["available"] = json!(true);
        assert!(!policy.redundant(&next));
    }
    #[test]
    fn never_compacts_exceptions_transitions_or_unrecognized_payloads() {
        let mut policy = EventCompaction::default();
        for outcome in ["revolution_started", "rejected", "deferred"] {
            let mut event = decision();
            event.payload["outcome"] = json!(outcome);
            policy.retained(&event);
            assert!(!policy.redundant(&event));
        }
        let mut event = decision();
        event.payload["selected"] = json!("Monarchy");
        policy.retained(&event);
        assert!(!policy.redundant(&event));
        for kind in ["government.changed", "diplomacy.transition", "custom.event"] {
            event.kind = kind.into();
            assert!(!policy.redundant(&event));
        }
        event.kind = "government.decision".into();
        event.payload = json!({});
        assert!(!policy.redundant(&event));
    }
    #[test]
    fn lean_growth_keeps_starvation_growth_and_unexpected_food_stalls() {
        let mut policy = EventCompaction::default();
        let mut event = decision();
        event.kind = "cities.growth_applied".into();
        event.payload = json!({"outcome":"unchanged","food_before":3,"food_after":5,
            "population_before":2,"population_after":2,"applied_surplus":2});
        assert!(policy.redundant(&event));
        event.payload["food_after"] = json!(4);
        assert!(!policy.redundant(&event));
        event.payload["food_after"] = json!(3);
        assert!(!policy.redundant(&event));
        for outcome in ["starved", "destroyed", "grew"] {
            event.payload["outcome"] = json!(outcome);
            assert!(!policy.redundant(&event));
        }
    }
}

#[cfg(test)]
mod funding_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn funding_compaction_keeps_rate_reason_and_outcome_changes_and_all_real_losses() {
        let mut compactor = EventCompaction::default();
        let mut event = super::tests::decision();
        event.kind = "economy.funding_decision".into();
        event.source = "economy".into();
        event.payload =
            json!({"owner":1,"rates":[70,30,0],"reason":"research","target_met":true,"gold":100});
        assert!(!compactor.redundant(&event));
        assert!(!compactor.redundant(&event)); // A dropped event cannot establish a baseline.
        compactor.retained(&event);
        event.payload["gold"] = json!(110);
        assert!(compactor.redundant(&event));
        for (key, value) in [
            ("rates", json!([60, 40, 0])),
            ("reason", json!("reserve_refill")),
            ("target_met", json!(false)),
        ] {
            let mut changed = event.clone();
            changed.payload[key] = value;
            assert!(!compactor.redundant(&changed));
        }
        event.kind = "economy.upkeep_shortfall".into();
        compactor.retained(&event);
        assert!(!compactor.redundant(&event));
    }
}
