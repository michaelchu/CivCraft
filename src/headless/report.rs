//! Streaming evidence and atomic summary checkpoints. No domain-specific checks.
use super::{config::Config, scenario::Provenance};
use crate::{
    analytics::Registry,
    sim::{observation::Snapshot, trace::Event},
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
    snapshots: BufWriter<File>,
    events: BufWriter<File>,
    started: Instant,
    commit: Option<String>,
    dirty: Option<bool>,
    snapshot_records: usize,
    event_records: usize,
    event_bytes: usize,
    pub omitted_events: usize,
    timeline: Vec<Value>,
    scenario: Provenance,
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
            path,
            snapshot_path,
            event_path,
            started: Instant::now(),
            commit: git(&["rev-parse", "HEAD"]),
            dirty: git(&["status", "--porcelain"]).map(|s| !s.is_empty()),
            snapshot_records: 0,
            event_records: 0,
            event_bytes: 0,
            omitted_events: 0,
            timeline: Vec::new(),
            scenario,
        })
    }
    pub fn snapshot(&mut self, snapshot: &Snapshot) -> Result<(), String> {
        serde_json::to_writer(&mut self.snapshots, snapshot).map_err(|e| e.to_string())?;
        self.snapshots.write_all(b"\n").map_err(|e| e.to_string())?;
        self.snapshot_records += 1;
        self.timeline.push(
            json!({"turn":snapshot.turn,"phase":snapshot.phase,"city_count":snapshot.city_count}),
        );
        Ok(())
    }
    pub fn scenario(&mut self, scenario: Provenance) {
        self.scenario = scenario;
    }
    pub fn event(&mut self, event: &Event, config: &Config) -> Result<bool, String> {
        if !config.retains(event) || self.event_records >= config.max_events {
            self.omitted_events += 1;
            return Ok(false);
        }
        let mut bytes = serde_json::to_vec(event).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        if self.event_bytes.saturating_add(bytes.len()) > config.max_event_bytes {
            self.omitted_events += 1;
            return Ok(false);
        }
        self.events.write_all(&bytes).map_err(|e| e.to_string())?;
        self.event_records += 1;
        self.event_bytes += bytes.len();
        Ok(true)
    }
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
            "version":5,
            "run":{"status":status,"stop_reason":stop_reason,
                "configuration":config.metadata(),"turns_simulated":turns,"final_turn":last.turn,"game_over":game_over,
                "scenario":self.scenario,
                "git_commit":self.commit,"working_tree_dirty":self.dirty,"elapsed_seconds":self.started.elapsed().as_secs_f64()},
            "analytics":{"enabled":registry.descriptors(),"observation_boundaries":["initial","turn_end"],"metrics":registry.output.metrics},
            "coverage":{"collector_dropped_events":dropped,"events_not_retained":self.omitted_events,
                "analyzer_event_coverage_complete":dropped==0,
                "effective_snapshot_providers":registry.providers().into_iter().map(str::to_owned).chain(config.providers.iter().cloned()).collect::<std::collections::BTreeSet<_>>(),
                "note":"Trace output filters do not filter analyzer input. Collector overflow does. No finding is not proof of correct gameplay."},
            "summary":{"finding_count":registry.output.findings.len(),"finding_observations":observations,
                "open_findings":registry.output.findings.iter().filter(|f|f.status=="open").count(),
                "scenario_checks_passed":scenario_checks_passed,"scenario_checks_total":self.scenario.checks.len(),
                "final_city_count":last.city_count,"civilizations":civilizations,"city_growth_status_counts":growth_counts},
            "findings":registry.output.findings,"timeline":self.timeline,
            "evidence":{
                "snapshots":{"format":"jsonl","path":self.snapshot_path.file_name().unwrap().to_string_lossy(),"records":self.snapshot_records,"lookup":"Select by turn; initial snapshot is before the first step. Entity IDs identify cities, civilizations, and units."},
                "events":{"format":"jsonl","path":self.event_path.file_name().unwrap().to_string_lossy(),"records":self.event_records,"bytes":self.event_bytes,"lookup":"Select by sequence. Event turn and phase identify when the action occurred, not the next turn's snapshot."},
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
            Provenance {
                kind: "generated",
                name: "test".into(),
                version: 1,
                checks: vec![],
            },
        )
        .unwrap();
        let registry = Registry::new(vec![]).unwrap();
        let snapshot = Snapshot::new(1, Phase::Initial, 0);
        report.snapshot(&snapshot).unwrap();
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
}
