//! Example event-only plugin: no snapshots and no changes to runner/reporting.
use crate::analytics::{AnalysisOutput, Analyzer, Descriptor};
use crate::sim::trace::Event;
use serde_json::json;
#[derive(Default)]
pub(crate) struct Production {
    completions: u64,
}
impl Analyzer for Production {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "production",
            version: 1,
            events: vec!["production.completed"],
            providers: vec![],
            configuration: json!({}),
        }
    }
    fn on_event(&mut self, _: &Event, out: &mut AnalysisOutput) {
        self.completions += 1;
        out.metric("completed_items", self.completions);
    }
    fn finish(&mut self, out: &mut AnalysisOutput) {
        out.metric("completed_items", self.completions);
    }
}
