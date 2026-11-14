use crate::sim::trace::Entity;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct EvidenceRef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_turn: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_sequence: Option<u64>,
    pub available: bool,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Finding {
    pub id: u64,
    pub analyzer: String,
    pub key: String,
    pub kind: String,
    pub classification: String,
    pub severity: String,
    pub entities: Vec<Entity>,
    pub status: String,
    pub first_observed_turn: i32,
    pub last_observed_turn: i32,
    pub resolved_turn: Option<i32>,
    pub observation_count: u64,
    pub explanation: String,
    pub latest_evidence: Value,
    pub evidence: Vec<EvidenceRef>,
}

pub(crate) struct AnalysisOutput {
    pub findings: Vec<Finding>,
    pub event_coverage_complete: bool,
    pub metrics: BTreeMap<String, Value>,
    active: BTreeMap<(String, String), usize>,
    analyzer: String,
    turn: i32,
    evidence: Option<EvidenceRef>,
}
impl Default for AnalysisOutput {
    fn default() -> Self {
        Self {
            findings: vec![],
            event_coverage_complete: true,
            metrics: Default::default(),
            active: Default::default(),
            analyzer: String::new(),
            turn: 0,
            evidence: None,
        }
    }
}
impl AnalysisOutput {
    pub fn context(
        &mut self,
        analyzer: &str,
        turn: i32,
        snapshot: Option<i32>,
        event: Option<(u64, bool)>,
    ) {
        self.analyzer = analyzer.into();
        self.turn = turn;
        self.evidence = Some(EvidenceRef {
            snapshot_turn: snapshot,
            event_sequence: event.map(|e| e.0),
            available: event.is_none_or(|e| e.1),
        });
    }
    pub fn finish_context(&mut self, analyzer: &str) {
        self.analyzer = analyzer.into();
        self.evidence = None;
    }
    pub fn metric(&mut self, name: &str, value: impl Into<Value>) {
        self.metrics
            .insert(format!("{}.{}", self.analyzer, name), value.into());
    }
    /// Report an established invariant violation; heuristic checks should use `check`.
    #[cfg(test)]
    pub fn invariant(
        &mut self,
        key: String,
        kind: &str,
        entities: Vec<Entity>,
        violated: bool,
        explanation: &str,
        evidence: Value,
    ) {
        self.check(key.clone(), kind, entities, violated, explanation, evidence);
        if let Some(&index) = self.active.get(&(self.analyzer.clone(), key)) {
            self.findings[index].classification = "invariant_violation".into();
            self.findings[index].severity = "error".into();
        }
    }
    /// Explicit false resolves a finding. Silence or missing observations never resolve it.
    pub fn check(
        &mut self,
        key: String,
        kind: &str,
        entities: Vec<Entity>,
        active: bool,
        explanation: &str,
        evidence: Value,
    ) {
        let identity = (self.analyzer.clone(), key.clone());
        if !active {
            // A later retained event cannot establish recovery across an unknown gap.
            if !self.event_coverage_complete
                && self
                    .evidence
                    .as_ref()
                    .is_some_and(|e| e.event_sequence.is_some())
            {
                return;
            }
            if let Some(index) = self.active.remove(&identity) {
                let finding = &mut self.findings[index];
                finding.status = "resolved".into();
                finding.resolved_turn = Some(self.turn);
                if let Some(reference) = &self.evidence {
                    finding.evidence.push(reference.clone());
                }
            }
            return;
        }
        let index = *self.active.entry(identity).or_insert_with(|| {
            let index = self.findings.len();
            self.findings.push(Finding {
                id: index as u64 + 1,
                analyzer: self.analyzer.clone(),
                key,
                kind: kind.into(),
                classification: "heuristic_candidate".into(),
                severity: "warning".into(),
                entities,
                status: "open".into(),
                first_observed_turn: self.turn,
                last_observed_turn: self.turn,
                resolved_turn: None,
                observation_count: 0,
                explanation: explanation.into(),
                latest_evidence: Value::Null,
                evidence: Vec::new(),
            });
            index
        });
        let finding = &mut self.findings[index];
        finding.last_observed_turn = self.turn;
        finding.observation_count += 1;
        finding.latest_evidence = evidence;
        if let Some(reference) = &self.evidence
            && finding.evidence.last() != Some(reference)
        {
            finding.evidence.push(reference.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn recovery_and_recurrence_create_separate_episodes() {
        let mut out = AnalysisOutput::default();
        for (turn, active) in [(1, true), (2, true), (3, false), (5, true)] {
            out.context("example", turn, Some(turn), None);
            out.check(
                "city:1".into(),
                "stall",
                vec![Entity::City(1)],
                active,
                "explanation",
                json!({"turn":turn}),
            );
        }
        assert_eq!(out.findings.len(), 2);
        assert_eq!(out.findings[0].status, "resolved");
        assert_eq!(out.findings[0].observation_count, 2);
        assert_eq!(out.findings[0].resolved_turn, Some(3));
        assert_eq!(out.findings[1].first_observed_turn, 5);
        assert_ne!(out.findings[0].id, out.findings[1].id);
        // Another entity's negative observation must not resolve this one.
        out.check(
            "city:2".into(),
            "stall",
            vec![Entity::City(2)],
            false,
            "",
            json!({}),
        );
        assert_eq!(out.findings[1].status, "open");
    }
    #[test]
    fn unavailable_event_evidence_and_invariants_are_explicit() {
        let mut out = AnalysisOutput::default();
        out.context("example", 3, None, Some((7, false)));
        out.invariant(
            "unit:1".into(),
            "negative_hp",
            vec![Entity::Unit(1)],
            true,
            "HP must not be negative",
            json!({"hp":-1}),
        );
        let finding = &out.findings[0];
        assert_eq!(finding.classification, "invariant_violation");
        assert!(!finding.evidence[0].available);
        assert_eq!(finding.evidence[0].event_sequence, Some(7));
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn event_gaps_require_snapshot_recovery_instead_of_silent_resolution() {
        let mut out = AnalysisOutput::default();
        out.context("example", 1, None, Some((1, true)));
        out.check(
            "unit:1".into(),
            "stall",
            vec![Entity::Unit(1)],
            true,
            "",
            json!({}),
        );
        out.event_coverage_complete = false;
        out.context("example", 2, None, Some((3, true)));
        out.check(
            "unit:1".into(),
            "stall",
            vec![Entity::Unit(1)],
            false,
            "",
            json!({}),
        );
        assert_eq!(out.findings[0].status, "open");
        out.context("example", 3, Some(3), None);
        out.check(
            "unit:1".into(),
            "stall",
            vec![Entity::Unit(1)],
            false,
            "",
            json!({}),
        );
        assert_eq!(out.findings[0].status, "resolved");
    }
}
