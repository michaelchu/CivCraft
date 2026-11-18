//! Streaming evidence and atomic summary checkpoints. No domain-specific checks.
use super::{config::Config, scenario::Provenance};
use crate::{
    analytics::Registry,
    sim::{
        observation::Snapshot,
        trace::{Event, TurnProfile},
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    time::Instant,
};

pub(crate) struct Report {
    path: PathBuf,
    snapshot_path: PathBuf,
    event_path: PathBuf,
    changes_path: PathBuf,
    changes: BufWriter<File>,
    observation_changes: super::observation_changes::ObservationChanges,
    change_records: usize,
    last_snapshot: Option<(i32, crate::sim::trace::Phase)>,
    snapshots: BufWriter<File>,
    events: BufWriter<File>,
    started: Instant,
    commit: Option<String>,
    dirty: Option<bool>,
    snapshot_records: usize,
    observation_boundaries: usize,
    event_records: usize,
    event_bytes: usize,
    pub omitted_events: usize,
    timeline: Vec<Value>,
    scenario: Provenance,
    turn_profiles: Vec<TurnProfile>,
    minimap: Value,
    compaction: super::event_compaction::EventCompaction,
    verbose: bool,
    snapshot_codec: super::verbose_codec::Snapshots,
    shared_values: super::verbose_codec::SharedValues,
    shared_path: PathBuf,
    shared: BufWriter<File>,
    shared_bytes: usize,
    shared_records: usize,
}
impl Report {
    pub fn new(path: &str, scenario: Provenance) -> Result<Self, String> {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let filename = path
            .file_name()
            .ok_or("report path must name a file")?
            .to_string_lossy();
        let snapshot_path = path.with_file_name(format!("{filename}.snapshots.jsonl"));
        let event_path = path.with_file_name(format!("{filename}.events.jsonl"));
        let changes_path = path.with_file_name(format!("{filename}.changes.jsonl"));
        let shared_path = path.with_file_name(format!("{filename}.shared.jsonl"));
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_owned())
        };
        Ok(Self {
            snapshots: BufWriter::new(File::create(&snapshot_path).map_err(|e| e.to_string())?),
            events: BufWriter::new(File::create(&event_path).map_err(|e| e.to_string())?),
            changes: BufWriter::new(File::create(&changes_path).map_err(|e| e.to_string())?),
            changes_path,
            observation_changes: Default::default(),
            change_records: 0,
            last_snapshot: None,
            verbose: false,
            snapshot_codec: Default::default(),
            shared_values: Default::default(),
            shared: BufWriter::new(File::create(&shared_path).map_err(|e| e.to_string())?),
            shared_path,
            shared_bytes: 0,
            shared_records: 0,
            minimap: Value::Null,
            compaction: Default::default(),
            path,
            snapshot_path,
            event_path,
            started: Instant::now(),
            commit: git(&["rev-parse", "HEAD"]),
            dirty: git(&["status", "--porcelain"]).map(|s| !s.is_empty()),
            snapshot_records: 0,
            observation_boundaries: 0,
            event_records: 0,
            event_bytes: 0,
            omitted_events: 0,
            timeline: Vec::new(),
            scenario,
            turn_profiles: Vec::new(),
        })
    }
    fn persist_snapshot(&mut self, snapshot: &Snapshot) -> Result<(), String> {
        if self.last_snapshot == Some((snapshot.turn, snapshot.phase)) {
            return Ok(());
        }
        let value = serde_json::to_value(snapshot).map_err(|e| e.to_string())?;
        let stored = if self.verbose {
            self.snapshot_codec.encode(value)
        } else {
            value
        };
        serde_json::to_writer(&mut self.snapshots, &stored).map_err(|e| e.to_string())?;
        self.snapshots.write_all(b"\n").map_err(|e| e.to_string())?;
        self.snapshot_records += 1;
        self.last_snapshot = Some((snapshot.turn, snapshot.phase));
        self.timeline.push(
            json!({"turn":snapshot.turn,"phase":snapshot.phase,"city_count":snapshot.city_count}),
        );
        Ok(())
    }
    pub fn snapshot(
        &mut self,
        snapshot: &Snapshot,
        config: &Config,
        required: bool,
    ) -> Result<(), String> {
        self.verbose = config.report_detail == "verbose";
        self.observation_boundaries += 1;
        if config.report_detail == "lean" {
            let value = serde_json::to_value(snapshot).map_err(|e| e.to_string())?;
            let changes = self.observation_changes.observe(&value);
            if !changes.is_empty() {
                serde_json::to_writer(
                    &mut self.changes,
                    &json!({"turn":snapshot.turn,"phase":snapshot.phase,"changes":changes}),
                )
                .map_err(|e| e.to_string())?;
                self.changes.write_all(b"\n").map_err(|e| e.to_string())?;
                self.change_records += 1;
            }
        }
        if config.report_detail == "verbose" || required {
            self.persist_snapshot(snapshot)?;
        }
        Ok(())
    }
    #[cfg(feature = "desktop")]
    pub fn final_minimap(
        &mut self,
        turn: i32,
        phase: crate::sim::trace::Phase,
        map: &crate::sim::observation::MinimapSnapshot,
    ) -> Result<(), String> {
        self.minimap = super::minimap::write_final(&self.path, turn, phase, map)?;
        Ok(())
    }
    pub fn scenario(&mut self, scenario: Provenance) {
        self.scenario = scenario;
    }
    pub fn turn_profiles(&mut self, profiles: Vec<TurnProfile>) {
        self.turn_profiles.extend(profiles);
    }
    pub fn event(&mut self, event: &Event, config: &Config) -> Result<bool, String> {
        if !config.retains(event) || self.event_records >= config.max_events {
            self.omitted_events += 1;
            return Ok(false);
        }
        if config.report_detail == "lean" && self.compaction.redundant(event) {
            self.omitted_events += 1;
            return Ok(false);
        }
        let (bytes, values) = if config.report_detail == "verbose" {
            let plan = self
                .shared_values
                .plan(serde_json::to_value(event).map_err(|e| e.to_string())?)?;
            (plan.bytes, plan.values)
        } else {
            let mut bytes = serde_json::to_vec(event).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            (bytes, Vec::new())
        };
        let new_shared_bytes = values.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
        if self
            .event_bytes
            .saturating_add(self.shared_bytes)
            .saturating_add(bytes.len())
            .saturating_add(new_shared_bytes)
            > config.max_event_bytes
        {
            self.omitted_events += 1;
            return Ok(false);
        }
        for (_, bytes) in &values {
            self.shared.write_all(bytes).map_err(|e| e.to_string())?;
        }
        self.events.write_all(&bytes).map_err(|e| e.to_string())?;
        self.shared_values.commit(&values);
        self.shared_bytes += new_shared_bytes;
        self.shared_records += values.len();
        self.event_records += 1;
        self.event_bytes += bytes.len();
        self.compaction.retained(event);
        Ok(true)
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Checkpoint output includes configuration, snapshot, termination state, and dropped-event accounting."
    )]
    pub fn checkpoint(
        &mut self,
        config: &Config,
        registry: &Registry,
        last: &Snapshot,
        turns: usize,
        reason: Option<&str>,
        game_over: Option<&str>,
        dropped: u64,
    ) -> Result<(), String> {
        self.verbose = config.report_detail == "verbose";
        self.persist_snapshot(last)?;
        self.shared.flush().map_err(|e| e.to_string())?;
        self.changes.flush().map_err(|e| e.to_string())?;
        self.snapshots.flush().map_err(|e| e.to_string())?;
        self.events.flush().map_err(|e| e.to_string())?;
        let mut civilizations =
            serde_json::to_value(last.civilizations()).map_err(|e| e.to_string())?;
        if let Some(players) = civilizations.as_array_mut() {
            for player in players {
                player.as_object_mut().unwrap().remove("unit_details");
            }
        }
        let mut growth_counts = BTreeMap::<&str, usize>::new();
        for city in last.cities().map(Vec::as_slice).unwrap_or_default() {
            *growth_counts.entry(city.growth_status).or_default() += 1;
        }
        let observations = registry
            .output
            .findings
            .iter()
            .map(|f| f.observation_count)
            .sum::<u64>();
        let scenario_checks_passed = self
            .scenario
            .checks
            .iter()
            .filter(|check| check.passed)
            .count();
        let scenario_failed = self.scenario.checks.iter().any(|check| !check.passed);
        let status = if reason.is_none() {
            "running"
        } else if scenario_failed {
            "failed"
        } else {
            "completed"
        };
        let stop_reason = if reason.is_some() && scenario_failed {
            "scenario_check"
        } else {
            reason.unwrap_or("checkpoint")
        };
        let report = json!({
            "version":10,
            "run":{"status":status,"stop_reason":stop_reason,
                "configuration":config.metadata(&self.scenario.world),"turns_simulated":turns,"final_turn":last.turn,"game_over":game_over,
                "scenario":self.scenario,
                "git_commit":self.commit,"working_tree_dirty":self.dirty,"elapsed_seconds":self.started.elapsed().as_secs_f64()},
            "analytics":{"enabled":registry.descriptors(),"observation_boundaries":["initial","turn_end"],"metrics":registry.output.metrics},
            "coverage":{"collector_dropped_events":dropped,"events_not_retained":self.omitted_events,
                "events_compacted_by_kind":self.compaction.counts,
                "snapshot_boundaries_not_retained":self.observation_boundaries.saturating_sub(self.snapshot_records),
                "analyzer_event_coverage_complete":dropped==0,
                "effective_snapshot_providers":registry.providers().into_iter().map(str::to_owned).chain(config.providers.iter().cloned()).collect::<std::collections::BTreeSet<_>>(),
                "note":"Trace output filters do not filter analyzer input. Collector overflow does. Lean output suppresses repeated government/unsuccessful trade decisions and routine growth/search events; counts are separate under events_compacted_by_kind. Analyzer snapshots remain complete. No finding is not proof of correct gameplay."},
            "technology_catalog":if last.research().is_some() {crate::sim::Simulation::technology_catalog()} else {Value::Null},
            "summary":{"research":last.research(),"lifecycle":last.lifecycle(),"finding_count":registry.output.findings.len(),"finding_observations":observations,
                "open_findings":registry.output.findings.iter().filter(|f|f.status=="open").count(),
                "scenario_checks_passed":scenario_checks_passed,"scenario_checks_total":self.scenario.checks.len(),
                "final_city_count":last.city_count,"civilizations":civilizations,"city_growth_status_counts":growth_counts},
            "findings":registry.output.findings,"timeline":self.timeline,"turn_profiles":self.turn_profiles,
            "evidence":{
                "changes":{"format":"jsonl","path":self.changes_path.file_name().unwrap().to_string_lossy(),"records":self.change_records,
                    "note":"Lean inspection projections with before/after entity rows and boundary-local lifecycle/research changes. Not a replay or full state history. Verbose mode leaves this stream empty."},
                "shared_values":{"format":"jsonl","path":self.shared_path.file_name().unwrap().to_string_lossy(),"records":self.shared_records,"bytes":self.shared_bytes,
                    "note":"Verbose event payload dictionaries. Read with report_read; shared bytes count against --trace-max-bytes."},
                "snapshots":{"retention":if config.report_detail == "lean" {"initial,scenario_actions,checkpoints,findings,final"} else {"every_boundary"},"format":if self.verbose {"jsonl_snapshot_delta_v1"} else {"jsonl"},"path":self.snapshot_path.file_name().unwrap().to_string_lossy(),"records":self.snapshot_records,"lookup":"Full snapshots at retained boundaries. Findings always retain their referenced snapshot; lean mode omits other non-checkpoint boundaries. Use changes for lean inspection. report_read expands verbose baselines/deltas into full snapshots for every boundary."},
                "events":{"format":if self.verbose {"jsonl_events_shared_v1"} else {"jsonl"},"path":self.event_path.file_name().unwrap().to_string_lossy(),"records":self.event_records,"bytes":self.event_bytes,"lookup":"Use report_read --stream events to resolve verbose shared values, then select by sequence. Event turn and phase identify when the action occurred, not the next turn's snapshot."},
                "lifecycle":{"enabled":last.lifecycle().is_some(),"lookup":"Read lifecycle.changes events in the lean changes stream, or lifecycle.changes across verbose snapshot records, for unit and city histories. Totals start at lifecycle.since_turn; they do not reconstruct earlier campaign history."},
                "minimap":self.minimap,
                "finding_semantics":"First/last observed turns do not imply continuous failure. Explicit recovery closes an episode; recurrence creates a new ID. Unavailable references are marked available:false."
            }
        });
        // Evidence is flushed before publishing the summary that references it.
        let temporary = self.path.with_file_name(format!(
            "{}.tmp",
            self.path.file_name().unwrap().to_string_lossy()
        ));
        let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
        fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::trace::{Entity, Phase};
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "civcraft-trace-report-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn summary_links_streamed_evidence_and_reports_event_limits() {
        let directory = directory();
        let path = directory.join("review.json");
        let config = Config::parse(&[
            "--report".into(),
            path.to_string_lossy().into(),
            "--trace-max-events".into(),
            "1".into(),
        ])
        .unwrap();
        let mut report = Report::new(
            path.to_str().unwrap(),
            super::super::scenario::generated(config.seed, config.world).provenance,
        )
        .unwrap();
        let registry = Registry::new(vec![]).unwrap();
        let snapshot = Snapshot::new(1, Phase::Initial, 0);
        report.snapshot(&snapshot, &config, true).unwrap();
        let event = Event {
            sequence: 1,
            turn: 1,
            phase: Phase::Initial,
            source: "example".into(),
            kind: "example.observed".into(),
            schema_version: 1,
            entities: vec![Entity::City(1)],
            cause: None,
            payload: json!({}),
        };
        assert!(report.event(&event, &config).unwrap());
        assert!(!report.event(&event, &config).unwrap());
        report
            .checkpoint(&config, &registry, &snapshot, 0, None, None, 2)
            .unwrap();
        let summary: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(summary["run"]["status"], "running");
        assert_eq!(summary["coverage"]["events_not_retained"], 1);
        assert_eq!(
            summary["coverage"]["analyzer_event_coverage_complete"],
            false
        );
        let snapshot_path =
            directory.join(summary["evidence"]["snapshots"]["path"].as_str().unwrap());
        let snapshots = fs::read_to_string(snapshot_path).unwrap();
        assert_eq!(snapshots.lines().count(), 1);
        let event_path = directory.join(summary["evidence"]["events"]["path"].as_str().unwrap());
        let events = fs::read_to_string(event_path).unwrap();
        assert_eq!(events.lines().count(), 1);
        report
            .checkpoint(&config, &registry, &snapshot, 0, Some("finding"), None, 2)
            .unwrap();
        let summary: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(summary["run"]["status"], "completed");
        assert_eq!(summary["run"]["stop_reason"], "finding");
        drop(report);
        for file in fs::read_dir(&directory).unwrap() {
            fs::remove_file(file.unwrap().path()).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn lean_retains_required_evidence_and_final_checkpoint_without_every_boundary() {
        let directory = directory();
        let path = directory.join("lean.json");
        let config = Config::parse(&["--report".into(), path.to_string_lossy().into()]).unwrap();
        let mut report = Report::new(
            path.to_str().unwrap(),
            super::super::scenario::generated(config.seed, config.world).provenance,
        )
        .unwrap();
        for turn in 1..=4 {
            report
                .snapshot(
                    &Snapshot::new(turn, Phase::TurnEnd, 0),
                    &config,
                    turn == 1 || turn == 3,
                )
                .unwrap();
        }
        let registry = Registry::new(vec![]).unwrap();
        report
            .checkpoint(
                &config,
                &registry,
                &Snapshot::new(4, Phase::TurnEnd, 0),
                3,
                Some("turn_limit"),
                None,
                0,
            )
            .unwrap();
        let records = fs::read_to_string(directory.join("lean.json.snapshots.jsonl")).unwrap();
        let turns: Vec<_> = records
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["turn"].clone())
            .collect();
        assert_eq!(turns, vec![json!(1), json!(3), json!(4)]);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn verbose_dictionary_bytes_obey_event_budget_and_rejected_values_are_not_written() {
        let directory = directory();
        let path = directory.join("verbose.json");
        let config = Config::parse(&[
            "--report".into(),
            path.to_string_lossy().into(),
            "--report-detail".into(),
            "verbose".into(),
            "--trace-max-bytes".into(),
            "1024".into(),
        ])
        .unwrap();
        let mut report = Report::new(
            path.to_str().unwrap(),
            super::super::scenario::generated(config.seed, config.world).provenance,
        )
        .unwrap();
        let mut event = Event {
            sequence: 1,
            turn: 1,
            phase: Phase::Initial,
            source: "custom".into(),
            kind: "custom.event".into(),
            schema_version: 1,
            entities: vec![],
            cause: None,
            payload: json!({"data":{"text":"x".repeat(2048)}}),
        };
        assert!(!report.event(&event, &config).unwrap());
        assert_eq!(report.shared_records, 0);
        event.payload = json!({"data":{"text":"y".repeat(128)}});
        assert!(report.event(&event, &config).unwrap());
        assert_eq!(report.shared_records, 1);
        assert!(report.event_bytes + report.shared_bytes <= config.max_event_bytes);
        report.shared.flush().unwrap();
        let values = fs::read_to_string(directory.join("verbose.json.shared.jsonl")).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(values.trim()).unwrap()["id"],
            0
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
