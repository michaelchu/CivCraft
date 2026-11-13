//! Independent observers. Adding an analyzer requires a module and registry entry,
//! never a change to the simulation loop or reporter.
pub(crate) mod analyzers;
pub(crate) mod finding;
pub(crate) mod providers;
pub(crate) mod registry;

use crate::sim::{observation::Snapshot, trace::Event};
pub(crate) use finding::AnalysisOutput;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Serialize)]
pub(crate) struct Descriptor {
    pub id: &'static str,
    pub version: u16,
    pub events: Vec<&'static str>,
    pub providers: Vec<&'static str>,
    pub configuration: Value,
}

pub(crate) trait Analyzer {
    fn descriptor(&self) -> Descriptor;
    fn on_start(&mut self, _snapshot: &Snapshot, _out: &mut AnalysisOutput) {}
    fn on_event(&mut self, _event: &Event, _out: &mut AnalysisOutput) {}
    fn on_boundary(&mut self, _snapshot: &Snapshot, _out: &mut AnalysisOutput) {}
    fn on_gap(&mut self, _dropped: u64, _out: &mut AnalysisOutput) {}
    fn finish(&mut self, _out: &mut AnalysisOutput) {}
}

pub(crate) struct Registry {
    analyzers: Vec<Box<dyn Analyzer>>,
    pub output: AnalysisOutput,
}
impl Registry {
    pub fn new(analyzers: Vec<Box<dyn Analyzer>>) -> Result<Self, String> {
        let ids = analyzers
            .iter()
            .map(|a| a.descriptor().id)
            .collect::<BTreeSet<_>>();
        if ids.len() != analyzers.len() {
            return Err("duplicate analyzer IDs".into());
        }
        Ok(Self {
            analyzers,
            output: AnalysisOutput::default(),
        })
    }
    pub fn descriptors(&self) -> Vec<Descriptor> {
        self.analyzers.iter().map(|a| a.descriptor()).collect()
    }
    pub fn providers(&self) -> BTreeSet<&'static str> {
        self.descriptors()
            .into_iter()
            .flat_map(|a| a.providers)
            .collect()
    }
    pub fn event_kinds(&self) -> BTreeSet<String> {
        self.descriptors()
            .into_iter()
            .flat_map(|a| a.events)
            .map(str::to_owned)
            .collect()
    }
    pub fn start(&mut self, snapshot: &Snapshot) {
        for analyzer in &mut self.analyzers {
            self.output.context(
                analyzer.descriptor().id,
                snapshot.turn,
                Some(snapshot.turn),
                None,
            );
            analyzer.on_start(snapshot, &mut self.output);
        }
    }
    pub fn event(&mut self, event: &Event, retained: bool) {
        for analyzer in &mut self.analyzers {
            if !analyzer.descriptor().events.contains(&event.kind.as_str()) {
                continue;
            }
            self.output.context(
                analyzer.descriptor().id,
                event.turn,
                None,
                Some((event.sequence, retained)),
            );
            analyzer.on_event(event, &mut self.output);
        }
    }
    pub fn boundary(&mut self, snapshot: &Snapshot) {
        for analyzer in &mut self.analyzers {
            self.output.context(
                analyzer.descriptor().id,
                snapshot.turn,
                Some(snapshot.turn),
                None,
            );
            analyzer.on_boundary(snapshot, &mut self.output);
        }
    }
    pub fn gap(&mut self, dropped: u64) {
        if dropped == 0 {
            return;
        }
        self.output.event_coverage_complete = false;
        for analyzer in &mut self.analyzers {
            self.output.finish_context(analyzer.descriptor().id);
            analyzer.on_gap(dropped, &mut self.output);
        }
    }
    pub fn finish(&mut self) {
        for analyzer in &mut self.analyzers {
            self.output.finish_context(analyzer.descriptor().id);
            analyzer.finish(&mut self.output);
        }
    }
}
