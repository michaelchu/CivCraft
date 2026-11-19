use assert_cmd::{Command, cargo::cargo_bin_cmd};
use std::path::{Path, PathBuf};

fn headless() -> Command {
    cargo_bin_cmd!("headless_sim")
}

fn report_directory(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "civcraft-headless-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn read_report(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn lists_registered_scenarios() {
    let output = headless().arg("--list-scenarios").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for scenario in [
        "generated-ai",
        "city-healing",
        "settlement-ferry",
        "ai-paradrops",
    ] {
        assert!(stdout.lines().any(|line| line == scenario), "{stdout}");
    }
}

#[test]
fn rejects_conflicting_scenario_inputs() {
    let output = headless()
        .args([
            "--scenario",
            "city-healing",
            "--load-campaign",
            "ignored.save",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("mutually exclusive")
    );
}

#[test]
fn runs_a_named_scenario_and_publishes_its_checks() {
    let directory = report_directory("passing");
    let report = directory.join("report.json");
    let output = headless()
        .args([
            "--scenario",
            "settlement-ferry",
            "--turns",
            "16",
            "--report",
            report.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report_json = read_report(&report);
    assert_eq!(report_json["run"]["scenario"]["version"], 2);
    assert!(
        report_json["run"]["scenario"]["observations"]
            .as_array()
            .is_some_and(|rows| rows.len() == 16)
    );
    assert!(
        report_json["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["phase"] != "scenario_actions")
    );
    assert_eq!(report_json["summary"]["scenario_checks_passed"], 3);
    assert_eq!(report_json["summary"]["scenario_checks_total"], 3);
    assert!(
        report_json["run"]["scenario"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["passed"] == true)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_scenario_publishes_its_checks_and_exits_unsuccessfully() {
    let directory = report_directory("failing");
    let report = directory.join("report.json");
    // A short normal-turn budget must publish unmet milestones and fail,
    // independent of incidental civilization selection or seed-specific AI wants.
    let output = headless()
        .args([
            "--scenario",
            "settlement-ferry",
            "--seed",
            "18",
            "--turns",
            "1",
            "--report",
        ])
        .arg(&report)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("scenario checks failed"));
    let report = read_report(&report);
    assert_eq!(report["run"]["status"], "failed");
    assert_eq!(report["run"]["stop_reason"], "scenario_check");
    assert!(
        report["summary"]["scenario_checks_passed"]
            .as_u64()
            .unwrap()
            < 3
    );
    assert_eq!(report["summary"]["scenario_checks_total"], 3);
    assert_eq!(
        report["run"]["scenario"]["checks"][0]["name"],
        "ferry_queued"
    );
    assert_eq!(report["run"]["scenario"]["checks"][2]["passed"], false);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn completed_scenario_publishes_only_required_checks_in_its_totals() {
    let directory = report_directory("completed-scenario");
    let path = directory.join("report.json");
    let output = headless()
        .args([
            "--scenario",
            "naval-ai-assault",
            "--turns",
            "30",
            "--report",
        ])
        .arg(&path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = read_report(&path);
    assert_eq!(report["run"]["status"], "completed");
    assert_eq!(report["summary"]["scenario_checks_total"], 4);
    assert_eq!(report["summary"]["scenario_checks_passed"], 4);
    let turns = report["run"]["turns_simulated"].as_u64().unwrap();
    assert!(turns > 1);
    assert_eq!(
        report["run"]["scenario"]["observations"]
            .as_array()
            .unwrap()
            .len() as u64,
        turns,
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn shortened_scenario_publishes_unmet_milestones_and_fails() {
    let directory = report_directory("shortened-scenario");
    let path = directory.join("report.json");
    let output = headless()
        .args(["--scenario", "naval-ai-assault", "--turns", "1", "--report"])
        .arg(&path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("naval_assault_captured_target"));
    let report = read_report(&path);
    assert_eq!(report["run"]["status"], "failed");
    assert_eq!(report["run"]["stop_reason"], "scenario_check");
    assert_eq!(
        report["run"]["scenario"]["observations"][0]["completed_turns"],
        1
    );
    assert_eq!(report["summary"]["scenario_checks_total"], 4);
    assert!(
        report["run"]["scenario"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| {
                check["name"] == "naval_assault_captured_target" && check["passed"] == false
            })
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn lifecycle_snapshots_survive_disabled_event_output() {
    let directory = report_directory("lifecycle");
    let report = directory.join("report.json");
    let output = headless()
        .args([
            "--scenario",
            "lifecycle-history",
            "--turns",
            "1",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "lifecycle,units",
            "--report",
        ])
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = read_report(&report);
    assert_eq!(report["version"], 10);
    assert_eq!(report["summary"]["lifecycle"]["history_available"], true);
    assert_eq!(report["summary"]["lifecycle"]["complete_since_start"], true);
    let snapshots = directory.join(report["evidence"]["snapshots"]["path"].as_str().unwrap());
    let snapshots = std::fs::read_to_string(snapshots).unwrap();
    let snapshots: Vec<serde_json::Value> = snapshots
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        snapshots[0]["lifecycle"]["changes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        snapshots
            .iter()
            .any(|s| !s["lifecycle"]["changes"].as_array().unwrap().is_empty())
    );
    assert!(
        snapshots[0]["units"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u.get("home_city").is_some())
    );
    let events = directory.join(report["evidence"]["events"]["path"].as_str().unwrap());
    assert!(std::fs::read(events).unwrap().is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(feature = "desktop")]
#[test]
fn minimap_exports_only_one_final_image_without_map_history() {
    let directory = report_directory("minimap with spaces");
    let report = directory.join("review #1.json");
    headless()
        .args([
            "--turns",
            "2",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "minimap",
            "--report",
        ])
        .arg(&report)
        .assert()
        .success();
    let output = read_report(&report);
    let evidence = &output["evidence"]["minimap"];
    assert!(evidence.get("frames").is_none());
    assert!(evidence.get("viewer").is_none());
    assert_eq!(evidence["turn"], output["run"]["final_turn"]);
    let image = image::open(directory.join(evidence["path"].as_str().unwrap())).unwrap();
    let snapshots = std::fs::read_to_string(
        directory.join(output["evidence"]["snapshots"]["path"].as_str().unwrap()),
    )
    .unwrap();
    let snapshots: Vec<serde_json::Value> = snapshots
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(snapshots.len(), 2);
    assert!(snapshots.iter().all(|s| s.get("minimap").is_none()));
    let width = evidence["width"].as_u64().unwrap();
    let height = evidence["height"].as_u64().unwrap();
    assert_eq!(
        image.width() as u64 * height,
        image.height() as u64 * width * 2
    );
    assert!(!directory.join("review #1.json.minimaps").exists());
    assert!(!directory.join("review #1.json.minimap.html").exists());
    assert_eq!(
        std::fs::read_dir(&directory)
            .unwrap()
            .filter(|f| f
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "png"))
            .count(),
        1
    );
    let disabled = directory.join("disabled.json");
    headless()
        .args([
            "--turns",
            "1",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "none",
            "--report",
        ])
        .arg(&disabled)
        .assert()
        .success();
    assert!(read_report(&disabled)["evidence"]["minimap"].is_null());
    assert!(!directory.join("disabled.json.minimaps").exists());
    assert!(!directory.join("disabled.json.minimap.png").exists());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn world_options_validate_against_the_generator_and_report_the_effective_inputs() {
    let message = headless()
        .args(["--ocean-coverage", "60"])
        .output()
        .unwrap();
    assert!(!message.status.success());
    assert!(
        String::from_utf8_lossy(&message.stderr).contains("requires --map-generator opensimplex")
    );

    let directory = report_directory("world");
    let mut streams = Vec::new();
    for (ocean, climate, age, landform, temperature) in [
        ("60", "arid", "3", "archipelago", "cool"),
        ("80", "wet", "5", "pangaea", "warm"),
    ] {
        let report = directory.join(format!("{ocean}.json"));
        let run = headless()
            .args([
                "--seed",
                "42",
                "--turns",
                "1",
                "--map-generator",
                "opensimplex",
                "--ocean-coverage",
                ocean,
                "--climate",
                climate,
                "--world-age",
                age,
                "--landform",
                landform,
                "--temperature",
                temperature,
                "--report",
            ])
            .arg(&report)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let report = read_report(&report);
        assert_eq!(report["version"], 10);
        let config = &report["run"]["configuration"];
        assert_eq!(config["map_generator"], "opensimplex");
        assert_eq!(
            config["world_settings"]["ocean_coverage"],
            ocean.parse::<i32>().unwrap()
        );
        assert_eq!(config["world_settings"]["climate"], climate);
        assert_eq!(
            config["world_settings"]["world_age_billions"],
            age.parse::<i32>().unwrap()
        );
        assert_eq!(config["world_settings"]["landform"], landform);
        assert_eq!(config["world_settings"]["temperature"], temperature);
        let file = report["evidence"]["snapshots"]["path"].as_str().unwrap();
        streams.push(std::fs::read(directory.join(file)).unwrap());
    }
    assert_ne!(streams[0], streams[1]);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn research_snapshots_have_compact_ids_and_survive_disabled_events() {
    let directory = report_directory("research");
    let report = directory.join("research.json");
    headless()
        .args([
            "--scenario",
            "research-history",
            "--turns",
            "1",
            "--analytics",
            "none",
            "--trace-domains",
            "none",
            "--snapshot-providers",
            "research",
            "--report",
        ])
        .arg(&report)
        .assert()
        .success();
    let output = read_report(&report);
    assert_eq!(output["summary"]["scenario_checks_passed"], 4);
    assert!(
        output["technology_catalog"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["id"].is_number() && t["name"].is_string())
    );
    assert_eq!(output["summary"]["research"]["complete_since_start"], true);
    let stream = std::fs::read_to_string(
        directory.join(output["evidence"]["snapshots"]["path"].as_str().unwrap()),
    )
    .unwrap();
    let snapshots: Vec<serde_json::Value> = stream
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(
        snapshots[0]["research"]["changes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        snapshots
            .iter()
            .any(|s| !s["research"]["changes"].as_array().unwrap().is_empty())
    );
    for s in snapshots {
        assert!(s.get("technology_catalog").is_none());
        for p in s["research"]["civilizations"].as_array().unwrap() {
            assert!(
                p["known_technology_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|id| id.is_number())
            );
        }
    }
    assert!(
        std::fs::read(directory.join(output["evidence"]["events"]["path"].as_str().unwrap()))
            .unwrap()
            .is_empty()
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rejects_civcraft_only_options_in_freeciv_mode() {
    let output = headless()
        .args(["--scenario", "city-healing", "--engine", "freeciv"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown option --scenario"));
}

#[test]
fn political_analytics_require_providers_and_publish_metrics_without_event_output() {
    let directory = report_directory("politics");
    let report = directory.join("report.json");
    headless()
        .args([
            "--turns",
            "1",
            "--analytics",
            "government,diplomacy",
            "--snapshot-providers",
            "none",
            "--trace-domains",
            "none",
            "--report",
        ])
        .arg(&report)
        .assert()
        .success();
    let summary = read_report(&report);
    let providers = summary["coverage"]["effective_snapshot_providers"]
        .as_array()
        .unwrap();
    assert!(providers.contains(&serde_json::json!("governments")));
    assert!(providers.contains(&serde_json::json!("diplomacy")));
    assert!(summary["analytics"]["metrics"]["government.decisions"].is_number());
    assert_eq!(
        summary["analytics"]["metrics"]["diplomacy.event_counts_complete"],
        true
    );
    let snapshots = std::fs::read_to_string(report.with_extension("json.snapshots.jsonl")).unwrap();
    let initial: serde_json::Value =
        serde_json::from_str(snapshots.lines().next().unwrap()).unwrap();
    assert!(initial["governments"].is_array());
    assert!(initial["diplomacy"].is_array());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn compact_events_preserve_analytics_and_snapshots_with_full_audit_opt_in() {
    let directory = report_directory("event-detail");
    let mut reports = Vec::new();
    for detail in ["lean", "verbose"] {
        let path = directory.join(format!("{detail}.json"));
        headless()
            .args([
                "--seed",
                "42",
                "--turns",
                "10",
                "--analytics",
                "government,diplomacy",
                "--snapshot-providers",
                "cities,civilizations,units,lifecycle,research,governments,diplomacy",
                "--report-detail",
                detail,
                "--report",
            ])
            .arg(&path)
            .assert()
            .success();
        reports.push(read_report(&path));
    }
    let compact = &reports[0];
    let full = &reports[1];
    assert_eq!(
        compact["analytics"]["metrics"],
        full["analytics"]["metrics"]
    );
    assert_eq!(compact["findings"], full["findings"]);
    let lean = std::fs::read_to_string(directory.join("lean.json.snapshots.jsonl")).unwrap();
    let output = cargo_bin_cmd!("report_read")
        .arg(directory.join("verbose.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let verbose = String::from_utf8(output.stdout).unwrap();
    assert_eq!(lean.lines().next(), verbose.lines().next());
    assert_eq!(lean.lines().last(), verbose.lines().last());
    assert!(lean.lines().count() < verbose.lines().count());
    assert!(compact["evidence"]["changes"]["records"].as_u64().unwrap() > 0);
    assert_eq!(full["evidence"]["changes"]["records"], 0);
    assert!(
        compact["coverage"]["events_compacted_by_kind"]["government.decision"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        compact["evidence"]["events"]["bytes"].as_u64().unwrap()
            < full["evidence"]["events"]["bytes"].as_u64().unwrap()
    );
    assert_eq!(
        compact["analytics"]["metrics"]["government.ai_nations_observed"],
        4
    );
    assert_eq!(
        compact["analytics"]["metrics"]["government.nations"][0]["name"],
        "Romans"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn report_reader_expands_verbose_events_and_filters_full_snapshots() {
    let directory = report_directory("verbose-reader");
    let report = directory.join("report.json");
    headless()
        .args(["--turns", "10", "--report-detail", "verbose", "--report"])
        .arg(&report)
        .assert()
        .success();
    let summary = read_report(&report);
    assert_eq!(
        summary["evidence"]["snapshots"]["format"],
        "jsonl_snapshot_delta_v1"
    );
    assert!(
        summary["evidence"]["shared_values"]["records"]
            .as_u64()
            .unwrap()
            > 0
    );
    let output = cargo_bin_cmd!("report_read")
        .arg(&report)
        .args(["--stream", "events"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let events: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        events.len() as u64,
        summary["evidence"]["events"]["records"].as_u64().unwrap()
    );
    let decision = events
        .iter()
        .find(|event| event["kind"] == "government.decision")
        .unwrap();
    assert!(decision["payload"]["advice"]["candidates"].is_array());
    let output = cargo_bin_cmd!("report_read")
        .arg(&report)
        .args(["--turn", "5", "--phase", "turn_end"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1);
    let snapshot: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(snapshot["turn"], 5);
    assert!(snapshot["cities"].is_array());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn report_reader_handles_legacy_raw_streams_and_ignores_unpublished_tail() {
    let directory = report_directory("reader-checkpoint");
    let report = directory.join("report.json");
    let stream = directory.join("events.jsonl");
    let event = serde_json::json!({"turn":5,"phase":"ai_planning","sequence":1,"payload":{"score":105.95744323730469}});
    std::fs::write(&stream, format!("{event}\n{{incomplete unpublished record")).unwrap();
    std::fs::write(&report, serde_json::json!({"version":9,"evidence":{"events":{"format":"jsonl","path":"events.jsonl","records":1}}}).to_string()).unwrap();
    let output = cargo_bin_cmd!("report_read")
        .arg(&report)
        .args(["--stream", "events", "--sequence", "1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        event
    );
    std::fs::write(&stream, "").unwrap();
    cargo_bin_cmd!("report_read")
        .arg(&report)
        .args(["--stream", "events"])
        .assert()
        .failure();
    std::fs::remove_dir_all(directory).unwrap();
}
